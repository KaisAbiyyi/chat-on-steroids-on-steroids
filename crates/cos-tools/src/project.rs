//! Project, Git and watch owner.
use crate::files::locate;
use cos_core::{
    config::Root,
    persistence::{DurableStore, invalid, lock},
    sandbox,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
mod links;
pub struct Projects {
    durable: DurableStore,
    mutation: Arc<Mutex<()>>,
    link_watchers: Arc<Mutex<Vec<mpsc::SyncSender<()>>>>,
    sweep: Mutex<Option<links::Sweep>>,
}
impl Projects {
    pub fn open(durable: DurableStore) -> Self {
        let projects = Self {
            durable,
            mutation: Arc::new(Mutex::new(())),
            link_watchers: Arc::new(Mutex::new(vec![])),
            sweep: Mutex::new(None),
        };
        // Restore the app-owned armed-attempt clock even before a browser reconnects.
        let _ = projects.resume_expiry();
        projects
    }
    pub fn list(&self) -> io::Result<Value> {
        let value = self.durable.read("projects", true)?.unwrap_or(json!([]));
        let list = value
            .as_array()
            .filter(|a| a.len() <= 200)
            .ok_or_else(|| invalid("Project catalog is invalid"))?;
        let mut ids = BTreeSet::new();
        for p in list {
            let id = p["id"]
                .as_str()
                .filter(|s| uuid::Uuid::parse_str(s).is_ok())
                .ok_or_else(|| invalid("Project catalog is invalid"))?;
            if !ids.insert(id)
                || p["name"]
                    .as_str()
                    .is_none_or(|s| s.is_empty() || cos_core::config::js_len(s) > 160)
                || p["path"].as_str().is_none_or(str::is_empty)
                || p["createdAt"].as_f64().is_none_or(|v| v < 0.)
                || !links::valid_project(p)
            {
                return Err(invalid("Project catalog is invalid"));
            }
        }
        Ok(value)
    }
    /// Catalog paths are canonical OS paths (including Windows extended prefixes),
    /// not untrusted tool path syntax. Re-project through a currently approved root.
    pub fn workspace(&self, roots: &[Root], id: &str) -> io::Result<sandbox::Resolved> {
        let projects = self.list()?;
        let project = projects
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .ok_or_else(|| invalid("Project not found"))?;
        if project.get("remote").is_some() {
            return Err(invalid(
                "This project is in a folder on a server; this works only for projects on this computer.",
            ));
        }
        let real = std::fs::canonicalize(project["path"].as_str().unwrap())?;
        for root in roots.iter().filter(|r| r.remote.is_none()) {
            let approved = sandbox::resolve(roots, &format!("/{}", root.name), None, false)?;
            if sandbox::contained(&approved.real, &real) {
                let tail = real
                    .components()
                    .skip(approved.real.components().count())
                    .map(|p| p.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                let candidate = if tail.is_empty() {
                    format!("/{}", root.name)
                } else {
                    format!("/{}/{tail}", root.name)
                };
                let resolved = sandbox::resolve(roots, &candidate, None, false)?;
                if sandbox::native_identity(&resolved.real) == sandbox::native_identity(&real) {
                    return Ok(resolved);
                }
                return Err(invalid(
                    "Project folder changed during workspace resolution",
                ));
            }
        }
        Err(invalid("Project folder is no longer approved"))
    }
    pub fn add(&self, roots: &[Root], path: &str) -> io::Result<Value> {
        let _guard = lock(&self.mutation)?;
        let resolved = sandbox::resolve(roots, path, None, false)?;
        let location = locate(roots, path, None, false)?;
        if !location.root.metadata(&location.relative)?.is_dir() {
            return Err(invalid("Choose a project folder"));
        }
        let mut projects = self.list()?.as_array().unwrap().clone();
        if let Some(p) = projects.iter_mut().find(|p| {
            p.get("remote").is_none() && {
                sandbox::native_identity(std::path::Path::new(p["path"].as_str().unwrap()))
                    == sandbox::native_identity(&resolved.real)
            }
        }) {
            let ungrouped = p["ungrouped"] == true;
            if ungrouped {
                p.as_object_mut().unwrap().remove("ungrouped");
            }
            let result = p.clone();
            if ungrouped {
                self.durable.write_now("projects", json!(projects))?;
            }
            return Ok(result);
        }
        if projects.len() >= 200 {
            return Err(invalid("Project catalog limit reached"));
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let p = json!({"id":uuid::Uuid::new_v4().to_string(),"name":resolved.real.file_name().unwrap_or_default().to_string_lossy().chars().take(160).collect::<String>(),"path":resolved.real,"createdAt":now,"chatgpt":{"state":"requested","requestId":uuid::Uuid::new_v4().to_string(),"updatedAt":now}});
        projects.push(p.clone());
        self.durable.write_now("projects", json!(projects))?;
        Ok(p)
    }
    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        ungrouped: Option<bool>,
    ) -> io::Result<Value> {
        let _guard = lock(&self.mutation)?;
        let mut projects = self.list()?.as_array().unwrap().clone();
        let p = projects
            .iter_mut()
            .find(|p| p["id"] == id)
            .ok_or_else(|| invalid("Project not found"))?;
        if let Some(name) = name {
            if name.trim().is_empty() || cos_core::config::js_len(name) > 160 {
                return Err(invalid("Invalid project name"));
            }
            p["name"] = json!(name.trim());
        }
        if let Some(ungrouped) = ungrouped {
            p["ungrouped"] = json!(ungrouped);
        }
        let result = p.clone();
        self.durable.write_now("projects", json!(projects))?;
        Ok(result)
    }
    pub fn add_remote(
        &self,
        config: &cos_core::persistence::ConfigStore,
        address: &str,
        folder: &str,
    ) -> io::Result<Value> {
        let asked = cos_exec::ssh::parse(address, folder).map_err(invalid)?;
        let mut client = crate::remote::Sftp::connect(&asked)?;
        self.add_remote_with(config, address, folder, &mut client)
    }
    /// The supplied OpenSSH subsystem is also used by native isolated qualification.
    pub fn add_remote_with(
        &self,
        config: &cos_core::persistence::ConfigStore,
        address: &str,
        folder: &str,
        client: &mut crate::remote::Sftp,
    ) -> io::Result<Value> {
        let _guard = lock(&self.mutation)?;
        let mut remote = cos_exec::ssh::parse(address, folder).map_err(invalid)?;
        let path = remote["path"].as_str().unwrap();
        let query = if path == "~" {
            "."
        } else {
            path.strip_prefix("~/").unwrap_or(path)
        };
        let real = client.realpath(query)?;
        if client.stat(&real)?["isDirectory"] != true {
            return Err(invalid("Choose a remote project folder"));
        }
        remote["path"] = json!(real);
        let identity = cos_exec::ssh::identity(&remote);
        let mut projects = self.list()?.as_array().unwrap().clone();
        let existing = projects.iter().position(|p| {
            p.get("remote")
                .is_some_and(|r| cos_exec::ssh::identity(r) == identity)
        });
        if existing.is_none() && projects.len() >= 200 {
            return Err(invalid("Project catalog limit reached"));
        }
        let mut full = false;
        config.update(|conf| {
            let roots = conf["roots"].as_array_mut().unwrap();
            if roots.iter().any(|r| {
                r.get("remote")
                    .is_some_and(|r| cos_exec::ssh::identity(r) == identity)
            }) {
                return;
            }
            if roots.len() >= 32 {
                full = true;
                return;
            }
            let base = real
                .rsplit('/')
                .find(|p| !p.is_empty())
                .unwrap_or("server")
                .to_lowercase()
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        c
                    } else {
                        '-'
                    }
                })
                .take(24)
                .collect::<String>();
            let base = base
                .trim_start_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_string();
            let base = if base.is_empty() {
                "server".into()
            } else {
                base
            };
            roots.push(json!({"name":base,"path":identity,"remote":remote}));
        })?;
        if full {
            return Err(invalid("Folder limit reached"));
        }
        let project = if let Some(at) = existing {
            projects[at].as_object_mut().unwrap().remove("ungrouped");
            projects[at].clone()
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let p = json!({"id":uuid::Uuid::new_v4().to_string(),"name":real.rsplit('/').find(|s| !s.is_empty()).unwrap_or(&identity).chars().take(160).collect::<String>(),"path":identity,"remote":remote,"createdAt":now,"chatgpt":{"state":"requested","requestId":uuid::Uuid::new_v4().to_string(),"updatedAt":now}});
            projects.push(p.clone());
            p
        };
        self.durable.write_now("projects", json!(projects))?;
        Ok(project)
    }
}
/// Read-only Git commands use the host Git but discard inherited repository overrides,
/// external diff commands, pagers and fsmonitor hooks before launch.
pub(crate) struct GitOutput {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}
pub(crate) fn git_output(
    cwd: &std::path::Path,
    args: &[&str],
    limit: usize,
) -> io::Result<GitOutput> {
    let executable = if cfg!(windows) {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .filter(|p| p.is_absolute())
            .flat_map(|p| [p.join("git.exe"), p.join("git.com")])
            .find(|p| p.is_file())
            .ok_or_else(|| invalid("Git was not found on PATH"))?
    } else {
        std::path::PathBuf::from("git")
    };
    let mut command = Command::new(executable);
    command
        .args([
            "--no-optional-locks",
            "-c",
            "core.pager=cat",
            "-c",
            "color.ui=false",
            "-c",
            "diff.external=",
            "-c",
            "diff.trustExitCode=false",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().to_uppercase().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00000004);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let tree = match cos_exec::tree::Tree::attach(child.id(), cfg!(windows)) {
        Ok(t) => t,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };
    use std::io::Read;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();
    let max = limit.min(4 * 1024 * 1024);
    std::thread::spawn(move || {
        let mut b = vec![];
        let r = stdout.take(max as u64 + 1).read_to_end(&mut b);
        let _ = out_tx.send((r, b));
    });
    std::thread::spawn(move || {
        let mut b = vec![];
        let r = stderr.take(4 * 1024 * 1024 + 1).read_to_end(&mut b);
        let _ = err_tx.send((r, b));
    });
    let started = Instant::now();
    let status = loop {
        if tree.ready_to_reap()?
            && let Some(status) = child.try_wait()?
        {
            break status;
        }
        if started.elapsed() > Duration::from_secs(15) {
            tree.terminate();
            let _ = child.wait();
            return Err(invalid("Git read timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let (r, stdout) = out_rx
        .recv_timeout(Duration::from_secs(1))
        .map_err(|_| invalid("Git output did not settle"))?;
    r?;
    let (r, stderr) = err_rx
        .recv_timeout(Duration::from_secs(1))
        .map_err(|_| invalid("Git error output did not settle"))?;
    r?;
    if stdout.len() > max || stderr.len() > 4 * 1024 * 1024 {
        return Err(invalid("Git output exceeds limit"));
    }
    Ok(GitOutput {
        code: status.code().unwrap_or(-1),
        stdout,
        stderr: String::from_utf8_lossy(&stderr).trim().into(),
    })
}
pub fn git_command(cwd: &std::path::Path, args: &[&str]) -> io::Result<Vec<u8>> {
    let out = git_output(cwd, args, 4 * 1024 * 1024)?;
    if out.code != 0 {
        return Err(invalid(if out.stderr.is_empty() {
            "Git read failed or folder is not a repository"
        } else {
            &out.stderr
        }));
    }
    Ok(out.stdout)
}

pub fn git(roots: &[Root], path: &str) -> io::Result<Value> {
    crate::git::legacy_snapshot(roots, path)
}

pub struct Watch {
    _watcher: RecommendedWatcher,
    receiver: Mutex<mpsc::Receiver<notify::Result<notify::Event>>>,
    base: std::path::PathBuf,
    virtual_base: String,
    dropped: Arc<AtomicUsize>,
}
impl Watch {
    pub fn valid(&self, roots: &[Root]) -> bool {
        sandbox::resolve(roots, &self.virtual_base, None, false).is_ok_and(|r| {
            sandbox::native_identity(&r.real) == sandbox::native_identity(&self.base)
        })
    }
    pub fn start(roots: &[Root], path: &str) -> io::Result<Self> {
        let resolved = sandbox::resolve(roots, path, None, false)?;
        let (tx, rx) = mpsc::sync_channel(64);
        let dropped = Arc::new(AtomicUsize::new(0));
        let count = dropped.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            if tx.try_send(event).is_err() {
                count.fetch_add(1, Ordering::AcqRel);
            }
        })
        .map_err(io::Error::other)?;
        watcher
            .watch(&resolved.real, RecursiveMode::Recursive)
            .map_err(io::Error::other)?;
        Ok(Self {
            _watcher: watcher,
            receiver: Mutex::new(rx),
            base: resolved.real,
            virtual_base: resolved.virtual_path,
            dropped,
        })
    }
    pub fn poll(&self) -> Value {
        let mut paths = BTreeSet::new();
        let mut errors = 0;
        if let Ok(receiver) = self.receiver.lock() {
            for event in receiver.try_iter().take(64) {
                match event {
                    Ok(e) => {
                        for path in e.paths {
                            if let Ok(tail) = path.strip_prefix(&self.base) {
                                paths.insert(format!(
                                    "{}/{}",
                                    self.virtual_base,
                                    tail.to_string_lossy().replace('\\', "/")
                                ));
                            }
                        }
                    }
                    Err(_) => errors += 1,
                }
            }
        }
        json!({"paths":paths,"errors":errors,"dropped":self.dropped.swap(0,Ordering::AcqRel)})
    }
}
