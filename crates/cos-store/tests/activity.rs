use cos_store::model;
use serde_json::{Value, json};

fn compare(names: &[&str], expected_refusals: usize) {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-activity.json"
    ))
    .unwrap();
    let mut refused = 0;
    for case in fixture["sequences"].as_array().unwrap() {
        if !names.contains(&case["name"].as_str().unwrap()) {
            continue;
        }
        let mut summary = case["initial"].clone();
        for (event, expected) in case["events"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
        {
            model::apply_summary(&mut summary, event);
            let mut projection = summary.clone();
            if expected["lastToolActivity"]["needsLosslessString"] == true {
                // The UTF-16 title itself is not serializable by serde_json; keep the
                // activity metadata absent and the raw event intact, never a scalar substitute.
                assert!(summary["lastToolActivity"].is_null());
                refused += 1;
                continue;
            }
            if let Some(text) = projection["lastToolActivity"]["title"].as_str() {
                let units = text.encode_utf16().collect::<Vec<_>>();
                projection["lastToolActivity"]
                    .as_object_mut()
                    .unwrap()
                    .remove("title");
                projection["lastToolActivity"]["titleUnits"] = json!(units);
                projection["lastToolActivity"]["needsLosslessString"] = json!(false);
            }
            assert_eq!(&projection, expected, "{} {event}", case["name"]);
        }
    }
    assert_eq!(
        refused, expected_refusals,
        "One refused projection remains an explicit exactness gap"
    );
}
#[test]
fn source_activity_preserves_fractional_time_and_newest_call() {
    compare(&["fractional-time", "recent-activity"], 0);
}
#[test]
fn source_activity_uses_utf16_title_units_without_substituting_lone_surrogates() {
    compare(
        &["utf16-activity-0", "utf16-activity-1", "utf16-activity-2"],
        1,
    );
}
#[test]
fn finish_decisions_require_their_typed_state_and_safe_sequence() {
    compare(
        &[
            "finish-work",
            "notified-does-not-decide",
            "invalid-decision-sequence",
            "negative-decision-sequence",
            "notified-does-not-advance-decision",
            "repeat-decision-advances",
            "legacy-finish-control",
        ],
        0,
    );
}
#[test]
fn source_finish_work_and_outcome_taxonomy_preserve_optional_identity() {
    compare(&["empty-turn-and-agent", "outcome-taxonomy"], 0);
}
#[test]
fn unrepresentable_activity_preview_keeps_the_exact_raw_call_across_restart() {
    use cos_store::{CreateOptions, ReadOptions, SessionStore};
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path()).unwrap();
    let id = "session-activity-surrogate";
    store
        .create(CreateOptions {
            id: Some(id.into()),
            ..Default::default()
        })
        .unwrap();
    let title = format!("{}😀", "a".repeat(199));
    let stored = json!({"text":"public","truncated":false,"chars":6});
    let event = store.append(id, json!({"kind":"tool_call","source":"mcp","time":1.5,"call":{"callId":"public-call","tool":"read","args":stored,"result":stored,"summary":{"kind":"read","title":title},"outcome":"ok"}})).unwrap();
    store.flush().unwrap();
    drop(store);
    let store = SessionStore::open(directory.path()).unwrap();
    let raw = std::fs::read_to_string(
        directory
            .path()
            .join("sessions")
            .join(id)
            .join("events.jsonl"),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(raw.trim_end()).unwrap(),
        event
    );
    let projected = store.read(id, ReadOptions::default()).unwrap();
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0]["call"], event["call"]);
    let summary = store.summary(id).unwrap().unwrap();
    assert_eq!(summary["lastToolCallAt"], 1.5);
    assert!(summary["lastToolActivity"].is_null());
}
