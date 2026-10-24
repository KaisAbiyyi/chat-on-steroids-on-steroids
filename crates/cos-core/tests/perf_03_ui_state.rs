//! PERF-03 cloud-only, synthetic: no native renderer, provider or user history.
use cos_core::ui_contract::{UI_PROTOCOL_VERSION, UiCommand, UiNavigation, UiRequest, UiRoute};
use cos_core::ui_conversation::{ConversationEntry, ConversationPage, ConversationRole, UiConversation};
use cos_core::ui_focus::UiFocus;
use cos_core::ui_panels::{PanelCatalog, ProjectItem, UiPanels};
use cos_core::ui_performance::{
    ChatGroup, ChatGroupCatalog, UiFrameCache, UiPerformanceError, MAX_CHAT_GROUPS,
    MAX_VISIBLE_FRAME_BYTES,
};
use cos_core::ui_rich::{RichDeck, RichPanel};

fn navigate(model: &mut UiNavigation, epoch: u64, seq: u64, route: UiRoute) {
    model.dispatch(UiRequest {
        protocol: UI_PROTOCOL_VERSION,
        host_epoch: epoch,
        sequence: seq,
        command: UiCommand::Navigate {
            route,
            expected_revision: model.snapshot().revision,
        },
    }).unwrap();
}

fn set_sidebar(model: &mut UiNavigation, epoch: u64, seq: u64, open: bool) {
    model.dispatch(UiRequest {
        protocol: UI_PROTOCOL_VERSION,
        host_epoch: epoch,
        sequence: seq,
        command: UiCommand::SetSidebar {
            open,
            expected_revision: model.snapshot().revision,
        },
    }).unwrap();
}

#[test]
fn idle_skips_serialization_and_heavy_inactive_routes() {
    let nav = UiNavigation::new(7).unwrap();
    let focus = UiFocus::new(7).unwrap();
    let mut panels = UiPanels::new(7).unwrap();
    let mut rich = RichDeck::new(7).unwrap();
    let mut history = UiConversation::new(7).unwrap();
    let mut catalog = PanelCatalog::default();
    catalog.projects.push(ProjectItem {
        id: "synthetic-project".into(),
        name: "HIDDEN_PROJECT_NAME".into(),
        remote: false,
    });
    panels.replace_catalog(catalog).unwrap();
    rich.publish_from_host(RichPanel::Code {
        key: "secret-code".into(),
        title: "HIDDEN_EDITOR_TITLE".into(),
        language: "rust".into(),
        text: "HIDDEN_CODE_CONTENT".into(),
    }).unwrap();
    let ticket = history.open_from_host("synthetic-session").unwrap();
    history.publish_from_host(&ticket, "synthetic-session", ConversationPage {
        start: 0,
        total: 1,
        entries: vec![ConversationEntry {
            position: 0,
            message_id: "message-0".into(),
            turn_id: None,
            role: ConversationRole::User,
            text: "HIDDEN_HISTORY_TEXT".into(),
            truncated: false,
        }],
    }).unwrap();

    let mut cache = UiFrameCache::new();
    let frame = cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), None,
    ).unwrap().unwrap().to_vec();
    let output = String::from_utf8(frame).unwrap();
    assert!(output.contains("navigation"));
    for secret in ["HIDDEN_PROJECT_NAME", "HIDDEN_EDITOR_TITLE", "HIDDEN_CODE_CONTENT", "HIDDEN_HISTORY_TEXT"] {
        assert!(!output.contains(secret));
    }
    assert!(cache.retained_bytes() <= MAX_VISIBLE_FRAME_BYTES);
    for _ in 0..1024 {
        assert!(cache.refresh(
            &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
            &rich.snapshot(), &history.snapshot(), None,
        ).unwrap().is_none());
    }
    assert_eq!(cache.serialization_count(), 1);
}

