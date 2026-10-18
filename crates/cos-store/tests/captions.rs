//! CL-03 source-oracle caption parity and durable event/summary projection.
use cos_core::persistence::DurableStore;
use cos_store::{
    CreateOptions, ReadOptions, SessionStore, caption,
    correlation::Correlations,
    recorder::{Recorder, ToolRecordContext},
};
use serde_json::{Value, json};
use std::sync::Arc;

#[test]
fn captured_source_caption_corpus_is_retained_as_a_differential_gate() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-summaries.json"
    ))
    .unwrap();
    let mut checked = 0;
    let mut mismatch = Vec::new();
    let mut excluded = 0;
    for (index, row) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        if row["needsLosslessString"] == true
            || !row["expected"]["title"].is_string()
            || !row["running"]["title"].is_string()
        {
            // Source emits an unpaired JS UTF-16 surrogate; serde_json's String
            // cannot represent it losslessly. Keep this an explicit future edge gate.
            excluded += 1;
            continue;
        }
        checked += 1;
        let tool = row["tool"].as_str().unwrap();
        let args = &row["args"];
        let evidence = &row["evidence"];
        let result_head = row["resultHead"].as_str().unwrap_or("");
        let actual = caption::completed(
            tool,
            args,
            evidence,
            row["outcome"].as_str().unwrap(),
            result_head,
        );
        let running = caption::running(tool, args, evidence);
        if actual != row["expected"] || running != row["running"] {
            mismatch.push(format!(
                "#{index} {tool} {} args={} expected={} actual={} running-expected={} actual={}",
                row["outcome"], args, row["expected"], actual, row["running"], running
            ));
        }
    }
    assert_eq!(
        excluded, 5,
        "Source UTF-16 corpus exclusion changed unexpectedly"
    );
    assert_eq!(checked, 338);
    assert!(
        mismatch.is_empty(),
        "{} of {checked} well-formed source captions differ:\n{}",
        mismatch.len(),
        mismatch
            .iter()
            .take(22)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn sanitized_evidence_captions_and_last_activity_survive_cold_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    let durable = DurableStore::open(dir.path()).unwrap();
    let registry = Arc::new(Correlations::restore(durable, &store).unwrap());
    let recorder = Recorder::new(store.clone(), registry);
    let first = recorder
        .record_call_details(
            "unattributed",
            "find",
            json!({"query":"public","mode":"content","api_key":"private-secret-777"}),
            r#"{"content":[{"type":"text","text":"2 matches"}]}"#,
            1_000,
            "ok",
            Some(&ToolRecordContext {
                call_id: "cl03-find-1".into(),
                parent_call_id: None,
                duration_ms: 5,
                evidence: json!({"count":2,"changes":[],"failed":false}),
            }),
        )
        .unwrap();
    assert_eq!(
        first["call"]["summary"],
        json!({
            "kind":"search","tone":"neutral","title":"Searched \"public\"",
            "detail":"2 matches in file contents"
        })
    );
    assert_eq!(
        first["call"]["runningSummary"]["title"],
        "Searching \"public\""
    );
    let second = recorder.record_call_details(
        "unattributed", "exec_command", json!({"cmd":"exit 7"}),
        r#"{"content":[{"type":"text","text":"Exit status 7"}],"structuredContent":{"exit_code":7}}"#,
        1_001, "process_exit_nonzero",
        Some(&ToolRecordContext {
            call_id:"cl03-exit-2".into(), parent_call_id:None,duration_ms:5,
            evidence:json!({"exitCode":7,"running":false,"durationMs":4}),
        }),
    ).unwrap();
    assert_eq!(second["call"]["summary"]["tone"], "bad");
    assert_eq!(second["call"]["summary"]["metric"], "✕ exit 7");
    assert!(!first.to_string().contains("private-secret-777"));
    recorder.flush().unwrap();
    drop(recorder);
    drop(store);
    let reopened = SessionStore::open(dir.path()).unwrap();
    let id = first["sessionId"].as_str().unwrap();
    let history = reopened.read(id, ReadOptions::default()).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["call"]["summary"], first["call"]["summary"]);
    assert_eq!(history[1]["call"]["summary"], second["call"]["summary"]);
    let overview = reopened.summary(id).unwrap().unwrap();
    assert_eq!(overview["lastToolActivity"]["kind"], "run");
    assert_eq!(
        overview["lastToolActivity"]["title"],
        second["call"]["summary"]["title"]
    );
    assert_eq!(overview["processExitNonzero"], 1);
    assert!(
        !reopened
            .export_json(id)
            .unwrap()
            .to_string()
            .contains("private-secret-777")
    );
}

#[test]
fn failure_captions_do_not_claim_success_or_leak_sensitive_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    store
        .create(CreateOptions {
            id: Some("cl03-errors".into()),
            ..Default::default()
        })
        .unwrap();
    let args = json!({"paths":["/p/public.txt"],"token":"private-token-999"});
    let summary = caption::completed(
        "read",
        &cos_store::redaction::arguments("read", &args),
        &Value::Null,
        "tool_internal_error",
        "private-token-999",
    );
    assert_eq!(summary["tone"], "bad");
    assert!(!summary["title"].as_str().unwrap().contains("success"));
    // Caller must redact result BEFORE invoking pure caption projection.
    let safe_result = cos_store::redaction::result("read", &args, "private-token-999");
    let safe = caption::completed(
        "read",
        &cos_store::redaction::arguments("read", &args),
        &Value::Null,
        "tool_internal_error",
        &safe_result,
    );
    assert!(!safe.to_string().contains("private-token-999"));
}
