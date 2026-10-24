//! Synthetic 15B tests. No private conversations, native renderer or backend IO.
use cos_core::ui_conversation::{
    ConversationEntry, ConversationError, ConversationPage, ConversationRole,
    MAX_CONVERSATION_PAGE_ITEMS, MAX_CONVERSATION_REQUEST_BYTES, PageDirection, UiConversation,
};
use serde_json::{Value, json};

fn entry(position: u64) -> ConversationEntry {
    ConversationEntry {
        position,
        message_id: format!("message-{position}"),
        turn_id: Some("turn-1".into()),
        role: if position % 2 == 0 {
            ConversationRole::User
        } else {
            ConversationRole::Assistant
        },
        text: format!("Synthetic turn {position}"),
        truncated: false,
    }
}

fn page(start: u64, total: u64, count: u64) -> ConversationPage {
    ConversationPage {
        start,
        total,
        entries: (start..start + count).map(entry).collect(),
    }
}

fn request(
    model: &mut UiConversation,
    epoch: u64,
    sequence: u64,
    command: Value,
) -> Result<cos_core::ui_conversation::ConversationReply, ConversationError> {
    model.dispatch_json(
        &serde_json::to_vec(&json!({
            "protocol": 1, "hostEpoch": epoch, "sequence": sequence, "command": command
        }))
        .unwrap(),
    )
}

#[test]
fn host_loads_latest_and_renderer_pages_older_then_newer() {
    let mut model = UiConversation::new(7).unwrap();
    let initial = model.open_from_host("session-1").unwrap();
    assert_eq!(initial.direction, PageDirection::Initial);
    model
        .publish_from_host(&initial, "session-1", page(90, 100, 10))
        .unwrap();
    let state = model.snapshot();
    assert_eq!(state.page.as_ref().unwrap().entries.len(), 10);
    assert_eq!(state.page.as_ref().unwrap().start, 90);

    let reply = request(
        &mut model,
        7,
        1,
        json!({
            "action": "load-older", "expected_revision": state.revision
        }),
    )
    .unwrap();
    let older = reply.state.pending.unwrap();
    assert_eq!(older.direction, PageDirection::Older);
    assert_eq!(
        request(
            &mut model,
            7,
            2,
            json!({
                "action": "load-newer", "expected_revision": reply.state.revision
            })
        )
        .unwrap_err(),
        ConversationError::Pending
    );
    assert_eq!(model.snapshot().page.unwrap().start, 90);

    model
        .publish_from_host(&older, "session-1", page(70, 100, 20))
        .unwrap();
    let revision = model.snapshot().revision;
    let newer = request(
        &mut model,
        7,
        3,
        json!({
            "action": "load-newer", "expected_revision": revision
        }),
    )
    .unwrap()
    .state
    .pending
    .unwrap();
    model
        .publish_from_host(&newer, "session-1", page(90, 100, 10))
        .unwrap();
    assert_eq!(model.snapshot().page.unwrap().start, 90);
    assert!(model.snapshot().pending.is_none());
}

#[test]
fn wrong_sessions_stale_tickets_and_rebind_never_reveal_prior_messages() {
    let mut model = UiConversation::new(10).unwrap();
    let one = model.open_from_host("session-1").unwrap();
    let two = model.open_from_host("session-2").unwrap();
    assert_eq!(
        model.publish_from_host(&one, "session-1", page(0, 1, 1)),
        Err(ConversationError::Stale)
    );
    assert_eq!(
        model.publish_from_host(&two, "session-1", page(0, 1, 1)),
        Err(ConversationError::Stale)
    );
    model
        .publish_from_host(&two, "session-2", page(0, 1, 1))
        .unwrap();
    assert_eq!(model.rebind(10), Err(ConversationError::Epoch));
    model.rebind(11).unwrap();
    let state = model.snapshot();
    assert_eq!(state.session_id, None);
    assert_eq!(state.page, None);
    assert_eq!(state.pending, None);
    assert_eq!(
        model.publish_from_host(&two, "session-2", page(0, 1, 1)),
        Err(ConversationError::Stale)
    );
    assert_eq!(
        request(&mut model, 10, 1, json!({"action":"snapshot"})).unwrap_err(),
        ConversationError::Epoch
    );
    request(&mut model, 11, 1, json!({"action":"snapshot"})).unwrap();
}

