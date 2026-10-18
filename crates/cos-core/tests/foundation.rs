use cos_core::{
    config::{self, Root},
    persistence::{ConfigStore, DurableStore, InstanceLease},
    sandbox,
};
use serde_json::{Value, json};
use std::{fs, sync::Arc};

#[test]
fn corrupted_settings_preserve_original_and_do_not_grant_fresh_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let original = b"{ damaged private settings";
    fs::write(dir.path().join("config.json"), original).unwrap();
    let store = ConfigStore::open(dir.path(), "win32").unwrap();
    assert!(store.recovered());
    assert_eq!(store.get()["readOnly"], true);
    assert_eq!(store.get()["capabilities"]["command"], false);
    let backup = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("unreadable-")
        })
        .unwrap();
    assert_eq!(fs::read(backup).unwrap(), original);
}
#[test]
fn bom_is_accepted_and_unknown_ui_choice_keeps_existing_roots() {
    let dir = tempfile::tempdir().unwrap();
    let mut input = cos_core::fresh_config("win32");
    input["roots"] = json!([{"name":"project","path":"C:\\project"}]);
    input["ui"]["chatBrowser"] = json!("future");
    fs::write(dir.path().join("config.json"), format!("\u{feff}{}", input)).unwrap();
    let store = ConfigStore::open(dir.path(), "win32").unwrap();
    assert!(!store.recovered());
    assert_eq!(store.get()["roots"], input["roots"]);
    assert_eq!(store.get()["ui"]["chatBrowser"], "chrome");
}
#[test]
fn failed_config_commit_does_not_publish_unsaved_permission() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigStore::open(dir.path(), "win32").unwrap();
    store
        .update(|v| v["capabilities"]["command"] = json!(false))
        .unwrap();
    fs::remove_file(dir.path().join("config.json")).unwrap();
    fs::create_dir(dir.path().join("config.json")).unwrap();
    assert!(
        store
            .update(|v| v["capabilities"]["command"] = json!(true))
            .is_err()
    );
    assert_eq!(store.get()["capabilities"]["command"], false);
}
#[test]
fn concurrent_config_updates_compose_from_latest_committed_state() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(ConfigStore::open(dir.path(), "win32").unwrap());
    std::thread::scope(|s| {
        let a = store.clone();
        s.spawn(move || a.update(|v| v["ui"]["theme"] = json!("light")).unwrap());
        let b = store.clone();
        s.spawn(move || b.update(|v| v["ui"]["keepAwake"] = json!(false)).unwrap());
    });
    let restored = ConfigStore::open(dir.path(), "win32").unwrap().get();
    assert_eq!(restored["ui"]["theme"], "light");
    assert_eq!(restored["ui"]["keepAwake"], false);
}
#[test]
fn readonly_and_platform_projection_cannot_widen_grants() {
    let mut config = cos_core::fresh_config("win32");
    config["readOnly"] = json!(true);
    let caps = config::effective_capabilities(&config, "linux");
    for name in config::WRITE_CAPABILITIES {
        assert_eq!(caps[name], false, "{name}");
    }
    assert_eq!(caps["read"], true);
    assert_eq!(caps["clipboardRead"], false);
    assert_eq!(caps["screen"], true);
}
#[test]
fn durable_coalescing_flush_and_null_removal_restore_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let store = DurableStore::open(dir.path()).unwrap();
    store.queue("intent", json!({"generation":1})).unwrap();
    store.queue("intent", json!({"generation":2})).unwrap();
    store.flush().unwrap();
    assert_eq!(
        store.read("intent", true).unwrap(),
        Some(json!({"generation":2}))
    );
    assert!(store.revision("intent") > 0);
    store.write_now("intent", Value::Null).unwrap();
    assert_eq!(store.read("intent", true).unwrap(), None);
}
#[test]
fn invalid_authority_file_is_not_an_empty_strict_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let store = DurableStore::open(dir.path()).unwrap();
    fs::write(dir.path().join("state/authority.json"), "{oops").unwrap();
    assert!(store.read("authority", true).is_err());
    assert_eq!(store.read("authority", false).unwrap(), None);
    assert!(store.write_now("../escape", json!(true)).is_err());
}
#[test]
fn second_runtime_cannot_take_the_same_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let first = InstanceLease::acquire(dir.path()).unwrap();
    assert!(InstanceLease::acquire(dir.path()).is_err());
    drop(first);
    assert!(InstanceLease::acquire(dir.path()).is_ok());
}
#[test]
fn approved_virtual_native_relative_and_missing_paths_share_one_boundary() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/file.txt"), "x").unwrap();
    let roots = vec![Root {
        name: "project".into(),
        path: fs::canonicalize(dir.path())
            .unwrap()
            .to_string_lossy()
            .into(),
        remote: None,
    }];
    let virtual_path = sandbox::resolve(&roots, "/project/src/file.txt", None, false).unwrap();
    assert_eq!(
        virtual_path.real,
        fs::canonicalize(dir.path().join("src/file.txt")).unwrap()
    );
    assert_eq!(
        sandbox::resolve(&roots, "file.txt", Some("/project/src"), false)
            .unwrap()
            .real,
        virtual_path.real
    );
    assert_eq!(
        sandbox::resolve(
            &roots,
            &fs::canonicalize(dir.path().join("src/file.txt"))
                .unwrap()
                .to_string_lossy(),
            None,
            false
        )
        .unwrap()
        .virtual_path,
        "/project/src/file.txt"
    );
    assert!(sandbox::resolve(&roots, "/project/../escape", None, true).is_err());
    assert!(
        sandbox::resolve(&roots, "/project/new/file.txt", None, true)
            .unwrap()
            .real
            .ends_with("new/file.txt")
    );
    assert!(sandbox::resolve(&roots, "/project/new/file.txt", None, false).is_err());
}
#[cfg(windows)]
#[test]
fn windows_devices_ads_and_aliased_segments_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let roots = vec![Root {
        name: "project".into(),
        path: fs::canonicalize(dir.path())
            .unwrap()
            .to_string_lossy()
            .into(),
        remote: None,
    }];
    for path in [
        "/project/CON.txt",
        "/project/file:stream",
        "/project/name.",
        "/project/name ",
        "/project/a?b",
    ] {
        assert!(
            sandbox::resolve(&roots, path, None, true).is_err(),
            "{path}"
        );
    }
}
