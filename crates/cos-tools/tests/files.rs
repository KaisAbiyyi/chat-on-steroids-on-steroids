use cos_core::config::Root;
use cos_tools::{files::Files, patch, text};
use serde_json::Value;
fn root(path: &std::path::Path) -> Vec<Root> {
    vec![Root {
        name: "project".into(),
        path: std::fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }]
}

#[test]
fn search_revocation_during_traversal_discards_collected_results() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..10 {
        std::fs::write(d.path().join(format!("{i}.txt")), "public needle\n").unwrap();
    }
    let calls = std::cell::Cell::new(0);
    let result = cos_tools::search::find_checked(
        &Files::default(),
        &root(d.path()),
        "/project",
        &cos_tools::search::Options {
            content: true,
            ..cos_tools::search::Options::new("needle")
        },
        &|| {
            calls.set(calls.get() + 1);
            if calls.get() >= 3 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "revoked",
                ))
            } else {
                Ok(())
            }
        },
    );
    assert!(
        result.is_err(),
        "search published after its scan authority was revoked"
    );
    assert_eq!(
        result.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

#[test]
fn patch_aggregate_snapshot_budget_rejects_before_any_mutation() {
    let d = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c"] {
        std::fs::File::create(d.path().join(name))
            .unwrap()
            .set_len(12 * 1024 * 1024)
            .unwrap();
    }
    let result = Files::default().apply(&root(d.path()), "/project", "*** Begin Patch\n*** Delete File: a\n*** Delete File: b\n*** Delete File: c\n*** End Patch");
    assert!(
        result.is_err(),
        "unbounded combined snapshots were accepted"
    );
    for name in ["a", "b", "c"] {
        assert_eq!(
            std::fs::metadata(d.path().join(name)).unwrap().len(),
            12 * 1024 * 1024
        );
    }
}

#[test]
fn patch_file_count_budget_rejects_before_creating_files() {
    let d = tempfile::tempdir().unwrap();
    let mut patch = "*** Begin Patch\n".to_owned();
    for i in 0..129 {
        patch.push_str(&format!("*** Add File: f{i}\n+public\n"));
    }
    patch.push_str("*** End Patch");
    assert!(
        Files::default()
            .apply(&root(d.path()), "/project", &patch)
            .is_err()
    );
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
}
#[test]
fn upstream_patch_parser_and_diagnostics() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase4-6/patch-parser.json"
    ))
    .unwrap();
    for c in cases.as_array().unwrap() {
        let r = patch::parse(c["patch"].as_str().unwrap());
        if let Some(error) = c["error"].as_str() {
            assert_eq!(r.unwrap_err(), error);
        } else {
            assert_eq!(r.unwrap(), c["expected"]);
        }
    }
}
#[test]
fn upstream_mixed_line_endings() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase4-6/text-replacements.json"
    ))
    .unwrap();
    for c in cases.as_array().unwrap() {
        assert_eq!(
            serde_json::to_value(text::lines(c["contents"].as_str().unwrap())).unwrap(),
            c["lines"]
        );
        assert_eq!(
            text::replacements(c["contents"].as_str().unwrap(), &c["replacements"]),
            c["expected"]
        );
    }
}
#[test]
fn failed_multifile_patch_changes_nothing() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a"), "one\r\ntwo\r\n").unwrap();
    let f = Files::default();
    assert!(f.apply(&root(d.path()),"/project","*** Begin Patch\n*** Update File: a\n@@\n-one\n+ONE\n*** Update File: missing\n@@\n-x\n+y\n*** End Patch").is_err());
    assert_eq!(
        std::fs::read(d.path().join("a")).unwrap(),
        b"one\r\ntwo\r\n"
    );
}
#[test]
fn encoding_and_stale_revision() {
    let d = tempfile::tempdir().unwrap();
    let bytes = std::iter::once(0xff)
        .chain(std::iter::once(0xfe))
        .chain(
            "hello\r\nworld\r\n"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        )
        .collect::<Vec<_>>();
    std::fs::write(d.path().join("a"), &bytes).unwrap();
    let roots = root(d.path());
    let f = Files::default();
    let read = f.read(&roots, "/project/a", 1, 2, 1024).unwrap();
    assert_eq!(read["text"], "hello\nworld");
    let rev = f.revision(&roots, "/project/a").unwrap();
    f.save(&roots, "/project/a", "changed\r\n", &rev).unwrap();
    let saved = std::fs::read(d.path().join("a")).unwrap();
    assert_eq!(&saved[..2], &[255, 254]);
    assert!(f.save(&roots, "/project/a", "stale", &rev).is_err());
    assert_eq!(std::fs::read(d.path().join("a")).unwrap(), saved);
}
#[test]
fn successful_patch_keeps_bom_and_context_newlines() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a"), b"\xef\xbb\xbfone\r\ntwo\nthree").unwrap();
    let f = Files::default();
    f.apply(
        &root(d.path()),
        "/project",
        "*** Begin Patch\n*** Update File: a\n@@\n one\n-two\n+TWO\n three\n*** End Patch",
    )
    .unwrap();
    assert_eq!(
        std::fs::read(d.path().join("a")).unwrap(),
        b"\xef\xbb\xbfone\r\nTWO\r\nthree\r\n"
    );
}
