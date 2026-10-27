//! Phase 15C: bounded, host-owned rich presentation panels.
//!
//! A panel is inert display data, never an executable command, path, URL, HTML
//! fragment or permission grant. A native host authenticates renderer origin
//! separately; epoch/sequence/revision checks prevent stale UI mutations but
//! are not authentication. Only the host publishes terminal/code/PDF content.
use serde::{Deserialize, Serialize};

use crate::ui_contract::MAX_JS_SAFE_INTEGER;

pub const RICH_PROTOCOL_VERSION: u32 = 1;
pub const MAX_RICH_FRAME_BYTES: usize = 16 * 1024;
pub const MAX_RICH_SNAPSHOT_BYTES: usize = 32 * 1024;
pub const MAX_RICH_TEXT_BYTES: usize = 8 * 1024;
pub const MAX_RICH_TITLE_BYTES: usize = 256;
pub const MAX_RICH_KEY_BYTES: usize = 128;
pub const MAX_RICH_PANELS: usize = 8;
pub const MAX_PDF_PAGES: u32 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RichPanel {
    Editor {
        key: String,
        title: String,
        language: String,
        text: String,
        read_only: bool,
    },
    Terminal {
        key: String,
        title: String,
        output: String,
    },
    Code {
        key: String,
        title: String,
        language: String,
        text: String,
    },
    Pdf {
        key: String,
        title: String,
        asset_id: String,
        page: u32,
        page_count: u32,
    },
    Math {
        key: String,
        title: String,
        source: String,
    },
}

impl RichPanel {
    pub fn key(&self) -> &str {
        match self {
            Self::Editor { key, .. }
            | Self::Terminal { key, .. }
            | Self::Code { key, .. }
            | Self::Pdf { key, .. }
            | Self::Math { key, .. } => key,
        }
    }

