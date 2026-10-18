//! Bounded search projections and explicit complete exports; no pruning.
use crate::{
    ReadOptions, SessionStore,
    model::{position, string},
};
use serde_json::{Value, json};
use std::{collections::HashSet, io};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
pub fn fold_case(text: &str) -> String {
    text.chars()
        .map(|c| {
            let original = c.to_string();
            let lower = c.to_lowercase().collect::<String>();
            let base = if lower.encode_utf16().count() == original.encode_utf16().count() {
                lower
            } else {
                original.clone()
            };
            let bare = base
                .nfd()
                .filter(|c| !is_combining_mark(*c))
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if bare.encode_utf16().count() == original.encode_utf16().count() {
                bare
            } else {
                base
            }
        })
        .collect()
}
pub fn query_terms(query: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    fold_case(query)
        .split_whitespace()
        .filter(|term| seen.insert((*term).to_owned()))
        .take(12)
        .map(String::from)
        .collect()
}
fn ranges(text: &str, terms: &[String]) -> Vec<(usize, usize)> {
    let mut matches = Vec::new();
    for term in terms {
        for (at, _) in text.match_indices(term) {
            let start = text[..at].encode_utf16().count();
            matches.push((start, start + term.encode_utf16().count()));
        }
    }
    matches.sort_unstable();
    matches
}
fn original_byte(original: &str, folded: &str, byte: usize) -> usize {
    let wanted = folded[..byte].encode_utf16().count();
    let mut units = 0;
    for (at, c) in original.char_indices() {
        if units == wanted {
            return at;
        }
        units += c.len_utf16();
    }
    original.len()
}
impl SessionStore {
    pub fn search(&self, query: &str, limit: usize) -> io::Result<Value> {
        let terms = query_terms(query);
        let summaries: Vec<_> = self
            .list()?
            .into_iter()
            .filter(|s| s["origin"]["kind"] != "helper")
            .collect();
        let total = summaries.len();
        let mut title_results = Vec::new();
        let mut body_results = Vec::new();
        if terms.is_empty() {
            return Ok(json!({"results":[],"indexed":total,"total":total}));
        }
        for summary in summaries {
            let id = string(&summary, "id");
            let title = fold_case(string(&summary, "title"));
            let mut result = json!({"id":id,"title":summary["title"],"projectId":summary.get("projectId").unwrap_or(&Value::Null)});
            let title_matches = ranges(&title, &terms);
            if !title_matches.is_empty() {
                result["titleMatches"] = json!(title_matches);
            }
            if terms.iter().all(|term| title.contains(term)) {
                title_results.push(result);
                continue;
            }
            let mut body = String::new();
            for event in self.read(
                id,
                ReadOptions {
                    kinds: vec!["user_message".into(), "assistant_message".into()],
                    ..Default::default()
                },
            )? {
                let text = self.read_full_text(id, &event["message"])?;
                let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !clean.is_empty() {
                    body.push_str(&clean);
                    body.push('\n');
                }
                if body.encode_utf16().count() >= 1_000_000 {
                    body = body
                        .chars()
                        .scan(0, |count, c| {
                            *count += c.len_utf16();
                            (*count <= 1_000_000).then_some(c)
                        })
                        .collect();
                    break;
                }
            }
            let lower = fold_case(&body);
            if terms
                .iter()
                .all(|term| title.contains(term) || lower.contains(term))
            {
                let first = terms
                    .iter()
                    .filter_map(|term| lower.find(term))
                    .min()
                    .unwrap_or(0);
                let line_start = lower[..first].rfind('\n').map(|n| n + 1).unwrap_or(0);
                let line_end = lower[first..]
                    .find('\n')
                    .map(|n| first + n)
                    .unwrap_or(lower.len());
                let line = &body[original_byte(&body, &lower, line_start)
                    ..original_byte(&body, &lower, line_end)];
                let at = lower[line_start..first].chars().count();
                let count = line.chars().count();
                let end = (at.saturating_sub(46) + 140).min(count);
                let start = end.saturating_sub(140);
                let text = line
                    .chars()
                    .skip(start)
                    .take(end - start)
                    .collect::<String>();
                let prefix = if start > 0 { "…" } else { "" };
                let suffix = if end < count { "…" } else { "" };
                let snippet = format!("{prefix}{text}{suffix}");
                result["snippet"] =
                    json!({"text":snippet,"matches":ranges(&fold_case(&snippet),&terms)});
                body_results.push(result);
            }
        }
        title_results.extend(body_results);
        let limited = title_results.len() > limit.min(50);
        title_results.truncate(limit.min(50));
        let mut reply = json!({"results":title_results,"indexed":total,"total":total});
        if limited {
            reply["limited"] = json!(true);
        }
        Ok(reply)
    }
    pub fn find_text(&self, id: &str, query: &str) -> io::Result<Vec<Value>> {
        let terms = query_terms(query);
        if terms.is_empty() {
            return Ok(vec![]);
        }
        let mut found = Vec::new();
        for event in self.read(
            id,
            ReadOptions {
                kinds: vec!["user_message".into(), "assistant_message".into()],
                ..Default::default()
            },
        )? {
            let text = self.read_full_text(id, &event["message"])?;
            let lower = fold_case(&text);
            if terms.iter().all(|term| lower.contains(term)) {
                let at = terms
                    .iter()
                    .filter_map(|term| lower.find(term))
                    .min()
                    .unwrap_or(0);
                found.push(json!({"seq":event["seq"],"kind":event["kind"],"messageId":event.get("messageId").unwrap_or(&Value::Null),"position":lower[..at].encode_utf16().count(),"origin":position(&event)}));
                if found.len() == 500 {
                    break;
                }
            }
        }
        Ok(found)
    }
    pub fn export_json(&self, id: &str) -> io::Result<Value> {
        let summary = self
            .summary(id)?
            .ok_or_else(|| cos_core::persistence::invalid("Unknown session"))?;
        let mut events = self.read(id, ReadOptions::default())?;
        for event in &mut events {
            if event["message"].is_object() {
                let text = self.read_full_text(id, &event["message"])?;
                event["hydratedText"] = json!(text);
            }
            if event["kind"] == "tool_call" {
                for field in ["args", "result"] {
                    let text = self.read_full_text(id, &event["call"][field])?;
                    event["call"][format!("{field}Text")] = json!(text);
                }
            }
        }
        Ok(
            json!({"format":"cos-rust-history-v1","summary":summary,"events":events,"sourceCommit":cos_core::UPSTREAM_COMMIT}),
        )
    }
    fn markdown_entry_text(&self, id: &str, entry: &Value) -> io::Result<String> {
        let stored = &entry["stored"];
        let mut text = string(stored, "text").to_owned();
        if stored["truncated"] == true
            && let Some(asset) = stored["assetId"].as_str()
            && crate::content::re!(r"^[0-9a-f]{8,64}\.(png|jpg|txt|bin)$").is_match(asset)
            && let Some(bytes) = self.read_asset(id, asset, None)?
        {
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
        let completed = crate::dil::completed_dil_text(&text, entry.get("html"))?;
        Ok(crate::content::provider_entities_as_markdown(&completed))
    }
    pub fn export_markdown(&self, id: &str) -> io::Result<String> {
        let summary = self
            .summary(id)?
            .ok_or_else(|| cos_core::persistence::invalid("Session not found"))?;
        let events = self.read(id, ReadOptions::default())?;
        let mut entries = crate::markdown::transcript_entries(&events);
        for entry in &mut entries {
            entry["text"] = json!(self.markdown_entry_text(id, entry)?);
        }
        Ok(crate::markdown::session_markdown(
            string(&summary, "title"),
            &entries,
            "You",
            "ChatGPT",
        ))
    }
    pub fn export_answer_markdown(&self, id: &str, turn_id: &str) -> io::Result<String> {
        self.summary(id)?
            .ok_or_else(|| cos_core::persistence::invalid("Session not found"))?;
        let events = self.read(id, ReadOptions::default())?;
        if turn_id.is_empty()
            || !crate::markdown::completed_turn_ids(&events)
                .iter()
                .any(|id| id == turn_id)
        {
            return Err(cos_core::persistence::invalid(
                "This turn has not completed",
            ));
        }
        let entries = crate::markdown::turn_answer_entries(&events, turn_id);
        if entries.is_empty() {
            return Err(cos_core::persistence::invalid(
                "This turn has no answer to export",
            ));
        }
        let texts = entries
            .iter()
            .map(|entry| self.markdown_entry_text(id, entry))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(crate::markdown::answer_markdown(&texts))
    }
}
