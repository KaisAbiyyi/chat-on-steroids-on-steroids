//! CL-10: real cross-crate backend closure guards.
//! Synthetic temp profiles only; no browser, desktop, real accounts, or network tunnel.
use cos_core::persistence::ConfigStore;
use cos_mcp::kernel::{Kernel, Surface};
use cos_store::{ReadOptions, SessionStore};
use serde_json::{Value, json};
use std::{fs, sync::Arc};

fn platform() -> &'static str {
    if cfg!(windows) { "win32" } else { "linux" }
}

#[test]
fn patch_evidence_survives_restart_but_revoked_root_cannot_be_reused() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let config = Arc::new(ConfigStore::open(profile.path(), platform()).unwrap());
    config.update(|v| {
        v["roots"] = json!([{"name":"p","path":fs::canonicalize(project.path()).unwrap()}]);
        v["multiAgent"]["allowUnattributedCalls"] = json!(true);
        v["sessions"]["record"] = json!(true);
        v["capabilities"]["read"] = json!(true);
        v["capabilities"]["create"] = json!(true);
        v["capabilities"]["edit"] = json!(true);
    }).unwrap();

    let kernel = Kernel::new(config.clone()).unwrap();
    let names = kernel.tools(Surface::Core);
    assert!(names.iter().any(|v| v["name"] == "apply_patch"));
    assert!(names.iter().any(|v| v["name"] == "read"));

    let made = kernel.invoke(Surface::Core, "cl10-public-call", "apply_patch", json!({
        "patch": "*** Begin Patch\n*** Add File: accepted.txt\n+public accepted\n*** End Patch"
    }));
    assert_ne!(made["isError"], true, "{made}");
    assert_eq!(fs::read(project.path().join("accepted.txt")).unwrap(), b"public accepted\n");

    let read = kernel.invoke(Surface::Core, "cl10-public-call", "read", json!({
        "paths": ["/p/accepted.txt"]
    }));
    assert_ne!(read["isError"], true, "{read}");
    assert!(read["content"][0]["text"].as_str().unwrap().contains("public accepted"), "{read}");

    config.update(|v| {
        v["roots"] = json!([]);
        v["capabilities"]["read"] = json!(false);
        v["capabilities"]["create"] = json!(false);
        v["capabilities"]["edit"] = json!(false);
    }).unwrap();

    let denied = kernel.invoke(Surface::Core, "cl10-public-call", "apply_patch", json!({
        "patch": "*** Begin Patch\n*** Add File: denied.txt\n+not permitted\n*** End Patch"
    }));
    assert_eq!(denied["isError"], true, "{denied}");
    assert_eq!(kernel.invoke(Surface::Core, "cl10-public-call", "read", json!({
        "paths": ["/p/accepted.txt"]
    }))["isError"], true);
    assert_eq!(kernel.invoke(Surface::Desktop, "cl10-public-call", "read", json!({
        "paths": ["/p/accepted.txt"]
    }))["isError"], true);
    assert!(!project.path().join("denied.txt").exists());
    kernel.recorder.flush().unwrap();
    drop(kernel);
    drop(config);

    // A positive write is durable evidence; a denied attempted write is not a
    // committed change, even after a cold replay of the journal.
    let reopened = SessionStore::open(profile.path()).unwrap();
    let calls = reopened.read("unattributed-activity", ReadOptions::default()).unwrap();
    let accepted = calls.iter().find(|e| {
        e["kind"] == "tool_call" && e["call"]["tool"] == "apply_patch"
            && e["call"]["evidence"]["failed"] == false
    }).expect("committed patch evidence");
    assert_eq!(accepted["call"]["evidence"]["changes"].as_array().unwrap().len(), 1);
    let rejected = calls.iter().rev().find(|e| {
        e["kind"] == "tool_call" && e["call"]["tool"] == "apply_patch"
            && e["call"]["evidence"]["failed"] == true
    }).expect("denial evidence");
    assert_eq!(rejected["call"]["evidence"]["changes"], json!([]));
    assert_eq!(fs::read(project.path().join("accepted.txt")).unwrap(), b"public accepted\n");
}

#[test]
fn persistent_recorder_redaction_and_live_surface_revocation_share_one_profile() {
    let profile = tempfile::tempdir().unwrap();
    let config = Arc::new(ConfigStore::open(profile.path(), platform()).unwrap());
    config.update(|v| {
        v["multiAgent"]["allowUnattributedCalls"] = json!(true);
        v["sessions"]["record"] = json!(true);
    }).unwrap();
    let kernel = Kernel::new(config.clone()).unwrap();
    let recorded = kernel.recorder.record_call(
        "cl10-public-redaction",
        "exec_command",
        json!({"cmd":"echo public","env":{"ACCESS_TOKEN":"cl10-private-sentinel"}}),
        "public output",
        100,
        "ok",
    ).unwrap();
    assert!(!recorded.to_string().contains("cl10-private-sentinel"));
    assert_eq!(
        kernel.invoke(Surface::Plugins, "cl10-public-redaction", "read", json!({"paths":[]}))["isError"],
        true
    );
    kernel.recorder.flush().unwrap();
    let session = recorded["sessionId"].as_str().unwrap().to_owned();
    drop(kernel);
    drop(config);

    let reopened = SessionStore::open(profile.path()).unwrap();
    let events = reopened.read(&session, ReadOptions::default()).unwrap();
    assert!(events.iter().any(|e| e["kind"] == "tool_call"));
    for event in &events {
        assert!(!event.to_string().contains("cl10-private-sentinel"), "{event}");
    }
    let serialized: Value = serde_json::from_slice(
        &serde_json::to_vec(&events).unwrap()
    ).unwrap();
    assert!(serialized.is_array());
}
