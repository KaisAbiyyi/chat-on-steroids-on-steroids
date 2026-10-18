//! CL-02: recorder evidence must describe trusted runtime facts, not tool prose.
use cos_core::persistence::ConfigStore;
use cos_mcp::kernel::{Kernel, Surface};
use cos_store::ReadOptions;
use serde_json::{Value, json};
use std::{fs, sync::Arc};

struct Fixture {
    profile: tempfile::TempDir,
    root: tempfile::TempDir,
    kernel: Arc<Kernel>,
}
impl Fixture {
    fn new(command: bool) -> Self {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let config = Arc::new(
            ConfigStore::open(profile.path(), if cfg!(windows) { "win32" } else { "linux" })
                .unwrap(),
        );
        config
            .update(|v| {
                v["roots"] = json!([{"name":"p","path":fs::canonicalize(root.path()).unwrap()}]);
                v["multiAgent"]["allowUnattributedCalls"] = json!(true);
                v["sessions"]["record"] = json!(true);
                v["capabilities"]["command"] = json!(command);
                v["capabilities"]["search"] = json!(true);
                v["capabilities"]["read"] = json!(true);
                v["capabilities"]["create"] = json!(true);
                v["capabilities"]["edit"] = json!(true);
            })
            .unwrap();
        let kernel = Kernel::new(config).unwrap();
        Self {
            profile,
            root,
            kernel,
        }
    }
    fn invoke(&self, tool: &str, args: Value) -> Value {
        let result = self.kernel.invoke(Surface::Core, "cl-02-fixture", tool, args);
        assert!(result.get("_recordingEvidence").is_none(), "{result}");
        result
    }
    fn recent(&self, name: &str) -> Value {
        self.kernel
            .sessions
            .read("unattributed-activity", ReadOptions::default())
            .unwrap()
            .into_iter()
            .rev()
            .find(|e| e["kind"] == "tool_call" && e["call"]["tool"] == name)
            .expect("tool call must be durably recorded")
    }
}

#[test]
fn find_count_is_from_engine_not_inferred_from_output() {
    let f = Fixture::new(false);
    fs::write(f.root.path().join("needle.txt"), "one needle\ntwo needle\n").unwrap();
    let output = f.invoke("find", json!({"query":"needle","mode":"content","path":"/p/needle.txt"}));
    assert_eq!(output["isError"], Value::Null, "{output}");
    assert_eq!(output.get("structuredContent"), None, "MCP wire contract changed");
    let event = f.recent("find");
    assert_eq!(event["call"]["evidence"]["count"], 2);
    assert_eq!(event["call"]["evidence"]["failed"], false);
    assert_eq!(event["call"]["evidence"]["changes"], json!([]));
    let rejected = f.invoke("find", json!({"query":"needle","regex":true}));
    assert_eq!(rejected["isError"], true, "{rejected}");
    assert!(f.recent("find")["call"]["evidence"]["count"].is_null());
    f.kernel.recorder.flush().unwrap();
    drop(f.kernel);
    let reopened = cos_store::SessionStore::open(f.profile.path()).unwrap();
    assert_eq!(reopened.read("unattributed-activity", ReadOptions::default()).unwrap()
        .iter().filter(|e| e["call"]["evidence"]["count"] == 2).count(), 1);
}

#[test]
fn patch_evidence_uses_committed_report_and_never_a_failed_patch_body() {
    let f = Fixture::new(false);
    let source = "*** Begin Patch\n*** Add File: made.txt\n+public evidence\n*** End Patch";
    let result = f.invoke("apply_patch", json!({"patch":source}));
    assert_ne!(result["isError"], true, "{result}");
    assert!(f.root.path().join("made.txt").is_file());
    let event = f.recent("apply_patch");
    let evidence = &event["call"]["evidence"];
    assert_eq!(evidence["failed"], false);
    let changes = evidence["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "{evidence}");
    assert!(changes[0]["path"].as_str().unwrap().ends_with("made.txt"));
    let broken = f.invoke("apply_patch", json!({"patch":"garbled patch body"}));
    assert_eq!(broken["isError"], true, "{broken}");
    assert_eq!(f.recent("apply_patch")["call"]["evidence"]["changes"], json!([]));
}

#[test]
fn actual_process_exit_and_background_are_recorded_from_structured_status() {
    let f = Fixture::new(true);
    let output = f.invoke(
        "exec_command",
        json!({"cmd":"exit 7","login":false,"yield_time_ms":10000}),
    );
    assert_ne!(output["isError"], true, "{output}");
    let initial = f.recent("exec_command");
    let facts = &initial["call"]["evidence"];
    if let Some(id) = output["structuredContent"]["session_id"].as_i64() {
        assert_eq!(facts["running"], true, "{facts}");
        assert_eq!(facts["processSessionId"], id);
        assert!(facts["exitCode"].is_null());
        let mut settled = Value::Null;
        for _ in 0..20 {
            let poll = f.invoke(
                "write_stdin",
                json!({"session_id":id,"chars":"","yield_time_ms":250}),
            );
            if poll["structuredContent"]["exit_code"].is_number() {
                settled = poll;
                break;
            }
        }
        assert_ne!(settled, Value::Null, "process never completed");
        let final_event = f.recent("write_stdin");
        assert_eq!(final_event["call"]["evidence"]["exitCode"], 7);
        assert_eq!(final_event["call"]["evidence"]["running"], false);
        assert_eq!(final_event["call"]["outcome"], "process_exit_nonzero");
    } else {
        assert_eq!(output["structuredContent"]["exit_code"], 7, "{output}");
        assert_eq!(facts["exitCode"], 7);
        assert_eq!(facts["running"], false);
        assert_eq!(initial["call"]["outcome"], "process_exit_nonzero");
    }
}

#[test]
fn shell_intercepted_patch_has_change_evidence_without_phantom_process() {
    let f = Fixture::new(true);
    let command = "apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: intercepted.txt\n+public\n*** End Patch\nPATCH";
    let result = f.invoke("exec_command", json!({"cmd":command,"login":false,"yield_time_ms":1000}));
    assert_ne!(result["isError"], true, "{result}");
    assert!(f.root.path().join("intercepted.txt").exists());
    assert!(result.get("_recordingEvidence").is_none());
    let event = f.recent("exec_command");
    let facts = &event["call"]["evidence"];
    assert_eq!(facts["changes"].as_array().unwrap().len(), 1, "{facts}");
    assert!(facts["running"].is_null(), "{facts}");
    assert!(facts["exitCode"].is_null(), "{facts}");
}
