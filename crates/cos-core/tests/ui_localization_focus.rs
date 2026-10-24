//! Phase 15E synthetic cloud regressions; native keyboard, IME and UIA NOT_RUN.
use cos_core::ui_focus::{
    UiFocus, UiFocusError, UiFocusTarget, UiKeyEvent, UiKeyFrame, UiKeyOutcome,
};
use cos_core::ui_localization::{text, UiLanguage, UiTextKey};
use serde_json::{json, Value};

fn key(epoch: u64, sequence: u64, event: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "hostEpoch": epoch, "sequence": sequence, "event": event
    }))
    .unwrap()
}

fn tab(shift: bool) -> Value {
    json!({
        "key":"tab", "shift":shift, "ctrl":false, "alt":false,
        "meta":false, "composing":false
    })
}

#[test]
fn all_thirteen_canonical_languages_have_safe_nonempty_shell_keys() {
    let upstream = [
        "en", "de", "es", "fr", "pt-BR", "pt-PT", "ru", "tr", "vi", "ja",
        "ko", "zh-CN", "zh-TW",
    ];
    assert_eq!(UiLanguage::ALL.len(), upstream.len());
    for (lang, tag) in UiLanguage::ALL.into_iter().zip(upstream) {
        assert_eq!(lang.tag(), tag);
        assert_eq!(UiLanguage::parse(tag), Some(lang));
        for key in UiTextKey::ALL {
            let s = text(lang, key);
            assert!(!s.trim().is_empty(), "{tag}: {key:?}");
            assert!(
                !s.chars().any(|ch| matches!(ch, '<' | '>' | '\0')),
                "{tag}: {key:?}"
            );
        }
    }
    assert_eq!(text(UiLanguage::PtBr, UiTextKey::Files), "Arquivos");
    assert_eq!(text(UiLanguage::PtPt, UiTextKey::Files), "Ficheiros");
    assert_ne!(
        text(UiLanguage::ZhCn, UiTextKey::Projects),
        text(UiLanguage::ZhTw, UiTextKey::Projects)
    );
}

#[test]
fn unsupported_malformed_or_unset_locale_falls_back_exactly_to_english() {
    let invalid = [
        None, Some(""), Some("EN"), Some("en-US"), Some("pt-br"),
        Some("ar"), Some("id"), Some("../en"), Some("<script>"),
    ];
    for value in invalid {
        assert_eq!(UiLanguage::select(value), UiLanguage::En);
    }
    assert_eq!(UiLanguage::select(Some("pt-BR")), UiLanguage::PtBr);
    assert_eq!(UiLanguage::select(Some("pt-PT")), UiLanguage::PtPt);
}

#[test]
fn tab_cycles_forward_backward_without_phantom_revisions() {
    let mut focus = UiFocus::new(9).unwrap();
    for (i, target) in [
        UiFocusTarget::Main, UiFocusTarget::Composer, UiFocusTarget::Navigation,
    ]
    .into_iter()
    .enumerate()
    {
        let reply = focus
            .handle_json(&key(9, i as u64 + 1, tab(false)))
            .unwrap();
        assert_eq!(reply.outcome, UiKeyOutcome::Handled);
        assert_eq!(reply.state.target, target);
        assert_eq!(reply.state.revision, i as u64 + 2);
    }
    assert_eq!(
        focus.handle_json(&key(9, 4, tab(true))).unwrap().state.target,
        UiFocusTarget::Composer
    );
    let current = focus.snapshot();
    focus.set_host_focus(UiFocusTarget::Composer).unwrap();
    assert_eq!(focus.snapshot(), current);
}

#[test]
fn modal_traps_navigation_and_restores_original_focus_on_escape() {
    let mut focus = UiFocus::new(10).unwrap();
    focus.set_host_focus(UiFocusTarget::Composer).unwrap();
    focus.open_dialog().unwrap();
    assert_eq!(focus.snapshot().target, UiFocusTarget::DialogConfirm);
    assert_eq!(
        focus.set_host_focus(UiFocusTarget::Main),
        Err(UiFocusError::Focus)
    );
    assert_eq!(focus.open_dialog(), Err(UiFocusError::Dialog));
    assert_eq!(
        focus.handle_json(&key(10, 1, tab(false))).unwrap().state.target,
        UiFocusTarget::DialogCancel
    );
    assert_eq!(
        focus.handle_json(&key(10, 2, tab(true))).unwrap().state.target,
        UiFocusTarget::DialogConfirm
    );
    let reply = focus
        .handle_json(&key(10, 3, json!({"key":"escape","composing":false})))
        .unwrap();
    assert_eq!(reply.outcome, UiKeyOutcome::Handled);
    assert_eq!(reply.state.target, UiFocusTarget::Composer);
    assert!(!reply.state.dialog_open);
    assert_eq!(focus.close_dialog(), Err(UiFocusError::Dialog));
    assert_eq!(
        focus.set_host_focus(UiFocusTarget::DialogCancel),
        Err(UiFocusError::Focus)
    );
}

