use cos_store::{CreateOptions, ReadOptions, SessionStore};
use serde_json::{Value, json};
use std::{fs, io};

fn session(dir: &std::path::Path, id: &str) -> SessionStore {
    let store = SessionStore::open(dir).unwrap();
    store
        .create(CreateOptions {
            id: Some(id.into()),
            now: Some(100),
            ..Default::default()
        })
        .unwrap();
    store
}

fn unsupported(error: io::Error) {
    assert_eq!(error.kind(), io::ErrorKind::Unsupported, "{error}");
    assert!(error.to_string().contains("UTF-16"), "{error}");
}

#[test]
fn metadata_refuses_nested_unpaired_units_but_keeps_original_and_backup() {
    let dir = tempfile::tempdir().unwrap();
    let id = "session-meta-lossless";
    let store = session(dir.path(), id);
    store.flush().unwrap();
    drop(store);
    let root = dir.path().join("sessions").join(id);
    let meta = root.join("meta.json");
    let backup = root.join("meta.backup.json");
    let original = fs::read_to_string(&meta).unwrap();
    let prior_backup = fs::read(&backup).ok();

    for suffix in [r"\uD800", r"\uDFFF", r"\uD800\u0041", r"\uD800\\uDC00"] {
        let modified = original.replacen(
            "\"id\":",
            &format!("\"nested\":{{\"unexpected\":\"{suffix}\"}},\"id\":"),
            1,
        );
        assert_ne!(modified, original);
        fs::write(&meta, modified.as_bytes()).unwrap();
        let reopened = SessionStore::open(dir.path()).unwrap();
        unsupported(reopened.summary(id).unwrap_err());
        assert_eq!(fs::read(&meta).unwrap(), modified.as_bytes());
        assert_eq!(fs::read(&backup).ok(), prior_backup);
    }
}

#[test]
fn escaped_literal_backslashes_and_valid_surrogate_pairs_are_lossless() {
    let dir = tempfile::tempdir().unwrap();
    let id = "session-paired-unicode";
    let store = session(dir.path(), id);
    drop(store);
    let meta = dir.path().join("sessions").join(id).join("meta.json");
    let original = fs::read_to_string(&meta).unwrap();
    // First escape is a literal slash-u sequence; second is a valid surrogate pair.
    let modified = original.replacen(
        "\"id\":",
        r#""edge":{"literal":"\\uD800","emoji":"\uD83D\uDE00","text":"é 中 😀"},"id":"#,
        1,
    );
    fs::write(&meta, modified.as_bytes()).unwrap();
    let reopened = SessionStore::open(dir.path()).unwrap();
    let summary = reopened.summary(id).unwrap().unwrap();
    assert_eq!(summary["edge"]["literal"], r"\uD800");
    assert_eq!(summary["edge"]["emoji"], "😀");
    assert_eq!(summary["edge"]["text"], "é 中 😀");
    assert_eq!(fs::read(&meta).unwrap(), modified.as_bytes());
}

#[test]
fn journal_refuses_nonrepresentable_utf16_without_skipping_or_overwriting_original() {
    let dir = tempfile::tempdir().unwrap();
    let id = "session-journal-lossless";
    let store = session(dir.path(), id);
    drop(store);
    let journal = dir.path().join("sessions").join(id).join("events.jsonl");
    let bad = br#"{"kind":"note","seq":1,"time":20,"source":"app","message":{"text":"\uD800","truncated":false,"chars":1}}"#;
    fs::write(&journal, [bad.as_slice(), b"\n"].concat()).unwrap();
    let original = fs::read(&journal).unwrap();
    let reopened = SessionStore::open(dir.path()).unwrap();
    unsupported(reopened.summary(id).unwrap_err());
    assert_eq!(fs::read(&journal).unwrap(), original);

    let paired = br#"{"kind":"note","seq":1,"time":20,"source":"app","message":{"text":"\uD83D\uDE00","truncated":false,"chars":2}}"#;
    fs::write(&journal, [paired.as_slice(), b"\n"].concat()).unwrap();
    let good = SessionStore::open(dir.path()).unwrap();
    assert_eq!(
        good.read(id, ReadOptions::default()).unwrap()[0]["message"]["text"],
        "😀"
    );
    assert_eq!(good.summary(id).unwrap().unwrap()["events"], 1);
}

