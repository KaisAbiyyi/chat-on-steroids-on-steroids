use crate::{
    http::{json_response, safe_equal, token},
    kernel::Kernel,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Request, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use cos_core::persistence::{InstanceLease, atomic_write, invalid, owned_path};
use cos_store::ReadOptions;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Semaphore, oneshot};
use tokio_stream::wrappers::ReceiverStream;
pub type Action = Arc<dyn Fn(&str, Value) -> io::Result<Value> + Send + Sync>;
const ROUTES: &[&str] = &[
    "/v1/health",
    "/v1/status",
    "/v1/sessions",
    "/v1/sessions/{id}",
    "/v1/sessions/{id}/events",
    "/v1/sessions/{id}/events/stream",
];
struct Control {
    kernel: Arc<Kernel>,
    port: u16,
    token: String,
    started: Instant,
    stopping: AtomicBool,
    rate: Mutex<VecDeque<Instant>>,
    action_rate: Mutex<VecDeque<Instant>>,
    heavy: Arc<Semaphore>,
    streams: Arc<Semaphore>,
    actions: Arc<Semaphore>,
    pending: Arc<Semaphore>,
    backend: Option<Action>,
}
pub struct Service {
    pub port: u16,
    state: Arc<Control>,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<io::Result<()>>,
    endpoint: PathBuf,
    _lease: InstanceLease,
}
impl Service {
    pub async fn start(kernel: Arc<Kernel>) -> io::Result<Self> {
        Self::start_with_actions(kernel, None).await
    }
    pub async fn start_with_actions(
        kernel: Arc<Kernel>,
        backend: Option<Action>,
    ) -> io::Result<Self> {
        if kernel.config.get()["controlApi"]["enabled"] != true {
            return Err(invalid("Control API is disabled"));
        }
        let profile = kernel.config.directory();
        let relative = std::path::Path::new("control-api");
        let dir = owned_path(profile, relative, true)?;
        match std::fs::create_dir(&dir) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
        // Validate from the existing profile owner; never make a link target the new root.
        owned_path(profile, relative, false)?;
        if !std::fs::metadata(&dir)?.is_dir() {
            return Err(invalid("Control API path is not a directory"));
        }
        let lease = InstanceLease::acquire(&dir)?;
        let token_path = owned_path(profile, std::path::Path::new("control-api/token"), true)?;
        let value = token()?;
        atomic_write(&token_path, value.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600))?;
        }
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let endpoint = owned_path(
            profile,
            std::path::Path::new("control-api/endpoint.json"),
            true,
        )?;
        atomic_write(
            &endpoint,
            &serde_json::to_vec(
                &json!({"protocol":1,"port":port,"pid":std::process::id(),"appVersion":cos_core::APP_VERSION,"startedAt":chrono::Utc::now().to_rfc3339()}),
            )?,
        )?;
        let state = Arc::new(Control {
            kernel,
            port,
            token: value,
            started: Instant::now(),
            stopping: AtomicBool::new(false),
            rate: Mutex::new(VecDeque::new()),
            action_rate: Mutex::new(VecDeque::new()),
            heavy: Arc::new(Semaphore::new(2)),
            streams: Arc::new(Semaphore::new(4)),
            actions: Arc::new(Semaphore::new(1)),
            pending: Arc::new(Semaphore::new(4)),
            backend,
        });
        let app = Router::new().fallback(handle).with_state(state.clone());
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await
        });
        Ok(Self {
            port,
            state,
            stop: Some(tx),
            task,
            endpoint,
            _lease: lease,
        })
    }
    pub async fn stop(mut self) {
        self.state.stopping.store(true, Ordering::Release);
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if tokio::time::timeout(Duration::from_secs(2), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
        }
        let _ = std::fs::remove_file(&self.endpoint);
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.state.stopping.store(true, Ordering::Release);
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        let _ = std::fs::remove_file(&self.endpoint);
    }
}
fn charged(rate: &Mutex<VecDeque<Instant>>, max: usize) -> bool {
    let now = Instant::now();
    let mut rate = rate.lock().unwrap();
    while rate
        .front()
        .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
    {
        rate.pop_front();
    }
    if rate.len() >= max {
        return false;
    }
    rate.push_back(now);
    true
}
fn actions_enabled(state: &Control) -> bool {
    let c = state.kernel.config.get();
    c["controlApi"]["enabled"] == true
        && c["controlApi"]["allowActions"] == true
        && !state.stopping.load(Ordering::Acquire)
}
fn action_path(path: &str) -> bool {
    path == "/v1/inputs"
        || (path.starts_with("/v1/inputs/") && path.ends_with("/cancel"))
        || (path.starts_with("/v1/workers/")
            && ["/spawn", "/message", "/ack"]
                .iter()
                .any(|p| path.ends_with(p)))
        || (path.starts_with("/v1/sessions/") && path.ends_with("/stop"))
}
async fn handle(State(state): State<Arc<Control>>, request: Request<Body>) -> Response {
    if state.stopping.load(Ordering::Acquire)
        || state.kernel.config.get()["controlApi"]["enabled"] != true
    {
        return json_response(503, json!({"error":"server_stopping"}));
    }
    if request.headers().contains_key(header::ORIGIN) {
        return json_response(403, json!({"error":"origin_forbidden"}));
    }
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host != format!("127.0.0.1:{}", state.port) && host != format!("localhost:{}", state.port) {
        return json_response(403, json!({"error":"host_forbidden"}));
    }
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !safe_equal(bearer, &state.token) {
        return json_response(401, json!({"error":"unauthorized"}));
    }
    let target = request.uri().to_string();
    if !target.starts_with('/') || target.starts_with("//") || target.contains('\\') {
        return json_response(400, json!({"error":"invalid_target"}));
    }
    let path = request.uri().path().to_owned();
    if request.method() == "POST" && action_path(&path) {
        if !actions_enabled(&state) {
            return json_response(403, json!({"error":"actions_disabled"}));
        }
        if request.uri().query().is_some() {
            return json_response(400, json!({"error":"invalid_query"}));
        }
        if !charged(&state.action_rate, 30) {
            return json_response(429, json!({"error":"rate_limited"}));
        }
        let pending = match state.pending.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => return json_response(503, json!({"error":"busy"})),
        };
        let bytes = match tokio::time::timeout(
            Duration::from_secs(5),
            to_bytes(request.into_body(), 512 * 1024),
        )
        .await
        {
            Ok(Ok(b)) => b,
            Ok(Err(_)) => return json_response(413, json!({"error":"payload_too_large"})),
            Err(_) => return json_response(408, json!({"error":"body_timeout"})),
        };
        let body = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return json_response(400, json!({"error":"invalid_json"})),
        };
        let serial = match state.actions.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => return json_response(503, json!({"error":"server_stopping"})),
        };
        if !actions_enabled(&state) {
            return json_response(403, json!({"error":"actions_disabled"}));
        }
        let Some(backend) = state.backend.clone() else {
            return json_response(
                503,
                json!({"error":"action_unavailable","detail":"The phase 9/11 action owner is not installed; no action ran"}),
            );
        };
        let operation = tokio::task::spawn_blocking(move || {
            let _pending = pending;
            let _serial = serial;
            backend(&path, body)
        });
        return match tokio::time::timeout(Duration::from_secs(20), operation).await {
            Ok(Ok(Ok(v))) => json_response(200, v),
            Ok(_) => json_response(500, json!({"error":"action_failed"})),
            Err(_) => json_response(
                504,
                json!({"error":"timeout","detail":"The action may still complete; inspect the owning operation before repeating it"}),
            ),
        };
    }
    if !charged(&state.rate, 600) {
        return json_response(429, json!({"error":"rate_limited"}));
    }
    if request.method() != "GET" {
        return json_response(405, json!({"error":"method_not_allowed"}));
    }
    if request.headers().contains_key(header::TRANSFER_ENCODING)
        || request
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|n| n > 0)
    {
        return json_response(413, json!({"error":"body_not_allowed"}));
    }
    if path == "/v1/health" {
        return json_response(
            200,
            json!({"ok":true,"protocol":1,"routes":ROUTES,"actions":{"enabled":actions_enabled(&state),"available":state.backend.is_some()},"pid":std::process::id(),"appVersion":cos_core::APP_VERSION,"uptimeSeconds":state.started.elapsed().as_secs()}),
        );
    }
    let query = form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect::<BTreeMap<_, _>>();
    if query
        .keys()
        .any(|k| !matches!(k.as_str(), "limit" | "from" | "cursor" | "projectId"))
    {
        return json_response(400, json!({"error":"invalid_query"}));
    }
    let stream = path
        .strip_prefix("/v1/sessions/")
        .and_then(|s| s.strip_suffix("/events/stream"));
    if let Some(id) = stream {
        let permit = match state.streams.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => return json_response(503, json!({"error":"busy"})),
        };
        if !cos_store::model::session_id(id) {
            return json_response(400, json!({"error":"invalid_session"}));
        }
        let mut from = query
            .get("from")
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(1);
        if let Some(last) = request
            .headers()
            .get("last-event-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
        {
            from = from.max(last.saturating_add(1));
        }
        let id = id.to_owned();
        if state.kernel.sessions.summary(&id).ok().flatten().is_none() {
            return json_response(404, json!({"error":"not_found"}));
        }
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        tokio::spawn(async move {
            let _permit = permit;
            let started = Instant::now();
            while !state.stopping.load(Ordering::Acquire)
                && started.elapsed() < Duration::from_secs(30 * 60)
            {
                let store = state.kernel.sessions.clone();
                let id = id.clone();
                let events = tokio::task::spawn_blocking(move || {
                    store.read(
                        &id,
                        ReadOptions {
                            from: Some(from),
                            limit: 100,
                            ..Default::default()
                        },
                    )
                })
                .await;
                match events {
                    Ok(Ok(events)) => {
                        for event in events {
                            let projected = event_view(&event);
                            from = from.max(event["seq"].as_u64().unwrap_or(0).saturating_add(1));
                            let frame = Event::default()
                                .id(event["seq"].to_string())
                                .event(event["kind"].as_str().unwrap_or("event"))
                                .data(projected.to_string());
                            if tx
                                .try_send(Ok::<_, std::convert::Infallible>(frame))
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    _ => break,
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            let _ = tx.try_send(Ok(Event::default()
                .event("end")
                .data("{\"reason\":\"stopped\"}")));
        });
        return Sse::new(ReceiverStream::new(rx))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response();
    }
    let permit = match state.heavy.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return json_response(503, json!({"error":"busy"})),
    };
    let work = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        read(&work.kernel, &path, &query)
    });
    match tokio::time::timeout(Duration::from_secs(15), result).await {
        Ok(Ok(Ok(v))) => json_response(200, v),
        Ok(Ok(Err(e))) => json_response(
            if e.kind() == io::ErrorKind::NotFound {
                404
            } else {
                400
            },
            json!({"error":"read_failed","detail":cos_core::logging::redact_credentials(&e.to_string())}),
        ),
        Ok(Err(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Err(_) => json_response(504, json!({"error":"timeout"})),
    }
}
fn text(value: &Value, max: usize) -> Value {
    value
        .as_str()
        .map(|s| {
            json!(
                cos_core::logging::redact_credentials(s)
                    .chars()
                    .take(max)
                    .collect::<String>()
            )
        })
        .unwrap_or(Value::Null)
}
fn session_view(s: &Value) -> Value {
    let mut result = json!({});
    for key in [
        "id",
        "status",
        "createdAt",
        "updatedAt",
        "endedAt",
        "eventCount",
        "projectId",
    ] {
        if let Some(value) = s.get(key) {
            result[key] = value.clone();
        }
    }
    result["title"] = text(&s["title"], 200);
    result
}
fn event_view(e: &Value) -> Value {
    let mut result = json!({});
    for key in ["seq", "time", "kind", "source", "state", "final", "turnId"] {
        if let Some(v) = e.get(key) {
            result[key] = v.clone();
        }
    }
    if e.get("message").is_some() {
        result["message"] =
            json!({"text":text(&e["message"]["text"],4000),"truncated":e["message"]["truncated"]});
    }
    if let Some(call) = e.get("call") {
        result["call"] = json!({"tool":call["tool"],"outcome":call["outcome"],"durationMs":call["durationMs"],"args":text(&call["args"]["text"],2000),"result":text(&call["result"]["text"],2000)});
    }
    result
}
fn read(kernel: &Kernel, path: &str, query: &BTreeMap<String, String>) -> io::Result<Value> {
    let limit = query
        .get("limit")
        .map(|n| n.parse::<usize>().map_err(|_| invalid("Invalid limit")))
        .transpose()?
        .unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(invalid("Invalid limit"));
    }
    if path == "/v1/status" {
        return Ok(
            json!({"appVersion":cos_core::APP_VERSION,"recording":kernel.config.get()["sessions"]["record"],"implementation":"Rust phases 4-6","browserReady":false,"tunnel":kernel.tunnel.status()}),
        );
    }
    if path == "/v1/sessions" {
        if limit > 50 {
            return Err(invalid("Invalid limit"));
        }
        let cursor = query
            .get("cursor")
            .map(|value| {
                let re = regex::Regex::new(r"^(\d{1,16})\.([0-9a-z-]{8,64})$").unwrap();
                let c = re
                    .captures(value)
                    .ok_or_else(|| invalid("Invalid session cursor"))?;
                let at = c[1]
                    .parse::<u64>()
                    .map_err(|_| invalid("Invalid session cursor"))?;
                if at > 9007199254740991 {
                    return Err(invalid("Invalid session cursor"));
                }
                Ok((at, c[2].to_owned()))
            })
            .transpose()?;
        let page =
            kernel
                .sessions
                .list_page(limit, cursor, query.get("projectId").map(String::as_str))?;
        let next = page["next"].as_object().map(|_| {
            format!(
                "{}.{}",
                page["next"]["updatedAt"],
                page["next"]["id"].as_str().unwrap_or("")
            )
        });
        return Ok(
            json!({"sessions":page["sessions"].as_array().unwrap().iter().map(session_view).collect::<Vec<_>>(),"nextCursor":next,"total":kernel.sessions.list()?.len(),"activeId":null}),
        );
    }
    if let Some(rest) = path.strip_prefix("/v1/sessions/") {
        let (id, events) = rest
            .strip_suffix("/events")
            .map(|id| (id, true))
            .unwrap_or((rest, false));
        if !cos_store::model::session_id(id) {
            return Err(invalid("Invalid session id"));
        }
        let session = kernel
            .sessions
            .summary(id)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Session not found"))?;
        if events {
            let from = query
                .get("from")
                .map(|n| {
                    n.parse::<u64>()
                        .map_err(|_| invalid("Invalid event cursor"))
                })
                .transpose()?
                .unwrap_or(1);
            let events = kernel.sessions.read(
                id,
                ReadOptions {
                    from: Some(from),
                    limit,
                    ..Default::default()
                },
            )?;
            let next = events
                .iter()
                .filter_map(|e| e["seq"].as_u64())
                .max()
                .map(|n| n + 1)
                .unwrap_or(from);
            return Ok(
                json!({"events":events.iter().map(event_view).collect::<Vec<_>>(),"nextFrom":next}),
            );
        }
        return Ok(session_view(&session));
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "Route not found"))
}
