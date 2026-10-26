//! 7D-FIX: private WebView2 -> Rust 7B/7D native browser tool bridge.
//! Input is exclusively the source-attested Windows host's child-process pipe.
//! BrowserDebugger callbacks are answered by the privileged WebView2 controller;
//! a renderer, content script or web message has NO ingress to this executable.
use cos_core::browser_protocol::DocumentIdentity;
use cos_core::browser_tools::{
    BrowserDebugger, BrowserSnapshot, DirectBrowserTools, ToolAuthorization,
};
use cos_core::native_browser_bridge::{NativeBrowserBridge, TrustedHostEvidence};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write};
use std::rc::Rc;

const MAX_COMMAND: usize = 32 * 1024;
const MAX_CDP_REPLY: usize = 300 * 1024;

#[derive(Debug, Clone, Deserialize)]
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
    // 7C persisted-ledger events use a separate signed MAC domain from tools.
    Authorize {
        host: HostWire,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: String,
        mac: String,
    },
    ToolAttach {
        host: HostWire,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: String,
        mac: String,
    },
    ToolSnapshot {
        host: HostWire,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: String,
        mac: String,
        handle: u64,
    },
    ToolDetach {
        host: HostWire,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: String,
        mac: String,
        handle: u64,
    },
    // The privileged host owns this child-process pipe; no renderer ingress.
    Handoff {
        source: HostWire,
        target: HostWire,
        // Explicit 8P version on the trusted C#/Rust pipe. Legacy synthetic
        // 8L1 callers intentionally omit this for their separate contract.
        #[serde(default)]
        contract_version: Option<String>,
        new_source_session_id: String,
        source_next_session_epoch: u64,
        target_next_session_epoch: u64,
    },
    Retire {
        channel_id: u64,
    },
}
const NATIVE_8P_TWO_PHASE_V1: &str = "CoSoS/8P/native-two-phase/v1";

// Versioning is enforced at the actual privileged Rust IPC entrypoint. It
// cannot be bypassed by merely tagging a C# selftest; "8p-" is reserved for
// the production-owned profiles created by the current Windows host.
fn require_handoff_wire_version(
    source: &HostWire,
    target: &HostWire,
    version: Option<&str>,
) -> Result<(), String> {
    let production_8p = source.identity.profile_id.starts_with("8p-")
        || target.identity.profile_id.starts_with("8p-");
    if production_8p {
        if !source.identity.profile_id.starts_with("8p-")
            || !target.identity.profile_id.starts_with("8p-")
            || version != Some(NATIVE_8P_TWO_PHASE_V1)
        {
            return Err("8P native handoff protocol downgrade/foreign profile".into());
        }
    } else if version.is_some() {
        return Err("unexpected 8P contract on legacy native handoff".into());
    }
    Ok(())
}

