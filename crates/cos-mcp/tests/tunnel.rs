use cos_exec::shell;
use cos_mcp::tunnel::Supervisor;
use std::time::{Duration, Instant};
#[test]
fn pending_output_publication_cannot_prevent_an_explicit_sidecar_stop() {
    let d = tempfile::tempdir().unwrap();
    let scope = cos_exec::process::PublicationScope::begin();
    let (program, args) = if cfg!(windows) {
        (
            "cmd.exe",
            vec!["/d".into(), "/c".into(), "ping -n 30 127.0.0.1 >nul".into()],
        )
    } else {
        ("sh", vec!["-c".into(), "sleep 30".into()])
    };
    let tunnel = Supervisor::default();
    let generation = tunnel.start(program, &args, d.path()).unwrap();
    tunnel.stop().unwrap();
    assert_eq!(tunnel.status()["state"], "stopped");
    assert!(tunnel.status()["generation"].as_u64().unwrap() > generation);
    drop(scope);
}
#[test]
fn adapters_match_actual_sidecar_flags_and_reject_invalid_ids() {
    assert_eq!(
        Supervisor::cloudflared_args(42),
        [
            "tunnel",
            "--no-autoupdate",
            "--url",
            "http://127.0.0.1:42",
            "--http-host-header",
            "127.0.0.1:42"
        ]
    );
    assert!(
        Supervisor::openai_args("public-bad-id", 42, std::path::Path::new("health.url")).is_err()
    );
    assert_eq!(
        Supervisor::openai_args(
            "tunnel_0123456789abcdef0123456789abcdef",
            42,
            std::path::Path::new("health.url")
        )
        .unwrap(),
        [
            "run",
            "--control-plane.tunnel-id",
            "tunnel_0123456789abcdef0123456789abcdef",
            "--health.listen-addr",
            "127.0.0.1:0",
            "--health.url-file",
            "health.url",
            "--log.format",
            "json",
            "--log.level",
            "info"
        ]
    );
}
#[test]
fn upstream_health_oracles() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase4-6/tunnel-health.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let result = if let Some(metrics) = case["metrics"].as_str() {
            cos_mcp::tunnel::parse_poll_health(metrics)
        } else {
            cos_mcp::tunnel::parse_client_status(&case["status"])
        };
        assert_eq!(
            serde_json::to_string(&result).unwrap(),
            serde_json::to_string(&case["expected"]).unwrap()
        );
    }
}
#[test]
fn real_process_generation_and_stop() {
    let d = tempfile::tempdir().unwrap();
    let s = shell::detect(None).unwrap();
    let a = s.args(
        if cfg!(windows) {
            "Write-Output 'https://public-fixture.trycloudflare.com'; Start-Sleep -Seconds 30"
        } else {
            "echo https://public-fixture.trycloudflare.com; sleep 30"
        },
        false,
    );
    let tunnel = Supervisor::default();
    let generation = tunnel.start(&s.path, &a, d.path()).unwrap();
    assert!(generation > 0);
    let start = Instant::now();
    while tunnel.status()["state"] != "running" && start.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(tunnel.status()["state"], "running");
    assert_eq!(tunnel.status()["generation"], generation);
    tunnel.stop().unwrap();
    assert_eq!(tunnel.status()["state"], "stopped");
    assert!(tunnel.status()["generation"].as_u64().unwrap() > generation);
}

#[test]
fn stopped_sidecars_do_not_exhaust_process_capacity() {
    let d = tempfile::tempdir().unwrap();
    let (program, args) = if cfg!(windows) {
        (
            "cmd.exe",
            vec!["/d".into(), "/c".into(), "ping -n 30 127.0.0.1 >nul".into()],
        )
    } else {
        ("sh", vec!["-c".into(), "sleep 30".into()])
    };
    let tunnel = Supervisor::default();
    for cycle in 0..70 {
        tunnel
            .start(program, &args, d.path())
            .unwrap_or_else(|e| panic!("restart {cycle}: {e}"));
        tunnel
            .stop()
            .unwrap_or_else(|e| panic!("stop {cycle}: {e}"));
    }
    assert_eq!(tunnel.status()["state"], "stopped");
}

#[test]
fn proxy_fallback_and_loopback_bypass_are_fail_closed() {
    use cos_mcp::tunnel::proxy_environment;
    use std::collections::BTreeMap;
    let vars = BTreeMap::from([
        ("ALL_PROXY".into(), "socks5://example.invalid:1080".into()),
        ("NO_PROXY".into(), "internal.example,localhost".into()),
    ]);
    let env = proxy_environment(&vars);
    assert_eq!(env["HTTPS_PROXY"], "socks5://example.invalid:1080");
    assert!(env["NO_PROXY"].contains("internal.example"));
    for local in ["localhost", "127.0.0.1", "::1", "[::1]"] {
        assert!(env["NO_PROXY"].split(',').any(|host| host == local));
    }
    let explicit = BTreeMap::from([
        ("HTTPS_PROXY".into(), "https://explicit.example:443".into()),
        ("ALL_PROXY".into(), "socks5://ignored.example:1080".into()),
    ]);
    let env = proxy_environment(&explicit);
    assert!(!env.contains_key("HTTPS_PROXY"));
    assert!(env["NO_PROXY"].contains("127.0.0.1"));
    let rejected = BTreeMap::from([("ALL_PROXY".into(), "socks5://host\r\nBAD:1".into())]);
    assert!(!proxy_environment(&rejected).contains_key("HTTPS_PROXY"));
}

#[test]
fn reconnect_budget_is_capped_and_stale_stop_clears_health() {
    use cos_mcp::tunnel::retry_delay;
    assert_eq!(retry_delay(0), Duration::from_millis(500));
    assert_eq!(retry_delay(1), Duration::from_millis(1000));
    assert_eq!(retry_delay(10), Duration::from_secs(30));
    assert_eq!(retry_delay(u32::MAX), Duration::from_secs(30));
    let tunnel = Supervisor::default();
    tunnel.stop().unwrap();
    assert_eq!(tunnel.status()["retryCount"], 0);
    assert!(tunnel.status()["health"].is_null());
}
