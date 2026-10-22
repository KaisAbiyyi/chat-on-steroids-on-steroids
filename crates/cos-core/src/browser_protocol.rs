//! 7A: normalized protocol-14 browser identity and first-proof fencing.
//!
//! This is deliberately NOT the upstream extension wire parser. A future
//! authenticated native adapter (7B) must translate and verify upstream fields
//! before calling `observe_verified`. No transport, CDP, browser or Send action
//! is exposed here; 7P-L1 native feasibility is separately on hold.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const PROTOCOL_VERSION: u32 = crate::BRIDGE_PROTOCOL;
pub const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
pub const MAX_NORMALIZED_FRAME_BYTES: usize = 8192;
pub const MAX_ACTIVE_DOCUMENTS: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BridgeContractError {
    #[error("unsupported browser bridge protocol version")]
    Version,
    #[error("malformed or oversized normalized browser frame")]
    Frame,
    #[error("invalid browser identity field: {0}")]
    Identity(&'static str),
    #[error("a session is already attributed to another browser scope")]
    ConflictingAttribution,
    #[error("browser scope belongs to a different session/epoch")]
    ConflictingSession,
    #[error("stale browser navigation or worker generation")]
    Stale,
    #[error("document changed without a navigation epoch advance")]
    ConflictingDocument,
    #[error("browser identity registry is at its capacity")]
    Capacity,
    #[error("browser identity is not the current verified owner")]
    NotCurrent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentIdentity {
    /// Host-scoped, non-secret profile identifier. Never a filesystem path.
    pub profile_id: String,
    pub tab_id: u32,
    pub frame_id: u32,
    /// An opaque document identity supplied by the verified browser adapter.
    pub document_id: String,
    pub navigation_epoch: u64,
    /// Local session identity is not inferred from a page URL or tab title.
    pub session_id: String,
    pub session_epoch: u64,
    /// MV3/background incarnation, not an authentication token.
    pub worker_generation: u64,
}

impl DocumentIdentity {
    pub fn validate(&self) -> Result<(), BridgeContractError> {
        fn valid_id(text: &str) -> bool {
            !text.trim().is_empty()
                && text.encode_utf16().count() <= 256
                && !text.chars().any(char::is_control)
        }
        if !valid_id(&self.profile_id) {
            return Err(BridgeContractError::Identity("profileId"));
        }
        if !valid_id(&self.document_id) {
            return Err(BridgeContractError::Identity("documentId"));
        }
        if !valid_id(&self.session_id) {
            return Err(BridgeContractError::Identity("sessionId"));
        }
        if self.navigation_epoch > MAX_SAFE_JS_INTEGER {
            return Err(BridgeContractError::Identity("navigationEpoch"));
        }
        if self.session_epoch > MAX_SAFE_JS_INTEGER {
            return Err(BridgeContractError::Identity("sessionEpoch"));
        }
        if self.worker_generation > MAX_SAFE_JS_INTEGER {
            return Err(BridgeContractError::Identity("workerGeneration"));
        }
        Ok(())
    }

    fn scope(&self) -> Scope {
        Scope {
            profile: self.profile_id.clone(),
            tab: self.tab_id,
            frame: self.frame_id,
        }
    }

    fn session(&self) -> Session {
        Session {
            id: self.session_id.clone(),
            epoch: self.session_epoch,
        }
    }
}

/// An *internal normalized* frame, not a claim about the upstream JSON shape.
/// Unknown fields fail closed; the verified adapter must map original v14 wire
/// format to this structure without widening the authority of any field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NormalizedIdentityFrame {
    pub protocol: u32,
    pub identity: DocumentIdentity,
}

impl NormalizedIdentityFrame {
    pub fn parse(bytes: &[u8]) -> Result<Self, BridgeContractError> {
        if bytes.is_empty() || bytes.len() > MAX_NORMALIZED_FRAME_BYTES {
            return Err(BridgeContractError::Frame);
        }
        let frame: Self = serde_json::from_slice(bytes).map_err(|_| BridgeContractError::Frame)?;
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), BridgeContractError> {
        if self.protocol != PROTOCOL_VERSION {
            return Err(BridgeContractError::Version);
        }
        self.identity.validate()
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct Scope {
    profile: String,
    tab: u32,
    frame: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct Session {
    id: String,
    epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    FirstProof,
    Idempotent,
    Advanced,
}

/// In-memory monotonic first-proof register. Not a session store or transport.
///
/// Only a *trusted adapter* that has already authenticated the host, checked
/// origin/profile/tab ownership and obtained a real navigation/document proof
/// may call `observe_verified`. This API alone does not authenticate a source.
///
/// No claim can move to another tab/frame/profile or another session merely
/// because a later event arrives. Intent claims/ACK/retry belong to 7B/7C.
#[derive(Debug, Default)]
pub struct IdentityFence {
    current: HashMap<Scope, DocumentIdentity>,
    first_proof: HashMap<Session, Scope>,
}

impl IdentityFence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_scopes(&self) -> usize {
        self.current.len()
    }

    pub fn observe_verified(
        &mut self,
        incoming: DocumentIdentity,
    ) -> Result<Observation, BridgeContractError> {
        incoming.validate()?;
        let scope = incoming.scope();
        let session = incoming.session();

        if let Some(winner) = self.first_proof.get(&session)
            && winner != &scope
        {
            return Err(BridgeContractError::ConflictingAttribution);
        }
        if let Some(existing) = self.current.get(&scope) {
            // Rebinding the session/epoch is a separate authenticated protocol
            // operation, never an implicit effect of a browser observation.
            if existing.session() != session {
                return Err(BridgeContractError::ConflictingSession);
            }
            if incoming.navigation_epoch < existing.navigation_epoch
                || incoming.worker_generation < existing.worker_generation
            {
                return Err(BridgeContractError::Stale);
            }
            if incoming.navigation_epoch == existing.navigation_epoch
                && incoming.document_id != existing.document_id
            {
                return Err(BridgeContractError::ConflictingDocument);
            }
            if *existing == incoming {
                return Ok(Observation::Idempotent);
            }
            // Nothing mutates until all checks succeed.
            self.current.insert(scope, incoming);
            return Ok(Observation::Advanced);
        }
        if self.current.len() >= MAX_ACTIVE_DOCUMENTS
            || self.first_proof.len() >= MAX_ACTIVE_DOCUMENTS
        {
            return Err(BridgeContractError::Capacity);
        }
        self.first_proof.insert(session, scope.clone());
        self.current.insert(scope, incoming);
        Ok(Observation::FirstProof)
    }

    /// Admission requires an *exact* current identity, not just a matching
    /// session, URL, tab, or increasing client-provided counter.
    pub fn require_current(&self, identity: &DocumentIdentity) -> Result<(), BridgeContractError> {
        identity.validate()?;
        if self.current.get(&identity.scope()) == Some(identity)
            && self.first_proof.get(&identity.session()) == Some(&identity.scope())
        {
            Ok(())
        } else {
            Err(BridgeContractError::NotCurrent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn identity() -> DocumentIdentity {
        DocumentIdentity {
            profile_id: "profile-A".into(),
            tab_id: 7,
            frame_id: 0,
            document_id: "doc-A".into(),
            navigation_epoch: 1,
            session_id: "local-session-A".into(),
            session_epoch: 3,
            worker_generation: 1,
        }
    }

    #[test]
    fn version_14_is_required_and_internal_shape_is_strict() {
        let value = json!({"protocol":14,"identity":identity()});
        let parsed = NormalizedIdentityFrame::parse(value.to_string().as_bytes()).unwrap();
        assert_eq!(parsed.protocol, crate::BRIDGE_PROTOCOL);
        for v in [13, 15, 0] {
            let mut wrong = value.clone();
            wrong["protocol"] = json!(v);
            assert_eq!(
                NormalizedIdentityFrame::parse(wrong.to_string().as_bytes()),
                Err(BridgeContractError::Version)
            );
        }
        let mut extra = value.clone();
        extra["unexpectedAuthority"] = json!(true);
        assert_eq!(
            NormalizedIdentityFrame::parse(extra.to_string().as_bytes()),
            Err(BridgeContractError::Frame)
        );
        assert_eq!(
            NormalizedIdentityFrame::parse(&vec![b' '; MAX_NORMALIZED_FRAME_BYTES + 1]),
            Err(BridgeContractError::Frame)
        );
        assert!(NormalizedIdentityFrame::parse(b"{").is_err());
    }

    #[test]
    fn first_proof_wins_across_tabs_profiles_and_frames() {
        let mut fence = IdentityFence::new();
        let id = identity();
        assert_eq!(
            fence.observe_verified(id.clone()),
            Ok(Observation::FirstProof)
        );
        assert_eq!(
            fence.observe_verified(id.clone()),
            Ok(Observation::Idempotent)
        );
        for spoof in [
            DocumentIdentity {
                tab_id: 8,
                ..id.clone()
            },
            DocumentIdentity {
                frame_id: 1,
                ..id.clone()
            },
            DocumentIdentity {
                profile_id: "other-profile".into(),
                ..id.clone()
            },
        ] {
            assert_eq!(
                fence.observe_verified(spoof),
                Err(BridgeContractError::ConflictingAttribution)
            );
        }
        assert_eq!(fence.active_scopes(), 1);
        assert!(fence.require_current(&id).is_ok());
    }

    #[test]
    fn navigation_and_mv3_restart_fence_previous_observations() {
        let mut fence = IdentityFence::new();
        let old = identity();
        fence.observe_verified(old.clone()).unwrap();
        let newer = DocumentIdentity {
            document_id: "doc-B".into(),
            navigation_epoch: 2,
            worker_generation: 2,
            ..old.clone()
        };
        assert_eq!(
            fence.observe_verified(newer.clone()),
            Ok(Observation::Advanced)
        );
        assert_eq!(
            fence.require_current(&old),
            Err(BridgeContractError::NotCurrent)
        );
        assert_eq!(fence.observe_verified(old), Err(BridgeContractError::Stale));
        assert!(fence.require_current(&newer).is_ok());
        let stale_worker = DocumentIdentity {
            worker_generation: 1,
            ..newer.clone()
        };
        assert_eq!(
            fence.observe_verified(stale_worker),
            Err(BridgeContractError::Stale)
        );
        assert!(fence.require_current(&newer).is_ok());
    }

    #[test]
    fn same_epoch_document_switch_and_session_rebind_fail_closed() {
        let mut fence = IdentityFence::new();
        let first = identity();
        fence.observe_verified(first.clone()).unwrap();
        let fake = DocumentIdentity {
            document_id: "replacement".into(),
            ..first.clone()
        };
        assert_eq!(
            fence.observe_verified(fake),
            Err(BridgeContractError::ConflictingDocument)
        );
        let fake = DocumentIdentity {
            session_id: "different-local-session".into(),
            session_epoch: 4,
            navigation_epoch: 2,
            ..first.clone()
        };
        assert_eq!(
            fence.observe_verified(fake),
            Err(BridgeContractError::ConflictingSession)
        );
        assert!(fence.require_current(&first).is_ok());
        assert_eq!(fence.active_scopes(), 1);
    }

    #[test]
    fn malformed_and_unsafe_js_ids_do_not_acquire_authority() {
        let mut fence = IdentityFence::new();
        for bad in [
            DocumentIdentity {
                profile_id: " \t".into(),
                ..identity()
            },
            DocumentIdentity {
                document_id: "doc\nrogue".into(),
                ..identity()
            },
            DocumentIdentity {
                session_id: "X".repeat(257),
                ..identity()
            },
            DocumentIdentity {
                navigation_epoch: MAX_SAFE_JS_INTEGER + 1,
                ..identity()
            },
            DocumentIdentity {
                session_epoch: MAX_SAFE_JS_INTEGER + 1,
                ..identity()
            },
            DocumentIdentity {
                worker_generation: MAX_SAFE_JS_INTEGER + 1,
                ..identity()
            },
        ] {
            assert!(matches!(
                fence.observe_verified(bad),
                Err(BridgeContractError::Identity(_))
            ));
        }
        assert_eq!(fence.active_scopes(), 0);
    }
}
