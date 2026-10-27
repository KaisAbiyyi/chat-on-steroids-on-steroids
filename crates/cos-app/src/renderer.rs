//! Explicit, source-verified launcher and typed-state adapter for 15HOST.
//!
//! The native renderer is a child process. Rust alone opens the explicitly
//! selected CoSoS profile and authorizes session reads. The child receives
//! bounded presentation snapshots over authenticated redirected stdio; it is
//! never granted filesystem, process, provider, or account authority.
use crate::renderer_integrity;
use cos_core::{
    ui_contract::{UI_PROTOCOL_VERSION, UiCommand, UiNavigation, UiRequest, UiRoute},
    ui_conversation::{
        ConversationCommand, ConversationEntry, ConversationPage, ConversationRequest,
        ConversationRole, MAX_CONVERSATION_PAGE_ITEMS, UiConversation,
    },
    ui_focus::{UiFocus, UiKeyEvent, UiKeyFrame},
    ui_localization::{self, UiLanguage, UiTextKey},
    ui_panels::{PanelCatalog, PanelCommand, PanelRequest, SafePreferences, Theme, UiPanels},
    ui_renderer_ipc::{
        MAX_RENDERER_IPC_FRAME_BYTES, RENDERER_IPC_PROTOCOL, RendererIpcAuthenticator,
    },
    ui_rich::{
        RICH_PROTOCOL_VERSION, RichDeck, RichPanel, RichRendererCommand, RichRendererRequest,
    },
    ui_snapshots::{CaptureConfig, CaptureDescriptor, CaptureSurface},
};
use cos_store::{SessionStore, model};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::Zeroize;

const HISTORY_WINDOW_LIMIT: usize = 6_000;
const SESSION_LIST_LIMIT: usize = 100;
const PAGE_LIMIT: u64 = MAX_CONVERSATION_PAGE_ITEMS as u64;
const MAX_INTENT_BYTES: usize = 16 * 1024;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(35);
const TRANSPORT_IDLE_TIMEOUT: Duration = Duration::from_secs(20);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

pub fn launch(
    store: &SessionStore,
    data_directory: &Path,
    config: Value,
    mut args: Vec<String>,
) -> io::Result<Value> {
    if !cfg!(windows) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "The native WebView2 renderer is supported on Windows only",
        ));
    }
    let renderer_path = take_option(&mut args, "--renderer")?
        .ok_or_else(|| invalid("UI launch requires --renderer <absolute executable>"))?;
    let expected_sha = take_option(&mut args, "--renderer-sha256")?
        .ok_or_else(|| invalid("UI launch requires --renderer-sha256 from the build receipt"))?;
    let expected_manifest =
        take_option(&mut args, "--renderer-manifest-sha256")?.ok_or_else(|| {
            invalid("UI launch requires --renderer-manifest-sha256 from trusted receipt")
        })?;
    let synthetic_probe = if let Some(position) = args
        .iter()
        .position(|option| option == "--synthetic-15host-probe")
    {
        args.remove(position);
        true
    } else {
        false
    };
    let ab_probe = if let Some(index) = args.iter().position(|item| item == "--synthetic-15host-ab")
    {
        args.remove(index);
        true
    } else {
        false
    };
    let injection = take_option(&mut args, "--synthetic-15host-inject")?;
    if injection.as_deref().is_some_and(|kind| {
        !matches!(
            kind,
            "hang" | "early-exit" | "malformed" | "replay" | "stale"
        )
    }) || ((synthetic_probe || ab_probe) && injection.is_some())
        || (synthetic_probe && ab_probe)
    {
        return Err(invalid("Unsupported or conflicting synthetic injection"));
    }
    if (synthetic_probe || ab_probe || injection.is_some())
        && std::fs::read(data_directory.join("15host-synthetic-fixture.marker"))?
            != b"15HOST_SYNTHETIC_CLOUD_E2E_V1"
    {
        return Err(invalid(
            "15HOST cloud probe requires an explicit synthetic fixture marker",
        ));
    }
    if !args.is_empty() {
        return Err(invalid("Unknown UI launch option"));
    }
    let requested_path = PathBuf::from(renderer_path);
    if !requested_path.is_absolute()
        || expected_sha.len() != 64
        || !expected_sha.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(invalid("Renderer path or SHA-256 receipt is invalid"));
    }
    let renderer_path = requested_path.canonicalize()?;
    // Validate the complete framework-dependent .NET application, not only its apphost EXE.
    // The manifest digest MUST be independently trusted, never read as an
    // authoritative value from the renderer directory itself.
    // Keep the validated EXE/managed DLL/dependencies/manifest and directory
    // handles open through native child exit, denying Windows replacement
    // and delete sharing across the complete verifier-to-loader lifetime.
    let locked_payload =
        renderer_integrity::verify(&renderer_path, &expected_sha, &expected_manifest)?;
    let manifest_sha = locked_payload.manifest_sha.clone();
    let actual_sha = expected_sha.to_ascii_lowercase();

    let mut key = [0u8; 32];
    key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    key[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    let epoch = (u64::from_be_bytes(key[..8].try_into().expect("fixed key length"))
        & cos_core::ui_contract::MAX_JS_SAFE_INTEGER)
        .max(1);
    let mut authenticator = RendererIpcAuthenticator::new(epoch, key)
        .map_err(|_| invalid("Could not initialize renderer authentication"))?;
    // Guard owns kill + reap, including bootstrap, authentication and state
    // initialization error paths. No raw Child may escape without Drop cleanup.
    let mut command = Command::new(&renderer_path);
    command.arg("--authenticated-ipc");
    if synthetic_probe {
        command.arg("--synthetic-probe");
    } else if ab_probe {
        command.arg("--synthetic-ab");
    } else if let Some(mode) = injection.as_deref() {
        command.args(["--synthetic-inject", mode]);
    }
    let child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut child = ReapedChild(child);
    let child_input = child
        .0
        .stdin
        .take()
        .ok_or_else(|| invalid("Renderer stdin unavailable"))?;
    let child_output = child
        .0
        .stdout
        .take()
        .ok_or_else(|| invalid("Renderer stdout unavailable"))?;
    let mut writer = BufWriter::new(child_input);
    let (frames, reader_thread) = start_bounded_reader(child_output);

    let outcome = (|| -> io::Result<Value> {
        let bootstrap = json!({
            "kind": "bootstrap",
            "protocol": RENDERER_IPC_PROTOCOL,
            "epoch": epoch,
            "key": encode_hex(&key)
        });
        write_json_line(&mut writer, &bootstrap)?;
        let handshake_deadline = if injection.is_some() {
            Duration::from_secs(3)
        } else {
            HANDSHAKE_TIMEOUT
        };
        let ready_raw = receive_frame(&frames, handshake_deadline)?
            .ok_or_else(|| invalid("Renderer exited before authenticated ready handshake"))?;
        let ready = authenticator
            .verify_inbound(&ready_raw)
            .map_err(|_| invalid("Renderer ready handshake authentication failed"))?;
        let nonce = ready.payload["rendererNonce"].as_str().unwrap_or("");
        if ready.payload["kind"] != "ready"
            || nonce.len() != 32
            || !nonce.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid("Renderer ready handshake contract rejected"));
        }
        let locale = config["ui"]["language"]
            .as_str()
            .or_else(|| config["ui"]["locale"].as_str());
        let theme = match config["ui"]["theme"].as_str() {
            Some("light") => Theme::Light,
            _ => Theme::Dark,
        };
        let language = UiLanguage::select(locale);
        let mut host = RendererHostState::new(epoch, store, theme, language)?;
        send_state(&mut writer, &mut authenticator, &host)?;
        renderer_loop(
            store,
            &mut child.0,
            &frames,
            &mut writer,
            &mut authenticator,
            &mut host,
            RendererProbe {
                synthetic: synthetic_probe || ab_probe,
                ab_isolation: ab_probe,
            },
        )?;
        // An authenticated closing frame does not mean the child exited.
        // Enforce bounded shutdown, and fail if it must be killed.
        let status = child.wait_bounded(SHUTDOWN_TIMEOUT)?;
        if !status.success() {
            return Err(invalid("Native renderer exited with a nonzero status"));
        }
        Ok(json!({
            "renderer":"closed",
            "authenticated":true,
            "epoch":epoch,
            "artifactSha256":actual_sha,
            "manifestSha256":manifest_sha,
            "sourceBound":true,
            "syntheticOnly":false,
            "syntheticProbeComplete":synthetic_probe || ab_probe,
            "abIsolationComplete":ab_probe
        }))
    })();
    // Close the write end to let a healthy child finish promptly.
    drop(writer);
    if outcome.is_err() {
        child.stop();
    }
    // Reader was bounded by the process lifetime, not an indefinite join.
    // Kill/reap occurs before the receiver is dropped on failure.
    drop(frames);
    if outcome.is_ok() {
        let _ = reader_thread.join();
    }
    // This must stay AFTER reaping the C# child (even on errors).
    drop(locked_payload);
    outcome
}

