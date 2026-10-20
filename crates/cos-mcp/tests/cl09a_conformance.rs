use cos_core::persistence::ConfigStore;
use cos_mcp::{
    http,
    kernel::{Kernel, Surface},
    protocol::{self, Server},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const MODERN: &str = "2026-07-28";

fn modern(method: &str, id: u64, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

fn meta() -> Value {
    json!({"io.modelcontextprotocol/protocolVersion":MODERN,
           "io.modelcontextprotocol/clientInfo":{"name":"public-cl09a-test","version":"1"},
           "io.modelcontextprotocol/clientCapabilities":{}})
}

fn headers(method: &str, name: Option<&str>) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::from([
        ("mcp-protocol-version".into(), MODERN.into()),
        ("mcp-method".into(), method.into()),
    ]);
    if let Some(name) = name {
        headers.insert("mcp-name".into(), name.into());
    }
    headers
}

struct CallCounter(AtomicUsize);
impl Server for CallCounter {
    fn info(&self, _: Surface) -> Value {
        json!({"name":"cl09a-public-fixture","version":"1"})
    }
    fn tools(&self, surface: Surface) -> Vec<Value> {
        if surface == Surface::Core {
            vec![
                json!({"name":"public_echo","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}}),
            ]
        } else {
            vec![]
        }
    }
    fn call(&self, _: Surface, _: &str, _: &str, _: Value) -> Value {
        self.0.fetch_add(1, Ordering::SeqCst);
        json!({"content":[{"type":"text","text":"called"}]})
    }
}

#[test]
fn malformed_params_and_cross_surface_calls_do_not_reach_a_handler() {
    let server = CallCounter(AtomicUsize::new(0));
    for (id, params) in [(1, Value::Null), (2, json!(42)), (3, json!([]))] {
        let reply = protocol::dispatch_server(
            &server,
            Surface::Core,
            "public-request",
            "POST",
            &headers("tools/list", None),
            modern("tools/list", id, params),
        );
        assert_eq!(reply.status, 200, "{id}: {}", reply.body);
        assert_eq!(reply.body["error"]["code"], -32602, "{id}: {}", reply.body);
    }

    for surface in [Surface::Desktop, Surface::Plugins] {
        let reply = protocol::dispatch_server(
            &server,
            surface,
            "public-request",
            "POST",
            &headers("tools/list", None),
            modern("tools/list", 10, json!({"_meta":meta()})),
        );
        assert_eq!(reply.body["result"]["tools"], json!([]));
        let reply = protocol::dispatch_server(
            &server,
            surface,
            "public-request",
            "POST",
            &headers("tools/call", Some("public_echo")),
            modern(
                "tools/call",
                11,
                json!({"_meta":meta(),"name":"public_echo","arguments":{"text":"public"}}),
            ),
        );
        assert_eq!(reply.body["error"]["code"], -32602);
    }
    assert_eq!(server.0.load(Ordering::SeqCst), 0);
}

#[test]
fn invalid_schema_and_live_revocation_never_publish_file_content() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("secret.txt"), "private-denied-fixture").unwrap();
    let root = std::fs::canonicalize(project.path()).unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    config
        .update(|c| {
            c["roots"] = json!([{"name":"p","path":root}]);
            c["multiAgent"]["allowUnattributedCalls"] = json!(true);
        })
        .unwrap();
    let kernel = Kernel::new(config.clone()).unwrap();
    let listed = protocol::dispatch(
        &kernel,
        Surface::Core,
        "public-request",
        "POST",
        &headers("tools/list", None),
        modern("tools/list", 12, json!({"_meta":meta()})),
    );
    assert!(
        listed.body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "read")
    );

    let bad = protocol::dispatch(
        &kernel,
        Surface::Core,
        "public-request",
        "POST",
        &headers("tools/call", Some("read")),
        modern(
            "tools/call",
            13,
            json!({"_meta":meta(),"name":"read","arguments":{"paths":["/p/secret.txt",23]}}),
        ),
    );
    assert_eq!(bad.body["result"]["isError"], true);
    assert!(bad.body.to_string().contains("Input validation error"));
    assert!(!bad.body.to_string().contains("private-denied-fixture"));

    config
        .update(|c| {
            c["capabilities"]["read"] = json!(false);
            c["capabilities"]["browse"] = json!(false);
            c["capabilities"]["metadata"] = json!(false);
        })
        .unwrap();
    // A cached tools/list declaration must not convey irrevocable permission.
    let denied = protocol::dispatch(
        &kernel,
        Surface::Core,
        "public-request",
        "POST",
        &headers("tools/call", Some("read")),
        modern(
            "tools/call",
            14,
            json!({"_meta":meta(),"name":"read","arguments":{"paths":["/p/secret.txt"]}}),
        ),
    );
    assert_eq!(denied.body["result"]["isError"], true);
    assert!(denied.body.to_string().contains("TOOL_DISABLED"));
    assert!(!denied.body.to_string().contains("private-denied-fixture"));
}

async fn wire(port: u16, request: String) -> String {
    tokio::task::spawn_blocking(move || {
        let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        socket.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        socket.read_to_string(&mut out).unwrap();
        out
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_json_uses_the_json_rpc_parse_error_envelope() {
    let profile = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    let service = http::Service::start(Kernel::new(config).unwrap(), 0)
        .await
        .unwrap();
    let path = service.urls["core"]
        .split_once(&format!(":{}", service.port))
        .unwrap()
        .1;
    for body in ["{", "not-json"] {
        let result = wire(service.port, format!(
            "POST {path} HTTP/1.1\r\nHost: localhost:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            service.port, body.len()
        )).await;
        assert!(result.starts_with("HTTP/1.1 400"), "{result}");
        let parsed: Value = serde_json::from_str(result.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], -32700);
        assert_eq!(parsed["id"], Value::Null);
    }
    service.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicate_mcp_authority_headers_are_rejected_before_dispatch() {
    let profile = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    let service = http::Service::start(Kernel::new(config).unwrap(), 0)
        .await
        .unwrap();
    let path = service.urls["core"]
        .split_once(&format!(":{}", service.port))
        .unwrap()
        .1;
    let body = modern("tools/list", 7, json!({"_meta":meta()})).to_string();
    let result = wire(service.port, format!(
        "POST {path} HTTP/1.1\r\nHost: localhost:{}\r\nContent-Type: application/json\r\nMCP-Protocol-Version: {MODERN}\r\nMcp-Method: tools/list\r\nMcp-Method: tools/call\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        service.port, body.len()
    )).await;
    assert!(result.starts_with("HTTP/1.1 400"), "{result}");
    let parsed: Value = serde_json::from_str(result.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["error"]["code"], -32600);
    service.stop().await;
}
