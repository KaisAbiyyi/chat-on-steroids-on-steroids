use cos_core::{
    config::{self, Root},
    persistence::{ConfigStore, DurableStore, invalid, lock},
    sandbox,
};
use cos_exec::{
    policy,
    process::{Launch, Manager},
    shell, ssh,
};
use cos_store::{
    SessionStore,
    access::{ConversationPolicy, WorkerOwner},
    correlation::{Correlation, Correlations},
    recorder::Recorder,
    usage,
};
use cos_tools::{
    files::{self, Files},
    project::{Projects, Watch},
    project_files::ProjectFiles,
    remote::RemoteFiles,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Core,
    Desktop,
    Plugins,
}
impl Surface {
    pub fn id(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Desktop => "desktop",
            Self::Plugins => "plugins",
        }
    }
}
pub struct Kernel {
    pub config: Arc<ConfigStore>,
    pub durable: DurableStore,
    pub sessions: SessionStore,
    pub correlations: Arc<Correlations>,
    pub policy: Arc<ConversationPolicy>,
    pub recorder: Arc<Recorder>,
    pub processes: Manager,
    pub files: Files,
    pub projects: Projects,
    /// Human file-panel actions; these do not add model-facing tool grants.
    pub project_files: ProjectFiles,
    pub tunnel: crate::tunnel::Supervisor,
    pub cancel: cos_core::runtime::Cancellation,
    exposed: Mutex<BTreeSet<String>>,
    find_exposed: bool,
    worker: Mutex<Option<PathBuf>>,
    remote: Mutex<BTreeMap<String, Arc<Mutex<RemoteFiles>>>>,
    trusted_ssh_paths: std::sync::atomic::AtomicBool,
    plans: Mutex<()>,
    watches: Mutex<BTreeMap<String, (String, Watch)>>,
}
fn platform() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}
pub fn fail(message: impl Into<String>) -> Value {
    json!({"content":[{"type":"text","text":message.into()}],"isError":true})
}
pub fn ok(message: impl Into<String>) -> Value {
    json!({"content":[{"type":"text","text":message.into()}]})
}
impl Kernel {
    pub fn new(config: Arc<ConfigStore>) -> io::Result<Arc<Self>> {
        let durable = DurableStore::open(config.directory())?;
        let store = SessionStore::open(config.directory())?;
        let correlations = Arc::new(Correlations::restore(durable.clone(), &store)?);
        let policy = Arc::new(ConversationPolicy::restore(durable.clone())?);
        let recorder = Arc::new(Recorder::new(store.clone(), correlations.clone()));
        Ok(Self::from_parts(
            config,
            durable,
            store,
            correlations,
            policy,
            recorder,
            cos_core::runtime::Cancellation::default(),
        ))
    }
    pub fn from_parts(
        config: Arc<ConfigStore>,
        durable: DurableStore,
        sessions: SessionStore,
        correlations: Arc<Correlations>,
        policy: Arc<ConversationPolicy>,
        recorder: Arc<Recorder>,
        cancel: cos_core::runtime::Cancellation,
    ) -> Arc<Self> {
        let caps = config::effective_capabilities(&config.get(), platform());
        let find_exposed = caps["command"] != true && caps["search"] == true;
        Arc::new(Self {
            config,
            projects: Projects::open(durable.clone()),
            project_files: ProjectFiles::default(),
            tunnel: crate::tunnel::Supervisor::default(),
            durable,
            sessions,
            correlations,
            policy,
            recorder,
            cancel,
            processes: Manager::default(),
            files: Files::default(),
            exposed: Mutex::new(BTreeSet::new()),
            find_exposed,
            worker: Mutex::new(None),
            remote: Mutex::new(BTreeMap::new()),
            trusted_ssh_paths: std::sync::atomic::AtomicBool::new(false),
            plans: Mutex::new(()),
            watches: Mutex::new(BTreeMap::new()),
        })
    }
    pub fn set_worker(&self, path: PathBuf) -> io::Result<()> {
        *lock(&self.worker)? = Some(path);
        Ok(())
    }
    /// Host-only opt-in. SFTP v3 cannot attest an opened handle's canonical path;
    /// the operator must trust the remote filesystem for this runtime's lifetime.
    pub fn trust_ssh_server_paths_for_lifetime(&self) {
        self.trusted_ssh_paths
            .store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn roots(&self) -> io::Result<Vec<Root>> {
        serde_json::from_value(self.config.get()["roots"].clone()).map_err(io::Error::other)
    }
    pub fn project_directory(&self, id: &str, path: &str) -> io::Result<Value> {
        self.project_file_read(id, path, false)
    }
    pub fn project_preview(&self, id: &str, path: &str) -> io::Result<Value> {
        self.project_file_read(id, path, true)
    }
    pub fn project_file_snapshot(
        &self,
        id: &str,
        path: &str,
    ) -> io::Result<Option<cos_tools::project_files::ServerSnapshot>> {
        self.with_project_files(id, |project, roots, check| {
            let Some(remote) = project.get("remote") else {
                return Ok(None);
            };
            let root = roots
                .iter()
                .find(|root| {
                    root.remote
                        .as_ref()
                        .is_some_and(|value| ssh::identity(value) == ssh::identity(remote))
                })
                .ok_or_else(|| invalid("Remote project root is no longer approved"))?;
            self.with_remote(root, |client| {
                ProjectFiles::server_snapshot(client, path, check).map(Some)
            })
        })
    }
    fn project_file_read(&self, id: &str, path: &str, preview: bool) -> io::Result<Value> {
        self.with_project_files(id, |project, roots, check| {
            if let Some(remote) = project.get("remote") {
                let root = roots
                    .iter()
                    .find(|root| {
                        root.remote
                            .as_ref()
                            .is_some_and(|value| ssh::identity(value) == ssh::identity(remote))
                    })
                    .ok_or_else(|| invalid("Remote project root is no longer approved"))?;
                self.with_remote(root, |client| {
                    ProjectFiles::server_read(
                        client,
                        id,
                        project["name"].as_str().unwrap(),
                        path,
                        preview,
                        check,
                    )
                })
            } else if preview {
                self.project_files.preview(&self.projects, roots, id, path)
            } else {
                self.project_files.list(&self.projects, roots, id, path)
            }
        })
    }
    fn with_project_files<T>(
        &self,
        id: &str,
        work: impl FnOnce(&Value, &[Root], &dyn Fn() -> io::Result<()>) -> io::Result<T>,
    ) -> io::Result<T> {
        let roots = self.roots()?;
        let project = self
            .projects
            .list()?
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .cloned()
            .ok_or_else(|| invalid("Project not found"))?;
        let check = || {
            self.cancel.check()?;
            if self.roots()? != roots
                || !self.projects.list()?.as_array().unwrap().contains(&project)
            {
                return Err(invalid("Project folder is no longer approved"));
            }
            Ok(())
        };
        check()?;
        let result = work(&project, &roots, &check)?;
        check()?;
        Ok(result)
    }
    pub fn tools(&self, surface: Surface) -> Vec<Value> {
        if surface != Surface::Core {
            return vec![];
        }
        let conf = self.config.get();
        let caps = config::effective_capabilities(&conf, platform());
        let live = |k: &str| caps[k] == true;
        let reading = live("read") || live("browse") || live("metadata");
        let mut e = self.exposed.lock().unwrap();
        if reading {
            e.insert("read".into());
            e.insert("usage".into());
        }
        if live("read") {
            e.insert("view_image".into());
        }
        if ["create", "edit", "move", "deleteFile"]
            .iter()
            .any(|k| live(k))
        {
            e.insert("apply_patch".into());
        }
        if live("command") {
            e.insert("exec_command".into());
            e.insert("write_stdin".into());
        }
        if self.find_exposed {
            e.insert("find".into());
        }
        if conf["sessions"]["record"] == true {
            e.insert("update_plan".into());
        }
        if !e.is_empty() && self.worker.lock().unwrap().is_some() {
            e.insert("exec".into());
        }
        let declarations: Vec<Value> = serde_json::from_str(include_str!(
            "../../../fixtures/upstream/phase4-6/tool-declarations.json"
        ))
        .unwrap();
        let mut result = vec![];
        for name in [
            "read",
            "view_image",
            "find",
            "apply_patch",
            "exec_command",
            "write_stdin",
            "update_plan",
            "usage",
            "exec",
        ] {
            if !e.contains(name) {
                continue;
            }
            let d = declarations.iter().find(|v| v["name"] == name).cloned();
            if let Some(mut d) = d {
                if name == "read" {
                    let paths = conf["roots"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|v| v["name"].as_str())
                        .map(|n| format!("/{n}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    if let Some(description) =
                        d["inputSchema"]["properties"]["paths"]["description"].as_str()
                    {
                        d["inputSchema"]["properties"]["paths"]["description"] =
                            json!(description.replace("/project", &paths));
                    }
                }
                result.push(d);
            }
        }
        result
    }
    pub fn admit(&self, request: &str) -> io::Result<Option<Correlation>> {
        for provisional in self.processes.provisional_requests()? {
            if let Some(owner) = self.correlations.get(&provisional)? {
                self.processes.promote(&provisional, &owner.session_id)?;
            }
        }
        if request.is_empty() || config::js_len(request) > 200 {
            return Err(invalid(
                "CALLER_IDENTITY_REQUIRED: a request-scoped caller is required",
            ));
        }
        let owner = self.correlations.get(request)?;
        let conf = self.config.get();
        if !self.policy.allowed(
            owner.as_ref().map(|o| o.conversation_id.as_str()),
            conf["multiAgent"]["strictChatAllowlist"] == true,
            &self.sessions,
            |_| WorkerOwner::default(),
        )? {
            return Err(invalid(
                "CHAT_ACCESS_REFUSED: this conversation is blocked or not trusted",
            ));
        }
        if let Some(owner) = &owner {
            let summary = self
                .sessions
                .summary(&owner.session_id)?
                .ok_or_else(|| invalid("CALLER_IDENTITY_REQUIRED: session epoch is unavailable"))?;
            if summary["conversationId"] != owner.conversation_id
                || summary["origin"]["kind"] == "worker"
            {
                return Err(invalid(
                    "CALLER_IDENTITY_REQUIRED: stale conversation or unavailable worker owner",
                ));
            }
            self.processes.promote(request, &owner.session_id)?;
            self.attach_request_plan(owner)?;
        } else if conf["multiAgent"]["allowUnattributedCalls"] != true {
            return Err(invalid(
                "CALLER_IDENTITY_REQUIRED: exact companion chat/session proof or Allow unattributed calls is required. No tool ran.",
            ));
        }
        Ok(owner)
    }
    fn attach_request_plan(&self, owner: &Correlation) -> io::Result<()> {
        let _guard = lock(&self.plans)?;
        if let Some(mut plans) = self.durable.read("request-plans", true)? {
            let map = plans
                .as_object_mut()
                .ok_or_else(|| invalid("Request plan ledger is invalid"))?;
            if let Some(mut plan) = map.remove(&owner.request_id) {
                let started = plan["updatedAt"]
                    .as_i64()
                    .ok_or_else(|| invalid("Invalid request plan clock"))?;
                plan.as_object_mut().unwrap().remove("updatedAt");
                self.sessions.update_plan(
                    &owner.session_id,
                    &owner.conversation_id,
                    plan,
                    started,
                    None,
                )?;
                self.durable.write_now("request-plans", plans)?;
            }
        }
        Ok(())
    }
    pub fn invoke(
        self: &Arc<Self>,
        surface: Surface,
        request: &str,
        name: &str,
        args: Value,
    ) -> Value {
        self.invoke_recorded(surface, request, name, args, None)
    }
    fn invoke_recorded(
        self: &Arc<Self>,
        surface: Surface,
        request: &str,
        name: &str,
        args: Value,
        parent: Option<&str>,
    ) -> Value {
        let call_id = uuid::Uuid::new_v4().to_string();
        let elapsed = std::time::Instant::now();
        let args = if name == "update_plan" {
            match cos_store::store::validate_plan(args, false) {
                Ok(value) => value,
                Err(e) => return fail(e.to_string()),
            }
        } else {
            args
        };
        let started = chrono::Utc::now().timestamp_millis();
        let result = (|| -> io::Result<Value> {
            self.cancel.check()?;
            let tools = self.tools(surface);
            let declaration = tools
                .iter()
                .find(|t| t["name"] == name)
                .ok_or_else(|| invalid(format!("Tool {name} not found")))?;
            crate::schema::validate(&declaration["inputSchema"], &args).map_err(|e| {
                invalid(format!(
                    "Input validation error: Invalid arguments for tool {name}: {e}"
                ))
            })?;
            let owner = self.admit(request)?;
            self.execute(
                surface,
                request,
                owner.as_ref(),
                name,
                args.clone(),
                &call_id,
            )
        })();
        let mut value = match result {
            Ok(v) => v,
            Err(e) => fail(cos_core::logging::redact_credentials(&e.to_string())),
        };
        if surface == Surface::Core
            && config::effective_capabilities(&self.config.get(), platform())["command"] == true
            && let Ok(owner) = self.admit(request)
        {
            let principal = owner
                .map(|o| format!("session:{}", o.session_id))
                .unwrap_or_else(|| format!("request:{request}"));
            if let Ok(sessions) = self.processes.snapshot(&principal) {
                let own = value["structuredContent"]["session_id"]
                    .as_i64()
                    .or_else(|| value["structuredContent"]["completed_session_id"].as_i64());
                let waiting = sessions
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|s| {
                        s["unread"] == true
                            && s["publicationPending"] != true
                            && s["session_id"].as_i64() != own
                    })
                    .take(20)
                    .map(|s| {
                        format!(
                            "{} ({})",
                            s["session_id"],
                            s["exit_code"]
                                .as_i64()
                                .map(|code| format!("completed, exit {code}"))
                                .unwrap_or("running".into())
                        )
                    })
                    .collect::<Vec<_>>();
                if !waiting.is_empty()
                    && let Some(content) = value["content"].as_array_mut()
                {
                    content.push(json!({"type":"text","text":format!("Background process sessions with unread output: {}. Read them with write_stdin and empty chars; no command is rerun.",waiting.join(", "))}));
                }
            }
        }
        self.recorder
            .set_enabled(self.config.get()["sessions"]["record"] == true);
        let outcome = if value["isError"] == true {
            "tool_rejected"
        } else if value["structuredContent"]["exit_code"]
            .as_i64()
            .is_some_and(|c| c != 0)
        {
            "process_exit_nonzero"
        } else {
            "ok"
        };
        if let Err(e) = self.recorder.record_call_details(
            request,
            name,
            args,
            &value.to_string(),
            started,
            outcome,
            Some(&cos_store::recorder::ToolRecordContext {
                call_id,
                parent_call_id: parent.map(str::to_owned),
                duration_ms: elapsed.elapsed().as_millis() as u64,
            }),
        ) {
            return fail(format!(
                "TOOL_RECORDING_FAILED: operation result retained by its owner; recording did not settle: {}",
                cos_core::logging::redact_credentials(&e.to_string())
            ));
        }
        value
    }
    fn cwd(
        &self,
        roots: &[Root],
        provided: Option<&str>,
        owner: Option<&Correlation>,
    ) -> io::Result<String> {
        if let Some(path) = provided {
            return Ok(path.into());
        }
        if let Some(owner) = owner
            && let Some(summary) = self.sessions.summary(&owner.session_id)?
            && let Some(id) = summary["projectId"].as_str()
        {
            let list = self.projects.list()?;
            if let Some(project) = list.as_array().unwrap().iter().find(|p| p["id"] == id) {
                if let Some(remote) = project.get("remote") {
                    return roots
                        .iter()
                        .find(|r| r.remote.as_ref() == Some(remote))
                        .map(|r| format!("/{}", r.name))
                        .ok_or_else(|| invalid("Remote project root is no longer approved"));
                }
                return Ok(self.projects.workspace(roots, id)?.virtual_path);
            }
            return Err(invalid("The session project is unavailable"));
        }
        roots
            .first()
            .map(|r| format!("/{}", r.name))
            .ok_or_else(|| invalid("No folder is approved"))
    }
    pub(crate) fn remote_root<'a>(
        &self,
        roots: &'a [Root],
        path: &str,
    ) -> Option<(&'a Root, String)> {
        roots
            .iter()
            .filter(|r| r.remote.is_some())
            .find_map(|root| {
                let prefix = format!("/{}", root.name);
                if path == prefix {
                    Some((root, String::new()))
                } else {
                    path.strip_prefix(&format!("{prefix}/"))
                        .map(|tail| (root, tail.to_string()))
                }
            })
    }
    pub fn with_remote<T>(
        &self,
        root: &Root,
        work: impl FnOnce(&mut RemoteFiles) -> io::Result<T>,
    ) -> io::Result<T> {
        if !self
            .trusted_ssh_paths
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "SSH_PATH_TRUST_REQUIRED: SFTP v3 cannot prove opened-handle containment. A human operator may opt in for this runtime with --trust-ssh-server-paths only when the server filesystem is trusted.",
            ));
        }
        let remote = root
            .remote
            .as_ref()
            .ok_or_else(|| invalid("Not a remote root"))?;
        let key = remote.to_string();
        self.cancel.check()?;
        if !self.roots()?.contains(root) {
            return Err(invalid("Remote root is no longer approved"));
        }
        let cached = lock(&self.remote)?.get(&key).cloned();
        let cached = cached.filter(|s| s.lock().is_ok_and(|s| !s.sftp.closed()));
        let shared = if let Some(shared) = cached {
            shared
        } else {
            let connected = Arc::new(Mutex::new(RemoteFiles::connect(remote)?));
            self.cancel.check()?;
            if !self.roots()?.contains(root) {
                return Err(invalid("Remote root was removed during connection"));
            }
            let mut known = lock(&self.remote)?;
            if known.len() >= 8 && !known.contains_key(&key) {
                return Err(invalid("Too many active SSH roots"));
            }
            known.insert(key, connected.clone());
            connected
        };
        work(&mut *lock(&shared)?)
    }
    fn execute(
        self: &Arc<Self>,
        surface: Surface,
        request: &str,
        owner: Option<&Correlation>,
        name: &str,
        args: Value,
        call_id: &str,
    ) -> io::Result<Value> {
        let conf = self.config.get();
        let caps = config::effective_capabilities(&conf, platform());
        let enabled = |k: &str| caps[k] == true;
        let require = |k: &str| {
            self.cancel.check()?;
            self.admit(request)?;
            if config::effective_capabilities(&self.config.get(), platform())[k] == true {
                Ok(())
            } else {
                Err(invalid(format!(
                    "TOOL_DISABLED: {name} is disabled by the current Chat On Steroids permissions."
                )))
            }
        };
        let roots = self.roots()?;
        match name {
            "read" => self.read_tool(&roots, &args, &caps, request),
            "view_image" => {
                require("read")?;
                let path = args["path"].as_str().unwrap();
                let raw = if let Some((root, tail)) = self.remote_root(&roots, path) {
                    self.with_remote(root, |r| r.read(&tail, cos_tools::image::MAX_BYTES))?
                } else {
                    files::locate(&roots, path, None, false)?
                        .open(cos_tools::image::MAX_BYTES as u64)?
                };
                require("read")?;
                if self.roots()? != roots {
                    return Err(invalid("Workspace changed while reading image"));
                }
                Ok(json!({"content":[cos_tools::image::content(&raw)?]}))
            }
            "find" => {
                require("search")?;
                if args["regex"] == true && args["mode"] != "content" {
                    return Err(invalid("regex=true is only valid with mode=content"));
                }
                let base = self.cwd(&roots, args["path"].as_str(), owner)?;
                let excludes = args["exclude"].as_array().map(|v| {
                    v.iter()
                        .map(|s| s.as_str().unwrap().into())
                        .collect::<Vec<String>>()
                });
                let report = cos_tools::search::find_checked(
                    &self.files,
                    &roots,
                    &base,
                    &cos_tools::search::Options {
                        query: args["query"].as_str().unwrap(),
                        content: args["mode"] == "content",
                        glob: args["include"].as_str(),
                        exclude: excludes.as_deref(),
                        limit: args["max_results"].as_u64().unwrap_or(50) as usize,
                        case_sensitive: args["case_sensitive"] == true,
                        regex: args["regex"] == true,
                    },
                    &|| {
                        require("search")?;
                        if self.roots()? != roots {
                            return Err(invalid("Workspace changed during search"));
                        }
                        Ok(())
                    },
                )?;
                Ok(
                    json!({"content":[{"type":"text","text":report.to_string()}],"structuredContent":report}),
                )
            }
            "apply_patch" => {
                let source = args["patch"].as_str().unwrap();
                let base = self.cwd(&roots, None, owner)?;
                self.patch_tool(&roots, &base, source, request, owner)
            }
            "exec_command" => {
                require("command")?;
                if args.get("cmd").is_some() == args.get("cmds").is_some() {
                    return Err(invalid("exec_command requires exactly one of cmd or cmds"));
                }
                let commands = if let Some(cmd) = args["cmd"].as_str() {
                    vec![cmd.to_owned()]
                } else {
                    args["cmds"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().into())
                        .collect::<Vec<String>>()
                };
                for key in ["yield_time_ms", "max_output_tokens"] {
                    unsigned_arg(&args, key)?;
                }
                let base = self.cwd(&roots, args["workdir"].as_str(), owner)?;
                let remote = self.remote_root(&roots, &base);
                let s = if remote.is_some() {
                    shell::Shell {
                        kind: "sh".into(),
                        path: ssh::program(),
                    }
                } else {
                    shell::detect(args["shell"].as_str()).map_err(invalid)?
                };
                let decision = policy::evaluate(&conf["commandAllowlist"], &commands, &s.kind);
                if decision["allowed"] != true {
                    return Err(invalid(format!(
                        "COMMAND_POLICY_REFUSED: {}",
                        decision["detail"].as_str().unwrap_or("Invalid policy")
                    )));
                }
                let principal = owner
                    .map(|o| format!("session:{}", o.session_id))
                    .unwrap_or_else(|| format!("request:{request}"));
                let obligations = self.processes.snapshot(&principal)?;
                let unread = obligations
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|s| s["exit_code"].is_number() && s["unread"] == true)
                    .map(|s| s["session_id"].to_string())
                    .collect::<Vec<_>>();
                if unread.len() >= 8 {
                    return Err(invalid(format!(
                        "EXEC_RESULTS_UNREAD: completed background results are waiting. Read IDs {} with write_stdin before launching another command. No child was spawned.",
                        unread.join(", ")
                    )));
                }
                let marker = if args["cmds"].is_array() {
                    Some(uuid::Uuid::new_v4().simple().to_string()[..24].to_owned())
                } else {
                    None
                };
                let command = marker
                    .as_ref()
                    .map(|m| cos_exec::batch::compose(&commands, &s.kind, m))
                    .unwrap_or_else(|| commands[0].clone());
                let (program, argv, cwd, raw) = if let Some((root, tail)) = remote {
                    let path = self.with_remote(root, |r| r.resolve(&tail, false))?;
                    let remote = root.remote.as_ref().unwrap();
                    (
                        ssh::program(),
                        ssh::args(
                            remote,
                            &if args["tty"] == true {
                                vec!["-tt".into()]
                            } else {
                                vec![]
                            },
                            &[ssh::command(&path, &command)],
                        ),
                        std::env::current_dir()?,
                        false,
                    )
                } else {
                    let path = sandbox::resolve(&roots, &base, None, false)?.real;
                    (
                        s.path.clone(),
                        s.args(&command, args["login"].as_bool().unwrap_or(!cfg!(windows))),
                        path,
                        s.kind == "cmd",
                    )
                };
                let launch = Launch {
                    owner: &principal,
                    program: &program,
                    args: &argv,
                    cwd: &cwd,
                    tty: args["tty"] == true,
                    raw_cmd: raw,
                };
                let wait = Duration::from_millis(
                    args["yield_time_ms"]
                        .as_u64()
                        .unwrap_or(10000)
                        .clamp(250, 30000),
                );
                require("command")?;
                if self.roots()? != roots {
                    return Err(invalid("Workspace changed before command launch"));
                }
                let output = self.processes.launch_with_limit(
                    launch,
                    wait,
                    marker.as_deref(),
                    args["max_output_tokens"]
                        .as_u64()
                        .unwrap_or(10000)
                        .min(cos_exec::process::OUTPUT_BYTES as u64 / 4)
                        as usize,
                )?;
                Ok(exec_result(output))
            }
            "write_stdin" => {
                require("command")?;
                unsigned_arg(&args, "yield_time_ms")?;
                unsigned_arg(&args, "max_output_tokens")?;
                let id = args["session_id"]
                    .as_f64()
                    .filter(|x| x.fract() == 0. && *x >= i32::MIN as f64 && *x <= i32::MAX as f64)
                    .ok_or_else(|| invalid("Invalid process session_id"))?
                    as i32;
                let principal = owner
                    .map(|o| format!("session:{}", o.session_id))
                    .unwrap_or_else(|| format!("request:{request}"));
                let chars = args["chars"].as_str().unwrap_or("");
                let wait = args["yield_time_ms"]
                    .as_u64()
                    .unwrap_or(if chars.is_empty() { 5000 } else { 250 });
                Ok(exec_result(
                    self.processes.poll_with_limit(
                        id,
                        &principal,
                        chars,
                        Duration::from_millis(
                            wait.clamp(250, if chars.is_empty() { 300000 } else { 30000 }),
                        ),
                        args["max_output_tokens"]
                            .as_u64()
                            .unwrap_or(10000)
                            .min(cos_exec::process::OUTPUT_BYTES as u64 / 4)
                            as usize,
                    )?,
                ))
            }
            "update_plan" => {
                if conf["sessions"]["record"] != true {
                    return Err(invalid("TOOL_DISABLED: Session recording is disabled"));
                }
                let started = chrono::Utc::now().timestamp_millis();
                if let Some(owner) = owner {
                    let accepted = self.sessions.update_plan(
                        &owner.session_id,
                        &owner.conversation_id,
                        args,
                        started,
                        None,
                    )?;
                    Ok(if accepted {
                        ok("Plan updated")
                    } else {
                        fail(
                            "This plan update is stale or its chat was replaced. The current plan was preserved.",
                        )
                    })
                } else {
                    let _guard = lock(&self.plans)?;
                    let mut plans = self
                        .durable
                        .read("request-plans", true)?
                        .unwrap_or(json!({}));
                    if plans.as_object().is_none_or(|m| m.len() > 200) {
                        return Err(invalid("Request plan ledger exceeds limit"));
                    }
                    let mut plan = args;
                    plan["updatedAt"] = json!(started);
                    plans[request] = plan;
                    self.durable.write_now("request-plans", plans)?;
                    Ok(ok(
                        "Plan updated for this request and will attach when its chat identity arrives.",
                    ))
                }
            }
            "usage" => {
                if !["read", "browse", "metadata"].iter().any(|k| enabled(k)) {
                    return Err(invalid("TOOL_DISABLED: usage is disabled"));
                }
                let limits = usage::ObservedLimits::default()
                    .current(chrono::Utc::now().timestamp_millis())?;
                let report = json!({"chatgpt":{"limits":limits,"reported":false},"codex":null,"claude":null,"source":"Provider observations and explicitly enabled local usage logs","notReported":["ChatGPT limits have not been observed by the phase 7 browser owner","Provider logs have not been enabled for this host"]});
                Ok(
                    json!({"content":[{"type":"text","text":"Usage and limits: not reported. No provider observations or enabled local-log readings are available. Estimates are not reported as quota."}],"structuredContent":report}),
                )
            }
            "exec" => {
                let executable = lock(&self.worker)?
                    .clone()
                    .ok_or_else(|| invalid("CODE_MODE_RUNTIME_UNAVAILABLE"))?;
                let tools = self
                    .tools(surface)
                    .into_iter()
                    .filter(|v| v["name"] != "exec")
                    .collect::<Vec<_>>();
                let host = self.clone();
                let request = request.to_owned();
                let parent = call_id.to_owned();
                let invoke = Arc::new(move |name: &str, args: Value| {
                    host.invoke_recorded(surface, &request, name, args, Some(&parent))
                });
                Ok(crate::worker::run(
                    &executable,
                    args["code"].as_str().unwrap(),
                    &tools,
                    invoke,
                    crate::code::Limits::default(),
                ))
            }
            _ => Err(invalid(format!("Tool {name} not found"))),
        }
    }
    pub fn watch(&self, owner: &str, path: &str) -> io::Result<String> {
        self.cancel.check()?;
        if owner.is_empty()
            || config::effective_capabilities(&self.config.get(), platform())["browse"] != true
        {
            return Err(invalid("Folder watch permission is disabled"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let watch = Watch::start(&self.roots()?, path)?;
        let mut watches = lock(&self.watches)?;
        if watches.len() >= 32 {
            return Err(invalid("Too many active file watchers"));
        }
        watches.insert(id.clone(), (owner.into(), watch));
        Ok(id)
    }
    pub fn watch_poll(&self, owner: &str, id: &str) -> io::Result<Value> {
        self.cancel.check()?;
        let mut watches = lock(&self.watches)?;
        let (held, watch) = watches.get(id).ok_or_else(|| invalid("Unknown watcher"))?;
        if held != owner {
            return Err(invalid("Watcher belongs to another session"));
        }
        if config::effective_capabilities(&self.config.get(), platform())["browse"] != true
            || !watch.valid(&self.roots()?)
        {
            watches.remove(id);
            return Err(invalid("Folder watch authority was revoked"));
        }
        Ok(watch.poll())
    }
    pub fn watch_close(&self, owner: &str, id: &str) -> io::Result<()> {
        let mut watches = lock(&self.watches)?;
        if watches.get(id).is_none_or(|(held, _)| held != owner) {
            return Err(invalid("Watcher belongs to another session"));
        }
        watches.remove(id);
        Ok(())
    }
    pub fn stop_services(&self) -> io::Result<()> {
        self.cancel.cancel();
        self.tunnel.stop()?;
        self.processes.shutdown();
        lock(&self.watches)?.clear();
        lock(&self.remote)?.clear();
        Ok(())
    }
    pub fn shutdown(&self) -> io::Result<()> {
        self.stop_services()?;
        self.sessions.flush()?;
        self.durable.flush()
    }
}
fn unsigned_arg(args: &Value, key: &str) -> io::Result<()> {
    if let Some(v) = args.get(key)
        && v.as_f64()
            .is_none_or(|n| n < 0. || n.fract() != 0. || n > 9007199254740991.)
    {
        return Err(invalid(format!("{key} must be a nonnegative safe integer")));
    }
    Ok(())
}
fn exec_result(output: Value) -> Value {
    let mut sections = vec![];
    if let Some(id) = output["chunk_id"].as_str().filter(|s| !s.is_empty()) {
        sections.push(format!("Chunk ID: {id}"));
    }
    sections.push(format!(
        "Wall time: {:.4} seconds",
        output["wall_time_seconds"].as_f64().unwrap_or(0.)
    ));
    if let Some(exit) = output["exit_code"].as_i64() {
        sections.push(format!("Process exited with code {exit}"));
    }
    if let Some(id) = output["session_id"].as_i64() {
        sections.push(format!("Process running with session ID {id}"));
    }
    if let Some(id) = output["completed_session_id"].as_i64() {
        sections.push(format!("Completed session ID: {id}"));
    }
    if output["output_replayed"] == true {
        sections.push("Retained output (already completed; command was not run again):".into());
    }
    if let Some(count) = output["original_token_count"].as_u64() {
        sections.push(format!("Original token count: {count}"));
    }
    sections.push("Output:".into());
    sections.push(output["output"].as_str().unwrap_or("").into());
    json!({"content":[{"type":"text","text":sections.join("\n")}],"structuredContent":output})
}

#[cfg(test)]
mod remote_trust_tests {
    use super::*;
    #[test]
    fn server_project_file_panel_uses_native_sftp_and_runtime_path_trust() {
        let folder = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join("public.txt"), "public line\r\n").unwrap();
        std::fs::write(
            folder.path().join("public.png"),
            "not a local image preview",
        )
        .unwrap();
        std::fs::create_dir(folder.path().join("nested")).unwrap();
        std::fs::create_dir(folder.path().join(".git")).unwrap();
        let program = if cfg!(windows) {
            "C:/Program Files/Git/usr/lib/ssh/sftp-server.exe"
        } else if std::path::Path::new("/usr/lib/openssh/sftp-server").is_file() {
            "/usr/lib/openssh/sftp-server"
        } else {
            "/usr/libexec/sftp-server"
        };
        assert!(
            std::path::Path::new(program).is_file(),
            "Native SFTP subsystem required"
        );
        let environment = if cfg!(windows) {
            BTreeMap::from([(
                "PATH".into(),
                format!(
                    "C:/Program Files/Git/usr/bin;C:/Program Files/Git/mingw64/bin;{}",
                    std::env::var("PATH").unwrap_or_default()
                ),
            )])
        } else {
            BTreeMap::new()
        };
        let mut client = cos_tools::remote::Sftp::spawn_environment(
            program,
            &[],
            Some(folder.path()),
            &environment,
        )
        .unwrap();
        let config = Arc::new(ConfigStore::open(profile.path(), platform()).unwrap());
        let kernel = Kernel::new(config.clone()).unwrap();
        let project = kernel
            .projects
            .add_remote_with(&config, "public-fixture", "~", &mut client)
            .unwrap();
        let id = project["id"].as_str().unwrap();
        kernel.remote.lock().unwrap().insert(
            project["remote"].to_string(),
            Arc::new(Mutex::new(RemoteFiles {
                sftp: client,
                root: project["remote"]["path"].as_str().unwrap().into(),
            })),
        );
        assert!(
            kernel
                .project_directory(id, "")
                .unwrap_err()
                .to_string()
                .contains("SSH_PATH_TRUST_REQUIRED")
        );
        assert!(kernel.project_file_snapshot(id, "public.txt").is_err());
        kernel.trust_ssh_server_paths_for_lifetime();
        let listing = kernel.project_directory(id, "").unwrap();
        assert_eq!(listing["entries"][0]["name"], "nested");
        assert_eq!(listing["entries"].as_array().unwrap().len(), 3);
        let preview = kernel.project_preview(id, "public.txt").unwrap();
        assert_eq!(preview["binary"], false);
        assert_eq!(preview["text"], "public line");
        assert!(preview.get("revision").is_none());
        let preview = kernel.project_preview(id, "public.png").unwrap();
        assert_eq!(preview["binary"], true);
        assert!(preview.get("imageDataUrl").is_none());
        assert_eq!(
            preview["note"],
            "Previews of images and PDFs in a server folder are not available yet."
        );
        assert!(kernel.project_preview(id, "../outside.txt").is_err());
        std::fs::write(folder.path().join("attachment.bin"), vec![42u8; 600_000]).unwrap();
        let snapshot = kernel
            .project_file_snapshot(id, "attachment.bin")
            .unwrap()
            .expect("A server attachment needs a private streamed snapshot");
        assert_eq!(std::fs::read(&snapshot.file).unwrap(), vec![42u8; 600_000]);
        let temporary = snapshot.file.parent().unwrap().to_path_buf();
        drop(snapshot);
        assert!(!temporary.exists());
        std::fs::File::create(folder.path().join("too-large.bin"))
            .unwrap()
            .set_len(512 * 1024 * 1024 + 1)
            .unwrap();
        assert!(
            kernel
                .project_file_snapshot(id, "too-large.bin")
                .err()
                .unwrap()
                .to_string()
                .contains("512 MB")
        );
        let original_roots = kernel.roots().unwrap();
        let checks = std::cell::Cell::new(0usize);
        let interrupted = kernel.with_project_files(id, |_, roots, check| {
            kernel.with_remote(&roots[0], |client| {
                ProjectFiles::server_snapshot(client, "attachment.bin", &|| {
                    checks.set(checks.get() + 1);
                    if checks.get() == 3 {
                        config.update(|value| value["roots"] = json!([]))?;
                    }
                    check()
                })
            })
        });
        assert!(
            interrupted.is_err(),
            "Root revocation during a multi-chunk attachment was ignored"
        );
        config
            .update(|value| value["roots"] = json!(original_roots))
            .unwrap();
        assert!(
            kernel.project_preview(id, "public.txt").is_ok(),
            "A failed copy leaked its SFTP file handle or desynchronized the transport"
        );
        let error = kernel
            .project_files
            .create(
                &kernel.projects,
                &kernel.roots().unwrap(),
                id,
                "",
                "denied.txt",
                "file",
            )
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Files in a server folder cannot be changed from the app yet; ChatGPT can still change them with its tools."
        );
        assert!(!folder.path().join("denied.txt").exists());
        config.update(|v| v["roots"] = json!([])).unwrap();
        assert!(kernel.project_preview(id, "public.txt").is_err());
    }
    #[test]
    fn remote_operations_require_human_lifetime_path_trust_before_transport_use() {
        let profile = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::open(profile.path(), platform()).unwrap());
        let root = Root {
            name: "r".into(),
            path: "/srv/project".into(),
            remote: Some(json!({"host":"public-fixture", "path":"/srv/project"})),
        };
        config
            .update(|v| v["roots"] = json!([root.clone()]))
            .unwrap();
        let kernel = Kernel::new(config).unwrap();
        // An already connected local protocol fixture; this test never launches SSH.
        let transport = cos_tools::remote::Sftp::from_io(
            std::io::Cursor::new(vec![0, 0, 0, 5, 2, 0, 0, 0, 3]),
            Vec::new(),
        )
        .unwrap();
        kernel.remote.lock().unwrap().insert(
            root.remote.as_ref().unwrap().to_string(),
            Arc::new(Mutex::new(RemoteFiles {
                sftp: transport,
                root: "/srv/project".into(),
            })),
        );
        let result = kernel.with_remote(&root, |_| Ok("untrusted path operation executed"));
        assert!(
            result.is_err(),
            "remote path operations were enabled without explicit host trust"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("SSH_PATH_TRUST_REQUIRED")
        );
        kernel.trust_ssh_server_paths_for_lifetime();
        assert_eq!(
            kernel
                .with_remote(&root, |_| Ok("public trusted operation"))
                .unwrap(),
            "public trusted operation"
        );
        let restarted = Kernel::new(kernel.config.clone()).unwrap();
        assert!(
            restarted.with_remote(&root, |_| Ok(())).is_err(),
            "path trust persisted across kernel lifetimes"
        );
    }
}
