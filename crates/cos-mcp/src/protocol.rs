use crate::kernel::{Kernel, Surface};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
pub struct Reply {
    pub status: u16,
    pub body: Value,
    pub sse: bool,
}
const LEGACY: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
pub trait Server {
    fn info(&self, surface: Surface) -> Value;
    fn tools(&self, surface: Surface) -> Vec<Value>;
    fn call(&self, surface: Surface, request: &str, name: &str, args: Value) -> Value;
}
impl Server for Arc<Kernel> {
    fn info(&self, surface: Surface) -> Value {
        json!({"name":format!("chat-on-steroids-{}",surface.id()),"version":cos_core::APP_VERSION})
    }
    fn tools(&self, surface: Surface) -> Vec<Value> {
        Kernel::tools(self, surface)
    }
    fn call(&self, surface: Surface, request: &str, name: &str, args: Value) -> Value {
        self.invoke(surface, request, name, args)
    }
}
pub fn error(id: Value, status: u16, code: i64, message: impl Into<String>) -> Reply {
    Reply {
        status,
        body: json!({"jsonrpc":"2.0","error":{"code":code,"message":message.into()},"id":id}),
        sse: false,
    }
}
fn mismatch(id: Value, header: &str, body: &str) -> Reply {
    let mut reply = error(
        id,
        400,
        -32020,
        format!("Bad Request: the request headers and body disagree: {body}"),
    );
    reply.body["error"]["data"] = json!({"mismatch":{"header":header,"body":body}});
    reply
}
pub fn dispatch(
    kernel: &Arc<Kernel>,
    surface: Surface,
    request: &str,
    http_method: &str,
    headers: &BTreeMap<String, String>,
    body: Value,
) -> Reply {
    dispatch_server(kernel, surface, request, http_method, headers, body)
}
pub fn dispatch_server(
    kernel: &dyn Server,
    surface: Surface,
    request: &str,
    http_method: &str,
    headers: &BTreeMap<String, String>,
    body: Value,
) -> Reply {
    if http_method != "POST" {
        return error(Value::Null, 405, -32000, "Method not allowed.");
    }
    if let Some(batch) = body.as_array() {
        if batch.iter().any(|v| {
            v["params"]["_meta"]
                .get("io.modelcontextprotocol/protocolVersion")
                .is_some()
        }) {
            return error(
                Value::Null,
                400,
                -32600,
                "Bad Request: JSON-RPC batches may not contain requests for protocol revision 2026-07-28 or later",
            );
        }
        if batch.iter().any(Value::is_array) {
            return error(Value::Null, 400, -32600, "Invalid Request");
        }
        if batch.is_empty() || batch.len() > 100 {
            return error(Value::Null, 400, -32600, "Invalid Request");
        }
        let mut replies = vec![];
        for message in batch {
            let reply = dispatch_server(
                kernel,
                surface,
                request,
                http_method,
                headers,
                message.clone(),
            );
            if reply.status != 202 {
                replies.push(reply.body);
            }
        }
        return Reply {
            status: if replies.is_empty() { 202 } else { 200 },
            body: json!(replies),
            sse: true,
        };
    }
    let id = body.get("id").cloned().unwrap_or(Value::Null);
    if body["jsonrpc"] != "2.0"
        || body["method"].as_str().is_none()
        || (!id.is_null() && !id.is_string() && !id.is_number())
    {
        return error(id, 400, -32600, "Invalid Request");
    }
    // MCP method parameters use named JSON objects. Reject scalar or array
    // params before dispatch, rather than silently turning malformed calls
    // into valid tools/list or invoking a handler.
    if body.get("params").is_some_and(|params| !params.is_object()) {
        return error(id, 200, -32602, "Invalid params");
    }
    let method = body["method"].as_str().unwrap();
    let meta = &body["params"]["_meta"];
    let claim = meta["io.modelcontextprotocol/protocolVersion"].as_str();
    let modern = claim.is_some();
    if !modern
        && headers
            .get("accept")
            .is_some_and(|a| !a.contains("application/json") || !a.contains("text/event-stream"))
    {
        return error(
            Value::Null,
            406,
            -32000,
            "Not Acceptable: Client must accept both application/json and text/event-stream",
        );
    }
    if let Some(claim) = claim {
        if claim != "2026-07-28" {
            return error(
                id,
                400,
                -32019,
                format!("Unsupported protocol version: {claim}"),
            );
        }
        let checks = [
            (
                "mcp-protocol-version",
                "MCP-Protocol-Version",
                claim,
                format!("the body envelope names protocol version {claim}"),
            ),
            (
                "mcp-method",
                "Mcp-Method",
                method,
                format!("the body names method {method}"),
            ),
        ];
        for (key, name, expected, detail) in checks {
            match headers.get(key) {
                None => {
                    return mismatch(
                        id,
                        "(missing)",
                        &format!("{detail} but the required {name} header is absent"),
                    );
                }
                Some(actual) if actual != expected => {
                    return mismatch(
                        id,
                        actual,
                        &format!("{detail} but the {name} header names {actual}"),
                    );
                }
                _ => {}
            }
        }
        if method == "tools/call"
            && let Some(name) = body["params"]["name"].as_str()
        {
            match headers.get("mcp-name") {
                None => {
                    return mismatch(
                        id,
                        "(missing)",
                        &format!(
                            "the body names {name} but the required Mcp-Name header is absent"
                        ),
                    );
                }
                Some(actual) if actual != name => {
                    return mismatch(
                        id,
                        actual,
                        &format!("the body names {name} but the Mcp-Name header names {actual}"),
                    );
                }
                _ => {}
            }
        }
    } else if headers
        .get("mcp-protocol-version")
        .is_some_and(|s| !LEGACY.contains(&s.as_str()))
    {
        return error(id, 400, -32000, "Bad Request: Unsupported protocol version");
    }
    if body.get("id").is_none() {
        return Reply {
            status: 202,
            body: Value::Null,
            sse: false,
        };
    }
    let server = kernel.info(surface);
    let mut result = match method {
        "initialize" if !modern => {
            let version = body["params"]["protocolVersion"].as_str().unwrap_or("");
            json!({"protocolVersion":if LEGACY.contains(&version){version}else{"2025-11-25"},"capabilities":{"tools":{"listChanged":true}},"serverInfo":server})
        }
        "server/discover" if modern => {
            json!({"supportedVersions":["2026-07-28"],"capabilities":{"tools":{"listChanged":true}}})
        }
        "ping" if !modern => json!({}),
        "tools/list" => json!({"tools":kernel.tools(surface)}),
        "tools/call" => {
            let Some(name) = body["params"]["name"].as_str() else {
                return error(id, 200, -32602, "Invalid tools/call parameters");
            };
            if !kernel.tools(surface).iter().any(|t| t["name"] == name) {
                return error(id, 200, -32602, format!("Tool {name} not found"));
            }
            kernel.call(
                surface,
                request,
                name,
                body["params"]
                    .get("arguments")
                    .cloned()
                    .unwrap_or(json!({})),
            )
        }
        _ => {
            return error(
                id,
                if modern { 404 } else { 200 },
                -32601,
                "Method not found",
            );
        }
    };
    if modern {
        result["resultType"] = json!("complete");
        result["_meta"] = json!({"io.modelcontextprotocol/serverInfo":server});
        if matches!(method, "tools/list" | "server/discover") {
            result["ttlMs"] = json!(0);
            result["cacheScope"] = json!("private");
        }
    }
    Reply {
        status: 200,
        body: json!({"result":result,"jsonrpc":"2.0","id":id}),
        sse: !modern,
    }
}
