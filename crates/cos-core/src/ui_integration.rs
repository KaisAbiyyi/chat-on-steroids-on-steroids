//! 15G: read-only desktop integration and bounded state-size audit.
//!
//! This is a host-side consistency/diagnostic boundary, not a renderer,
//! native desktop, browser transport or security authenticator. Call only
//! after the separately authenticated host has stabilized its five models.
//! No identifiers, message bodies, file paths or screenshot pixels are
//! returned in the diagnostic receipt.
use serde::Serialize;

use crate::ui_contract::{MAX_UI_IPC_BYTES, UiRoute, UiSnapshot};
use crate::ui_conversation::{ConversationSnapshot, MAX_CONVERSATION_PAGE_BYTES};
use crate::ui_focus::UiFocusSnapshot;
use crate::ui_panels::{MAX_PANEL_CATALOG_BYTES, MAX_PANEL_ITEMS, Panel, PanelSnapshot};
use crate::ui_rich::{MAX_RICH_SNAPSHOT_BYTES, RichSnapshot};
use crate::ui_snapshots::{RouteKind, VisualProjection};

/// Aggregate audit budget, not a claim about process RSS or GPU allocations.
pub const MAX_DESKTOP_AUDIT_BYTES: usize = 192 * 1024;
const MAX_PRESENTATION_METADATA_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DesktopAuditError {
    #[error("desktop host snapshots disagree or contain stale state")]
    InconsistentHosts,
    #[error("navigation and panel selection are not yet synchronized")]
    PanelRouteMismatch,
    #[error("bounded desktop presentation state exceeds audit limit")]
    Oversize,
}

impl DesktopAuditError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InconsistentHosts => "inconsistent-host-state",
            Self::PanelRouteMismatch => "unsynchronized-panel-route",
            Self::Oversize => "desktop-state-over-budget",
        }
    }
}

/// Privacy-minimized, non-persisted diagnostic facts. None of the five raw
/// snapshots is retained by this report. In particular a background history
/// page never becomes a visible conversation row on another route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopAudit {
    pub host_epoch: u64,
    pub projection: VisualProjection,
    pub retained_serialized_bytes: usize,
    pub visible_conversation_rows: u8,
    pub history_pending: bool,
    pub catalog_items: usize,
}

impl DesktopAudit {
    /// The native host must independently authenticate each renderer frame
    /// and authorize each session/catalog request. Epoch equality is only a
    /// stale-work fence. Audit does not mutate, authorize or publish payloads.
    pub fn inspect(
        navigation: &UiSnapshot,
        focus: &UiFocusSnapshot,
        panels: &PanelSnapshot,
        rich: &RichSnapshot,
        conversation: &ConversationSnapshot,
    ) -> Result<Self, DesktopAuditError> {
        let projection =
            VisualProjection::from_hosts(navigation, focus, panels, rich, conversation)
                .map_err(|_| DesktopAuditError::InconsistentHosts)?;

        // These routes correspond to a host-projected panel surface. A host
        // should audit only after the route and panel models have settled.
        // Project details and non-panel routes have no mandatory panel state.
        let expected = match navigation.route {
            UiRoute::Projects {} => Some(Panel::Projects),
            UiRoute::Files { .. } => Some(Panel::Files),
            UiRoute::Agents {} => Some(Panel::Agents),
            UiRoute::Skills {} => Some(Panel::Skills),
            UiRoute::Plugins {} => Some(Panel::Plugins),
            UiRoute::Settings {} => Some(Panel::Settings),
            _ => None,
        };
        if expected.is_some_and(|p| panels.panel != p) {
            return Err(DesktopAuditError::PanelRouteMismatch);
        }

        let catalog_items = panels.catalog.projects.len()
            + panels.catalog.files.len()
            + panels.catalog.agents.len()
            + panels.catalog.extensions.len();
        if catalog_items > MAX_PANEL_ITEMS {
            return Err(DesktopAuditError::Oversize);
        }
        if conversation.session_id.is_none()
            && (conversation.page.is_some() || conversation.pending.is_some())
        {
            return Err(DesktopAuditError::InconsistentHosts);
        }
        if conversation.pending.as_ref().is_some_and(|ticket| {
            ticket.host_epoch != navigation.host_epoch
                || ticket.session_generation != conversation.session_generation
        }) {
            return Err(DesktopAuditError::InconsistentHosts);
        }

        // Serialization is used solely for an explicit diagnostic path, not
        // in the renderer hot loop. Never emit the serialized private bytes.
        let sizes = [
            bounded_size(navigation, MAX_UI_IPC_BYTES)?,
            bounded_size(focus, MAX_UI_IPC_BYTES)?,
            bounded_size(
                panels,
                MAX_PANEL_CATALOG_BYTES + MAX_PRESENTATION_METADATA_BYTES,
            )?,
            bounded_size(rich, MAX_RICH_SNAPSHOT_BYTES)?,
            bounded_size(
                conversation,
                MAX_CONVERSATION_PAGE_BYTES + MAX_PRESENTATION_METADATA_BYTES,
            )?,
        ];
        let total = sizes.iter().sum::<usize>();
        if total > MAX_DESKTOP_AUDIT_BYTES {
            return Err(DesktopAuditError::Oversize);
        }
        Ok(Self {
            host_epoch: navigation.host_epoch,
            visible_conversation_rows: if projection.route == RouteKind::Conversation {
                projection.conversation_rows
            } else {
                0
            },
            history_pending: conversation.pending.is_some(),
            projection,
            retained_serialized_bytes: total,
            catalog_items,
        })
    }
}

fn bounded_size<T: Serialize>(snapshot: &T, limit: usize) -> Result<usize, DesktopAuditError> {
    // All inputs come from validated, bounded host model snapshots.
    let bytes = serde_json::to_vec(snapshot).map_err(|_| DesktopAuditError::InconsistentHosts)?;
    if bytes.len() > limit {
        return Err(DesktopAuditError::Oversize);
    }
    Ok(bytes.len())
}
