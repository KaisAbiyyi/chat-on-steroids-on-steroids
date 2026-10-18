use cos_core::config::Root;
use cos_tools::{
    files::Files,
    search::{self, Options},
};
use serde_json::json;
use std::{fs, path::Path};
fn root(path: &Path) -> [Root; 1] {
    [Root {
        name: "p".into(),
        path: fs::canonicalize(path).unwrap().to_string_lossy().into(),
        remote: None,
    }]
}
#[test]
fn limited_name_search_returns_the_root_file_before_descending() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("a")).unwrap();
    fs::write(temp.path().join("a/needle.txt"), "public").unwrap();
    fs::write(temp.path().join("z-needle.txt"), "public").unwrap();
    let result = search::find_checked(
        &Files::default(),
        &root(temp.path()),
        "/p",
        &Options {
            limit: 1,
            ..Options::new("needle")
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        result["matches"],
        json!([{"path":"/p/z-needle.txt"}]),
        "{result}"
    );
    assert_eq!(
        result["scanned"], 1,
        "name limit must stop traversal immediately: {result}"
    );
}
#[test]
fn traversal_counts_regular_files_before_applying_the_include_filter() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("public.rs"), "public").unwrap();
    fs::write(temp.path().join("ignored.txt"), "public").unwrap();
    fs::create_dir(temp.path().join("build")).unwrap();
    fs::write(temp.path().join("build/hidden.rs"), "public").unwrap();
    let result = search::find_checked(
        &Files::default(),
        &root(temp.path()),
        "/p",
        &Options {
            glob: Some("*.rs"),
            ..Options::new("public")
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        result["matches"],
        json!([{"path":"/p/public.rs"}]),
        "{result}"
    );
    assert_eq!(
        result["scanned"], 2,
        "excluded folders contribute no files; filtered files do: {result}"
    );
}
#[test]
fn explicitly_named_file_is_not_truncated_at_an_exact_match_limit() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("needle.txt"), "public needle\n").unwrap();
    let result = search::find_checked(
        &Files::default(),
        &root(temp.path()),
        "/p/needle.txt",
        &Options {
            content: true,
            limit: 1,
            ..Options::new("needle")
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        result["truncated"], false,
        "the source explicit-file path only truncates when another hit exists: {result}"
    );
}

#[test]
fn native_search_does_not_traverse_child_links() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("needle.txt"), "public outside fixture").unwrap();
    let link = temp.path().join("opaque");
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    let result = search::find_checked(
        &Files::default(),
        &root(temp.path()),
        "/p",
        &Options::new("needle"),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result["matches"], json!([]));
    assert_eq!(result["scanned"], 0);
}

#[test]
fn a_held_search_directory_cannot_publish_after_moving_outside_the_approved_root() {
    use search::{engine::Backend, native::Native};
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("held")).unwrap();
    fs::write(temp.path().join("held/needle.txt"), "public").unwrap();
    let roots = root(temp.path());
    let mut native = Native::new(&roots);
    let mut entries = native.open_folder("/p/held").unwrap();
    match fs::rename(temp.path().join("held"), outside.path().join("moved")) {
        Ok(()) => {
            let error = entries
                .next()
                .unwrap()
                .err()
                .expect("Moved directory entries must not be published");
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied,
                "{error:?}"
            );
        }
        Err(error) => {
            // Windows may refuse relocation while the live directory is open.
            assert!(cfg!(windows), "{error}");
            assert_eq!(
                error.raw_os_error(),
                Some(32),
                "Expected Windows sharing violation: {error:?}"
            );
            assert!(!outside.path().join("moved").exists());
            assert_eq!(
                fs::read_to_string(temp.path().join("held/needle.txt")).unwrap(),
                "public"
            );
        }
    }
}

#[test]
fn revoked_search_is_refused_before_path_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let called = std::cell::Cell::new(false);
    let result = search::find_checked(
        &Files::default(),
        &root(temp.path()),
        "/p/missing-private-path",
        &Options::new("needle"),
        &|| {
            called.set(true);
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Search revoked before resolution",
            ))
        },
    );
    let error = result.unwrap_err();
    assert!(
        called.get(),
        "Revoked search must not resolve or stat a path first: {error}"
    );
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "Search revoked before resolution");
}
