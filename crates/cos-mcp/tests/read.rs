use cos_core::persistence::ConfigStore;
use cos_mcp::kernel::{Kernel, Surface};
use serde_json::json;
use std::sync::Arc;
#[test]
fn mixed_read_keeps_successes_and_path_specific_line_ranges() {
    let d = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let approved = std::fs::canonicalize(d.path()).unwrap();
    std::fs::write(d.path().join("a"), "first\npublic second\nthird\n").unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    config
        .update(|v| {
            v["roots"] = json!([{"name":"p","path":approved}]);
            v["multiAgent"]["allowUnattributedCalls"] = json!(true);
        })
        .unwrap();
    let kernel = Kernel::new(config).unwrap();
    let result = kernel.invoke(
        Surface::Core,
        "public",
        "read",
        json!({"paths":["/p/missing","/p/a:2-2"]}),
    );
    let content = result["content"].as_array().unwrap();
    assert!(
        content.iter().any(|v| v["text"]
            .as_str()
            .is_some_and(|t| t.contains("public second"))),
        "{result}"
    );
    assert!(
        !content
            .iter()
            .any(|v| v["text"].as_str().is_some_and(|t| t.contains("1\tfirst")))
    );
    assert!(
        content
            .iter()
            .any(|v| v["text"].as_str().is_some_and(|t| t.contains("missing")))
    );
}
