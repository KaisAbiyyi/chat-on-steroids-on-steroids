//! 15B: bounded, host-owned conversation presentation and large-history paging.
//!
//! Renderer requests are inert navigation intents. Only a separately authenticated
//! native host may resolve session history and publish pages. Epochs, generations,
//! sequence and revisions fence stale work but do not authenticate a renderer.
//! Entries are plain text, not trusted Markdown, HTML, URLs or tool capabilities.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::ui_contract::{MAX_JS_SAFE_INTEGER, UI_PROTOCOL_VERSION};

pub const MAX_CONVERSATION_REQUEST_BYTES: usize = 8 * 1024;
pub const MAX_CONVERSATION_PAGE_BYTES: usize = 64 * 1024;
pub const MAX_CONVERSATION_PAGE_ITEMS: usize = 24;
pub const MAX_CONVERSATION_TEXT_BYTES: usize = 2 * 1024;
pub const MAX_CONVERSATION_KEY_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConversationRole {
    User,
    Assistant,
    Tool,
    Activity,
    Error,
}

/// Stable store chronology position, not a client-supplied sort key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEntry {
    pub position: u64,
    pub message_id: String,
    pub turn_id: Option<String>,
    pub role: ConversationRole,
    pub text: String,
    pub truncated: bool,
}

impl ConversationEntry {
    fn validate(&self) -> Result<(), ConversationError> {
        valid_key(&self.message_id)?;
        if let Some(turn) = &self.turn_id {
            valid_key(turn)?;
        }
        if self.position > MAX_JS_SAFE_INTEGER {
            return Err(ConversationError::InvalidPage);
        }
        if self.text.len() > MAX_CONVERSATION_TEXT_BYTES
            || self.text.chars().any(|c| {
                (c.is_control() && c != '\n' && c != '\t')
                    || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            })
        {
            return Err(ConversationError::InvalidPage);
        }
        Ok(())
    }
}

/// Host-only page. No serialized arbitrary storage paths, URLs or HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationPage {
    pub start: u64,
    pub total: u64,
    pub entries: Vec<ConversationEntry>,
}

impl ConversationPage {
    fn validate(&self) -> Result<(), ConversationError> {
        if self.entries.len() > MAX_CONVERSATION_PAGE_ITEMS
            || self.total > MAX_JS_SAFE_INTEGER
            || self.start > self.total
        {
            return Err(ConversationError::InvalidPage);
        }
        let end = self
            .start
            .checked_add(self.entries.len() as u64)
            .ok_or(ConversationError::InvalidPage)?;
        if end > self.total || (self.entries.is_empty() && self.total != 0) {
            return Err(ConversationError::InvalidPage);
        }
        let mut ids = HashSet::new();
        for (offset, item) in self.entries.iter().enumerate() {
            item.validate()?;
            if item.position != self.start + offset as u64 || !ids.insert(item.message_id.as_str())
            {
                return Err(ConversationError::InvalidPage);
            }
        }
        let bytes = serde_json::to_vec(self).map_err(|_| ConversationError::InvalidPage)?;
        if bytes.len() > MAX_CONVERSATION_PAGE_BYTES {
            return Err(ConversationError::Oversize);
        }
        Ok(())
    }

