//! Phase 15A: host-owned UI navigation and renderer IPC contract.
//!
//! This is presentation state only. It deliberately has no tool invocation,
//! file IO, browser permission, arbitrary URL or native-window authority.
//! A privileged native host must authenticate the actual transport separately;
//! the epoch/sequence below fence stale messages, they are NOT credentials.
use serde::{Deserialize, Serialize};

pub const UI_PROTOCOL_VERSION: u32 = 1;
pub const MAX_UI_IPC_BYTES: usize = 8 * 1024;
pub const MAX_UI_ROUTE_ID_BYTES: usize = 128;
pub const MAX_JS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "view", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UiRoute {
    Home,
    Conversation { session_id: String },
    Projects,
    Project { project_id: String },
    Files { project_id: String },
    Agents,
    Skills,
    Plugins,
    Settings,
    Activity,
}

impl UiRoute {
    fn validate(&self) -> Result<(), UiError> {
        match self {
            Self::Conversation { session_id } => valid_route_id(session_id),
            Self::Project { project_id } | Self::Files { project_id } => valid_route_id(project_id),
            _ => Ok(()),
        }
    }
}

/// A route identifier is an opaque key, NEVER a path or URL.
fn valid_route_id(id: &str) -> Result<(), UiError> {
    if id.is_empty()
        || id.len() > MAX_UI_ROUTE_ID_BYTES
        || id.starts_with('.')
        || id.ends_with('.')
        || id.contains("..")
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
    {
        return Err(UiError::Route);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UiCommand {
    Snapshot,
    Navigate {
        route: UiRoute,
        expected_revision: u64,
    },
    SetSidebar {
        open: bool,
        expected_revision: u64,
    },
}

/// Transport source must be verified by the native host, independently of
/// this deserialized envelope. Renderer fields cannot grant capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiRequest {
    pub protocol: u32,
    pub host_epoch: u64,
    pub sequence: u64,
    pub command: UiCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSnapshot {
    pub host_epoch: u64,
    pub revision: u64,
    pub route: UiRoute,
    pub sidebar_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiReply {
    pub protocol: u32,
    pub host_epoch: u64,
    pub sequence: u64,
    pub state: UiSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UiError {
    #[error("UI IPC payload exceeds limit")]
    Oversize,
    #[error("malformed or unknown UI IPC command")]
    Malformed,
    #[error("unsupported UI IPC protocol")]
    Protocol,
    #[error("stale or invalid host epoch")]
    Epoch,
    #[error("out-of-order or replayed UI IPC command")]
    Sequence,
    #[error("stale navigation state revision")]
    Revision,
    #[error("unsafe or invalid opaque route identifier")]
    Route,
    #[error("UI IPC counter exhausted")]
    Exhausted,
}

impl UiError {
    /// Stable, non-sensitive response codes; never return raw renderer input.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Oversize => "payload-too-large",
            Self::Malformed => "invalid-request",
            Self::Protocol => "unsupported-protocol",
            Self::Epoch => "stale-host-epoch",
            Self::Sequence => "invalid-sequence",
            Self::Revision => "stale-revision",
            Self::Route => "invalid-route",
            Self::Exhausted => "counter-exhausted",
        }
    }
}

/// Only the owning host constructs/rebinds this model. One ordered renderer
/// channel per host epoch. Native transport origin/identity checks are external.
#[derive(Debug)]
pub struct UiNavigation {
    state: UiSnapshot,
    last_sequence: u64,
}

impl UiNavigation {
    pub fn new(host_epoch: u64) -> Result<Self, UiError> {
        valid_counter(host_epoch).map_err(|_| UiError::Epoch)?;
        Ok(Self {
            state: UiSnapshot {
                host_epoch,
                revision: 1,
                route: UiRoute::Home,
                sidebar_open: false,
            },
            last_sequence: 0,
        })
    }

    pub fn snapshot(&self) -> UiSnapshot {
        self.state.clone()
    }

    /// Retire old renderer messages after native window/renderer recreation.
    /// A new epoch is host-assigned and cannot reuse an earlier generation.
    /// Returning to Home avoids implicitly reviving a previous conversation.
    pub fn rebind(&mut self, new_host_epoch: u64) -> Result<(), UiError> {
        valid_counter(new_host_epoch).map_err(|_| UiError::Epoch)?;
        if new_host_epoch <= self.state.host_epoch {
            return Err(UiError::Epoch);
        }
        let revision = increment(self.state.revision)?;
        self.state = UiSnapshot {
            host_epoch: new_host_epoch,
            revision,
            route: UiRoute::Home,
            sidebar_open: false,
        };
        self.last_sequence = 0;
        Ok(())
    }

    /// Decode only a complete bounded JSON frame; no side effects for
    /// oversized, malformed, duplicate-field or unknown-field messages.
    pub fn dispatch_json(&mut self, bytes: &[u8]) -> Result<UiReply, UiError> {
        if bytes.len() > MAX_UI_IPC_BYTES {
            return Err(UiError::Oversize);
        }
        let request: UiRequest = serde_json::from_slice(bytes).map_err(|_| UiError::Malformed)?;
        self.dispatch(request)
    }

    pub fn dispatch(&mut self, request: UiRequest) -> Result<UiReply, UiError> {
        if request.protocol != UI_PROTOCOL_VERSION {
            return Err(UiError::Protocol);
        }
        if request.host_epoch != self.state.host_epoch {
            return Err(UiError::Epoch);
        }
        valid_counter(request.sequence).map_err(|_| UiError::Sequence)?;
        if request.sequence != increment(self.last_sequence).map_err(|_| UiError::Sequence)? {
            return Err(UiError::Sequence);
        }

        // This sequence is consumed even if a well-formed action is refused;
        // a rejected old revision cannot later be replayed as a fresh action.
        self.last_sequence = request.sequence;
        match request.command {
            UiCommand::Snapshot => {}
            UiCommand::Navigate {
                route,
                expected_revision,
            } => {
                self.expect_revision(expected_revision)?;
                route.validate()?;
                if self.state.route != route {
                    let next = increment(self.state.revision)?;
                    self.state.route = route;
                    self.state.revision = next;
                }
            }
            UiCommand::SetSidebar {
                open,
                expected_revision,
            } => {
                self.expect_revision(expected_revision)?;
                if self.state.sidebar_open != open {
                    let next = increment(self.state.revision)?;
                    self.state.sidebar_open = open;
                    self.state.revision = next;
                }
            }
        }

        Ok(UiReply {
            protocol: UI_PROTOCOL_VERSION,
            host_epoch: self.state.host_epoch,
            sequence: request.sequence,
            state: self.snapshot(),
        })
    }

    fn expect_revision(&self, expected: u64) -> Result<(), UiError> {
        if expected != self.state.revision {
            Err(UiError::Revision)
        } else {
            Ok(())
        }
    }
}

fn valid_counter(value: u64) -> Result<(), UiError> {
    if value == 0 || value > MAX_JS_SAFE_INTEGER {
        Err(UiError::Exhausted)
    } else {
        Ok(())
    }
}

fn increment(value: u64) -> Result<u64, UiError> {
    value
        .checked_add(1)
        .filter(|next| *next <= MAX_JS_SAFE_INTEGER)
        .ok_or(UiError::Exhausted)
}
