//! Phase 15F: fixed-DPI/theme image regression contracts for the UI host.
//!
//! The separately authenticated native renderer supplies opaque RGBA8 pixels;
//! this module never captures a screen, reads a profile, persists pixel data,
//! generates a synthetic "native screenshot", or grants renderer authority.
//! Source-bound metadata contains no session IDs, filenames or user text.
//! Golden proofs MUST be created from approved, synthetic fixture captures and
//! checked in separately: generating a golden from the candidate under test
//! would make a regression test meaningless.
use sha2::{Digest, Sha256};

use crate::ui_contract::{MAX_JS_SAFE_INTEGER, UiRoute, UiSnapshot};
use crate::ui_conversation::{ConversationSnapshot, MAX_CONVERSATION_PAGE_ITEMS};
use crate::ui_focus::{UiFocusSnapshot, UiFocusTarget};
use crate::ui_localization::UiLanguage;
use crate::ui_panels::{Panel, PanelSnapshot, Theme};
use crate::ui_rich::{MAX_RICH_PANELS, RichPanel, RichSnapshot};

pub const UI_CAPTURE_VERSION: u32 = 1;
pub const MAX_CAPTURE_RGBA_BYTES: usize = 16 * 1024 * 1024;
pub const FIXED_CAPTURE_DPI: [u16; 3] = [96, 144, 192];
pub const FIXED_CAPTURE_VIEWPORTS: [(u16, u16); 2] = [(800, 600), (1280, 720)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    InvalidConfig,
    InconsistentState,
    WrongSurface,
    InvalidPixels,
    ChangedContract,
    ChangedPixels,
}

impl CaptureError {
    /// No untrusted frame, route identifier, pixel data or private text is logged.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidConfig => "invalid-capture-config",
            Self::InconsistentState => "inconsistent-host-snapshots",
            Self::WrongSurface => "capture-surface-mismatch",
            Self::InvalidPixels => "invalid-capture-pixels",
            Self::ChangedContract => "capture-contract-changed",
            Self::ChangedPixels => "capture-pixels-changed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureSurface {
    Shell,
    Conversation,
    Rich,
    Panels,
    Modal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    Home,
    Conversation,
    Projects,
    Project,
    Files,
    Agents,
    Skills,
    Plugins,
    Settings,
    Activity,
}

