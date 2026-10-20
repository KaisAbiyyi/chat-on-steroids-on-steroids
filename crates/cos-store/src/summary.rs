//! Validated metadata is a checkpoint, never a replacement for unreadable history.
use serde_json::{Value, json};

/// Detect the JSON string representation serde_json cannot own, before a valid
/// source checkpoint could be mistaken for damaged metadata and overwritten.
pub(crate) fn requires_lossless_strings(text: &str) -> bool {
    requires_lossless_strings_bytes(text.as_bytes())
}

/// Detect surrogate escapes in JSON string tokens without decoding or replacing
/// original bytes. Valid high+low pairs and escaped literal backslashes remain valid.
/// This is a representation guard, not a replacement for JSON syntax validation.
pub(crate) fn requires_lossless_strings_bytes(bytes: &[u8]) -> bool {
    fn escaped_unit(bytes: &[u8], at: usize) -> Option<u16> {
        let encoded = bytes.get(at..at.checked_add(6)?)?;
        if &encoded[..2] != b"\\u" || !encoded[2..].iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        u16::from_str_radix(std::str::from_utf8(&encoded[2..]).ok()?, 16).ok()
    }
    let mut inside = false;
    let mut at = 0;
    while let Some(byte) = bytes.get(at) {
        if *byte == b'"' {
            inside = !inside;
            at += 1;
        } else if inside && *byte == b'\\' {
            match escaped_unit(bytes, at) {
                Some(0xd800..=0xdbff) => {
                    if !matches!(escaped_unit(bytes, at + 6), Some(0xdc00..=0xdfff)) {
                        return true;
                    }
                    at += 12;
                }
                Some(0xdc00..=0xdfff) => return true,
                Some(_) => at += 6,
                None => at += 2,
            }
        } else {
            at += 1;
        }
    }
    false
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub history_seq: Option<u64>,
    pub canonical_projection_current: bool,
    pub token_estimate_current: bool,
    pub outcome_counters_missing: bool,
    pub activity_boundary_missing: bool,
    pub summary: Value,
}
fn finite(value: &Value) -> bool {
    value.as_f64().is_some_and(f64::is_finite)
}
fn safe_sequence(value: &Value) -> Option<u64> {
    value
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 9_007_199_254_740_991.0 && v.fract() == 0.0)
        .map(|v| v as u64)
}
fn revision(value: &Value) -> bool {
    value.as_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
// The source regex coerces decisionRevision to a JS string. Among JSON nonstrings
// only a nested singleton array of a valid revision matches that expression.
fn coerced_revision(value: &Value) -> bool {
    revision(value)
        || value
            .as_array()
            .is_some_and(|rows| rows.len() == 1 && coerced_revision(&rows[0]))
}
fn nullable(value: Option<&Value>, predicate: impl FnOnce(&Value) -> bool) -> bool {
    value.is_some_and(|v| v.is_null() || predicate(v))
}
fn valid_model(value: &Value) -> bool {
    value["conversationId"].is_string()
        && value["model"].as_str().is_some_and(|s| {
            !s.is_empty()
                && s.len() <= 80
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b" ._-".contains(&b))
        })
        && finite(&value["observedAt"])
}
fn activity(value: &Value) -> bool {
    value.is_null()
        || value["title"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.encode_utf16().count() <= 200)
            && value["kind"].as_str().is_some_and(|s| {
                matches!(
                    s,
                    "edit"
                        | "create"
                        | "delete"
                        | "move"
                        | "read"
                        | "search"
                        | "browse"
                        | "run"
                        | "process"
                        | "screen"
                        | "input"
                        | "clipboard"
                        | "session"
                        | "agent"
                        | "other"
                )
            })
}
fn assignment(value: &Value) -> bool {
    value["conversationId"].is_string()
        && value["agentId"].is_string()
        && value["label"]
            .as_str()
            .is_some_and(|s| s.encode_utf16().count() <= 60)
        && value["task"]
            .as_str()
            .is_some_and(|s| s.encode_utf16().count() <= 8200)
        && finite(&value["recordedAt"])
}
fn finish(value: &Value) -> bool {
    value.is_null()
        || value.is_object()
            && value["turnId"].is_string()
            && finite(&value["startedAt"])
            && value["notified"].is_boolean()
            && value["released"].is_boolean()
            && safe_sequence(&value["workSeq"]).is_some()
            && safe_sequence(&value["decisionSeq"]).is_some()
            && nullable(value.get("decisionInputRevision"), revision)
            && nullable(value.get("conversationId"), Value::is_string)
            && nullable(value.get("decisionRevision"), coerced_revision)
}
pub fn normalize(id: &str, mut raw: Value) -> Option<Checkpoint> {
    if !raw.is_object() || raw["id"] != id {
        return None;
    }
    let history_seq = safe_sequence(&raw["__historySeq"]);
    let canonical_projection_current = raw["__canonicalProjection"].as_f64() == Some(1.0);
    let token_estimate_current = raw["__tokenEstimate"].as_f64() == Some(1.0);
    for key in ["__historySeq", "__canonicalProjection", "__tokenEstimate"] {
        raw.as_object_mut()?.remove(key);
    }
    if let Some(retired) = raw.get("retiredChatAt") {
        let filtered = retired
            .as_object()
            .map(|rows| {
                rows.iter()
                    .filter(|(chat, at)| {
                        raw["chatIds"]
                            .as_array()
                            .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(chat)))
                            && raw["conversationId"] != chat.as_str()
                            && finite(at)
                            && at.as_f64().unwrap() >= 0.0
                    })
                    .map(|(chat, at)| (chat.clone(), at.clone()))
                    .collect()
            })
            .unwrap_or_default();
        raw["retiredChatAt"] = Value::Object(filtered);
    }
    if raw
        .get("titleSource")
        .is_some_and(|source| !matches!(source.as_str(), Some("fallback" | "provider" | "manual")))
    {
        raw.as_object_mut()?.remove("titleSource");
    }
    if raw.get("autoTitle").is_some_and(|auto| {
        raw["titleSource"] != "manual"
            || !auto["title"].is_string()
            || !matches!(auto["source"].as_str(), Some("fallback" | "provider"))
    }) {
        raw.as_object_mut()?.remove("autoTitle");
    }
    for (key, valid) in [
        ("selectedModel", valid_model as fn(&Value) -> bool),
        ("lastToolActivity", activity),
        ("workerAssignment", assignment),
        ("finishTurn", finish),
    ] {
        if raw.get(key).is_some_and(|value| !valid(value)) {
            raw.as_object_mut()?.remove(key);
        }
    }
    let outcome_counters_missing = ["processExitNonzero", "toolRejected", "toolInternalErrors"]
        .into_iter()
        .any(|key| !raw[key].is_number());
    let activity_boundary_missing = raw.get("lastAssistantFinalAt").is_none();
    for key in ["processExitNonzero", "toolRejected", "toolInternalErrors"] {
        if raw[key].is_null() {
            raw[key] = json!(0);
        }
    }
    for key in [
        "lastToolCallAt",
        "lastAssistantFinalAt",
        "lastTurnEndAt",
        "lastFinishReportAt",
    ] {
        if !finite(&raw[key]) {
            raw[key] = Value::Null;
        }
    }
    if raw["lastToolActivity"].is_null() {
        raw["lastToolActivity"] = Value::Null;
    }
    if !raw["agents"].is_array() {
        raw["agents"] = json!([]);
    }
    if raw["origin"].is_null() {
        raw["origin"] = Value::Null;
    }
    if !raw["chatIds"].is_array() {
        raw["chatIds"] = raw["conversationId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map_or_else(|| json!([]), |s| json!([s]));
    }
    if !raw["contextTokens"].is_number() {
        if let Some(tokens) = raw.get("estimatedTokens").cloned() {
            raw["contextTokens"] = tokens;
        } else {
            raw.as_object_mut()?.remove("contextTokens");
        }
    }
    if !raw["lastCommittedResumeHandoffId"]
        .as_str()
        .is_some_and(|s| {
            (8..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        raw["lastCommittedResumeHandoffId"] = Value::Null;
    }
    Some(Checkpoint {
        history_seq,
        canonical_projection_current,
        token_estimate_current,
        outcome_counters_missing,
        activity_boundary_missing,
        summary: raw,
    })
}
