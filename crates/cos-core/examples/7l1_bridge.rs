//! Isolated native 7L1 browser gate: private child-process stdin/stdout only.
//! The trusted WebView2 host owns source attestation; no page/HTTP ingress.
use cos_core::browser_protocol::DocumentIdentity;
use cos_core::native_browser_bridge::{NativeBrowserBridge, TrustedHostEvidence};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostWire {
    identity: DocumentIdentity,
    origin: String,
    host_generation: u64,
}

impl From<HostWire> for TrustedHostEvidence {
    fn from(h: HostWire) -> Self {
        Self {
            identity: h.identity,
            origin: h.origin,
            host_generation: h.host_generation,
        }
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Request {
    Register {
        host: HostWire,
    },
    Handshake {
        host: HostWire,
        channel_id: u64,
        generation: u64,
        proof: String,
    },
    Observe {
        host: HostWire,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: String,
        mac: String,
    },
    Authorize {
        host: HostWire,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: String,
        mac: String,
    },
    Retire {
        channel_id: u64,
    },
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    out
}

fn unhex(input: &str) -> Result<Vec<u8>, String> {
    if !input.len().is_multiple_of(2) || input.len() > 16384 {
        return Err("invalid hex length".into());
    }
    input
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|_| "hex encoding".to_string())?;
            u8::from_str_radix(text, 16).map_err(|_| "invalid hex digit".to_string())
        })
        .collect()
}

fn digest(input: &str) -> Result<[u8; 32], String> {
    unhex(input)?
        .try_into()
        .map_err(|_| "MAC must be 32 bytes".into())
}

fn apply(bridge: &mut NativeBrowserBridge, req: Request) -> Result<Value, String> {
    match req {
        Request::Register { host } => {
            let boot = bridge
                .register_host(host.into())
                .map_err(|e| e.to_string())?;
            // The key only traverses this dedicated native-to-native private
            // stdio pipe, then an extension-ID-checked CDP worker session.
            Ok(json!({
                "channelId":boot.channel_id,
                "hostGeneration":boot.host_generation,
                "challenge":hex(&boot.challenge),
                "key":hex(&boot.isolated_worker_key())
            }))
        }
        Request::Handshake {
            host,
            channel_id,
            generation,
            proof,
        } => {
            bridge
                .complete_handshake(&host.into(), channel_id, generation, &digest(&proof)?)
                .map_err(|e| e.to_string())?;
            Ok(json!({"authenticated":true}))
        }
        Request::Observe {
            host,
            channel_id,
            generation,
            sequence,
            payload,
            mac,
        } => {
            let observation = bridge
                .observe(
                    &host.into(),
                    channel_id,
                    generation,
                    sequence,
                    &unhex(&payload)?,
                    &digest(&mac)?,
                )
                .map_err(|e| e.to_string())?;
            Ok(json!({"observation":format!("{observation:?}")}))
        }
        Request::Authorize {
            host,
            channel_id,
            generation,
            sequence,
            payload,
            mac,
        } => {
            let receipt = bridge
                .authorize_companion_event(
                    &host.into(),
                    channel_id,
                    generation,
                    sequence,
                    &unhex(&payload)?,
                    &digest(&mac)?,
                )
                .map_err(|e| e.to_string())?;
            Ok(json!({"receipt":hex(&receipt)}))
        }
        Request::Retire { channel_id } => Ok(json!({"retired":bridge.retire_host(channel_id)})),
    }
}

fn main() {
    let mut bridge = NativeBrowserBridge::new();
    let input = io::stdin();
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in input.lock().lines() {
        let response = match line {
            Ok(line) if line.len() <= 32768 => serde_json::from_str::<Request>(&line)
                .map_err(|_| "invalid request schema".to_string())
                .and_then(|request| apply(&mut bridge, request)),
            _ => Err("input unavailable or too large".into()),
        };
        let reply = match response {
            Ok(value) => json!({"ok":true, "value":value}),
            Err(error) => json!({"ok":false, "error":error}),
        };
        if writeln!(output, "{reply}").is_err() || output.flush().is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_wire_uses_exact_camel_case_for_channel_and_generation() {
        let value = json!({
            "op":"handshake",
            "host":{
                "identity":{
                    "profileId":"7l1-A", "tabId":1, "frameId":0,
                    "documentId":"doc-A", "navigationEpoch":1,
                    "sessionId":"session-A", "sessionEpoch":1,
                    "workerGeneration":1
                },
                "origin":"https://synthetic.cos.test",
                "hostGeneration":1
            },
            "channelId":17,
            "generation":1,
            "proof":"0".repeat(64)
        });
        assert!(matches!(
            serde_json::from_value::<Request>(value),
            Ok(Request::Handshake {
                channel_id: 17,
                generation: 1,
                ..
            })
        ));
        assert!(matches!(
            serde_json::from_value::<Request>(json!({"op":"retire","channelId":17})),
            Ok(Request::Retire { channel_id: 17 })
        ));
        assert!(serde_json::from_value::<Request>(json!({"op":"retire","channel_id":17})).is_err());
    }
}
