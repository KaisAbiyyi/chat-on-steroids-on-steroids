use cos_core::persistence::ConfigStore;
use cos_mcp::{http, kernel::Kernel};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::Arc,
};
async fn request(port: u16, request: String) -> String {
    tokio::task::spawn_blocking(move || {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        s.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn control_directory_link_cannot_redirect_credentials_outside_profile() {
    let profile = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    config
        .update(|v| v["controlApi"]["enabled"] = json!(true))
        .unwrap();
    let link = profile.path().join("control-api");
    #[cfg(windows)]
    assert!(
        std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/j"])
            .arg(&link)
            .arg(outside.path())
            .output()
            .unwrap()
            .status
            .success()
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    let started = cos_mcp::control::Service::start(Kernel::new(config).unwrap()).await;
    let rejected = started.is_err();
    if let Ok(service) = started {
        service.stop().await;
    }
    std::fs::remove_dir(&link)
        .or_else(|_| std::fs::remove_file(&link))
        .unwrap();
    assert!(rejected, "linked control directory was accepted");
    assert_eq!(
        std::fs::read_dir(outside.path()).unwrap().count(),
        0,
        "credentials or locks escaped the profile"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loopback_http_admission_control_and_secret_separation() {
    let d = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(d.path(), if cfg!(windows) { "win32" } else { "linux" }).unwrap(),
    );
    config
        .update(|v| v["controlApi"]["enabled"] = json!(true))
        .unwrap();
    let kernel = Kernel::new(config).unwrap();
    let service = http::Service::start(kernel, 0).await.unwrap();
    let path = service.urls["core"]
        .split_once(&format!(":{}", service.port))
        .unwrap()
        .1;
    let bad = request(
        service.port,
        format!("GET {path} HTTP/1.1\r\nHost: evil\r\nConnection: close\r\n\r\n"),
    )
    .await;
    assert!(bad.starts_with("HTTP/1.1 403"));
    let bad = request(
        service.port,
        format!(
            "GET /mcp/core/wrong HTTP/1.1\r\nHost: localhost:{}\r\nConnection: close\r\n\r\n",
            service.port
        ),
    )
    .await;
    assert!(bad.starts_with("HTTP/1.1 404"));
    let bad=request(service.port,format!("POST {path} HTTP/1.1\r\nHost: localhost:{}\r\nContent-Type: application/json\r\nContent-Length: 9000000\r\nConnection: close\r\n\r\n",service.port)).await;
    assert!(bad.starts_with("HTTP/1.1 413"));
    let control = service.control.as_ref().unwrap();
    assert!(control.port > 0);
    let token = std::fs::read_to_string(d.path().join("control-api/token")).unwrap();
    let health=request(control.port,format!("GET /v1/health HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",control.port,token.trim())).await;
    assert!(health.contains("\"protocol\":1"), "{health}");
    assert!(!health.contains(path));
    let denied=request(control.port,format!("POST /v1/inputs HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {}\r\nContent-Length: 9999999\r\nConnection: close\r\n\r\n",control.port,token.trim())).await;
    assert!(denied.contains("actions_disabled"), "{denied}");
    service.stop().await;
    assert!(!d.path().join("control-api/endpoint.json").exists());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_session_cursor_does_not_repeat_the_first_page() {
    let d = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(d.path(), if cfg!(windows) { "win32" } else { "linux" }).unwrap(),
    );
    config
        .update(|v| v["controlApi"]["enabled"] = json!(true))
        .unwrap();
    let kernel = Kernel::new(config).unwrap();
    for i in 1..=3 {
        kernel
            .sessions
            .create(cos_store::CreateOptions {
                id: Some(format!("control-page-{i}")),
                now: Some(100 + i),
                ..Default::default()
            })
            .unwrap();
    }
    let service = http::Service::start(kernel, 0).await.unwrap();
    let port = service.control.as_ref().unwrap().port;
    let token = std::fs::read_to_string(d.path().join("control-api/token")).unwrap();
    let get = |query: &str| {
        format!(
            "GET /v1/sessions?{query} HTTP/1.1\r\nHost: localhost:{port}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
            token.trim()
        )
    };
    let first = request(port, get("limit=1")).await;
    let first: serde_json::Value =
        serde_json::from_str(first.split_once("\r\n\r\n").unwrap().1).unwrap();
    let next = first["nextCursor"]
        .as_str()
        .expect("Control API cursor must be usable for the next page");
    let second = request(port, get(&format!("limit=1&cursor={next}"))).await;
    let second: serde_json::Value =
        serde_json::from_str(second.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_ne!(first["sessions"][0]["id"], second["sessions"][0]["id"]);
    assert!(
        request(port, get("cursor=invalid"))
            .await
            .starts_with("HTTP/1.1 400")
    );
    service.stop().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnect_keeps_accepted_effects_and_restores_unread_completion() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    config
        .update(|v| {
            v["roots"] = json!([{"name":"p","path":project.path()}]);
            v["multiAgent"]["allowUnattributedCalls"] = json!(true);
        })
        .unwrap();
    let kernel = Kernel::new(config).unwrap();
    let service = http::Service::start(kernel.clone(), 0).await.unwrap();
    let path = service.urls["core"]
        .split_once(&format!(":{}", service.port))
        .unwrap()
        .1;
    let command = if cfg!(windows) {
        "Set-Content -LiteralPath accepted -Value public; Start-Sleep -Milliseconds 600; Set-Content -LiteralPath finished -Value public; Write-Output public-disconnect"
    } else {
        "touch accepted; sleep 1; touch finished; echo public-disconnect"
    };
    let body=json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"exec_command","arguments":{"cmd":command,"workdir":"/p","yield_time_ms":2000,"login":false},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}}}).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", service.port)).unwrap();
    write!(stream,"POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nAccept: application/json\r\nMCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/call\r\nMcp-Name: exec_command\r\nx-request-id: wfr_drop/suffix\r\nContent-Length: {}\r\n\r\n{body}",service.port,body.len()).unwrap();
    let start = std::time::Instant::now();
    while !project.path().join("accepted").exists()
        && start.elapsed() < std::time::Duration::from_secs(5)
    {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(project.path().join("accepted").exists());
    stream.shutdown(std::net::Shutdown::Both).unwrap();
    drop(stream);
    let mut snapshot = json!([]);
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        snapshot = kernel.processes.snapshot("request:wfr_drop").unwrap();
        if snapshot[0]["exit_code"].is_number() && snapshot[0]["publicationPending"] == false {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(project.path().join("finished").exists());
    assert_eq!(snapshot[0]["unread"], true, "{snapshot}");
    let id = snapshot[0]["session_id"].as_i64().unwrap();
    let recovery = kernel.invoke(
        cos_mcp::kernel::Surface::Core,
        "wfr_drop",
        "read",
        json!({"paths":["/p/accepted"]}),
    );
    assert!(
        recovery["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["text"]
                .as_str()
                .is_some_and(|s| s.contains(&id.to_string()))),
        "{recovery}"
    );
    let result = kernel.invoke(
        cos_mcp::kernel::Surface::Core,
        "wfr_drop",
        "write_stdin",
        json!({"session_id":id,"chars":"","yield_time_ms":250}),
    );
    assert!(
        result["structuredContent"]["output"]
            .as_str()
            .unwrap()
            .contains("public-disconnect"),
        "{result}"
    );
    assert_ne!(result["structuredContent"]["output_replayed"], true);
    assert_eq!(service.stop().await["drained"], true);
    kernel.shutdown().unwrap();
}
