//! PERF-03: revision-driven, single-frame UI serialization with lazy heavy data.
//!
//! This is a cloud-qualified host-side projection contract, NOT a native renderer
//! or an authorization boundary. A native host must authenticate and authorize
//! all sessions and renderer transport before publishing this frame.
//! Idle calls do not serialize, and the cache holds only the most recent frame.

use std::collections::HashSet;

use serde::Serialize;

use crate::ui_contract::{MAX_JS_SAFE_INTEGER, UiRoute, UiSnapshot};
use crate::ui_conversation::ConversationSnapshot;
use crate::ui_focus::UiFocusSnapshot;
use crate::ui_integration::{DesktopAudit, DesktopAuditError};
use crate::ui_panels::{
    AgentItem, ExtensionItem, FileItem, Panel, PanelSnapshot, ProjectItem, SettingsTab,
};
use crate::ui_rich::{RichPanel, RichSnapshot};

pub const MAX_VISIBLE_FRAME_BYTES: usize = 96 * 1024;
pub const MAX_GROUP_CATALOG_BYTES: usize = 32 * 1024;
pub const MAX_CHAT_GROUPS: usize = 64;
pub const MAX_GROUP_SESSIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGroup {
    pub id: String,
    pub title: String,
    /// Only opaque session identifiers: no messages, previews, or transcripts.
    pub session_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGroupCatalog {
    pub host_epoch: u64,
    pub revision: u64,
    pub groups: Vec<ChatGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UiPerformanceError {
    #[error("inconsistent or unauthorized host presentation state")]
    Inconsistent,
    #[error("invalid or oversized chat group metadata")]
    InvalidGroups,
    #[error("visible UI projection exceeds serialization budget")]
    Oversize,
}

impl From<DesktopAuditError> for UiPerformanceError {
    fn from(_: DesktopAuditError) -> Self {
        Self::Inconsistent
    }
}

fn valid_opaque_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('.')
        && !value.ends_with('.')
        && !value.contains("..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

impl ChatGroupCatalog {
    /// Called on the cold/change path only; owners reauthorize session listings.
    pub fn validate(&self, expected_epoch: u64) -> Result<(), UiPerformanceError> {
        if self.host_epoch != expected_epoch
            || self.revision == 0
            || self.revision > MAX_JS_SAFE_INTEGER
            || self.groups.len() > MAX_CHAT_GROUPS
        {
            return Err(UiPerformanceError::InvalidGroups);
        }
        let mut groups = HashSet::new();
        let mut sessions = HashSet::new();
        for group in &self.groups {
            if !valid_opaque_key(&group.id)
                || !groups.insert(group.id.as_str())
                || group.title.is_empty()
                || group.title.len() > 128
                || group.title.chars().any(|c| {
                    c.is_control()
                        || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                })
            {
                return Err(UiPerformanceError::InvalidGroups);
            }
            for session in &group.session_ids {
                if !valid_opaque_key(session) || !sessions.insert(session.as_str()) {
                    return Err(UiPerformanceError::InvalidGroups);
                }
                if sessions.len() > MAX_GROUP_SESSIONS {
                    return Err(UiPerformanceError::InvalidGroups);
                }
            }
        }
        let bytes = serde_json::to_vec(self).map_err(|_| UiPerformanceError::InvalidGroups)?;
        if bytes.len() > MAX_GROUP_CATALOG_BYTES {
            return Err(UiPerformanceError::InvalidGroups);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrameVersion {
    host_epoch: u64,
    nav: u64,
    focus: u64,
    panels: u64,
    rich: u64,
    conversation: u64,
    session_generation: u64,
    group: u64,
    // Route equality avoids sharing a frame between different session keys.
    route: UiRoute,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisiblePanel<'a> {
    revision: u64,
    panel: Panel,
    settings_tab: SettingsTab,
    selected_project: Option<&'a str>,
    selected_file: Option<&'a str>,
    selected_agent: Option<&'a str>,
    selected_extension: Option<&'a (crate::ui_panels::ExtensionKind, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    projects: Option<&'a [ProjectItem]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<&'a [FileItem]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agents: Option<&'a [AgentItem]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Option<&'a [ExtensionItem]>,
}

impl<'a> VisiblePanel<'a> {
    fn from_snapshot(panel: &'a PanelSnapshot) -> Self {
        Self {
            revision: panel.revision,
            panel: panel.panel,
            settings_tab: panel.settings_tab,
            selected_project: panel.selected_project.as_deref(),
            selected_file: panel.selected_file.as_deref(),
            selected_agent: panel.selected_agent.as_deref(),
            selected_extension: panel.selected_extension.as_ref(),
            projects: (panel.panel == Panel::Projects).then_some(panel.catalog.projects.as_slice()),
            files: (panel.panel == Panel::Files).then_some(panel.catalog.files.as_slice()),
            agents: (panel.panel == Panel::Agents).then_some(panel.catalog.agents.as_slice()),
            extensions: matches!(panel.panel, Panel::Skills | Panel::Plugins)
                .then_some(panel.catalog.extensions.as_slice()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisibleFrame<'a> {
    navigation: &'a UiSnapshot,
    focus: &'a UiFocusSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    panel: Option<VisiblePanel<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rich: Option<&'a RichPanel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation: Option<&'a ConversationSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    groups: Option<&'a ChatGroupCatalog>,
}

/// No history LRU: retaining old serialized pages would duplicate private data.
/// A renderer needs exactly one currently visible frame; any other cache is a
/// separate, explicitly budgeted and authorized host-owned concern.
#[derive(Debug, Default)]
pub struct UiFrameCache {
    version: Option<FrameVersion>,
    frame: Vec<u8>,
    builds: u64,
}

impl UiFrameCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn retained_bytes(&self) -> usize {
        self.frame.len()
    }

    pub fn serialization_count(&self) -> u64 {
        self.builds
    }

    /// Drops serialized private UI data on logout, lock, session revocation,
    /// renderer recreation, or teardown. Host must call this on authorization
    /// changes even when all presentation revisions happen to stay identical.
    pub fn clear(&mut self) {
        self.version = None;
        self.frame.clear();
        self.frame.shrink_to_fit();
    }

    /// Returns None for unchanged state: no JSON allocation and no IPC send.
    /// Otherwise a single bounded byte frame is returned. Never log its bytes.
    /// The caller owns publication, origin authentication and reauthorization.
    pub fn refresh(
        &mut self,
        nav: &UiSnapshot,
        focus: &UiFocusSnapshot,
        panels: &PanelSnapshot,
        rich: &RichSnapshot,
        conversation: &ConversationSnapshot,
        groups: Option<&ChatGroupCatalog>,
    ) -> Result<Option<&[u8]>, UiPerformanceError> {
        let show_groups = matches!(nav.route, UiRoute::Home {}) && nav.sidebar_open;
        let groups = if show_groups { groups } else { None };
        let key = FrameVersion {
            host_epoch: nav.host_epoch,
            nav: nav.revision,
            focus: focus.revision,
            panels: panels.revision,
            rich: rich.revision,
            conversation: conversation.revision,
            session_generation: conversation.session_generation,
            group: groups.map_or(0, |g| g.revision),
            route: nav.route.clone(),
        };
        // Cheap consistency fences even on the idle fast path.
        if [focus.host_epoch, panels.host_epoch, rich.host_epoch, conversation.host_epoch]
            .iter()
            .any(|epoch| *epoch != nav.host_epoch)
        {
            self.clear();
            return Err(UiPerformanceError::Inconsistent);
        }
        if self.version.as_ref() == Some(&key) {
            return Ok(None);
        }
        // Evict before processing a changed session/epoch or invalid frame.
        self.clear();
        DesktopAudit::inspect(nav, focus, panels, rich, conversation)?;
        if let Some(group) = groups {
            group.validate(nav.host_epoch)?;
        }
        let show_panel = matches!(
            nav.route,
            UiRoute::Projects {}
                | UiRoute::Files { .. }
                | UiRoute::Agents {}
                | UiRoute::Skills {}
                | UiRoute::Plugins {}
                | UiRoute::Settings {}
        );
        let show_rich = matches!(
            nav.route,
            UiRoute::Conversation { .. }
                | UiRoute::Project { .. }
                | UiRoute::Files { .. }
                | UiRoute::Activity {}
        );
        let active_rich = if show_rich {
            rich.active_key
                .as_ref()
                .and_then(|id| rich.panels.iter().find(|panel| panel.key() == id))
        } else {
            None
        };
        let wire = VisibleFrame {
            navigation: nav,
            focus,
            panel: show_panel.then(|| VisiblePanel::from_snapshot(panels)),
            rich: active_rich,
            conversation: matches!(nav.route, UiRoute::Conversation { .. }).then_some(conversation),
            groups,
        };
        let bytes = serde_json::to_vec(&wire).map_err(|_| UiPerformanceError::Inconsistent)?;
        if bytes.len() > MAX_VISIBLE_FRAME_BYTES {
            return Err(UiPerformanceError::Oversize);
        }
        self.frame = bytes;
        self.version = Some(key);
        self.builds = self.builds.saturating_add(1);
        Ok(Some(&self.frame))
    }
}