    fn end(&self) -> u64 {
        self.start + self.entries.len() as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PageDirection {
    Initial,
    Older,
    Newer,
}

/// Token is a correlation fence, not a bearer credential or access grant.
/// The host must check backend session authorization again on every page load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageTicket {
    pub host_epoch: u64,
    pub session_generation: u64,
    pub request_id: u64,
    pub direction: PageDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSnapshot {
    pub host_epoch: u64,
    pub session_generation: u64,
    pub revision: u64,
    pub session_id: Option<String>,
    pub page: Option<ConversationPage>,
    pub pending: Option<PageTicket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ConversationCommand {
    Snapshot {},
    LoadOlder { expected_revision: u64 },
    LoadNewer { expected_revision: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationRequest {
    pub protocol: u32,
    pub host_epoch: u64,
    pub sequence: u64,
    pub command: ConversationCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReply {
    pub protocol: u32,
    pub sequence: u64,
    pub state: ConversationSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConversationError {
    #[error("conversation frame or page too large")]
    Oversize,
    #[error("invalid conversation request")]
    Malformed,
    #[error("unsupported protocol")]
    Protocol,
    #[error("stale host epoch")]
    Epoch,
    #[error("out-of-order renderer sequence")]
    Sequence,
    #[error("stale conversation revision")]
    Revision,
    #[error("unsafe conversation key")]
    Key,
    #[error("invalid or non-contiguous history page")]
    InvalidPage,
    #[error("no eligible history page available")]
    Boundary,
    #[error("conversation load already pending")]
    Pending,
    #[error("stale history result")]
    Stale,
    #[error("conversation counter exhausted")]
    Exhausted,
}

impl ConversationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Oversize => "payload-too-large",
            Self::Malformed => "invalid-request",
            Self::Protocol => "unsupported-protocol",
            Self::Epoch => "stale-host-epoch",
            Self::Sequence => "invalid-sequence",
            Self::Revision => "stale-revision",
            Self::Key => "invalid-key",
            Self::InvalidPage => "invalid-page",
            Self::Boundary => "history-boundary",
            Self::Pending => "load-pending",
            Self::Stale => "stale-page",
            Self::Exhausted => "counter-exhausted",
        }
    }
}

fn valid_key(key: &str) -> Result<(), ConversationError> {
    if key.is_empty()
        || key.len() > MAX_CONVERSATION_KEY_BYTES
        || key.starts_with('.')
        || key.ends_with('.')
        || key.contains("..")
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ConversationError::Key);
    }
    Ok(())
}

fn counter(value: u64) -> Result<(), ConversationError> {
    if value == 0 || value > MAX_JS_SAFE_INTEGER {
        Err(ConversationError::Exhausted)
    } else {
        Ok(())
    }
}

fn next(value: u64) -> Result<u64, ConversationError> {
    value
        .checked_add(1)
        .filter(|value| *value <= MAX_JS_SAFE_INTEGER)
        .ok_or(ConversationError::Exhausted)
}

/// At most one bounded page is retained. The host alone opens sessions and
/// resolves page requests. No filesystem, provider, network or tool authority.
#[derive(Debug)]
pub struct UiConversation {
    state: ConversationSnapshot,
    sequence: u64,
    next_request_id: u64,
}

impl UiConversation {
    pub fn new(host_epoch: u64) -> Result<Self, ConversationError> {
        counter(host_epoch).map_err(|_| ConversationError::Epoch)?;
        Ok(Self {
            state: ConversationSnapshot {
                host_epoch,
                session_generation: 1,
                revision: 1,
                session_id: None,
                page: None,
                pending: None,
            },
            sequence: 0,
            next_request_id: 0,
        })
    }

    pub fn snapshot(&self) -> ConversationSnapshot {
        self.state.clone()
    }

    /// Host binds an already-authorized store session; drop previous content.
    /// Returns a ticket for a fresh initial page. No data may be published
    /// without an exact outstanding ticket.
    pub fn open_from_host(&mut self, session_id: &str) -> Result<PageTicket, ConversationError> {
        valid_key(session_id)?;
        let generation = next(self.state.session_generation)?;
        let revision = next(self.state.revision)?;
        let request_id = next(self.next_request_id)?;
        self.state.session_generation = generation;
        self.state.revision = revision;
        self.state.session_id = Some(session_id.to_owned());
        self.state.page = None;
        self.next_request_id = request_id;
        let ticket = PageTicket {
            host_epoch: self.state.host_epoch,
            session_generation: generation,
            request_id,
            direction: PageDirection::Initial,
        };
        self.state.pending = Some(ticket.clone());
        Ok(ticket)
    }

    /// Retire renderer and all session data, including pending async results.
    pub fn rebind(&mut self, new_host_epoch: u64) -> Result<(), ConversationError> {
        counter(new_host_epoch).map_err(|_| ConversationError::Epoch)?;
        if new_host_epoch <= self.state.host_epoch {
            return Err(ConversationError::Epoch);
        }
        let generation = next(self.state.session_generation)?;
        let revision = next(self.state.revision)?;
        self.state = ConversationSnapshot {
            host_epoch: new_host_epoch,
            session_generation: generation,
            revision,
            session_id: None,
            page: None,
            pending: None,
        };
        self.sequence = 0;
        // Never reuse ticket identity during the lifetime of this model.
        Ok(())
    }

    /// Host can publish only the response to the currently pending request.
    /// Invalid or stale pages are rejected atomically; original page persists.
    pub fn publish_from_host(
        &mut self,
        ticket: &PageTicket,
        session_id: &str,
        page: ConversationPage,
    ) -> Result<(), ConversationError> {
        if self.state.pending.as_ref() != Some(ticket)
            || self.state.session_id.as_deref() != Some(session_id)
        {
            return Err(ConversationError::Stale);
        }
        page.validate()?;
        match ticket.direction {
            PageDirection::Initial => {
                if self.state.page.is_some() {
                    return Err(ConversationError::Stale);
                }
            }
            PageDirection::Older => {
                let previous = self.state.page.as_ref().ok_or(ConversationError::Stale)?;
                if page.total != previous.total || page.end() != previous.start {
                    return Err(ConversationError::InvalidPage);
                }
            }
            PageDirection::Newer => {
                let previous = self.state.page.as_ref().ok_or(ConversationError::Stale)?;
                if page.total != previous.total || page.start != previous.end() {
                    return Err(ConversationError::InvalidPage);
                }
            }
        }
        let revision = next(self.state.revision)?;
        self.state.page = Some(page);
        self.state.pending = None;
        self.state.revision = revision;
        Ok(())
    }

    /// Host reports an async load failure. Preserve visible content and
    /// allow an explicit retry; never echo backend error details to renderer.
    pub fn fail_from_host(&mut self, ticket: &PageTicket) -> Result<(), ConversationError> {
        if self.state.pending.as_ref() != Some(ticket) {
            return Err(ConversationError::Stale);
        }
        let revision = next(self.state.revision)?;
        self.state.pending = None;
        self.state.revision = revision;
        Ok(())
    }

    pub fn dispatch_json(&mut self, bytes: &[u8]) -> Result<ConversationReply, ConversationError> {
        if bytes.len() > MAX_CONVERSATION_REQUEST_BYTES {
            return Err(ConversationError::Oversize);
        }
        let request: ConversationRequest =
            serde_json::from_slice(bytes).map_err(|_| ConversationError::Malformed)?;
        self.dispatch(request)
    }

    pub fn dispatch(
        &mut self,
        request: ConversationRequest,
    ) -> Result<ConversationReply, ConversationError> {
        if request.protocol != UI_PROTOCOL_VERSION {
            return Err(ConversationError::Protocol);
        }
        if request.host_epoch != self.state.host_epoch {
            return Err(ConversationError::Epoch);
        }
        counter(request.sequence).map_err(|_| ConversationError::Sequence)?;
        if request.sequence != next(self.sequence).map_err(|_| ConversationError::Sequence)? {
            return Err(ConversationError::Sequence);
        }
        // Well-formed rejected commands consume sequence to prevent replay.
        self.sequence = request.sequence;
        match request.command {
            ConversationCommand::Snapshot {} => {}
            ConversationCommand::LoadOlder { expected_revision } => {
                self.request_page(expected_revision, PageDirection::Older)?;
            }
            ConversationCommand::LoadNewer { expected_revision } => {
                self.request_page(expected_revision, PageDirection::Newer)?;
            }
        }
        Ok(ConversationReply {
            protocol: UI_PROTOCOL_VERSION,
            sequence: request.sequence,
            state: self.snapshot(),
        })
    }

    fn request_page(
        &mut self,
        expected_revision: u64,
        direction: PageDirection,
    ) -> Result<(), ConversationError> {
        if expected_revision != self.state.revision {
            return Err(ConversationError::Revision);
        }
        if self.state.pending.is_some() {
            return Err(ConversationError::Pending);
        }
        let page = self
            .state
            .page
            .as_ref()
            .ok_or(ConversationError::Boundary)?;
        match direction {
            PageDirection::Older if page.start == 0 => return Err(ConversationError::Boundary),
            PageDirection::Newer if page.end() == page.total => {
                return Err(ConversationError::Boundary);
            }
            _ => {}
        }
        let request_id = next(self.next_request_id)?;
        let revision = next(self.state.revision)?;
        let ticket = PageTicket {
            host_epoch: self.state.host_epoch,
            session_generation: self.state.session_generation,
            request_id,
            direction,
        };
        self.next_request_id = request_id;
        self.state.revision = revision;
        self.state.pending = Some(ticket);
        Ok(())
    }
}
