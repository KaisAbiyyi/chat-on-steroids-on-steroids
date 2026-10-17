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
pub struct Projects {
    durable: DurableStore,
    mutation: Mutex<()>,
}
impl Projects {
    pub fn open(durable: DurableStore) -> Self {
        Self {
            durable,
            mutation: Mutex::new(()),
        }
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
            {
                return Err(invalid("Project catalog is invalid"));
            }
        }
        Ok(value)
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
pub fn git_command(cwd: &std::path::Path, args: &[&str]) -> io::Result<Vec<u8>> {
    let mut command = Command::new("git");
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
    command.env("GIT_OPTIONAL_LOCKS", "0");
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
            return Err(e);
        }
    };
    use std::io::Read;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = vec![];
        let r = stdout.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes);
        let _ = tx.send((r, bytes));
    });
    std::thread::spawn(move || {
        let mut bytes = vec![];
        let _ = stderr.take(8192).read_to_end(&mut bytes);
    });
    let started = Instant::now();
    loop {
        if let Some(status) = if tree.ready_to_reap()? {
            child.try_wait()?
        } else {
            None
        } {
            if !status.success() {
                return Err(invalid("Git read failed or folder is not a repository"));
            }
            break;
        }
        if started.elapsed() > Duration::from_secs(15) {
            tree.terminate();
            let _ = child.kill();
            let _ = child.wait();
            return Err(invalid("Git read timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let (r, bytes) = rx
        .recv_timeout(Duration::from_secs(1))
        .map_err(|_| invalid("Git output did not settle"))?;
    r?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(invalid("Git output exceeds limit"));
    }
    Ok(bytes)
}
pub fn git(roots: &[Root], path: &str) -> io::Result<Value> {
    let resolved = sandbox::resolve(roots, path, None, false)?;
    let bytes = git_command(
        &resolved.real,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut records = bytes.split(|b| *b == 0).filter(|p| !p.is_empty());
    let mut files = vec![];
    while let Some(row) = records.next() {
        if row.len() < 3 {
            continue;
        }
        let status = String::from_utf8_lossy(&row[..2]);
        let path = String::from_utf8_lossy(&row[3..]);
        let mut item = json!({"path":path,"status":status,"staged":row[0]!=b' '&&row[0]!=b'?',"untracked":status=="??"});
        if matches!(row[0], b'R' | b'C') || matches!(row[1], b'R' | b'C') {
            item["originalPath"] = json!(records.next().map(String::from_utf8_lossy));
        }
        files.push(item);
        if files.len() >= 2000 {
            break;
        }
    }
    let branch = git_command(
        &resolved.real,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
    )
    .ok()
    .map(|s| String::from_utf8_lossy(&s).trim().to_string());
    let revision = git_command(&resolved.real, &["rev-parse", "--verify", "HEAD"])
        .ok()
        .map(|s| String::from_utf8_lossy(&s).trim().to_string());
    Ok(
        json!({"status":"ready","branch":branch,"revision":revision,"truncated":files.len()>=2000,"files":files}),
    )
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
