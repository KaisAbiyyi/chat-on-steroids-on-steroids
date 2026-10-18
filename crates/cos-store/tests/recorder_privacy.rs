use cos_core::persistence::DurableStore;
use cos_store::{ReadOptions, SessionStore, correlation::Correlations, recorder::Recorder};
use serde_json::{Value, json};
use std::{fs, path::Path, sync::Arc};

fn open_recorder(path: &Path) -> (SessionStore, Recorder) {
    let store = SessionStore::open(path).unwrap();
    let durable = DurableStore::open(path).unwrap();
    let registry = Arc::new(Correlations::restore(durable, &store).unwrap());
    let recorder = Recorder::new(store.clone(), registry);
    (store, recorder)
}

fn assert_no_secrets(value: &str, secrets: &[&str]) {
    for needle in secrets {
        assert!(!value.contains(needle), "private input persisted: {needle}");
    }
}

fn check_persisted_files(path: &Path, secrets: &[&str]) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            check_persisted_files(&path, secrets);
        } else if entry.file_type().unwrap().is_file() {
            let contents = fs::read(&path).unwrap();
            let text = String::from_utf8_lossy(&contents);
            assert_no_secrets(&text, secrets);
        }
    }
}

#[test]
fn recorder_redacts_before_inline_journal_and_asset_persistence_even_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let secrets = [
        "env-sentinel-unique-001",
        "clipboard-sentinel-002",
        "password-sentinel-003",
        "token-sentinel-004",
        "binary-sentinel-005",
        "result-secret-sentinel-006",
    ];
    let (store, recorder) = open_recorder(temp.path());
    let mut args = json!({
        "path": "/public/a.txt",
        "env": {"HOME":"env-sentinel-unique-001", "PUBLIC_SETTING":"normal"},
        "nested":{"password":"password-sentinel-003","authorization":"token-sentinel-004"},
        "dataBase64":"binary-sentinel-005",
        "actions":[{"type":"write_clipboard","text":"clipboard-sentinel-002"}]
    });
    args["notes"] = json!("public metadata ".repeat(850));
    let long_result = format!(
        "{} token=result-secret-sentinel-006; Authorization: Bearer inline-token",
        "safe output ".repeat(1000)
    );
    let out = recorder
        .record_call("unknown", "computer", args, &long_result, 200, "ok")
        .unwrap();
    let out_json = out.to_string();
    assert_no_secrets(&out_json, &secrets);
    assert!(out_json.contains("/public/a.txt"));
    assert!(!out_json.contains("inline-token"));
    assert!(
        out["call"]["result"]["assetId"].is_string(),
        "result overflow must write asset"
    );
    assert!(
        out["call"]["args"]["assetId"].is_string(),
        "argument overflow must write asset"
    );
    let id = out["sessionId"].as_str().unwrap().to_owned();
    let stored_args = store.read_full_text(&id, &out["call"]["args"]).unwrap();
    let stored_result = store.read_full_text(&id, &out["call"]["result"]).unwrap();
    assert_no_secrets(&stored_args, &secrets);
    assert_no_secrets(&stored_result, &secrets);
    assert!(!stored_result.contains("inline-token"));
    recorder.flush().unwrap();
    drop(recorder);
    drop(store);

    // Cold reopen is essential: a clean API projection can still hide a raw
    // JSONL record or an overflow asset containing the original payload.
    let (reopened, _) = open_recorder(temp.path());
    let events = reopened.read(&id, ReadOptions::default()).unwrap();
    let event = events
        .iter()
        .find(|event| event["kind"] == "tool_call")
        .unwrap();
    assert_no_secrets(&event.to_string(), &secrets);
    assert_no_secrets(
        &reopened
            .read_full_text(&id, &event["call"]["args"])
            .unwrap(),
        &secrets,
    );
    assert_no_secrets(
        &reopened
            .read_full_text(&id, &event["call"]["result"])
            .unwrap(),
        &secrets,
    );
    drop(reopened);
    check_persisted_files(&temp.path().join("sessions"), &secrets);
}

#[test]
fn clipboard_reads_and_binary_payloads_fail_closed_in_recorded_events() {
    let temp = tempfile::tempdir().unwrap();
    let (_, recorder) = open_recorder(temp.path());
    let clipboard = recorder
        .record_call(
            "",
            "read_clipboard",
            json!({"format":"text"}),
            "SECRET_READ_CLIPBOARD_SENTINEL",
            300,
            "ok",
        )
        .unwrap();
    assert_no_secrets(&clipboard.to_string(), &["SECRET_READ_CLIPBOARD_SENTINEL"]);
    let nested_clipboard = recorder
        .record_call(
            "",
            "computer",
            json!({"actions":[{"type":"read_clipboard"}]}),
            "clipboard result: SECRET_NESTED_CLIPBOARD_SENTINEL",
            301,
            "ok",
        )
        .unwrap();
    assert_no_secrets(
        &nested_clipboard.to_string(),
        &["SECRET_NESTED_CLIPBOARD_SENTINEL"],
    );
    let download = recorder.record_call(
        "", "download_artifact",
        json!({"file":"PRIVATE_FILE_HANDLE","dataBase64":"LONG_BINARY_BODY", "image":{"type":"image","mimeType":"image/png","data":"ENCODED_IMAGE_BODY"}}),
        "public downloaded", 302, "ok"
    ).unwrap();
    assert_no_secrets(
        &download.to_string(),
        &[
            "PRIVATE_FILE_HANDLE",
            "LONG_BINARY_BODY",
            "ENCODED_IMAGE_BODY",
        ],
    );
    let clipboard_write = recorder
        .record_call(
            "",
            "write_clipboard",
            json!({"text":"SECRET_CLIPBOARD_WRITE"}),
            "public clipboard written",
            303,
            "ok",
        )
        .unwrap();
    assert_no_secrets(&clipboard_write.to_string(), &["SECRET_CLIPBOARD_WRITE"]);
    recorder.flush().unwrap();
    check_persisted_files(
        &temp.path().join("sessions"),
        &[
            "SECRET_READ_CLIPBOARD_SENTINEL",
            "SECRET_NESTED_CLIPBOARD_SENTINEL",
            "PRIVATE_FILE_HANDLE",
            "LONG_BINARY_BODY",
            "ENCODED_IMAGE_BODY",
            "SECRET_CLIPBOARD_WRITE",
        ],
    );
}

#[test]
fn non_sensitive_execution_evidence_remains_recordable() {
    let temp = tempfile::tempdir().unwrap();
    let (store, recorder) = open_recorder(temp.path());
    let event = recorder
        .record_call(
            "request-public",
            "exec_command",
            json!({"cmd":"printf hello", "workdir":"/project/public"}),
            "hello: command completed",
            400,
            "ok",
        )
        .unwrap();
    let id = event["sessionId"].as_str().unwrap();
    let args: Value =
        serde_json::from_str(&store.read_full_text(id, &event["call"]["args"]).unwrap()).unwrap();
    assert_eq!(args["cmd"], "printf hello");
    assert_eq!(args["workdir"], "/project/public");
    assert_eq!(
        store.read_full_text(id, &event["call"]["result"]).unwrap(),
        "hello: command completed"
    );
}
