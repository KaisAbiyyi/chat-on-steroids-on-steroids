//! Compatible wire records preserve unknown forward fields; identities are validated at ingress.
use regex::Regex;
use serde_json::{Value, json};
use std::{io, sync::OnceLock};
/// JSON.stringify emits integral JavaScript Numbers without a decimal suffix.
pub fn javascript_numbers(value: Value) -> Value {
    match value {
        Value::Number(ref n) if n.is_f64() => {
            let f = n.as_f64().unwrap();
            if f.fract() == 0.0 && f.abs() <= 9_007_199_254_740_991.0 {
                json!(f as i64)
            } else {
                value
            }
        }
        Value::Array(rows) => Value::Array(rows.into_iter().map(javascript_numbers).collect()),
        Value::Object(rows) => Value::Object(
            rows.into_iter()
                .map(|(key, value)| (key, javascript_numbers(value)))
                .collect(),
        ),
        _ => value,
    }
}
pub fn number(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap_or(0)
}
pub fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
pub fn session_id(id: &str) -> bool {
    (8..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
pub fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
pub fn message_key(event: &Value) -> Option<String> {
    let kind = string(event, "kind");
    match kind {
        "tool_call" => {
            let id = event["call"]["callId"].as_str()?;
            (!id.is_empty()).then(|| format!("tool_call\0{id}"))
        }
        "native_image" => {
            let id = event["messageId"].as_str()?;
            let asset = event["providerAssetId"].as_str()?;
            (!id.is_empty() && !asset.is_empty()).then(|| format!("native_image\0{id}\0{asset}"))
        }
        "user_message" | "assistant_message" => {
            let id = event["messageId"].as_str()?;
            (!id.is_empty()).then(|| format!("{kind}\0{id}"))
        }
        _ => None,
    }
}
pub fn validate_event(event: &Value) -> io::Result<()> {
    if !event.is_object()
        || string(event, "kind").is_empty()
        || event["time"].as_f64().is_none_or(|v| !v.is_finite())
    {
        return Err(cos_core::persistence::invalid("Invalid history event"));
    }
    if matches!(
        string(event, "kind"),
        "user_message" | "assistant_message" | "native_image" | "tool_call"
    ) && message_key(event).is_none()
    {
        return Err(cos_core::persistence::invalid(
            "Missing stable history identity",
        ));
    }
    if string(event, "kind") == "tool_call" {
        for key in ["args", "result"] {
            validate_text(&event["call"][key])?;
        }
    } else if matches!(
        string(event, "kind"),
        "user_message" | "assistant_message" | "note" | "chat_error" | "progress" | "agent_message"
    ) {
        validate_text(&event["message"])?;
    }
    Ok(())
}
fn validate_text(value: &Value) -> io::Result<()> {
    if value["text"].is_string()
        && value["truncated"].is_boolean()
        && value["chars"]
            .as_u64()
            .is_some_and(|n| n <= 9_007_199_254_740_991)
    {
        Ok(())
    } else {
        Err(cos_core::persistence::invalid("Invalid stored text"))
    }
}
pub fn estimate_tokens(text: &str) -> u64 {
    (text.encode_utf16().count() as u64).div_ceil(4)
}
pub fn stored_tokens(value: &Value) -> u64 {
    if value["truncated"] == true
        && let Some(chars) = value["chars"]
            .as_u64()
            .filter(|n| *n <= 9_007_199_254_740_991)
    {
        return chars.div_ceil(4);
    }
    estimate_tokens(string(value, "text"))
}
pub fn event_tokens(event: &Value) -> u64 {
    match string(event, "kind") {
        "user_message" => stored_tokens(&event["message"]).max(
            event["wireTokenEstimate"]
                .as_f64()
                .filter(|n| n.is_finite())
                .unwrap_or(0.0)
                .max(0.0)
                .floor() as u64,
        ),
        "assistant_message" | "chat_error" | "note" | "agent_message" => {
            stored_tokens(&event["message"])
        }
        "tool_call" if event["call"]["nested"] != true => {
            stored_tokens(&event["call"]["args"])
                + stored_tokens(&event["call"]["result"]).min(10000)
                + estimate_tokens(string(&event["call"]["summary"], "title"))
        }
        _ => 0,
    }
}
pub fn terminal(event: &Value) -> bool {
    event["final"] == true || event["state"] == "final"
}
pub fn work_sequence(event: &Value) -> u64 {
    match string(event, "kind") {
        "user_message" | "page_tool" => event["contentSeq"]
            .as_u64()
            .unwrap_or_else(|| position(event)),
        "assistant_message" => event["contentSeq"]
            .as_u64()
            .or(event["finalContentSeq"].as_u64())
            .unwrap_or_else(|| position(event)),
        _ => position(event),
    }
}
pub fn position(event: &Value) -> u64 {
    event["origin"]
        .as_u64()
        .unwrap_or_else(|| number(event, "seq"))
}
pub fn residence(event: &Value) -> u64 {
    if event["kind"] == "assistant_message" && event["final"] == true {
        position(event).max(number(event, "contentSeq"))
    } else {
        position(event)
    }
}
pub fn normalized_outcome(call: &Value) -> Option<&str> {
    match string(call, "outcome") {
        v @ ("ok"
        | "process_exit_nonzero"
        | "tool_rejected"
        | "tool_execution_error"
        | "tool_internal_error") => Some(v),
        "rejected" => Some("tool_rejected"),
        "error" => {
            static EXIT: OnceLock<Regex> = OnceLock::new();
            if matches!(string(call, "tool"), "exec_command" | "write_stdin")
                && EXIT
                    .get_or_init(|| Regex::new(r"^✕ exit -?\d+$").unwrap())
                    .is_match(string(&call["summary"], "metric"))
            {
                Some("process_exit_nonzero")
            } else {
                None
            }
        }
        _ => None,
    }
}
pub fn empty_summary(id: &str, title: &str, conversation: Option<&str>, now: i64) -> Value {
    json!({"id":id,"title":title,"conversationId":conversation,"chatIds":conversation.into_iter().collect::<Vec<_>>(),"startedAt":now,"updatedAt":now,"endedAt":null,"events":0,"timelineTurns":{},"nativeQuestion":null,"requestTurns":{},"userMessages":0,"toolCalls":0,"lastToolCallAt":null,"lastToolActivity":null,"lastAssistantFinalAt":null,"lastTurnEndAt":null,"lastFinishReportAt":null,"processExitNonzero":0,"toolRejected":0,"toolInternalErrors":0,"errors":0,"estimatedTokens":0,"contextTokens":0,"lastHandoffId":null,"lastHandoffAt":null,"lastCommittedResumeHandoffId":null,"lastTurnOutcome":null,"activeTurnId":null,"finishTurn":null,"agents":[],"origin":null})
}
fn increment(summary: &mut Value, key: &str, amount: u64) {
    summary[key] = json!(number(summary, key).saturating_add(amount));
}
pub fn max_time(summary: &mut Value, key: &str, time: &Value) {
    let value = summary[key]
        .as_i64()
        .unwrap_or(0)
        .max(time.as_i64().unwrap_or(0));
    summary[key] = json!(value);
}
pub fn note_finish_work(summary: &mut Value, event: &Value) {
    if !summary["finishTurn"].is_object()
        || event["turnId"].is_string() && event["turnId"] != summary["finishTurn"]["turnId"]
    {
        return;
    }
    let meaningful = match string(event, "kind") {
        "user_message" | "assistant_message" => true,
        "progress" => event["source"] != "app",
        "tool_call" => !matches!(
            string(&event["call"], "tool"),
            "keep_astra_on_forever" | "session_finish"
        ),
        _ => false,
    };
    if meaningful {
        let n = number(&summary["finishTurn"], "workSeq").max(work_sequence(event));
        summary["finishTurn"]["workSeq"] = json!(n);
    }
}
pub fn apply_summary(summary: &mut Value, event: &Value) {
    crate::chronology::apply_identity(summary, event);
    increment(summary, "events", 1);
    max_time(summary, "updatedAt", &event["time"]);
    let tokens = event_tokens(event);
    increment(summary, "estimatedTokens", tokens);
    increment(summary, "contextTokens", tokens);
    match string(event, "kind") {
        "user_message" => increment(summary, "userMessages", 1),
        "tool_call" => {
            increment(summary, "toolCalls", 1);
            if event["time"].as_i64().unwrap_or(0)
                >= summary["lastToolCallAt"].as_i64().unwrap_or(0)
            {
                let mut activity = json!({"title":string(&event["call"]["summary"],"title").chars().take(200).collect::<String>()});
                if let Some(kind) = event["call"]["summary"].get("kind") {
                    activity["kind"] = kind.clone();
                }
                summary["lastToolActivity"] = activity;
            }
            max_time(summary, "lastToolCallAt", &event["time"]);
            if event["call"]["endsActivity"] == true {
                max_time(summary, "lastFinishReportAt", &event["time"]);
            }
            match normalized_outcome(&event["call"]) {
                Some("process_exit_nonzero") => increment(summary, "processExitNonzero", 1),
                Some("tool_rejected") => increment(summary, "toolRejected", 1),
                Some("tool_internal_error") => {
                    increment(summary, "toolInternalErrors", 1);
                    increment(summary, "errors", 1);
                }
                _ => {}
            }
        }
        "assistant_message" if terminal(event) => {
            max_time(summary, "lastAssistantFinalAt", &event["time"])
        }
        "chat_error" => increment(summary, "errors", 1),
        "turn_start" => {
            summary["activeTurnId"] = event
                .get("turnId")
                .cloned()
                .unwrap_or_else(|| json!(format!("seq-{}", number(event, "seq"))));
            if summary["finishTurn"]["turnId"] != summary["activeTurnId"] {
                summary["finishTurn"] = json!({"turnId":summary["activeTurnId"],"conversationId":summary["conversationId"],"startedAt":event["time"],"notified":false,"released":false,"decisionRevision":null,"workSeq":0,"decisionSeq":0,"decisionInputRevision":null});
            }
        }
        "turn_end" => {
            summary["lastTurnOutcome"] = event["outcome"].clone();
            max_time(summary, "lastTurnEndAt", &event["time"]);
            if !event["turnId"].is_string() || summary["activeTurnId"] == event["turnId"] {
                summary["activeTurnId"] = Value::Null;
            }
        }
        "handoff" => {
            summary["lastHandoffId"] = event["handoffId"].clone();
            summary["lastHandoffAt"] = event["time"].clone();
        }
        "progress"
            if event["source"] == "app"
                && event["turnId"].is_string()
                && summary["finishTurn"]["turnId"] == event["turnId"] =>
        {
            let turn = string(event, "turnId");
            if event["progressId"] == format!("finish:{turn}")
                || event["finishControl"]["state"] == "notified"
            {
                summary["finishTurn"]["notified"] = json!(true);
            }
            if event["finishControl"]["state"] == "released" {
                summary["finishTurn"]["released"] = json!(true);
            }
            let prefix = format!("finish-goal:{turn}:");
            let revision = event["finishControl"]["revision"]
                .as_str()
                .or_else(|| string(event, "progressId").strip_prefix(&prefix));
            if let Some(revision) = revision.filter(|r| {
                r.len() == 64
                    && r.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }) {
                let prior = summary["finishTurn"]["decisionRevision"].clone();
                summary["finishTurn"]["decisionRevision"] = json!(revision);
                if prior != revision {
                    summary["finishTurn"]["decisionAt"] = event["time"].clone();
                    summary["finishTurn"]["decisionSeq"] = event["finishControl"]
                        .get("workSeq")
                        .cloned()
                        .unwrap_or_else(|| event["seq"].clone());
                    summary["finishTurn"]["decisionInputRevision"] =
                        event["finishControl"]["inputRevision"].clone();
                } else {
                    let next = number(&summary["finishTurn"], "decisionSeq")
                        .max(number(&event["finishControl"], "workSeq"));
                    summary["finishTurn"]["decisionSeq"] = json!(next);
                }
            }
            if let Some(conversation) = event["finishControl"].get("conversationId") {
                summary["finishTurn"]["conversationId"] = conversation.clone();
            }
        }
        _ => {}
    }
    note_finish_work(summary, event);
    if let Some(agent) = event["agent"].as_str()
        && let Some(agents) = summary["agents"].as_array_mut()
        && !agents.iter().any(|v| v == agent)
    {
        agents.push(json!(agent));
    }
}
pub fn fold_progress(events: &[Value]) -> Vec<Value> {
    use std::collections::HashMap;
    let mut anchors = HashMap::<String, usize>::new();
    let mut out = Vec::<Value>::new();
    for event in events {
        let key = match string(event, "kind") {
            "progress" if event["progressId"].is_string() => {
                Some(format!("progress\0{}", string(event, "progressId")))
            }
            "page_tool" if event["messageId"].is_string() => {
                Some(format!("page_tool\0{}", string(event, "messageId")))
            }
            "tool_call" => message_key(event),
            _ => None,
        };
        if let Some(key) = key {
            if let Some(at) = anchors.get(&key) {
                let held = &mut out[*at];
                match string(event, "kind") {
                    "progress" => held["message"] = event["message"].clone(),
                    "page_tool" => held["label"] = event["label"].clone(),
                    "tool_call" if number(event, "seq") >= number(held, "seq") => {
                        let origin = position(held);
                        let time = held["time"].clone();
                        *held = event.clone();
                        held["origin"] = json!(origin);
                        held["time"] = time;
                    }
                    _ => {}
                }
                continue;
            }
            anchors.insert(key, out.len());
        }
        out.push(event.clone());
    }
    out
}
