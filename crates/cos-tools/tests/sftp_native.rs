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
}
