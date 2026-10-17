//! The upstream advisory formula: bounded segment context / 2 per distinct MCP call.
//! Recorded sends, estimates, observed limits and provider token receipts are separate.
use crate::{
    ReadOptions, SessionStore,
    model::{self, string},
};
use chrono::{Duration, Local, TimeZone};
use cos_core::persistence::{invalid, lock};
use regex::Regex;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    sync::{Mutex, OnceLock},
};
pub fn family(model: &str) -> Option<&'static str> {
    static RE: OnceLock<(Regex, Regex)> = OnceLock::new();
    let (a, b) = RE.get_or_init(|| {
        (
            Regex::new(r"^(?:gpt-?)?5[.-]6(?:-(?:instant|thinking|pro|sol|terra|luna))?$").unwrap(),
            Regex::new(r"^(?:gpt-?)?6(?:\.0)?(?:-(?:instant|thinking|pro|astra))?$").unwrap(),
        )
    });
    let id = cos_core::config::js_trim(model)
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    if a.is_match(&id) || id == "sol" {
        Some("gpt-5.6")
    } else if b.is_match(&id) || id == "astra" {
        Some("gpt-6")
    } else {
        None
    }
}
pub fn date_key(at: i64) -> String {
    Local
        .timestamp_millis_opt(at)
        .single()
        .map(|t| t.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "invalid".into())
}
fn legacy() -> Value {
    json!({"model":"gpt-5.6","reasoningEffort":"high","assumed":true})
}
fn attribution(event: &Value, previous: &Value) -> Value {
    let model = cos_core::config::js_trim(string(event, "model"));
    let effort = cos_core::config::js_trim(string(event, "reasoningEffort"));
    if !model.is_empty() {
        json!({"model":model,"reasoningEffort":if effort.is_empty(){if previous["assumed"]==false&&previous["model"]==model{previous["reasoningEffort"].clone()}else{Value::Null}}else{json!(effort)},"assumed":false})
    } else if !effort.is_empty() {
        let mut row = previous.clone();
        row["reasoningEffort"] = json!(effort);
        row
    } else {
        previous.clone()
    }
}
fn key(row: &Value) -> String {
    serde_json::to_string(&json!([
        row["model"],
        row["reasoningEffort"],
        row["assumed"]
    ]))
    .unwrap()
}
fn add(target: &mut BTreeMap<String, Value>, identity: &Value, tokens: f64) {
    let key = key(identity);
    let mut row = identity.clone();
    row["tokens"] = json!(
        target
            .get(&key)
            .and_then(|r| r["tokens"].as_f64())
            .unwrap_or(0.0)
            + tokens
    );
    target.insert(key, row);
}
pub fn overview(store: &SessionStore) -> io::Result<Value> {
    overview_at(store, 256000, chrono::Utc::now().timestamp_millis())
}
pub fn overview_at(store: &SessionStore, cap: u64, through: i64) -> io::Result<Value> {
    if ![256000, 400000].contains(&cap) {
        return Err(invalid("Unknown context comparison cap"));
    }
    let summaries = store.list()?;
    let mut models = BTreeMap::new();
    let mut days = BTreeMap::<String, BTreeMap<String, Value>>::new();
    let mut verified = HashMap::<String, Option<(i64, String)>>::new();
    for summary in &summaries {
        let mut context = 0_u64;
        let mut conversation = String::new();
        let mut selected = legacy();
        let mut calls = Vec::<(String, Value)>::new();
        let mut counted = HashSet::new();
        let mut unproven = None::<(String, i64)>;
        let finish = |context: &mut u64,
                      calls: &mut Vec<(String, Value)>,
                      selected: &mut Value,
                      days: &mut BTreeMap<String, BTreeMap<String, Value>>,
                      models: &mut BTreeMap<String, Value>| {
            let weight = (*context).min(cap) as f64 / 2.0;
            for (day, identity) in calls.drain(..) {
                add(days.entry(day).or_default(), &identity, weight);
                add(models, &identity, weight);
            }
            *context = 0;
            *selected = legacy();
        };
        for event in store.read(string(summary, "id"), ReadOptions::default())? {
            let kind = string(&event, "kind");
            if kind == "user_message"
                && !string(&event, "messageId").starts_with("input:")
                && event["inputDelivery"] != "offered"
            {
                unproven = None;
                let at = event["authoredAt"]
                    .as_i64()
                    .or(event["time"].as_i64())
                    .unwrap_or(0);
                if at > 0 {
                    if let Some(family) = family(string(&event, "model")) {
                        record_verified(&mut verified, string(&event, "messageId"), at, family);
                    } else if !event["model"].is_string() {
                        unproven = Some((string(&event, "messageId").into(), at));
                    }
                }
            } else if kind == "user_message" {
                unproven = None;
            }
            if kind == "assistant_message"
                && !string(&event, "resolvedModel").is_empty()
                && let Some((id, at)) = unproven.take()
                && let Some(family) = family(string(&event, "resolvedModel"))
            {
                record_verified(&mut verified, &id, at, family);
            }
            if kind == "tool_call" && event["call"]["nested"] == true {
                continue;
            }
            if kind == "session_start" {
                finish(
                    &mut context,
                    &mut calls,
                    &mut selected,
                    &mut days,
                    &mut models,
                );
                conversation = string(&event, "conversationId").into();
            }
            if kind == "tool_call" {
                if !counted.insert(string(&event["call"], "callId").to_owned()) {
                    continue;
                }
                let current = string(&event["call"], "conversationId");
                if current.is_empty() || !conversation.is_empty() && conversation != current {
                    finish(
                        &mut context,
                        &mut calls,
                        &mut selected,
                        &mut days,
                        &mut models,
                    );
                }
                conversation = current.into();
            }
            if kind == "user_message" && !string(&event, "messageId").starts_with("input:") {
                selected = legacy();
            }
            if kind != "user_message" || !string(&event, "messageId").starts_with("input:") {
                selected = attribution(&event, &selected);
            }
            context = context.saturating_add(model::event_tokens(&event));
            if kind == "tool_call" {
                calls.push((
                    date_key(event["time"].as_i64().unwrap_or(0)),
                    attribution(&event["call"], &selected),
                ));
                if conversation.is_empty() {
                    finish(
                        &mut context,
                        &mut calls,
                        &mut selected,
                        &mut days,
                        &mut models,
                    );
                }
            }
        }
        finish(
            &mut context,
            &mut calls,
            &mut selected,
            &mut days,
            &mut models,
        );
    }
    let today = Local
        .timestamp_millis_opt(through)
        .single()
        .ok_or_else(|| invalid("Invalid usage clock"))?;
    let earliest = today.date_naive() - Duration::days(6);
    let mut message_days = BTreeMap::<String, Value>::new();
    for (at, family) in verified.into_values().flatten() {
        if at > through {
            continue;
        }
        let Some(date) = Local
            .timestamp_millis_opt(at)
            .single()
            .map(|t| t.date_naive())
        else {
            continue;
        };
        if date < earliest {
            continue;
        }
        let day = message_days
            .entry(date.to_string())
            .or_insert_with(|| json!({"date":date.to_string(),"gpt56":0,"gpt6":0}));
        let field = if family == "gpt-6" { "gpt6" } else { "gpt56" };
        day[field] = json!(model::number(day, field) + 1);
    }
    let total = models
        .values()
        .map(|r| r["tokens"].as_f64().unwrap_or(0.0))
        .sum::<f64>();
    let days=days.into_iter().map(|(date,models)|json!({"date":date,"tokens":models.values().map(|r|r["tokens"].as_f64().unwrap_or(0.0)).sum::<f64>(),"models":models.into_values().collect::<Vec<_>>()})).collect::<Vec<_>>();
    Ok(
        json!({"estimated":true,"contextTokenCap":cap,"messages":{"through":through,"days":message_days.into_values().collect::<Vec<_>>()},"limits":[],"days":days,"models":models.into_values().collect::<Vec<_>>(),"tokens":total,"sessions":summaries.len()}),
    )
}
fn record_verified(
    verified: &mut HashMap<String, Option<(i64, String)>>,
    id: &str,
    time: i64,
    family: &str,
) {
    let next = Some((time, family.to_owned()));
    verified
        .entry(id.into())
        .and_modify(|prior| {
            if *prior != next {
                *prior = None;
            }
        })
        .or_insert(next);
}
pub struct ObservedLimits {
    rows: Mutex<(i64, Vec<Value>)>,
}
impl Default for ObservedLimits {
    fn default() -> Self {
        Self {
            rows: Mutex::new((0, vec![])),
        }
    }
}
impl ObservedLimits {
    pub fn observe(&self, raw: &Value, captured: i64, now: i64) -> io::Result<bool> {
        let rows = raw
            .as_array()
            .filter(|r| r.len() <= 80)
            .ok_or_else(|| invalid("Invalid provider limits"))?;
        for row in rows {
            if string(row, "model").is_empty()
                || string(row, "model").len() > 100
                || !matches!(
                    string(row, "scope"),
                    "model" | "feature" | "codex" | "shared"
                )
            {
                return Err(invalid("Invalid provider limit identity"));
            }
            for field in ["remaining", "remainingPercent", "resetAt", "windowSeconds"] {
                if !row[field].is_null()
                    && row[field].as_f64().is_none_or(|n| {
                        !n.is_finite() || n < 0.0 || field == "remainingPercent" && n > 100.0
                    })
                {
                    return Err(invalid("Invalid provider limit value"));
                }
            }
        }
        let mut state = lock(&self.rows)?;
        if captured > now + 5000 || now - captured > 600000 || captured < state.0 {
            return Ok(false);
        }
        state.0 = captured;
        state.1 = rows
            .iter()
            .map(|row| {
                let mut row = row.clone();
                if row["scope"] == "shared" {
                    row["scope"] = json!("codex");
                    if row["model"] == "Shared usage" {
                        row["model"] = json!("Codex");
                    }
                }
                row["observedAt"] = json!(captured);
                row
            })
            .collect();
        Ok(true)
    }
    pub fn current(&self, now: i64) -> io::Result<Vec<Value>> {
        Ok(lock(&self.rows)?
            .1
            .iter()
            .filter(|row| {
                now - row["observedAt"].as_i64().unwrap_or(0) <= 600000
                    && row["resetAt"].as_i64().is_none_or(|reset| reset > now)
            })
            .cloned()
            .collect())
    }
}
