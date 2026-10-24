use cos_core::ui_rich::{
    RichDeck, RichError, RichPanel, RichReply, MAX_RICH_FRAME_BYTES, MAX_RICH_PANELS,
};
use serde_json::{json, Value};

fn request(deck: &mut RichDeck, sequence: u64, command: Value) -> Result<RichReply, RichError> {
    let bytes = serde_json::to_vec(&json!({
        "protocol": 1,
        "hostEpoch": 7,
        "sequence": sequence,
        "command": command,
    }))
    .unwrap();
    deck.dispatch_renderer_json(&bytes)
}

fn editor(key: &str, read_only: bool) -> RichPanel {
    RichPanel::Editor {
        key: key.to_owned(),
        title: "Draft".into(),
        language: "rust".into(),
        text: "initial".into(),
        read_only,
    }
}

#[test]
fn host_publishes_all_five_surfaces_and_renderer_can_only_change_editable_data() {
    let mut deck = RichDeck::new(7).unwrap();
    deck.publish_from_host(editor("editor", false)).unwrap();
    deck.publish_from_host(RichPanel::Terminal {
        key: "terminal".into(),
        title: "PTY log".into(),
        output: "Finished".into(),
    })
    .unwrap();
    deck.publish_from_host(RichPanel::Code {
        key: "code".into(),
        title: "Diff".into(),
        language: "diff".into(),
        text: "+line".into(),
    })
    .unwrap();
    deck.publish_from_host(RichPanel::Pdf {
        key: "pdf".into(),
        title: "Document".into(),
        asset_id: "opaque-asset".into(),
        page: 1,
        page_count: 3,
    })
    .unwrap();
    deck.publish_from_host(RichPanel::Math {
        key: "math".into(),
        title: "Formula".into(),
        source: "x^2".into(),
    })
    .unwrap();
    assert_eq!(deck.snapshot().panels.len(), 5);
    assert_eq!(deck.snapshot().active_key.as_deref(), Some("editor"));

    let revision = deck.snapshot().revision;
    request(
        &mut deck,
        1,
        json!({"action":"replace-editor","key":"editor","expected_revision":revision,"text":"safe edits"}),
    )
    .unwrap();

    let revision = deck.snapshot().revision;
    request(
        &mut deck,
        2,
        json!({"action":"set-pdf-page","key":"pdf","expected_revision":revision,"page":3}),
    )
    .unwrap();

    let revision = deck.snapshot().revision;
    request(
        &mut deck,
        3,
        json!({"action":"replace-math","key":"math","expected_revision":revision,"source":"a+b"}),
    )
    .unwrap();

    let revision = deck.snapshot().revision;
    request(
        &mut deck,
        4,
        json!({"action":"activate","key":"terminal","expected_revision":revision}),
    )
    .unwrap();

    let state = deck.snapshot();
    assert_eq!(state.active_key.as_deref(), Some("terminal"));
    assert!(matches!(&state.panels[0], RichPanel::Editor { text, .. } if text == "safe edits"));
    assert!(matches!(&state.panels[3], RichPanel::Pdf { page: 3, .. }));
    assert!(matches!(&state.panels[4], RichPanel::Math { source, .. } if source == "a+b"));

    let revision = state.revision;
    assert_eq!(
        request(
            &mut deck,
            5,
            json!({"action":"replace-editor","key":"terminal","expected_revision":revision,"text":"injection"})
        )
        .unwrap_err(),
        RichError::Forbidden
    );
    assert_eq!(deck.snapshot(), state);
    assert_eq!(request(&mut deck, 5, json!({"action":"snapshot"})).unwrap_err(), RichError::Sequence);
    assert_eq!(request(&mut deck, 6, json!({"action":"snapshot"})).unwrap().state, state);
}

#[test]
fn renderer_must_not_modify_read_only_code_or_terminal_or_invent_permissions() {
    let mut deck = RichDeck::new(7).unwrap();
    deck.publish_from_host(editor("readonly", true)).unwrap();
    deck.publish_from_host(RichPanel::Code {
        key: "code".into(),
        title: "Code".into(),
        language: "typescript".into(),
        text: "let x = 1".into(),
    })
    .unwrap();
    let revision = deck.snapshot().revision;
    for (sequence, key) in [(1, "readonly"), (2, "code")] {
        assert_eq!(
            request(
                &mut deck,
                sequence,
                json!({"action":"replace-editor","key":key,"expected_revision":revision,"text":"nope"})
            )
            .unwrap_err(),
            RichError::Forbidden
        );
    }
    let bytes = serde_json::to_vec(&json!({
        "protocol":1,"hostEpoch":7,"sequence":3,
        "command":{"action":"run-terminal","key":"readonly","command":"id"}
    }))
    .unwrap();
    assert_eq!(deck.dispatch_renderer_json(&bytes).unwrap_err(), RichError::Malformed);
    assert_eq!(deck.snapshot().revision, revision);
}

