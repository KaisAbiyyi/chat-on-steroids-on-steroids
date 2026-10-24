//! 15F synthetic cloud regressions. Actual native screenshot acceptance is NOT_RUN.
use cos_core::ui_contract::{UiCommand, UiNavigation, UiRequest, UiRoute};
use cos_core::ui_conversation::UiConversation;
use cos_core::ui_focus::{UiFocus, UiFocusTarget};
use cos_core::ui_localization::UiLanguage;
use cos_core::ui_panels::{PanelCatalog, Theme, UiPanels};
use cos_core::ui_rich::{RichDeck, RichPanel};
use cos_core::ui_snapshots::{
    CaptureConfig, CaptureDescriptor, CaptureError, CaptureSurface,
    FIXED_CAPTURE_DPI, FIXED_CAPTURE_VIEWPORTS, MAX_CAPTURE_RGBA_BYTES,
    RouteKind, RichKind, VisualProjection,
};

fn config(dpi: u16, theme: Theme, lang: UiLanguage) -> CaptureConfig {
    CaptureConfig::new(800, 600, dpi, theme, lang).unwrap()
}

fn shell(
    dpi: u16,
    theme: Theme,
    lang: UiLanguage,
) -> CaptureDescriptor {
    let nav = UiNavigation::new(7).unwrap();
    let focus = UiFocus::new(7).unwrap();
    let mut panels = UiPanels::new(7).unwrap();
    let rich = RichDeck::new(7).unwrap();
    let conversation = UiConversation::new(7).unwrap();
    let mut catalog = PanelCatalog::default();
    catalog.preferences.locale = lang.tag().to_owned();
    catalog.preferences.theme = theme;
    panels.replace_catalog(catalog).unwrap();
    CaptureDescriptor::new(
        config(dpi, theme, lang),
        CaptureSurface::Shell,
        &nav.snapshot(),
        &focus.snapshot(),
        &panels.snapshot(),
        &rich.snapshot(),
        &conversation.snapshot(),
    )
    .unwrap()
}

fn pixels(config: CaptureConfig, rgba: [u8; 4]) -> Vec<u8> {
    rgba.repeat(config.rgba_bytes().unwrap() / 4)
}

