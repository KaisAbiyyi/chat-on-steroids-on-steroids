//! 15G cloud-only integration and synthetic bounded-state/performance proof.
//! No real graphics, installed desktop, user profiles or OS resource metrics.
use std::hint::black_box;
use std::time::Instant;

use cos_core::ui_contract::{UI_PROTOCOL_VERSION, UiCommand, UiNavigation, UiRequest, UiRoute};
use cos_core::ui_conversation::{
    ConversationEntry, ConversationError, ConversationPage, ConversationRole, UiConversation,
};
use cos_core::ui_focus::UiFocus;
use cos_core::ui_integration::{
    DesktopAudit, DesktopAuditError, MAX_DESKTOP_AUDIT_BYTES,
};
use cos_core::ui_localization::UiLanguage;
use cos_core::ui_panels::{
    Panel, PanelCatalog, PanelCommand, PanelRequest, ProjectItem, Theme, UiPanels,
};
use cos_core::ui_rich::{RichDeck, RichPanel};
use cos_core::ui_snapshots::{
    CaptureConfig, CaptureDescriptor, CaptureSurface, FIXED_CAPTURE_DPI,
    FIXED_CAPTURE_VIEWPORTS, RouteKind,
};

struct Fixture {
    nav: UiNavigation,
    history: UiConversation,
    focus: UiFocus,
    panels: UiPanels,
    rich: RichDeck,
    epoch: u64,
    nav_seq: u64,
    panel_seq: u64,
}

impl Fixture {
    fn new(epoch: u64) -> Self {
        Self {
            nav: UiNavigation::new(epoch).unwrap(),
            history: UiConversation::new(epoch).unwrap(),
            focus: UiFocus::new(epoch).unwrap(),
            panels: UiPanels::new(epoch).unwrap(),
            rich: RichDeck::new(epoch).unwrap(),
            epoch,
            nav_seq: 0,
            panel_seq: 0,
        }
    }

    fn audit(&self) -> Result<DesktopAudit, DesktopAuditError> {
        DesktopAudit::inspect(
            &self.nav.snapshot(),
            &self.focus.snapshot(),
            &self.panels.snapshot(),
            &self.rich.snapshot(),
            &self.history.snapshot(),
        )
    }

    fn navigate(&mut self, route: UiRoute) {
        self.nav_seq += 1;
        self.nav
            .dispatch(UiRequest {
                protocol: UI_PROTOCOL_VERSION,
                host_epoch: self.epoch,
                sequence: self.nav_seq,
                command: UiCommand::Navigate {
                    route,
                    expected_revision: self.nav.snapshot().revision,
                },
            })
            .unwrap();
    }

    fn panel(&mut self, panel: Panel) {
        self.panel_seq += 1;
        self.panels
            .dispatch(PanelRequest {
                protocol: UI_PROTOCOL_VERSION,
                host_epoch: self.epoch,
                sequence: self.panel_seq,
                command: PanelCommand::OpenPanel {
                    panel,
                    expected_revision: self.panels.snapshot().revision,
                },
            })
            .unwrap();
    }
}

fn synthetic_page(start: u64, total: u64, count: usize, bytes: usize) -> ConversationPage {
    ConversationPage {
        start,
        total,
        entries: (0..count)
            .map(|i| ConversationEntry {
                position: start + i as u64,
                message_id: format!("synthetic-message-{i}"),
                turn_id: None,
                role: ConversationRole::Assistant,
                text: "x".repeat(bytes),
                truncated: false,
            })
            .collect(),
    }
}

#[test]
fn route_panel_convergence_and_rejection_are_explicit() {
    let mut host = Fixture::new(7);
    assert_eq!(host.audit().unwrap().projection.route, RouteKind::Home);
    for (route, panel) in [
        (UiRoute::Projects {}, Panel::Projects),
        (UiRoute::Files { project_id: "synthetic-project".into() }, Panel::Files),
        (UiRoute::Agents {}, Panel::Agents),
        (UiRoute::Skills {}, Panel::Skills),
        (UiRoute::Plugins {}, Panel::Plugins),
        (UiRoute::Settings {}, Panel::Settings),
    ] {
        host.navigate(route);
        if host.panels.snapshot().panel != panel {
            assert_eq!(host.audit(), Err(DesktopAuditError::PanelRouteMismatch));
        }
        host.panel(panel);
        let report = host.audit().unwrap();
        assert_eq!(report.projection.panel, panel);
        assert!(report.retained_serialized_bytes < MAX_DESKTOP_AUDIT_BYTES);
    }
}

