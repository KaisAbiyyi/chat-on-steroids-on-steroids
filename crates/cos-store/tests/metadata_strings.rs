use base64::{Engine, engine::general_purpose::STANDARD};
use cos_store::{CreateOptions, SessionStore, title::TitleSource};
use serde_json::Value;

#[test]
fn unpaired_metadata_is_refused_without_overwriting_the_primary_or_valid_backup() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-metadata.json"
    ))
    .unwrap();
    for case in fixture["losslessCases"].as_array().unwrap() {
        let directory = tempfile::tempdir().unwrap();
        let id = fixture["id"].as_str().unwrap();
        let store = SessionStore::open(directory.path()).unwrap();
        store
            .create(CreateOptions {
                id: Some(id.into()),
                ..Default::default()
            })
            .unwrap();
        store
            .rename(id, "Public backup", TitleSource::Manual, None)
            .unwrap();
        drop(store);
        let session = directory.path().join("sessions").join(id);
        let meta = session.join("meta.json");
        let backup = session.join("meta.backup.json");
        let backup_before = std::fs::read(&backup).unwrap();
        let bytes = case["raw"].as_str().unwrap().as_bytes();
        std::fs::write(&meta, bytes).unwrap();
        let store = SessionStore::open(directory.path()).unwrap();
        assert_eq!(
            case["expectedTitleUnits"], case["storeTitleUnits"],
            "actual source must accept this input"
        );
        if case["needsLosslessString"] == true {
            assert_eq!(
                store.summary(id).unwrap_err().kind(),
                std::io::ErrorKind::Unsupported
            );
        } else {
            let summary = store.summary(id).unwrap().unwrap();
            assert_eq!(
                serde_json::to_value(
                    summary["title"]
                        .as_str()
                        .unwrap()
                        .encode_utf16()
                        .collect::<Vec<_>>()
                )
                .unwrap(),
                case["storeTitleUnits"]
            );
        }
        assert_eq!(
            std::fs::read(&meta).unwrap(),
            bytes,
            "read must preserve the original metadata bytes"
        );
        assert_eq!(std::fs::read(&backup).unwrap(), backup_before);
    }
}
#[test]
fn metadata_file_utf8_decoding_matches_the_actual_source_before_json_parsing() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-metadata.json"
    ))
    .unwrap();
    for case in fixture["encodedCases"].as_array().unwrap() {
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
        let bytes = STANDARD.decode(case["rawBytes"].as_str().unwrap()).unwrap();
        std::fs::write(&meta, &bytes).unwrap();
        let store = SessionStore::open(directory.path()).unwrap();
        assert_eq!(
            serde_json::to_value(store.summary(id).unwrap()).unwrap(),
            case["storeExpected"]
        );
        assert_eq!(std::fs::read(meta).unwrap(), bytes);
    }
}
