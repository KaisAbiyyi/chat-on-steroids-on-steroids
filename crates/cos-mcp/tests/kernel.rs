use cos_core::{config::Root, persistence::ConfigStore};
use cos_mcp::{
    kernel::{Kernel, Surface},
    schema,
};
use serde_json::{Value, json};
use std::sync::Arc;
fn platform() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}
#[test]
fn exec_text_contains_continuation_and_completion_metadata() {
    let d = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let approved = std::fs::canonicalize(project.path()).unwrap();
    let conf = Arc::new(ConfigStore::open(d.path(), platform()).unwrap());
    conf.update(|v| {
        v["roots"] = json!([{"name":"p","path":approved}]);
        v["multiAgent"]["allowUnattributedCalls"] = json!(true);
    })
    .unwrap();
    let kernel = Kernel::new(conf).unwrap();
    let launched = kernel.invoke(
        Surface::Core,
        "public-response",
        "exec_command",
        json!({"cmd":"echo public-result","shell":if cfg!(windows){"cmd"}else{"sh"},"max_output_tokens":0}),
    );
    // With process scheduling on shared cloud runners, launch may return a
    // running session. That is not an error or a completed-process receipt.
    // Assert the original 0-token launch never leaks output independently.
    assert!(
        !launched["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("public-result")
    );
    let mut result = launched;
    if let Some(session) = result["structuredContent"]["session_id"].as_i64() {
        for _ in 0..20 {
            result = kernel.invoke(
                Surface::Core,
                "public-response",
                "write_stdin",
                json!({"session_id":session,"yield_time_ms":250}),
            );
            if result["structuredContent"]["exit_code"].as_i64().is_some() {
                break;
            }
        }
    }
    let text = result["content"][0]["text"].as_str().unwrap();
    assert_eq!(result["structuredContent"]["exit_code"], 0, "{result}");
    assert!(text.contains("Process exited with code 0"), "{result}");
    assert!(text.contains("Completed session ID:"), "{result}");
    assert!(text.contains("Output:"), "{result}");
}
#[test]
fn removing_a_root_revokes_its_watch() {
    let d = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let approved = std::fs::canonicalize(project.path()).unwrap();
    let conf = Arc::new(ConfigStore::open(d.path(), platform()).unwrap());
    conf.update(|v| v["roots"] = json!([{"name":"p","path":approved}]))
        .unwrap();
    let kernel = Kernel::new(conf.clone()).unwrap();
    let id = kernel.watch("renderer-public", "/p").unwrap();
    conf.update(|v| v["roots"] = json!([])).unwrap();
    assert!(kernel.watch_poll("renderer-public", &id).is_err());
}
#[test]
fn real_tools_live_revocation_and_surface_boundary() {
    let d = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a"), "public file").unwrap();
    let config = Arc::new(ConfigStore::open(profile.path(), platform()).unwrap());
    let roots = [Root {
        name: "p".into(),
        path: std::fs::canonicalize(d.path())
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }];
    config
        .update(|v| {
            v["roots"] = json!(roots);
            v["multiAgent"]["allowUnattributedCalls"] = json!(true);
        })
        .unwrap();
    let kernel = Kernel::new(config.clone()).unwrap();
    let tools = kernel.tools(Surface::Core);
    assert!(tools.iter().any(|v| v["name"] == "read"));
    assert!(!tools.iter().any(|v| v["name"] == "save_image"));
    assert_eq!(kernel.tools(Surface::Desktop), Vec::<Value>::new());
    assert_eq!(kernel.tools(Surface::Plugins), Vec::<Value>::new());
    let r = kernel.invoke(
        Surface::Core,
        "request-a",
        "read",
        json!({"paths":["/p/a"]}),
    );
    assert!(
        r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("public file")
    );
    config
        .update(|v| {
            v["capabilities"]["read"] = json!(false);
            v["capabilities"]["browse"] = json!(false);
            v["capabilities"]["metadata"] = json!(false);
        })
        .unwrap();
    assert!(
        kernel
            .tools(Surface::Core)
            .iter()
            .any(|v| v["name"] == "read")
    );
    let r = kernel.invoke(
        Surface::Core,
        "request-a",
        "read",
        json!({"paths":["/p/a"]}),
    );
    assert_eq!(r["isError"], true);
    assert!(
        r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("TOOL_DISABLED")
    );
    assert_eq!(
        kernel.invoke(Surface::Desktop, "r", "read", json!({"paths":["/p/a"]}))["isError"],
        true
    );
}
#[test]
fn exact_argument_constraints_before_handlers() {
    let schema = json!({"type":"object","properties":{"n":{"type":"integer","minimum":1,"maximum":10}},"required":["n"],"additionalProperties":false});
    assert!(schema::validate(&schema, &json!({"n":2})).is_ok());
    assert!(schema::validate(&schema, &json!({"n":2.5})).is_err());
    assert!(schema::validate(&schema, &json!({"n":1,"extra":true})).is_err());
    assert!(schema::validate(&schema, &json!({})).is_err());
}
