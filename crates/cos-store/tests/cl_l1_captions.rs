//! Additive source-oracle regressions for the CL-L1 native caption failures.
use cos_core::persistence::DurableStore;
use cos_store::{
    ReadOptions, SessionStore, caption,
    correlation::Correlations,
    recorder::{Recorder, ToolRecordContext},
};
use serde_json::{Value, json};
use std::sync::Arc;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/cl-l1-captions.json"
    ))
    .unwrap()
}

fn check_group(group: &str, expected_count: usize) {
    let fixture = corpus();
    let rows = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["group"] == group)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), expected_count);
    let mut mismatches = Vec::new();
    for row in rows {
        let tool = row["tool"].as_str().unwrap();
        let actual = caption::completed(
            tool,
            &row["args"],
            &row["evidence"],
            row["outcome"].as_str().unwrap(),
            row["resultHead"].as_str().unwrap(),
        );
        let running = caption::running(tool, &row["args"], &row["evidence"]);
        if actual != row["expected"] || running != row["running"] {
            mismatches.push(format!(
                "{}: expected={} actual={} running-expected={} actual={}",
                row["name"], row["expected"], actual, row["running"], running
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn native_windows_captions_match_pinned_source() {
    check_group("native", 14);
}

#[test]
fn completion_metrics_require_exit_evidence_and_preserve_errors() {
    check_group("completion", 6);
}

#[test]
fn command_labels_match_source_whitespace_and_utf16_boundaries() {
    check_group("label", 24);
}

#[test]
fn corrected_captions_and_full_arguments_survive_recording_and_cold_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    let registry =
        Arc::new(Correlations::restore(DurableStore::open(dir.path()).unwrap(), &store).unwrap());
    let recorder = Recorder::new(store.clone(), registry);
    let fixture = corpus();
    let rows = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["group"] == "native")
        .collect::<Vec<_>>();
    for (index, row) in rows.iter().enumerate() {
        let event = recorder
            .record_call_details(
                "cl-l1-public",
                row["tool"].as_str().unwrap(),
                row["args"].clone(),
                r#"{"content":[{"type":"text","text":"public"}]}"#,
                1_000 + index as i64,
                row["outcome"].as_str().unwrap(),
                Some(&ToolRecordContext {
                    call_id: format!("cl-l1-{index}"),
                    parent_call_id: None,
                    duration_ms: row["durationMs"].as_u64().unwrap(),
                    evidence: row["evidence"].clone(),
                }),
            )
            .unwrap();
        assert_eq!(event["call"]["summary"], row["expected"], "{}", row["name"]);
    }
    recorder.flush().unwrap();
    drop(recorder);
    drop(store);
    let reopened = SessionStore::open(dir.path()).unwrap();
    let history = reopened
        .read("unattributed-activity", ReadOptions::default())
        .unwrap();
    assert_eq!(history.len(), 14);
    for (event, row) in history.iter().zip(&rows) {
        assert_eq!(event["call"]["summary"], row["expected"]);
        assert_eq!(event["call"]["runningSummary"], row["running"]);
        let args: Value =
            serde_json::from_str(event["call"]["args"]["text"].as_str().unwrap()).unwrap();
        assert_eq!(
            args, row["args"],
            "Caption truncation changed recorded args"
        );
    }
    let overview = reopened.summary("unattributed-activity").unwrap().unwrap();
    // Last activity is a title/kind projection; full metrics live in history.
    let last = rows.last().unwrap();
    assert_eq!(
        overview["lastToolActivity"],
        json!({"title":last["expected"]["title"],"kind":last["expected"]["kind"]})
    );
    assert_eq!(overview["processExitNonzero"], json!(1));
}
