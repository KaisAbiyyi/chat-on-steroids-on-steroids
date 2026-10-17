use cos_exec::{
    process::{Launch, Manager},
    shell,
};
use std::time::Duration;
#[test]
fn pipes_exit_nonzero_and_exact_cmd_quoting() {
    let d = tempfile::tempdir().unwrap();
    let s = shell::detect(Some(if cfg!(windows) { "cmd" } else { "sh" })).unwrap();
    let a = s.args(
        if cfg!(windows) {
            "echo public & exit /b 7"
        } else {
            "printf 'public\\n'; exit 7"
        },
        false,
    );
    let m = Manager::default();
    let r = m
        .launch(
            Launch {
                owner: "a",
                program: &s.path,
                args: &a,
                cwd: d.path(),
                tty: false,
                raw_cmd: cfg!(windows),
            },
            Duration::from_secs(5),
        )
        .unwrap();
    assert!(r["output"].as_str().unwrap().contains("public"));
    assert_eq!(r["exit_code"], 7);
    m.shutdown();
}
#[test]
fn foreign_poll_refused_and_unread_completion_retained() {
    let d = tempfile::tempdir().unwrap();
    let s = shell::detect(None).unwrap();
    let a = s.args(
        if cfg!(windows) {
            "Write-Output first; Start-Sleep -Milliseconds 800; Write-Output last"
        } else {
            "echo first; sleep 1; echo last"
        },
        false,
    );
    let m = Manager::default();
    let r = m
        .launch(
            Launch {
                owner: "session:a",
                program: &s.path,
                args: &a,
                cwd: d.path(),
                tty: false,
                raw_cmd: false,
            },
            Duration::from_millis(50),
        )
        .unwrap();
    let id = r["session_id"].as_i64().unwrap() as i32;
    assert!(m.poll(id, "session:b", "", Duration::ZERO).is_err());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut delivered = String::new();
    let r = loop {
        let page = m.poll(id, "session:a", "", Duration::from_secs(1)).unwrap();
        delivered.push_str(page["output"].as_str().unwrap());
        if page["exit_code"].is_number() {
            break page;
        }
        assert!(std::time::Instant::now() < deadline);
    };
    assert!(delivered.contains("last"));
    assert_eq!(r["exit_code"], 0);
    let again = m.poll(id, "session:a", "", Duration::ZERO).unwrap();
    assert!(again["output"].as_str().unwrap().contains("last"));
    assert_eq!(again["output_replayed"], true);
    assert!(
        m.poll(id, "session:a", "must not write", Duration::ZERO)
            .is_err()
    );
    m.shutdown();
}
#[test]
fn native_pty_accepts_input_resize_and_interrupt() {
    let d = tempfile::tempdir().unwrap();
    let s = shell::detect(None).unwrap();
    let a=s.args(if cfg!(windows){"Write-Output ready; $x=[Console]::ReadLine(); Write-Output ('received:'+ $x); Start-Sleep -Seconds 30"}else{"echo ready; read x; echo received:$x; sleep 30"},false);
    let m = Manager::default();
    let r = m
        .launch(
            Launch {
                owner: "session:a",
                program: &s.path,
                args: &a,
                cwd: d.path(),
                tty: true,
                raw_cmd: false,
            },
            Duration::from_millis(500),
        )
        .unwrap();
    let id = r["session_id"].as_i64().unwrap() as i32;
    m.resize(id, "session:a", 35, 110).unwrap();
    assert!(m.resize(id, "session:b", 35, 110).is_err());
    let r = m
        .poll(id, "session:a", "public\r\n", Duration::from_secs(2))
        .unwrap();
    let mut output = r["output"].as_str().unwrap().to_string();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !output.contains("received:public") && std::time::Instant::now() < deadline {
        let page = m
            .poll(id, "session:a", "", Duration::from_millis(250))
            .unwrap();
        output.push_str(page["output"].as_str().unwrap());
    }
    assert!(output.contains("received:public"), "{output}");
    m.poll(id, "session:a", "\u{3}", Duration::from_secs(3))
        .unwrap();
    m.shutdown();
}
