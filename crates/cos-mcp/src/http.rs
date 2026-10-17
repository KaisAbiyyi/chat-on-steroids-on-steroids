pub fn valid_host(host: &str) -> bool {
    let (name, port) = if let Some(rest) = host.strip_prefix("[::1]") {
        ("[::1]", rest.strip_prefix(':'))
    } else {
        host.split_once(':')
            .map(|(a, b)| (a, Some(b)))
            .unwrap_or((host, None))
    };
    matches!(name, "localhost" | "127.0.0.1" | "[::1]")
        && port.is_none_or(|p| p.parse::<u16>().is_ok_and(|p| p > 0))
}
pub fn valid_origin(origin: Option<&str>) -> bool {
    origin.is_none_or(|origin| {
        origin
            .strip_prefix("http://")
            .or_else(|| origin.strip_prefix("https://"))
            .is_some_and(|host| !host.contains(['/', '?', '#', '@']) && valid_host(host))
    })
}
use crate::{
    kernel::{Kernel, Surface},
    protocol,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cos_core::secrets::{SecretCipher, SecretStore, UnavailableCipher, WindowsCipher};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, oneshot};
struct PublishedBody {
    bytes: Option<axum::body::Bytes>,
    publication: cos_exec::process::Publication,
}
impl tokio_stream::Stream for PublishedBody {
    type Item = Result<axum::body::Bytes, std::convert::Infallible>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let body = self.get_mut();
        std::task::Poll::Ready(if let Some(bytes) = body.bytes.take() {
            Some(Ok(bytes))
        } else {
            body.publication.commit();
            None
        })
    }
}
fn published(
    status: u16,
    mime: &'static str,
    payload: String,
    publication: cos_exec::process::Publication,
) -> Response {
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(PublishedBody {
            bytes: Some(payload.into()),
            publication,
        }),
    )
        .into_response()
}
pub const BODY_LIMIT: usize = 8 * 1024 * 1024;
pub fn token() -> io::Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
pub fn safe_equal(a: &str, b: &str) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(
            a.as_bytes().get(i).copied().unwrap_or(0) ^ b.as_bytes().get(i).copied().unwrap_or(0),
        );
    }
    diff == 0
}
fn tokens(kernel: &Kernel) -> io::Result<BTreeMap<String, String>> {
    let persistent = kernel.config.get()["mcp"]["persistentUrls"] == true;
    let directory = kernel.config.directory();
    let secrets = if persistent || directory.join("secrets.bin").exists() {
        let cipher: Arc<dyn SecretCipher> = if cfg!(windows) {
            WindowsCipher::open(directory, true)
                .map(|c| Arc::new(c) as Arc<dyn SecretCipher>)
                .unwrap_or_else(|_| Arc::new(UnavailableCipher))
        } else {
            Arc::new(UnavailableCipher)
        };
        Some(SecretStore::open(
            directory,
            if cfg!(windows) {
                "win32"
            } else if cfg!(target_os = "macos") {
                "darwin"
            } else {
                "linux"
            },
            cipher,
        )?)
    } else {
        None
    };
    let mut result = BTreeMap::new();
    for id in ["core", "desktop", "plugins"] {
        let key = format!("mcp:path:{id}");
        let kept = if persistent {
            secrets
                .as_ref()
                .and_then(|s| s.get(&key).ok().flatten())
                .filter(|s| {
                    s.len() == 43
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                })
        } else {
            None
        };
        let value = kept.unwrap_or(token()?);
        if let Some(s) = &secrets {
            let _ = s.set(&key, if persistent { &value } else { "" });
        }
        result.insert(id.into(), format!("/mcp/{id}/{value}"));
    }
    Ok(result)
}
struct HttpState {
    port: u16,
    kernel: Arc<Kernel>,
    paths: BTreeMap<String, String>,
    stopping: AtomicBool,
    active: AtomicUsize,
    budget: Arc<Semaphore>,
    seen: Mutex<BTreeMap<String, i64>>,
}
struct Active(Arc<HttpState>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}
pub struct Service {
    pub port: u16,
    pub urls: BTreeMap<String, String>,
    state: Arc<HttpState>,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<io::Result<()>>,
    pub control: Option<crate::control::Service>,
}
impl Service {
    pub async fn start(kernel: Arc<Kernel>, port: u16) -> io::Result<Self> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        let port = listener.local_addr()?.port();
        let paths = tokens(&kernel)?;
        let urls = paths
            .iter()
            .map(|(k, v)| (k.clone(), format!("http://127.0.0.1:{port}{v}")))
            .collect();
        let state = Arc::new(HttpState {
            port,
            kernel: kernel.clone(),
            paths,
            stopping: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            budget: Arc::new(Semaphore::new(8)),
            seen: Mutex::new(BTreeMap::new()),
        });
        let app = Router::new().fallback(handle).with_state(state.clone());
        // Establish all fallible owners before publishing either listener task.
        let control = if kernel.config.get()["controlApi"]["enabled"] == true {
            Some(crate::control::Service::start(kernel).await?)
        } else {
            None
        };
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
            urls,
            state,
            stop: Some(tx),
            task,
            control,
        })
    }
    pub fn snapshot(&self) -> Value {
        json!({"port":self.port,"accepting":!self.state.stopping.load(Ordering::Acquire),"accepted":self.state.active.load(Ordering::Acquire),"lastRequestAt":self.state.seen.lock().unwrap().clone(),"controlPort":self.control.as_ref().map(|c|c.port)})
    }
    pub async fn stop(self) -> Value {
        self.stop_with_deadline(None).await
    }
    pub async fn stop_for_exit(self, deadline: Duration) -> Value {
        self.stop_with_deadline(Some(deadline)).await
    }
    async fn stop_with_deadline(mut self, deadline: Option<Duration>) -> Value {
        self.state.stopping.store(true, Ordering::Release);
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if let Some(c) = self.control.take() {
            c.stop().await;
        }
        let settled = if let Some(deadline) = deadline {
            tokio::time::timeout(deadline, &mut self.task).await.is_ok()
        } else {
            (&mut self.task).await.is_ok()
        };
        if !settled {
            self.task.abort();
        }
        json!({"drained":settled&&self.state.active.load(Ordering::Acquire)==0,"listenerSettled":settled,"acceptedRemaining":self.state.active.load(Ordering::Acquire)})
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.state.stopping.store(true, Ordering::Release);
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
    }
}
pub fn json_response(status: u16, body: Value) -> Response {
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body.to_string(),
    )
        .into_response()
}
fn request_id(headers: &axum::http::HeaderMap) -> String {
    let values = headers.get_all("x-request-id").iter().collect::<Vec<_>>();
    if values.len() != 1 {
        return String::new();
    }
    let raw = values[0].to_str().unwrap_or("");
    let id = raw.split('/').next().unwrap_or("").trim();
    if id.is_empty()
        || id.len() > 100
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        String::new()
    } else {
        id.into()
    }
}
async fn handle(State(state): State<Arc<HttpState>>, request: Request<Body>) -> Response {
    if state.stopping.load(Ordering::Acquire) {
        return json_response(503, json!({"error":"server_stopping"}));
    }
    let path = request.uri().path();
    let metadata = path.strip_prefix("/.well-known/oauth-protected-resource");
    let route = state
        .paths
        .iter()
        .find(|(_, p)| safe_equal(path, p) || metadata.is_some_and(|m| safe_equal(m, p)));
    let Some((id, _)) = route else {
        return json_response(404, json!({"error":"not_found"}));
    };
    if !request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(valid_host)
    {
        return json_response(403, json!({"error":"host_forbidden"}));
    }
    if request.headers().contains_key(header::ORIGIN)
        && request.headers().get_all(header::ORIGIN).iter().count() != 1
        || request
            .headers()
            .get(header::ORIGIN)
            .is_some_and(|v| v.to_str().is_err())
        || !valid_origin(
            request
                .headers()
                .get(header::ORIGIN)
                .and_then(|v| v.to_str().ok()),
        )
    {
        return json_response(403, json!({"error":"origin_forbidden"}));
    }
    if metadata.is_some() {
        return json_response(
            200,
            json!({"resource":format!("http://127.0.0.1:{}{}",state.port,state.paths[id]),"resource_name":format!("Chat on Steroids {}",match id.as_str(){"core"=>"Core","desktop"=>"Desktop",_=>"Plugins"}),"authorization_servers":[],"scopes_supported":[]}),
        );
    }
    let permit = match state.budget.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return json_response(503, json!({"error":"busy"})),
    };
    let method = request.method().to_string();
    let mut headers = BTreeMap::<String, String>::new();
    for (key, val) in request.headers() {
        if let Ok(val) = val.to_str() {
            headers.insert(key.to_string(), val.into());
        }
    }
    if method == "POST"
        && headers
            .get("content-type")
            .is_none_or(|v| v.split(';').next().unwrap_or("").trim() != "application/json")
    {
        return json_response(415, json!({"error":"unsupported_media_type"}));
    }
    if headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|n| n > BODY_LIMIT)
    {
        return json_response(413, json!({"error":"payload_too_large"}));
    }
    let request_id = request_id(request.headers());
    let surface = match id.as_str() {
        "core" => Surface::Core,
        "desktop" => Surface::Desktop,
        _ => Surface::Plugins,
    };
    let now = chrono::Utc::now().timestamp_millis();
    state.seen.lock().unwrap().insert(id.clone(), now);
    let body = if method == "POST" {
        match tokio::time::timeout(
            Duration::from_secs(5),
            to_bytes(request.into_body(), BODY_LIMIT),
        )
        .await
        {
            Ok(Ok(bytes)) => match serde_json::from_slice(&bytes) {
                Ok(body) => body,
                Err(_) => return json_response(400, json!({"error":"invalid_json"})),
            },
            Ok(Err(_)) => return json_response(413, json!({"error":"payload_too_large"})),
            Err(_) => return json_response(408, json!({"error":"body_timeout"})),
        }
    } else {
        Value::Null
    };
    state.active.fetch_add(1, Ordering::AcqRel);
    let work = state.clone();
    let active = Active(state);
    let reply = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _active = active;
        let scope = cos_exec::process::PublicationScope::begin();
        let reply = protocol::dispatch(&work.kernel, surface, &request_id, &method, &headers, body);
        (reply, scope.finish())
    })
    .await;
    match reply {
        Ok((reply, mut publication)) => {
            if reply.status == 202 {
                publication.commit();
                return StatusCode::ACCEPTED.into_response();
            }
            if reply.sse && reply.status == 200 {
                let payload = if let Some(batch) = reply.body.as_array() {
                    batch
                        .iter()
                        .map(|v| format!("event: message\ndata: {v}\n\n"))
                        .collect::<String>()
                } else {
                    format!("event: message\ndata: {}\n\n", reply.body)
                };
                published(200, "text/event-stream", payload, publication)
            } else {
                published(
                    reply.status,
                    "application/json",
                    reply.body.to_string(),
                    publication,
                )
            }
        }
        Err(_) => json_response(500, json!({"error":"internal_error"})),
    }
}
