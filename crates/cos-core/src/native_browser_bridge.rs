//! 7B: authenticated **native** bridge authority, not an upstream wire adapter.
//!
//! Privileged host glue (future 7C/7L1) must obtain TrustedHostEvidence from
//! native tab/profile/document events, not page-supplied JSON, and deliver the
//! bootstrap key only to its isolated extension transport. No network listener,
//! WebView2 integration, provider login or browser command execution is implied.
use crate::browser_protocol::{
    BridgeContractError, DocumentIdentity, IdentityFence, MAX_ACTIVE_DOCUMENTS,
    MAX_NORMALIZED_FRAME_BYTES, MAX_SAFE_JS_INTEGER, NormalizedIdentityFrame, Observation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const KEY_BYTES: usize = 32;
const HMAC_BLOCK: usize = 64;
const HANDSHAKE: &[u8] = b"CoSoS/7B/handshake/v1";
const OBSERVATION: &[u8] = b"CoSoS/7B/observation/v1";
const AUTHORIZATION: &[u8] = b"CoSoS/7B/authorization/v1";
const COMPANION_EVENT: &[u8] = b"CoSoS/7L1/companion-event/v1";
const COMPANION_RECEIPT: &[u8] = b"CoSoS/7L1/companion-receipt/v1";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NativeBridgeError {
    #[error("untrusted host context or non-HTTPS origin")]
    Host,
    #[error("host generation is stale or invalid")]
    Generation,
    #[error("unknown, retired or mismatched native bridge channel")]
    Channel,
    #[error("bridge handshake is incomplete or already consumed")]
    Handshake,
    #[error("invalid bridge MAC")]
    Authentication,
    #[error("replayed or out-of-order bridge message")]
    Sequence,
    #[error("bridge channel capacity exhausted")]
    Capacity,
    #[error("bridge randomness unavailable")]
    Entropy,
    #[error(transparent)]
    Contract(#[from] BridgeContractError),
}

/// MUST come from the native host's trusted browser-event surface. Caller
/// asserts the exact profile/tab/frame/document/session/worker binding and
/// origin from outside the renderer. It is never deserialized from page input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedHostEvidence {
    pub identity: DocumentIdentity,
    pub origin: String,
    pub host_generation: u64,
}

impl TrustedHostEvidence {
    fn validate(&self) -> Result<(), NativeBridgeError> {
        self.identity.validate()?;
        // Fail closed: there is no custom-scheme, file, blank or opaque origin
        // admission in this bounded cloud unit. Product allowlist is future work.
        let authority = self
            .origin
            .strip_prefix("https://")
            .ok_or(NativeBridgeError::Host)?;
        if authority.is_empty()
            || authority.bytes().any(|b| b <= 0x20 || b == 0x7f)
            || authority
                .chars()
                .any(|c| matches!(c, '/' | '?' | '#' | '@' | '\\'))
            || authority.starts_with('.')
            || authority.ends_with('.')
            || self.origin.len() > 512
        {
            return Err(NativeBridgeError::Host);
        }
        if self.host_generation == 0 || self.host_generation > MAX_SAFE_JS_INTEGER {
            return Err(NativeBridgeError::Generation);
        }
        Ok(())
    }

    fn scope(&self) -> (String, u32, u32) {
        (
            self.identity.profile_id.clone(),
            self.identity.tab_id,
            self.identity.frame_id,
        )
    }
}

/// One-use privileged bootstrap. Its key MUST NOT be logged, exposed in the
/// page's MAIN world or placed in persistent browser storage. Only an isolated
/// host/extension transport with verified source metadata may receive it.
#[derive(Clone)]
pub struct BridgeBootstrap {
    pub channel_id: u64,
    pub host_generation: u64,
    pub challenge: [u8; KEY_BYTES],
    key: [u8; KEY_BYTES],
}

impl BridgeBootstrap {
    /// Only a privileged browser-host adapter may deliver this one-use secret
    /// directly to an attached, extension-ID-verified MV3 worker target.
    /// Never pass it through WebMessageReceived, pages, storage or logs.
    pub fn isolated_worker_key(&self) -> [u8; KEY_BYTES] {
        self.key
    }
    pub fn handshake_proof(&self) -> [u8; KEY_BYTES] {
        self.mac(HANDSHAKE, 0, &[])
    }

    pub fn observation_mac(&self, sequence: u64, bytes: &[u8]) -> [u8; KEY_BYTES] {
        self.mac(OBSERVATION, sequence, bytes)
    }

    pub fn authorization_mac(&self, sequence: u64, bytes: &[u8]) -> [u8; KEY_BYTES] {
        self.mac(AUTHORIZATION, sequence, bytes)
    }

    /// Only the isolated MV3 worker uses this to sign the exact 7C event.
    pub fn companion_event_mac(&self, sequence: u64, bytes: &[u8]) -> [u8; KEY_BYTES] {
        self.mac(COMPANION_EVENT, sequence, bytes)
    }

    /// Worker checks the host's successful authorization receipt prior to
    /// dispatching the corresponding 7C event.
    pub fn companion_receipt_mac(&self, sequence: u64, bytes: &[u8]) -> [u8; KEY_BYTES] {
        self.mac(COMPANION_RECEIPT, sequence, bytes)
    }

    fn mac(&self, domain: &[u8], sequence: u64, bytes: &[u8]) -> [u8; KEY_BYTES] {
        authenticated_mac(
            &self.key,
            domain,
            self.channel_id,
            self.host_generation,
            sequence,
            &self.challenge,
            bytes,
        )
    }
}

/// Authenticated 7C mutation. The event itself is included in the 7B MAC;
/// authorization of an identity frame alone never authorizes a ledger write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanionEvent {
    pub protocol: u32,
    pub kind: String,
    pub sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_id: Option<String>,
}

