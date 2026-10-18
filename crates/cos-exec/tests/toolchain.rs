#![cfg(windows)]
use cos_exec::toolchain::{Environment, NativeProbe, Probe, Toolchains, sort_versions};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
};
struct FixtureProbe {
    files: BTreeSet<String>,
    directories: BTreeMap<String, Vec<String>>,
    calls: RefCell<Vec<Value>>,
}
impl Probe for FixtureProbe {
    fn is_file(&self, path: &str) -> bool {
        self.calls.borrow_mut().push(json!(["isFile", path]));
        self.files.contains(path)
    }
    fn directories(&self, path: &str) -> Vec<String> {
        self.calls.borrow_mut().push(json!(["directories", path]));
        self.directories.get(path).cloned().unwrap_or_default()
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/toolchain.json"
    ))
    .unwrap()
}
#[test]
fn discovery_environment_and_memoization_match_source_steps() {
    for scenario in fixture()["scenarios"].as_array().unwrap() {
        let probe = FixtureProbe {
            files: serde_json::from_value(scenario["files"].clone()).unwrap(),
            directories: serde_json::from_value(scenario["directories"].clone()).unwrap(),
            calls: RefCell::new(vec![]),
        };
        let mut tools = Toolchains::new(probe);
        for step in scenario["steps"].as_array().unwrap() {
            let mut env = step["env"].as_object().unwrap().clone();
            tools.probe().calls.borrow_mut().clear();
            let added = tools.ensure(&mut env);
            assert_eq!(json!({"env":env,"added":added}), step["expected"], "{step}");
            assert_eq!(
                json!(*tools.probe().calls.borrow()),
                step["calls"],
                "{step}"
            );
        }
    }
}
#[test]
fn version_ranking_retains_js_number_and_utf16_order() {
    for case in fixture()["versions"].as_array().unwrap() {
        let mut names: Vec<String> = serde_json::from_value(case["names"].clone()).unwrap();
        sort_versions(&mut names);
        assert_eq!(json!(names), case["expected"], "{case}");
    }
}
#[test]
fn native_probe_selects_a_compiler_and_passes_it_only_to_the_child() {
    let temp = tempfile::tempdir().unwrap();
    let program_files = temp.path().join("Program Files");
    let java = program_files.join("Java").join("jdk-21");
    let jre = program_files.join("Android/Android Studio/jre");
    let go = program_files.join("Go");
    for file in [
        java.join("bin/javac.exe"),
        jre.join("bin/java.exe"),
        go.join("bin/go.exe"),
    ] {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, b"public probe fixture").unwrap();
    }
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    let mut env:Environment=json!({"ProgramFiles":program_files,"ProgramFiles(x86)":temp.path().join("unused"),"LOCALAPPDATA":temp.path().join("local"),"Path":temp.path().join("empty"),"SystemRoot":system_root}).as_object().unwrap().clone();
    let parent_java = std::env::var_os("JAVA_HOME");
    let parent_path = std::env::var_os("PATH");
    let added = Toolchains::new(NativeProbe).ensure(&mut env);
    assert_eq!(
        env["JAVA_HOME"],
        json!(java),
        "JRE must not displace a compiler: {env:?}"
    );
    assert_eq!(env["GOROOT"], json!(go));
    assert_eq!(added.len(), 2);
    let output =
        std::process::Command::new(std::path::Path::new(&system_root).join("System32/cmd.exe"))
            .env_clear()
            .envs(env.iter().map(|(k, v)| (k, v.as_str().unwrap())))
            .args(["/d", "/c", "echo %JAVA_HOME%&echo %GOROOT%&echo %PATH%"])
            .output()
            .unwrap();
    assert!(output.status.success());
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.contains(&java.to_string_lossy().to_string())
            && output.contains(&go.to_string_lossy().to_string()),
        "{output}"
    );
    assert_eq!(std::env::var_os("JAVA_HOME"), parent_java);
    assert_eq!(std::env::var_os("PATH"), parent_path);
}
#[test]
fn unrepresentable_native_values_cannot_erase_a_toolchain_choice_or_path() {
    use std::os::windows::ffi::OsStringExt;
    if let Some(mode) = std::env::var_os("COS_RUST_PUBLIC_TOOLCHAIN_CHILD") {
        let changes = cos_exec::toolchain::child_overrides(None).unwrap();
        assert!(
            changes.is_empty(),
            "{mode:?}: discovery replaced unrepresentable inherited values: {changes:?}"
        );
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let program_files = temp.path().join("Program Files");
    let compiler = program_files.join("Android/Android Studio/jbr/bin/javac.exe");
    fs::create_dir_all(compiler.parent().unwrap()).unwrap();
    fs::write(compiler, b"public probe fixture").unwrap();
    for mode in ["home", "path"] {
        let invalid = std::ffi::OsString::from_wide(&[0xd800]);
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .env_clear()
            .env("SystemRoot", std::env::var_os("SystemRoot").unwrap())
            .env("ProgramFiles", &program_files)
            .env("ProgramFiles(x86)", temp.path().join("unused"))
            .env("LOCALAPPDATA", temp.path().join("local"))
            .env("GOROOT", "chosen")
            .env("COS_RUST_PUBLIC_TOOLCHAIN_CHILD", mode);
        if mode == "home" {
            command
                .env("JAVA_HOME", &invalid)
                .env("Path", temp.path().join("empty"));
        } else {
            command.env("Path", &invalid);
        }
        let output = command
            .args([
                "--exact",
                "unrepresentable_native_values_cannot_erase_a_toolchain_choice_or_path",
                "--nocapture",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