#[test]
fn authorized_conversation_page_and_modal_report_only_visible_metadata() {
    let mut host = Fixture::new(7);
    let ticket = host.history.open_from_host("session-PRIVATE-opaque").unwrap();
    let mut page = synthetic_page(100, 120, 20, 42);
    page.entries[0].text = "PRIVATE-TEXT-DO-NOT-LOG".into();
    host.history
        .publish_from_host(&ticket, "session-PRIVATE-opaque", page)
        .unwrap();
    host.rich
        .publish_from_host(RichPanel::Editor {
            key: "secret-editor".into(),
            title: "PRIVATE-FILE".into(),
            language: "rust".into(),
            text: "PRIVATE-SOURCE".into(),
            read_only: true,
        })
        .unwrap();
    host.navigate(UiRoute::Conversation { session_id: "session-PRIVATE-opaque".into() });
    host.focus.open_dialog().unwrap();
    let report = host.audit().unwrap();
    assert_eq!(report.visible_conversation_rows, 20);
    assert!(report.projection.dialog_open);
    assert_eq!(report.projection.rich_count, 1);
    assert!(!report.history_pending);
    let safe = format!("{report:?}");
    for hidden in ["PRIVATE", "secret-editor", "synthetic-message"] {
        assert!(!safe.contains(hidden), "audit leaked private source text");
    }

    // Switching away hides history counts; host must still retire private
    // session state on its own lifecycle/authorization boundary.
    host.navigate(UiRoute::Home {});
    assert_eq!(host.audit().unwrap().visible_conversation_rows, 0);
    host.navigate(UiRoute::Conversation { session_id: "another-session".into() });
    assert_eq!(host.audit(), Err(DesktopAuditError::InconsistentHosts));
}

#[test]
fn mixed_epochs_rebind_and_async_history_staleness_fail_closed() {
    let mut host = Fixture::new(17);
    let stale = host.history.open_from_host("old-session").unwrap();
    host.focus.open_dialog().unwrap();
    host.rich
        .publish_from_host(RichPanel::Terminal {
            key: "term".into(), title: "Output".into(), output: "old-state".into()
        })
        .unwrap();
    host.nav.rebind(18).unwrap();
    assert_eq!(host.audit(), Err(DesktopAuditError::InconsistentHosts));
    host.history.rebind(18).unwrap();
    host.panels.rebind(18).unwrap();
    host.focus.rebind(18).unwrap();
    host.rich.rebind(18).unwrap();
    host.epoch = 18;
    host.nav_seq = 0;
    host.panel_seq = 0;
    assert_eq!(
        host.history.publish_from_host(&stale, "old-session", synthetic_page(0, 1, 1, 1)),
        Err(ConversationError::Stale),
    );
    let current = host.audit().unwrap();
    assert_eq!(current.projection.route, RouteKind::Home);
    assert!(!current.projection.dialog_open);
    assert_eq!(current.projection.rich_count, 0);
    assert_eq!(current.visible_conversation_rows, 0);
    assert_eq!(host.history.snapshot().session_id, None);
    assert!(host.panels.snapshot().catalog.projects.is_empty());
}

