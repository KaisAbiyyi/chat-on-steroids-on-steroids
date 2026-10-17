use cos_core::persistence::ConfigStore;
use cos_mcp::kernel::Kernel;
use serde_json::json;
use std::sync::Arc;
#[test]
fn catalog_project_native_path_opens_terminal_and_revoked_root_refuses() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    config
        .update(|v| v["roots"] = json!([{"name":"p","path":project.path()}]))
        .unwrap();
    let kernel = Kernel::new(config.clone()).unwrap();
    let p = kernel.projects.add(&kernel.roots().unwrap(), "/p").unwrap();
    let id = p["id"].as_str().unwrap();
    let terminals = kernel
        .terminal_owner(profile.path(), Arc::new(|_| {}))
        .unwrap();
    let opened = terminals.create("project-fixture", Some(id), 80, 24);
    assert!(opened.is_ok(), "{opened:?}");
    terminals.close("project-fixture");
    config.update(|v| v["roots"] = json!([])).unwrap();
    assert!(
        terminals
            .create("revoked-fixture", Some(id), 80, 24)
            .is_err()
    );
    terminals.dispose();
}

#[test]
fn proven_session_project_resolves_default_tool_workspace() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    config
        .update(|v| v["roots"] = json!([{"name":"p","path":project.path()}]))
        .unwrap();
    let kernel = Kernel::new(config).unwrap();
    let p = kernel.projects.add(&kernel.roots().unwrap(), "/p").unwrap();
    let conversation = "11111111-1111-4111-8111-111111111111";
    kernel
        .sessions
        .create(cos_store::CreateOptions {
            id: Some("public-project-call".into()),
            conversation_id: Some(conversation.into()),
            ..Default::default()
        })
        .unwrap();
    kernel
        .sessions
        .update_metadata("public-project-call", |v| v["projectId"] = p["id"].clone())
        .unwrap();
    kernel
        .correlations
        .observe_and_commit(&[cos_store::correlation::Correlation {
            request_id: "wfr_project".into(),
            conversation_id: conversation.into(),
            session_id: "public-project-call".into(),
            message_id: "public-message".into(),
            tool: "apply_patch".into(),
            observed_at: chrono::Utc::now().timestamp_millis(),
        }])
        .unwrap();
    let result=kernel.invoke(cos_mcp::kernel::Surface::Core,"wfr_project","apply_patch",json!({"patch":"*** Begin Patch\n*** Add File: native.txt\n+public project\n*** End Patch"}));
    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        std::fs::read(project.path().join("native.txt")).unwrap(),
        b"public project\n"
    );
}

#[test]
fn missing_bound_project_cannot_fallback_to_another_approved_folder() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(
            profile.path(),
            if cfg!(windows) { "win32" } else { "linux" },
        )
        .unwrap(),
    );
    config
        .update(|v| v["roots"] = json!([{"name":"p","path":project.path()}]))
        .unwrap();
    let kernel = Kernel::new(config).unwrap();
    let conversation = "11111111-1111-4111-8111-111111111111";
    kernel
        .sessions
        .create(cos_store::CreateOptions {
            id: Some("public-project-call".into()),
            conversation_id: Some(conversation.into()),
            ..Default::default()
        })
        .unwrap();
    kernel
        .sessions
        .update_metadata("public-project-call", |v| {
            v["projectId"] = json!("22222222-2222-4222-8222-222222222222")
        })
        .unwrap();
    kernel
        .correlations
        .observe_and_commit(&[cos_store::correlation::Correlation {
            request_id: "wfr_project".into(),
            conversation_id: conversation.into(),
            session_id: "public-project-call".into(),
            message_id: "public-message".into(),
            tool: "apply_patch".into(),
            observed_at: chrono::Utc::now().timestamp_millis(),
        }])
        .unwrap();
    let result=kernel.invoke(cos_mcp::kernel::Surface::Core,"wfr_project","apply_patch",json!({"patch":"*** Begin Patch\n*** Add File: native.txt\n+public project\n*** End Patch"}));
    assert_eq!(result["isError"], true, "{result}");
    assert!(!project.path().join("native.txt").exists());
}
