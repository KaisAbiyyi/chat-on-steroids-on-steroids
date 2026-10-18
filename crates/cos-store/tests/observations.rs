use cos_core::persistence::DurableStore;
use cos_store::{
    ReadOptions, SessionStore, correlation::Correlations, recorder::Recorder, title::TitleSource,
};
use serde_json::json;
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};

fn recorder(directory: &std::path::Path, store: &SessionStore) -> Recorder {
    let correlations =
        Arc::new(Correlations::restore(DurableStore::open(directory).unwrap(), store).unwrap());
    Recorder::new(store.clone(), correlations)
}
#[test]
fn native_title_and_model_observations_keep_manual_and_conversation_authority() {
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path()).unwrap();
    let recorder = Arc::new(recorder(directory.path(), &store));
    let id = recorder
        .session_for("conversation-public", 100)
        .unwrap()
        .unwrap();
    store
        .rename(&id, "Manual public", TitleSource::Manual, None)
        .unwrap();
    let observed = recorder.observe("conversation-public", &[
        json!({"kind":"model_selection","model":"gpt-6","reasoningEffort":"high","time":120}),
        json!({"kind":"conversation_title","text":"  Provider public  ","time":110}),
    ], 130).unwrap().unwrap();
    assert_eq!(observed, id);
    let summary = store.summary(&id).unwrap().unwrap();
    assert_eq!(summary["title"], "Manual public");
    assert_eq!(
        summary["autoTitle"],
        json!({"title":"Provider public","source":"provider"})
    );
    assert_eq!(
        summary["selectedModel"],
        json!({"conversationId":"conversation-public","model":"gpt-6","reasoningEffort":"high","observedAt":120})
    );
    assert!(
        store.read(&id, ReadOptions::default()).unwrap().is_empty(),
        "Presentation observations are not history events"
    );
    recorder
        .observe(
            "conversation-public",
            &[json!({"kind":"model_selection","model":"gpt-5","time":100})],
            140,
        )
        .unwrap();
    assert_eq!(
        store.summary(&id).unwrap().unwrap()["selectedModel"],
        summary["selectedModel"]
    );
    store
        .rebind(
            &id,
            Some("conversation-public"),
            "conversation-new",
            None,
            150,
        )
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let retired_recorder = recorder.clone();
    let job = std::thread::spawn(move || {
        sender
            .send(retired_recorder.observe(
                "conversation-public",
                &[json!({"kind":"model_selection","model":"gpt-5","time":1000})],
                1001,
            ))
            .unwrap();
    });
    assert!(
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("Retired binding lookup must settle without recursive locking")
            .unwrap()
            .is_none()
    );
    job.join().unwrap();
    assert_eq!(
        store.summary(&id).unwrap().unwrap()["selectedModel"],
        summary["selectedModel"]
    );
    recorder.set_enabled(false);
    assert!(
        recorder
            .observe(
                "conversation-new",
                &[json!({"kind":"model_selection","model":"gpt-5","time":1000})],
                1001
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.summary(&id).unwrap().unwrap()["selectedModel"],
        summary["selectedModel"]
    );
}
#[test]
fn raw_native_request_observations_use_source_framing_and_keep_authored_time() {
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path()).unwrap();
    let recorder = recorder(directory.path(), &store);
    let text = cos_store::title::prepend_user_prompt("public\n  request", "instructions");
    let id = recorder
        .observe(
            "conversation-public",
            &[
                json!({"kind":"user_message","text":text,"messageId":"public-user","time":110}),
                json!({"kind":"conversation_title","text":"ChatGPT - Public project","time":100}),
            ],
            120,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        store.summary(&id).unwrap().unwrap()["title"],
        "public\n  request"
    );
    recorder.observe("conversation-public", &[json!({"kind":"user_message","text":text,"messageId":"public-user","authoredTime":true,"authoredAt":80,"time":80})], 130).unwrap();
    let events = store.read(&id, ReadOptions::default()).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["message"]["text"], text);
    assert_eq!(events[0]["time"], 80);
    assert_eq!(events[0]["authoredAt"], 80);
}
#[test]
fn persistent_model_selection_matches_source_stale_time_epoch_and_restart_rules() {
    use cos_store::CreateOptions;
    use serde_json::Value;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-model-selection.json"
    ))
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut store = SessionStore::open(directory.path()).unwrap();
    let id = fixture["id"].as_str().unwrap();
    store
        .create(CreateOptions {
            id: Some(id.into()),
            conversation_id: Some("conversation-public".into()),
            now: Some(1792108800000),
            ..Default::default()
        })
        .unwrap();
    for step in fixture["selection"].as_array().unwrap() {
        let operation = &step["operation"];
        let input = &operation[1];
        match operation[0].as_str().unwrap() {
            "observe" => {
                let time = match input["nonFinite"].as_str() {
                    Some("NaN") => f64::NAN,
                    Some("Infinity") => f64::INFINITY,
                    _ => input["observedAt"].as_f64().unwrap(),
                };
                store
                    .observe_model(
                        id,
                        input["conversationId"].as_str().unwrap(),
                        input["model"].as_str().unwrap(),
                        time,
                        input["reasoningEffort"].as_str(),
                    )
                    .unwrap();
            }
            "rebind" => {
                assert!(
                    store
                        .rebind(
                            id,
                            input["from"].as_str(),
                            input["to"].as_str().unwrap(),
                            None,
                            1792108800000
                        )
                        .unwrap()
                );
            }
            "restart" => {
                store.flush().unwrap();
                drop(store);
                store = SessionStore::open(directory.path()).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            store.summary(id).unwrap().unwrap(),
            step["expected"],
            "{operation}"
        );
    }
}
#[test]
fn failed_model_selection_write_keeps_the_committed_provider_metadata() {
    use cos_store::CreateOptions;
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path()).unwrap();
    let id = "session-model-commit";
    store
        .create(CreateOptions {
            id: Some(id.into()),
            conversation_id: Some("conversation-public".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(
        store
            .observe_model(id, "conversation-public", "gpt-6", 10.0, None)
            .unwrap()
    );
    let before = store.summary(id).unwrap().unwrap();
    let meta = directory.path().join("sessions").join(id).join("meta.json");
    std::fs::remove_file(&meta).unwrap();
    std::fs::create_dir(meta).unwrap();
    assert!(
        !store
            .observe_model(id, "conversation-public", "gpt-6", 10.0, None)
            .unwrap(),
        "Exact duplicate needs no durable write"
    );
    assert!(
        store
            .observe_model(id, "conversation-public", "gpt-5", 20.0, None)
            .is_err()
    );
    assert_eq!(store.summary(id).unwrap().unwrap(), before);
}
