use cos_exec::{
    process::{Launch, Manager},
    shell,
};
use std::time::Duration;
#[test]
fn explicit_environment_and_inherited_connector_secrets_are_separate() {
    let d = tempfile::tempdir().unwrap();
    let s = shell::detect(None).unwrap();
    let args = s.args(
        if cfg!(windows) {
            "Write-Output $env:COS_PUBLIC_FIXTURE"
        } else {
            "printf '%s' \"$COS_PUBLIC_FIXTURE\""
        },
        false,
    );
    let m = Manager::default();
    let env = std::collections::BTreeMap::from([(
        "COS_PUBLIC_FIXTURE".to_string(),
        "public".to_string(),
    )]);
    let r = m
        .launch_environment(
            Launch {
                owner: "r",
                program: &s.path,
                args: &args,
                cwd: d.path(),
                tty: false,
                raw_cmd: false,
            },
            Duration::from_secs(3),
            &env,
        )
        .unwrap();
    assert!(r["output"].as_str().unwrap().contains("public"));
    m.shutdown();
}