#[test]
fn current_conversation_only_and_explicit_cache_revocation() {
    let mut nav = UiNavigation::new(9).unwrap();
    let mut history = UiConversation::new(9).unwrap();
    let focus = UiFocus::new(9).unwrap();
    let panels = UiPanels::new(9).unwrap();
    let rich = RichDeck::new(9).unwrap();
    let ticket = history.open_from_host("synthetic-1").unwrap();
    history.publish_from_host(&ticket, "synthetic-1", ConversationPage {
        start: 99,
        total: 100,
        entries: vec![ConversationEntry {
            position: 99,
            message_id: "message-99".into(),
            turn_id: None,
            role: ConversationRole::Assistant,
            text: "CURRENT_PAGE_VISIBLE".into(),
            truncated: false,
        }],
    }).unwrap();
    navigate(&mut nav, 9, 1, UiRoute::Conversation { session_id: "synthetic-1".into() });
    let mut cache = UiFrameCache::new();
    let bytes = cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), None,
    ).unwrap().unwrap();
    assert!(String::from_utf8_lossy(bytes).contains("CURRENT_PAGE_VISIBLE"));
    assert!(cache.retained_bytes() > 0);
    cache.clear(); // native host logout/revocation hook
    assert_eq!(cache.retained_bytes(), 0);
    assert!(cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), None,
    ).unwrap().is_some());
    navigate(&mut nav, 9, 2, UiRoute::Home {});
    let home = cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), None,
    ).unwrap().unwrap();
    assert!(!String::from_utf8_lossy(home).contains("CURRENT_PAGE_VISIBLE"));

    navigate(&mut nav, 9, 3, UiRoute::Conversation { session_id: "another-session".into() });
    assert_eq!(cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), None,
    ).unwrap_err(), UiPerformanceError::Inconsistent);
    assert_eq!(cache.retained_bytes(), 0);
}

#[test]
fn grouping_is_metadata_only_hidden_until_sidebar_and_validated() {
    let mut nav = UiNavigation::new(11).unwrap();
    let focus = UiFocus::new(11).unwrap();
    let panels = UiPanels::new(11).unwrap();
    let rich = RichDeck::new(11).unwrap();
    let history = UiConversation::new(11).unwrap();
    let groups = ChatGroupCatalog {
        host_epoch: 11,
        revision: 1,
        groups: vec![ChatGroup {
            id: "group-1".into(),
            title: "synthetic group".into(),
            session_ids: vec!["synthetic-session-1".into()],
        }],
    };
    let mut cache = UiFrameCache::new();
    let hidden = cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), Some(&groups),
    ).unwrap().unwrap();
    assert!(!String::from_utf8_lossy(hidden).contains("synthetic-session-1"));
    set_sidebar(&mut nav, 11, 1, true);
    let shown = cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), Some(&groups),
    ).unwrap().unwrap();
    assert!(String::from_utf8_lossy(shown).contains("synthetic-session-1"));
    let mut bad = groups.clone();
    bad.revision += 1;
    bad.groups[0].session_ids.push("synthetic-session-1".into());
    assert_eq!(cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), Some(&bad),
    ).unwrap_err(), UiPerformanceError::InvalidGroups);
    assert_eq!(cache.retained_bytes(), 0);

    bad.groups = (0..=MAX_CHAT_GROUPS).map(|i| ChatGroup {
        id: format!("group-{i}"), title: "test".into(), session_ids: vec![],
    }).collect();
    assert_eq!(bad.validate(11), Err(UiPerformanceError::InvalidGroups));
    assert_eq!(groups.validate(12), Err(UiPerformanceError::InvalidGroups));
}

#[test]
fn new_renderer_epoch_requires_new_authoritative_state() {
    let mut nav = UiNavigation::new(13).unwrap();
    let mut focus = UiFocus::new(13).unwrap();
    let mut panels = UiPanels::new(13).unwrap();
    let mut rich = RichDeck::new(13).unwrap();
    let mut history = UiConversation::new(13).unwrap();
    let mut cache = UiFrameCache::new();
    assert!(cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), None,
    ).unwrap().is_some());
    nav.rebind(14).unwrap();
    assert_eq!(cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), None,
    ).unwrap_err(), UiPerformanceError::Inconsistent);
    assert_eq!(cache.retained_bytes(), 0);
    focus.rebind(14).unwrap();
    panels.rebind(14).unwrap();
    rich.rebind(14).unwrap();
    history.rebind(14).unwrap();
    assert!(cache.refresh(
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &history.snapshot(), None,
    ).unwrap().is_some());
    assert!(cache.retained_bytes() <= MAX_VISIBLE_FRAME_BYTES);
}