#[test]
fn page_integrity_and_atomic_rejections() {
    let mut model = UiConversation::new(7).unwrap();
    let ticket = model.open_from_host("session-1").unwrap();
    let baseline = model.snapshot();
    let wrong = [
        page(0, 100, 0),
        page(99, 100, 2),
        page(101, 100, 0),
        page(0, 100, (MAX_CONVERSATION_PAGE_ITEMS + 1) as u64),
        page(0, 0, 1),
    ];
    for candidate in wrong {
        assert_eq!(
            model.publish_from_host(&ticket, "session-1", candidate),
            Err(ConversationError::InvalidPage)
        );
        assert_eq!(model.snapshot(), baseline);
    }
    let mut duplicate = page(0, 2, 2);
    duplicate.entries[1].message_id = duplicate.entries[0].message_id.clone();
    assert_eq!(
        model.publish_from_host(&ticket, "session-1", duplicate),
        Err(ConversationError::InvalidPage)
    );
    let mut discontinuity = page(0, 2, 2);
    discontinuity.entries[1].position = 99;
    assert_eq!(
        model.publish_from_host(&ticket, "session-1", discontinuity),
        Err(ConversationError::InvalidPage)
    );
    model
        .publish_from_host(&ticket, "session-1", page(2, 10, 2))
        .unwrap();
    let revision = model.snapshot().revision;
    let ticket = request(
        &mut model,
        7,
        1,
        json!({
            "action":"load-older", "expected_revision":revision
        }),
    )
    .unwrap()
    .state
    .pending
    .unwrap();
    let baseline = model.snapshot();
    assert_eq!(
        model.publish_from_host(&ticket, "session-1", page(0, 11, 2)),
        Err(ConversationError::InvalidPage)
    );
    assert_eq!(
        model.publish_from_host(&ticket, "session-1", page(0, 10, 1)),
        Err(ConversationError::InvalidPage)
    );
    assert_eq!(model.snapshot(), baseline);
    model
        .publish_from_host(&ticket, "session-1", page(0, 10, 2))
        .unwrap();
}

#[test]
fn unsafe_ids_and_untrusted_text_fail_closed() {
    let mut model = UiConversation::new(7).unwrap();
    let oversized = "a".repeat(129);
    for key in [
        "",
        "../secrets",
        "/private",
        "https://evil",
        "<img>",
        "x y",
        ".hidden",
        "a\\b",
        "é",
        oversized.as_str(),
    ] {
        assert_eq!(model.open_from_host(key), Err(ConversationError::Key));
    }
    let ticket = model.open_from_host("session-1").unwrap();
    let mut bad = page(0, 1, 1);
    bad.entries[0].text = "abc\u{202e}exe".into();
    assert_eq!(
        model.publish_from_host(&ticket, "session-1", bad),
        Err(ConversationError::InvalidPage)
    );
    let mut bad = page(0, 1, 1);
    bad.entries[0].text = "x".repeat(2049);
    assert_eq!(
        model.publish_from_host(&ticket, "session-1", bad),
        Err(ConversationError::InvalidPage)
    );
    let mut bad = page(0, 1, 1);
    bad.entries[0].message_id = "../private".into();
    assert_eq!(
        model.publish_from_host(&ticket, "session-1", bad),
        Err(ConversationError::Key)
    );
    let mut plain = page(0, 1, 1);
    plain.entries[0].text = "<script>alert('display only')</script>".into();
    model
        .publish_from_host(&ticket, "session-1", plain)
        .unwrap();
    assert!(
        model.snapshot().page.unwrap().entries[0]
            .text
            .contains("<script>")
    );
    // The integration MUST render text as a text node, not unsafe HTML.
}

#[test]
fn malformed_frames_duplicate_fields_and_unauthorized_commands_do_nothing() {
    let mut model = UiConversation::new(7).unwrap();
    let attacks = [
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"execute","code":"rm"}}"#.as_slice(),
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"snapshot","grant":"all"}}"#,
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"snapshot"},"command":{"action":"snapshot"}}"#,
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"snapshot"},"token":"secret"}"#,
        br#"{"protocol":1,"hostEpoch":7,"sequence":1,"command":{"action":"load-older","expected_revision":1,"path":"/etc"}}"#,
        b"{",
    ];
    for payload in attacks {
        assert_eq!(
            model.dispatch_json(payload),
            Err(ConversationError::Malformed)
        );
    }
    assert_eq!(
        model.dispatch_json(&vec![b'x'; MAX_CONVERSATION_REQUEST_BYTES + 1]),
        Err(ConversationError::Oversize)
    );
    request(&mut model, 7, 1, json!({"action":"snapshot"})).unwrap();
}

