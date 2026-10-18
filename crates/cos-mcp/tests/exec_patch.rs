use cos_core::persistence::ConfigStore;
use cos_mcp::kernel::{Kernel, Surface};
use serde_json::{Value, json};
use std::{fs, sync::Arc};
struct Fixture {
    _profile: tempfile::TempDir,
    root: tempfile::TempDir,
    config: Arc<ConfigStore>,
    kernel: Arc<Kernel>,
}
impl Fixture {
    fn new() -> Self {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let config = Arc::new(
            ConfigStore::open(
                profile.path(),
                if cfg!(windows) { "win32" } else { "linux" },
            )
            .unwrap(),
        );
        config
            .update(|v| {
                v["roots"] = json!([{"name":"p","path":root.path()}]);
                v["multiAgent"]["allowUnattributedCalls"] = json!(true);
            })
            .unwrap();
        let kernel = Kernel::new(config.clone()).unwrap();
        Self {
            _profile: profile,
            root,
            config,
            kernel,
        }
    }
    fn exec(&self, cmd: &str) -> Value {
        self.kernel.invoke(
            Surface::Core,
            "public-exec-patch",
            "exec_command",
            json!({"cmd":cmd,"login":false,"yield_time_ms":1000}),
        )
    }
    fn no_child(&self) {
        assert_eq!(
            self.kernel
                .processes
                .snapshot("request:public-exec-patch")
                .unwrap(),
            json!([])
        );
    }
}
fn patch() -> &'static str {
    "*** Begin Patch\n*** Add File: public.txt\n+public\n*** End Patch"
}
fn command(body: &str) -> String {
    format!("apply_patch <<'PATCH'\n{body}\nPATCH")
}
#[test]
fn explicit_patch_is_intercepted_before_native_shell_under_command_authority() {
    let f = Fixture::new();
    f.config
        .update(|v| {
            for capability in ["edit", "create", "move", "deleteFile", "read"] {
                v["capabilities"][capability] = json!(false);
            }
        })
        .unwrap();
    let output = f.exec(&command(patch()));
    assert_eq!(
        fs::read_to_string(f.root.path().join("public.txt"))
            .ok()
            .as_deref(),
        Some("public\n"),
        "{output}"
    );
    assert_ne!(output["isError"], true, "{output}");
    assert_eq!(output["structuredContent"]["wall_time_seconds"], 0.0);
    assert!(output["structuredContent"]["exit_code"].is_null());
    assert!(output["structuredContent"]["session_id"].is_null());
    assert!(
        output["structuredContent"]["chunk_id"]
            .as_str()
            .is_none_or(str::is_empty)
    );
    f.no_child();
}
#[test]
fn intercepted_cd_uses_an_existing_approved_directory() {
    let f = Fixture::new();
    fs::create_dir(f.root.path().join("public folder")).unwrap();
    let output = f.exec(&format!("cd 'public folder' && {}", command(patch())));
    assert_eq!(
        fs::read_to_string(f.root.path().join("public folder/public.txt"))
            .ok()
            .as_deref(),
        Some("public\n"),
        "{output}"
    );
    assert!(!f.root.path().join("public.txt").exists());
    let missing = f.exec(&format!("cd missing && {}", command(patch())));
    assert_eq!(missing["isError"], true, "{missing}");
    assert!(!f.root.path().join("missing").exists());
    let outside = f.exec(&format!("cd /unapproved && {}", command(patch())));
    assert_eq!(outside["isError"], true, "{outside}");
    f.no_child();
}
#[test]
fn implicit_and_malformed_patches_return_correctness_errors_without_a_process() {
    let f = Fixture::new();
    for (cmd, expected) in [
        (
            patch().to_owned(),
            "patch detected without explicit call to apply_patch",
        ),
        (command("bad"), "invalid patch: The first line"),
    ] {
        let output = f.exec(&cmd);
        assert_eq!(output["isError"], true, "{output}");
        let text = output["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("apply_patch verification failed:") && text.contains(expected),
            "{output}"
        );
    }
    assert!(!f.root.path().join("public.txt").exists());
    f.no_child();
}
#[test]
fn command_revocation_still_refuses_an_intercepted_patch() {
    let f = Fixture::new();
    f.config
        .update(|v| v["capabilities"]["command"] = json!(false))
        .unwrap();
    assert_eq!(f.exec(&command(patch()))["isError"], true);
    assert!(!f.root.path().join("public.txt").exists());
    f.no_child();
}
#[test]
fn interception_does_not_bypass_the_command_allowlist() {
    let f = Fixture::new();
    f.config
        .update(|v| {
            v["commandAllowlist"]["enabled"] = json!(true);
            v["commandAllowlist"]["rules"] = json!([]);
        })
        .unwrap();
    let output = f.exec(&command(patch()));
    assert_eq!(output["isError"], true, "{output}");
    assert!(
        output["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("COMMAND_POLICY_REFUSED")
    );
    assert!(!f.root.path().join("public.txt").exists());
    f.no_child();
}
