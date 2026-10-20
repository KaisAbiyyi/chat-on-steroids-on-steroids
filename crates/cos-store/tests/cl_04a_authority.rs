use cos_core::persistence::DurableStore;
use cos_store::{
    CreateOptions, SessionStore,
    correlation::{Correlation, Correlations},
    title::TitleSource,
};
use serde_json::json;

fn create(store: &SessionStore, id: &str, conversation: &str) {
    store
        .create(CreateOptions {
            id: Some(id.into()),
            conversation_id: Some(conversation.into()),
            now: Some(100),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn non_uuid_conversation_identity_is_unique_across_active_and_retired_epochs() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::open(temp.path()).unwrap();
    create(&store, "session-alpha", "conversation-alpha");
    create(&store, "session-beta", "conversation-beta");
    assert!(
        store
            .create(CreateOptions {
                id: Some("session-duplicate".into()),
                conversation_id: Some("conversation-alpha".into()),
                ..Default::default()
            })
            .is_err()
    );
    assert!(
        !store
            .rebind(
                "session-beta",
                Some("conversation-beta"),
                "conversation-alpha",
                None,
                110
            )
            .unwrap()
    );
    assert!(
        store
            .rebind(
                "session-alpha",
                Some("conversation-alpha"),
                "conversation-gamma",
                None,
                120
            )
            .unwrap()
    );
    assert!(
        !store
            .rebind(
                "session-beta",
                Some("conversation-beta"),
                "conversation-alpha",
                None,
                130
            )
            .unwrap()
    );
    assert!(
        store
            .create(CreateOptions {
                id: Some("session-retired-duplicate".into()),
                conversation_id: Some("conversation-alpha".into()),
                ..Default::default()
            })
            .is_err()
    );
    drop(store);
    let reopened = SessionStore::open(temp.path()).unwrap();
    assert_eq!(
        reopened
            .find_conversation("conversation-gamma")
            .unwrap()
            .unwrap()["chatIds"],
        json!(["conversation-alpha", "conversation-gamma"])
    );
    assert!(
        !reopened
            .rebind(
                "session-beta",
                Some("conversation-beta"),
                "conversation-alpha",
                None,
                140
            )
            .unwrap()
    );
}

#[test]
fn rebinding_fences_stale_manual_titles_models_and_request_correlation_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::open(temp.path()).unwrap();
    let durable = DurableStore::open(temp.path()).unwrap();
    let registry = Correlations::restore(durable.clone(), &store).unwrap();
    let id = "session-owner";
    create(&store, id, "conversation-old");
    let owner = Correlation {
        request_id: "request-public".into(),
        conversation_id: "conversation-old".into(),
        session_id: id.into(),
        message_id: "message-public".into(),
        tool: "read".into(),
        observed_at: 10,
    };
    assert_eq!(
        registry
            .observe_and_commit(std::slice::from_ref(&owner))
            .unwrap(),
        ["stored"]
    );
    store
        .rename(
            id,
            "Manual first",
            TitleSource::Manual,
            Some("conversation-old"),
        )
        .unwrap();
    assert!(
        store
            .observe_model(id, "conversation-old", "gpt-6", 20.0, Some("high"))
            .unwrap()
    );
    assert!(
        store
            .rebind(
                id,
                Some("conversation-old"),
                "conversation-new",
                Some("handoff-public"),
                100
            )
            .unwrap()
    );
    let before = store.summary(id).unwrap().unwrap();
    assert!(
        !store
            .observe_model(id, "conversation-old", "gpt-5", 200.0, None)
            .unwrap()
    );
    assert_eq!(
        registry
            .observe_and_commit(&[Correlation {
                conversation_id: "conversation-new".into(),
                session_id: "session-other".into(),
                ..owner
            }])
            .unwrap(),
        ["refused"]
    );
    store
        .rename(
            id,
            "Stale manual",
            TitleSource::Manual,
            Some("conversation-old"),
        )
        .unwrap();
    store
        .rename(
            id,
            "Stale provider",
            TitleSource::Provider,
            Some("conversation-old"),
        )
        .unwrap();
    assert_eq!(store.summary(id).unwrap().unwrap(), before);
    assert!(
        store
            .observe_model(id, "conversation-new", "gpt-6-sol", 30.0, Some("low"))
            .unwrap()
    );
    store
        .rename(
            id,
            "Current manual",
            TitleSource::Manual,
            Some("conversation-new"),
        )
        .unwrap();
    drop(registry);
    drop(store);
    let reopened = SessionStore::open(temp.path()).unwrap();
    let restored = Correlations::restore(durable, &reopened).unwrap();
    assert_eq!(
        restored
            .get("request-public")
            .unwrap()
            .unwrap()
            .conversation_id,
        "conversation-old"
    );
    let summary = reopened.summary(id).unwrap().unwrap();
    assert_eq!(summary["title"], "Current manual");
    assert_eq!(summary["titleSource"], "manual");
    assert_eq!(summary["selectedModel"]["model"], "gpt-6-sol");
    assert_eq!(
        summary["selectedModel"]["conversationId"],
        "conversation-new"
    );
    assert_eq!(summary["lastCommittedResumeHandoffId"], "handoff-public");
}

#[test]
fn reactivating_an_owned_retired_alias_is_consistent_before_and_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::open(temp.path()).unwrap();
    create(&store, "session-roundtrip", "conversation-original");
    assert!(
        store
            .rebind(
                "session-roundtrip",
                Some("conversation-original"),
                "conversation-next",
                None,
                110
            )
            .unwrap()
    );
    assert!(
        store
            .rebind(
                "session-roundtrip",
                Some("conversation-next"),
                "conversation-original",
                None,
                120
            )
            .unwrap()
    );
    let before = store.summary("session-roundtrip").unwrap().unwrap();
    assert_eq!(before["conversationId"], "conversation-original");
    assert_eq!(before["retiredChatAt"]["conversation-original"], serde_json::Value::Null);
    assert_eq!(before["retiredChatAt"]["conversation-next"], 120);
    drop(store);
    let after = SessionStore::open(temp.path())
        .unwrap()
        .summary("session-roundtrip")
        .unwrap()
        .unwrap();
    assert_eq!(after, before);
}

#[test]
fn failed_rebind_keeps_last_committed_epoch_on_disk_and_in_memory() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::open(temp.path()).unwrap();
    create(&store, "session-failure", "conversation-old");
    let original = store.summary("session-failure").unwrap().unwrap();
    let meta = temp.path().join("sessions/session-failure/meta.json");
    let bytes = std::fs::read(&meta).unwrap();
    std::fs::remove_file(&meta).unwrap();
    std::fs::create_dir(&meta).unwrap();
    assert!(
        !store
            .rebind(
                "session-failure",
                Some("conversation-old"),
                "conversation-new",
                None,
                200
            )
            .unwrap()
    );
    assert_eq!(store.summary("session-failure").unwrap().unwrap(), original);
    std::fs::remove_dir(&meta).unwrap();
    std::fs::write(&meta, bytes).unwrap();
    drop(store);
    let reopened = SessionStore::open(temp.path()).unwrap();
    assert_eq!(
        reopened.summary("session-failure").unwrap().unwrap(),
        original
    );
}
