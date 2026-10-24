//! Phase 15E: host-owned, presentation-only keyboard and focus contract.
//! Epoch and sequence are replay fences, never renderer authentication.
//! Native host MUST authenticate transport before dispatching any frames.
use serde::{Deserialize, Serialize};

use crate::ui_contract::{MAX_JS_SAFE_INTEGER, MAX_UI_IPC_BYTES};
use crate::ui_localization::UiTextKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiFocusTarget {
    Navigation,
    Main,
    Composer,
    DialogConfirm,
    DialogCancel,
}

impl UiFocusTarget {
    pub const fn label(self) -> UiTextKey {
        match self {
            Self::Navigation => UiTextKey::Navigation,
            Self::Main => UiTextKey::MainContent,
            Self::Composer => UiTextKey::MessageInput,
            Self::DialogConfirm => UiTextKey::Confirm,
            Self::DialogCancel => UiTextKey::Cancel,
        }
    }

    fn is_dialog(self) -> bool {
        matches!(self, Self::DialogConfirm | Self::DialogCancel)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiFocusSnapshot {
    pub host_epoch: u64,
    pub revision: u64,
    pub target: UiFocusTarget,
    pub dialog_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiKeyOutcome {
    Handled,
    PassThrough,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiKeyReply {
    pub sequence: u64,
    pub outcome: UiKeyOutcome,
    pub state: UiFocusSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiKeyFrame {
    pub host_epoch: u64,
    pub sequence: u64,
    pub event: UiKeyEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "key", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UiKeyEvent {
    Tab {
        shift: bool,
        ctrl: bool,
        alt: bool,
        meta: bool,
        composing: bool,
    },
    Escape {
        composing: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UiFocusError {
    #[error("malformed focus event")]
    Malformed,
    #[error("oversized focus event")]
    Oversize,
    #[error("invalid host epoch")]
    Epoch,
    #[error("invalid sequence")]
    Sequence,
    #[error("focus target rejected")]
    Focus,
    #[error("dialog transition rejected")]
    Dialog,
    #[error("focus revision exhausted")]
    Exhausted,
}

impl UiFocusError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Malformed => "invalid-key-event",
            Self::Oversize => "payload-too-large",
            Self::Epoch => "stale-host-epoch",
            Self::Sequence => "invalid-sequence",
            Self::Focus => "invalid-focus",
            Self::Dialog => "invalid-dialog-state",
            Self::Exhausted => "counter-exhausted",
        }
    }
}

/// Native-host presentation state, with no file, tool, OS input or DOM authority.
#[derive(Debug)]
pub struct UiFocus {
    state: UiFocusSnapshot,
    restore_target: Option<UiFocusTarget>,
    last_sequence: u64,
}

impl UiFocus {
    pub fn new(host_epoch: u64) -> Result<Self, UiFocusError> {
        valid_epoch(host_epoch)?;
        Ok(Self {
            state: UiFocusSnapshot {
                host_epoch,
                revision: 1,
                target: UiFocusTarget::Navigation,
                dialog_open: false,
            },
            restore_target: None,
            last_sequence: 0,
        })
    }

    pub fn snapshot(&self) -> UiFocusSnapshot {
        self.state.clone()
    }

    /// For privileged host callbacks ONLY; renderer cannot request arbitrary nodes.
    pub fn set_host_focus(&mut self, target: UiFocusTarget) -> Result<(), UiFocusError> {
        if target.is_dialog() != self.state.dialog_open {
            return Err(UiFocusError::Focus);
        }
        self.move_focus(target)
    }

    pub fn open_dialog(&mut self) -> Result<(), UiFocusError> {
        if self.state.dialog_open {
            return Err(UiFocusError::Dialog);
        }
        let revision = next(self.state.revision)?;
        self.restore_target = Some(self.state.target);
        self.state.dialog_open = true;
        self.state.target = UiFocusTarget::DialogConfirm;
        self.state.revision = revision;
        Ok(())
    }

    pub fn close_dialog(&mut self) -> Result<(), UiFocusError> {
        if !self.state.dialog_open {
            return Err(UiFocusError::Dialog);
        }
        let revision = next(self.state.revision)?;
        self.state.dialog_open = false;
        self.state.target = self
            .restore_target
            .take()
            .unwrap_or(UiFocusTarget::Navigation);
        self.state.revision = revision;
        Ok(())
    }

    /// Window recreation retires all modal state and old keyboard frames.
    pub fn rebind(&mut self, next_epoch: u64) -> Result<(), UiFocusError> {
        valid_epoch(next_epoch)?;
        if next_epoch <= self.state.host_epoch {
            return Err(UiFocusError::Epoch);
        }
        let revision = next(self.state.revision)?;
        self.state = UiFocusSnapshot {
            host_epoch: next_epoch,
            revision,
            target: UiFocusTarget::Navigation,
            dialog_open: false,
        };
        self.restore_target = None;
        self.last_sequence = 0;
        Ok(())
    }

    pub fn handle_json(&mut self, payload: &[u8]) -> Result<UiKeyReply, UiFocusError> {
        if payload.len() > MAX_UI_IPC_BYTES {
            return Err(UiFocusError::Oversize);
        }
        let frame: UiKeyFrame =
            serde_json::from_slice(payload).map_err(|_| UiFocusError::Malformed)?;
        self.handle(frame)
    }

    pub fn handle(&mut self, frame: UiKeyFrame) -> Result<UiKeyReply, UiFocusError> {
        if frame.host_epoch != self.state.host_epoch {
            return Err(UiFocusError::Epoch);
        }
        if frame.sequence == 0
            || frame.sequence > MAX_JS_SAFE_INTEGER
            || next(self.last_sequence).map_err(|_| UiFocusError::Sequence)? != frame.sequence
        {
            return Err(UiFocusError::Sequence);
        }
        // Consume valid sequence even for pass-through or rejected host revision.
        self.last_sequence = frame.sequence;
        let outcome = match frame.event {
            UiKeyEvent::Tab {
                shift,
                ctrl,
                alt,
                meta,
                composing,
            } => {
                if composing || ctrl || alt || meta {
                    UiKeyOutcome::PassThrough
                } else {
                    let target = if self.state.dialog_open {
                        // Two bounded known controls; keyboard cannot escape an open modal.
                        match self.state.target {
                            UiFocusTarget::DialogConfirm => UiFocusTarget::DialogCancel,
                            _ => UiFocusTarget::DialogConfirm,
                        }
                    } else {
                        match (self.state.target, shift) {
                            (UiFocusTarget::Navigation, false) => UiFocusTarget::Main,
                            (UiFocusTarget::Main, false) => UiFocusTarget::Composer,
                            (UiFocusTarget::Composer, false) => UiFocusTarget::Navigation,
                            (UiFocusTarget::Navigation, true) => UiFocusTarget::Composer,
                            (UiFocusTarget::Main, true) => UiFocusTarget::Navigation,
                            _ => UiFocusTarget::Main,
                        }
                    };
                    self.move_focus(target)?;
                    UiKeyOutcome::Handled
                }
            }
            UiKeyEvent::Escape { composing } => {
                if composing || !self.state.dialog_open {
                    UiKeyOutcome::PassThrough
                } else {
                    self.close_dialog()?;
                    UiKeyOutcome::Handled
                }
            }
        };
        Ok(UiKeyReply {
            sequence: frame.sequence,
            outcome,
            state: self.snapshot(),
        })
    }

    fn move_focus(&mut self, target: UiFocusTarget) -> Result<(), UiFocusError> {
        if self.state.target != target {
            let revision = next(self.state.revision)?;
            self.state.target = target;
            self.state.revision = revision;
        }
        Ok(())
    }
}

fn valid_epoch(epoch: u64) -> Result<(), UiFocusError> {
    if epoch == 0 || epoch > MAX_JS_SAFE_INTEGER {
        Err(UiFocusError::Epoch)
    } else {
        Ok(())
    }
}

fn next(value: u64) -> Result<u64, UiFocusError> {
    value
        .checked_add(1)
        .filter(|next| *next <= MAX_JS_SAFE_INTEGER)
        .ok_or(UiFocusError::Exhausted)
}
