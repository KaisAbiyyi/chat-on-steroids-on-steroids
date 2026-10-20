use cos_exec::process::{Launch, Manager, PublicationScope};
use std::{io, time::Duration};

fn command(kind: &str) -> (&'static str, Vec<String>) {
    if cfg!(windows) {
        ("cmd.exe", vec!["/d".into(), "/c".into(), kind.into()])
    } else {
        ("sh", vec!["-c".into(), kind.into()])
    }
}

fn launch(manager: &Manager, owner: &str, kind: &str, wait: Duration) -> serde_json::Value {
    let cwd = std::env::current_dir().unwrap();
    let (program, args) = command(kind);
    manager
        .launch(
            Launch {
                owner,
                program,
                args: &args,
                cwd: &cwd,
                tty: false,
                raw_cmd: false,
            },
            wait,
        )
        .unwrap()
}

#[test]
fn foreign_principal_cannot_cancel_or_discard_owned_child() {
    let m = Manager::default();
    let sleep = if cfg!(windows) {
        "ping -n 30 127.0.0.1 >nul"
    } else {
        "sleep 30"
    };
    let started = launch(&m, "session:owner", sleep, Duration::ZERO);
    let id = started["session_id"].as_i64().unwrap() as i32;
    assert_eq!(
        m.terminate(id, "session:attacker").unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        m.retire(id, "session:attacker", Duration::ZERO)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        m.poll(id, "session:attacker", "", Duration::ZERO)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        m.snapshot("session:owner")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    m.terminate(id, "session:owner").unwrap();
    m.retire(id, "session:owner", Duration::from_secs(5))
        .unwrap();
    assert_eq!(m.snapshot("session:owner").unwrap(), serde_json::json!([]));
}

#[test]
fn pending_http_publication_cannot_be_retired_until_rollback() {
    let m = Manager::default();
    let scope = PublicationScope::begin();
    let finished = launch(
        &m,
        "session:read",
        if cfg!(windows) {
            "echo custody-once"
        } else {
            "printf 'custody-once\\n'"
        },
        Duration::from_secs(5),
    );
    let id = finished["completed_session_id"].as_i64().unwrap() as i32;
    let delivery = scope.finish();
    let timed_out = m
        .retire(id, "session:read", Duration::from_millis(30))
        .unwrap_err();
    assert_eq!(timed_out.kind(), io::ErrorKind::TimedOut);
    assert!(timed_out.to_string().contains("publication_pending=true"));
    assert_eq!(
        m.snapshot("session:read").unwrap()[0]["publicationPending"],
        true
    );
    drop(delivery); // the HTTP response did not commit
    let retry = m.poll(id, "session:read", "", Duration::ZERO).unwrap();
    assert!(retry["output"].as_str().unwrap().contains("custody-once"));
    assert_ne!(
        retry["output_replayed"], true,
        "rolled-back result must not be mistaken for replay"
    );
    m.retire(id, "session:read", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn provisional_owner_loses_input_and_output_authority_after_rebind() {
    let m = Manager::default();
    let scope = PublicationScope::begin();
    let finished = launch(
        &m,
        "request:before",
        if cfg!(windows) {
            "echo rebound-once"
        } else {
            "printf 'rebound-once\\n'"
        },
        Duration::from_secs(5),
    );
    let id = finished["completed_session_id"].as_i64().unwrap() as i32;
    let delivery = scope.finish();
    m.promote("before", "after").unwrap();
    assert_eq!(
        m.poll(id, "request:before", "", Duration::ZERO)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        m.terminate(id, "request:before").unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(
        m.poll(id, "session:after", "", Duration::ZERO)
            .unwrap_err()
            .to_string()
            .contains("EXEC_OUTPUT_PENDING")
    );
    drop(delivery);
    let settled = m.poll(id, "session:after", "", Duration::ZERO).unwrap();
    assert!(settled["output"].as_str().unwrap().contains("rebound-once"));
    m.retire(id, "session:after", Duration::from_secs(2))
        .unwrap();
}