#[test]
fn malformed_stale_and_oversized_frames_are_rejected_without_side_effects() {
    let mut deck = RichDeck::new(7).unwrap();
    deck.publish_from_host(editor("editor", false)).unwrap();
    let state = deck.snapshot();
    let invalid = [
        json!({"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"snapshot","extra":1}}),
        json!({"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"activate","key":"editor","expected_revision":state.revision,"extra":true}}),
        json!({"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"snapshot"},"hidden":true}),
    ];
    for payload in invalid {
        let bytes = serde_json::to_vec(&payload).unwrap();
        assert_eq!(deck.dispatch_renderer_json(&bytes).unwrap_err(), RichError::Malformed);
    }
    assert_eq!(
        deck.dispatch_renderer_json(&vec![b'x'; MAX_RICH_FRAME_BYTES + 1]).unwrap_err(),
        RichError::Oversize
    );
    let wrong_protocol = json!({"protocol":2,"hostEpoch":7,"sequence":1,"command":{"action":"snapshot"}});
    assert_eq!(
        deck.dispatch_renderer_json(&serde_json::to_vec(&wrong_protocol).unwrap()).unwrap_err(),
        RichError::Protocol
    );
    let stale_epoch = json!({"protocol":1,"hostEpoch":6,"sequence":1,"command":{"action":"snapshot"}});
    assert_eq!(
        deck.dispatch_renderer_json(&serde_json::to_vec(&stale_epoch).unwrap()).unwrap_err(),
        RichError::Epoch
    );
    assert_eq!(
        request(&mut deck, 2, json!({"action":"snapshot"})).unwrap_err(),
        RichError::Sequence
    );
    assert_eq!(
        request(
            &mut deck,
            1,
            json!({"action":"replace-editor","key":"editor","expected_revision":1,"text":"stale"})
        )
        .unwrap_err(),
        RichError::Revision
    );
    assert_eq!(request(&mut deck, 1, json!({"action":"snapshot"})).unwrap_err(), RichError::Sequence);
    assert_eq!(deck.snapshot(), state);
}

#[test]
fn unsafe_ids_content_pdf_pages_and_capacity_are_refused_atomically() {
    let mut deck = RichDeck::new(7).unwrap();
    let state = deck.snapshot();
    assert_eq!(
        deck.publish_from_host(editor("../root", false)).unwrap_err(),
        RichError::Key
    );
    assert_eq!(
        deck.publish_from_host(RichPanel::Math {
            key: "math".into(),
            title: "Unsafe".into(),
            source: "hello\u{202e}world".into(),
        })
        .unwrap_err(),
        RichError::Text
    );
    assert_eq!(
        deck.publish_from_host(RichPanel::Pdf {
            key: "pdf".into(),
            title: "Pdf".into(),
            asset_id: "safe".into(),
            page: 2,
            page_count: 1,
        })
        .unwrap_err(),
        RichError::Page
    );
    assert_eq!(deck.snapshot(), state);
    for i in 0..MAX_RICH_PANELS {
        deck.publish_from_host(editor(&format!("editor-{i}"), false))
            .unwrap();
    }
    assert_eq!(
        deck.publish_from_host(editor("extra", false)).unwrap_err(),
        RichError::Capacity
    );
    let before = deck.snapshot();
    assert_eq!(
        deck.publish_from_host(RichPanel::Terminal {
            key: "editor-0".into(),
            title: "TTY".into(),
            output: "x".repeat(8_193),
        })
        .unwrap_err(),
        RichError::Oversize
    );
    assert_eq!(deck.snapshot(), before);
    assert_eq!(
        request(
            &mut deck,
            1,
            json!({"action":"set-pdf-page","key":"editor-0","expected_revision":before.revision,"page":2})
        )
        .unwrap_err(),
        RichError::Forbidden
    );
}

#[test]
fn pdf_page_boundaries_snapshot_quota_and_rebind_fence() {
    let mut deck = RichDeck::new(7).unwrap();
    deck.publish_from_host(RichPanel::Pdf {
        key: "pdf".into(),
        title: "Preview".into(),
        asset_id: "opaque".into(),
        page: 1,
        page_count: 2,
    })
    .unwrap();
    let before = deck.snapshot();
    assert_eq!(
        request(
            &mut deck,
            1,
            json!({"action":"set-pdf-page","key":"pdf","expected_revision":before.revision,"page":0})
        )
        .unwrap_err(),
        RichError::Page
    );
    assert_eq!(
        request(
            &mut deck,
            2,
            json!({"action":"set-pdf-page","key":"pdf","expected_revision":before.revision,"page":3})
        )
        .unwrap_err(),
        RichError::Page
    );
    assert_eq!(deck.snapshot(), before);
    assert_eq!(deck.rebind(7).unwrap_err(), RichError::Epoch);
    deck.rebind(8).unwrap();
    assert!(deck.snapshot().panels.is_empty());
    assert_eq!(deck.snapshot().active_key, None);
    assert_eq!(
        request(&mut deck, 3, json!({"action":"snapshot"})).unwrap_err(),
        RichError::Epoch
    );
    assert_eq!(deck.remove_from_host("pdf").unwrap_err(), RichError::NotFound);

    let mut bounded = RichDeck::new(11).unwrap();
    for i in 0..7 {
        bounded
            .publish_from_host(RichPanel::Code {
                key: format!("code-{i}"),
                title: "Preview".into(),
                language: "text".into(),
                text: "x".repeat(4_000),
            })
            .unwrap();
    }
    let safe_state = bounded.snapshot();
    assert_eq!(
        bounded
            .publish_from_host(RichPanel::Code {
                key: "code-7".into(),
                title: "Preview".into(),
                language: "text".into(),
                text: "x".repeat(4_000),
            })
            .unwrap_err(),
        RichError::Oversize
    );
    assert_eq!(bounded.snapshot(), safe_state);
}