impl From<&UiRoute> for RouteKind {
    fn from(route: &UiRoute) -> Self {
        match route {
            UiRoute::Home {} => Self::Home,
            UiRoute::Conversation { .. } => Self::Conversation,
            UiRoute::Projects {} => Self::Projects,
            UiRoute::Project { .. } => Self::Project,
            UiRoute::Files { .. } => Self::Files,
            UiRoute::Agents {} => Self::Agents,
            UiRoute::Skills {} => Self::Skills,
            UiRoute::Plugins {} => Self::Plugins,
            UiRoute::Settings {} => Self::Settings,
            UiRoute::Activity {} => Self::Activity,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichKind {
    Editor,
    Terminal,
    Code,
    Pdf,
    Math,
}

impl From<&RichPanel> for RichKind {
    fn from(panel: &RichPanel) -> Self {
        match panel {
            RichPanel::Editor { .. } => Self::Editor,
            RichPanel::Terminal { .. } => Self::Terminal,
            RichPanel::Code { .. } => Self::Code,
            RichPanel::Pdf { .. } => Self::Pdf,
            RichPanel::Math { .. } => Self::Math,
        }
    }
}

/// Only low-cardinality presentation metadata; no raw text, paths, opaque IDs,
/// mutable host epochs, secret settings, or screenshots enter a persisted proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualProjection {
    pub route: RouteKind,
    pub sidebar_open: bool,
    pub focus: UiFocusTarget,
    pub dialog_open: bool,
    pub panel: Panel,
    pub rich_active: Option<RichKind>,
    pub rich_count: u8,
    pub conversation_rows: u8,
    pub conversation_has_older: bool,
    pub conversation_has_newer: bool,
}

impl VisualProjection {
    /// All five snapshots must originate from the same authenticated host epoch.
    /// This is a consistency fence, NOT transport authentication.
    pub fn from_hosts(
        navigation: &UiSnapshot,
        focus: &UiFocusSnapshot,
        panels: &PanelSnapshot,
        rich: &RichSnapshot,
        conversation: &ConversationSnapshot,
    ) -> Result<Self, CaptureError> {
        let epoch = navigation.host_epoch;
        if epoch == 0
            || epoch > MAX_JS_SAFE_INTEGER
            || [
                focus.host_epoch,
                panels.host_epoch,
                rich.host_epoch,
                conversation.host_epoch,
            ]
            .into_iter()
            .any(|other| other != epoch)
            || [
                navigation.revision,
                focus.revision,
                panels.revision,
                rich.revision,
                conversation.revision,
            ]
            .into_iter()
            .any(|rev| rev == 0 || rev > MAX_JS_SAFE_INTEGER)
        {
            return Err(CaptureError::InconsistentState);
        }
        let dialog_target = matches!(
            focus.target,
            UiFocusTarget::DialogConfirm | UiFocusTarget::DialogCancel
        );
        if focus.dialog_open != dialog_target || rich.panels.len() > MAX_RICH_PANELS {
            return Err(CaptureError::InconsistentState);
        }
        let active = match rich.active_key.as_deref() {
            Some(key) => {
                let mut matches = rich.panels.iter().filter(|panel| panel.key() == key);
                let active = matches.next().ok_or(CaptureError::InconsistentState)?;
                if matches.next().is_some() {
                    return Err(CaptureError::InconsistentState);
                }
                Some(RichKind::from(active))
            }
            None if rich.panels.is_empty() => None,
            None => return Err(CaptureError::InconsistentState),
        };
        let route = RouteKind::from(&navigation.route);
        let mut rows = 0;
        let mut older = false;
        let mut newer = false;
        if let Some(page) = &conversation.page {
            if conversation.session_id.is_none()
                || page.entries.len() > MAX_CONVERSATION_PAGE_ITEMS
                || page.start > page.total
            {
                return Err(CaptureError::InconsistentState);
            }
            let end = page
                .start
                .checked_add(page.entries.len() as u64)
                .ok_or(CaptureError::InconsistentState)?;
            if end > page.total || page.total > MAX_JS_SAFE_INTEGER {
                return Err(CaptureError::InconsistentState);
            }
            rows = page.entries.len() as u8;
            older = page.start > 0;
            newer = end < page.total;
        }
        if let UiRoute::Conversation { session_id } = &navigation.route {
            if conversation.session_id.as_deref() != Some(session_id.as_str()) {
                return Err(CaptureError::InconsistentState);
            }
        }

        Ok(Self {
            route,
            sidebar_open: navigation.sidebar_open,
            focus: focus.target,
            dialog_open: focus.dialog_open,
            panel: panels.panel,
            rich_active: active,
            rich_count: rich.panels.len() as u8,
            conversation_rows: rows,
            conversation_has_older: older,
            conversation_has_newer: newer,
        })
    }
}

/// Fixed synthetic viewport and target pixels. Scale is integer DPI/96: 1,
/// 1.5 or 2, so 800x600 at 144 DPI must capture exactly 1200x900 physical px.
/// No arbitrary dimensions, CSS zoom or platform-dependent float rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureConfig {
    pub logical_width: u16,
    pub logical_height: u16,
    pub dpi: u16,
    pub theme: Theme,
    pub language: UiLanguage,
}

impl CaptureConfig {
    pub fn new(
        logical_width: u16,
        logical_height: u16,
        dpi: u16,
        theme: Theme,
        language: UiLanguage,
    ) -> Result<Self, CaptureError> {
        if !FIXED_CAPTURE_VIEWPORTS.contains(&(logical_width, logical_height))
            || !FIXED_CAPTURE_DPI.contains(&dpi)
            || matches!(theme, Theme::System)
        {
            return Err(CaptureError::InvalidConfig);
        }
        let config = Self { logical_width, logical_height, dpi, theme, language };
        config.rgba_bytes()?;
        Ok(config)
    }

