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
        Self { identity: h.identity, origin: h.origin, host_generation: h.host_generation }
    }
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", rename_all_fields = "camelCase", deny_unknown_fields)]
enum Request {
    Register { host: HostWire },
    Handshake { host: HostWire, channel_id: u64, generation: u64, proof: String },
    Observe {
        host: HostWire, channel_id: u64, generation: u64, sequence: u64,
        payload: String, mac: String,
    },
    ToolAttach {
        host: HostWire, channel_id: u64, generation: u64, sequence: u64,
        payload: String, mac: String,
    },
    ToolSnapshot {
        host: HostWire, channel_id: u64, generation: u64, sequence: u64,
        payload: String, mac: String, handle: u64,
    },
    ToolDetach {
        host: HostWire, channel_id: u64, generation: u64, sequence: u64,
        payload: String, mac: String, handle: u64,
    },
    Retire { channel_id: u64 },
}
fn unhex(input: &str) -> Result<Vec<u8>, String> {
    if !input.len().is_multiple_of(2) || input.len() > 16384 {
        return Err("invalid hex length".into());
    }
    input.as_bytes().chunks_exact(2).map(|pair| {
        let s = std::str::from_utf8(pair).map_err(|_| "hex encoding".to_string())?;
        u8::from_str_radix(s, 16).map_err(|_| "invalid hex".to_string())
    }).collect()
}
fn digest(input: &str) -> Result<[u8; 32], String> {
    unhex(input)?.try_into().map_err(|_| "MAC must be 32 bytes".into())
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
        self.output.flush().map_err(|_| "host pipe flush error".to_string())
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
        Ok(reply.get("value").cloned().ok_or("missing native CDP value")?)
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
        #[serde(rename_all="camelCase",deny_unknown_fields)]
        struct SnapshotWire {
            identity: DocumentIdentity,
            url: String,
            title: String,
            text: String,
        }
        let data: SnapshotWire = serde_json::from_value(value)
            .map_err(|_| "invalid native debugger snapshot schema".to_string())?;
        Ok(BrowserSnapshot {
            identity:data.identity, url:data.url, title:data.title, text:data.text
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
        host: &'a TrustedHostEvidence, channel_id: u64, generation: u64,
        sequence: u64, payload: &'a [u8], mac: [u8; 32],
    ) -> ToolAuthorization<'a> {
        ToolAuthorization {
            trusted: host, channel_id, generation, sequence, signed_frame: payload, mac
        }
    }
    fn apply(&mut self, req: Request) -> Result<Value,String> {
        match req {
            Request::Register {host} => {
                let host: TrustedHostEvidence = host.into();
                let boot = self.bridge.register_host(host.clone()).map_err(|e|e.to_string())?;
                self.owned.insert(boot.channel_id, host);
                Ok(json!({"channelId":boot.channel_id,
                    "hostGeneration":boot.host_generation,
                    "challenge":hex(&boot.challenge),
                    "key":hex(&boot.isolated_worker_key())}))
            }
            Request::Handshake {host,channel_id,generation,proof} => {
                self.bridge.complete_handshake(&host.into(),channel_id,generation,&digest(&proof)?)
                    .map_err(|e|e.to_string())?;
                Ok(json!({"authenticated":true}))
            }
            Request::Observe {host,channel_id,generation,sequence,payload,mac} => {
                let result=self.bridge.observe(&host.into(),channel_id,generation,sequence,
                    &unhex(&payload)?,&digest(&mac)?).map_err(|e|e.to_string())?;
                Ok(json!({"observation":format!("{result:?}")}))
            }
            Request::ToolAttach {host,channel_id,generation,sequence,payload,mac} => {
                let host: TrustedHostEvidence=host.into();
                let frame=unhex(&payload)?;
                let handle=self.tools.attach(&mut self.bridge,
                    Self::auth(&host,channel_id,generation,sequence,&frame,digest(&mac)?))
                    .map_err(|e|e.to_string())?;
                Ok(json!({"handle":handle}))
            }
            Request::ToolSnapshot {host,channel_id,generation,sequence,payload,mac,handle} => {
                let host: TrustedHostEvidence=host.into();
                let frame=unhex(&payload)?;
                let snapshot=self.tools.snapshot(&mut self.bridge,
                    Self::auth(&host,channel_id,generation,sequence,&frame,digest(&mac)?),
                    handle).map_err(|e|e.to_string())?;
                Ok(json!({"identity":snapshot.identity,
                    "url":snapshot.url,"title":snapshot.title,"text":snapshot.text}))
            }
            Request::ToolDetach {host,channel_id,generation,sequence,payload,mac,handle} => {
                let host: TrustedHostEvidence=host.into();
                let frame=unhex(&payload)?;
                self.tools.detach(&mut self.bridge,
                    Self::auth(&host,channel_id,generation,sequence,&frame,digest(&mac)?),
                    handle).map_err(|e|e.to_string())?;
                Ok(json!({"detached":true}))
            }
            Request::Retire {channel_id} => {
                let was_owned = self.owned.get(&channel_id);
                // If physical detach fails, retain the Rust lease: never grant
                // a new contender an ambiguously live debugger.
                if let Some(host) = was_owned {
                    if self.tools.has_lease_for(host,channel_id,host.host_generation)
                        && !self.tools.retire_from_native(host,channel_id,host.host_generation)
                    {
                        return Err("native debugger detach unresolved".into());
                    }
                }
                self.owned.remove(&channel_id);
                Ok(json!({"retired":self.bridge.retire_host(channel_id)}))
            }
        }
    }
}
fn main() {
    let pipe=Rc::new(RefCell::new(NativePipe {
        input:io::stdin(),output:io::BufWriter::new(io::stdout())
    }));
    let mut state=State {
        bridge:NativeBrowserBridge::new(),
        tools:DirectBrowserTools::new(WebView2Debugger{pipe:pipe.clone()}),
        owned:HashMap::new(),
    };
    loop {
        let line=match pipe.borrow_mut().read(MAX_COMMAND) {
            Ok(line)=>line,
            Err(_)=>break,
        };
        let response=serde_json::from_str::<Request>(&line)
            .map_err(|_|"invalid request schema".to_string())
            .and_then(|req|state.apply(req));
        let value=match response {
            Ok(v)=>json!({"ok":true,"value":v}),
            Err(error)=>json!({"ok":false,"error":error}),
        };
        if pipe.borrow_mut().write(&value).is_err(){break;}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_wire_requires_exact_tool_operation_fields() {
        let good=json!({"op":"tool_attach",
            "host":{"identity":{"profileId":"a","tabId":1,"frameId":0,
                "documentId":"d","navigationEpoch":1,"sessionId":"s",
                "sessionEpoch":1,"workerGeneration":1},
                "origin":"https://synthetic.cos.test","hostGeneration":1},
            "channelId":1,"generation":1,"sequence":2,"payload":"00",
            "mac":"00".repeat(32)});
        assert!(serde_json::from_value::<Request>(good.clone()).is_ok());
        let mut bad=good;
        bad["command"]=json!("Runtime.evaluate");
        assert!(serde_json::from_value::<Request>(bad).is_err());
        assert!(serde_json::from_value::<Request>(json!({"op":"tool_send"})).is_err());
    }
}
