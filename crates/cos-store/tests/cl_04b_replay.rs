use cos_store::{
    CreateOptions, ReadOptions, SessionStore,
    model,
    store::MAX_HISTORY_SEQUENCE,
};
use serde_json::{Value, json};
use std::{fs, io::Write};

fn setup(root: &std::path::Path, id: &str) -> SessionStore {
    let store = SessionStore::open(root).unwrap();
    store
        .create(CreateOptions {
            id: Some(id.into()),
            conversation_id: Some("conversation-public".into()),
            now: Some(100),
            ..Default::default()
        })
        .unwrap();
    store
}

fn note(seq: u64) -> Value {
    json!({
        "kind":"note",
        "seq":seq,
        "time":100,
        "source":"app",
        "message":{"text":"public fixture","truncated":false,"chars":14}
    })
}

#[test]
fn replay_fails_closed_on_nonrepresentable_journal_cursor_without_touching_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let id = "session-sequence-corrupt";
    let store = setup(temp.path(), id);
    drop(store);
    let journal = temp.path().join("sessions").join(id).join("events.jsonl");
    let bytes = format!("{}\n", note(MAX_HISTORY_SEQUENCE + 1)).into_bytes();
    fs::write(&journal, &bytes).unwrap();
    let restored = SessionStore::open(temp.path()).unwrap();
    assert!(restored.summary(id).is_err());
    assert_eq!(fs::read(&journal).unwrap(), bytes);
    assert!(SessionStore::open(temp.path()).unwrap().summary(id).is_err());
}

#[test]
fn max_safe_journal_cursor_replays_but_never_advances_into_unsafe_range() {
    let temp = tempfile::tempdir().unwrap();
    let id = "session-sequence-last";
    let store = setup(temp.path(), id);
    drop(store);
    let journal = temp.path().join("sessions").join(id).join("events.jsonl");
    let bytes = format!("{}\n", note(MAX_HISTORY_SEQUENCE)).into_bytes();
    fs::write(&journal, &bytes).unwrap();
    let restored = SessionStore::open(temp.path()).unwrap();
    assert_eq!(restored.read(id, ReadOptions::default()).unwrap().len(), 1);
    assert!(restored.append(id, note(0)).is_err());
    assert_eq!(fs::read(&journal).unwrap(), bytes);
    drop(restored);
    assert_eq!(SessionStore::open(temp.path()).unwrap().read(id, ReadOptions::default()).unwrap().len(), 1);
}

#[test]
fn replay_refuses_unsafe_canonical_shard_without_discarding_stored_history() {
    let temp = tempfile::tempdir().unwrap();
    let id = "session-shard-corrupt";
    let store = setup(temp.path(), id);
    store
        .upsert(
            id,
            json!({
                "kind":"user_message",
                "messageId":"public-user",
                "time":200,
                "source":"extension",
                "message":{"text":"hello","truncated":false,"chars":5}
            }),
            Default::default(),
        )
        .unwrap();
    store.flush().unwrap();
    drop(store);
    let key = "user_message\0public-user";
    let shard = temp
        .path()
        .join("sessions")
        .join(id)
        .join("messages")
        .join(format!("{}.json", model::hash(key.as_bytes())));
    let mut event: Value = serde_json::from_slice(&fs::read(&shard).unwrap()).unwrap();
    event["seq"] = json!(MAX_HISTORY_SEQUENCE + 1);
    let corrupted = serde_json::to_vec(&event).unwrap();
    fs::write(&shard, &corrupted).unwrap();
    let recovered = SessionStore::open(temp.path()).unwrap();
    assert!(recovered.summary(id).is_err());
    assert_eq!(fs::read(&shard).unwrap(), corrupted);
}

#[test]
fn corrupted_checkpoints_can_replay_valid_journal_without_losing_messages() {
    let temp = tempfile::tempdir().unwrap();
    let id = "session-rebuild-guard";
    let store = setup(temp.path(), id);
    store.append(id, note(0)).unwrap();
    store.flush().unwrap();
    drop(store);
    let base = temp.path().join("sessions").join(id);
    let primary = base.join("meta.json");
    let backup = base.join("meta.backup.json");
    fs::write(&primary, b"{corrupt primary").unwrap();
    fs::write(&backup, b"{corrupt backup").unwrap();
    let recovered = SessionStore::open(temp.path()).unwrap();
    let before = recovered.summary(id).unwrap().unwrap();
    assert_eq!(before["events"], 1);
    assert_eq!(recovered.read(id, ReadOptions::default()).unwrap().len(), 1);
    drop(recovered);
    let again = SessionStore::open(temp.path()).unwrap();
    assert_eq!(again.summary(id).unwrap().unwrap(), before);
    assert_eq!(again.read(id, ReadOptions::default()).unwrap().len(), 1);
}

#[test]
fn interrupted_tail_replays_once_and_retention_never_deletes_original_journal() {
    let temp = tempfile::tempdir().unwrap();
    let id = "session-torn-journal";
    let store = setup(temp.path(), id);
    store.append(id, note(0)).unwrap();
    store.flush().unwrap();
    drop(store);
    let journal = temp.path().join("sessions").join(id).join("events.jsonl");
    fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap()
        .write_all(b"{\"seq\":2,\"kind\":")
        .unwrap();
    let recovered = SessionStore::open(temp.path()).unwrap();
    assert_eq!(recovered.append(id, note(0)).unwrap()["seq"], 2);
    recovered.flush().unwrap();
    let before = fs::read(&journal).unwrap();
    assert_eq!(recovered.prune(0), 0);
    assert_eq!(recovered.prune(365), 0);
    assert_eq!(fs::read(&journal).unwrap(), before);
    drop(recovered);
    let again = SessionStore::open(temp.path()).unwrap();
    assert_eq!(again.read(id, ReadOptions::default()).unwrap().len(), 2);
    assert_eq!(again.summary(id).unwrap().unwrap()["events"], 2);
}
