//! Bounded, read-only parsers for provider-owned logs. No credential lookup or network.
use crate::usage::date_key;
use chrono::{DateTime, Duration, Local, TimeZone};
use cos_core::persistence::{invalid, is_link, owned_path};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
};
fn n(value: &Value) -> f64 {
    value
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(0.0)
}
fn text(value: &Value) -> Value {
    value
        .as_str()
        .map(cos_core::config::js_trim)
        .filter(|s| !s.is_empty())
        .map(|s| json!(s.chars().take(80).collect::<String>()))
        .unwrap_or(Value::Null)
}
fn parse(line: &str) -> Option<Value> {
    if !line.starts_with('{') {
        return None;
    }
    serde_json::from_str::<Value>(line)
        .ok()
        .filter(Value::is_object)
}
fn at(value: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(value["timestamp"].as_str()?)
        .ok()
        .map(|t| t.timestamp_millis())
}
#[derive(Default)]
pub struct CodexFile {
    last: Option<Value>,
    days: BTreeMap<String, Value>,
    limits: BTreeMap<String, Value>,
}
impl CodexFile {
    pub fn fold(&mut self, line: &str) {
        let Some(event) = parse(line) else {
            return;
        };
        if event["type"] != "event_msg" || event["payload"]["type"] != "token_count" {
            return;
        }
        let Some(at) = at(&event) else {
            return;
        };
        let limits = &event["payload"]["rate_limits"];
        if let Some(id) = text(&limits["limit_id"]).as_str() {
            let mut windows = Vec::new();
            for field in ["primary", "secondary"] {
                let w = &limits[field];
                if w["used_percent"].is_number() && w["window_minutes"].is_number() {
                    windows.push(json!({"windowMinutes":n(&w["window_minutes"]),"usedPercent":n(&w["used_percent"]).min(100.0),"resetsAt":w["resets_at"].as_f64().filter(|n|*n>0.0).map(|n|n*1000.0)}));
                }
            }
            let credits = &limits["credits"];
            let row = json!({"id":id,"plan":text(&limits["plan_type"]),"reached":text(&limits["rate_limit_reached_type"]),"observedAt":at,"windows":windows,"credits":if credits.is_object(){json!({"hasCredits":credits["has_credits"]==true,"unlimited":credits["unlimited"]==true,"balance":if credits["balance"].is_string()||credits["balance"].is_number(){credits["balance"].clone()}else{Value::Null}})}else{Value::Null}});
            if self
                .limits
                .get(id)
                .is_none_or(|old| old["observedAt"].as_i64().unwrap_or(0) <= at)
            {
                self.limits.insert(id.into(), row);
            }
        }
        let raw = &event["payload"]["info"]["total_token_usage"];
        if !raw.is_object() {
            return;
        }
        let total = json!({"input":n(&raw["input_tokens"]),"cachedInput":n(&raw["cached_input_tokens"]),"output":n(&raw["output_tokens"])});
        let previous = self.last.as_ref().cloned().unwrap_or_else(|| json!({}));
        let mut delta = json!({});
        let mut grew = false;
        for field in ["input", "cachedInput", "output"] {
            let value = n(&total[field]);
            let before = n(&previous[field]);
            let increase = if value >= before {
                value - before
            } else {
                value
            };
            delta[field] = json!(increase);
            grew |= increase != 0.0;
        }
        self.last = Some(total);
        if !grew {
            return;
        }
        let day = self
            .days
            .entry(date_key(at))
            .or_insert_with(|| json!({"input":0,"cachedInput":0,"output":0}));
        for field in ["input", "cachedInput", "output"] {
            day[field] = json!(n(&day[field]) + n(&delta[field]));
        }
    }
    pub fn snapshot(&self) -> Value {
        crate::model::javascript_numbers(
            json!({"days":self.days,"limits":self.limits.values().collect::<Vec<_>>()}),
        )
    }
}
#[derive(Clone, Default)]
pub struct ClaudeFile {
    messages: BTreeMap<String, Value>,
}
impl ClaudeFile {
    pub fn fold(&mut self, line: &str) {
        let Some(event) = parse(line) else {
            return;
        };
        if event["type"] != "assistant" {
            return;
        }
        let message = &event["message"];
        let usage = &message["usage"];
        let id = text(&message["id"]);
        let Some(id) = id.as_str() else {
            return;
        };
        let Some(at) = at(&event) else {
            return;
        };
        if !usage.is_object() || self.messages.contains_key(id) {
            return;
        }
        let tokens = json!({"input":n(&usage["input_tokens"]),"cacheWrite":n(&usage["cache_creation_input_tokens"]),"cacheRead":n(&usage["cache_read_input_tokens"]),"output":n(&usage["output_tokens"])});
        if tokens.as_object().unwrap().values().all(|v| n(v) == 0.0) {
            return;
        }
        self.messages.insert(id.into(),json!({"day":date_key(at),"model":text(&message["model"]).as_str().unwrap_or("unknown"),"tokens":tokens}));
    }
    pub fn totals(states: &[Self]) -> Value {
        let mut seen = HashSet::new();
        let mut days = json!({});
        for state in states {
            for (id, message) in &state.messages {
                if !seen.insert(id) {
                    continue;
                }
                let day = message["day"].as_str().unwrap();
                let model = message["model"].as_str().unwrap();
                if !days[day].is_object() {
                    days[day] = json!({});
                }
                if !days[day][model].is_object() {
                    days[day][model] = json!({"input":0,"cacheWrite":0,"cacheRead":0,"output":0});
                }
                for field in ["input", "cacheWrite", "cacheRead", "output"] {
                    days[day][model][field] =
                        json!(n(&days[day][model][field]) + n(&message["tokens"][field]));
                }
            }
        }
        crate::model::javascript_numbers(days)
    }
}
pub struct ProviderUsage;
impl ProviderUsage {
    pub fn read(enabled: bool, now: i64, codex: &Path, claude: &Path) -> io::Result<Value> {
        Self::read_with_budget(enabled, now, codex, claude, 16 * 1024 * 1024)
    }
    pub fn read_with_budget(
        enabled: bool,
        now: i64,
        codex: &Path,
        claude: &Path,
        budget: u64,
    ) -> io::Result<Value> {
        if !enabled {
            return Ok(json!({"enabled":false,"readAt":now,"codex":null,"claude":null}));
        }
        let today = Local
            .timestamp_millis_opt(now)
            .single()
            .ok_or_else(|| invalid("Invalid provider usage clock"))?
            .date_naive();
        let days: Vec<_> = (0..7)
            .rev()
            .map(|back| (today - Duration::days(back)).to_string())
            .collect();
        let since = Local
            .from_local_datetime(&(today - Duration::days(6)).and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .ok_or_else(|| invalid("Invalid provider local date"))?
            .timestamp_millis();
        let mut remaining = budget.min(512 * 1024 * 1024);
        let admitted = remaining;
        let mut limited = false;
        let mut codex_days = BTreeMap::<String, Value>::new();
        let mut limits = BTreeMap::<String, Value>::new();
        let codex_files = candidates(codex, "sessions", 4, since)?;
        for path in &codex_files {
            if remaining == 0 {
                limited = true;
                break;
            }
            let mut state = CodexFile::default();
            limited |= lines(path, &mut remaining, |line| state.fold(line))?;
            for (day, tokens) in state.days {
                let target = codex_days
                    .entry(day)
                    .or_insert_with(|| json!({"input":0,"cachedInput":0,"output":0}));
                for field in ["input", "cachedInput", "output"] {
                    target[field] = json!(n(&target[field]) + n(&tokens[field]));
                }
            }
            for (id, row) in state.limits {
                if limits.get(&id).is_none_or(|old| {
                    old["observedAt"].as_i64().unwrap_or(0)
                        < row["observedAt"].as_i64().unwrap_or(0)
                }) {
                    limits.insert(id, row);
                }
            }
        }
        let mut claude_states = Vec::new();
        let claude_files = candidates(claude, "projects", 4, since)?;
        for path in &claude_files {
            if remaining == 0 {
                limited = true;
                break;
            }
            let mut state = ClaudeFile::default();
            limited |= lines(path, &mut remaining, |line| state.fold(line))?;
            claude_states.push(state);
        }
        let totals = ClaudeFile::totals(&claude_states);
        let mut limits = limits.into_values().collect::<Vec<_>>();
        limits.sort_by(|a, b| {
            let rank = |r: &Value| if r["id"] == "codex" { 0 } else { 1 };
            rank(a)
                .cmp(&rank(b))
                .then_with(|| b["observedAt"].as_i64().cmp(&a["observedAt"].as_i64()))
        });
        let codex_value = if codex.join("sessions").is_dir() {
            json!({"limits":limits,"days":days.iter().map(|day|{let mut row=codex_days.get(day).cloned().unwrap_or_else(||json!({"input":0,"cachedInput":0,"output":0}));row["date"]=json!(day);row}).collect::<Vec<_>>()})
        } else {
            Value::Null
        };
        let claude_value = if claude.join("projects").is_dir() {
            json!({"days":days.iter().map(|day|json!({"date":day,"models":totals[day].as_object().map(|models|models.iter().map(|(model,tokens)|{let mut row=tokens.clone();row["model"]=json!(model);row}).collect::<Vec<_>>()).unwrap_or_default()})).collect::<Vec<_>>()})
        } else {
            Value::Null
        };
        let mut reply = json!({"enabled":true,"readAt":now,"codex":codex_value,"claude":claude_value,"scannedBytes":admitted-remaining,"scanBudgetBytes":admitted});
        if limited {
            reply["limited"] = json!(true);
        }
        Ok(crate::model::javascript_numbers(reply))
    }
}
fn candidates(home: &Path, relative: &str, depth: usize, since: i64) -> io::Result<Vec<PathBuf>> {
    if !home.is_dir() {
        return Ok(vec![]);
    }
    let canonical_home = fs::canonicalize(home)?;
    let home = canonical_home.as_path();
    let root = owned_path(home, Path::new(relative), true)?;
    let mut files = Vec::new();
    fn walk(
        root: &Path,
        directory: &Path,
        depth: usize,
        files: &mut Vec<PathBuf>,
        since: i64,
        entry_budget: &mut usize,
    ) -> io::Result<()> {
        let entries = match fs::read_dir(directory) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        for entry in entries {
            if *entry_budget == 0 {
                return Err(invalid("Provider inventory bound exceeded"));
            }
            *entry_budget -= 1;
            let entry = entry?;
            let meta = fs::symlink_metadata(entry.path())?;
            if is_link(&meta) {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| invalid("Provider path escaped its root"))?
                .to_owned();
            let path = owned_path(root, &relative, false)?;
            if meta.is_dir() && depth > 0 {
                walk(root, &path, depth - 1, files, since, entry_budget)?;
            } else if meta.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
                let modified = meta
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|t| t.as_millis() as i64)
                    .unwrap_or(0);
                if modified >= since {
                    files.push(path);
                }
                if files.len() > 100000 {
                    return Err(invalid("Provider log file bound exceeded"));
                }
            }
        }
        Ok(())
    }
    if root.is_dir() {
        walk(home, &root, depth, &mut files, since, &mut 200000)?;
    }
    files.sort();
    Ok(files)
}
fn lines(path: &Path, remaining: &mut u64, mut fold: impl FnMut(&str)) -> io::Result<bool> {
    let file = File::open(path)?;
    let limited = file.metadata()?.len() > *remaining;
    let mut reader = BufReader::with_capacity(64 * 1024, file.take(*remaining));
    let mut line = Vec::new();
    let mut skip = false;
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        let count = buffer
            .iter()
            .position(|b| *b == b'\n')
            .map(|at| at + 1)
            .unwrap_or(buffer.len());
        let end = buffer[count - 1] == b'\n';
        if !skip {
            if line.len() + count > 4 * 1024 * 1024 {
                skip = true;
                line.clear();
            } else {
                line.extend_from_slice(&buffer[..count]);
            }
        }
        if end {
            if !skip && let Ok(text) = std::str::from_utf8(&line) {
                fold(text);
            }
            line.clear();
            skip = false;
        }
        reader.consume(count);
        *remaining = remaining.saturating_sub(count as u64);
    }
    Ok(limited)
}
