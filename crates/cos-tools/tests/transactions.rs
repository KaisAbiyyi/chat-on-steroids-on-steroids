use cos_core::config::Root;
use cos_tools::files::Files;
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
fn commit_failure_rolls_back_all_bytes() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a"), "first\r\n").unwrap();
    std::fs::write(d.path().join("b"), "second\n").unwrap();
    let f = Files::default();
    assert!(f.apply_with_failure(&root(d.path()),"/project","*** Begin Patch\n*** Update File: a\n@@\n-first\n+changed\n*** Delete File: b\n*** Add File: new/nested/c\n+created\n*** End Patch",Some(1)).is_err());
    assert_eq!(std::fs::read(d.path().join("a")).unwrap(), b"first\r\n");
    assert_eq!(std::fs::read(d.path().join("b")).unwrap(), b"second\n");
    assert!(!d.path().join("new").exists());
}
#[test]
fn edit_keeps_file_identity() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a"), "first\n").unwrap();
    std::fs::hard_link(d.path().join("a"), d.path().join("alias")).unwrap();
    let f = Files::default();
    f.apply(
        &root(d.path()),
        "/project",
        "*** Begin Patch\n*** Update File: a\n@@\n-first\n+changed\n*** End Patch",
    )
    .unwrap();
    assert_eq!(std::fs::read(d.path().join("alias")).unwrap(), b"changed\n");
}
#[test]
fn patch_refuses_utf16() {
    let d = tempfile::tempdir().unwrap();
    let f = Files::default();
    let utf16 = std::iter::once(255)
        .chain(std::iter::once(254))
        .chain("first\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect::<Vec<_>>();
    std::fs::write(d.path().join("u"), &utf16).unwrap();
    let error = f
        .apply(
            &root(d.path()),
            "/project",
            "*** Begin Patch\n*** Update File: u\n@@\n-first\n+changed\n*** End Patch",
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("stream did not contain valid UTF-8")
    );
    assert_eq!(std::fs::read(d.path().join("u")).unwrap(), utf16);
}
#[test]
fn create_permission_cannot_overwrite_a_file_appearing_during_preflight() {
    let d = tempfile::tempdir().unwrap();
    let f = Files::default();
    let appeared = std::cell::Cell::new(false);
    let result = f.apply_checked(
        &root(d.path()),
        "/project",
        "*** Begin Patch\n*** Add File: appeared\n+changed\n*** End Patch",
        None,
        &|kind| {
            if !appeared.replace(true) {
                std::fs::write(d.path().join("appeared"), "external revision").unwrap();
            }
            if kind == "edit" {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Edit was not granted",
                ))
            } else {
                Ok(())
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(d.path().join("appeared")).unwrap(),
        "external revision"
    );
}
#[cfg(windows)]
#[test]
fn junction_escape_refused() {
    use std::process::Command;
    let d = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    std::fs::write(
        other.path().join("secret"),
        "public outside boundary fixture",
    )
    .unwrap();
    let junction = d.path().join("escape");
    let status = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&junction)
        .arg(other.path())
        .output()
        .unwrap();
    assert!(status.status.success());
    let f = Files::default();
    assert!(
        f.read(&root(d.path()), "/project/escape/secret", 1, 10, 1024)
            .is_err()
    );
}
#[cfg(unix)]
#[test]
fn fifo_does_not_block_and_symlink_escape_refused() {
    use std::os::unix::fs::symlink;
    let d = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    std::fs::write(other.path().join("secret"), "outside fixture").unwrap();
    symlink(other.path(), d.path().join("escape")).unwrap();
    let f = Files::default();
    assert!(
        f.read(&root(d.path()), "/project/escape/secret", 1, 10, 1024)
            .is_err()
    );
    let fifo = d.path().join("fifo");
    let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    unsafe {
        assert_eq!(libc::mkfifo(c.as_ptr(), 0o600), 0);
    }
    let at = std::time::Instant::now();
    assert!(
        f.read(&root(d.path()), "/project/fifo", 1, 10, 1024)
            .is_err()
    );
    assert!(at.elapsed().as_secs() < 1);
}
