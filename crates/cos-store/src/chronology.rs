//! Presentation order is turn-local; append sequence stays immutable.
use crate::model::{number, position, string, terminal};
use regex::Regex;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    sync::OnceLock,
};
fn time(v: &Value) -> i64 {
    v["time"].as_i64().unwrap_or(0)
}
pub fn response_turn<'a>(turns: &'a Value, id: &'a str) -> &'a str {
    let owner = turns[id]["responseTurnId"].as_str();
    owner
        .filter(|owner| {
            turns[*owner].is_object()
                && number(&turns[*owner], "origin") < number(&turns[id], "origin")
        })
        .unwrap_or(id)
}
pub fn injected(event: &Value, turns: &Value) -> bool {
    event["kind"] == "user_message"
        && event["inputId"].as_str().is_some_and(|s| !s.is_empty())
        && (string(event, "messageId").starts_with("input:")
            || turns.get(string(event, "turnId")).is_some())
}
pub fn authored_time(event: &Value) -> Option<i64> {
    if let Some(at) = event["authoredAt"].as_i64().filter(|n| *n > 0) {
        return Some(at);
    }
    if event["kind"] != "assistant_message" {
        return None;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let caps = RE
        .get_or_init(|| {
            Regex::new(r"(?i)^assistant:([a-f0-9-]{36})?:([a-f0-9-]{36})?:(\d{13})$").unwrap()
        })
        .captures(string(event, "messageId"))?;
    if caps.get(1).is_none() && caps.get(2).is_none() {
        return None;
    }
    caps.get(3)?.as_str().parse().ok()
}
pub fn recorded_request<'a>(
    requests: &'a Value,
    request: &str,
    conversation: &str,
) -> Option<&'a Value> {
    let owner = requests.get(request)?;
    Some(if owner["conversationId"] == conversation {
        owner
    } else {
        &Value::Null
    })
}
fn continued(turns: &Value, held: &str, later: &str, at: i64) -> bool {
    let held = response_turn(turns, held);
    let later = response_turn(turns, later);
    let a = &turns[held];
    let b = &turns[later];
    held != later
        && a["continued"] == true
        && a["endTime"]
            .as_i64()
            .is_some_and(|end| at > end && time(b) >= end && at >= time(b))
}
fn overlapping(turns: &Value, left: &str, right: &str, requests: &Value, request: &str) -> bool {
    let left = response_turn(turns, left);
    let right = response_turn(turns, right);
    let a = &turns[left];
    let b = &turns[right];
    if !a.is_object()
        || !b.is_object()
        || a["questionId"].as_str().is_none_or(str::is_empty)
        || a["questionId"] != b["questionId"]
    {
        return false;
    }
    if left != right
        && requests.as_object().is_some_and(|map| {
            map.iter().any(|(id, owner)| {
                id != request
                    && owner.is_object()
                    && [left, right].contains(&response_turn(turns, string(owner, "turnId")))
            })
        })
    {
        return false;
    }
    let (earlier, later) = if number(a, "origin") <= number(b, "origin") {
        (a, b)
    } else {
        (b, a)
    };
    earlier["endOrigin"]
        .as_u64()
        .is_none_or(|end| number(later, "origin") < end)
}
pub fn apply_identity(identity: &mut Value, event: &Value) {
    let origin = position(event);
    let turn = string(event, "turnId");
    if event["kind"] == "user_message"
        && !string(event, "messageId").is_empty()
        && !injected(event, &identity["timelineTurns"])
        && (!identity["nativeQuestion"].is_object()
            || origin > number(&identity["nativeQuestion"], "origin"))
    {
        identity["nativeQuestion"] = json!({"messageId":event["messageId"],"origin":origin});
    }
    if !identity["timelineTurns"].is_object() {
        identity["timelineTurns"] = json!({});
    }
    if !identity["requestTurns"].is_object() {
        identity["requestTurns"] = json!({});
    }
    if !turn.is_empty() {
        if event["kind"] == "turn_start" {
            if !identity["timelineTurns"][turn].is_object() {
                let mut start = json!({"origin":origin,"time":event["time"]});
                if identity["nativeQuestion"].is_object() {
                    start["questionId"] = identity["nativeQuestion"]["messageId"].clone();
                }
                identity["timelineTurns"][turn] = start;
            } else if identity["timelineTurns"][turn]["endOrigin"]
                .as_u64()
                .is_some_and(|end| origin > end)
            {
                let held = identity["timelineTurns"][turn].as_object_mut().unwrap();
                held.remove("endTime");
                held.remove("endOrigin");
            }
        } else if event["kind"] == "turn_end" && identity["timelineTurns"][turn].is_object() {
            let held = &mut identity["timelineTurns"][turn];
            held["endTime"] = json!(held["endTime"].as_i64().unwrap_or(0).max(time(event)));
            held["endOrigin"] = json!(held["endOrigin"].as_u64().unwrap_or(u64::MAX).min(origin));
            if event["continued"] == true {
                held["continued"] = json!(true);
            }
        }
    }
    if event["kind"] != "tool_call"
        || event["source"] != "mcp"
        || turn.is_empty()
        || event["call"]["attribution"] != "request_id"
    {
        return;
    }
    let request = string(&event["call"], "requestId");
    let conversation = string(&event["call"], "conversationId");
    if request.is_empty() || conversation.is_empty() {
        return;
    }
    let mut turns = identity["timelineTurns"].clone();
    let requests = &identity["requestTurns"];
    let held = recorded_request(requests, request, conversation).cloned();
    let mut owner = response_turn(&turns, turn).to_owned();
    if let Some(held) = held.as_ref().filter(|h| h.is_object()) {
        let prior = response_turn(&turns, string(held, "turnId")).to_owned();
        if prior != owner && continued(&turns, &prior, &owner, time(event)) {
            identity["requestTurns"][request] =
                json!({"turnId":owner,"conversationId":conversation,"origin":origin});
            return;
        }
        if prior != owner
            && continued(&turns, &owner, &prior, i64::MAX)
            && turns[&owner]["endTime"]
                .as_i64()
                .is_some_and(|end| time(event) <= end)
        {
            return;
        }
        if prior != owner {
            if !overlapping(&turns, &prior, &owner, requests, request) {
                identity["requestTurns"][request] = Value::Null;
                return;
            }
            let (earlier, later) =
                if number(&turns[&prior], "origin") <= number(&turns[&owner], "origin") {
                    (prior, owner)
                } else {
                    (owner, prior)
                };
            for (id, turn) in turns.as_object_mut().unwrap() {
                if *id == later || turn["responseTurnId"] == later {
                    turn["responseTurnId"] = json!(earlier);
                }
            }
            owner = earlier;
            identity["timelineTurns"] = turns;
        }
    }
    if held.as_ref().is_some_and(Value::is_null) {
        identity["requestTurns"][request] = Value::Null;
        return;
    }
    let prior = held.as_ref().filter(|h| h.is_object());
    if prior.is_some_and(|h| h["turnId"] == owner && number(h, "origin") <= origin) {
        return;
    }
    identity["requestTurns"][request] = json!({"turnId":owner,"conversationId":conversation,"origin":prior.map(|h|number(h,"origin")).unwrap_or(u64::MAX).min(origin)});
}
fn response_key(event: &Value) -> Option<String> {
    if event["kind"] != "assistant_message" || event["source"] != "extension" {
        return None;
    }
    let parts = string(event, "messageId").split(':').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != "assistant" {
        return None;
    }
    let timestamp = parts[3].len() == 13 && parts[3].bytes().all(|b| b.is_ascii_digit());
    let valid = |s: &str| uuid::Uuid::parse_str(s).is_ok() && s.len() == 36;
    if !timestamp && !valid(parts[1]) {
        return None;
    }
    let working = parts[if timestamp { 1 } else { 2 }];
    let exchange = parts[if timestamp { 2 } else { 3 }];
    if !valid(working) || !valid(exchange) {
        return None;
    }
    Some(format!(
        "{}\0{}:{}",
        string(event, "agent"),
        working.to_lowercase(),
        exchange.to_lowercase()
    ))
}
pub fn project(entries: &[Value], summary: &Value, messages: &[Value]) -> Vec<Value> {
    let turns = &summary["timelineTurns"];
    let requests = &summary["requestTurns"];
    let mut starts: Vec<Value> = turns
        .as_object()
        .map(|t| t.values().cloned().collect())
        .unwrap_or_default();
    starts.sort_by_key(|v| number(v, "origin"));
    let mut responses = HashMap::<String, Value>::new();
    for message in messages {
        let Some(key) = response_key(message) else {
            continue;
        };
        let turn = string(message, "turnId");
        if turn.is_empty() {
            continue;
        }
        let boundary = turns[response_turn(turns, turn)].clone();
        responses
            .entry(key)
            .and_modify(|held| {
                if boundary.is_null() || held["origin"] != boundary["origin"] {
                    *held = Value::Null;
                }
            })
            .or_insert(boundary);
    }
    entries
        .iter()
        .map(|entry| {
            let turn = string(entry, "turnId");
            let mut boundary = if turn.is_empty() {
                response_key(entry)
                    .and_then(|key| responses.get(&key))
                    .cloned()
                    .unwrap_or(Value::Null)
            } else {
                turns[response_turn(turns, turn)].clone()
            };
            let request = if turn.is_empty()
                && entry["kind"] == "tool_call"
                && entry["source"] == "mcp"
                && entry["call"]["attribution"] == "request_id"
            {
                recorded_request(
                    requests,
                    string(&entry["call"], "requestId"),
                    string(&entry["call"], "conversationId"),
                )
            } else {
                None
            };
            if let Some(owner) = request.filter(|o| o.is_object()) {
                boundary = turns[response_turn(turns, string(owner, "turnId"))].clone();
            }
            if boundary.is_null()
                && turn.is_empty()
                && entry["kind"] == "assistant_message"
                && let Some(written) = authored_time(entry)
                && starts
                    .iter()
                    .any(|start| time(start) > written && number(start, "origin") < position(entry))
                && let Some((id, _)) = turns
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(_, start)| {
                        time(start) <= written
                            && start["endTime"].as_i64().is_none_or(|end| written <= end)
                    })
                    .max_by_key(|(_, start)| time(start))
            {
                boundary = turns[response_turn(turns, id)].clone();
            }
            if boundary.is_null()
                && request.is_none()
                && turn.is_empty()
                && !matches!(
                    string(entry, "kind"),
                    "user_message" | "assistant_message" | "native_image"
                )
                && let Some(candidate) = starts
                    .iter()
                    .rev()
                    .find(|s| number(s, "origin") < position(entry))
                && candidate["endTime"]
                    .as_i64()
                    .is_none_or(|end| time(entry) <= end)
            {
                boundary = candidate.clone();
            }
            let mut out = entry.clone();
            out["turnOrigin"] = boundary["origin"].clone();
            if let Some(at) = authored_time(entry) {
                out["authoredAt"] = json!(at);
            }
            out
        })
        .collect()
}
pub fn chronological(entries: &[Value]) -> Vec<Value> {
    let mut ordered = entries.to_vec();
    ordered.sort_by_key(|v| (position(v), number(v, "seq")));
    let mut anchors = HashMap::<String, u64>::new();
    let mut ends = HashMap::<u64, i64>::new();
    for entry in &ordered {
        let turn = string(entry, "turnId");
        if turn.is_empty() {
            continue;
        }
        if let Some(origin) = entry["turnOrigin"].as_u64() {
            anchors
                .entry(turn.into())
                .and_modify(|at| *at = (*at).min(origin))
                .or_insert(origin);
        }
        if entry["kind"] == "turn_start" {
            anchors.entry(turn.into()).or_insert(position(entry));
        }
    }
    for entry in &ordered {
        if entry["kind"] == "turn_end"
            && let Some(anchor) = anchors.get(string(entry, "turnId"))
        {
            ends.entry(*anchor)
                .and_modify(|t| *t = (*t).max(time(entry)))
                .or_insert(time(entry));
        }
    }
    let mut groups = BTreeMap::<u64, Vec<Value>>::new();
    let mut active = None;
    let mut pending = None;
    let mut current = None;
    for entry in ordered {
        let at = position(&entry);
        if current != Some(at) {
            if pending.is_some() {
                active = pending;
            }
            pending = None;
            current = Some(at);
        }
        let turn = string(&entry, "turnId");
        let inferred = if entry.get("turnOrigin") != Some(&Value::Null)
            && turn.is_empty()
            && entry["kind"] != "user_message"
        {
            active.filter(|anchor| ends.get(anchor).is_none_or(|end| time(&entry) <= *end))
        } else {
            None
        };
        let anchor = anchors
            .get(turn)
            .copied()
            .or(entry["turnOrigin"].as_u64())
            .or(inferred)
            .unwrap_or(at);
        if entry["kind"] == "turn_start" && !turn.is_empty() {
            pending = anchors.get(turn).copied();
        }
        groups.entry(anchor).or_default().push(entry);
    }
    let mut out = Vec::new();
    for mut group in groups.into_values() {
        let closing = group
            .iter()
            .filter(|e| e["kind"] == "assistant_message" && terminal(e))
            .max_by_key(|e| position(e))
            .map(|e| number(e, "seq"));
        let paragraphs: Vec<_> = group
            .iter()
            .filter(|e| e["kind"] == "assistant_message" && authored_time(e).is_some())
            .collect();
        let mut live: Vec<_> = paragraphs
            .iter()
            .copied()
            .filter(|e| {
                group
                    .iter()
                    .any(|later| later["kind"] != "turn_end" && time(later) > time(e))
            })
            .collect();
        live.sort_by_key(|e| authored_time(e));
        let mut before = HashMap::<u64, f64>::new();
        for work in &group {
            if !matches!(string(work, "kind"), "page_tool" | "tool_call")
                || authored_time(work).is_some()
            {
                continue;
            }
            let prose = if work["kind"] == "page_tool" {
                paragraphs.iter().copied().find(|e| {
                    position(e) > position(work)
                        && authored_time(e).unwrap() < time(work)
                        && time(work) <= time(e) + 50
                })
            } else {
                live.iter().copied().find(|e| {
                    authored_time(e).unwrap() < time(work)
                        && time(work) <= authored_time(e).unwrap() + 1000
                })
            };
            if let Some(prose) = prose {
                let opened = authored_time(prose).unwrap();
                let span = (time(prose) + 50).max(opened + 1000) - opened;
                before.insert(
                    number(work, "seq"),
                    opened as f64 - 1.0 + 0.9 * (time(work) - opened) as f64 / span as f64,
                );
            }
        }
        let rank = |e: &Value| {
            if e["kind"] == "turn_start" {
                -1.0_f64
            } else if e["kind"] == "turn_end" {
                1.0
            } else if closing == Some(number(e, "seq")) {
                0.5
            } else {
                0.0
            }
        };
        let effective = |e: &Value| {
            before
                .get(&number(e, "seq"))
                .copied()
                .unwrap_or_else(|| authored_time(e).unwrap_or_else(|| time(e)) as f64)
        };
        group.sort_by(|a, b| {
            rank(a)
                .total_cmp(&rank(b))
                .then_with(|| effective(a).total_cmp(&effective(b)))
                .then_with(|| position(a).cmp(&position(b)))
                .then_with(|| number(a, "seq").cmp(&number(b, "seq")))
        });
        out.extend(group);
    }
    out
}
