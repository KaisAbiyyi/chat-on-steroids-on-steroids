use cos_core::persistence::ConfigStore;
use cos_mcp::{
    http,
    kernel::{Kernel, Surface},
    protocol,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
#[test]
fn host_origin_validation() {
    for host in ["localhost:12", "127.0.0.1:65535", "[::1]:1"] {
        assert!(http::valid_host(host));
    }
    for host in [
        "127.0.0.1.evil:12",
        "evil",
        "localhost@evil",
        "127.0.0.2",
        "localhost:0",
        "localhost:65536",
    ] {
        assert!(!http::valid_host(host), "{host}");
    }
    assert!(http::valid_origin(None));
    assert!(http::valid_origin(Some("http://localhost:123")));
    assert!(!http::valid_origin(Some("https://evil.com")));
    assert!(!http::valid_origin(Some("null")));
}
#[test]
fn protocol_versions_and_cross_surface_unknown() {
    let d = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(d.path(), if cfg!(windows) { "win32" } else { "linux" }).unwrap(),
    );
    let k = Kernel::new(config).unwrap();
    let mut headers = BTreeMap::new();
    headers.insert("mcp-protocol-version".into(), "2026-07-28".into());
    headers.insert("mcp-method".into(), "tools/list".into());
    let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}}});
    let r = protocol::dispatch(&k, Surface::Core, "r", "POST", &headers, body.clone());
    assert_eq!(r.status, 200);
    assert_eq!(r.body["result"]["resultType"], "complete");
    assert!(
        r.body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "read")
    );
    let r = protocol::dispatch(&k, Surface::Desktop, "r", "POST", &headers, body);
    assert_eq!(r.body["result"]["tools"], json!([]));
    let r = protocol::dispatch(&k, Surface::Core, "r", "GET", &BTreeMap::new(), Value::Null);
    assert_eq!(r.status, 405);
    let r = protocol::dispatch(
        &k,
        Surface::Core,
        "r",
        "POST",
        &BTreeMap::new(),
        json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"fixture","version":"1"},"capabilities":{}}}),
    );
    assert_eq!(r.body["result"]["protocolVersion"], "2025-11-25");
    assert!(r.sse);
}
