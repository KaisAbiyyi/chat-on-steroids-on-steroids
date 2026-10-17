use cos_mcp::{
    kernel::Surface,
    protocol::{self, Server},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
struct Echo;
impl Server for Echo {
    fn info(&self, _: Surface) -> Value {
        json!({"name":"fixture","version":"1"})
    }
    fn tools(&self, _: Surface) -> Vec<Value> {
        vec![
            json!({"name":"echo","inputSchema":{"type":"object","$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}}),
        ]
    }
    fn call(&self, _: Surface, _: &str, _: &str, args: Value) -> Value {
        if let Err(e) =
            cos_mcp::schema::validate(&self.tools(Surface::Core)[0]["inputSchema"], &args)
        {
            cos_mcp::kernel::fail(format!(
                "Input validation error: Invalid arguments for tool echo: {e}"
            ))
        } else {
            json!({"content":[{"type":"text","text":args["text"]}]})
        }
    }
}
fn normalize(v: Value) -> Value {
    if let Some(text) = v.as_str() {
        if text.is_empty() {
            return Value::Null;
        }
        let frames = text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|s| serde_json::from_str::<Value>(s).unwrap())
            .collect::<Vec<_>>();
        if frames.len() == 1 {
            frames[0].clone()
        } else {
            json!(frames)
        }
    } else if let Some(a) = v.as_array() {
        if a.len() == 1 { a[0].clone() } else { v }
    } else {
        v
    }
}
#[test]
fn actual_sdk_protocol_oracle() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase4-6/mcp-protocol.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let request = &case["request"];
        if request.get("raw").is_some() {
            continue;
        }
        let mut headers = BTreeMap::from([
            (
                "accept".to_string(),
                "application/json, text/event-stream".to_string(),
            ),
            ("content-type".into(), "application/json".into()),
        ]);
        if let Some(h) = request["headers"].as_object() {
            headers.extend(
                h.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())),
            );
        }
        let reply = protocol::dispatch_server(
            &Echo,
            Surface::Core,
            "public",
            request["method"].as_str().unwrap(),
            &headers,
            request["body"].clone(),
        );
        assert_eq!(
            reply.status,
            case["status"].as_u64().unwrap() as u16,
            "{request}"
        );
        assert_eq!(
            normalize(reply.body),
            normalize(case["result"].clone()),
            "{request}"
        );
    }
}