#[test]
fn fixed_dpi_and_theme_locale_matrix_has_exact_dimensions_and_finite_budget() {
    let mut cases = 0;
    for (width, height) in FIXED_CAPTURE_VIEWPORTS {
        for dpi in FIXED_CAPTURE_DPI {
            for theme in [Theme::Light, Theme::Dark] {
                for lang in UiLanguage::ALL {
                    let cfg = CaptureConfig::new(width, height, dpi, theme, lang).unwrap();
                    let (physical_width, physical_height) = cfg.physical_dimensions();
                    assert_eq!(physical_width, u32::from(width) * u32::from(dpi) / 96);
                    assert_eq!(physical_height, u32::from(height) * u32::from(dpi) / 96);
                    assert!(cfg.rgba_bytes().unwrap() <= MAX_CAPTURE_RGBA_BYTES);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 156); // 2 viewports x 3 DPIs x 2 themes x 13 locales
    assert_eq!(config(144, Theme::Light, UiLanguage::En).physical_dimensions(), (1200, 900));
    assert_eq!(config(192, Theme::Dark, UiLanguage::Ja).physical_dimensions(), (1600, 1200));
}

#[test]
fn invalid_viewport_dpi_or_unresolved_system_theme_is_rejected() {
    for (w, h, dpi, theme) in [
        (799, 600, 96, Theme::Light),
        (800, 601, 96, Theme::Light),
        (800, 600, 0, Theme::Light),
        (800, 600, 120, Theme::Dark),
        (800, 600, 65535, Theme::Dark),
        (800, 600, 96, Theme::System),
        (u16::MAX, u16::MAX, 192, Theme::Light),
    ] {
        assert_eq!(
            CaptureConfig::new(w, h, dpi, theme, UiLanguage::En),
            Err(CaptureError::InvalidConfig)
        );
    }
}

#[test]
fn deterministic_pixel_hash_and_changed_pixel_are_not_silently_rebaselined() {
    let descriptor = shell(96, Theme::Light, UiLanguage::En);
    let mut rgba = pixels(descriptor.config, [17, 34, 51, 255]);
    let frozen = descriptor.fingerprint(&rgba).unwrap();
    assert_eq!(frozen.sha256.len(), 64);
    let unchanged = descriptor.fingerprint(&rgba).unwrap();
    frozen.verify(&unchanged).unwrap();
    rgba[0] ^= 1; // 1 channel, 1 pixel out of 480,000
    let mutated = descriptor.fingerprint(&rgba).unwrap();
    assert_eq!(frozen.verify(&mutated), Err(CaptureError::ChangedPixels));
    // No configurable pixel tolerance may silently mask a regression.
    assert_ne!(frozen.sha256, mutated.sha256);
}

#[test]
fn mismatched_theme_dpi_or_locale_cannot_pass_a_baseline() {
    let source = shell(96, Theme::Light, UiLanguage::En);
    let golden = source.fingerprint(&pixels(source.config, [1, 2, 3, 255])).unwrap();
    for candidate in [
        shell(144, Theme::Light, UiLanguage::En),
        shell(96, Theme::Dark, UiLanguage::En),
        shell(96, Theme::Light, UiLanguage::De),
    ] {
        let observed = candidate.fingerprint(&pixels(candidate.config, [1, 2, 3, 255])).unwrap();
        assert_eq!(golden.verify(&observed), Err(CaptureError::ChangedContract));
    }
}

#[test]
fn transparent_hidden_rgb_truncated_or_oversized_capture_is_invalid() {
    let descriptor = shell(96, Theme::Light, UiLanguage::En);
    let mut rgba = pixels(descriptor.config, [5, 6, 7, 255]);
    assert_eq!(descriptor.fingerprint(&rgba[..rgba.len() - 4]), Err(CaptureError::InvalidPixels));
    rgba.push(0);
    assert_eq!(descriptor.fingerprint(&rgba), Err(CaptureError::InvalidPixels));
    rgba.pop();
    rgba[3] = 0; // hidden RGB must not enter golden artifact
    assert_eq!(descriptor.fingerprint(&rgba), Err(CaptureError::InvalidPixels));
    rgba[3] = 255;
    assert!(descriptor.fingerprint(&rgba).is_ok());
}

#[test]
fn mixed_host_epochs_and_stale_revisions_reject_capture_before_pixels() {
    let mut nav = UiNavigation::new(7).unwrap();
    let focus = UiFocus::new(7).unwrap();
    let panels = UiPanels::new(7).unwrap();
    let rich = RichDeck::new(7).unwrap();
    let conversation = UiConversation::new(7).unwrap();
    let mut nav_state = nav.snapshot();
    nav_state.revision = 0;
    assert_eq!(
        VisualProjection::from_hosts(
            &nav_state, &focus.snapshot(), &panels.snapshot(),
            &rich.snapshot(), &conversation.snapshot()
        ),
        Err(CaptureError::InconsistentState)
    );
    nav.rebind(8).unwrap();
    assert_eq!(
        VisualProjection::from_hosts(
            &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
            &rich.snapshot(), &conversation.snapshot()
        ),
        Err(CaptureError::InconsistentState)
    );
}

#[test]
fn route_must_match_authorized_conversation_and_projection_drops_private_ids() {
    let mut nav = UiNavigation::new(7).unwrap();
    let focus = UiFocus::new(7).unwrap();
    let panels = UiPanels::new(7).unwrap();
    let rich = RichDeck::new(7).unwrap();
    let mut convo = UiConversation::new(7).unwrap();
    nav.dispatch(UiRequest {
        protocol: 1,
        host_epoch: 7,
        sequence: 1,
        command: UiCommand::Navigate {
            route: UiRoute::Conversation { session_id: "private-session-id".into() },
            expected_revision: 1,
        },
    }).unwrap();
    assert_eq!(
        CaptureDescriptor::new(config(96, Theme::Light, UiLanguage::En),
            CaptureSurface::Conversation, &nav.snapshot(), &focus.snapshot(),
            &panels.snapshot(), &rich.snapshot(), &convo.snapshot()),
        Err(CaptureError::InconsistentState)
    );
    convo.open_from_host("private-session-id").unwrap();
    let descriptor = CaptureDescriptor::new(
        config(96, Theme::Light, UiLanguage::En),
        CaptureSurface::Conversation, &nav.snapshot(), &focus.snapshot(),
        &panels.snapshot(), &rich.snapshot(), &convo.snapshot(),
    ).unwrap();
    assert_eq!(descriptor.projection.route, RouteKind::Conversation);
    assert_eq!(descriptor.projection.conversation_rows, 0);
    // Source identifiers and typed panel text never enter persisted proofs.
    let proof = descriptor.fingerprint(&pixels(descriptor.config, [7, 8, 9, 255])).unwrap();
    let safe_debug = format!("{proof:?}");
    assert!(!safe_debug.contains("private-session-id"));
    assert!(!safe_debug.contains("password"));
}

#[test]
fn rich_focus_and_modal_surface_constraints_remain_source_bound() {
    let nav = UiNavigation::new(7).unwrap();
    let mut focus = UiFocus::new(7).unwrap();
    let panels = UiPanels::new(7).unwrap();
    let mut rich = RichDeck::new(7).unwrap();
    let conversation = UiConversation::new(7).unwrap();
    let cfg = config(96, Theme::Light, UiLanguage::En);
    assert_eq!(
        CaptureDescriptor::new(cfg, CaptureSurface::Rich, &nav.snapshot(),
            &focus.snapshot(), &panels.snapshot(), &rich.snapshot(), &conversation.snapshot()),
        Err(CaptureError::WrongSurface)
    );
    assert_eq!(
        CaptureDescriptor::new(cfg, CaptureSurface::Modal, &nav.snapshot(),
            &focus.snapshot(), &panels.snapshot(), &rich.snapshot(), &conversation.snapshot()),
        Err(CaptureError::WrongSurface)
    );
    rich.publish_from_host(RichPanel::Math {
        key: "fixture-1".into(), title: "Fixture only".into(), source: "x^2".into(),
    }).unwrap();
    let rich_case = CaptureDescriptor::new(cfg, CaptureSurface::Rich, &nav.snapshot(),
        &focus.snapshot(), &panels.snapshot(), &rich.snapshot(), &conversation.snapshot()).unwrap();
    assert_eq!(rich_case.projection.rich_active, Some(RichKind::Math));
    focus.open_dialog().unwrap();
    let modal = CaptureDescriptor::new(cfg, CaptureSurface::Modal, &nav.snapshot(),
        &focus.snapshot(), &panels.snapshot(), &rich.snapshot(), &conversation.snapshot()).unwrap();
    assert!(modal.projection.dialog_open);
    assert_eq!(modal.projection.focus, UiFocusTarget::DialogConfirm);
    let mut bad_focus = focus.snapshot();
    bad_focus.dialog_open = false;
    assert_eq!(
        VisualProjection::from_hosts(&nav.snapshot(), &bad_focus,
            &panels.snapshot(), &rich.snapshot(), &conversation.snapshot()),
        Err(CaptureError::InconsistentState)
    );
    focus.close_dialog().unwrap();
    assert_eq!(focus.snapshot().target, UiFocusTarget::Navigation);
}

#[test]
fn preference_theme_and_locale_must_match_fixed_capture_environment() {
    let nav = UiNavigation::new(7).unwrap();
    let focus = UiFocus::new(7).unwrap();
    let rich = RichDeck::new(7).unwrap();
    let convo = UiConversation::new(7).unwrap();
    let mut panels = UiPanels::new(7).unwrap();
    let mut catalog = PanelCatalog::default();
    catalog.preferences.theme = Theme::Dark;
    catalog.preferences.locale = "ja".into();
    panels.replace_catalog(catalog).unwrap();
    for cfg in [config(96, Theme::Light, UiLanguage::Ja),
        config(96, Theme::Dark, UiLanguage::En)] {
        assert_eq!(
            CaptureDescriptor::new(cfg, CaptureSurface::Shell, &nav.snapshot(),
                &focus.snapshot(), &panels.snapshot(), &rich.snapshot(), &convo.snapshot()),
            Err(CaptureError::InvalidConfig)
        );
    }
    let cfg = config(96, Theme::Dark, UiLanguage::Ja);
    CaptureDescriptor::new(cfg, CaptureSurface::Shell, &nav.snapshot(),
        &focus.snapshot(), &panels.snapshot(), &rich.snapshot(), &convo.snapshot()).unwrap();
}

#[test]
fn structural_change_fails_even_when_pixel_bytes_identical() {
    let nav = UiNavigation::new(7).unwrap();
    let mut focus = UiFocus::new(7).unwrap();
    let panels = UiPanels::new(7).unwrap();
    let rich = RichDeck::new(7).unwrap();
    let convo = UiConversation::new(7).unwrap();
    let cfg = config(96, Theme::Light, UiLanguage::En);
    let baseline = CaptureDescriptor::new(cfg, CaptureSurface::Shell,
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &convo.snapshot()).unwrap();
    focus.set_host_focus(UiFocusTarget::Main).unwrap();
    let candidate = CaptureDescriptor::new(cfg, CaptureSurface::Shell,
        &nav.snapshot(), &focus.snapshot(), &panels.snapshot(),
        &rich.snapshot(), &convo.snapshot()).unwrap();
    let rgba = pixels(cfg, [0, 0, 0, 255]);
    let before = baseline.fingerprint(&rgba).unwrap();
    let after = candidate.fingerprint(&rgba).unwrap();
    assert_eq!(before.sha256, after.sha256);
    assert_eq!(before.verify(&after), Err(CaptureError::ChangedContract));
}
