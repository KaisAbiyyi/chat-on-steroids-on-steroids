//! Generation-fenced sidecar supervision.
use cos_core::persistence::{invalid, lock};
use cos_exec::process::{Launch, Manager};
use serde_json::{Value, json};
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
struct State {
    generation: u64,
    phase: String,
    process: Option<(i32, u64)>,
    url: Option<String>,
    exit: Option<i64>,
    health: Value,
    work: Option<Arc<tempfile::TempDir>>,
}
struct Inner {
    state: Mutex<State>,
    processes: Manager,
    lifecycle: Mutex<()>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.processes.shutdown();
    }
}
#[derive(Clone)]
pub struct Supervisor(Arc<Inner>);
impl Default for Supervisor {
    fn default() -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(State {
                generation: 0,
                phase: "stopped".into(),
                process: None,
                url: None,
                exit: None,
                health: Value::Null,
                work: None,
            }),
            processes: Manager::default(),
            lifecycle: Mutex::new(()),
        }))
    }
}
fn observe(state: &mut State, result: &Value) {
    if let Some(exit) = result["exit_code"].as_i64() {
        state.phase = "failed".into();
        state.exit = Some(exit);
        state.process = None;
        return;
    }
    if let Some(text) = result["output"].as_str() {
        let re =
            regex::Regex::new(r"https://[a-zA-Z0-9][a-zA-Z0-9.-]*\.trycloudflare\.com\b").unwrap();
        if let Some(url) = re.find(text) {
            state.url = Some(url.as_str().into());
            state.phase = "running".into();
        }
    }
}
impl Supervisor {
    pub fn start(&self, program: &str, args: &[String], cwd: &Path) -> io::Result<u64> {
        self.start_environment(program, args, cwd, None, None)
    }
    pub fn start_openai(
        &self,
        program: &str,
        tunnel_id: &str,
        api_key: &str,
        local_url: &str,
        discovery_headers: &Value,
    ) -> io::Result<u64> {
        if api_key.trim().is_empty() {
            return Err(invalid("Secure Tunnel API key is not configured"));
        }
        if !regex::Regex::new(
            r"^http://127\.0\.0\.1:[0-9]+/mcp/(core|desktop|plugins)/[A-Za-z0-9_-]+$",
        )
        .unwrap()
        .is_match(local_url)
        {
            return Err(invalid(
                "Secure Tunnel requires this service's loopback MCP URL",
            ));
        }
        let work = Arc::new(tempfile::tempdir()?);
        let args = Self::openai_args(tunnel_id, 0, &work.path().join("health.url"))?;
        let mut env = std::collections::BTreeMap::from([
            ("CONTROL_PLANE_API_KEY".into(), api_key.into()),
            (
                "MCP_SERVER_URL".into(),
                format!("url={local_url},channel=main"),
            ),
        ]);
        if discovery_headers.as_object().is_some_and(|h| !h.is_empty()) {
            env.insert(
                "MCP_DISCOVERY_EXTRA_HEADERS".into(),
                discovery_headers.to_string(),
            );
        }
        // Go ignores ALL_PROXY. An explicitly configured HTTPS_PROXY always wins.
        let vars = std::env::vars()
            .map(|(k, v)| (k.to_uppercase(), v))
            .collect::<std::collections::BTreeMap<_, _>>();
        if vars.get("HTTPS_PROXY").is_none_or(|s| s.trim().is_empty())
            && let Some(proxy) = vars.get("ALL_PROXY").filter(|s| {
                regex::Regex::new(r"(?i)^(https?|socks5h?)://\S+$")
                    .unwrap()
                    .is_match(s)
            })
        {
            env.insert("HTTPS_PROXY".into(), proxy.clone());
            if vars.get("NO_PROXY").is_none_or(|s| s.trim().is_empty()) {
                env.insert("NO_PROXY".into(), "localhost,127.0.0.1,::1".into());
            }
        }
        self.start_environment(program, &args, work.path(), Some(&env), Some(work.clone()))
    }
    fn start_environment(
        &self,
        program: &str,
        args: &[String],
        cwd: &Path,
        environment: Option<&std::collections::BTreeMap<String, String>>,
        work: Option<Arc<tempfile::TempDir>>,
    ) -> io::Result<u64> {
        let _serial = lock(&self.0.lifecycle)?;
        self.stop_inner()?;
        let generation = {
            let mut state = lock(&self.0.state)?;
            state.generation += 1;
            state.phase = "starting".into();
            state.work = work;
            state.health = Value::Null;
            state.generation
        };
        let owner = format!("tunnel:{generation}");
        let result = self.0.processes.launch_environment(
            Launch {
                owner: &owner,
                program,
                args,
                cwd,
                tty: false,
                raw_cmd: false,
            },
            Duration::from_millis(250),
            environment.unwrap_or(&std::collections::BTreeMap::new()),
        );
        let result = match result {
            Ok(r) => r,
            Err(e) => {
                lock(&self.0.state)?.phase = "failed".into();
                return Err(e);
            }
        };
        let process = result["session_id"].as_i64().map(|id| id as i32);
        {
            let mut state = lock(&self.0.state)?;
            state.process = process.map(|id| (id, generation));
            observe(&mut state, &result);
        }
        if let Some(process) = process {
            let weak = Arc::downgrade(&self.0);
            std::thread::spawn(move || {
                loop {
                    let Some(inner) = weak.upgrade() else {
                        break;
                    };
                    if !inner.state.lock().is_ok_and(|s| s.generation == generation) {
                        break;
                    }
                    let result =
                        inner
                            .processes
                            .poll(process, &owner, "", Duration::from_millis(250));
                    let work = inner.state.lock().ok().and_then(|s| s.work.clone());
                    let health = work
                        .as_ref()
                        .and_then(|d| read_health(&d.path().join("health.url")));
                    let mut state = match inner.state.lock() {
                        Ok(s) => s,
                        Err(_) => break,
                    };
                    if state.generation != generation {
                        break;
                    }
                    match result {
                        Ok(r) => {
                            observe(&mut state, &r);
                            if let Some(health) = health {
                                let connected = health["controlPlaneConnected"] == true;
                                state.health = health;
                                if state.phase != "failed" {
                                    state.phase = if connected {
                                        "running"
                                    } else {
                                        "awaiting_control_plane"
                                    }
                                    .into();
                                }
                            }
                            if state.phase == "failed" {
                                break;
                            }
                        }
                        Err(_) => {
                            state.phase = "failed".into();
                            break;
                        }
                    }
                }
            });
        }
        Ok(generation)
    }
    pub fn status(&self) -> Value {
        let s = self.0.state.lock().unwrap();
        json!({"generation":s.generation,"state":s.phase,"url":s.url,"exitCode":s.exit,"health":s.health})
    }
    fn stop_inner(&self) -> io::Result<()> {
        let (process, work) = {
            let mut state = lock(&self.0.state)?;
            state.generation += 1;
            state.phase = "stopping".into();
            (state.process.take(), state.work.take())
        };
        if let Some((id, owner_generation)) = process {
            let owner = format!("tunnel:{owner_generation}");
            if let Err(error) =
                self.0
                    .processes
                    .retire_discard_pending(id, &owner, Duration::from_secs(2))
            {
                let mut state = lock(&self.0.state)?;
                // Keep custody on every failure. A generation never goes backwards,
                // so a delayed old observer cannot become current again.
                state.process = process;
                state.work = work;
                state.phase = "failed".into();
                return Err(error);
            }
        }
        // Retain the sidecar's health directory until its child and readers have exited.
        drop(work);
        let mut state = lock(&self.0.state)?;
        state.phase = "stopped".into();
        state.url = None;
        state.exit = None;
        state.health = Value::Null;
        Ok(())
    }
    pub fn stop(&self) -> io::Result<()> {
        let _serial = lock(&self.0.lifecycle)?;
        self.stop_inner()
    }
    pub fn cloudflared_args(local_port: u16) -> Vec<String> {
        [
            "tunnel".into(),
            "--no-autoupdate".into(),
            "--url".into(),
            format!("http://127.0.0.1:{local_port}"),
            "--http-host-header".into(),
            format!("127.0.0.1:{local_port}"),
        ]
        .into()
    }
    pub fn openai_args(
        tunnel_id: &str,
        _local_port: u16,
        status: &Path,
    ) -> io::Result<Vec<String>> {
        if !regex::Regex::new(r"^tunnel_[0-9a-f]{32}$")
            .unwrap()
            .is_match(tunnel_id)
        {
            return Err(invalid("Invalid Secure Tunnel ID"));
        }
        Ok(vec![
            "run".into(),
            "--control-plane.tunnel-id".into(),
            tunnel_id.into(),
            "--health.listen-addr".into(),
            "127.0.0.1:0".into(),
            "--health.url-file".into(),
            status.to_string_lossy().into_owned(),
            "--log.format".into(),
            "json".into(),
            "--log.level".into(),
            "info".into(),
        ])
    }
}
pub fn read_metric(text: &str, name: &str) -> Option<f64> {
    let mut total = None;
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.trim().strip_prefix(name) else {
            continue;
        };
        if !rest.is_empty() && !rest.starts_with([' ', '{']) {
            continue;
        }
        let rest = if rest.starts_with('{') {
            rest.split_once('}').map(|(_, r)| r).unwrap_or(rest)
        } else {
            rest
        };
        if let Some(value) = rest
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|f| f.is_finite())
        {
            total = Some(total.unwrap_or(0.) + value);
        }
    }
    total
}
fn number(value: Option<f64>) -> Value {
    match value {
        Some(v) if v.fract() == 0. && v >= i64::MIN as f64 && v < i64::MAX as f64 => {
            json!(v as i64)
        }
        Some(v) => json!(v),
        None => Value::Null,
    }
}
pub fn parse_poll_health(metrics: &str) -> Value {
    json!({"lastSuccessMs":read_metric(metrics,"commands_poll_last_successful_timestamp_seconds").filter(|s| *s>0.).map(|s| (s*1000.).round() as i64),"polls":number(read_metric(metrics,"commands_poll_cycles_total")),"errors":number(read_metric(metrics,"commands_poll_errors_total"))})
}
pub fn parse_client_status(raw: &Value) -> Value {
    let string = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let main = raw["channels"]
        .as_array()
        .and_then(|a| a.iter().find(|c| c["name"] == "main"));
    let route = &raw["control_plane_route"];
    let route = string(&route["target"]).map(|target| {
        format!(
            "{target} · {}",
            string(&route["proxy_source"])
                .filter(|p| p != "none")
                .map(|p| format!("via {p}"))
                .unwrap_or_else(|| string(&route["route_mode"]).unwrap_or("direct".into()))
        )
    });
    json!({"version":string(&raw["version"]),"probe":main.and_then(|m| string(&m["probe_status"])),"metadataError":string(&raw["tunnel_metadata_error"]),"uptimeSeconds":if raw["uptime_seconds"].is_number(){raw["uptime_seconds"].clone()}else{Value::Null},"route":route})
}
fn loopback_get(base: &str, route: &str) -> Option<String> {
    use std::io::{Read, Write};
    let address = base
        .trim()
        .strip_prefix("http://")?
        .trim_end_matches('/')
        .parse::<std::net::SocketAddr>()
        .ok()?;
    if !address.ip().is_loopback() {
        return None;
    }
    let mut stream =
        std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(750)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(750)))
        .ok()?;
    write!(
        stream,
        "GET {route} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nAccept: */*\r\n\r\n"
    )
    .ok()?;
    let mut bytes = vec![];
    stream.take(256 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 256 * 1024 {
        return None;
    }
    let raw = String::from_utf8(bytes).ok()?;
    let (headers, body) = raw.split_once("\r\n\r\n")?;
    if headers
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)
        .is_none_or(|s| s != "200")
    {
        return None;
    }
    if headers
        .lines()
        .any(|h| h.eq_ignore_ascii_case("transfer-encoding: chunked"))
    {
        let mut input = body;
        let mut decoded = String::new();
        loop {
            let (size, rest) = input.split_once("\r\n")?;
            let size = usize::from_str_radix(size.split(';').next()?, 16).ok()?;
            if size == 0 {
                break;
            }
            let chunk = rest.get(..size)?;
            decoded.push_str(chunk);
            input = rest.get(size..)?.strip_prefix("\r\n")?;
        }
        Some(decoded)
    } else {
        Some(body.into())
    }
}
fn read_health(file: &Path) -> Option<Value> {
    use std::io::Read;
    let mut base = String::new();
    std::fs::File::open(file)
        .ok()?
        .take(513)
        .read_to_string(&mut base)
        .ok()?;
    if base.len() > 512 {
        return None;
    }
    let base = base.trim();
    let local_healthy = loopback_get(base, "/readyz").is_some();
    let poll = loopback_get(base, "/metrics").map(|s| parse_poll_health(&s));
    let status = loopback_get(base, "/api/status")
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .map(|s| parse_client_status(&s));
    let now = chrono::Utc::now().timestamp_millis();
    let handshake = poll.as_ref().and_then(|p| p["lastSuccessMs"].as_i64());
    Some(
        json!({"localHealthy":local_healthy,"controlPlaneConnected":handshake.is_some_and(|at|now.saturating_sub(at)<=95000),"handshakeAt":handshake,"poll":poll,"client":status,"chatgptReached":false}),
    )
}