struct RendererProbe {
    synthetic: bool,
    ab_isolation: bool,
}

fn renderer_loop(
    store: &SessionStore,
    child: &mut Child,
    frames: &RendererReceiver,
    writer: &mut BufWriter<ChildStdin>,
    authenticator: &mut RendererIpcAuthenticator,
    host: &mut RendererHostState,
    probe: RendererProbe,
) -> io::Result<()> {
    loop {
        if child.try_wait()?.is_some() {
            return Err(invalid(
                "Native renderer exited without authenticated closing",
            ));
        }
        let Some(raw) = receive_frame(frames, TRANSPORT_IDLE_TIMEOUT)? else {
            return Err(invalid(
                "Native renderer pipe closed without authenticated closing",
            ));
        };
        let verified = authenticator.verify_inbound(&raw).map_err(|_| {
            invalid("Renderer IPC authentication, epoch, sequence, or size check failed")
        })?;
        if verified.payload["kind"] == "renderer-closing" {
            if probe.synthetic && verified.payload["probeComplete"] != true {
                return Err(invalid("Synthetic 15HOST product E2E proof incomplete"));
            }
            if probe.ab_isolation && verified.payload["abIsolation"] != true {
                return Err(invalid(
                    "Cross-profile production-path E2E proof incomplete",
                ));
            }
            return Ok(());
        }
        if verified.payload["kind"] == "renderer-heartbeat" {
            continue;
        }
        if verified.payload["kind"] != "ui-intent" {
            let _ = child.kill();
            return Err(invalid("Renderer sent an unknown authenticated frame type"));
        }
        let intent = verified
            .payload
            .get("intent")
            .cloned()
            .ok_or_else(|| invalid("Renderer intent is missing"))?;
        host.last_error = None;
        if let Err(code) = host.handle_intent(store, intent) {
            host.last_error = Some(code.to_owned());
        }
        send_state(writer, authenticator, host)?;
    }
}

