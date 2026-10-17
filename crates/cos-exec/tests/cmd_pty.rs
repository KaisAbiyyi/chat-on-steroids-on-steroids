#[cfg(windows)]
#[test]
fn native_cmd_pty_uses_verbatim_shell_command() {
    use cos_exec::{
        process::{Launch, Manager},
        shell,
    };
    use std::time::Duration;
    let d = tempfile::tempdir().unwrap();
    let s = shell::detect(Some("cmd")).unwrap();
    let args = s.args("echo first & echo \"two words\" & exit /b 7", false);
    let manager = Manager::default();
    let mut r = manager
        .launch(
            Launch {
                owner: "r",
                program: &s.path,
                args: &args,
                cwd: d.path(),
                tty: true,
                raw_cmd: true,
            },
            Duration::from_secs(1),
        )
        .unwrap();
    let mut output = r["output"].as_str().unwrap().to_string();
    let start = std::time::Instant::now();
    while let Some(id) = r["session_id"].as_i64() {
        assert!(start.elapsed() < Duration::from_secs(4));
        r = manager
            .poll(id as i32, "r", "", Duration::from_millis(250))
            .unwrap();
        output.push_str(r["output"].as_str().unwrap());
    }
    assert_eq!(r["exit_code"], 7);
    assert!(output.contains("\"two words\""), "{output}");
    assert!(!output.contains("\\\""), "{output}");
    manager.shutdown();
}
