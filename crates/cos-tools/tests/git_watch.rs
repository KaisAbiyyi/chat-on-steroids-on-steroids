use cos_core::config::Root;
use cos_tools::git::GitWatchSet;
use serde_json::{Value, json};
use std::{
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
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
        path: std::fs::canonicalize(dir).unwrap().to_string_lossy().into(),
        remote: None,
    }]
}
fn wait(w: &mut GitWatchSet, roots: &[Root]) -> Option<Value> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let result = w.poll(roots).unwrap();
        if result.is_some() || Instant::now() >= deadline {
            return result;
        };
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn nested_worktree_writes_invalidate_one_active_project() {
    let d = tempfile::tempdir().unwrap();
    let r = roots(d.path());
    command(d.path(), &["init", "--quiet", "-b", "main"]);
    std::fs::create_dir(d.path().join("nested")).unwrap();
    let mut w = GitWatchSet::default();
    w.sync(&r, "/p", Some("first")).unwrap();
    std::fs::write(d.path().join("nested/a"), "public change").unwrap();
    assert_eq!(wait(&mut w, &r), Some(json!({"projectId":"first"})));
    assert!(w.poll(&r).unwrap().is_none());
}
#[test]
fn linked_worktree_common_refs_are_watched_outside_selected_project() {
    let d = tempfile::tempdir().unwrap();
    let main = d.path().join("main");
    std::fs::create_dir(&main).unwrap();
    command(&main, &["init", "--quiet", "-b", "main"]);
    std::fs::write(main.join("a"), "public base").unwrap();
    command(&main, &["add", "a"]);
    command(&main, &["commit", "--quiet", "-m", "base"]);
    command(
        &main,
        &["worktree", "add", "--quiet", "-b", "feature", "../linked"],
    );
    let r = roots(d.path());
    let mut w = GitWatchSet::default();
    w.sync(&r, "/p/linked", Some("linked")).unwrap();
    command(&main, &["branch", "new-ref"]);
    assert_eq!(wait(&mut w, &r), Some(json!({"projectId":"linked"})));
}

#[test]
fn linked_worktree_watches_new_ref_namespaces_after_refresh() {
    let d = tempfile::tempdir().unwrap();
    let main = d.path().join("main");
    std::fs::create_dir(&main).unwrap();
    command(&main, &["init", "--quiet", "-b", "main"]);
    std::fs::write(main.join("a"), "public base").unwrap();
    command(&main, &["add", "a"]);
    command(&main, &["commit", "--quiet", "-m", "base"]);
    command(
        &main,
        &["worktree", "add", "--quiet", "-b", "feature", "../linked"],
    );
    let r = roots(d.path());
    let mut w = GitWatchSet::default();
    w.sync(&r, "/p/linked", Some("linked")).unwrap();
    command(&main, &["branch", "team/first"]);
    assert_eq!(wait(&mut w, &r), Some(json!({"projectId":"linked"})));
    command(&main, &["branch", "team/second"]);
    assert_eq!(wait(&mut w, &r), Some(json!({"projectId":"linked"})));
}

#[test]
fn switching_projects_and_closing_discard_previous_generation_events() {
    let d = tempfile::tempdir().unwrap();
    let r = roots(d.path());
    for name in ["one", "two"] {
        let dir = d.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        command(&dir, &["init", "--quiet"]);
    }
    let mut w = GitWatchSet::default();
    w.sync(&r, "/p/one", Some("one")).unwrap();
    std::fs::write(d.path().join("one/a"), "old event").unwrap();
    w.sync(&r, "/p/two", Some("two")).unwrap();
    std::thread::sleep(Duration::from_millis(220));
    assert!(w.poll(&r).unwrap().is_none());
    std::fs::write(d.path().join("two/a"), "new event").unwrap();
    assert_eq!(wait(&mut w, &r), Some(json!({"projectId":"two"})));
    w.close();
    std::fs::write(d.path().join("two/b"), "closed event").unwrap();
    assert!(w.poll(&r).unwrap().is_none());
}
#[test]
fn revoked_roots_refuse_queued_git_watch_events_and_drop_handles() {
    let d = tempfile::tempdir().unwrap();
    let r = roots(d.path());
    command(d.path(), &["init", "--quiet"]);
    let mut w = GitWatchSet::default();
    w.sync(&r, "/p", Some("one")).unwrap();
    std::fs::write(d.path().join("a"), "public event").unwrap();
    assert!(w.poll(&[]).is_err());
    assert!(w.poll(&[]).unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn pruning_optional_ref_directories_keeps_watcher_usable() {
    let d = tempfile::tempdir().unwrap();
    let r = roots(d.path());
    command(d.path(), &["init", "--quiet", "-b", "main"]);
    let mut w = GitWatchSet::default();
    w.sync(&r, "/p", Some("one")).unwrap();
    std::fs::remove_dir(d.path().join(".git/refs/tags")).unwrap();
    assert_eq!(wait(&mut w, &r), Some(json!({"projectId":"one"})));
    std::fs::write(d.path().join("after-prune"), "public change").unwrap();
    assert_eq!(wait(&mut w, &r), Some(json!({"projectId":"one"})));
}

#[cfg(target_os = "linux")]
#[test]
fn reading_project_files_does_not_invalidate_git_review() {
    let d = tempfile::tempdir().unwrap();
    let r = roots(d.path());
    command(d.path(), &["init", "--quiet", "-b", "main"]);
    std::fs::write(d.path().join("a"), "public file").unwrap();
    let mut w = GitWatchSet::default();
    w.sync(&r, "/p", Some("one")).unwrap();
    // Drain setup activity before testing read-only access.
    let _ = wait(&mut w, &r);
    std::fs::read(d.path().join("a")).unwrap();
    assert!(wait(&mut w, &r).is_none());
}
