//! Authenticated, bounded transport for the 15HOST native renderer.
//!
//! This transport authenticates the Rust-owned child process independently of
//! the typed UI epoch/sequence contracts. It grants no file, tool, provider or
//! account authority. Frames are JSON-line records; payloadJson is signed as
//! exact UTF-8 bytes so cross-language serializers cannot change the MAC input.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::ui_contract::MAX_JS_SAFE_INTEGER;

pub const RENDERER_IPC_PROTOCOL: u32 = 1;
pub const MAX_RENDERER_IPC_FRAME_BYTES: usize = 512 * 1024;
pub const MAX_RENDERER_IPC_PAYLOAD_BYTES: usize = 256 * 1024;
const MAC_BYTES: usize = 32;
const DOMAIN: &[u8] = b"CoSoS/15HOST/native-ui/v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RendererIpcError {
    #[error("renderer IPC frame is malformed")]
    Malformed,
    #[error("renderer IPC frame exceeds the byte limit")]
    Oversize,
    #[error("renderer IPC protocol is unsupported")]
    Protocol,
    #[error("renderer IPC host epoch is stale")]
    Epoch,
    #[error("renderer IPC sequence is stale or replayed")]
    Sequence,
    #[error("renderer IPC authentication failed")]
    Authentication,
    #[error("renderer IPC counter exhausted")]
    Exhausted,
}

impl RendererIpcError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Malformed => "invalid-renderer-frame",
            Self::Oversize => "renderer-frame-too-large",
            Self::Protocol => "unsupported-renderer-protocol",
            Self::Epoch => "stale-renderer-epoch",
            Self::Sequence => "invalid-renderer-sequence",
            Self::Authentication => "renderer-authentication-failed",
            Self::Exhausted => "renderer-counter-exhausted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRendererPayload {
    pub sequence: u64,
    pub payload: Value,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireFrame {
    protocol: u32,
    epoch: u64,
    sequence: u64,
    payload_json: String,
    mac: String,
}

/// Per-child authenticator. The key must be generated afresh by the Rust
/// launcher and sent only through that child's redirected stdin bootstrap.
/// Do not log or serialize this type: it contains the authentication key.
pub struct RendererIpcAuthenticator {
    epoch: u64,
    key: [u8; MAC_BYTES],
    inbound_sequence: u64,
    outbound_sequence: u64,
}