fn unhex(input: &str) -> Result<Vec<u8>, String> {
    if !input.len().is_multiple_of(2) || input.len() > 16384 {
        return Err("invalid hex length".into());
    }
    input
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let s = std::str::from_utf8(pair).map_err(|_| "hex encoding".to_string())?;
            u8::from_str_radix(s, 16).map_err(|_| "invalid hex".to_string())
        })
        .collect()
}
fn digest(input: &str) -> Result<[u8; 32], String> {
    unhex(input)?
        .try_into()
        .map_err(|_| "MAC must be 32 bytes".into())
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
struct NativePipe {
    input: io::Stdin,
    output: io::BufWriter<io::Stdout>,
}
impl NativePipe {
    fn read(&mut self, max: usize) -> Result<String, String> {
        // Reader is a private host pipe, not an HTTP or renderer endpoint.
        // The bounded line reader avoids allocating unbounded malformed frames.
        let mut bytes = Vec::new();
        let mut one = [0u8; 1];
        use std::io::Read;
        while bytes.len() <= max {
            match self.input.read(&mut one) {
                Ok(0) => return Err("host pipe closed".into()),
                Ok(_) if one[0] == b'\n' => {
                    return String::from_utf8(bytes).map_err(|_| "non-UTF8 native frame".into());
                }
                Ok(_) => bytes.push(one[0]),
                Err(_) => return Err("host pipe read error".into()),
            }
        }
        Err("oversized native frame".into())
    }
    fn write(&mut self, value: &Value) -> Result<(), String> {
        writeln!(self.output, "{value}").map_err(|_| "host pipe write error".to_string())?;
        self.output
            .flush()
            .map_err(|_| "host pipe flush error".to_string())
    }
}

struct WebView2Debugger {
    pipe: Rc<RefCell<NativePipe>>,
}
impl WebView2Debugger {
    fn cdp(&mut self, operation: &str, trusted: &TrustedHostEvidence) -> Result<Value, String> {
        // Only these three operations can cross to the C# host. The native
        // callback independently matches exact browser-owned context and
        // checks both sides of each awaited CDP call.
        let mut pipe = self.pipe.borrow_mut();
        pipe.write(&json!({"cdpCallback":{
            "op":operation,
            "host":{
                "identity": &trusted.identity, "origin": &trusted.origin,
                "hostGeneration":trusted.host_generation
            }
        }}))?;
        let reply: Value = serde_json::from_str(&pipe.read(MAX_CDP_REPLY)?)
            .map_err(|_| "invalid native CDP callback reply".to_string())?;
        if reply.get("cdpResponse").and_then(Value::as_bool) != Some(true) {
            return Err("unexpected native callback response".into());
        }
        if reply.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err("privileged WebView2 debugger denied or failed".into());
        }
        Ok(reply
            .get("value")
            .cloned()
            .ok_or("missing native CDP value")?)
    }
}
impl BrowserDebugger for WebView2Debugger {
    fn attach(&mut self, host: &TrustedHostEvidence) -> Result<(), String> {
        self.cdp("attach", host).map(|_| ())
    }
    fn detach(&mut self, host: &TrustedHostEvidence) -> Result<(), String> {
        self.cdp("detach", host).map(|_| ())
    }
    fn snapshot(&mut self, host: &TrustedHostEvidence) -> Result<BrowserSnapshot, String> {
        let value = self.cdp("snapshot", host)?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct SnapshotWire {
            identity: DocumentIdentity,
            url: String,
            title: String,
            text: String,
        }
        let data: SnapshotWire = serde_json::from_value(value)
            .map_err(|_| "invalid native debugger snapshot schema".to_string())?;
        Ok(BrowserSnapshot {
            identity: data.identity,
            url: data.url,
            title: data.title,
            text: data.text,
        })
    }
}
struct State {
    bridge: NativeBrowserBridge,
    tools: DirectBrowserTools<WebView2Debugger>,
    owned: HashMap<u64, TrustedHostEvidence>,
}
impl State {
    fn auth<'a>(
        host: &'a TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
        sequence: u64,
        payload: &'a [u8],
        mac: [u8; 32],
    ) -> ToolAuthorization<'a> {
        ToolAuthorization {
            trusted: host,
            channel_id,
            generation,
            sequence,
            signed_frame: payload,
            mac,
        }
    }
    fn apply(&mut self, req: Request) -> Result<Value, String> {
        match req {
            Request::Register { host } => {
                let host: TrustedHostEvidence = host.into();
                let boot = self
                    .bridge
                    .register_host(host.clone())
                    .map_err(|e| e.to_string())?;
                self.owned.insert(boot.channel_id, host);
                Ok(json!({"channelId":boot.channel_id,
                    "hostGeneration":boot.host_generation,
                    "challenge":hex(&boot.challenge),
                    "key":hex(&boot.isolated_worker_key())}))
            }
            Request::Handshake {
                host,
                channel_id,
                generation,
                proof,
            } => {
                self.bridge
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
                let result = self
                    .bridge
                    .observe(
                        &host.into(),
                        channel_id,
                        generation,
                        sequence,
                        &unhex(&payload)?,
                        &digest(&mac)?,
                    )
                    .map_err(|e| e.to_string())?;
                Ok(json!({"observation":format!("{result:?}")}))
            }
            Request::Authorize {
                host,
                channel_id,
                generation,
                sequence,
                payload,
                mac,
            } => {
                // Never treat a ledger authorization as a generic tool command.
                // The bridge verifies host identity, current frame, 7B sequence,
                // companion-event MAC domain, exact event shape and receipt.
                let receipt = self
                    .bridge
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
            Request::ToolAttach {
                host,
                channel_id,
                generation,
                sequence,
                payload,
                mac,
            } => {
                let host: TrustedHostEvidence = host.into();
                let frame = unhex(&payload)?;
                let handle = self
                    .tools
                    .attach(
                        &mut self.bridge,
                        Self::auth(
                            &host,
                            channel_id,
                            generation,
                            sequence,
                            &frame,
                            digest(&mac)?,
                        ),
                    )
                    .map_err(|e| e.to_string())?;
                Ok(json!({"handle":handle}))
            }
            Request::ToolSnapshot {
                host,
                channel_id,
                generation,
                sequence,
                payload,
                mac,
                handle,
            } => {
                let host: TrustedHostEvidence = host.into();
                let frame = unhex(&payload)?;
                let snapshot = self
                    .tools
                    .snapshot(
                        &mut self.bridge,
                        Self::auth(
                            &host,
                            channel_id,
                            generation,
                            sequence,
                            &frame,
                            digest(&mac)?,
                        ),
                        handle,
                    )
                    .map_err(|e| e.to_string())?;
                Ok(json!({"identity":snapshot.identity,
                    "url":snapshot.url,"title":snapshot.title,"text":snapshot.text}))
            }
            Request::ToolDetach {
                host,
                channel_id,
                generation,
                sequence,
                payload,
                mac,
                handle,
            } => {
                let host: TrustedHostEvidence = host.into();
                let frame = unhex(&payload)?;
                self.tools
                    .detach(
                        &mut self.bridge,
                        Self::auth(
                            &host,
                            channel_id,
                            generation,
                            sequence,
                            &frame,
                            digest(&mac)?,
                        ),
                        handle,
                    )
                    .map_err(|e| e.to_string())?;
                Ok(json!({"detached":true}))
            }
            Request::Handoff {
                source,
                target,
                contract_version,
                new_source_session_id,
                source_next_session_epoch,
                target_next_session_epoch,
            } => {
                // Production 8P may never downgrade to the legacy 8L1 wire
                // semantics by omitting or forging the contract version.
                require_handoff_wire_version(
                    &source, &target, contract_version.as_deref()
                )?;
                self.bridge
                    .handoff_sessions(
                        &source.into(),
                        &target.into(),
                        &new_source_session_id,
                        source_next_session_epoch,
                        target_next_session_epoch,
                    )
                    .map_err(|e| e.to_string())?;
                Ok(json!({"committed":true}))
            }
            Request::Retire { channel_id } => {
                let was_owned = self.owned.get(&channel_id);
                // If physical detach fails, retain the Rust lease: never grant
                // a new contender an ambiguously live debugger.
                if let Some(host) = was_owned
                    && self
                        .tools
                        .has_lease_for(host, channel_id, host.host_generation)
                    && !self
                        .tools
                        .retire_from_native(host, channel_id, host.host_generation)
                {
                    return Err("native debugger detach unresolved".into());
                }
                self.owned.remove(&channel_id);
                Ok(json!({"retired":self.bridge.retire_host(channel_id)}))
            }
        }
    }
}
fn main() {
    let pipe = Rc::new(RefCell::new(NativePipe {
        input: io::stdin(),
        output: io::BufWriter::new(io::stdout()),
    }));
    let mut state = State {
        bridge: NativeBrowserBridge::new(),
        tools: DirectBrowserTools::new(WebView2Debugger { pipe: pipe.clone() }),
        owned: HashMap::new(),
    };
    loop {
        let line = match pipe.borrow_mut().read(MAX_COMMAND) {
            Ok(line) => line,
            Err(_) => break,
        };
        let response = serde_json::from_str::<Request>(&line)
            .map_err(|_| "invalid request schema".to_string())
            .and_then(|req| state.apply(req));
        let value = match response {
            Ok(v) => json!({"ok":true,"value":v}),
            Err(error) => json!({"ok":false,"error":error}),
        };
        if pipe.borrow_mut().write(&value).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_wire_requires_exact_tool_operation_fields() {
        let good = json!({"op":"tool_attach",
            "host":{"identity":{"profileId":"a","tabId":1,"frameId":0,
                "documentId":"d","navigationEpoch":1,"sessionId":"s",
                "sessionEpoch":1,"workerGeneration":1},
                "origin":"https://synthetic.cos.test","hostGeneration":1},
            "channelId":1,"generation":1,"sequence":2,"payload":"00",
            "mac":"00".repeat(32)});
        assert!(serde_json::from_value::<Request>(good.clone()).is_ok());
        let mut bad = good;
        bad["command"] = json!("Runtime.evaluate");
        assert!(serde_json::from_value::<Request>(bad).is_err());
        assert!(serde_json::from_value::<Request>(json!({"op":"tool_send"})).is_err());
    }

    #[test]
    fn production_8p_ipc_handoff_must_have_exact_version_never_downgrade() {
        let host = |profile: &str| -> HostWire {
            serde_json::from_value(json!({
                "identity": {
                    "profileId": profile, "tabId": 1, "frameId": 0,
                    "documentId": "doc", "navigationEpoch": 1,
                    "sessionId": "session", "sessionEpoch": 1,
                    "workerGeneration": 1
                },
                "origin": "https://chatgpt.com", "hostGeneration": 1
            }))
            .unwrap()
        };
        let a = host("8p-A");
        let b = host("8p-B");
        let legacy = host("7l1-A");
        assert!(require_handoff_wire_version(
            &a, &b, Some(NATIVE_8P_TWO_PHASE_V1)).is_ok());
        assert!(require_handoff_wire_version(&a, &b, None).is_err());
        assert!(require_handoff_wire_version(&a, &b, Some("v0")).is_err());
        assert!(require_handoff_wire_version(&a, &legacy,
            Some(NATIVE_8P_TWO_PHASE_V1)).is_err());
        assert!(require_handoff_wire_version(&legacy, &legacy, None).is_ok());
        assert!(require_handoff_wire_version(&legacy, &legacy,
            Some(NATIVE_8P_TWO_PHASE_V1)).is_err());
        let request = json!({
            "op": "handoff",
            "source": a, "target": b,
            "contractVersion": NATIVE_8P_TWO_PHASE_V1,
            "newSourceSessionId": "empty",
            "sourceNextSessionEpoch": 2,
            "targetNextSessionEpoch": 2
        });
        assert!(matches!(
            serde_json::from_value::<Request>(request),
            Ok(Request::Handoff {
                contract_version: Some(version), ..
            }) if version == NATIVE_8P_TWO_PHASE_V1
        ));
    }

    #[test]
    fn private_wire_accepts_only_exact_signed_ledger_authorize() {
        let valid = json!({
            "op": "authorize",
            "host": {
                "identity": {
                    "profileId": "a", "tabId": 1, "frameId": 0,
                    "documentId": "d", "navigationEpoch": 1,
                    "sessionId": "s", "sessionEpoch": 1, "workerGeneration": 1
                },
                "origin": "https://synthetic.cos.test", "hostGeneration": 1
            },
            "channelId": 1, "generation": 1, "sequence": 2,
            "payload": "00", "mac": "00".repeat(32)
        });
        assert!(matches!(
            serde_json::from_value::<Request>(valid.clone()),
            Ok(Request::Authorize {
                channel_id: 1,
                generation: 1,
                sequence: 2,
                ..
            })
        ));
        let mut missing_mac = valid.clone();
        missing_mac.as_object_mut().unwrap().remove("mac");
        assert!(serde_json::from_value::<Request>(missing_mac).is_err());
        let mut extra = valid.clone();
        extra["command"] = json!("Runtime.evaluate");
        assert!(serde_json::from_value::<Request>(extra).is_err());
        let mut host_extra = valid.clone();
        host_extra["host"]["ownerOverride"] = json!(true);
        assert!(serde_json::from_value::<Request>(host_extra).is_err());
        let mut wrong_case = valid.clone();
        wrong_case.as_object_mut().unwrap().remove("channelId");
        wrong_case["channel_id"] = json!(1);
        assert!(serde_json::from_value::<Request>(wrong_case).is_err());
        let mut unknown_op = valid;
        unknown_op["op"] = json!("authorize_current");
        assert!(serde_json::from_value::<Request>(unknown_op).is_err());
    }

    #[test]
    fn signed_ledger_wire_yields_mac_receipt_and_denies_replay_and_forgery() {
        let host: HostWire = serde_json::from_value(json!({
            "identity": {
                "profileId": "a", "tabId": 1, "frameId": 0,
                "documentId": "d", "navigationEpoch": 1,
                "sessionId": "s", "sessionEpoch": 1, "workerGeneration": 1
            },
            "origin": "https://synthetic.cos.test", "hostGeneration": 1
        }))
        .unwrap();
        let trusted: TrustedHostEvidence = host.clone().into();
        let pipe = Rc::new(RefCell::new(NativePipe {
            input: io::stdin(),
            output: io::BufWriter::new(io::stdout()),
        }));
        let mut state = State {
            bridge: NativeBrowserBridge::new(),
            tools: DirectBrowserTools::new(WebView2Debugger { pipe }),
            owned: HashMap::new(),
        };
        let boot = state.bridge.register_host(trusted.clone()).unwrap();
        state
            .bridge
            .complete_handshake(
                &trusted,
                boot.channel_id,
                boot.host_generation,
                &boot.handshake_proof(),
            )
            .unwrap();
        let observation = serde_json::to_vec(&json!({
            "protocol": 14, "identity": &trusted.identity
        }))
        .unwrap();
        state
            .bridge
            .observe(
                &trusted,
                boot.channel_id,
                boot.host_generation,
                1,
                &observation,
                &boot.observation_mac(1, &observation),
            )
            .unwrap();
        let payload = serde_json::to_vec(&json!({
            "frame": {"protocol": 14, "identity": &trusted.identity},
            "event": {"protocol": 14, "kind": "attach", "sequence": 1}
        }))
        .unwrap();
        let good_mac = boot.companion_event_mac(2, &payload);
        let command = json!({
            "op": "authorize",
            "host": {
                "identity": &trusted.identity,
                "origin": &trusted.origin,
                "hostGeneration": trusted.host_generation
            },
            "channelId": boot.channel_id,
            "generation": boot.host_generation,
            "sequence": 2,
            "payload": hex(&payload),
            "mac": hex(&good_mac)
        });
        let mut invalid_mac = command.clone();
        invalid_mac["mac"] = json!(hex(&[0u8; 32]));
        assert!(
            state
                .apply(serde_json::from_value(invalid_mac).unwrap())
                .is_err()
        );
        let signed = state
            .apply(serde_json::from_value(command.clone()).unwrap())
            .unwrap();
        assert_eq!(
            signed["receipt"],
            json!(hex(&boot.companion_receipt_mac(2, &payload)))
        );
        // A successful receipt consumes the signed 7B sequence exactly once.
        assert!(
            state
                .apply(serde_json::from_value(command).unwrap())
                .is_err()
        );
        // Reusing the observed sequence for a ledger write is also forbidden.
        let prior_sequence = json!({
            "op": "authorize", "host": {
                "identity": &trusted.identity,
                "origin": &trusted.origin,
                "hostGeneration": trusted.host_generation
            },
            "channelId": boot.channel_id, "generation": boot.host_generation,
            "sequence": 1, "payload": hex(&payload),
            "mac": hex(&boot.companion_event_mac(1, &payload))
        });
        assert!(
            state
                .apply(serde_json::from_value(prior_sequence).unwrap())
                .is_err()
        );
    }
}