#[test]
fn composition_modified_keys_and_nonmodal_escape_pass_through() {
    let mut focus = UiFocus::new(3).unwrap();
    let initial = focus.snapshot();
    let events = [
        json!({"key":"tab","shift":false,"ctrl":false,"alt":false,"meta":false,"composing":true}),
        json!({"key":"tab","shift":false,"ctrl":true,"alt":false,"meta":false,"composing":false}),
        json!({"key":"tab","shift":false,"ctrl":false,"alt":true,"meta":false,"composing":false}),
        json!({"key":"tab","shift":false,"ctrl":false,"alt":false,"meta":true,"composing":false}),
        json!({"key":"escape","composing":false}),
    ];
    for (i, event) in events.into_iter().enumerate() {
        let reply = focus
            .handle_json(&key(3, i as u64 + 1, event))
            .unwrap();
        assert_eq!(reply.outcome, UiKeyOutcome::PassThrough);
        assert_eq!(reply.state, initial);
    }
    focus.open_dialog().unwrap();
    let modal = focus.snapshot();
    let during_ime = key(3, 6, json!({"key":"escape","composing":true}));
    assert_eq!(focus.handle_json(&during_ime).unwrap().state, modal);
}

#[test]
fn replay_stale_unknown_oversize_and_duplicate_events_fail_closed() {
    let mut focus = UiFocus::new(7).unwrap();
    let first = key(7, 1, tab(false));
    focus.handle_json(&first).unwrap();
    assert_eq!(focus.handle_json(&first).unwrap_err(), UiFocusError::Sequence);
    assert_eq!(
        focus.handle_json(&key(6, 2, tab(false))).unwrap_err(),
        UiFocusError::Epoch
    );
    assert_eq!(
        focus.handle_json(&key(7, 3, tab(false))).unwrap_err(),
        UiFocusError::Sequence
    );
    let snapshot = focus.snapshot();
    let bad = [
        br#"{"hostEpoch":7,"sequence":2,"event":{"key":"execute","script":"x"}}"#.as_slice(),
        br#"{"hostEpoch":7,"sequence":2,"event":{"key":"escape","composing":false,"grant":true}}"#.as_slice(),
        br#"{"hostEpoch":7,"sequence":2,"event":{"key":"tab","shift":false}}"#.as_slice(),
        br#"{"hostEpoch":7,"sequence":2,"sequence":3,"event":{"key":"escape","composing":false}}"#.as_slice(),
    ];
    for payload in bad {
        assert_eq!(focus.handle_json(payload).unwrap_err(), UiFocusError::Malformed);
    }
    assert_eq!(
        focus.handle_json(&vec![b' '; 8193]).unwrap_err(),
        UiFocusError::Oversize
    );
    assert_eq!(focus.snapshot(), snapshot);
    focus.handle_json(&key(7, 2, tab(false))).unwrap();
}

#[test]
fn host_rebind_retires_modal_focus_and_old_keyboard_epoch() {
    assert!(UiFocus::new(0).is_err());
    let mut focus = UiFocus::new(20).unwrap();
    focus.open_dialog().unwrap();
    assert_eq!(focus.rebind(20), Err(UiFocusError::Epoch));
    focus.rebind(21).unwrap();
    assert!(!focus.snapshot().dialog_open);
    assert_eq!(focus.snapshot().target, UiFocusTarget::Navigation);
    assert_eq!(
        focus.handle_json(&key(20, 1, tab(false))),
        Err(UiFocusError::Epoch)
    );
    assert_eq!(
        focus.handle(UiKeyFrame {
            host_epoch: 21,
            sequence: 1,
            event: UiKeyEvent::Escape { composing: false },
        })
        .unwrap()
        .outcome,
        UiKeyOutcome::PassThrough
    );
}