impl RendererIpcAuthenticator {
    pub fn new(epoch: u64, mut key: [u8; MAC_BYTES]) -> Result<Self, RendererIpcError> {
        if epoch == 0 || epoch > MAX_JS_SAFE_INTEGER {
            key.zeroize();
            return Err(RendererIpcError::Epoch);
        }
        let stored_key = key;
        key.zeroize();
        Ok(Self {
            epoch,
            key: stored_key,
            inbound_sequence: 0,
            outbound_sequence: 0,
        })
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Retire every in-flight frame on a renderer restart or host rebind.
    /// The caller must provide a new random per-process key.
    pub fn rebind(&mut self, epoch: u64, mut key: [u8; MAC_BYTES]) -> Result<(), RendererIpcError> {
        if epoch <= self.epoch || epoch > MAX_JS_SAFE_INTEGER {
            return Err(RendererIpcError::Epoch);
        }
        self.epoch = epoch;
        self.key = key;
        self.inbound_sequence = 0;
        self.outbound_sequence = 0;
        Ok(())
    }

    /// Sign a compact JSON payload for the renderer. Invalid JSON and payloads
    /// over the strict byte budget are rejected before consuming a sequence.
    pub fn sign_outbound(&mut self, payload_json: &str) -> Result<String, RendererIpcError> {
        let payload = payload_json.as_bytes();
        if payload.len() > MAX_RENDERER_IPC_PAYLOAD_BYTES {
            return Err(RendererIpcError::Oversize);
        }
        serde_json::from_str::<Value>(payload_json).map_err(|_| RendererIpcError::Malformed)?;
        let sequence = next_sequence(self.outbound_sequence)?;
        let mac = hmac_sha256(
            &self.key,
            &mac_input(RENDERER_IPC_PROTOCOL, self.epoch, sequence, payload),
        );
        let frame = WireFrame {
            protocol: RENDERER_IPC_PROTOCOL,
            epoch: self.epoch,
            sequence,
            payload_json: payload_json.to_owned(),
            mac: encode_hex(&mac),
        };
        let serialized = serde_json::to_string(&frame).map_err(|_| RendererIpcError::Malformed)?;
        if serialized.len() > MAX_RENDERER_IPC_FRAME_BYTES {
            return Err(RendererIpcError::Oversize);
        }
        self.outbound_sequence = sequence;
        Ok(serialized)
    }

    /// Authenticate an incoming frame before any UI contract dispatch. A valid
    /// MAC consumes its sequence even if the signed inner payload is malformed.
    pub fn verify_inbound(
        &mut self,
        raw: &[u8],
    ) -> Result<VerifiedRendererPayload, RendererIpcError> {
        if raw.len() > MAX_RENDERER_IPC_FRAME_BYTES {
            return Err(RendererIpcError::Oversize);
        }
        let frame: WireFrame =
            serde_json::from_slice(raw).map_err(|_| RendererIpcError::Malformed)?;
        if frame.protocol != RENDERER_IPC_PROTOCOL {
            return Err(RendererIpcError::Protocol);
        }
        if frame.epoch != self.epoch {
            return Err(RendererIpcError::Epoch);
        }
        if frame.sequence == 0 || frame.sequence > MAX_JS_SAFE_INTEGER {
            return Err(RendererIpcError::Sequence);
        }
        let expected_sequence = next_sequence(self.inbound_sequence)?;
        if frame.sequence != expected_sequence {
            return Err(RendererIpcError::Sequence);
        }
        let payload = frame.payload_json.as_bytes();
        if payload.len() > MAX_RENDERER_IPC_PAYLOAD_BYTES {
            return Err(RendererIpcError::Oversize);
        }
        let supplied = decode_hex(&frame.mac).ok_or(RendererIpcError::Malformed)?;
        let expected = hmac_sha256(
            &self.key,
            &mac_input(frame.protocol, frame.epoch, frame.sequence, payload),
        );
        if !constant_time_equal(&supplied, &expected) {
            return Err(RendererIpcError::Authentication);
        }

        // Consume a correctly authenticated sequence before parsing the inner
        // payload. Otherwise a malformed signed frame could be replayed forever.
        self.inbound_sequence = frame.sequence;
        let payload =
            serde_json::from_str(&frame.payload_json).map_err(|_| RendererIpcError::Malformed)?;
        Ok(VerifiedRendererPayload {
            sequence: frame.sequence,
            payload,
        })
    }
}

fn next_sequence(current: u64) -> Result<u64, RendererIpcError> {
    current
        .checked_add(1)
        .filter(|next| *next <= MAX_JS_SAFE_INTEGER)
        .ok_or(RendererIpcError::Exhausted)
}

fn mac_input(protocol: u32, epoch: u64, sequence: u64, payload: &[u8]) -> Vec<u8> {
    let mut input = Vec::with_capacity(DOMAIN.len() + 20 + payload.len());
    input.extend_from_slice(DOMAIN);
    input.extend_from_slice(&protocol.to_be_bytes());
    input.extend_from_slice(&epoch.to_be_bytes());
    input.extend_from_slice(&sequence.to_be_bytes());
    input.extend_from_slice(payload);
    input
}

fn hmac_sha256(key: &[u8; MAC_BYTES], message: &[u8]) -> [u8; MAC_BYTES] {
    const BLOCK: usize = 64;
    let mut inner_key = [0x36u8; BLOCK];
    let mut outer_key = [0x5cu8; BLOCK];
    for (index, byte) in key.iter().enumerate() {
        inner_key[index] ^= byte;
        outer_key[index] ^= byte;
    }
    let mut inner = Sha256::new();
    inner.update(inner_key);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_key);
    outer.update(inner_digest);
    let digest = outer.finalize();
    let mut result = [0u8; MAC_BYTES];
    result.copy_from_slice(&digest);
    result
}

fn constant_time_equal(left: &[u8; MAC_BYTES], right: &[u8; MAC_BYTES]) -> bool {
    let mut difference = 0u8;
    for index in 0..MAC_BYTES {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}

fn encode_hex(bytes: &[u8; MAC_BYTES]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(MAC_BYTES * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(value: &str) -> Option<[u8; MAC_BYTES]> {
    if value.len() != MAC_BYTES * 2 {
        return None;
    }
    let bytes = value.as_bytes();
    let mut decoded = [0u8; MAC_BYTES];
    for index in 0..MAC_BYTES {
        decoded[index] = (hex_nibble(bytes[index * 2])? << 4) | hex_nibble(bytes[index * 2 + 1])?;
    }
    Some(decoded)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

impl Drop for RendererIpcAuthenticator {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
