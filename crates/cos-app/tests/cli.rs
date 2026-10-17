use serde_json::{Value, json};
use std::{fs, process::Command};
fn run(directory: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cos-rust"))
        .arg("--data-dir")
        .arg(directory)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn project_color_and_soft_removal_are_runnable_without_losing_catalog_identity() {
    let profile = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    assert!(run(profile.path(), &["info"]).status.success());
    let roots = json!([{"name":"p","path":folder.path()}]).to_string();
    assert!(
        run(profile.path(), &["config", "set", "roots", &roots])
            .status
            .success()
    );
    let p: Value =
        serde_json::from_slice(&run(profile.path(), &["projects", "add", "/p"]).stdout).unwrap();
    let id = p["id"].as_str().unwrap();
    let result = run(profile.path(), &["projects", "color", id, "rose"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["color"],
        "rose"
    );
    assert!(
        run(profile.path(), &["projects", "remove", id])
            .status
            .success()
    );
    let rows: Value =
        serde_json::from_slice(&run(profile.path(), &["projects", "list"]).stdout).unwrap();
    assert_eq!(rows[0]["id"], id);
    assert_eq!(rows[0]["ungrouped"], true);
    assert_eq!(rows[0]["chatgpt"], p["chatgpt"]);
    assert!(folder.path().is_dir());
}
#[test]
fn project_git_review_and_revision_fenced_diff_are_runnable() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet", "-b", "main"])
            .arg(project.path())
            .status()
            .unwrap()
            .success()
    );
    fs::write(project.path().join("a.txt"), "public lines\n").unwrap();
    assert!(run(profile.path(), &["info"]).status.success());
    let roots = json!([{"name":"p","path":project.path()}]).to_string();
    assert!(
        run(profile.path(), &["config", "set", "roots", &roots])
            .status
            .success()
    );
    let p: Value =
        serde_json::from_slice(&run(profile.path(), &["projects", "add", "/p"]).stdout).unwrap();
    let id = p["id"].as_str().unwrap();
    let result = run(profile.path(), &["projects", "review", id]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(snapshot["state"], "ready", "{snapshot}");
    let diff = run(
        profile.path(),
        &[
            "projects",
            "diff",
            id,
            "a.txt",
            "--revision",
            snapshot["revision"].as_str().unwrap(),
        ],
    );
    assert!(
        diff.status.success(),
        "{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&diff.stdout).unwrap()["currentText"],
        "public lines\n"
    );
    fs::write(project.path().join("b.txt"), "changed\n").unwrap();
    assert!(
        !run(
            profile.path(),
            &[
                "projects",
                "diff",
                id,
                "a.txt",
                "--revision",
                snapshot["revision"].as_str().unwrap()
            ]
        )
        .status
        .success()
    );
}
#[test]
fn explicit_profile_and_restart_are_required_and_supported() {
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_cos-rust"))
            .arg("info")
            .output()
            .unwrap()
            .status
            .success()
    );
    let dir = tempfile::tempdir().unwrap();
    for _ in 0..2 {
        let reply = run(dir.path(), &["info"]);
        assert!(
            reply.status.success(),
            "{}",
            String::from_utf8_lossy(&reply.stderr)
        );
        let info: Value = serde_json::from_slice(&reply.stdout).unwrap();
        assert_eq!(info["bridgeProtocol"], 14);
        assert_eq!(info["fullApplicationReady"], false);
    }
}
#[test]
fn public_fixture_smoke_is_durable_and_searchable() {
    let dir = tempfile::tempdir().unwrap();
    let reply = run(dir.path(), &["self-test"]);
    assert!(
        reply.status.success(),
        "{}",
        String::from_utf8_lossy(&reply.stderr)
    );
    let data: Value = serde_json::from_slice(&reply.stdout).unwrap();
    let id = data["sessionId"].as_str().unwrap();
    let history = run(dir.path(), &["history", id, "--limit", "10"]);
    assert!(history.status.success());
    let history: Value = serde_json::from_slice(&history.stdout).unwrap();
    assert!(
        history["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "assistant_message" && e["final"] == true)
    );
    let search = run(dir.path(), &["search", "public baseline"]);
    assert!(search.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&search.stdout).unwrap()["results"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn import_is_a_verified_copy_and_never_overwrites_source() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original");
    fs::create_dir(&original).unwrap();
    let config = cos_core::fresh_config("win32");
    fs::write(
        original.join("config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    let before = fs::read(original.join("config.json")).unwrap();
    let target = dir.path().join("rust-copy");
    let reply = run(&target, &["import", "--source", original.to_str().unwrap()]);
    assert!(
        reply.status.success(),
        "{}",
        String::from_utf8_lossy(&reply.stderr)
    );
    assert_eq!(fs::read(original.join("config.json")).unwrap(), before);
    assert_eq!(fs::read(target.join("config.json")).unwrap(), before);
    assert!(
        !run(&target, &["import", "--source", original.to_str().unwrap()])
            .status
            .success()
    );
}
#[test]
fn unknown_or_invalid_config_never_widens_saved_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let show = run(dir.path(), &["config", "show"]);
    assert!(show.status.success());
    assert!(
        run(dir.path(), &["config", "set", "readOnly", "true"])
            .status
            .success()
    );
    let before = fs::read(dir.path().join("config.json")).unwrap();
    assert!(
        !run(
            dir.path(),
            &["config", "set", "capabilities.command", "\"invalid\""]
        )
        .status
        .success()
    );
    assert_eq!(fs::read(dir.path().join("config.json")).unwrap(), before);
    let info = run(dir.path(), &["info"]);
    let info: Value = serde_json::from_slice(&info.stdout).unwrap();
    assert_eq!(info["effectiveCapabilities"]["command"], json!(false));
}
#[test]
fn unmarked_original_profile_is_not_opened_for_writing() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("config.json"),
        b"{\"privateOriginal\":true}",
    )
    .unwrap();
    assert!(!run(dir.path(), &["info"]).status.success());
    assert!(!dir.path().join("state").exists());
}
#[test]
fn usage_comparison_cap_is_explicit_without_a_provider_catalog() {
    let directory = tempfile::tempdir().unwrap();
    let output = run(directory.path(), &["usage", "--context-cap", "400000"]);
    assert!(output.status.success());
    let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reply["contextTokenCap"], 400000);
    assert_eq!(reply["contextCapSource"], "explicit");
}
