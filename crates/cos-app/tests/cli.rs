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
fn add_project(directory: &std::path::Path) -> Value {
    let output = run(directory, &["projects", "add", "/p"]);
    assert!(
        output.status.success(),
        "projects add /p failed: status={} stderr={} stdout={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).expect("successful project add must return JSON")
}
#[test]
fn session_names_track_provider_authority_and_clear_through_the_real_cli() {
    let profile = tempfile::tempdir().unwrap();
    assert!(run(profile.path(), &["info"]).status.success());
    assert!(
        run(
            profile.path(),
            &[
                "sessions",
                "create",
                "--id",
                "session-title-cli",
                "--conversation",
                "conversation-public"
            ]
        )
        .status
        .success()
    );
    let renamed = run(
        profile.path(),
        &["sessions", "rename", "session-title-cli", "Manual public"],
    );
    assert!(
        renamed.status.success(),
        "{}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    let provider = run(
        profile.path(),
        &[
            "sessions",
            "rename",
            "session-title-cli",
            "Provider public",
            "--source",
            "provider",
            "--conversation",
            "conversation-public",
        ],
    );
    assert!(
        provider.status.success(),
        "{}",
        String::from_utf8_lossy(&provider.stderr)
    );
    let manual: Value = serde_json::from_slice(&provider.stdout).unwrap();
    assert_eq!(manual["title"], "Manual public");
    assert_eq!(
        manual["autoTitle"],
        json!({"title":"Provider public","source":"provider"})
    );
    let cleared = run(
        profile.path(),
        &["sessions", "clear-name", "session-title-cli"],
    );
    assert!(
        cleared.status.success(),
        "{}",
        String::from_utf8_lossy(&cleared.stderr)
    );
    let result: Value = serde_json::from_slice(&cleared.stdout).unwrap();
    assert_eq!(result["title"], "Provider public");
    assert_eq!(result["titleSource"], "provider");
    assert!(result.get("autoTitle").is_none());
}
#[test]
fn markdown_answer_export_requires_a_completed_turn_and_uses_only_its_answer() {
    let profile = tempfile::tempdir().unwrap();
    let id = "session-answer-cli";
    assert!(run(profile.path(), &["info"]).status.success());
    assert!(
        run(
            profile.path(),
            &["sessions", "create", "Public export", "--id", id]
        )
        .status
        .success()
    );
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/session-content.json"
    ))
    .unwrap();
    let case = fixture["exports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "entities")
        .unwrap();
    for (index, event) in case["events"].as_array().unwrap().iter().enumerate() {
        let input = profile.path().join(format!("public-event-{index}.json"));
        fs::write(&input, event.to_string()).unwrap();
        let appended = run(profile.path(), &["append", id, input.to_str().unwrap()]);
        assert!(
            appended.status.success(),
            "{}",
            String::from_utf8_lossy(&appended.stderr)
        );
    }
    let output = run(
        profile.path(),
        &[
            "export",
            id,
            "--format",
            "md",
            "--scope",
            "answer",
            "--turn",
            "turn-public",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected = case["outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["scope"] == "answer")
        .unwrap();
    assert_eq!(actual["text"], expected["text"]);
    let unknown = run(
        profile.path(),
        &[
            "export",
            id,
            "--format",
            "md",
            "--scope",
            "answer",
            "--turn",
            "turn-unknown",
        ],
    );
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("This turn has not completed"));
}
#[test]
fn project_file_panel_is_runnable_with_stale_edit_and_root_revocation_guards() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    assert!(run(profile.path(), &["info"]).status.success());
    let roots = json!([{"name":"p","path":root.path()}]).to_string();
    assert!(
        run(profile.path(), &["config", "set", "roots", &roots])
            .status
            .success()
    );
    let p = add_project(profile.path());
    let id = p["id"].as_str().unwrap();
    let result = run(
        profile.path(),
        &["projects", "create-entry", id, "", "public.txt", "file"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    fs::write(root.path().join("public.txt"), "public\r\n").unwrap();
    let preview = run(profile.path(), &["projects", "preview", id, "public.txt"]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    let edit = profile.path().join("public-edit.json");
    fs::write(&edit, json!({"text":"edited\r\n","expectedModifiedAt":preview["modifiedAt"],"expectedBytes":preview["bytes"],"expectedRevision":preview["revision"]}).to_string()).unwrap();
    let args = [
        "projects",
        "save-file",
        id,
        "public.txt",
        edit.to_str().unwrap(),
    ];
    let saved = run(profile.path(), &args);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    assert_eq!(
        fs::read(root.path().join("public.txt")).unwrap(),
        b"edited\r\n"
    );
    assert!(!run(profile.path(), &args).status.success());
    assert!(
        run(
            profile.path(),
            &["projects", "rename-entry", id, "public.txt", "renamed.txt"]
        )
        .status
        .success()
    );
    let listing = run(profile.path(), &["projects", "files", id]);
    assert!(listing.status.success());
    let listing: Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(listing["entries"][0]["name"], "renamed.txt");
    assert!(
        run(profile.path(), &["config", "set", "roots", "[]"])
            .status
            .success()
    );
    assert!(
        !run(profile.path(), &["projects", "preview", id, "renamed.txt"])
            .status
            .success()
    );
    assert!(
        !run(
            profile.path(),
            &["projects", "create-entry", id, "", "revoked.txt", "file"]
        )
        .status
        .success()
    );
    assert!(!root.path().join("revoked.txt").exists());
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
    let p = add_project(profile.path());
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
    let p = add_project(profile.path());
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