struct ReapedChild(Child);
impl ReapedChild {
    fn stop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
    fn wait_bounded(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                self.stop();
                return Err(invalid("Native renderer shutdown timed out"));
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}
impl Drop for ReapedChild {
    fn drop(&mut self) {
        self.stop();
    }
}

type RendererFrame = io::Result<Option<Vec<u8>>>;
type RendererReceiver = Receiver<RendererFrame>;

fn start_bounded_reader(output: ChildStdout) -> (RendererReceiver, thread::JoinHandle<()>) {
    // A small queue bounds memory even when a compromised child floods stdout.
    let (sender, receiver) = mpsc::sync_channel(4);
    let thread = thread::spawn(move || {
        let mut reader = BufReader::new(output);
        loop {
            let frame = read_bounded_line(&mut reader, MAX_RENDERER_IPC_FRAME_BYTES);
            let terminal = !matches!(frame, Ok(Some(_)));
            if sender.try_send(frame).is_err() || terminal {
                break;
            }
        }
    });
    (receiver, thread)
}

fn receive_frame(frames: &RendererReceiver, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
    match frames.recv_timeout(timeout) {
        Ok(frame) => frame,
        Err(RecvTimeoutError::Timeout) => Err(invalid("Native renderer IPC deadline exceeded")),
        Err(RecvTimeoutError::Disconnected) => Err(invalid("Native renderer reader disconnected")),
    }
}

fn send_state(
    writer: &mut BufWriter<ChildStdin>,
    authenticator: &mut RendererIpcAuthenticator,
    host: &RendererHostState,
) -> io::Result<()> {
    let state = host.state_json()?;
    let payload = serde_json::to_string(&state).map_err(io::Error::other)?;
    let frame = authenticator
        .sign_outbound(&payload)
        .map_err(|_| invalid("Host UI state exceeded the authenticated renderer frame contract"))?;
    writer.write_all(frame.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

struct RendererHostState {
    epoch: u64,
    navigation: UiNavigation,
    conversation: UiConversation,
    panels: UiPanels,
    rich: RichDeck,
    focus: UiFocus,
    navigation_sequence: u64,
    conversation_sequence: u64,
    panels_sequence: u64,
    rich_sequence: u64,
    focus_sequence: u64,
    language: UiLanguage,
    theme: Theme,
    sessions: Vec<Value>,
    history: Vec<ConversationEntry>,
    active_history_session: Option<String>,
    pdf_pages: Vec<AuthorizedPdfPage>,
    last_error: Option<String>,
}

impl RendererHostState {
    fn new(
        epoch: u64,
        store: &SessionStore,
        theme: Theme,
        language: UiLanguage,
    ) -> io::Result<Self> {
        let navigation =
            UiNavigation::new(epoch).map_err(|_| invalid("Invalid navigation epoch"))?;
        let conversation =
            UiConversation::new(epoch).map_err(|_| invalid("Invalid history epoch"))?;
        let mut panels = UiPanels::new(epoch).map_err(|_| invalid("Invalid panels epoch"))?;
        let rich = RichDeck::new(epoch).map_err(|_| invalid("Invalid rich-panel epoch"))?;
        let focus = UiFocus::new(epoch).map_err(|_| invalid("Invalid focus epoch"))?;
        let catalog = PanelCatalog {
            preferences: SafePreferences {
                theme,
                locale: language.tag().to_owned(),
                font_scale_percent: 100,
                reduced_motion: false,
                confirm_tool_actions: true,
            },
            ..PanelCatalog::default()
        };
        panels
            .replace_catalog(catalog)
            .map_err(|_| invalid("Initial UI preferences rejected"))?;
        let sessions = read_sessions(store)?;
        Ok(Self {
            epoch,
            navigation,
            conversation,
            panels,
            rich,
            focus,
            navigation_sequence: 0,
            conversation_sequence: 0,
            panels_sequence: 0,
            rich_sequence: 0,
            focus_sequence: 0,
            language,
            theme,
            sessions,
            history: Vec::new(),
            active_history_session: None,
            pdf_pages: Vec::new(),
            last_error: None,
        })
    }

    fn state_json(&self) -> io::Result<Value> {
        let navigation = self.navigation.snapshot();
        let conversation = self.conversation.snapshot();
        let panels = self.panels.snapshot();
        let rich = self.rich.snapshot();
        let focus = self.focus.snapshot();
        let config = CaptureConfig::new(800, 600, 96, self.theme, self.language)
            .map_err(|_| invalid("15F capture configuration rejected"))?;
        let descriptor = CaptureDescriptor::new(
            config,
            CaptureSurface::Shell,
            &navigation,
            &focus,
            &panels,
            &rich,
            &conversation,
        )
        .map_err(|_| invalid("15F typed host snapshots are inconsistent"))?;
        let projection = &descriptor.projection;
        let capture_contract = json!({
            "version": descriptor.version,
            "logicalWidth": descriptor.config.logical_width,
            "logicalHeight": descriptor.config.logical_height,
            "dpi": descriptor.config.dpi,
            "theme": format!("{:?}", descriptor.config.theme).to_lowercase(),
            "locale": descriptor.config.language.tag(),
            "surface": "shell",
            "route": format!("{:?}", projection.route).to_lowercase(),
            "focus": format!("{:?}", projection.focus).to_lowercase(),
            "dialogOpen": projection.dialog_open,
            "richActive": projection.rich_active.as_ref().map(|kind| format!("{kind:?}").to_lowercase()),
            "richCount": projection.rich_count,
            "conversationRows": projection.conversation_rows,
            "conversationHasOlder": projection.conversation_has_older,
            "conversationHasNewer": projection.conversation_has_newer
        });
        let selected_pdf = rich.panels.iter().find_map(|panel| {
            if let RichPanel::Pdf {
                key,
                asset_id,
                page,
                ..
            } = panel
                && rich.active_key.as_deref() == Some(key.as_str())
            {
                return self
                    .pdf_pages
                    .iter()
                    .find(|item| item.page == *page)
                    .map(|item| {
                        json!({
                            "assetId":asset_id,
                            "page":item.page,
                            "format":"image/png",
                            "sha256":format!("{:x}", Sha256::digest(&item.bytes)),
                            "base64":base64_encode_png(&item.bytes)
                        })
                    });
            }
            None
        });
        Ok(json!({
            "kind": "state",
            "mode": "authenticated",
            "syntheticOnly": false,
            "epoch": self.epoch,
            "navigation": navigation,
            "conversation": conversation,
            "panels": panels,
            "rich": rich,
            "pdfPage": selected_pdf,
            "focus": focus,
            "localization": localization_json(self.language),
            "availableLocales": UiLanguage::ALL.iter().map(|language| language.tag()).collect::<Vec<_>>(),
            "sessions": self.sessions,
            "historyWindow": { "total": self.history.len(), "pageLimit": PAGE_LIMIT },
            "captureContract": capture_contract,
            "lastError": self.last_error
        }))
    }

    fn handle_intent(&mut self, store: &SessionStore, intent: Value) -> Result<(), &'static str> {
        let bytes = serde_json::to_vec(&intent).map_err(|_| "invalid-intent")?;
        if bytes.len() > MAX_INTENT_BYTES || !intent.is_object() {
            return Err("payload-too-large");
        }
        let kind = intent["kind"].as_str().ok_or("invalid-intent")?;
        match kind {
            "navigate" => {
                let request: NavigateIntent = decode_intent(intent)?;
                if request.kind != "navigate" {
                    return Err("invalid-intent");
                }
                if let UiRoute::Conversation { session_id } = request.route {
                    self.open_session(store, &session_id, request.expected_revision)
                } else {
                    let sequence = next_counter(&mut self.navigation_sequence)?;
                    self.navigation
                        .dispatch(UiRequest {
                            protocol: UI_PROTOCOL_VERSION,
                            host_epoch: self.epoch,
                            sequence,
                            command: UiCommand::Navigate {
                                route: request.route,
                                expected_revision: request.expected_revision,
                            },
                        })
                        .map_err(|error| error.code())?;
                    Ok(())
                }
            }
            "select-session" => {
                let request: SelectSessionIntent = decode_intent(intent)?;
                if request.kind != "select-session" {
                    return Err("invalid-intent");
                }
                self.open_session(store, &request.session_id, request.expected_revision)
            }
            "load-page" => {
                let request: LoadPageIntent = decode_intent(intent)?;
                if request.kind != "load-page" {
                    return Err("invalid-intent");
                }
                self.move_to_page(request.start, request.expected_revision)
            }
            "rich-command" => {
                let request: RichIntent = decode_intent(intent)?;
                if request.kind != "rich-command" {
                    return Err("invalid-intent");
                }
                let sequence = next_counter(&mut self.rich_sequence)?;
                let frame = RichRendererRequest {
                    protocol: RICH_PROTOCOL_VERSION,
                    host_epoch: self.epoch,
                    sequence,
                    command: request.command,
                };
                let bytes = serde_json::to_vec(&frame).map_err(|_| "invalid-intent")?;
                self.rich
                    .dispatch_renderer_json(&bytes)
                    .map_err(|error| error.code())?;
                Ok(())
            }
            "panel-command" => {
                let request: PanelIntent = decode_intent(intent)?;
                if request.kind != "panel-command" {
                    return Err("invalid-intent");
                }
                let sequence = next_counter(&mut self.panels_sequence)?;
                self.panels
                    .dispatch(PanelRequest {
                        protocol: UI_PROTOCOL_VERSION,
                        host_epoch: self.epoch,
                        sequence,
                        command: request.command,
                    })
                    .map_err(|error| error.code())?;
                Ok(())
            }
            "focus-event" => {
                let request: FocusIntent = decode_intent(intent)?;
                if request.kind != "focus-event" {
                    return Err("invalid-intent");
                }
                let sequence = next_counter(&mut self.focus_sequence)?;
                self.focus
                    .handle(UiKeyFrame {
                        host_epoch: self.epoch,
                        sequence,
                        event: request.event,
                    })
                    .map_err(|error| error.code())?;
                Ok(())
            }
            "set-locale" => {
                let request: LocaleIntent = decode_intent(intent)?;
                if request.kind != "set-locale" {
                    return Err("invalid-intent");
                }
                let language = UiLanguage::parse(&request.tag).ok_or("unsupported-locale")?;
                let mut catalog = self.panels.snapshot().catalog;
                catalog.preferences.locale = language.tag().to_owned();
                self.panels
                    .replace_catalog(catalog)
                    .map_err(|error| error.code())?;
                self.language = language;
                Ok(())
            }
            "dialog-action" => {
                let request: DialogIntent = decode_intent(intent)?;
                if request.kind != "dialog-action" {
                    return Err("invalid-intent");
                }
                match request.action.as_str() {
                    "open" => self.focus.open_dialog().map_err(|error| error.code())?,
                    "confirm" | "cancel" => {
                        self.focus.close_dialog().map_err(|error| error.code())?
                    }
                    _ => return Err("invalid-dialog-action"),
                }
                Ok(())
            }
            "open-rich" => {
                let request: OpenRichIntent = decode_intent(intent)?;
                if request.kind != "open-rich" {
                    return Err("invalid-intent");
                }
                let panel = match request.surface.as_str() {
                    "editor" => RichPanel::Editor {
                        key: "scratch-editor".into(),
                        title: "Untitled editor".into(),
                        language: "text".into(),
                        text: String::new(),
                        read_only: false,
                    },
                    "terminal" => {
                        let session = self
                            .active_history_session
                            .as_deref()
                            .ok_or("no-active-session")?;
                        RichPanel::Terminal {
                            key: "host-terminal-output".into(),
                            title: "Recorded session terminal output".into(),
                            output: terminal_output_for_session(store, session)
                                .map_err(|_| "terminal-output-unavailable")?,
                        }
                    }
                    "pdf" => {
                        let session = self
                            .active_history_session
                            .clone()
                            .ok_or("no-active-session")?;
                        self.clear_pdf_pages();
                        let pages = authorized_pdf_pages(store, &session)
                            .map_err(|_| "pdf-asset-unavailable")?;
                        if pages.is_empty() {
                            return Err("pdf-asset-unavailable");
                        }
                        let page_count = pages.len() as u32;
                        self.pdf_pages = pages;
                        RichPanel::Pdf {
                            key: "host-pdf-raster".into(),
                            title: "Host-authorized PDF page".into(),
                            asset_id: "host-pdf-raster".into(),
                            page: 1,
                            page_count,
                        }
                    }
                    "math" => RichPanel::Math {
                        key: "math-preview".into(),
                        title: "Math preview".into(),
                        source: "x^2 + y^2 = z^2".into(),
                    },
                    _ => return Err("invalid-rich-surface"),
                };
                let key = panel.key().to_owned();
                self.rich
                    .publish_from_host(panel)
                    .map_err(|error| error.code())?;
                // Published host panels must become the actually visible surface.
                // Use the accepted 15C typed activation sequence/revision contract.
                let sequence = next_counter(&mut self.rich_sequence)?;
                let frame = RichRendererRequest {
                    protocol: RICH_PROTOCOL_VERSION,
                    host_epoch: self.epoch,
                    sequence,
                    command: RichRendererCommand::Activate {
                        key,
                        expected_revision: self.rich.snapshot().revision,
                    },
                };
                let bytes = serde_json::to_vec(&frame).map_err(|_| "invalid-intent")?;
                self.rich
                    .dispatch_renderer_json(&bytes)
                    .map_err(|error| error.code())?;
                Ok(())
            }
            _ => Err("unknown-intent"),
        }
    }

    fn open_session(
        &mut self,
        store: &SessionStore,
        session_id: &str,
        expected_navigation_revision: u64,
    ) -> Result<(), &'static str> {
        if !model::session_id(session_id)
            || store
                .summary(session_id)
                .map_err(|_| "session-unavailable")?
                .is_none()
        {
            return Err("session-not-found");
        }
        let sequence = next_counter(&mut self.navigation_sequence)?;
        self.navigation
            .dispatch(UiRequest {
                protocol: UI_PROTOCOL_VERSION,
                host_epoch: self.epoch,
                sequence,
                command: UiCommand::Navigate {
                    route: UiRoute::Conversation {
                        session_id: session_id.to_owned(),
                    },
                    expected_revision: expected_navigation_revision,
                },
            })
            .map_err(|error| error.code())?;

        // Erase all session-owned rich panels and associated raster bytes
        // before displaying the next session; not just the history list.
        for panel in self.rich.snapshot().panels {
            self.rich
                .remove_from_host(panel.key())
                .map_err(|error| error.code())?;
        }
        // Erase the prior visible page and cache before touching the new
        // session. A failed read cannot leave previous-session text visible.
        self.clear_history();
        self.clear_pdf_pages();
        self.active_history_session = Some(session_id.to_owned());
        let ticket = self
            .conversation
            .open_from_host(session_id)
            .map_err(|error| error.code())?;
        let events = match store.recent(session_id, HISTORY_WINDOW_LIMIT, None) {
            Ok(events) => events,
            Err(_) => {
                let _ = self.conversation.fail_from_host(&ticket);
                return Err("history-unavailable");
            }
        };
        let history = events
            .iter()
            .enumerate()
            .map(|(index, event)| history_entry(index, event))
            .collect::<Vec<_>>();
        self.history = history;
        let start = if self.history.is_empty() {
            0
        } else {
            ((self.history.len() as u64 - 1) / PAGE_LIMIT) * PAGE_LIMIT
        };
        let page = page_at(&self.history, start).ok_or("history-page-invalid")?;
        self.conversation
            .publish_from_host(&ticket, session_id, page)
            .map_err(|error| error.code())?;
        Ok(())
    }

    fn clear_pdf_pages(&mut self) {
        for page in &mut self.pdf_pages {
            page.bytes.zeroize();
        }
        self.pdf_pages.clear();
    }

    fn clear_history(&mut self) {
        for entry in &mut self.history {
            entry.text.zeroize();
            entry.message_id.zeroize();
            if let Some(turn_id) = &mut entry.turn_id {
                turn_id.zeroize();
            }
        }
        self.history.clear();
    }

    fn move_to_page(&mut self, target: u64, expected_revision: u64) -> Result<(), &'static str> {
        let current = self.conversation.snapshot();
        if current.revision != expected_revision {
            return Err("stale-revision");
        }
        if target > self.history.len() as u64
            || !target.is_multiple_of(PAGE_LIMIT)
            || (target == self.history.len() as u64 && !self.history.is_empty())
        {
            return Err("invalid-history-page");
        }
        let session_id = self
            .active_history_session
            .clone()
            .ok_or("no-active-session")?;
        for _ in 0..=HISTORY_WINDOW_LIMIT.div_ceil(PAGE_LIMIT as usize) {
            let current = self.conversation.snapshot();
            let current_start = current.page.as_ref().map(|page| page.start).unwrap_or(0);
            if current_start == target {
                return Ok(());
            }
            let direction = if target < current_start {
                "older"
            } else {
                "newer"
            };
            let command = if direction == "older" {
                ConversationCommand::LoadOlder {
                    expected_revision: current.revision,
                }
            } else {
                ConversationCommand::LoadNewer {
                    expected_revision: current.revision,
                }
            };
            let sequence = next_counter(&mut self.conversation_sequence)?;
            let reply = self.conversation.dispatch(ConversationRequest {
                protocol: UI_PROTOCOL_VERSION,
                host_epoch: self.epoch,
                sequence,
                command,
            });
            // A well-formed typed request consumes its sequence even when
            // rejected for a stale revision or a history boundary.
            match reply {
                Ok(_) => {}
                Err(error) => return Err(error.code()),
            }
            let ticket = self
                .conversation
                .snapshot()
                .pending
                .clone()
                .ok_or("history-ticket-missing")?;
            let next_start = match direction {
                "older" => current_start.saturating_sub(PAGE_LIMIT),
                _ => current
                    .page
                    .as_ref()
                    .map(|page| page.start + page.entries.len() as u64)
                    .unwrap_or(0),
            };
            let page = page_at(&self.history, next_start).ok_or("history-page-invalid")?;
            self.conversation
                .publish_from_host(&ticket, &session_id, page)
                .map_err(|error| error.code())?;
        }
        Err("history-page-step-limit")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NavigateIntent {
    kind: String,
    route: UiRoute,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectSessionIntent {
    kind: String,
    session_id: String,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoadPageIntent {
    kind: String,
    start: u64,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RichIntent {
    kind: String,
    command: RichRendererCommand,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PanelIntent {
    kind: String,
    command: PanelCommand,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FocusIntent {
    kind: String,
    event: UiKeyEvent,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocaleIntent {
    kind: String,
    tag: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DialogIntent {
    kind: String,
    action: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenRichIntent {
    kind: String,
    surface: String,
}

fn decode_intent<T: DeserializeOwned>(value: Value) -> Result<T, &'static str> {
    serde_json::from_value(value).map_err(|_| "invalid-intent")
}

fn read_sessions(store: &SessionStore) -> io::Result<Vec<Value>> {
    let value = store.list_page(SESSION_LIST_LIMIT, None, None)?;
    let sessions = value["sessions"].as_array().cloned().unwrap_or_default();
    Ok(sessions
        .into_iter()
        .filter_map(|row| {
            let id = row["id"].as_str()?.to_owned();
            if !model::session_id(&id) {
                return None;
            }
            let title = safe_text(
                row["title"]
                    .as_str()
                    .or_else(|| row["name"].as_str())
                    .unwrap_or("Conversation"),
                128,
            );
            Some(json!({
                "id": id,
                "title": if title.is_empty() { "Conversation".to_owned() } else { title },
                "updatedAt": row["updatedAt"].as_u64().unwrap_or(0)
            }))
        })
        .collect())
}

fn history_entry(index: usize, event: &Value) -> ConversationEntry {
    let kind = event["kind"].as_str().unwrap_or("activity");
    let (role, text) = match kind {
        "user_message" => (
            ConversationRole::User,
            event["message"]["text"].as_str().unwrap_or(""),
        ),
        "assistant_message" => (
            ConversationRole::Assistant,
            event["message"]["text"].as_str().unwrap_or(""),
        ),
        "tool_call" => (
            ConversationRole::Tool,
            event["call"]["result"]["text"]
                .as_str()
                .or_else(|| event["call"]["summary"]["text"].as_str())
                .unwrap_or("Tool activity"),
        ),
        "chat_error" => (
            ConversationRole::Error,
            event["message"]["text"].as_str().unwrap_or("Error"),
        ),
        _ => (
            ConversationRole::Activity,
            event["message"]["text"].as_str().unwrap_or(kind),
        ),
    };
    let safe = safe_text(text, cos_core::ui_conversation::MAX_CONVERSATION_TEXT_BYTES);
    ConversationEntry {
        position: index as u64,
        message_id: format!("msg-{index:04}"),
        turn_id: None,
        role,
        truncated: safe.len() < text.len() || event["message"]["truncated"] == true,
        text: safe,
    }
}

fn page_at(history: &[ConversationEntry], start: u64) -> Option<ConversationPage> {
    let total = history.len() as u64;
    if start > total || (total > 0 && start == total) || !start.is_multiple_of(PAGE_LIMIT) {
        return None;
    }
    let end = start.saturating_add(PAGE_LIMIT).min(total);
    Some(ConversationPage {
        start,
        total,
        entries: history[start as usize..end as usize].to_vec(),
    })
}

fn safe_text(value: &str, max_bytes: usize) -> String {
    let mut output = String::with_capacity(value.len().min(max_bytes));
    for character in value.chars() {
        if (character.is_control() && character != '\n' && character != '\t')
            || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            continue;
        }
        if output.len().saturating_add(character.len_utf8()) > max_bytes {
            break;
        }
        output.push(character);
    }
    output
}

fn localization_json(language: UiLanguage) -> Value {
    let pairs = [
        (UiTextKey::Home, "home"),
        (UiTextKey::Conversations, "conversations"),
        (UiTextKey::Projects, "projects"),
        (UiTextKey::Files, "files"),
        (UiTextKey::Agents, "agents"),
        (UiTextKey::Skills, "skills"),
        (UiTextKey::Plugins, "plugins"),
        (UiTextKey::Settings, "settings"),
        (UiTextKey::Activity, "activity"),
        (UiTextKey::Search, "search"),
        (UiTextKey::NewChat, "newChat"),
        (UiTextKey::Save, "save"),
        (UiTextKey::Cancel, "cancel"),
        (UiTextKey::Close, "close"),
        (UiTextKey::Navigation, "navigation"),
        (UiTextKey::MainContent, "mainContent"),
        (UiTextKey::MessageInput, "messageInput"),
        (UiTextKey::Confirm, "confirm"),
    ];
    let mut texts = serde_json::Map::new();
    for (key, name) in pairs {
        texts.insert(name.into(), json!(ui_localization::text(language, key)));
    }
    json!({ "tag": language.tag(), "texts": texts })
}

fn next_counter(counter: &mut u64) -> Result<u64, &'static str> {
    let next = counter
        .checked_add(1)
        .filter(|value| *value <= cos_core::ui_contract::MAX_JS_SAFE_INTEGER)
        .ok_or("counter-exhausted")?;
    *counter = next;
    Ok(next)
}

// PDF document parsing/rasterization never runs in WebView2. The trusted Rust
// profile stores pre-rendered image/png page assets explicitly marked
// role=pdf-page in session events. A foreign session ID or arbitrary file path
// is never accepted. A PDF without host-rendered pages is rejected, not mocked.
struct AuthorizedPdfPage {
    page: u32,
    bytes: Vec<u8>,
}
const MAX_AUTHORIZED_PDF_PAGES: usize = 16;
const MAX_PDF_PNG_BYTES: usize = 96 * 1024;

fn authorized_pdf_pages(store: &SessionStore, id: &str) -> io::Result<Vec<AuthorizedPdfPage>> {
    if !model::session_id(id) || store.summary(id)?.is_none() {
        return Err(invalid("PDF session is outside the selected profile"));
    }
    let events = store.recent(id, 128, None)?;
    let mut ids = std::collections::BTreeMap::new();
    for event in events {
        if let Some(assets) = event["assets"].as_array() {
            for asset in assets {
                if asset["role"] != "pdf-page" || asset["mimeType"] != "image/png" {
                    continue;
                }
                let Some(page) = asset["page"]
                    .as_u64()
                    .filter(|n| *n > 0 && *n <= MAX_AUTHORIZED_PDF_PAGES as u64)
                else {
                    continue;
                };
                let Some(asset_id) = asset["id"].as_str() else {
                    continue;
                };
                // Last entry is authoritative within this selected session.
                ids.insert(page as u32, asset_id.to_owned());
            }
        }
    }
    if ids.is_empty() || ids.len() > MAX_AUTHORIZED_PDF_PAGES {
        return Ok(Vec::new());
    }
    let mut pages = Vec::new();
    for (index, (page, asset_id)) in ids.iter().enumerate() {
        if *page as usize != index + 1 {
            return Err(invalid("PDF page raster indices must be contiguous"));
        }
        let bytes = store
            .read_asset(id, asset_id, Some(MAX_PDF_PNG_BYTES))?
            .ok_or_else(|| invalid("PDF page asset unavailable in authorized session"))?;
        if bytes.len() < 33
            || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
            || &bytes[12..16] != b"IHDR"
            || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) == 0
            || u32::from_be_bytes(bytes[20..24].try_into().unwrap()) == 0
            || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) > 2048
            || u32::from_be_bytes(bytes[20..24].try_into().unwrap()) > 2048
        {
            return Err(invalid("PDF page asset is not a bounded PNG raster"));
        }
        pages.push(AuthorizedPdfPage { page: *page, bytes });
    }
    Ok(pages)
}

fn base64_encode_png(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(T[(b0 >> 2) as usize] as char);
        out.push(T[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(b2 & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// An output-only terminal surface. The Rust host projects recorded tool-call
/// results from the *selected* SessionStore; C#/WebView2 cannot execute commands.
fn terminal_output_for_session(store: &SessionStore, session_id: &str) -> io::Result<String> {
    const MAX_RECORDS: usize = 64;
    const MAX_TERMINAL_BYTES: usize = 16 * 1024;
    if !model::session_id(session_id) || store.summary(session_id)?.is_none() {
        return Err(invalid("Terminal session is outside the selected profile"));
    }
    let events = store.recent(session_id, MAX_RECORDS, None)?;
    let mut lines = Vec::new();
    let mut remaining = MAX_TERMINAL_BYTES;
    for item in events {
        if item["kind"] != "tool_call" {
            continue;
        }
        let title = item["call"]["summary"]["title"]
            .as_str()
            .unwrap_or("Tool output");
        let result = item["call"]["result"]["text"].as_str().unwrap_or("");
        let title = safe_text(title, 128);
        let redacted = cos_core::logging::redact_credentials(result);
        let cleaned = safe_text(&redacted, remaining.min(4096));
        if cleaned.is_empty() {
            continue;
        }
        let line = format!("{title}\n{cleaned}");
        if line.len() > remaining {
            break;
        }
        remaining -= line.len();
        lines.push(line);
        if remaining < 256 {
            break;
        }
    }
    Ok(if lines.is_empty() {
        "No terminal output recorded in this session".to_owned()
    } else {
        lines.join("\n\n")
    })
}

fn take_option(args: &mut Vec<String>, option: &str) -> io::Result<Option<String>> {
    if let Some(index) = args.iter().position(|arg| arg == option) {
        if index + 1 >= args.len() {
            return Err(invalid("Option requires a value"));
        }
        let value = args.remove(index + 1);
        args.remove(index);
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

fn write_json_line<W: Write>(writer: &mut W, value: &Value) -> io::Result<()> {
    let encoded = serde_json::to_vec(value).map_err(io::Error::other)?;
    if encoded.len() > 4096 {
        return Err(invalid("Renderer bootstrap exceeds its limit"));
    }
    writer.write_all(&encoded)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn read_bounded_line<R: BufRead>(reader: &mut R, max_bytes: usize) -> io::Result<Option<Vec<u8>>> {
    let mut output = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if output.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Truncated renderer frame",
                ))
            };
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if output.len().saturating_add(take) > max_bytes.saturating_add(1) {
            return Err(invalid("Renderer frame exceeds the byte limit"));
        }
        let complete = available.get(take - 1) == Some(&b'\n');
        output.extend_from_slice(&available[..take]);
        reader.consume(take);
        if complete {
            output.pop();
            if output.last() == Some(&b'\r') {
                output.pop();
            }
            if output.len() > max_bytes {
                return Err(invalid("Renderer frame exceeds the byte limit"));
            }
            return Ok(Some(output));
        }
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl Drop for RendererHostState {
    fn drop(&mut self) {
        self.clear_history();
        self.clear_pdf_pages();
        if let Some(session_id) = &mut self.active_history_session {
            session_id.zeroize();
        }
        for session in &mut self.sessions {
            zeroize_json(session);
        }
    }
}

fn zeroize_json(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(items) => items.iter_mut().for_each(zeroize_json),
        Value::Object(items) => items.values_mut().for_each(zeroize_json),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_page_window_is_bounded_to_twenty_four_of_six_thousand() {
        let history = (0..HISTORY_WINDOW_LIMIT)
            .map(|index| ConversationEntry {
                position: index as u64,
                message_id: format!("msg-{index:04}"),
                turn_id: None,
                role: ConversationRole::User,
                text: format!("Synthetic message {index}"),
                truncated: false,
            })
            .collect::<Vec<_>>();
        let page = page_at(&history, 5_976).unwrap();
        assert_eq!(page.total, 6_000);
        assert_eq!(page.entries.len(), 24);
        assert_eq!(page.entries.first().unwrap().position, 5_976);
        assert_eq!(page.entries.last().unwrap().position, 5_999);
        assert!(page_at(&history, 5_975).is_none());
        assert!(page_at(&history, 6_000).is_none());
    }

    #[test]
    fn display_projection_removes_control_and_bidi_overrides_and_bounds_utf8() {
        let unsafe_text = "safe\u{202e} text\u{0000} and more";
        let projected = safe_text(unsafe_text, 10);
        assert!(!projected.contains('\u{202e}'));
        assert!(!projected.contains('\u{0000}'));
        assert!(projected.len() <= 10);
    }
    #[test]
    fn host_terminal_output_is_selected_session_only_and_bounded() {
        use cos_store::CreateOptions;
        let temp = tempfile::tempdir().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();
        for id in ["synthetic-terminal-a", "synthetic-terminal-b"] {
            store
                .create(CreateOptions {
                    id: Some(id.into()),
                    ..Default::default()
                })
                .unwrap();
        }
        for (id, word) in [
            ("synthetic-terminal-a", "private-output-a"),
            ("synthetic-terminal-b", "private-output-b"),
        ] {
            store
                .append(
                    id,
                    json!({
                        "kind":"tool_call", "time":10, "call":{
                            "callId": format!("run-{word}"),
                            "summary":{"title":"Run synthetic command"},
                            "args":{"text":"safe","truncated":false,"chars":4},
                            "result":{"text":word,"truncated":false,"chars":word.len()}
                        }
                    }),
                )
                .unwrap();
        }
        let a = terminal_output_for_session(&store, "synthetic-terminal-a").unwrap();
        assert!(a.contains("private-output-a"));
        assert!(!a.contains("private-output-b"));
        assert!(
            terminal_output_for_session(&store, "synthetic-terminal-b")
                .unwrap()
                .contains("private-output-b")
        );
        assert!(terminal_output_for_session(&store, "synthetic-terminal-foreign").is_err());
    }

    #[test]
    fn bounded_reader_timeout_is_not_silently_accepted() {
        let (sender, receiver) = mpsc::channel::<io::Result<Option<Vec<u8>>>>();
        assert!(receive_frame(&receiver, Duration::from_millis(20)).is_err());
        drop(sender);
        assert!(receive_frame(&receiver, Duration::from_millis(20)).is_err());
    }

    #[test]
    fn child_shutdown_timeout_kills_and_reaps() {
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new("powershell");
            command.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 20"]);
            command
        };
        #[cfg(not(windows))]
        let mut command = {
            let mut command = Command::new("sleep");
            command.arg("20");
            command
        };
        let mut child = ReapedChild(command.spawn().unwrap());
        assert!(child.wait_bounded(Duration::from_millis(50)).is_err());
        assert!(child.0.try_wait().unwrap().is_some());
    }

    #[cfg(windows)]
    #[test]
    fn hostile_native_child_bootstrap_failure_matrix() {
        let exe = match std::env::var("COSOS_15HOST_E2E_EXE") {
            Ok(path) => path,
            Err(_) => return,
        };
        let exe_sha = std::env::var("COSOS_15HOST_E2E_EXE_SHA").unwrap();
        let manifest_sha = std::env::var("COSOS_15HOST_E2E_MANIFEST_SHA").unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::write(
            root.join("15host-synthetic-fixture.marker"),
            b"15HOST_SYNTHETIC_CLOUD_E2E_V1",
        )
        .unwrap();
        let store = SessionStore::open(root).unwrap();
        for kind in ["early-exit", "malformed", "replay", "stale", "hang"] {
            let now = Instant::now();
            let outcome = launch(
                &store,
                root,
                json!({"ui":{"language":"en"}}),
                vec![
                    "--renderer".into(),
                    exe.clone(),
                    "--renderer-sha256".into(),
                    exe_sha.clone(),
                    "--renderer-manifest-sha256".into(),
                    manifest_sha.clone(),
                    "--synthetic-15host-inject".into(),
                    kind.into(),
                ],
            );
            assert!(outcome.is_err(), "hostile {kind} child must fail closed");
            assert!(
                now.elapsed() < Duration::from_secs(12),
                "hostile {kind} child hung"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn hosted_two_profile_native_e2e_foreign_denial_and_stale_clearing() {
        use cos_store::CreateOptions;
        let exe = match std::env::var("COSOS_15HOST_E2E_EXE") {
            Ok(value) => value,
            Err(_) => return,
        };
        let exe_sha = std::env::var("COSOS_15HOST_E2E_EXE_SHA").unwrap();
        let manifest_sha = std::env::var("COSOS_15HOST_E2E_MANIFEST_SHA").unwrap();
        // Separate explicitly selected profile roots. No owner data.
        for tag in ["a", "b"] {
            let fixture = tempfile::tempdir().unwrap();
            let root = fixture.path();
            std::fs::write(
                root.join("15host-synthetic-fixture.marker"),
                b"15HOST_SYNTHETIC_CLOUD_E2E_V1",
            )
            .unwrap();
            let store = SessionStore::open(root).unwrap();
            let other = if tag == "a" { "b" } else { "a" };
            for role in ["primary", "secondary"] {
                let id = format!("synthetic-15host-{tag}-{role}");
                store
                    .create(CreateOptions {
                        id: Some(id.clone()),
                        title: Some(format!("Cloud-only {tag} {role}")),
                        ..Default::default()
                    })
                    .unwrap();
                let png_fixture = std::fs::read(
                    std::env::var("COSOS_15HOST_E2E_PNG")
                        .expect("Synthetic PDF page raster fixture missing"),
                )
                .unwrap();
                let png_asset = store.write_asset(&id, &png_fixture, "image/png").unwrap();
                for index in 0..24 {
                    let content = format!("ISOLATED-{tag}-{role}-{index:02}");
                    let event = if index == 0 {
                        json!({
                            "kind":"note", "source":"15host-cloud-synthetic",
                            "time":index + 1,
                            "message":{"text":content,"truncated":false,"chars":content.len()},
                            "assets":[{
                                "id":png_asset["id"],"mimeType":"image/png",
                                "role":"pdf-page","page":1
                            }]
                        })
                    } else if index == 23 {
                        let result = format!("HOST-TERMINAL-{tag}-{role}");
                        json!({
                            "kind":"tool_call", "time":index + 1,
                            "call":{
                                "callId":format!("synthetic-call-{tag}-{role}"),
                                "summary":{"title":"Synthetic recorded terminal"},
                                "args":{"text":"no execution","truncated":false,"chars":12},
                                "result":{"text":result,"truncated":false,"chars":result.len()}
                            }
                        })
                    } else {
                        json!({
                            "kind":"note", "source":"15host-cloud-synthetic",
                            "time":index + 1,
                            "message":{"text":content,"truncated":false,"chars":content.len()}
                        })
                    };
                    store.append(&id, event).unwrap();
                }
            }
            let forbidden = format!("synthetic-15host-{other}-primary");
            assert!(store.summary(&forbidden).unwrap().is_none());
            // Check the authorized host projection before the child IPC test.
            // This differentiates a missing asset from a native renderer crash.
            let mut projection =
                RendererHostState::new(12345, &store, Theme::Dark, UiLanguage::select(Some("en")))
                    .unwrap();
            let selected = format!("synthetic-15host-{tag}-secondary");
            let expected_revision = projection.navigation.snapshot().revision;
            projection
                .handle_intent(
                    &store,
                    json!({
                        "kind":"select-session", "sessionId":selected,
                        "expectedRevision":expected_revision
                    }),
                )
                .unwrap();
            projection
                .handle_intent(
                    &store,
                    json!({
                        "kind":"open-rich","surface":"pdf"
                    }),
                )
                .unwrap();
            let state = projection.state_json().unwrap();
            assert!(state["pdfPage"].is_object(), "Host PDF raster projection absent");
            assert_eq!(state["pdfPage"]["format"], "image/png");
            assert!(!state["pdfPage"]["base64"].as_str().unwrap().is_empty());
            let proof = launch(
                &store,
                root,
                json!({"ui":{"language":"en"}}),
                vec![
                    "--renderer".into(),
                    exe.clone(),
                    "--renderer-sha256".into(),
                    exe_sha.clone(),
                    "--renderer-manifest-sha256".into(),
                    manifest_sha.clone(),
                    "--synthetic-15host-ab".into(),
                ],
            )
            .unwrap();
            assert_eq!(proof["authenticated"], true);
            assert_eq!(proof["abIsolationComplete"], true);
            assert!(store.summary(&forbidden).unwrap().is_none());
        }
    }

    #[cfg(windows)]
    #[test]
    fn hosted_native_product_path_synthetic_six_thousand() {
        use cos_store::CreateOptions;
        let exe = match std::env::var("COSOS_15HOST_E2E_EXE") {
            Ok(path) => path,
            Err(_) => return, // General workspace tests do not have the Windows artifact.
        };
        let exe_sha = std::env::var("COSOS_15HOST_E2E_EXE_SHA").expect("E2E EXE hash required");
        let manifest_sha =
            std::env::var("COSOS_15HOST_E2E_MANIFEST_SHA").expect("E2E manifest digest required");
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::write(
            root.join("15host-synthetic-fixture.marker"),
            b"15HOST_SYNTHETIC_CLOUD_E2E_V1",
        )
        .unwrap();
        let store = SessionStore::open(root).unwrap();
        let session_id = "synthetic-15host-e2e";
        store
            .create(CreateOptions {
                id: Some(session_id.into()),
                title: Some("15HOST cloud-only synthetic session".into()),
                ..Default::default()
            })
            .unwrap();
        for index in 0..HISTORY_WINDOW_LIMIT {
            let content = format!("SYNTHETIC-15HOST-E2E-{index:04}");
            store
                .append(
                    session_id,
                    json!({
                        "kind":"note", "source":"15host-cloud-synthetic", "time":index + 1,
                        "message":{"text":content,"truncated":false,"chars":content.len()}
                    }),
                )
                .unwrap();
        }
        let reply = launch(
            &store,
            root,
            json!({"ui":{"language":"en","theme":"dark"}}),
            vec![
                "--renderer".into(),
                exe,
                "--renderer-sha256".into(),
                exe_sha,
                "--renderer-manifest-sha256".into(),
                manifest_sha,
                "--synthetic-15host-probe".into(),
            ],
        )
        .unwrap();
        assert_eq!(reply["authenticated"], true);
        assert_eq!(reply["syntheticProbeComplete"], true);
    }
}
