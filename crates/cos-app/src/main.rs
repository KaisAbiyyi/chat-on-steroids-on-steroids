mod profile;
use cos_core::{
    config,
    logging::{self, DiagnosticLog},
    persistence::{ConfigStore, DurableStore, InstanceLease, atomic_write, invalid},
    runtime::{Cancellation, ShutdownPhase, shutdown},
    secrets::{SecretCipher, SecretStore, UnavailableCipher, WindowsCipher},
};
use cos_store::{
    CreateOptions, ReadOptions, SessionStore, UpsertOptions,
    access::ConversationPolicy,
    correlation::{Correlation, Correlations},
    model,
    provider_usage::ProviderUsage,
    recorder::Recorder,
    usage,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
const HELP: &str = "CoS-Rust 0.1 — Rust services, phases 1–6\nUse: cos-rust --data-dir <absolute directory> <command>\nServices: serve [--trust-ssh-server-paths] [--port N] [--duration-ms N] [--tunnel openai/cloudflared] [--tunnel-binary path] | tools list/call | projects list/add/add-ssh/color/remove/git/review/diff/files/preview/create-entry/rename-entry/save-file/bind | files read/save/patch\nCommands: info | self-test | config show/validate/set | sessions list/create/rename/clear-name | history <id> | append <id> <json-file> | message <id> <json-file> | observe <conversation> <json-file> | correlate <json-file> | record-call <json-file> | rebind <id> <from> <to> [handoff] | search <query> | find <id> <query> | export <id> [--format json/md] [--scope session/answer --turn id] [--output path] | usage | provider-usage --codex-home path --claude-home path | policy trust/untrust/block/release <ids> | plan <id> <conversation> <json-file> | trace <id> <turn> <json-file> | secrets-status | import --source <original profile>\nProfiles must be new/marked; import copies a legacy profile into a new directory. serve opens loopback listeners; SSH and public tunnels run only when explicitly requested. No signed-in provider is selected automatically. SSH path access through tools/terminals is off by default; --trust-ssh-server-paths opts in for this runtime and requires a trusted server filesystem.\n";
fn platform() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}
struct Runtime {
    _lease: InstanceLease,
    config: Arc<ConfigStore>,
    durable: DurableStore,
    store: SessionStore,
    correlations: Arc<Correlations>,
    policy: Arc<ConversationPolicy>,
    recorder: Arc<Recorder>,
    kernel: Arc<cos_mcp::kernel::Kernel>,
    cancel: Cancellation,
    log: DiagnosticLog,
    directory: PathBuf,
}
impl Runtime {
    fn open(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let directory = fs::canonicalize(directory)?;
        let lease = InstanceLease::acquire(&directory)?;
        profile::ensure_marked(&directory)?;
        let config = Arc::new(ConfigStore::open(&directory, platform())?);
        let durable = DurableStore::open(&directory)?;
        let store = SessionStore::open(&directory)?;
        store.set_asset_limit(
            config.get()["sessions"]["storageLimitGiB"]
                .as_u64()
                .unwrap_or(2),
        )?;
        let correlations = Arc::new(Correlations::restore(durable.clone(), &store)?);
        let policy = Arc::new(ConversationPolicy::restore(durable.clone())?);
        let recorder = Arc::new(Recorder::new(store.clone(), correlations.clone()));
        recorder.set_enabled(config.get()["sessions"]["record"] == true);
        let cancel = Cancellation::default();
        let kernel = cos_mcp::kernel::Kernel::from_parts(
            config.clone(),
            durable.clone(),
            store.clone(),
            correlations.clone(),
            policy.clone(),
            recorder.clone(),
            cancel.clone(),
        );
        kernel.set_worker(std::env::current_exe()?)?;
        let runtime = Self {
            _lease: lease,
            config,
            durable,
            store,
            correlations,
            policy,
            recorder,
            kernel,
            cancel,
            log: DiagnosticLog::new(1000),
            directory,
        };
        runtime.log.push("info", "Rust foundation started");
        Ok(runtime)
    }
    fn close(&self) -> io::Result<()> {
        self.kernel.stop_services()?;
        let sessions = self.store.clone();
        let durable = self.durable.clone();
        let report = shutdown(
            &self.cancel,
            vec![
                ShutdownPhase::new("sessions", Duration::from_secs(3), move || sessions.flush()),
                ShutdownPhase::new("durable", Duration::from_secs(3), move || durable.flush()),
            ],
        );
        if report.iter().any(|r| r.status != "ok") {
            return Err(io::Error::other(
                "Persistence shutdown did not fully settle",
            ));
        }
        Ok(())
    }
    fn dispatch(&self, mut args: Vec<String>) -> io::Result<Value> {
        self.cancel.check()?;
        let command = args.remove(0);
        let at = chrono::Utc::now().timestamp_millis();
        match command.as_str() {
            "tools" => match required(&args, 0)?.as_str() {
                "list" => Ok(json!(
                    self.kernel
                        .tools(surface(args.get(1).map(String::as_str).unwrap_or("core"))?)
                )),
                "call" => {
                    let mut options = args[1..].to_vec();
                    let request=take_option(&mut options,"--request-id")?.ok_or_else(||invalid("Tool calls require --request-id; proof or Allow unattributed calls also applies"))?;
                    let name = required(&options, 0)?;
                    let value = json_file(Path::new(required(&options, 1)?))?;
                    Ok(self
                        .kernel
                        .invoke(cos_mcp::kernel::Surface::Core, &request, name, value))
                }
                _ => Err(invalid(
                    "Use tools list [core/desktop/plugins] or tools call <name> <json-file> --request-id <id>",
                )),
            },
            "projects" => match required(&args, 0)?.as_str() {
                "list" => self.kernel.projects.list(),
                "files" | "preview" | "create-entry" | "rename-entry" | "save-file" => {
                    let action = required(&args, 0)?.as_str();
                    let id = required(&args, 1)?;
                    let roots = self.kernel.roots()?;
                    let files = &self.kernel.project_files;
                    let projects = &self.kernel.projects;
                    match action {
                        "files" if (2..=3).contains(&args.len()) => self
                            .kernel
                            .project_directory(id, args.get(2).map_or("", String::as_str)),
                        "preview" if args.len() == 3 => {
                            self.kernel.project_preview(id, required(&args, 2)?)
                        }
                        "create-entry" if args.len() == 5 => files.create(
                            projects,
                            &roots,
                            id,
                            required(&args, 2)?,
                            required(&args, 3)?,
                            required(&args, 4)?,
                        ),
                        "rename-entry" if args.len() == 4 => files.rename(
                            projects,
                            &roots,
                            id,
                            required(&args, 2)?,
                            required(&args, 3)?,
                        ),
                        "save-file" if args.len() == 4 => {
                            let edit = json_file(Path::new(required(&args, 3)?))?;
                            files.save(
                                projects,
                                &roots,
                                id,
                                required(&args, 2)?,
                                &cos_tools::project_files::Edit {
                                    text: edit["text"]
                                        .as_str()
                                        .ok_or_else(|| invalid("Edited text is required"))?,
                                    modified_at: edit["expectedModifiedAt"].as_str().ok_or_else(
                                        || invalid("Expected modifiedAt is required"),
                                    )?,
                                    bytes: edit["expectedBytes"].as_u64().ok_or_else(|| {
                                        invalid("Expected byte count is required")
                                    })?,
                                    revision: edit["expectedRevision"]
                                        .as_str()
                                        .ok_or_else(|| invalid("Expected revision is required"))?,
                                },
                            )
                        }
                        _ => Err(invalid(
                            "Use projects files <id> [directory], preview <id> <path>, create-entry <id> <directory> <name> <file|directory>, rename-entry <id> <path> <name>, or save-file <id> <path> <edit-json>",
                        )),
                    }
                }
                "color" => self.kernel.projects.set_color(
                    required(&args, 1)?,
                    match required(&args, 2)?.as_str() {
                        "-" => None,
                        color => Some(color),
                    },
                ),
                "remove" => self.kernel.projects.remove(required(&args, 1)?),
                "add" => self
                    .kernel
                    .projects
                    .add(&self.kernel.roots()?, required(&args, 1)?),
                "add-ssh" => self.kernel.projects.add_remote(
                    &self.config,
                    required(&args, 1)?,
                    required(&args, 2)?,
                ),
                "git" => cos_tools::project::git(&self.kernel.roots()?, required(&args, 1)?),
                "review" | "diff" => {
                    let action = required(&args, 0)?.clone();
                    let base = take_option(&mut args, "--base-ref")?;
                    let revision = take_option(&mut args, "--revision")?;
                    if args.len() != if action == "review" { 2 } else { 3 } {
                        return Err(invalid(
                            "Use projects review <project-id> [--base-ref ref] or diff <project-id> <path> [--base-ref ref] [--revision sha256]",
                        ));
                    }
                    let project = self
                        .kernel
                        .projects
                        .list()?
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|p| p["id"] == required(&args, 1).unwrap().as_str())
                        .cloned()
                        .ok_or_else(|| invalid("Project not found"))?;
                    if project.get("remote").is_some() {
                        return Err(invalid(
                            "Git changes of a project on a server are not shown in the app yet.",
                        ));
                    }
                    let roots = self.kernel.roots()?;
                    let id = project["id"].as_str().unwrap();
                    let workspace = self.kernel.projects.workspace(&roots, id)?;
                    let path = &workspace.virtual_path;
                    let result = if action == "review" {
                        cos_tools::git::snapshot(&roots, path, id, base.as_deref())?
                    } else {
                        cos_tools::git::diff(
                            &roots,
                            path,
                            id,
                            required(&args, 2)?,
                            base.as_deref(),
                            revision.as_deref(),
                        )?
                    };
                    if self.kernel.roots()? != roots
                        || !self
                            .kernel
                            .projects
                            .list()?
                            .as_array()
                            .unwrap()
                            .contains(&project)
                    {
                        return Err(invalid("Workspace changed during Git review"));
                    }
                    Ok(result)
                }
                "bind" => {
                    let id = required(&args, 1)?;
                    let project = required(&args, 2)?;
                    if !self
                        .kernel
                        .projects
                        .list()?
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|p| p["id"] == project.as_str())
                    {
                        return Err(invalid("Unknown project"));
                    }
                    self.store
                        .update_metadata(id, |v| v["projectId"] = json!(project))
                }
                _ => Err(invalid(
                    "Use projects list/add/add-ssh/color/remove/git/review/diff/files/preview/create-entry/rename-entry/save-file/bind",
                )),
            },
            "files" => {
                let action = required(&args, 0)?.as_str();
                let caps = config::effective_capabilities(&self.config.get(), platform());
                let require = |key: &str| -> io::Result<()> {
                    if caps[key] == true {
                        Ok(())
                    } else {
                        Err(invalid(format!(
                            "TOOL_DISABLED: file {key} permission is disabled"
                        )))
                    }
                };
                match action {
                    "read" => require("read")?,
                    "save" => require("edit")?,
                    "patch" => {
                        let source = fs::read_to_string(required(&args, 2)?)?;
                        let parsed = cos_tools::patch::parse_typed(&source).map_err(invalid)?;
                        let roots = self.kernel.roots()?;
                        for hunk in parsed.hunks {
                            match hunk {
                                cos_tools::patch::Hunk::AddFile { path, .. } => require(
                                    if cos_core::sandbox::resolve(
                                        &roots,
                                        &path,
                                        Some(required(&args, 1)?),
                                        false,
                                    )
                                    .is_ok()
                                    {
                                        "edit"
                                    } else {
                                        "create"
                                    },
                                )?,
                                cos_tools::patch::Hunk::DeleteFile { .. } => require("deleteFile")?,
                                cos_tools::patch::Hunk::UpdateFile { move_path, .. } => {
                                    require("edit")?;
                                    if move_path.is_some() {
                                        require("move")?;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                match action {
                    "read" => self.kernel.files.read(
                        &self.kernel.roots()?,
                        required(&args, 1)?,
                        1,
                        100_000_000,
                        256 * 1024,
                    ),
                    "save" => {
                        let text = fs::read_to_string(required(&args, 2)?)?;
                        Ok(
                            json!({"revision":self.kernel.files.save(&self.kernel.roots()?,required(&args,1)?,&text,required(&args,3)?)?}),
                        )
                    }
                    "patch" => self.kernel.files.apply_checked(
                        &self.kernel.roots()?,
                        required(&args, 1)?,
                        &fs::read_to_string(required(&args, 2)?)?,
                        None,
                        &|kind| {
                            if kind.is_empty() {
                                Ok(())
                            } else {
                                require(kind)
                            }
                        },
                    ),
                    _ => Err(invalid(
                        "Use files read <path>, save <path> <text-file> <revision>, or patch <base> <patch-file>",
                    )),
                }
            }
            "info" => Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"compatibleAppVersion":cos_core::APP_VERSION,"bridgeProtocol":cos_core::BRIDGE_PROTOCOL,"upstreamCommit":cos_core::UPSTREAM_COMMIT,"platform":platform(),"dataDirectory":self.directory,"configRecovered":self.config.recovered(),"effectiveCapabilities":config::effective_capabilities(&self.config.get(),platform()),"fullApplicationReady":false,"scope":"Rust runtime, sessions, MCP/code mode, file/project/SSH and process/terminal services; GUI/browser hosts and whole-phase exact qualification remain open"}),
            ),
            "self-test" => {
                let id = format!(
                    "probe-{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(at)
                );
                let conversation = format!("public-{at}");
                self.store.create(CreateOptions {
                    id: Some(id.clone()),
                    title: Some("Public baseline probe".into()),
                    conversation_id: Some(conversation.clone()),
                    ..Default::default()
                })?;
                let text = self
                    .store
                    .store_text(&id, "public baseline hello 😀", 12000)?;
                self.store.append(
                    &id,
                    json!({"kind":"turn_start","source":"app","time":at,"turnId":"probe-turn"}),
                )?;
                self.store.upsert(&id,json!({"kind":"user_message","source":"extension","time":at,"messageId":"probe-user","conversationId":conversation,"turnId":"probe-turn","message":text}),UpsertOptions::default())?;
                self.store.upsert(&id,json!({"kind":"assistant_message","source":"extension","time":at+1,"messageId":"probe-assistant","conversationId":conversation,"turnId":"probe-turn","message":{"text":"Public probe completed","truncated":false,"chars":22},"state":"final","final":true}),UpsertOptions::default())?;
                self.store.append(&id,json!({"kind":"turn_end","source":"app","time":at+2,"turnId":"probe-turn","outcome":"completed"}))?;
                Ok(json!({"sessionId":id,"publicFixture":true,"summary":self.store.summary(&id)?}))
            }
            "config" => match required(&args, 0)?.as_str() {
                "show" => Ok(self.config.get()),
                "validate" => Ok(config::parse(json_file(Path::new(required(&args, 1)?))?)
                    .map_err(|_| invalid("Invalid configuration"))?),
                "set" => {
                    let path = required(&args, 1)?.split('.').collect::<Vec<_>>();
                    let value = serde_json::from_str::<Value>(required(&args, 2)?)
                        .map_err(|_| invalid("Config value must be JSON"))?;
                    let current = self.config.get();
                    let mut pointer = &current;
                    for segment in &path {
                        pointer = pointer
                            .get(*segment)
                            .ok_or_else(|| invalid("Unknown config field"))?;
                    }
                    let result = self.config.update(|config| {
                        let mut target = config;
                        for segment in &path {
                            target = &mut target[*segment];
                        }
                        *target = value;
                    })?;
                    Ok(result)
                }
                _ => Err(invalid("Unknown config command")),
            },
            "sessions" => match required(&args, 0)?.as_str() {
                "list" => self.store.list_page(
                    option_number(&mut args, "--limit", 200)?.min(5000),
                    None,
                    take_option(&mut args, "--project")?.as_deref(),
                ),
                "create" => {
                    let id = take_option(&mut args, "--id")?;
                    let conversation = take_option(&mut args, "--conversation")?;
                    self.store.create(CreateOptions {
                        id,
                        title: args.get(1).cloned(),
                        conversation_id: conversation,
                        ..Default::default()
                    })
                }
                "rename" => {
                    let source = take_option(&mut args, "--source")?;
                    let source = match source.as_deref().unwrap_or("manual") {
                        "manual" => cos_store::title::TitleSource::Manual,
                        "provider" => cos_store::title::TitleSource::Provider,
                        "fallback" => cos_store::title::TitleSource::Fallback,
                        _ => return Err(invalid("Unknown session title source")),
                    };
                    let conversation = take_option(&mut args, "--conversation")?;
                    self.store.rename(
                        required(&args, 1)?,
                        required(&args, 2)?,
                        source,
                        conversation.as_deref(),
                    )
                }
                "clear-name" => self.store.clear_name(required(&args, 1)?),
                _ => Err(invalid("Unknown sessions command")),
            },
            "history" => {
                let limit = option_number(&mut args, "--limit", 200)?.min(5000);
                let from = take_option(&mut args, "--from")?
                    .map(|v| {
                        v.parse::<u64>()
                            .map_err(|_| invalid("Invalid history cursor"))
                    })
                    .transpose()?;
                let id = required(&args, 0)?;
                let events = self.store.read(
                    id,
                    ReadOptions {
                        from,
                        limit,
                        ..Default::default()
                    },
                )?;
                let next = events
                    .iter()
                    .map(|e| model::number(e, "seq"))
                    .max()
                    .map(|seq| seq + 1);
                Ok(json!({"events":events,"next":next}))
            }
            "append" => Ok(self.store.append(
                required(&args, 0)?,
                json_file(Path::new(required(&args, 1)?))?,
            )?),
            "message" => serde_json::to_value(self.store.upsert(
                required(&args, 0)?,
                json_file(Path::new(required(&args, 1)?))?,
                UpsertOptions::default(),
            )?)
            .map_err(io::Error::other),
            "observe" => {
                let observations = json_file(Path::new(required(&args, 1)?))?;
                let observations = observations
                    .as_array()
                    .ok_or_else(|| invalid("Observations must be a JSON array"))?;
                Ok(json!({"sessionId":self.recorder.observe(required(&args,0)?,observations,at)?}))
            }
            "correlate" => {
                let raw = json_file(Path::new(required(&args, 0)?))?;
                let rows = if raw.is_array() { raw } else { json!([raw]) };
                let owners: Vec<Correlation> = serde_json::from_value(rows)
                    .map_err(|_| invalid("Invalid request proof payload"))?;
                Ok(json!(self.correlations.observe_and_commit(&owners)?))
            }
            "record-call" => {
                let raw = json_file(Path::new(required(&args, 0)?))?;
                self.recorder.record_call(
                    model::string(&raw, "requestId"),
                    model::string(&raw, "tool"),
                    raw["args"].clone(),
                    model::string(&raw, "result"),
                    raw["startedAt"].as_i64().unwrap_or(at),
                    raw["outcome"].as_str().unwrap_or("ok"),
                )
            }
            "rebind" => Ok(
                json!({"moved":self.store.rebind(required(&args,0)?,if required(&args,1)?=="-"{None}else{Some(required(&args,1)?)},required(&args,2)?,args.get(3).map(String::as_str),at)?}),
            ),
            "search" => self.store.search(&args.join(" "), 50),
            "find" => Ok(json!(
                self.store
                    .find_text(required(&args, 0)?, &args[1..].join(" "))?
            )),
            "export" => {
                let format = take_option(&mut args, "--format")?.unwrap_or_else(|| "json".into());
                let output = take_option(&mut args, "--output")?;
                let scope = take_option(&mut args, "--scope")?.unwrap_or_else(|| "session".into());
                let turn = take_option(&mut args, "--turn")?;
                if !matches!(scope.as_str(), "session" | "answer") {
                    return Err(invalid("Export scope must be session or answer"));
                }
                if scope == "answer" && format != "md" {
                    return Err(invalid("Answer scope requires Markdown format"));
                }
                let id = required(&args, 0)?;
                let bytes = match format.as_str() {
                    "json" => serde_json::to_vec_pretty(&self.store.export_json(id)?)?,
                    "md" if scope == "answer" => self
                        .store
                        .export_answer_markdown(id, turn.as_deref().unwrap_or(""))?
                        .into_bytes(),
                    "md" => self.store.export_markdown(id)?.into_bytes(),
                    _ => return Err(invalid("Export format must be json or md")),
                };
                if let Some(output) = output {
                    let output = Path::new(&output);
                    if !output.is_absolute() || output.exists() {
                        return Err(invalid("Export output must be a new absolute file path"));
                    }
                    atomic_write(output, &bytes)?;
                    Ok(json!({"path":output,"bytes":bytes.len(),"format":format}))
                } else if format == "json" {
                    serde_json::from_slice(&bytes).map_err(io::Error::other)
                } else {
                    Ok(
                        json!({"format":"md","text":String::from_utf8(bytes).map_err(io::Error::other)?}),
                    )
                }
            }
            "usage" => {
                let explicit = take_option(&mut args, "--context-cap")?;
                let cap = explicit
                    .as_ref()
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| invalid("Invalid context comparison cap"))
                    })
                    .transpose()?
                    .unwrap_or(256000);
                let mut overview = usage::overview_at(&self.store, cap, at)?;
                overview["contextCapSource"] = json!(if explicit.is_some() {
                    "explicit"
                } else {
                    "default; provider catalog unavailable"
                });
                Ok(overview)
            }
            "provider-usage" => {
                let codex = take_option(&mut args, "--codex-home")?
                    .ok_or_else(|| invalid("Explicit --codex-home required"))?;
                let claude = take_option(&mut args, "--claude-home")?
                    .ok_or_else(|| invalid("Explicit --claude-home required"))?;
                if !Path::new(&codex).is_absolute() || !Path::new(&claude).is_absolute() {
                    return Err(invalid("Provider homes must be absolute"));
                }
                ProviderUsage::read(
                    self.config.get()["ui"]["localAppUsage"] != false,
                    at,
                    Path::new(&codex),
                    Path::new(&claude),
                )
            }
            "policy" => {
                let action = required(&args, 0)?.as_str();
                let ids = args[1..].to_vec();
                match action {
                    "trust" | "untrust" => self.policy.set_trusted(&ids, action == "trust")?,
                    "block" | "release" => self.policy.set_blocked(&ids, action == "block", at)?,
                    _ => return Err(invalid("Unknown conversation policy action")),
                }
                self.policy.snapshot()
            }
            "plan" => Ok(
                json!({"updated":self.store.update_plan(required(&args,0)?,required(&args,1)?,json_file(Path::new(required(&args,2)?))?,at,None)?}),
            ),
            "trace" => Ok(
                json!({"updated":self.store.write_trace(required(&args,0)?,required(&args,1)?,&json_file(Path::new(required(&args,2)?))?,None)?}),
            ),
            "secrets-status" => {
                let cipher: Arc<dyn SecretCipher> = if cfg!(windows) {
                    match WindowsCipher::open(&self.directory, true) {
                        Ok(cipher) => Arc::new(cipher),
                        Err(_) => Arc::new(UnavailableCipher),
                    }
                } else {
                    Arc::new(UnavailableCipher)
                };
                let available = cipher.available();
                let store = SecretStore::open(&self.directory, platform(), cipher)?;
                Ok(
                    json!({"available":available,"unreadable":store.is_unreadable()?,"platform":platform(),"qualified":cfg!(windows)}),
                )
            }
            _ => Err(invalid("Unknown command; use --help")),
        }
    }
}
fn required(args: &[String], index: usize) -> io::Result<&String> {
    args.get(index)
        .ok_or_else(|| invalid("Missing command argument; use --help"))
}
fn take_option(args: &mut Vec<String>, flag: &str) -> io::Result<Option<String>> {
    if let Some(index) = args.iter().position(|arg| arg == flag) {
        if index + 1 >= args.len() {
            return Err(invalid(format!("{flag} requires a value")));
        }
        args.remove(index);
        Ok(Some(args.remove(index)))
    } else {
        Ok(None)
    }
}
fn option_number(args: &mut Vec<String>, flag: &str, default: usize) -> io::Result<usize> {
    take_option(args, flag)?
        .map(|value| {
            value
                .parse()
                .map_err(|_| invalid(format!("Invalid {flag}")))
        })
        .unwrap_or(Ok(default))
}
fn json_file(path: &Path) -> io::Result<Value> {
    let mut file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Input must be a regular JSON file"));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(invalid("JSON input exceeds 8 MiB"));
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid JSON input"))
}
fn run() -> io::Result<Value> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|a| a == "--code-worker") {
        cos_mcp::worker::entry()?;
        return Ok(Value::Null);
    }
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(Value::Null);
    }
    let directory = take_option(&mut args, "--data-dir")?.ok_or_else(|| {
        invalid("Explicit --data-dir required; personal profiles are never selected automatically")
    })?;
    let directory = Path::new(&directory);
    if !directory.is_absolute() {
        return Err(invalid("Data directory must be absolute"));
    }
    if args.first().is_some_and(|a| a == "import") {
        let source = take_option(&mut args, "--source")?
            .ok_or_else(|| invalid("Import requires --source"))?;
        return profile::import(Path::new(&source), directory);
    }
    if args.is_empty() {
        return Err(invalid("Missing command"));
    }
    let trust_ssh_paths =
        if let Some(at) = args.iter().position(|a| a == "--trust-ssh-server-paths") {
            args.remove(at);
            true
        } else {
            false
        };
    let runtime = Runtime::open(directory)?;
    if trust_ssh_paths {
        runtime.kernel.trust_ssh_server_paths_for_lifetime();
    }
    let result = if args.first().is_some_and(|a| a == "serve") {
        args.remove(0);
        let port = take_option(&mut args, "--port")?
            .map(|s| s.parse::<u16>().map_err(|_| invalid("Invalid port")))
            .transpose()?
            .unwrap_or(0);
        let duration = take_option(&mut args, "--duration-ms")?
            .map(|s| s.parse::<u64>().map_err(|_| invalid("Invalid duration")))
            .transpose()?;
        let tunnel_kind = take_option(&mut args, "--tunnel")?;
        let tunnel_binary = take_option(&mut args, "--tunnel-binary")?;
        if !args.is_empty()
            || tunnel_kind
                .as_deref()
                .is_some_and(|kind| !matches!(kind, "openai" | "cloudflared"))
        {
            return Err(invalid(
                "Use serve [--port N] [--duration-ms N] [--tunnel openai/cloudflared] [--tunnel-binary path]",
            ));
        }
        let executor = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(32)
            .enable_all()
            .build()?;
        let served = executor.block_on(async {
            let service=cos_mcp::http::Service::start(runtime.kernel.clone(),port as u16).await?;
            if let Some(kind) = tunnel_kind {
                let settings = runtime.config.get()["tunnel"].clone();
                let program = tunnel_binary.as_deref().or_else(||settings["binaryPath"].as_str().filter(|s|!s.is_empty())).unwrap_or(if kind=="openai"{"tunnel-client"}else{"cloudflared"});
                if kind=="cloudflared" { runtime.kernel.tunnel.start(program,&cos_mcp::tunnel::Supervisor::cloudflared_args(service.port),&runtime.directory)?; }
                else {
                    let cipher: Arc<dyn SecretCipher> = if cfg!(windows) { Arc::new(WindowsCipher::open(&runtime.directory,true)?) } else { Arc::new(UnavailableCipher) };
                    let secrets = SecretStore::open(&runtime.directory,platform(),cipher)?;
                    let profile = settings["profileId"].as_str().unwrap_or("default"); let slot = if profile=="default"{"openaiApiKey".into()}else{format!("setup:{profile}")};
                    let key = secrets.get(&slot)?.ok_or_else(||invalid("Secure Tunnel API key is not configured in this explicit profile"))?;
                    runtime.kernel.tunnel.start_openai(program,settings["tunnelId"].as_str().unwrap_or(""),&key,&service.urls["core"],&json!({}))?;
                }
            }
            println!("{}",json!({"event":"ready","port":service.port,"urls":service.urls,"controlPort":service.control.as_ref().map(|c|c.port),"controlEndpointFile":runtime.directory.join("control-api/endpoint.json"),"profile":runtime.directory,"tunnel":runtime.kernel.tunnel.status(),"fullApplicationReady":false}));
            if let Some(duration)=duration {tokio::time::sleep(Duration::from_millis(duration)).await;}else{tokio::signal::ctrl_c().await?;}
            Ok::<_,io::Error>(service.stop_for_exit(Duration::from_secs(2)).await)
        });
        executor.shutdown_timeout(Duration::from_secs(2));
        served
    } else {
        runtime.dispatch(args)
    };
    let close = runtime.close();
    match (result, close) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}
fn surface(name: &str) -> io::Result<cos_mcp::kernel::Surface> {
    match name {
        "core" => Ok(cos_mcp::kernel::Surface::Core),
        "desktop" => Ok(cos_mcp::kernel::Surface::Desktop),
        "plugins" => Ok(cos_mcp::kernel::Surface::Plugins),
        _ => Err(invalid("Unknown tool surface")),
    }
}
fn main() {
    match run() {
        Ok(value) => {
            if !value.is_null() {
                match serde_json::to_string_pretty(&value) {
                    Ok(text) => println!("{text}"),
                    Err(_) => std::process::exit(1),
                }
            }
        }
        Err(error) => {
            eprintln!("{}", logging::redact(&error.to_string()));
            std::process::exit(1)
        }
    }
}
