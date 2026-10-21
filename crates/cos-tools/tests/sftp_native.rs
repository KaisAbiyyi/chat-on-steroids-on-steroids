use cos_tools::remote::{RemoteFiles, Sftp};
use std::path::Path;
#[test]
fn native_openssh_subsystem_round_trip() {
    let program = if cfg!(windows) {
        "C:/Program Files/Git/usr/lib/ssh/sftp-server.exe"
    } else if Path::new("/usr/lib/openssh/sftp-server").is_file() {
        "/usr/lib/openssh/sftp-server"
    } else {
        "/usr/libexec/sftp-server"
    };
    assert!(
        Path::new(program).is_file(),
        "Native SFTP qualification requires an installed OpenSSH subsystem"
    );
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.txt"), "public original\n").unwrap();
    let environment = if cfg!(windows) {
        std::collections::BTreeMap::from([(
            "PATH".to_string(),
            format!(
                "C:/Program Files/Git/usr/bin;C:/Program Files/Git/mingw64/bin;{}",
                std::env::var("PATH").unwrap_or_default()
            ),
        )])
    } else {
        std::collections::BTreeMap::new()
    };
    let mut client = Sftp::spawn_environment(program, &[], Some(d.path()), &environment).unwrap();
    let root = client.realpath(".").unwrap();
    assert!(client.stat(&root).unwrap()["isDirectory"] == true);
    assert_eq!(
        client.read(&format!("{root}/a.txt"), 1024).unwrap(),
        b"public original\n"
    );
    client
        .write_new(&format!("{root}/b.txt"), b"public replacement\n")
        .unwrap();
    client
        .rename(&format!("{root}/b.txt"), &format!("{root}/a.txt"))
        .unwrap();
    assert_eq!(
        std::fs::read(d.path().join("a.txt")).unwrap(),
        b"public replacement\n"
    );
    assert_eq!(client.list(&root).unwrap().len(), 1);
    assert_eq!(
        client.stat(&format!("{root}/missing")).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(client.read(&format!("{root}/a.txt"), 1).is_err());
    let profile = tempfile::tempdir().unwrap();
    let config = cos_core::persistence::ConfigStore::open(
        profile.path(),
        if cfg!(windows) { "win32" } else { "linux" },
    )
    .unwrap();
    let projects = cos_tools::project::Projects::open(
        cos_core::persistence::DurableStore::open(profile.path()).unwrap(),
    );
    let p = projects
        .add_remote_with(&config, "public-fixture", "~", &mut client)
        .unwrap();
    assert_eq!(p["remote"]["path"], root);
    assert_eq!(
        projects
            .add_remote_with(&config, "public-fixture", "~", &mut client)
            .unwrap()["id"],
        p["id"]
    );
    std::fs::write(d.path().join("large.txt"), "public line\n".repeat(600000)).unwrap();
    let preview = client
        .preview(&format!("{root}/large.txt"), 1, 1, 1024)
        .unwrap();
    assert_eq!(preview["text"], "public line");
    assert!(preview["totalLines"].is_null());
    assert_eq!(preview["more"], true);
    let mut remote = RemoteFiles { sftp: client, root };
    let mut oversized = "*** Begin Patch\n".to_owned();
    for i in 0..129 {
        oversized.push_str(&format!("*** Add File: too-many-{i}\n+public\n"));
    }
    oversized.push_str("*** End Patch");
    assert!(
        remote.apply_patch(&oversized, None).is_err(),
        "remote transaction has no file count budget"
    );
    assert!(!d.path().join("too-many-0").exists());
    for name in ["budget-a", "budget-b", "budget-c"] {
        std::fs::File::create(d.path().join(name))
            .unwrap()
            .set_len(12 * 1024 * 1024)
            .unwrap();
    }
    assert!(remote.apply_patch("*** Begin Patch\n*** Delete File: budget-a\n*** Delete File: budget-b\n*** Delete File: budget-c\n*** End Patch", None).is_err(), "remote snapshots have no aggregate budget");
    for name in ["budget-a", "budget-b", "budget-c"] {
        assert!(d.path().join(name).exists());
        std::fs::remove_file(d.path().join(name)).unwrap();
    }
    let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-public replacement\n+changed\n*** Add File: nested/new.txt\n+new public file\n*** End Patch";
    assert!(remote.apply_patch(patch, Some(1)).is_err());
    assert_eq!(
        std::fs::read(d.path().join("a.txt")).unwrap(),
        b"public replacement\n"
    );
    assert!(!d.path().join("nested").exists());
    remote.apply_patch(patch, None).unwrap();
    assert_eq!(std::fs::read(d.path().join("a.txt")).unwrap(), b"changed\n");
    assert_eq!(
        std::fs::read(d.path().join("nested/new.txt")).unwrap(),
        b"new public file\n"
    );
    use cos_tools::patch::{self, FileUpdateMode, UpdateError};
    std::fs::write(d.path().join("mode.txt"), b"one\r\ntwo\nthree").unwrap();
    let hunks = patch::parse_typed(
        "*** Begin Patch\n*** Update File: mode.txt\n@@\n one\n-two\n+TWO\n three\n*** End Patch",
    )
    .unwrap()
    .hunks;
    remote
        .apply_hunks_checked_mode(hunks, None, FileUpdateMode::NormalizeToLf, &|_| Ok(()))
        .unwrap();
    assert_eq!(
        std::fs::read(d.path().join("mode.txt")).unwrap(),
        b"one\nTWO\nthree\n"
    );
    std::fs::write(d.path().join("mode.txt"), b"one\r\n").unwrap();
    let hunks=patch::parse_typed("*** Begin Patch\n*** Update File: mode.txt\n*** Move to: moved-mode.txt\n@@\n one\n*** End Patch").unwrap().hunks;
    remote
        .apply_hunks_checked_mode(
            hunks,
            None,
            FileUpdateMode::PreserveLineEndings,
            &|permission| {
                if permission == "edit" {
                    Err(std::io::Error::other("Edit disabled"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
    assert_eq!(
        std::fs::read(d.path().join("moved-mode.txt")).unwrap(),
        b"one\r\n"
    );
    std::fs::write(
        d.path().join("diagnostic.txt"),
        b"public unique anchor\noriginal\n",
    )
    .unwrap();
    let hunks=patch::parse_typed("*** Begin Patch\n*** Update File: diagnostic.txt\n@@\n public unique anchor\n-wrong\n+new\n*** End Patch").unwrap().hunks;
    let error = remote
        .apply_hunks_checked_mode(hunks, None, FileUpdateMode::NormalizeToLf, &|_| Ok(()))
        .unwrap_err();
    assert!(
        error
            .get_ref()
            .and_then(|error| error.downcast_ref::<UpdateError>())
            .unwrap()
            .source_context
            .as_ref()
            .unwrap()
            .contains("2\toriginal")
    );
    assert_eq!(
        std::fs::read(d.path().join("diagnostic.txt")).unwrap(),
        b"public unique anchor\noriginal\n"
    );
    // CL-06B: direct SFTP library callers must not bypass delete and move
    // permissions just because the bytes are unchanged.
    std::fs::write(d.path().join("deny-delete.txt"), b"keep me\n").unwrap();
    let delete =
        patch::parse_typed("*** Begin Patch\n*** Delete File: deny-delete.txt\n*** End Patch")
            .unwrap()
            .hunks;
    let denied = remote
        .apply_hunks_checked(delete, None, &|right| {
            if right == "deleteFile" {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "delete disabled",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(
        std::fs::read(d.path().join("deny-delete.txt")).unwrap(),
        b"keep me\n"
    );

    std::fs::write(d.path().join("deny-move.txt"), b"unchanged\n").unwrap();
    let move_hunks = patch::parse_typed(
        "*** Begin Patch\n*** Update File: deny-move.txt\n*** Move to: move-should-not-exist.txt\n@@\n unchanged\n*** End Patch",
    ).unwrap().hunks;
    let denied = remote
        .apply_hunks_checked(move_hunks, None, &|right| {
            if right == "move" {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "move disabled",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(
        std::fs::read(d.path().join("deny-move.txt")).unwrap(),
        b"unchanged\n"
    );
    assert!(!d.path().join("move-should-not-exist.txt").exists());

    // An authorization that disappears after remote staging must be checked
    // again at commit; stage cleanup leaves no published file or partial.
    let create_calls = std::sync::atomic::AtomicUsize::new(0);
    let add = patch::parse_typed(
        "*** Begin Patch\n*** Add File: revoke-at-commit.txt\n+public\n*** End Patch",
    )
    .unwrap()
    .hunks;
    let denied = remote
        .apply_hunks_checked(add, None, &|right| {
            if right == "create"
                && create_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 2
            {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "create revoked",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(!d.path().join("revoke-at-commit.txt").exists());
    assert!(create_calls.load(std::sync::atomic::Ordering::SeqCst) >= 3);
    assert!(!std::fs::read_dir(d.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("revoke-at-commit.txt.cos-rust-")
    }));
    let same_path = "*** Begin Patch\n*** Add File: before-invalid-move\n+must not be written\n*** Update File: a.txt\n*** Move to: a.txt\n@@\n changed\n*** End Patch";
    let error = remote
        .apply_patch(same_path, None)
        .expect_err("A move to the same SFTP path was accepted");
    assert!(
        error
            .to_string()
            .contains("move source and destination resolve to the same file"),
        "{error}"
    );
    assert!(!d.path().join("before-invalid-move").exists());
    assert_eq!(std::fs::read(d.path().join("a.txt")).unwrap(), b"changed\n");
    #[cfg(unix)]
    {
        remote.apply_patch("*** Begin Patch\n*** Update File: a.txt\n*** Move to: A.txt\n@@\n changed\n*** End Patch",None).unwrap();
        assert!(!d.path().join("a.txt").exists());
        assert_eq!(std::fs::read(d.path().join("A.txt")).unwrap(), b"changed\n");
    }
}