#[test]
fn rejected_actions_consume_sequence_and_do_not_mutate_pages() {
    let mut model = UiConversation::new(7).unwrap();
    let initial = model.open_from_host("session-1").unwrap();
    model
        .publish_from_host(&initial, "session-1", page(0, 1, 1))
        .unwrap();
    let before = model.snapshot();
    assert_eq!(
        request(
            &mut model,
            7,
            1,
            json!({
                "action":"load-older", "expected_revision":before.revision
            })
        ),
        Err(ConversationError::Boundary)
    );
    assert_eq!(
        request(&mut model, 7, 1, json!({"action":"snapshot"})),
        Err(ConversationError::Sequence)
    );
    assert_eq!(
        request(
            &mut model,
            7,
            2,
            json!({
                "action":"load-newer", "expected_revision":before.revision - 1
            })
        ),
        Err(ConversationError::Revision)
    );
    assert_eq!(model.snapshot(), before);
    request(&mut model, 7, 3, json!({"action":"snapshot"})).unwrap();
}

#[test]
fn async_failure_allows_retry_and_rejects_old_completion() {
    let mut model = UiConversation::new(7).unwrap();
    let initial = model.open_from_host("session-1").unwrap();
    model.fail_from_host(&initial).unwrap();
    assert_eq!(
        model.fail_from_host(&initial),
        Err(ConversationError::Stale)
    );
    let next = model.open_from_host("session-1").unwrap();
    assert_ne!(initial.request_id, next.request_id);
    assert_eq!(
        model.publish_from_host(&initial, "session-1", page(0, 0, 0)),
        Err(ConversationError::Stale)
    );
    model
        .publish_from_host(&next, "session-1", page(0, 0, 0))
        .unwrap();
    assert_eq!(model.snapshot().page.unwrap().total, 0);
}

#[test]
fn thousand_page_windows_keep_memory_bounded() {
    let mut model = UiConversation::new(3).unwrap();
    let ticket = model.open_from_host("synthetic-1").unwrap();
    model
        .publish_from_host(&ticket, "synthetic-1", page(23_976, 24_000, 24))
        .unwrap();
    for i in 0..999_u64 {
        let revision = model.snapshot().revision;
        let next = request(
            &mut model,
            3,
            i + 1,
            json!({
                "action":"load-older", "expected_revision":revision
            }),
        )
        .unwrap()
        .state
        .pending
        .unwrap();
        let start = 23_952 - i * 24;
        model
            .publish_from_host(&next, "synthetic-1", page(start, 24_000, 24))
            .unwrap();
        let page = model.snapshot().page.unwrap();
        assert_eq!(page.entries.len(), 24);
        assert_eq!(page.start, start);
    }
    assert_eq!(model.snapshot().page.unwrap().start, 0);
    let revision = model.snapshot().revision;
    assert_eq!(
        request(
            &mut model,
            3,
            1000,
            json!({
                "action":"load-older", "expected_revision":revision
            })
        ),
        Err(ConversationError::Boundary)
    );
}

#[test]
fn js_integer_and_protocol_bounds() {
    const MAX: u64 = 9_007_199_254_740_991;
    assert!(matches!(
        UiConversation::new(0),
        Err(ConversationError::Epoch)
    ));
    assert!(matches!(
        UiConversation::new(MAX + 1),
        Err(ConversationError::Epoch)
    ));
    let mut model = UiConversation::new(MAX).unwrap();
    let invalid = json!({"protocol":2,"hostEpoch":MAX,"sequence":1,
        "command":{"action":"snapshot"}});
    assert_eq!(
        model.dispatch_json(&serde_json::to_vec(&invalid).unwrap()),
        Err(ConversationError::Protocol)
    );
    assert_eq!(
        request(&mut model, MAX, MAX + 1, json!({"action":"snapshot"})),
        Err(ConversationError::Sequence)
    );
    request(&mut model, MAX, 1, json!({"action":"snapshot"})).unwrap();
    assert_eq!(model.rebind(MAX), Err(ConversationError::Epoch));
}