impl CompanionEvent {
    fn validate(&self) -> Result<(), NativeBridgeError> {
        fn valid_id(text: &str) -> bool {
            !text.is_empty()
                && text.len() <= 128
                && text
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
        }
        let intent = || {
            self.intent_id.as_ref().is_some_and(|s| valid_id(s))
                && self.digest.as_ref().is_some_and(|s| {
                    s.len() == 64
                        && s.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
        };
        if self.protocol != crate::BRIDGE_PROTOCOL
            || self.sequence == 0
            || self.sequence > MAX_SAFE_JS_INTEGER
            || !match self.kind.as_str() {
                "attach" | "retire" => {
                    self.intent_id.is_none() && self.digest.is_none() && self.receipt_id.is_none()
                }
                "claim" | "repair" => intent() && self.receipt_id.is_none(),
                "ack" => intent() && self.receipt_id.as_ref().is_some_and(|s| valid_id(s)),
                _ => false,
            }
        {
            return Err(NativeBridgeError::Contract(BridgeContractError::Frame));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionSignedPayload {
    pub frame: NormalizedIdentityFrame,
    pub event: CompanionEvent,
}

struct Channel {
    evidence: TrustedHostEvidence,
    bootstrap: BridgeBootstrap,
    authenticated: bool,
    last_sequence: u64,
}

/// Host-scoped, memory-only generation fencing and strict MAC/sequence gate.
/// The identity fence survives a channel re-registration; a host generation
/// bump never silently transfers a local session to another browser scope.
#[derive(Default)]
pub struct NativeBrowserBridge {
    fence: IdentityFence,
    channels: HashMap<u64, Channel>,
    owners: HashMap<(String, u32, u32), u64>,
    next_channel_id: u64,
}

impl NativeBrowserBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_channels(&self) -> usize {
        self.channels.len()
    }

    /// Called ONLY after the native host has separately attested its event
    /// source. Registering the same scope requires a strictly newer native
    /// generation, retires the old channel, and forces a new handshake.
    pub fn register_host(
        &mut self,
        evidence: TrustedHostEvidence,
    ) -> Result<BridgeBootstrap, NativeBridgeError> {
        evidence.validate()?;
        let scope = evidence.scope();
        let old = self.owners.get(&scope).copied();
        if let Some(id) = old {
            let previous = self.channels.get(&id).ok_or(NativeBridgeError::Channel)?;
            if evidence.host_generation <= previous.evidence.host_generation {
                return Err(NativeBridgeError::Generation);
            }
        } else if self.channels.len() >= MAX_ACTIVE_DOCUMENTS {
            return Err(NativeBridgeError::Capacity);
        }
        let channel_id = self
            .next_channel_id
            .checked_add(1)
            .ok_or(NativeBridgeError::Capacity)?;
        if channel_id > MAX_SAFE_JS_INTEGER {
            return Err(NativeBridgeError::Capacity);
        }
        let mut challenge = [0u8; KEY_BYTES];
        let mut key = [0u8; KEY_BYTES];
        getrandom::fill(&mut challenge).map_err(|_| NativeBridgeError::Entropy)?;
        getrandom::fill(&mut key).map_err(|_| NativeBridgeError::Entropy)?;
        let bootstrap = BridgeBootstrap {
            channel_id,
            host_generation: evidence.host_generation,
            challenge,
            key,
        };
        if let Some(id) = old {
            self.channels.remove(&id);
        }
        self.owners.insert(scope, channel_id);
        self.channels.insert(
            channel_id,
            Channel {
                evidence,
                bootstrap: bootstrap.clone(),
                authenticated: false,
                last_sequence: 0,
            },
        );
        self.next_channel_id = channel_id;
        Ok(bootstrap)
    }

    /// Retire a native channel on host teardown. A stale teardown cannot
    /// revoke its replacement. Session identity is deliberately NOT reset.
    pub fn retire_host(&mut self, channel_id: u64) -> bool {
        let Some(old) = self.channels.remove(&channel_id) else {
            return false;
        };
        if self.owners.get(&old.evidence.scope()) == Some(&channel_id) {
            self.owners.remove(&old.evidence.scope());
        }
        true
    }

    pub fn complete_handshake(
        &mut self,
        trusted: &TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
        proof: &[u8; KEY_BYTES],
    ) -> Result<(), NativeBridgeError> {
        let channel = self.current_channel(trusted, channel_id, generation)?;
        if channel.authenticated {
            return Err(NativeBridgeError::Handshake);
        }
        let expected = channel.bootstrap.handshake_proof();
        if !constant_time_equal(&expected, proof) {
            return Err(NativeBridgeError::Authentication);
        }
        self.channels
            .get_mut(&channel_id)
            .ok_or(NativeBridgeError::Channel)?
            .authenticated = true;
        Ok(())
    }

    /// A signed, version-checked normalized observation, never raw upstream
    /// extension JSON. Host proof must match the FULL identity in the frame.
    pub fn observe(
        &mut self,
        trusted: &TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: &[u8],
        mac: &[u8; KEY_BYTES],
    ) -> Result<Observation, NativeBridgeError> {
        self.check_message(
            trusted,
            channel_id,
            generation,
            sequence,
            (OBSERVATION, payload, mac),
        )?;
        let frame = NormalizedIdentityFrame::parse(payload)?;
        if frame.identity != trusted.identity {
            return Err(NativeBridgeError::Host);
        }
        let observation = self.fence.observe_frame(frame)?;
        self.channels
            .get_mut(&channel_id)
            .ok_or(NativeBridgeError::Channel)?
            .last_sequence = sequence;
        Ok(observation)
    }

    /// Strict per-event admission for a future command handler. Does NOT
    /// execute commands, grant user consent or bypass higher-level tool policy.
    pub fn authorize_current(
        &mut self,
        trusted: &TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: &[u8],
        mac: &[u8; KEY_BYTES],
    ) -> Result<(), NativeBridgeError> {
        self.check_message(
            trusted,
            channel_id,
            generation,
            sequence,
            (AUTHORIZATION, payload, mac),
        )?;
        let frame = NormalizedIdentityFrame::parse(payload)?;
        if frame.identity != trusted.identity {
            return Err(NativeBridgeError::Host);
        }
        self.fence.require_current_frame(&frame)?;
        self.channels
            .get_mut(&channel_id)
            .ok_or(NativeBridgeError::Channel)?
            .last_sequence = sequence;
        Ok(())
    }

    fn current_channel(
        &self,
        trusted: &TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
    ) -> Result<&Channel, NativeBridgeError> {
        trusted.validate()?;
        let channel = self
            .channels
            .get(&channel_id)
            .ok_or(NativeBridgeError::Channel)?;
        if self.owners.get(&trusted.scope()) != Some(&channel_id)
            || channel.evidence != *trusted
            || generation != channel.evidence.host_generation
        {
            return Err(NativeBridgeError::Channel);
        }
        Ok(channel)
    }

    fn check_message(
        &self,
        trusted: &TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        signed: (&[u8], &[u8], &[u8; KEY_BYTES]),
    ) -> Result<(), NativeBridgeError> {
        let (domain, payload, mac) = signed;
        let channel = self.current_channel(trusted, channel_id, generation)?;
        if !channel.authenticated {
            return Err(NativeBridgeError::Handshake);
        }
        let next = channel
            .last_sequence
            .checked_add(1)
            .ok_or(NativeBridgeError::Sequence)?;
        if sequence != next || sequence > MAX_SAFE_JS_INTEGER {
            return Err(NativeBridgeError::Sequence);
        }
        if payload.is_empty() || payload.len() > MAX_NORMALIZED_FRAME_BYTES {
            return Err(NativeBridgeError::Contract(BridgeContractError::Frame));
        }
        let expected = channel.bootstrap.mac(domain, sequence, payload);
        if !constant_time_equal(&expected, mac) {
            return Err(NativeBridgeError::Authentication);
        }
        Ok(())
    }

    /// Authorize exactly one signed ledger event, then issue a signed receipt
    /// to the isolated worker. The worker MUST verify the receipt over the
    /// identical event bytes before mutating its chrome.storage.session ledger.
    /// 7B's sequence is separate from the 7C event.sequence field.
    pub fn authorize_companion_event(
        &mut self,
        trusted: &TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
        bridge_sequence: u64,
        payload: &[u8],
        mac: &[u8; KEY_BYTES],
    ) -> Result<[u8; KEY_BYTES], NativeBridgeError> {
        self.check_message(
            trusted,
            channel_id,
            generation,
            bridge_sequence,
            (COMPANION_EVENT, payload, mac),
        )?;
        let request: CompanionSignedPayload =
            serde_json::from_slice(payload).map_err(|_| BridgeContractError::Frame)?;
        request.frame.validate()?;
        request.event.validate()?;
        // serde Option<T> accepts an explicit JSON null as None. The 7C
        // extension rejects extra keys even when null, so enforce the exact
        // signed event key set before granting a native receipt. A MAC alone
        // does not make a widened payload schema safe.
        let raw: serde_json::Value =
            serde_json::from_slice(payload).map_err(|_| BridgeContractError::Frame)?;
        let fields = raw
            .get("event")
            .and_then(serde_json::Value::as_object)
            .ok_or(BridgeContractError::Frame)?;
        let required: &[&str] = match request.event.kind.as_str() {
            "attach" | "retire" => &["protocol", "kind", "sequence"],
            "claim" | "repair" => &["protocol", "kind", "sequence", "intentId", "digest"],
            "ack" => &[
                "protocol",
                "kind",
                "sequence",
                "intentId",
                "digest",
                "receiptId",
            ],
            _ => return Err(BridgeContractError::Frame.into()),
        };
        if fields.len() != required.len() || required.iter().any(|key| !fields.contains_key(*key)) {
            return Err(BridgeContractError::Frame.into());
        }
        if request.frame.identity != trusted.identity {
            return Err(NativeBridgeError::Host);
        }
        self.fence.require_current_frame(&request.frame)?;
        let receipt = self
            .current_channel(trusted, channel_id, generation)?
            .bootstrap
            .mac(COMPANION_RECEIPT, bridge_sequence, payload);
        self.channels
            .get_mut(&channel_id)
            .ok_or(NativeBridgeError::Channel)?
            .last_sequence = bridge_sequence;
        Ok(receipt)
    }
}

fn constant_time_equal(expected: &[u8; KEY_BYTES], actual: &[u8; KEY_BYTES]) -> bool {
    let mut difference = 0;
    for (a, b) in expected.iter().zip(actual.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

fn authenticated_mac(
    key: &[u8; KEY_BYTES],
    domain: &[u8],
    channel_id: u64,
    generation: u64,
    sequence: u64,
    challenge: &[u8; KEY_BYTES],
    payload: &[u8],
) -> [u8; KEY_BYTES] {
    // HMAC-SHA256 with length-delimited domain/bytes, rather than an
    // ambiguous bare SHA256(secret || user-controlled payload).
    let mut inner_pad = [0x36; HMAC_BLOCK];
    let mut outer_pad = [0x5c; HMAC_BLOCK];
    for (index, byte) in key.iter().enumerate() {
        inner_pad[index] ^= byte;
        outer_pad[index] ^= byte;
    }
    let mut hash = Sha256::new();
    hash.update(inner_pad);
    hash.update((domain.len() as u64).to_be_bytes());
    hash.update(domain);
    hash.update(channel_id.to_be_bytes());
    hash.update(generation.to_be_bytes());
    hash.update(sequence.to_be_bytes());
    hash.update(challenge);
    hash.update((payload.len() as u64).to_be_bytes());
    hash.update(payload);
    let inner = hash.finalize();
    let mut hash = Sha256::new();
    hash.update(outer_pad);
    hash.update(inner);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn evidence() -> TrustedHostEvidence {
        TrustedHostEvidence {
            identity: DocumentIdentity {
                profile_id: "profile-A".into(),
                tab_id: 1,
                frame_id: 0,
                document_id: "doc-A".into(),
                navigation_epoch: 1,
                session_id: "session-A".into(),
                session_epoch: 1,
                worker_generation: 1,
            },
            origin: "https://synthetic.example".into(),
            host_generation: 1,
        }
    }

    fn bytes(e: &TrustedHostEvidence) -> Vec<u8> {
        serde_json::to_vec(&json!({"protocol":14, "identity":e.identity})).unwrap()
    }

    fn authenticated() -> (NativeBrowserBridge, TrustedHostEvidence, BridgeBootstrap) {
        let mut bridge = NativeBrowserBridge::new();
        let e = evidence();
        let bootstrap = bridge.register_host(e.clone()).unwrap();
        bridge
            .complete_handshake(
                &e,
                bootstrap.channel_id,
                bootstrap.host_generation,
                &bootstrap.handshake_proof(),
            )
            .unwrap();
        (bridge, e, bootstrap)
    }

    #[test]
    fn companion_event_must_bind_exact_kind_digest_receipt_and_fresh_sequence() {
        let (mut bridge, host, bootstrap) = authenticated();
        let identity = bytes(&host);
        bridge
            .observe(
                &host,
                bootstrap.channel_id,
                1,
                1,
                &identity,
                &bootstrap.observation_mac(1, &identity),
            )
            .unwrap();
        let attach = serde_json::to_vec(&json!({
            "frame": {"protocol": 14, "identity": host.identity},
            "event": {"protocol": 14, "kind": "attach", "sequence": 1}
        }))
        .unwrap();
        let mac = bootstrap.companion_event_mac(2, &attach);
        let mut substituted = attach.clone();
        let target = substituted.windows(6).position(|v| v == b"attach").unwrap();
        substituted[target..target + 6].copy_from_slice(b"retire");
        assert_eq!(
            bridge.authorize_companion_event(&host, bootstrap.channel_id, 1, 2, &substituted, &mac),
            Err(NativeBridgeError::Authentication)
        );
        let accepted = bridge
            .authorize_companion_event(&host, bootstrap.channel_id, 1, 2, &attach, &mac)
            .unwrap();
        assert_eq!(accepted, bootstrap.companion_receipt_mac(2, &attach));
        assert_eq!(
            bridge.authorize_companion_event(&host, bootstrap.channel_id, 1, 2, &attach, &mac),
            Err(NativeBridgeError::Sequence)
        );
        let unknown = serde_json::to_vec(&json!({
            "frame":{"protocol":14,"identity":host.identity},
            "event":{"protocol":14,"kind":"claim","sequence":2,
                     "intentId":"intent-1","digest":"A".repeat(64)}
        }))
        .unwrap();
        assert!(matches!(
            bridge.authorize_companion_event(
                &host,
                bootstrap.channel_id,
                1,
                3,
                &unknown,
                &bootstrap.companion_event_mac(3, &unknown)
            ),
            Err(NativeBridgeError::Contract(BridgeContractError::Frame))
        ));
    }

    #[test]
    fn legitimate_handshake_observe_and_current_authorization() {
        let (mut bridge, host, b) = authenticated();
        let frame = bytes(&host);
        assert_eq!(
            bridge.observe(
                &host,
                b.channel_id,
                1,
                1,
                &frame,
                &b.observation_mac(1, &frame)
            ),
            Ok(Observation::FirstProof)
        );
        assert_eq!(
            bridge.authorize_current(
                &host,
                b.channel_id,
                1,
                2,
                &frame,
                &b.authorization_mac(2, &frame)
            ),
            Ok(())
        );
        assert_eq!(bridge.active_channels(), 1);
    }

    #[test]
    fn unauthenticated_channel_and_wrong_proof_cannot_mutate_authority() {
        let mut bridge = NativeBrowserBridge::new();
        let e = evidence();
        let b = bridge.register_host(e.clone()).unwrap();
        let frame = bytes(&e);
        let mac = b.observation_mac(1, &frame);
        assert_eq!(
            bridge.observe(&e, b.channel_id, 1, 1, &frame, &mac),
            Err(NativeBridgeError::Handshake)
        );
        assert_eq!(
            bridge.complete_handshake(&e, b.channel_id, 1, &[0; 32]),
            Err(NativeBridgeError::Authentication)
        );
        bridge
            .complete_handshake(&e, b.channel_id, 1, &b.handshake_proof())
            .unwrap();
        assert_eq!(
            bridge.complete_handshake(&e, b.channel_id, 1, &b.handshake_proof()),
            Err(NativeBridgeError::Handshake)
        );
        assert_eq!(
            bridge.observe(&e, b.channel_id, 1, 1, &frame, &mac),
            Ok(Observation::FirstProof)
        );
    }

    #[test]
    fn origin_profile_tab_frame_and_generation_mismatch_rejected() {
        let (mut bridge, e, b) = authenticated();
        let frame = bytes(&e);
        for hostile in [
            TrustedHostEvidence {
                origin: "https://evil.example".into(),
                ..e.clone()
            },
            TrustedHostEvidence {
                identity: DocumentIdentity {
                    tab_id: 2,
                    ..e.identity.clone()
                },
                ..e.clone()
            },
            TrustedHostEvidence {
                identity: DocumentIdentity {
                    frame_id: 1,
                    ..e.identity.clone()
                },
                ..e.clone()
            },
            TrustedHostEvidence {
                identity: DocumentIdentity {
                    profile_id: "different".into(),
                    ..e.identity.clone()
                },
                ..e.clone()
            },
        ] {
            assert_eq!(
                bridge.observe(
                    &hostile,
                    b.channel_id,
                    1,
                    1,
                    &frame,
                    &b.observation_mac(1, &frame)
                ),
                Err(NativeBridgeError::Channel)
            );
        }
        assert_eq!(
            bridge.observe(
                &e,
                b.channel_id,
                2,
                1,
                &frame,
                &b.observation_mac(1, &frame)
            ),
            Err(NativeBridgeError::Channel)
        );
        assert_eq!(
            bridge.observe(
                &e,
                b.channel_id,
                1,
                1,
                &frame,
                &b.observation_mac(1, &frame)
            ),
            Ok(Observation::FirstProof)
        );
    }

    #[test]
    fn strict_sequence_mac_and_domain_prevent_replay() {
        let (mut bridge, e, b) = authenticated();
        let frame = bytes(&e);
        let sign = |sequence| b.observation_mac(sequence, &frame);
        assert_eq!(
            bridge.observe(&e, b.channel_id, 1, 2, &frame, &sign(2)),
            Err(NativeBridgeError::Sequence)
        );
        assert_eq!(
            bridge.observe(
                &e,
                b.channel_id,
                1,
                1,
                &frame,
                &b.authorization_mac(1, &frame)
            ),
            Err(NativeBridgeError::Authentication)
        );
        assert_eq!(
            bridge.observe(&e, b.channel_id, 1, 1, &frame, &[0; 32]),
            Err(NativeBridgeError::Authentication)
        );
        assert_eq!(
            bridge.observe(&e, b.channel_id, 1, 1, &frame, &sign(1)),
            Ok(Observation::FirstProof)
        );
        assert_eq!(
            bridge.observe(&e, b.channel_id, 1, 1, &frame, &sign(1)),
            Err(NativeBridgeError::Sequence)
        );
        assert_eq!(
            bridge.observe(&e, b.channel_id, 1, 2, &frame, &sign(2)),
            Ok(Observation::Idempotent)
        );
    }

    #[test]
    fn trusted_host_frame_mismatch_and_wrong_protocol_fail_closed() {
        let (mut bridge, e, b) = authenticated();
        let mut spoof = e.clone();
        spoof.identity.document_id = "untrusted".into();
        let frame = bytes(&spoof);
        assert_eq!(
            bridge.observe(
                &e,
                b.channel_id,
                1,
                1,
                &frame,
                &b.observation_mac(1, &frame)
            ),
            Err(NativeBridgeError::Host)
        );
        let wrong_version =
            serde_json::to_vec(&json!({"protocol":13,"identity":e.identity})).unwrap();
        assert_eq!(
            bridge.observe(
                &e,
                b.channel_id,
                1,
                1,
                &wrong_version,
                &b.observation_mac(1, &wrong_version)
            ),
            Err(NativeBridgeError::Contract(BridgeContractError::Version))
        );
        let valid = bytes(&e);
        assert_eq!(
            bridge.observe(
                &e,
                b.channel_id,
                1,
                1,
                &valid,
                &b.observation_mac(1, &valid)
            ),
            Ok(Observation::FirstProof)
        );
    }

    #[test]
    fn host_reregistration_retains_first_proof_and_fences_old_channel() {
        let (mut bridge, e, b) = authenticated();
        let frame = bytes(&e);
        bridge
            .observe(
                &e,
                b.channel_id,
                1,
                1,
                &frame,
                &b.observation_mac(1, &frame),
            )
            .unwrap();
        let mut newer = e.clone();
        newer.host_generation = 2;
        newer.identity.navigation_epoch = 2;
        newer.identity.document_id = "doc-B".into();
        newer.identity.worker_generation = 2;
        assert!(matches!(
            bridge.register_host(e.clone()),
            Err(NativeBridgeError::Generation)
        ));
        let replacement = bridge.register_host(newer.clone()).unwrap();
        assert!(!bridge.retire_host(b.channel_id));
        assert_eq!(
            bridge.authorize_current(
                &e,
                b.channel_id,
                1,
                2,
                &frame,
                &b.authorization_mac(2, &frame)
            ),
            Err(NativeBridgeError::Channel)
        );
        let next = bytes(&newer);
        assert_eq!(
            bridge.observe(
                &newer,
                replacement.channel_id,
                2,
                1,
                &next,
                &replacement.observation_mac(1, &next)
            ),
            Err(NativeBridgeError::Handshake)
        );
        bridge
            .complete_handshake(
                &newer,
                replacement.channel_id,
                2,
                &replacement.handshake_proof(),
            )
            .unwrap();
        assert_eq!(
            bridge.observe(
                &newer,
                replacement.channel_id,
                2,
                1,
                &next,
                &replacement.observation_mac(1, &next)
            ),
            Ok(Observation::Advanced)
        );
        assert_eq!(
            bridge.authorize_current(
                &newer,
                replacement.channel_id,
                2,
                2,
                &frame,
                &replacement.authorization_mac(2, &frame)
            ),
            Err(NativeBridgeError::Host)
        );
        assert_eq!(
            bridge.authorize_current(
                &newer,
                replacement.channel_id,
                2,
                2,
                &next,
                &replacement.authorization_mac(2, &next)
            ),
            Ok(())
        );
        assert!(bridge.retire_host(replacement.channel_id));
        assert!(!bridge.retire_host(replacement.channel_id));
    }

    #[test]
    fn cross_scope_session_theft_is_rejected_even_with_new_valid_mac() {
        let (mut bridge, e, b) = authenticated();
        let frame = bytes(&e);
        bridge
            .observe(
                &e,
                b.channel_id,
                1,
                1,
                &frame,
                &b.observation_mac(1, &frame),
            )
            .unwrap();
        let mut thief = evidence();
        thief.identity.tab_id = 40;
        let t = bridge.register_host(thief.clone()).unwrap();
        bridge
            .complete_handshake(&thief, t.channel_id, 1, &t.handshake_proof())
            .unwrap();
        let bytes = bytes(&thief);
        assert_eq!(
            bridge.observe(
                &thief,
                t.channel_id,
                1,
                1,
                &bytes,
                &t.observation_mac(1, &bytes)
            ),
            Err(NativeBridgeError::Contract(
                BridgeContractError::ConflictingAttribution
            ))
        );
        assert_eq!(
            bridge.authorize_current(
                &e,
                b.channel_id,
                1,
                2,
                &frame,
                &b.authorization_mac(2, &frame)
            ),
            Ok(())
        );
    }

    #[test]
    fn unsafe_origins_and_exhausted_epoch_are_not_admitted() {
        let mut bridge = NativeBrowserBridge::new();
        for origin in [
            "http://localhost",
            "file:///a",
            "https://evil/a",
            "https://user@host",
            "https://host?query",
            "https://host#fragment",
            "https://",
            "",
        ] {
            let mut host = evidence();
            host.origin = origin.into();
            assert!(matches!(
                bridge.register_host(host),
                Err(NativeBridgeError::Host)
            ));
        }
        let mut host = evidence();
        host.host_generation = MAX_SAFE_JS_INTEGER + 1;
        assert!(matches!(
            bridge.register_host(host),
            Err(NativeBridgeError::Generation)
        ));
        assert_eq!(bridge.active_channels(), 0);
    }
}
