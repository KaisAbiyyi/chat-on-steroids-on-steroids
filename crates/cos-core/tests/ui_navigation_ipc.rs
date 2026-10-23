//! Phase 15A cloud-only contract tests. No renderer, native window or user data.
use cos_core::ui_contract::{
    MAX_JS_SAFE_INTEGER, MAX_UI_IPC_BYTES, UI_PROTOCOL_VERSION, UiCommand, UiError, UiNavigation,
    UiRequest, UiRoute,
};
use serde_json::{Value, json};

fn frame(epoch: u64, sequence: u64, command: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "protocol": UI_PROTOCOL_VERSION,
        "hostEpoch": epoch,
        "sequence": sequence,
        "command": command,
    }))
    .expect("synthetic request")
}

#[test]
fn navigation_roundtrip_is_revision_checked_and_idempotent() {
    let mut shell = UiNavigation::new(12).unwrap();
    assert_eq!(shell.snapshot().revision, 1);
    let r = shell
        .dispatch_json(&frame(
            12,
            1,
            json!({"action":"navigate", "expected_revision":1,
                "route":{"view":"conversation","session_id":"conv-a1"}}),
        ))
        .unwrap();
    let wire = serde_json::to_value(r).unwrap();
    assert_eq!(wire["hostEpoch"], 12);
    assert_eq!(wire["sequence"], 1);
    assert_eq!(wire["state"]["revision"], 2);
    assert_eq!(wire["state"]["route"]["session_id"], "conv-a1");

    // Navigating to the same destination does not emit a phantom revision.
    let same = shell
        .dispatch_json(&frame(
            12,
            2,
            json!({"action":"navigate", "expected_revision":2,
                "route":{"view":"conversation","session_id":"conv-a1"}}),
        ))
        .unwrap();
    assert_eq!(same.state.revision, 2);

    let sidebar = shell
        .dispatch_json(&frame(
            12,
            3,
            json!({"action":"set-sidebar","expected_revision":2,"open":true}),
        ))
        .unwrap();
    assert!(sidebar.state.sidebar_open);
    assert_eq!(sidebar.state.revision, 3);
    assert_eq!(
        shell.snapshot().route,
        UiRoute::Conversation {
            session_id: "conv-a1".to_string(),
        }
    );
}

#[test]
fn stale_or_replayed_mutations_are_rejected_without_state_changes() {
    let mut shell = UiNavigation::new(2).unwrap();
    let first = frame(
        2,
        1,
        json!({"action":"navigate","expected_revision":1,"route":{"view":"projects"}}),
    );
    shell.dispatch_json(&first).unwrap();
    assert_eq!(shell.dispatch_json(&first).unwrap_err(), UiError::Sequence);
    assert_eq!(
        shell
            .dispatch_json(&frame(2, 3, json!({"action":"snapshot"})))
            .unwrap_err(),
        UiError::Sequence
    );
    let before = shell.snapshot();
    let err = shell.dispatch_json(&frame(
        2,
        2,
        json!({"action":"navigate","expected_revision":1,"route":{"view":"settings"}}),
    ));
    assert_eq!(err.unwrap_err().code(), "stale-revision");
    assert_eq!(shell.snapshot(), before);
    // Rejected commands still consume the sequence.
    assert_eq!(
        shell
            .dispatch_json(&frame(2, 2, json!({"action":"snapshot"})))
            .unwrap_err(),
        UiError::Sequence
    );
    shell
        .dispatch_json(&frame(2, 3, json!({"action":"snapshot"})))
        .unwrap();
}

#[test]
fn rebind_retires_old_epoch_and_does_not_resurrect_selected_session() {
    let mut shell = UiNavigation::new(10).unwrap();
    shell
        .dispatch_json(&frame(
            10,
            1,
            json!({"action":"navigate","expected_revision":1,
                "route":{"view":"conversation","session_id":"old-session"}}),
        ))
        .unwrap();
    let old = shell.snapshot();
    assert_eq!(shell.rebind(10), Err(UiError::Epoch));
    assert_eq!(shell.rebind(9), Err(UiError::Epoch));
    assert_eq!(shell.snapshot(), old);
    shell.rebind(11).unwrap();
    assert_eq!(shell.snapshot().route, UiRoute::Home);
    assert_eq!(shell.snapshot().revision, 3);
    assert_eq!(
        shell
            .dispatch_json(&frame(10, 2, json!({"action":"snapshot"})))
            .unwrap_err(),
        UiError::Epoch
    );
    let new = shell
        .dispatch_json(&frame(11, 1, json!({"action":"snapshot"})))
        .unwrap();
    assert_eq!(new.state.host_epoch, 11);
    assert!(!new.state.sidebar_open);
}

