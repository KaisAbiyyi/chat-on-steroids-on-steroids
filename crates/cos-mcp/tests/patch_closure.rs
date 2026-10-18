use cos_core::persistence::ConfigStore;
use cos_mcp::kernel::{Kernel, Surface};
use serde_json::json;
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
            .update(|value| {
                value["roots"] = json!([{"name":"p","path":root.path()}]);
                value["multiAgent"]["allowUnattributedCalls"] = json!(true);
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
    fn patch(&self, patch: &str) -> serde_json::Value {
        self.kernel.invoke(
            Surface::Core,
            "public-patch-closure",
            "apply_patch",
            json!({"patch":patch}),
        )
    }
}
#[test]
fn patch_default_mode_matches_source_normalization_on_a_real_crlf_file() {
    let f = Fixture::new();
    fs::write(f.root.path().join("a"), b"\xef\xbb\xbfone\r\ntwo\nthree").unwrap();
    let result =
        f.patch("*** Begin Patch\n*** Update File: a\n@@\n one\n-two\n+TWO\n three\n*** End Patch");
    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        fs::read(f.root.path().join("a")).unwrap(),
        b"\xef\xbb\xbfone\nTWO\nthree\n"
    );
}
#[test]
fn move_only_patch_keeps_original_bytes_when_edit_permission_is_off() {
    let f = Fixture::new();
    let original = b"\xef\xbb\xbfone\r\ntwo\r\n";
    fs::write(f.root.path().join("a"), original).unwrap();
    f.config
        .update(|value| value["capabilities"]["edit"] = json!(false))
        .unwrap();
    let result = f.patch(
        "*** Begin Patch\n*** Update File: a\n*** Move to: moved\n@@\n one\n two\n*** End Patch",
    );
    assert_ne!(result["isError"], true, "{result}");
    assert!(!f.root.path().join("a").exists());
    assert_eq!(fs::read(f.root.path().join("moved")).unwrap(), original);
}
#[test]
fn add_overwrite_requires_create_as_well_as_edit() {
    let f = Fixture::new();
    fs::write(f.root.path().join("a"), "kept").unwrap();
    f.config
        .update(|value| value["capabilities"]["create"] = json!(false))
        .unwrap();
    let result = f.patch("*** Begin Patch\n*** Add File: a\n+overwritten\n*** End Patch");
    assert_eq!(
        result["isError"], true,
        "Add syntax bypassed its Create permission: {result}"
    );
    assert_eq!(fs::read_to_string(f.root.path().join("a")).unwrap(), "kept");
}
#[test]
fn move_without_edit_cannot_replace_an_occupied_destination_or_rewrite_bytes() {
    let f = Fixture::new();
    f.config
        .update(|value| value["capabilities"]["edit"] = json!(false))
        .unwrap();
    fs::write(f.root.path().join("a"), "one\n").unwrap();
    fs::write(f.root.path().join("occupied"), "other\n").unwrap();
    let result = f.patch(
        "*** Begin Patch\n*** Update File: a\n*** Move to: occupied\n@@\n one\n*** End Patch",
    );
    assert_eq!(result["isError"], true);
    assert_eq!(
        fs::read_to_string(f.root.path().join("occupied")).unwrap(),
        "other\n"
    );
    fs::write(f.root.path().join("a"), "one").unwrap();
    let result =
        f.patch("*** Begin Patch\n*** Update File: a\n*** Move to: new\n@@\n one\n*** End Patch");
    assert_eq!(
        result["isError"], true,
        "Move alone must not rewrite an unterminated file"
    );
    assert_eq!(fs::read_to_string(f.root.path().join("a")).unwrap(), "one");
    assert!(!f.root.path().join("new").exists());
}
#[test]
fn mismatch_source_excerpt_requires_live_read_permission() {
    let f = Fixture::new();
    fs::write(
        f.root.path().join("a"),
        "public unique anchor\nprivate original line\n",
    )
    .unwrap();
    let patch = "*** Begin Patch\n*** Update File: a\n@@\n public unique anchor\n-wrong line\n+new\n*** End Patch";
    let result = f.patch(patch);
    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("Source excerpt from the patch verification snapshot"),
        "{text}"
    );
    assert!(text.contains("2\tprivate original line"));
    f.config
        .update(|value| value["capabilities"]["read"] = json!(false))
        .unwrap();
    let result = f.patch(patch);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        !text.contains("private original line"),
        "Read-disabled patch leaked the file: {text}"
    );
    assert!(!text.contains("Source excerpt"));
    assert_eq!(
        fs::read_to_string(f.root.path().join("a")).unwrap(),
        "public unique anchor\nprivate original line\n"
    );
}
#[test]
fn native_move_path_verification_matches_source_and_preserves_atomic_refusal() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/patch-verifier-paths.json"
    ))
    .unwrap();
    let platform = if cfg!(windows) { "win32" } else { "linux" };
    for case in corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["local"] == true && case["platform"] == platform)
    {
        let destination = case["destination"].as_str().unwrap();
        let f = Fixture::new();
        f.config
            .update(|value| value["capabilities"]["edit"] = json!(false))
            .unwrap();
        fs::write(f.root.path().join("a"), b"public\r\n").unwrap();
        let result=f.patch(&format!("*** Begin Patch\n*** Add File: earlier\n+must not be written\n*** Update File: a\n*** Move to: {destination}\n@@\n public\n*** End Patch"));
        if case["expected"]["allowed"] == true {
            assert_ne!(result["isError"], true, "{result}");
            assert!(!f.root.path().join("a").exists());
            assert_eq!(
                fs::read(f.root.path().join(destination)).unwrap(),
                b"public\r\n"
            );
            assert!(f.root.path().join("earlier").exists());
        } else {
            assert_eq!(result["isError"], true, "{result}");
            assert!(
                result["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains(case["expected"]["error"].as_str().unwrap()),
                "{result}"
            );
            assert_eq!(fs::read(f.root.path().join("a")).unwrap(), b"public\r\n");
            assert!(!f.root.path().join("earlier").exists());
        }
    }
}
