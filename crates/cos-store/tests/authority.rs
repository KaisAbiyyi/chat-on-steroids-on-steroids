use cos_core::persistence::DurableStore;
use cos_store::{
    CreateOptions, ReadOptions, SessionStore,
    access::{ConversationPolicy, WorkerOwner},
    correlation::{Correlation, Correlations},
    model,
    recorder::Recorder,
    usage,
};
use serde_json::json;
use std::{fs, sync::Arc, time::Duration};
fn owner(request: &str, conv: &str, session: &str, at: i64) -> Correlation {
    Correlation {
        request_id: request.into(),
        conversation_id: conv.into(),
        session_id: session.into(),
        message_id: "provider-message".into(),
        tool: "read".into(),
        observed_at: at,
    }
}
#[test]
fn proven_request_owner_and_session_epoch_cannot_move_on_reobservation() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    let durable = DurableStore::open(dir.path()).unwrap();
    let registry = Correlations::restore(durable.clone(), &store).unwrap();
    assert_eq!(
        registry
            .observe_and_commit(&[owner("r", "conversation-a", "session-a", 10)])
            .unwrap(),
        ["stored"]
    );
    assert_eq!(
        registry
            .observe_and_commit(&[owner("r", "conversation-a", "session-b", 20)])
            .unwrap(),
        ["same"]
    );
    assert_eq!(
        registry
            .observe_and_commit(&[owner("r", "conversation-b", "session-c", 30)])
            .unwrap(),
        ["refused"]
    );
    assert_eq!(registry.get("r").unwrap().unwrap().session_id, "session-a");
    drop(registry);
    let restored = Correlations::restore(durable, &store).unwrap();
    assert_eq!(
        restored.get("r").unwrap().unwrap().conversation_id,
        "conversation-a"
    );
    assert_eq!(restored.get("r").unwrap().unwrap().observed_at, 20);
}
#[test]
fn correlation_wait_uses_exact_id_and_committed_publication() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    let registry =
        Arc::new(Correlations::restore(DurableStore::open(dir.path()).unwrap(), &store).unwrap());
    let waiting = registry.clone();
    let job = std::thread::spawn(move || waiting.wait("r", Duration::from_secs(1)).unwrap());
    registry
        .observe_and_commit(&[owner("r", "conversation-a", "session-a", 10)])
        .unwrap();
    assert_eq!(job.join().unwrap().unwrap().request_id, "r");
    assert_eq!(
        registry.wait("unknown", Duration::from_millis(5)).unwrap(),
        None
    );
}
#[test]
fn malformed_authority_ledgers_refuse_startup() {
    let dir = tempfile::tempdir().unwrap();
    let durable = DurableStore::open(dir.path()).unwrap();
    fs::write(
        dir.path().join("state/request-correlations.json"),
        b"broken",
    )
    .unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    assert!(Correlations::restore(durable.clone(), &store).is_err());
    fs::write(
        dir.path().join("state/trusted-chats.json"),
        b"{\"version\":99,\"entries\":[]}",
    )
    .unwrap();
    assert!(ConversationPolicy::restore(durable).is_err());
}
#[test]
fn failed_trust_commit_cannot_grant_now_or_later_after_retry() {
    let dir = tempfile::tempdir().unwrap();
    let durable = DurableStore::open(dir.path()).unwrap();
    let policy = ConversationPolicy::restore(durable.clone()).unwrap();
    fs::create_dir(dir.path().join("state/trusted-chats.json")).unwrap();
    assert!(
        policy
            .set_trusted(&["conversation-a".into()], true)
            .is_err()
    );
    assert!(!policy.is_trusted("conversation-a").unwrap());
    fs::remove_dir(dir.path().join("state/trusted-chats.json")).unwrap();
    durable.flush().unwrap();
    assert!(
        !ConversationPolicy::restore(durable)
            .unwrap()
            .is_trusted("conversation-a")
            .unwrap()
    );
}
#[test]
fn strict_access_follows_exact_prime_and_committed_ancestry_with_revocation() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    store
        .create(CreateOptions {
            id: Some("session-a".into()),
            conversation_id: Some("conversation-a".into()),
            ..Default::default()
        })
        .unwrap();
    let policy = ConversationPolicy::restore(DurableStore::open(dir.path()).unwrap()).unwrap();
    policy
        .set_trusted(&["conversation-a".into()], true)
        .unwrap();
    assert!(
        policy
            .allowed(Some("conversation-a"), true, &store, |_| {
                WorkerOwner::default()
            })
            .unwrap()
    );
    assert!(
        store
            .rebind(
                "session-a",
                Some("conversation-a"),
                "conversation-b",
                Some("handoff-a"),
                10
            )
            .unwrap()
    );
    assert!(
        policy
            .allowed(Some("conversation-b"), true, &store, |_| {
                WorkerOwner::default()
            })
            .unwrap()
    );
    assert!(
        policy
            .allowed(Some("worker-conversation"), true, &store, |c| {
                if c == "worker-conversation" {
                    WorkerOwner {
                        owned: true,
                        prime: Some("conversation-b".into()),
                    }
                } else {
                    WorkerOwner::default()
                }
            })
            .unwrap()
    );
    policy
        .set_blocked(&["conversation-a".into()], true, 11)
        .unwrap();
    assert!(
        !policy
            .allowed(Some("conversation-b"), true, &store, |_| {
                WorkerOwner::default()
            })
            .unwrap()
    );
    assert!(
        !policy
            .allowed(None, true, &store, |_| WorkerOwner::default())
            .unwrap()
    );
    assert!(
        !policy
            .allowed(Some("conversation-a"), false, &store, |_| {
                WorkerOwner::default()
            })
            .unwrap()
    );
}
#[test]
fn recorder_restart_does_not_revive_completed_turn_and_unproved_call_stays_unattributed() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    let registry =
        Arc::new(Correlations::restore(DurableStore::open(dir.path()).unwrap(), &store).unwrap());
    let recorder = Recorder::new(store.clone(), registry.clone());
    let id = recorder
        .session_for("conversation-a", 100)
        .unwrap()
        .unwrap();
    let text = json!({"text":"done","truncated":false,"chars":4});
    recorder.observe("conversation-a",&[json!({"kind":"turn_start","time":100,"source":"extension","turnId":"turn-a"}),json!({"kind":"assistant_message","time":110,"source":"extension","messageId":"assistant-a","message":text,"turnId":"turn-a","state":"final","final":true})],110).unwrap();
    let before = store.read(&id, ReadOptions::default()).unwrap();
    assert_eq!(before.iter().filter(|e| e["kind"] == "turn_end").count(), 1);
    store.flush().unwrap();
    drop(recorder);
    let recorder = Recorder::new(store.clone(), registry);
    recorder.observe("conversation-a",&[json!({"kind":"turn_start","time":120,"source":"extension","turnId":"turn-a"}),json!({"kind":"assistant_message","time":120,"source":"extension","messageId":"assistant-a","message":text,"turnId":"turn-a","state":"final","final":true})],120).unwrap();
    assert_eq!(
        store
            .read(&id, ReadOptions::default())
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "turn_end")
            .count(),
        1
    );
    let call = recorder
        .record_call(
            "unknown-request",
            "read",
            json!({"path":"/project/a"}),
            "public result",
            130,
            "ok",
        )
        .unwrap();
    assert_ne!(call["sessionId"], id);
    assert_eq!(call["call"]["attributionMethod"], "unattributed");
    assert_eq!(model::event_tokens(&call["event"]), 11);
}
#[test]
fn search_export_and_estimates_preserve_body_and_model_attribution() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path()).unwrap();
    store
        .create(CreateOptions {
            id: Some("session-a".into()),
            conversation_id: Some("conversation-a".into()),
            ..Default::default()
        })
        .unwrap();
    let text = store
        .store_text("session-a", "alpha public 😀 omega", 5)
        .unwrap();
    store.upsert("session-a",json!({"kind":"user_message","time":1792108800000i64,"source":"extension","messageId":"u","message":text,"model":"gpt-6","reasoningEffort":"high"}),Default::default()).unwrap();
    let found = store.search("public omega", 50).unwrap();
    assert_eq!(found["results"].as_array().unwrap().len(), 1);
    assert!(
        store
            .export_markdown("session-a")
            .unwrap()
            .contains("alpha public 😀 omega")
    );
    store.append("session-a",json!({"kind":"tool_call","source":"mcp","time":1792108800001i64,"call":{"callId":"usage-call","tool":"read","conversationId":"conversation-a","args":{"text":"{}","truncated":false,"chars":2},"result":{"text":"ok","truncated":false,"chars":2},"summary":{"title":"read"},"outcome":"ok"}})).unwrap();
    let overview = usage::overview(&store).unwrap();
    assert!(overview["estimated"].as_bool().unwrap());
    assert_eq!(overview["models"][0]["model"], "gpt-6");
    assert!(overview.get("actualBilling").is_none());
}