#[test]
fn unsafe_canonical_shard_and_legacy_map_fail_closed_without_dropping_messages() {
    let dir = tempfile::tempdir().unwrap();
    let id = "session-shard-lossless";
    let store = session(dir.path(), id);
    store
        .upsert(
            id,
            json!({"kind":"user_message","source":"extension","time":11,
                "messageId":"public-user","message":{"text":"hello","truncated":false,"chars":5}}),
            Default::default(),
        )
        .unwrap();
    store.flush().unwrap();
    drop(store);

    let base = dir.path().join("sessions").join(id);
    let message_dir = base.join("messages");
    let entry = fs::read_dir(&message_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original_shard = fs::read(&entry).unwrap();
    let bad_shard = String::from_utf8(original_shard.clone())
        .unwrap()
        .replace("\"hello\"", r#""\uDFFF""#);
    assert_ne!(bad_shard.as_bytes(), original_shard);
    fs::write(&entry, bad_shard.as_bytes()).unwrap();
    unsupported(
        SessionStore::open(dir.path())
            .unwrap()
            .summary(id)
            .unwrap_err(),
    );
    assert_eq!(fs::read(&entry).unwrap(), bad_shard.as_bytes());

    fs::write(&entry, &original_shard).unwrap();
    let event: Value = serde_json::from_slice(&original_shard).unwrap();
    let mut map = serde_json::Map::new();
    map.insert("user_message\0public-user".into(), event);
    let original_map = serde_json::to_string(&map).unwrap();
    let bad_map = original_map.replace("\"hello\"", r#""\uD800""#);
    assert_ne!(original_map, bad_map);
    let legacy = base.join("messages.json");
    fs::write(&legacy, bad_map.as_bytes()).unwrap();
    unsupported(
        SessionStore::open(dir.path())
            .unwrap()
            .summary(id)
            .unwrap_err(),
    );
    assert_eq!(fs::read(&legacy).unwrap(), bad_map.as_bytes());
    assert_eq!(fs::read(&entry).unwrap(), original_shard);
}

#[test]
fn traces_and_history_documents_cannot_silently_replace_unrepresentable_content() {
    let dir = tempfile::tempdir().unwrap();
    let id = "session-doc-lossless";
    let store = session(dir.path(), id);
    let base = dir.path().join("sessions").join(id);
    let traces = base.join("traces");
    fs::create_dir_all(&traces).unwrap();
    let trace = traces.join("turn-public.json");
    let bad_trace = br#"[{"kind":"say","text":"\uD800"}]"#;
    fs::write(&trace, bad_trace).unwrap();
    unsupported(store.read_trace(id, "turn-public").unwrap_err());
    unsupported(
        store
            .write_trace(
                id,
                "turn-public",
                &json!([{"kind":"say","text":"safe new text"}]),
                None,
            )
            .unwrap_err(),
    );
    assert_eq!(fs::read(&trace).unwrap(), bad_trace);

    let handoffs = base.join("handoffs");
    fs::create_dir_all(&handoffs).unwrap();
    let handoff = handoffs.join("handoff-public.json");
    let bad_doc = br#"{"id":"handoff-public","text":"\uDFFF"}"#;
    fs::write(&handoff, bad_doc).unwrap();
    unsupported(
        store
            .read_document(id, "handoffs", "handoff-public")
            .unwrap_err(),
    );
    assert_eq!(fs::read(&handoff).unwrap(), bad_doc);
}
