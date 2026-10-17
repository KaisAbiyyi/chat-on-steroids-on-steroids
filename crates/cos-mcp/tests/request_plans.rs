use cos_core::persistence::ConfigStore;
use cos_mcp::kernel::{Kernel, Surface};
use cos_store::{CreateOptions, correlation::Correlation};
use serde_json::json;
use std::sync::Arc;
#[test]
fn exact_proof_attaches_provisional_plan_without_changing_epoch() {
    let d = tempfile::tempdir().unwrap();
    let config = Arc::new(
        ConfigStore::open(d.path(), if cfg!(windows) { "win32" } else { "linux" }).unwrap(),
    );
    config
        .update(|v| v["multiAgent"]["allowUnattributedCalls"] = json!(true))
        .unwrap();
    let kernel = Kernel::new(config).unwrap();
    let result = kernel.invoke(
        Surface::Core,
        "wfr_plan",
        "update_plan",
        json!({"plan":[{"step":"  public plan  ","status":"in_progress"}]}),
    );
    assert_ne!(result["isError"], true);
    let id = "phase4-plan-proof";
    let conversation = "11111111-1111-4111-8111-111111111111";
    kernel
        .sessions
        .create(CreateOptions {
            id: Some(id.into()),
            conversation_id: Some(conversation.into()),
            ..Default::default()
        })
        .unwrap();
    kernel
        .correlations
        .observe_and_commit(&[Correlation {
            request_id: "wfr_plan".into(),
            conversation_id: conversation.into(),
            session_id: id.into(),
            message_id: "public-message".into(),
            tool: "update_plan".into(),
            observed_at: chrono::Utc::now().timestamp_millis(),
        }])
        .unwrap();
    kernel.admit("wfr_plan").unwrap();
    let plan = kernel.sessions.read_plan(id).unwrap().unwrap();
    assert_eq!(plan["plan"][0]["step"], "public plan");
    assert!(
        kernel
            .durable
            .read("request-plans", true)
            .unwrap()
            .unwrap()
            .get("wfr_plan")
            .is_none()
    );
}
