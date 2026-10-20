//! Exact recordings retain opaque identifiers; diagnostics mask opaque tokens too.
use regex::Regex;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};
pub fn redact_credentials(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?-u:\b)sk-(?:or-v1-|proj-|svcacct-)?[A-Za-z0-9_-]{20,}(?-u:\b)").unwrap()
    })
    .replace_all(text, "[redacted]")
    .into_owned()
}
pub fn redact(text: &str) -> String {
    static PATTERNS: OnceLock<Vec<(Regex, &str)>> = OnceLock::new();
    let mut out = text.to_owned();
    for (pattern, replacement) in PATTERNS.get_or_init(|| {
        vec![
            (r"(?-u:\b)sk-[A-Za-z0-9_-]{8,}", "sk-***"),
            (
                r"(?-u:\b)ey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
                "***jwt***",
            ),
            (r"(?-u:\b)[A-Za-z0-9_-]{40,}(?-u:\b)", "***"),
        ]
        .into_iter()
        .map(|(p, r)| (Regex::new(p).unwrap(), r))
        .collect()
    }) {
        out = pattern.replace_all(&out, *replacement).into_owned();
    }
    out
}
pub struct DiagnosticLog {
    capacity: usize,
    entries: Mutex<VecDeque<Value>>,
}
impl DiagnosticLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.clamp(1, 10000),
            entries: Mutex::new(VecDeque::new()),
        }
    }
    pub fn push(&self, level: &str, message: &str) {
        // Reject an oversized diagnostic in full, not a prefix of a credential.
        const MAX_INPUT_BYTES: usize = 1024 * 1024;
        let level = match level {
            "trace" | "debug" | "info" | "warn" | "error" => level,
            _ => "unknown",
        };
        let clean = if message.len() > MAX_INPUT_BYTES {
            "[diagnostic omitted: size limit]".to_owned()
        } else {
            redact(message)
        };
        let bounded: String = clean.chars().take(16000).collect();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        entries.push_back(
            json!({"time":chrono::Utc::now().timestamp_millis(),"level":level,"message":bounded}),
        );
        while entries.len() > self.capacity {
            entries.pop_front();
        }
    }
    pub fn snapshot(&self) -> Vec<Value> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }
    pub fn export(&self) -> String {
        serde_json::to_string_pretty(&self.snapshot()).expect("diagnostic JSON")
    }
}
