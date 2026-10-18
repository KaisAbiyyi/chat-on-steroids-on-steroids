//! Shared transcript selection and Markdown formatting, independent of UI hosts.
use crate::{
    content::re,
    model::{number, string, work_sequence},
};
use cos_core::config::js_trim;
use serde_json::{Value, json};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

fn is_continuation(text: &str) -> bool {
    if re!(&format!(
        r"^[{0}]*\[\[CLF-(HANDOFF|RESUME):([A-Za-z0-9_-]{{16,64}})\]\](?:[{0}]|$)",
        crate::title::JS_SPACE
    ))
    .is_match(text)
    {
        return true;
    }
    // Only the escaped source branch is capped at 200 UTF-16 units. Check its
    // boundary directly so a split surrogate is never replaced just to run a regex.
    let Some(marker) = re!(&format!(r"^[{0}]*(?:\\?\[){{2}}CLF\\?-(HANDOFF|RESUME)\\?:((?:[A-Za-z0-9]|\\?[_-]){{16,64}})(?:\\?\]){{2}}", crate::title::JS_SPACE)).find(text) else { return false };
    let mut units = marker.as_str().encode_utf16().count();
    if units > 200 {
        return false;
    }
    let mut tail = &text[marker.end()..];
    if units < 200
        && let Some(rest) = tail.strip_prefix('\\')
    {
        tail = rest;
        units += 1;
    }
    units == 200
        || tail.is_empty()
        || tail
            .chars()
            .next()
            .is_some_and(|ch| js_trim(&ch.to_string()).is_empty())
}
pub fn transcript_entries(events: &[Value]) -> Vec<Value> {
    let mut messages: Vec<(Value, u64)> = Vec::new();
    let mut keys = HashMap::new();
    let mut anonymous = 0;
    for event in events {
        let kind = string(event, "kind");
        if !matches!(kind, "user_message" | "assistant_message")
            || kind == "assistant_message" && event["final"] != true
        {
            continue;
        }
        let id = string(event, "messageId");
        let key = if id.is_empty() {
            anonymous += 1;
            format!("anonymous\0{anonymous}")
        } else {
            format!("{kind}\0{id}")
        };
        let place = work_sequence(event);
        if let Some(&index) = keys.get(&key) {
            let (latest, origin): &mut (Value, u64) = &mut messages[index];
            *origin = (*origin).min(place);
            if number(event, "seq") > number(latest, "seq") {
                *latest = event.clone();
            }
        } else {
            keys.insert(key, messages.len());
            messages.push((event.clone(), place));
        }
    }
    messages.retain(|(event, _)| {
        !is_continuation(string(&event["message"], "text"))
            && !js_trim(string(&spoken(event), "text")).is_empty()
    });
    messages.sort_by_key(|(event, place)| (*place, number(event, "seq")));
    messages.into_iter().map(|(event, _)| {
        let assistant = event["kind"] == "assistant_message";
        let mut entry = json!({"role":if assistant {"assistant"} else {"user"},"stored":spoken(&event),"turnId":event.get("turnId").unwrap_or(&Value::Null),"seq":event["seq"]});
        if assistant && let Some(html) = event.get("renderedHtml").filter(|v| !v.is_null()) { entry["html"] = html.clone(); }
        entry
    }).collect()
}
fn spoken(event: &Value) -> Value {
    if event["kind"] == "user_message"
        && let Some(text) = event["authoredText"]
            .as_str()
            .filter(|s| !js_trim(s).is_empty())
    {
        return json!({"text":text,"truncated":false,"chars":text.encode_utf16().count()});
    }
    event["message"].clone()
}
pub fn completed_turn_ids(events: &[Value]) -> Vec<String> {
    let mut completed = Vec::new();
    for event in events {
        if event["kind"] != "turn_end" {
            continue;
        }
        let id = string(event, "turnId");
        if id.is_empty() {
            continue;
        }
        if event["outcome"] == "completed" {
            if !completed.iter().any(|v| v == id) {
                completed.push(id.to_owned());
            }
        } else {
            completed.retain(|v| v != id);
        }
    }
    completed
}
pub fn turn_answer_entries(events: &[Value], turn_id: &str) -> Vec<Value> {
    transcript_entries(events)
        .into_iter()
        .filter(|entry| entry["role"] == "assistant" && entry["turnId"] == turn_id)
        .collect()
}
pub fn answer_anchors(events: &[Value]) -> Vec<(String, u64)> {
    let completed = completed_turn_ids(events);
    let mut anchors: Vec<(String, u64)> = Vec::new();
    for entry in transcript_entries(events) {
        let turn = string(&entry, "turnId");
        if entry["role"] != "assistant" || !completed.iter().any(|v| v == turn) {
            continue;
        }
        if let Some((_, seq)) = anchors.iter_mut().find(|(id, _)| id == turn) {
            *seq = number(&entry, "seq");
        } else {
            anchors.push((turn.into(), number(&entry, "seq")));
        }
    }
    anchors
}
pub fn answer_markdown(texts: &[String]) -> String {
    format!(
        "{}\n",
        texts
            .iter()
            .map(|s| js_trim(s))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    )
}
pub fn session_markdown(
    title: &str,
    entries: &[Value],
    user_label: &str,
    assistant_label: &str,
) -> String {
    let title = js_trim(title);
    let mut parts = vec![format!(
        "# {}",
        if title.is_empty() { "Chat" } else { title }
    )];
    for entry in entries {
        let text = js_trim(string(entry, "text"));
        if !text.is_empty() {
            parts.push(format!(
                "## {}\n\n{text}",
                if entry["role"] == "user" {
                    user_label
                } else {
                    assistant_label
                }
            ));
        }
    }
    format!("{}\n", parts.join("\n\n"))
}
pub fn markdown_file_name(title: &str, suffix: &str) -> String {
    let normalized = title.nfc().collect::<String>();
    let normalized = re!(r#"[\\/:*?"<>|\u{0000}-\u{001f}]+"#).replace_all(&normalized, " ");
    let normalized = re!(&format!(r"[{}]+", crate::title::JS_SPACE)).replace_all(&normalized, " ");
    let base = js_trim(&normalized).trim_end_matches(['.', ' ']);
    let sliced = crate::title::utf16_prefix(base, 80)
        .unwrap_or_else(|_| crate::title::utf16_prefix(base, 79).unwrap());
    let base = js_trim(&sliced);
    format!("{}{suffix}.md", if base.is_empty() { "chat" } else { base })
}
