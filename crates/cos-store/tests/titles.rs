use base64::{Engine, engine::general_purpose::STANDARD};
use cos_store::{
    CreateOptions, ReadOptions, SessionStore,
    title::{self, TitleSource},
};
use serde_json::{Value, json};

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-title.json"
    ))
    .unwrap()
}
fn projection(summary: &Value) -> Value {
    Value::Object(
        ["title", "titleSource", "autoTitle", "origin"]
            .into_iter()
            .filter_map(|key| summary.get(key).map(|value| (key.into(), value.clone())))
            .collect(),
    )
}
fn options(case: &Value) -> CreateOptions {
    CreateOptions {
        id: Some(case["id"].as_str().unwrap().into()),
        title: case["options"]["title"].as_str().map(str::to_owned),
        title_source: case["options"]["titleSource"].as_str().map(str::to_owned),
        origin: case["options"].get("origin").cloned(),
        conversation_id: Some("conversation-public".into()),
        now: Some(1792108800000),
    }
}
fn replay(names: &[&str]) {
    let fixture = corpus();
    for case in fixture["scenarios"].as_array().unwrap() {
        if !names.contains(&case["name"].as_str().unwrap()) {
            continue;
        }
        let directory = tempfile::tempdir().unwrap();
        let mut store = SessionStore::open(directory.path()).unwrap();
        let id = case["id"].as_str().unwrap();
        assert_eq!(
            projection(&store.create(options(case)).unwrap()),
            case["initial"]
        );
        for step in case["steps"].as_array().unwrap() {
            match step["operation"][0].as_str().unwrap() {
                "upsert" => {
                    store
                        .upsert(id, step["operation"][1].clone(), Default::default())
                        .unwrap();
                }
                "restart" => {
                    store.flush().unwrap();
                    drop(store);
                    store = SessionStore::open(directory.path()).unwrap();
                }
                "rename" => {
                    let input = &step["operation"][1];
                    let source = match input["source"].as_str().unwrap() {
                        "manual" => TitleSource::Manual,
                        "provider" => TitleSource::Provider,
                        "fallback" => TitleSource::Fallback,
                        _ => unreachable!(),
                    };
                    store
                        .rename(
                            id,
                            input["title"].as_str().unwrap(),
                            source,
                            input["conversationId"].as_str(),
                        )
                        .unwrap();
                }
                "clear" => {
                    store.clear_name(id).unwrap();
                }
                other => panic!("Unexpected operation {other}"),
            }
            assert_eq!(
                projection(&store.summary(id).unwrap().unwrap()),
                step["expected"],
                "{} {:?}",
                case["name"],
                step["operation"]
            );
        }
        let rows = store.read(id, ReadOptions::default()).unwrap();
        // Repair is presentation only: all original canonical text and stable message IDs survive.
        assert_eq!(json!(rows), case["events"], "{} raw history", case["name"]);
    }
}
#[test]
fn framed_first_request_and_its_revisions_keep_source_spacing_and_identity() {
    replay(&["framed-revisions", "first-message-identity"]);
}
#[test]
fn restart_repairs_legacy_project_page_and_damaged_context_titles() {
    replay(&["project-page-repair", "damaged-context-title"]);
}
#[test]
fn planner_helper_is_reclassified_without_removing_its_recorded_message() {
    replay(&["planner-helper"]);
}
#[test]
fn manual_and_provider_titles_follow_actual_store_authority_across_restarts() {
    replay(&[
        "manual-provider-tracking",
        "desktop-provider-ignored",
        "worker-authority",
        "manual-empty-auto",
    ]);
}
#[test]
fn pinned_prompt_title_and_legacy_repair_functions_match_exact_source_units() {
    let fixture = corpus();
    for case in fixture["promptCases"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        assert_eq!(
            json!(title::user_prompt_text(text)),
            case["expected"],
            "prompt {text:?}"
        );
        assert_eq!(
            title::prepend_user_prompt(text, case["instructions"].as_str().unwrap()),
            case["prepended"]
        );
    }
    let mut refused = 0;
    for case in fixture["userCases"].as_array().unwrap() {
        let result = title::user_title(case["text"].as_str().unwrap(), case["authored"].as_str());
        if case["needsLosslessString"] == true {
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Unsupported);
            refused += 1;
        } else {
            assert_eq!(
                json!(result.unwrap().encode_utf16().collect::<Vec<_>>()),
                case["expectedUnits"],
                "{case}"
            );
        }
    }
    assert_eq!(refused, 1, "Keep refusal separate from exactness passes");
    for case in fixture["refreshCases"].as_array().unwrap() {
        let mut summary = case["summary"].clone();
        let events = case["events"].as_array().unwrap();
        let first = title::first_title_message(events);
        assert_eq!(
            first.map_or(Value::Null, |v| v["messageId"].clone()),
            case["firstMessageId"]
        );
        assert_eq!(title::automatic_title(&summary, first), case["automatic"]);
        assert_eq!(title::legacy_context_title(&summary), case["legacy"]);
        assert_eq!(title::legacy_label_pending(&summary), case["pending"]);
        assert_eq!(
            title::provider_title_ignored(&summary),
            case["providerIgnored"]
        );
        assert_eq!(
            title::project_page_title(summary["title"].as_str().unwrap()),
            case["projectPage"]
        );
        assert_eq!(
            title::refresh_user_title(&mut summary, events).unwrap(),
            case["changed"]
        );
        assert_eq!(summary, case["expected"], "{case}");
    }
}
#[test]
fn refused_title_and_failed_metadata_publication_preserve_the_last_durable_name() {
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path()).unwrap();
    let id = "session-title-write";
    let before = store
        .create(CreateOptions {
            id: Some(id.into()),
            ..Default::default()
        })
        .unwrap();
    let invalid_slice = format!("{}😀", "a".repeat(119));
    assert_eq!(
        store
            .rename(id, &invalid_slice, TitleSource::Manual, None)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Unsupported
    );
    assert_eq!(store.summary(id).unwrap().unwrap(), before);
    // A real filesystem failure after staging must not expose the uncommitted name.
    let meta = directory.path().join("sessions").join(id).join("meta.json");
    std::fs::remove_file(&meta).unwrap();
    std::fs::create_dir(&meta).unwrap();
    assert!(
        store
            .rename(id, "Uncommitted public", TitleSource::Manual, None)
            .is_err()
    );
    assert_eq!(store.summary(id).unwrap().unwrap(), before);
}
#[cfg(windows)]
#[test]
fn unreadable_primary_metadata_without_a_valid_backup_refuses_recovery() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let id = "session-meta-locked";
    let store = SessionStore::open(directory.path()).unwrap();
    let before = store
        .create(CreateOptions {
            id: Some(id.into()),
            title: Some("Persisted manual public".into()),
            title_source: Some("manual".into()),
            ..Default::default()
        })
        .unwrap();
    drop(store);
    let meta = directory.path().join("sessions").join(id).join("meta.json");
    let guard = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&meta)
        .unwrap();
    let store = SessionStore::open(directory.path()).unwrap();
    assert!(
        store.summary(id).is_err(),
        "An unreadable file is not an absent or empty session"
    );
    drop(guard);
    assert_eq!(store.summary(id).unwrap().unwrap(), before);
}
#[cfg(windows)]
#[test]
fn unreadable_primary_can_use_a_valid_backup_without_overwriting_the_locked_file() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let id = "session-meta-backup";
    let store = SessionStore::open(directory.path()).unwrap();
    let backup = store
        .create(CreateOptions {
            id: Some(id.into()),
            title: Some("Public backup".into()),
            ..Default::default()
        })
        .unwrap();
    let latest = store
        .rename(id, "Public latest", TitleSource::Manual, None)
        .unwrap();
    drop(store);
    let meta = directory.path().join("sessions").join(id).join("meta.json");
    let before = std::fs::read(&meta).unwrap();
    let guard = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&meta)
        .unwrap();
    let restored = SessionStore::open(directory.path()).unwrap();
    assert_eq!(restored.summary(id).unwrap().unwrap(), backup);
    drop(restored);
    drop(guard);
    assert_eq!(std::fs::read(&meta).unwrap(), before);
    assert_eq!(
        SessionStore::open(directory.path())
            .unwrap()
            .summary(id)
            .unwrap()
            .unwrap(),
        latest
    );
}
#[test]
fn legacy_and_invalid_presentation_metadata_are_normalized_without_losing_unknown_fields() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-metadata.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let directory = tempfile::tempdir().unwrap();
        let id = fixture["id"].as_str().unwrap();
        let store = SessionStore::open(directory.path()).unwrap();
        store
            .create(CreateOptions {
                id: Some(id.into()),
                ..Default::default()
            })
            .unwrap();
        drop(store);
        let meta = directory.path().join("sessions").join(id).join("meta.json");
        std::fs::write(meta, serde_json::to_vec(&case["raw"]).unwrap()).unwrap();
        let store = SessionStore::open(directory.path()).unwrap();
        assert_eq!(
            json!(store.summary(id).unwrap()),
            case["storeExpected"],
            "{}",
            case["name"]
        );
    }
}
#[test]
fn actual_source_metadata_normalization_matches_all_legacy_and_damaged_inputs() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-metadata.json"
    ))
    .unwrap();
    let id = fixture["id"].as_str().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        assert_eq!(
            serde_json::to_value(cos_store::summary::normalize(id, case["raw"].clone())).unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
    }
    for case in fixture["rawCases"].as_array().unwrap() {
        let parsed = serde_json::from_str(case["raw"].as_str().unwrap()).ok();
        assert_eq!(
            serde_json::to_value(parsed.and_then(|raw| cos_store::summary::normalize(id, raw)))
                .unwrap(),
            case["expected"]
        );
    }
}
#[test]
fn source_stale_checkpoint_recovery_keeps_metadata_and_rebound_context_delta() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-recovery.json"
    ))
    .unwrap();
    for case in fixture["recovery"].as_array().unwrap() {
        let directory = tempfile::tempdir().unwrap();
        let store = SessionStore::open(directory.path()).unwrap();
        let id = case["id"].as_str().unwrap();
        let session = directory.path().join("sessions").join(id);
        std::fs::create_dir_all(&session).unwrap();
        for (name, bytes) in case["files"].as_object().unwrap() {
            assert!(!name.contains("..") && !name.starts_with('/') && !name.contains('\\'));
            let target = session.join(name);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, STANDARD.decode(bytes.as_str().unwrap()).unwrap()).unwrap();
        }
        assert_eq!(
            store.summary(id).unwrap().unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
        assert_eq!(
            json!(store.read(id, ReadOptions::default()).unwrap()),
            case["events"],
            "{} raw history",
            case["name"]
        );
        store.flush().unwrap();
        drop(store);
        let store = SessionStore::open(directory.path()).unwrap();
        assert_eq!(
            store.summary(id).unwrap().unwrap(),
            case["expected"],
            "{} after second restart",
            case["name"]
        );
    }
}

#[test]
fn title_projection_never_discards_an_unrepresentable_utf16_preview_message() {
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path()).unwrap();
    let id = "session-title-lossless";
    store
        .create(CreateOptions {
            id: Some(id.into()),
            ..Default::default()
        })
        .unwrap();
    let text = format!("{}😀tail", "a".repeat(79));
    store.upsert(id, json!({"kind":"user_message","source":"extension","time":1,"messageId":"public-surrogate","message":{"text":text,"chars":85,"truncated":false}}), Default::default()).unwrap();
    assert_eq!(
        store.read(id, ReadOptions::default()).unwrap()[0]["message"]["text"],
        text
    );
    // serde_json cannot represent the original source's lone high surrogate. Do not claim
    // an exact projection by substituting U+FFFD, a scalar-based truncation or a private escape.
    assert_eq!(
        store.summary(id).unwrap().unwrap()["title"],
        "ChatGPT session"
    );
}
