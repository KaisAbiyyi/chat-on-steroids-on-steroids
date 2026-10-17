use cos_exec::{
    batch,
    process::{Launch, Manager},
    shell,
};
use std::time::Duration;
#[test]
fn real_batch_keeps_variables_and_first_nonzero() {
    let d = tempfile::tempdir().unwrap();
    let s = shell::detect(None).unwrap();
    let commands = if cfg!(windows) {
        vec![
            "$cos_public='kept'".into(),
            "cmd /c exit 7".into(),
            "Write-Output $cos_public".into(),
        ]
    } else {
        vec![
            "cos_public=kept".into(),
            "false".into(),
            "echo $cos_public".into(),
        ]
    };
    let command = batch::compose(&commands, &s.kind, "000000000000000000000000");
    let args = s.args(&command, false);
    let m = Manager::default();
    let mut result = m
        .launch_batch(
            Launch {
                owner: "r",
                program: &s.path,
                args: &args,
                cwd: d.path(),
                tty: false,
                raw_cmd: false,
            },
            Duration::from_secs(3),
            "000000000000000000000000",
        )
        .unwrap();
    let mut output = result["output"].as_str().unwrap().to_string();
    let started = std::time::Instant::now();
    while let Some(id) = result["session_id"].as_i64() {
        assert!(started.elapsed() < Duration::from_secs(5));
        result = m.poll(id as i32, "r", "", Duration::from_secs(1)).unwrap();
        output.push_str(result["output"].as_str().unwrap());
    }
    assert_eq!(result["exit_code"], if cfg!(windows) { 7 } else { 1 });
    assert!(output.contains("kept"));
    assert!(!output.contains("clf-batch"));
    m.shutdown();
}
#[cfg(windows)]
#[test]
fn cmd_multiline_batch_uses_a_managed_script_file() {
    let d = tempfile::tempdir().unwrap();
    let s = shell::detect(Some("cmd")).unwrap();
    let commands = vec![
        "set COS_PUBLIC=kept".into(),
        "cmd /c exit 7".into(),
        "echo %COS_PUBLIC%".into(),
    ];
    let command = batch::compose(&commands, &s.kind, "000000000000000000000000");
    let args = s.args(&command, false);
    let m = Manager::default();
    let mut result = m
        .launch_batch(
            Launch {
                owner: "r",
                program: &s.path,
                args: &args,
                cwd: d.path(),
                tty: false,
                raw_cmd: true,
            },
            Duration::from_secs(3),
            "000000000000000000000000",
        )
        .unwrap();
    let mut output = result["output"].as_str().unwrap().to_string();
    let started = std::time::Instant::now();
    while let Some(id) = result["session_id"].as_i64() {
        assert!(started.elapsed() < Duration::from_secs(5));
        result = m.poll(id as i32, "r", "", Duration::from_secs(1)).unwrap();
        output.push_str(result["output"].as_str().unwrap());
    }
    assert_eq!(result["exit_code"], 7);
    assert!(output.contains("kept"), "{output}");
    assert!(!output.contains("clf-batch"));
    m.shutdown();
}