#[test]
fn route_keys_never_act_as_paths_urls_or_commands() {
    let mut shell = UiNavigation::new(1).unwrap();
    let invalid = [
        "",
        "..",
        "../secrets",
        "/etc/passwd",
        "https://example.test",
        "a\\b",
        "with space",
        "<script>",
        "é",
    ];
    for (i, id) in invalid.iter().enumerate() {
        let result = shell.dispatch_json(&frame(
            1,
            (i + 1) as u64,
            json!({"action":"navigate","expected_revision":1,
                "route":{"view":"project","project_id":id}}),
        ));
        assert_eq!(result.unwrap_err(), UiError::Route, "{id}");
        assert_eq!(shell.snapshot().route, UiRoute::Home);
        assert_eq!(shell.snapshot().revision, 1);
    }
    assert_eq!(
        shell
            .dispatch_json(&frame(
                1,
                (invalid.len() + 1) as u64,
                json!({"action":"navigate","expected_revision":1,
                    "route":{"view":"files","project_id":"x".repeat(129)}})
            ))
            .unwrap_err(),
        UiError::Route
    );
}

#[test]
fn malformed_oversized_or_unknown_messages_do_not_advance_sequence() {
    let mut shell = UiNavigation::new(7).unwrap();
    let negative = [
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"execute","script":"alert(1)"}}"#
            .as_slice(),
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"snapshot","grant":"all"}}"#
            .as_slice(),
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"navigate","route":{"view":"settings","url":"https://example.test"},"expected_revision":1}}"#
            .as_slice(),
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"sequence":2,"command":{"action":"snapshot"}}"#
            .as_slice(),
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"snapshot"},"foo":null}"#
            .as_slice(),
        b"{",
    ];
    for payload in negative {
        assert_eq!(
            shell.dispatch_json(payload).unwrap_err(),
            UiError::Malformed
        );
    }
    assert_eq!(
        shell
            .dispatch_json(&vec![b' '; MAX_UI_IPC_BYTES + 1])
            .unwrap_err(),
        UiError::Oversize
    );
    assert_eq!(shell.snapshot().revision, 1);
    shell
        .dispatch_json(&frame(7, 1, json!({"action":"snapshot"})))
        .unwrap();
}

#[test]
fn protocol_epoch_and_js_integer_boundaries_fail_closed() {
    assert_eq!(UiNavigation::new(0).unwrap_err(), UiError::Epoch);
    assert_eq!(
        UiNavigation::new(MAX_JS_SAFE_INTEGER + 1).unwrap_err(),
        UiError::Epoch
    );
    let mut shell = UiNavigation::new(MAX_JS_SAFE_INTEGER).unwrap();
    let mut request = UiRequest {
        protocol: 777,
        host_epoch: MAX_JS_SAFE_INTEGER,
        sequence: 1,
        command: UiCommand::Snapshot,
    };
    assert_eq!(shell.dispatch(request.clone()), Err(UiError::Protocol));
    request.protocol = UI_PROTOCOL_VERSION;
    request.host_epoch = 2;
    assert_eq!(shell.dispatch(request.clone()), Err(UiError::Epoch));
    request.host_epoch = MAX_JS_SAFE_INTEGER;
    request.sequence = MAX_JS_SAFE_INTEGER + 1;
    assert_eq!(shell.dispatch(request.clone()), Err(UiError::Sequence));
    request.sequence = 1;
    shell.dispatch(request).unwrap();
    assert_eq!(shell.rebind(MAX_JS_SAFE_INTEGER), Err(UiError::Epoch));
    assert_eq!(shell.rebind(MAX_JS_SAFE_INTEGER + 1), Err(UiError::Epoch));
}
