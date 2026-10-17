use base64::{Engine, engine::general_purpose::STANDARD};
use cos_store::{CreateOptions, ReadOptions, SessionStore, UpsertOptions, chronology, model};
use serde_json::{Value, json};
use std::{fs, sync::Arc};
const NOW: i64 = 1792108800000;
#[test]
fn cleanup_byte_target_retires_only_files_actually_selected() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    create(&store, "session-cap", "conversation-a");
    for (index, bytes) in [
        b"public image one".as_slice(),
        b"public image two".as_slice(),
    ]
    .iter()
    .enumerate()
    {
        let asset = store
            .write_asset("session-cap", bytes, "image/png")
            .unwrap();
        store.upsert("session-cap",json!({"kind":"user_message","time":100+index,"source":"extension","messageId":format!("u{index}"),"message":text("image"),"assets":[asset]}),Default::default()).unwrap();
    }
    let before = store.read("session-cap", ReadOptions::default()).unwrap();
    assert_eq!(store.clear_images(Some(0)).unwrap()["removedFiles"], 0);
    assert_eq!(
        store.read("session-cap", ReadOptions::default()).unwrap(),
        before
    );
    assert_eq!(store.clear_images(Some(1)).unwrap()["removedFiles"], 1);
    let rows = store.read("session-cap", ReadOptions::default()).unwrap();
    assert_eq!(rows.iter().filter(|r| r["assets"].is_array()).count(), 1);
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/upstream/history-case.json")).unwrap()
}
fn text(s: &str) -> Value {
    json!({"text":s,"truncated":false,"chars":s.encode_utf16().count()})
}
fn create(store: &SessionStore, id: &str, conversation: &str) {
    store
        .create(CreateOptions {
            id: Some(id.into()),
            conversation_id: Some(conversation.into()),
            now: Some(NOW),
            ..Default::default()
        })
        .unwrap();
}
#[test]
fn actual_upstream_operations_and_restart_projection_match() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    let case = fixture();
    let id = case["id"].as_str().unwrap();
    for (i, operation) in case["operations"].as_array().unwrap().iter().enumerate() {
        let input = operation[1].clone();
        let actual = match operation[0].as_str().unwrap() {
            "create" => store
                .create(CreateOptions {
                    id: Some(id.into()),
                    title: input["title"].as_str().map(String::from),
                    conversation_id: input["conversationId"].as_str().map(String::from),
                    now: Some(NOW),
                    ..Default::default()
                })
                .unwrap(),
            "append" => store.append(id, input).unwrap(),
            "upsert" => serde_json::to_value(
                store
                    .upsert(
                        id,
                        input,
                        UpsertOptions {
                            now: Some(NOW),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
            )
            .unwrap(),
            "asset" => store
                .write_asset(
                    id,
                    input["text"].as_str().unwrap().as_bytes(),
                    input["mimeType"].as_str().unwrap(),
                )
                .unwrap(),
            _ => panic!(),
        };
        assert_eq!(actual, case["results"][i], "operation {i}");
    }
    store.flush().unwrap();
    assert_eq!(store.summary(id).unwrap().unwrap(), case["summary"]);
    assert_eq!(
        store.read(id, ReadOptions::default()).unwrap(),
        case["events"].as_array().unwrap().clone()
    );
    assert_eq!(
        store.recent(id, 3, None).unwrap(),
        case["recent"].as_array().unwrap().clone()
    );
    drop(store);
    let restarted = SessionStore::open(dir.path()).unwrap();
    assert_eq!(
        restarted.read(id, ReadOptions::default()).unwrap(),
        case["events"].as_array().unwrap().clone()
    );
    assert_eq!(restarted.summary(id).unwrap().unwrap(), case["summary"]);
}
#[test]
fn reads_original_files_and_lazy_legacy_overlay_without_modifying_history() {
    let dir = tempfile::tempdir().unwrap();
    let case = fixture();
    let id = case["id"].as_str().unwrap();
    let base = dir.path().join("sessions").join(id);
    fs::create_dir_all(&base).unwrap();
    for (name, body) in case["files"].as_object().unwrap() {
        let path = base.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, STANDARD.decode(body.as_str().unwrap()).unwrap()).unwrap();
    }
    let shard =
        base.join("messages/24aa1c542e1e97b6bd469275d3ea7df8d68dada4d2641795d13aeb7cb7a162ca.json");
    let user: Value = serde_json::from_slice(&fs::read(&shard).unwrap()).unwrap();
    fs::write(
        base.join("messages.json"),
        serde_json::to_vec(&json!({"user_message\u{0}user-a":user})).unwrap(),
    )
    .unwrap();
    fs::remove_file(shard).unwrap();
    let original = fs::read(base.join("events.jsonl")).unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    assert_eq!(
        store.read(id, ReadOptions::default()).unwrap(),
        case["events"].as_array().unwrap().clone()
    );
    let mut revised = user;
    revised["message"] = text("edited");
    store.upsert(id, revised, UpsertOptions::default()).unwrap();
    assert_eq!(fs::read(base.join("events.jsonl")).unwrap(), original);
    assert!(base.join("messages.json").is_file());
}
#[test]
fn concurrent_appends_torn_tail_and_restart_never_reuse_sequences() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(SessionStore::open(dir.path()).unwrap());
    let id = "test-concurrent";
    create(&store, id, "conversation-a");
    let jobs: Vec<_> = (0..24)
        .map(|n| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .append(
                        id,
                        json!({"time":100+n,"kind":"note","source":"app","message":text("x")}),
                    )
                    .unwrap()
            })
        })
        .collect();
    for job in jobs {
        job.join().unwrap();
    }
    store.flush().unwrap();
    drop(store);
    let file = dir.path().join("sessions").join(id).join("events.jsonl");
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap()
        .write_all(b"{\"seq\":25,\"broken\":")
        .unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    assert_eq!(
        store
            .append(
                id,
                json!({"time":200,"kind":"note","source":"app","message":text("new")})
            )
            .unwrap()["seq"],
        25
    );
    assert_eq!(store.read(id, ReadOptions::default()).unwrap().len(), 25);
}
#[test]
fn rebind_preserves_local_epoch_and_lifetime_but_refuses_collision_and_stale_source() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    let a = "11111111-1111-4111-8111-111111111111";
    let b = "22222222-2222-4222-8222-222222222222";
    create(&store, "session-a", a);
    store.upsert("session-a",json!({"kind":"user_message","time":100,"messageId":"user","source":"extension","message":text("12345678")}),UpsertOptions::default()).unwrap();
    assert!(
        store
            .rebind("session-a", Some(a), b, Some("handoff-a"), NOW + 1)
            .unwrap()
    );
    let moved = store.summary("session-a").unwrap().unwrap();
    assert_eq!(moved["id"], "session-a");
    assert_eq!(moved["estimatedTokens"], 2);
    assert_eq!(moved["contextTokens"], 0);
    assert_eq!(moved["chatIds"], json!([a, b]));
    assert!(
        !store
            .rebind("session-a", Some(a), "other", None, NOW + 2)
            .unwrap()
    );
    create(&store, "session-c", "33333333-3333-4333-8333-333333333333");
    assert!(
        !store
            .rebind(
                "session-c",
                Some("33333333-3333-4333-8333-333333333333"),
                b,
                None,
                NOW + 2
            )
            .unwrap()
    );
}
#[test]
fn failed_shard_write_does_not_publish_message_or_advance_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    create(&store, "session-fail", "conversation-a");
    fs::write(
        dir.path().join("sessions/session-fail/messages"),
        b"not a directory",
    )
    .unwrap();
    assert!(store.upsert("session-fail",json!({"kind":"user_message","time":100,"messageId":"u","message":text("x"),"source":"extension"}),UpsertOptions::default()).is_err());
    assert_eq!(store.summary("session-fail").unwrap().unwrap()["events"], 0);
    fs::remove_file(dir.path().join("sessions/session-fail/messages")).unwrap();
    assert_eq!(
        store
            .append(
                "session-fail",
                json!({"kind":"note","time":101,"message":text("x"),"source":"app"})
            )
            .unwrap()["seq"],
        1
    );
}
#[test]
fn chronology_and_usage_match_actual_upstream_helpers() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/chronology-cases.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        assert_eq!(
            chronology::chronological(case["entries"].as_array().unwrap()),
            case["expected"].as_array().unwrap().clone(),
            "{}",
            case["name"]
        );
    }
    let cases: Value =
        serde_json::from_str(include_str!("../../../fixtures/upstream/usage-cases.json")).unwrap();
    for case in cases["tokens"].as_array().unwrap() {
        assert_eq!(
            model::event_tokens(&case["event"]),
            case["expected"].as_u64().unwrap()
        );
    }
}
#[test]
fn assets_are_deduplicated_bounded_and_not_pruned_with_history() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    create(&store, "session-assets", "conversation-a");
    let a = store
        .write_asset("session-assets", b"same", "text/plain")
        .unwrap();
    let b = store
        .write_asset("session-assets", b"same", "text/plain")
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(
        store
            .read_asset("session-assets", a["id"].as_str().unwrap(), Some(3))
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .read_asset("session-assets", "../meta.json", None)
            .unwrap(),
        None
    );
    assert_eq!(store.prune(1), 0);
    assert!(store.summary("session-assets").unwrap().is_some());
}
#[test]
fn markdown_transcript_matches_upstream() {
    let dir = tempfile::tempdir().unwrap();
    let case = fixture();
    let id = case["id"].as_str().unwrap();
    let base = dir.path().join("sessions").join(id);
    fs::create_dir_all(&base).unwrap();
    for (name, body) in case["files"].as_object().unwrap() {
        let path = base.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, STANDARD.decode(body.as_str().unwrap()).unwrap()).unwrap();
    }
    let store = SessionStore::open(dir.path()).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/markdown-case.json"
    ))
    .unwrap();
    assert_eq!(
        store.export_markdown(id).unwrap(),
        expected["expected"].as_str().unwrap()
    );
}
#[test]
fn process_completion_is_a_single_stable_item_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    create(&store, "session-process", "conversation-a");
    let event = json!({"kind":"tool_call","source":"mcp","time":100,"call":{"callId":"process-one","tool":"exec_command","conversationId":"conversation-a","args":text("pwd"),"result":text("running"),"outcome":"ok","summary":{"title":"Started pwd","kind":"command"},"process":{"sessionId":12}}});
    store.record_process("session-process", event).unwrap();
    assert!(
        store
            .complete_process(
                "session-process",
                "process-one",
                json!({"completedAt":110,"durationMs":10,"exitCode":3})
            )
            .unwrap()
    );
    assert!(
        !store
            .complete_process(
                "session-process",
                "process-one",
                json!({"completedAt":120,"durationMs":20,"exitCode":0})
            )
            .unwrap()
    );
    store.flush().unwrap();
    drop(store);
    let store = SessionStore::open(dir.path()).unwrap();
    let events = store
        .read("session-process", ReadOptions::default())
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["call"]["process"]["exitCode"], 3);
    assert_eq!(events[0]["origin"], 1);
}
#[test]
fn plan_epoch_and_revision_fences_preserve_details() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    create(&store, "session-plan", "conversation-a");
    let plan = json!({"plan":[{"step":"one","status":"in_progress","details":"public evidence"}]});
    assert!(
        store
            .update_plan("session-plan", "conversation-a", plan.clone(), 100, None)
            .unwrap()
    );
    assert!(
        !store
            .update_plan("session-plan", "conversation-a", plan.clone(), 99, None)
            .unwrap()
    );
    assert!(
        store
            .rebind(
                "session-plan",
                Some("conversation-a"),
                "conversation-b",
                Some("handoff-a"),
                110
            )
            .unwrap()
    );
    assert!(
        !store
            .update_plan("session-plan", "conversation-a", plan.clone(), 120, None)
            .unwrap()
    );
    assert!(
        store
            .update_plan("session-plan", "conversation-a", plan, 105, Some(106))
            .unwrap()
    );
    assert_eq!(
        store.read_plan("session-plan").unwrap().unwrap()["plan"][0]["details"],
        "public evidence"
    );
}
#[test]
fn explicit_image_clear_cannot_remove_overflow_or_let_replay_revive_retired_reference() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    create(&store, "session-images", "conversation-a");
    let image = store
        .write_asset("session-images", b"public png bytes", "image/png")
        .unwrap();
    let overflow = store
        .write_asset("session-images", b"full transcript", "text/plain")
        .unwrap();
    let event = json!({"kind":"user_message","time":100,"source":"extension","messageId":"u","message":text("image"),"assets":[image.clone()]});
    store
        .upsert("session-images", event.clone(), Default::default())
        .unwrap();
    store.clear_images(None).unwrap();
    assert!(
        store
            .read_asset("session-images", image["id"].as_str().unwrap(), None)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .read_asset("session-images", overflow["id"].as_str().unwrap(), None)
            .unwrap()
            .is_some()
    );
    store
        .upsert("session-images", event, Default::default())
        .unwrap();
    let events = store
        .read("session-images", ReadOptions::default())
        .unwrap();
    assert!(!events[0]["assets"].is_array());
    assert_eq!(events[0]["retiredImageAssetIds"], json!([image["id"]]));
}
#[test]
fn unicode_snippets_and_unrecognized_shard_names_do_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    create(&store, "session-unicode", "conversation-a");
    store.upsert("session-unicode",json!({"kind":"user_message","time":100,"source":"extension","messageId":"u","message":text("École café 😀 alpha omega")}),Default::default()).unwrap();
    assert_eq!(
        store.search("cafe omega", 50).unwrap()["results"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let folder = dir.path().join("sessions/session-unicode/messages");
    let invalid_name = format!("{}éx.json", "a".repeat(61));
    fs::write(folder.join(invalid_name), b"{}").unwrap();
    store.flush().unwrap();
    drop(store);
    assert!(
        SessionStore::open(dir.path())
            .unwrap()
            .summary("session-unicode")
            .unwrap()
            .is_some()
    );
}