#[test]
fn bounded_large_history_catalog_and_rich_panels_are_measured_without_private_receipts() {
    let mut host = Fixture::new(7);
    let ticket = host.history.open_from_host("synthetic-session").unwrap();
    host.history
        .publish_from_host(
            &ticket,
            "synthetic-session",
            synthetic_page(4000, 100_000, 24, 1900),
        )
        .unwrap();
    let mut catalog = PanelCatalog::default();
    for i in 0..240 {
        catalog.projects.push(ProjectItem {
            id: format!("synthetic-{i}"),
            name: format!("Synthetic {}", "p".repeat(110)),
            remote: false,
        });
    }
    host.panels.replace_catalog(catalog).unwrap();
    for i in 0..4 {
        host.rich
            .publish_from_host(RichPanel::Code {
                key: format!("code-{i}"),
                title: "Synthetic Code".into(),
                language: "rust".into(),
                text: "r".repeat(5_000),
            })
            .unwrap();
    }
    host.navigate(UiRoute::Conversation { session_id: "synthetic-session".into() });
    let report = host.audit().unwrap();
    assert_eq!(report.visible_conversation_rows, 24);
    assert!(report.projection.conversation_has_older);
    assert!(report.projection.conversation_has_newer);
    assert_eq!(report.catalog_items, 240);
    assert!(report.retained_serialized_bytes < MAX_DESKTOP_AUDIT_BYTES);

    let nav = host.nav.snapshot();
    let focus = host.focus.snapshot();
    let panels = host.panels.snapshot();
    let rich = host.rich.snapshot();
    let history = host.history.snapshot();
    let start = Instant::now();
    let mut last = 0;
    for _ in 0..256 {
        let audit = black_box(
            DesktopAudit::inspect(&nav, &focus, &panels, &rich, &history).unwrap(),
        );
        last = audit.retained_serialized_bytes;
    }
    assert_eq!(last, report.retained_serialized_bytes);
    // Hosted CI wall time is only a diagnostic, NEVER an FPS/RSS acceptance
    // threshold. The deterministic bound is the serialized byte count.
    eprintln!("15G synthetic audit 256 iterations; bytes={last}; elapsed_ms={}", start.elapsed().as_millis());
}

#[test]
fn forged_unbounded_catalog_and_pending_generation_are_rejected() {
    let host = Fixture::new(7);
    let nav = host.nav.snapshot();
    let focus = host.focus.snapshot();
    let rich = host.rich.snapshot();
    let history = host.history.snapshot();
    let mut panels = host.panels.snapshot();
    panels.catalog.projects = (0..257).map(|i| ProjectItem {
        id: format!("synthetic-{i}"), name: "Synthetic".into(), remote: false
    }).collect();
    assert_eq!(
        DesktopAudit::inspect(&nav, &focus, &panels, &rich, &history),
        Err(DesktopAuditError::Oversize)
    );

    let mut host = Fixture::new(7);
    let ticket = host.history.open_from_host("synthetic").unwrap();
    let mut snapshot = host.history.snapshot();
    snapshot.pending.as_mut().unwrap().session_generation = ticket.session_generation + 1;
    assert_eq!(
        DesktopAudit::inspect(
            &host.nav.snapshot(), &host.focus.snapshot(),
            &host.panels.snapshot(), &host.rich.snapshot(), &snapshot
        ),
        Err(DesktopAuditError::InconsistentHosts)
    );
}

#[test]
fn all_156_synthetic_appearance_contracts_share_consistent_desktop_snapshot() {
    let mut host = Fixture::new(7);
    let mut cases = 0;
    for (width, height) in FIXED_CAPTURE_VIEWPORTS {
        for dpi in FIXED_CAPTURE_DPI {
            for theme in [Theme::Light, Theme::Dark] {
                for language in UiLanguage::ALL {
                    let mut catalog = PanelCatalog::default();
                    catalog.preferences.locale = language.tag().to_owned();
                    catalog.preferences.theme = theme;
                    host.panels.replace_catalog(catalog).unwrap();
                    let cfg = CaptureConfig::new(width, height, dpi, theme, language).unwrap();
                    let descriptor = CaptureDescriptor::new(
                        cfg, CaptureSurface::Shell,
                        &host.nav.snapshot(), &host.focus.snapshot(),
                        &host.panels.snapshot(), &host.rich.snapshot(),
                        &host.history.snapshot()
                    ).unwrap();
                    assert_eq!(descriptor.projection, host.audit().unwrap().projection);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 156);
}
