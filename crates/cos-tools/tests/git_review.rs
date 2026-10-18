use cos_core::config::Root;
use cos_tools::git;
use std::{path::Path, process::Command};
fn command(dir: &Path, args: &[&str]) {
    let result = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "user.name=Public Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgSign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
fn roots(dir: &Path) -> [Root; 1] {
    [Root {
        name: "p".into(),
        path: std::fs::canonicalize(dir)
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        remote: None,
    }]
}
fn repository() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    command(d.path(), &["init", "--quiet", "-b", "main"]);
    d
}
fn commit(dir: &Path) {
    command(dir, &["add", "--all"]);
    command(dir, &["commit", "--quiet", "-m", "public fixture"]);
}
#[test]
fn unborn_snapshot_has_branches_and_untracked_line_counts() {
    let d = repository();
    std::fs::write(d.path().join("new.txt"), "one\ntwo\n").unwrap();
    let s = git::snapshot(&roots(d.path()), "/p", "fixture", None).unwrap();
    assert_eq!(s["state"], "ready");
    assert_eq!(s["projectId"], "fixture");
    assert_eq!(s["currentBranch"], "main");
    assert_eq!(
        s["changes"][0],
        serde_json::json!({"status":"U","path":"new.txt","additions":2,"deletions":0,"binary":false})
    );
    let diff = git::diff(
        &roots(d.path()),
        "/p",
        "fixture",
        "new.txt",
        None,
        s["revision"].as_str(),
    )
    .unwrap();
    assert_eq!(diff["baseText"], "");
    assert_eq!(diff["currentText"], "one\ntwo\n");
}
#[test]
fn rename_binary_and_stale_comparison_are_projected() {
    let d = repository();
    std::fs::write(d.path().join("old.txt"), "one\ntwo\n").unwrap();
    commit(d.path());
    command(d.path(), &["mv", "old.txt", "new.txt"]);
    std::fs::write(d.path().join("binary"), [0, 1, 2]).unwrap();
    let s = git::snapshot(&roots(d.path()), "/p", "fixture", None).unwrap();
    let changes = s["changes"].as_array().expect("missing Git changes");
    let renamed = changes.iter().find(|c| c["path"] == "new.txt").unwrap();
    assert_eq!(renamed["status"], "R");
    assert_eq!(renamed["previousPath"], "old.txt");
    let binary = git::diff(&roots(d.path()), "/p", "fixture", "binary", None, None).unwrap();
    assert_eq!(binary["binary"], true);
    assert!(binary["baseText"].is_null());
    std::fs::write(d.path().join("another"), "change").unwrap();
    assert!(
        git::diff(
            &roots(d.path()),
            "/p",
            "fixture",
            "new.txt",
            None,
            s["revision"].as_str()
        )
        .is_err()
    );
}
#[test]
fn branch_comparison_uses_commits_and_excludes_dirty_worktree() {
    let d = repository();
    std::fs::write(d.path().join("a"), "base\n").unwrap();
    commit(d.path());
    command(d.path(), &["branch", "base"]);
    std::fs::write(d.path().join("a"), "committed\n").unwrap();
    commit(d.path());
    std::fs::write(d.path().join("a"), "dirty\n").unwrap();
    let s = git::snapshot(&roots(d.path()), "/p", "fixture", Some("refs/heads/base")).unwrap();
    assert_eq!(s["state"], "ready");
    assert_eq!(s["comparison"]["ref"], "refs/heads/base");
    let diff = git::diff(
        &roots(d.path()),
        "/p",
        "fixture",
        "a",
        Some("refs/heads/base"),
        s["revision"].as_str(),
    )
    .unwrap();
    assert_eq!(diff["baseText"], "base\n");
    assert_eq!(diff["currentText"], "committed\n");
    assert_eq!(
        git::snapshot(&roots(d.path()), "/p", "fixture", Some("--unsafe")).unwrap()["state"],
        "unavailable"
    );
}
#[test]
fn subproject_cannot_read_unapproved_repository_metadata() {
    let d = repository();
    std::fs::create_dir(d.path().join("nested")).unwrap();
    let approved = roots(&d.path().join("nested"));
    let s = git::snapshot(&approved, "/p", "fixture", None).unwrap();
    assert_eq!(s["state"], "unavailable");
}

#[test]
fn legacy_git_status_also_refuses_metadata_outside_approved_folder() {
    let d = repository();
    std::fs::create_dir(d.path().join("nested")).unwrap();
    assert!(cos_tools::project::git(&roots(&d.path().join("nested")), "/p").is_err());
}

#[test]
fn oversized_untracked_binary_keeps_binary_classification_without_preview() {
    let d = repository();
    std::fs::File::create(d.path().join("large.bin"))
        .unwrap()
        .set_len(600 * 1024)
        .unwrap();
    let s = git::snapshot(&roots(d.path()), "/p", "fixture", None).unwrap();
    assert_eq!(s["changes"][0]["binary"], true);
    let diff = git::diff(&roots(d.path()), "/p", "fixture", "large.bin", None, None).unwrap();
    assert_eq!(diff["tooLarge"], true);
    assert!(diff["currentText"].is_null());
}