    fn validate(&self) -> Result<(), RichError> {
        validate_key(self.key())?;
        match self {
            Self::Editor {
                title,
                language,
                text,
                ..
            }
            | Self::Code {
                title,
                language,
                text,
                ..
            } => {
                validate_title(title)?;
                validate_language(language)?;
                validate_text(text)?;
            }
            Self::Terminal { title, output, .. } => {
                validate_title(title)?;
                validate_text(output)?;
            }
            Self::Pdf {
                title,
                asset_id,
                page,
                page_count,
                ..
            } => {
                validate_title(title)?;
                validate_key(asset_id)?;
                if *page_count == 0
                    || *page_count > MAX_PDF_PAGES
                    || *page == 0
                    || page > page_count
                {
                    return Err(RichError::Page);
                }
            }
            Self::Math { title, source, .. } => {
                validate_title(title)?;
                validate_text(source)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichSnapshot {
    pub host_epoch: u64,
    pub revision: u64,
    pub active_key: Option<String>,
    pub panels: Vec<RichPanel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RichRendererCommand {
    Snapshot {},
    Activate {
        key: String,
        expected_revision: u64,
    },
    ReplaceEditor {
        key: String,
        expected_revision: u64,
        text: String,
    },
    ReplaceMath {
        key: String,
        expected_revision: u64,
        source: String,
    },
    SetPdfPage {
        key: String,
        expected_revision: u64,
        page: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RichRendererRequest {
    pub protocol: u32,
    pub host_epoch: u64,
    pub sequence: u64,
    pub command: RichRendererCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichReply {
    pub protocol: u32,
    pub host_epoch: u64,
    pub sequence: u64,
    pub state: RichSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RichError {
    #[error("rich panel payload too large")]
    Oversize,
    #[error("invalid rich panel request")]
    Malformed,
    #[error("unsupported rich panel protocol")]
    Protocol,
    #[error("stale host epoch")]
    Epoch,
    #[error("out-of-order rich panel request")]
    Sequence,
    #[error("stale rich panel revision")]
    Revision,
    #[error("invalid opaque panel identifier")]
    Key,
    #[error("invalid panel text or metadata")]
    Text,
    #[error("invalid PDF page")]
    Page,
    #[error("panel not found")]
    NotFound,
    #[error("renderer cannot modify this surface")]
    Forbidden,
    #[error("rich panel capacity exceeded")]
    Capacity,
    #[error("rich panel counter exhausted")]
    Exhausted,
}

impl RichError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Oversize => "payload-too-large",
            Self::Malformed => "invalid-request",
            Self::Protocol => "unsupported-protocol",
            Self::Epoch => "stale-host-epoch",
            Self::Sequence => "invalid-sequence",
            Self::Revision => "stale-revision",
            Self::Key => "invalid-panel-key",
            Self::Text => "invalid-panel-text",
            Self::Page => "invalid-pdf-page",
            Self::NotFound => "panel-not-found",
            Self::Forbidden => "forbidden-surface-mutation",
            Self::Capacity => "panel-capacity",
            Self::Exhausted => "counter-exhausted",
        }
    }
}

/// Host-owned presentation model. No process, file, URL, browser, or asset IO.
#[derive(Debug)]
pub struct RichDeck {
    state: RichSnapshot,
    last_sequence: u64,
}

impl RichDeck {
    pub fn new(host_epoch: u64) -> Result<Self, RichError> {
        validate_counter(host_epoch).map_err(|_| RichError::Epoch)?;
        Ok(Self {
            state: RichSnapshot {
                host_epoch,
                revision: 1,
                active_key: None,
                panels: Vec::new(),
            },
            last_sequence: 0,
        })
    }

    pub fn snapshot(&self) -> RichSnapshot {
        self.state.clone()
    }

    /// An authenticated host, not the renderer, supplies display data.
    /// Updates are atomic even for invalid or over-capacity snapshots.
    pub fn publish_from_host(&mut self, panel: RichPanel) -> Result<(), RichError> {
        panel.validate()?;
        let mut next = self.state.clone();
        if let Some(current) = next.panels.iter_mut().find(|p| p.key() == panel.key()) {
            *current = panel;
        } else {
            if next.panels.len() >= MAX_RICH_PANELS {
                return Err(RichError::Capacity);
            }
            if next.active_key.is_none() {
                next.active_key = Some(panel.key().to_owned());
            }
            next.panels.push(panel);
        }
        if next != self.state {
            next.revision = advance(next.revision)?;
            validate_snapshot(&next)?;
            self.state = next;
        }
        Ok(())
    }

    pub fn remove_from_host(&mut self, key: &str) -> Result<(), RichError> {
        validate_key(key)?;
        let mut next = self.state.clone();
        let before = next.panels.len();
        next.panels.retain(|panel| panel.key() != key);
        if next.panels.len() == before {
            return Err(RichError::NotFound);
        }
        if next.active_key.as_deref() == Some(key) {
            next.active_key = next.panels.first().map(|panel| panel.key().to_owned());
        }
        next.revision = advance(next.revision)?;
        self.state = next;
        Ok(())
    }

    /// Recreated renderer must rebind to a strictly higher host-owned epoch.
    /// Old rich data does not implicitly cross user-profile/renderer boundaries.
    pub fn rebind(&mut self, host_epoch: u64) -> Result<(), RichError> {
        validate_counter(host_epoch).map_err(|_| RichError::Epoch)?;
        if host_epoch <= self.state.host_epoch {
            return Err(RichError::Epoch);
        }
        let revision = advance(self.state.revision)?;
        self.state = RichSnapshot {
            host_epoch,
            revision,
            active_key: None,
            panels: Vec::new(),
        };
        self.last_sequence = 0;
        Ok(())
    }

    /// Native transport must additionally check the actual sender's identity.
    pub fn dispatch_renderer_json(&mut self, frame: &[u8]) -> Result<RichReply, RichError> {
        if frame.len() > MAX_RICH_FRAME_BYTES {
            return Err(RichError::Oversize);
        }
        let request: RichRendererRequest =
            serde_json::from_slice(frame).map_err(|_| RichError::Malformed)?;
        if request.protocol != RICH_PROTOCOL_VERSION {
            return Err(RichError::Protocol);
        }
        if request.host_epoch != self.state.host_epoch {
            return Err(RichError::Epoch);
        }
        validate_counter(request.sequence).map_err(|_| RichError::Sequence)?;
        if request.sequence != advance(self.last_sequence).map_err(|_| RichError::Sequence)? {
            return Err(RichError::Sequence);
        }
        // Consume a valid sequence even when the operation is denied.
        self.last_sequence = request.sequence;
        let mut next = self.state.clone();
        match request.command {
            RichRendererCommand::Snapshot {} => {}
            RichRendererCommand::Activate {
                key,
                expected_revision,
            } => {
                expect_revision(&next, expected_revision)?;
                validate_key(&key)?;
                if !next.panels.iter().any(|panel| panel.key() == key) {
                    return Err(RichError::NotFound);
                }
                next.active_key = Some(key);
            }
            RichRendererCommand::ReplaceEditor {
                key,
                expected_revision,
                text,
            } => {
                expect_revision(&next, expected_revision)?;
                validate_key(&key)?;
                validate_text(&text)?;
                let panel = find_panel_mut(&mut next, &key)?;
                match panel {
                    RichPanel::Editor {
                        text: existing,
                        read_only: false,
                        ..
                    } => *existing = text,
                    _ => return Err(RichError::Forbidden),
                }
            }
            RichRendererCommand::ReplaceMath {
                key,
                expected_revision,
                source,
            } => {
                expect_revision(&next, expected_revision)?;
                validate_key(&key)?;
                validate_text(&source)?;
                let panel = find_panel_mut(&mut next, &key)?;
                match panel {
                    RichPanel::Math {
                        source: existing, ..
                    } => *existing = source,
                    _ => return Err(RichError::Forbidden),
                }
            }
            RichRendererCommand::SetPdfPage {
                key,
                expected_revision,
                page,
            } => {
                expect_revision(&next, expected_revision)?;
                validate_key(&key)?;
                let panel = find_panel_mut(&mut next, &key)?;
                match panel {
                    RichPanel::Pdf {
                        page: current,
                        page_count,
                        ..
                    } if page > 0 && page <= *page_count => *current = page,
                    RichPanel::Pdf { .. } => return Err(RichError::Page),
                    _ => return Err(RichError::Forbidden),
                }
            }
        }
        if next != self.state {
            next.revision = advance(next.revision)?;
            validate_snapshot(&next)?;
            self.state = next;
        }
        Ok(RichReply {
            protocol: RICH_PROTOCOL_VERSION,
            host_epoch: self.state.host_epoch,
            sequence: request.sequence,
            state: self.snapshot(),
        })
    }
}

fn find_panel_mut<'a>(
    state: &'a mut RichSnapshot,
    key: &str,
) -> Result<&'a mut RichPanel, RichError> {
    state
        .panels
        .iter_mut()
        .find(|panel| panel.key() == key)
        .ok_or(RichError::NotFound)
}

fn expect_revision(state: &RichSnapshot, expected: u64) -> Result<(), RichError> {
    if state.revision == expected {
        Ok(())
    } else {
        Err(RichError::Revision)
    }
}

fn validate_snapshot(state: &RichSnapshot) -> Result<(), RichError> {
    if serde_json::to_vec(state)
        .map_err(|_| RichError::Malformed)?
        .len()
        > MAX_RICH_SNAPSHOT_BYTES
    {
        Err(RichError::Oversize)
    } else {
        Ok(())
    }
}

fn validate_key(value: &str) -> Result<(), RichError> {
    if value.is_empty()
        || value.len() > MAX_RICH_KEY_BYTES
        || value.starts_with('.')
        || value.contains("..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
    {
        Err(RichError::Key)
    } else {
        Ok(())
    }
}

fn validate_language(value: &str) -> Result<(), RichError> {
    if value.is_empty()
        || value.len() > 32
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'+'))
    {
        Err(RichError::Text)
    } else {
        Ok(())
    }
}

fn validate_title(value: &str) -> Result<(), RichError> {
    if value.is_empty() || value.len() > MAX_RICH_TITLE_BYTES {
        Err(RichError::Text)
    } else {
        validate_visible_text(value)
    }
}

fn validate_text(value: &str) -> Result<(), RichError> {
    if value.len() > MAX_RICH_TEXT_BYTES {
        Err(RichError::Oversize)
    } else {
        validate_visible_text(value)
    }
}

fn validate_visible_text(value: &str) -> Result<(), RichError> {
    if value.chars().any(|c| {
        (c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            || ('\u{202a}'..='\u{202e}').contains(&c)
            || ('\u{2066}'..='\u{2069}').contains(&c)
    }) {
        Err(RichError::Text)
    } else {
        Ok(())
    }
}

fn validate_counter(value: u64) -> Result<(), RichError> {
    if value == 0 || value > MAX_JS_SAFE_INTEGER {
        Err(RichError::Exhausted)
    } else {
        Ok(())
    }
}

fn advance(value: u64) -> Result<u64, RichError> {
    value
        .checked_add(1)
        .filter(|next| *next <= MAX_JS_SAFE_INTEGER)
        .ok_or(RichError::Exhausted)
}