    pub fn physical_dimensions(self) -> (u32, u32) {
        (
            u32::from(self.logical_width) * u32::from(self.dpi) / 96,
            u32::from(self.logical_height) * u32::from(self.dpi) / 96,
        )
    }

    pub fn rgba_bytes(self) -> Result<usize, CaptureError> {
        let (width, height) = self.physical_dimensions();
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|count| count.checked_mul(4))
            .ok_or(CaptureError::InvalidConfig)?;
        if bytes > MAX_CAPTURE_RGBA_BYTES {
            return Err(CaptureError::InvalidConfig);
        }
        Ok(bytes)
    }
}

/// A typed fixture case for a specific UI state and host-selected appearance.
/// The renderer must use the *same* resolved theme and 13-locale catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureDescriptor {
    pub version: u32,
    pub config: CaptureConfig,
    pub surface: CaptureSurface,
    pub projection: VisualProjection,
}

impl CaptureDescriptor {
    pub fn new(
        config: CaptureConfig,
        surface: CaptureSurface,
        navigation: &UiSnapshot,
        focus: &UiFocusSnapshot,
        panels: &PanelSnapshot,
        rich: &RichSnapshot,
        conversation: &ConversationSnapshot,
    ) -> Result<Self, CaptureError> {
        // An explicit preference overrides the system theme. The native host
        // resolves "system" before constructing this fixed capture config.
        let preferences = &panels.catalog.preferences;
        if preferences.theme != Theme::System && preferences.theme != config.theme {
            return Err(CaptureError::InvalidConfig);
        }
        if UiLanguage::select(Some(&preferences.locale)) != config.language {
            return Err(CaptureError::InvalidConfig);
        }
        let projection = VisualProjection::from_hosts(
            navigation, focus, panels, rich, conversation,
        )?;
        let valid_surface = match surface {
            CaptureSurface::Shell => true,
            CaptureSurface::Conversation => projection.route == RouteKind::Conversation,
            CaptureSurface::Rich => projection.rich_active.is_some(),
            CaptureSurface::Panels => matches!(
                projection.route,
                RouteKind::Projects | RouteKind::Project | RouteKind::Files
                    | RouteKind::Agents | RouteKind::Skills | RouteKind::Plugins
                    | RouteKind::Settings
            ),
            CaptureSurface::Modal => projection.dialog_open,
        };
        if !valid_surface {
            return Err(CaptureError::WrongSurface);
        }
        Ok(Self { version: UI_CAPTURE_VERSION, config, surface, projection })
    }

    /// Trusted native/fixture host supplies tightly packed, opaque sRGB RGBA8
    /// row-major pixels, with no row padding, ICC changes or compression.
    /// Reject transparent pixels to prevent hidden RGB data and noncanonical
    /// screenshots. No captured pixels leave this call.
    pub fn fingerprint(&self, rgba: &[u8]) -> Result<CaptureProof, CaptureError> {
        if self.version != UI_CAPTURE_VERSION
            || !FIXED_CAPTURE_VIEWPORTS.contains(&(self.config.logical_width, self.config.logical_height))
            || !FIXED_CAPTURE_DPI.contains(&self.config.dpi)
            || matches!(self.config.theme, Theme::System)
            || rgba.len() != self.config.rgba_bytes()?
            || rgba.chunks_exact(4).any(|pixel| pixel[3] != 255)
        {
            return Err(CaptureError::InvalidPixels);
        }
        Ok(CaptureProof {
            descriptor: self.clone(),
            sha256: format!("{:x}", Sha256::digest(rgba)),
        })
    }
}

/// Content-addressed proof only. Golden digests must be reviewed/frozen as
/// immutable fixtures, never overwritten automatically on a failed test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureProof {
    pub descriptor: CaptureDescriptor,
    pub sha256: String,
}

impl CaptureProof {
    pub fn verify(&self, actual: &Self) -> Result<(), CaptureError> {
        if self.descriptor != actual.descriptor {
            return Err(CaptureError::ChangedContract);
        }
        if self.sha256 != actual.sha256 {
            return Err(CaptureError::ChangedPixels);
        }
        Ok(())
    }
}
