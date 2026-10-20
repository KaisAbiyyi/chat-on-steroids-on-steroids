//! One mutation fence per session; only durable records advance identities/cursors.
use crate::{
    chronology,
    model::{self, event_tokens, hash, message_key, number, position, residence, string, terminal},
    summary, title,
};
use cos_core::persistence::{atomic_write, invalid, is_link, lock, owned_path};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
pub const MAX_LINE_BYTES: usize = 512 * 1024;
/// JSON/JS readers cannot round-trip integers beyond Number.MAX_SAFE_INTEGER.
pub const MAX_HISTORY_SEQUENCE: u64 = 9_007_199_254_740_991;
/// Fail closed on implausibly large journals rather than scanning unbounded files.
pub const MAX_HISTORY_JOURNAL_BYTES: u64 = 1024 * 1024 * 1024;
/// Guard canonical in-memory overlays against unbounded shard proliferation.
pub const MAX_CANONICAL_MESSAGES: usize = 100_000;
pub const MAX_CANONICAL_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_HISTORY_READ_EVENTS: usize = 100_000;
fn budget_canonical_bytes(total: &mut usize, previous: usize, next: usize) -> io::Result<()> {
    *total = total
        .saturating_sub(previous)
        .checked_add(next)
        .filter(|bytes| *bytes <= MAX_CANONICAL_BYTES)
        .ok_or_else(|| invalid("Canonical history exceeds in-memory byte budget"))?;
    Ok(())
}
fn next_sequence(next: u64) -> io::Result<u64> {
    if (1..=MAX_HISTORY_SEQUENCE).contains(&next) {
        Ok(next)
    } else {
        Err(invalid("History sequence exhausted its safe integer range"))
    }
}
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
pub const MAX_ASSET_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SESSION_ASSETS: u64 = 192 * 1024 * 1024;
#[derive(Default)]
pub struct CreateOptions {
    pub id: Option<String>,
    pub title: Option<String>,
    pub title_source: Option<String>,
    pub conversation_id: Option<String>,
    pub origin: Option<Value>,
    pub now: Option<i64>,
}
#[derive(Clone)]
pub struct ReadOptions {
    pub from: Option<u64>,
    pub limit: usize,
    pub kinds: Vec<String>,
    pub agent: Option<String>,
}
impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            from: None,
            limit: usize::MAX,
            kinds: vec![],
            agent: None,
        }
    }
}
#[derive(Default)]
pub struct UpsertOptions {
    pub prefer_time: bool,
    pub work: Option<bool>,
    pub now: Option<i64>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertResult {
    pub event: Value,
    pub changed: bool,
    pub content_changed: bool,
}
struct Entry {
    summary: Value,
    messages: BTreeMap<String, Value>,
    canonical_bytes: usize,
    next: u64,
    dirty: bool,
}
fn admitted_canonical_bytes(entry: &Entry, key: &str, event: &Value) -> io::Result<usize> {
    if !entry.messages.contains_key(key) && entry.messages.len() >= MAX_CANONICAL_MESSAGES {
        return Err(invalid("Canonical history exceeds in-memory record budget"));
    }
    let previous = entry
        .messages
        .get(key)
        .map(serde_json::to_vec)
        .transpose()?
        .map_or(0, |bytes| bytes.len());
    let next = serde_json::to_vec(event)?.len();
    let mut budget = entry.canonical_bytes;
    budget_canonical_bytes(&mut budget, previous, next)?;
    Ok(budget)
}
struct Assets {
    limit: u64,
    retired: HashSet<String>,
}
struct Inner {
    directory: PathBuf,
    entries: Mutex<HashMap<String, Arc<Mutex<Entry>>>>,
    catalog: Mutex<()>,
    assets: Mutex<Assets>,
}
#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Inner>,
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
fn read_json(path: &Path, limit: u64) -> io::Result<Value> {
    let mut file = File::open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > limit {
        return Err(invalid("History record exceeds its read bound"));
    }
    let mut bytes = Vec::new();
    (&mut file).take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("History record grew beyond its read bound"));
    }
    if summary::requires_lossless_strings_bytes(&bytes) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "History JSON requires lossless unpaired UTF-16 strings; original file retained",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unreadable history JSON"))
}
fn read_checkpoint(path: &Path, id: &str) -> io::Result<Option<summary::Checkpoint>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::IsADirectory,
            "Session metadata is not a regular file",
        ));
    }
    const LIMIT: u64 = 8 * 1024 * 1024;
    if metadata.len() > LIMIT {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    (&mut file).take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    if summary::requires_lossless_strings(&text) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Session metadata requires lossless unpaired UTF-16 JSON strings; original files retained",
        ));
    }
    Ok(serde_json::from_str(&text)
        .ok()
        .and_then(|raw| summary::normalize(id, raw)))
}
fn scan(path: &Path, mut accept: impl FnMut(Value)) -> io::Result<usize> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(invalid("Journal is not a regular file"));
    }
    if metadata.len() > MAX_HISTORY_JOURNAL_BYTES {
        return Err(invalid(
            "History journal exceeds replay budget; original bytes preserved",
        ));
    }
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut carry = Vec::new();
    let mut skipping = false;
    let mut damaged = 0;
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            if skipping {
                damaged += 1;
            } else if !carry.is_empty() {
                accept_line(&carry, &mut accept, &mut damaged)?;
            }
            break;
        }
        let consumed = buffer
            .iter()
            .position(|b| *b == b'\n')
            .map(|n| n + 1)
            .unwrap_or(buffer.len());
        let newline = buffer[consumed - 1] == b'\n';
        let part = &buffer[..consumed - usize::from(newline)];
        if !skipping {
            if carry.len() + part.len() > MAX_LINE_BYTES {
                skipping = true;
                carry.clear();
            } else {
                carry.extend_from_slice(part);
            }
        }
        if newline {
            if skipping {
                damaged += 1;
            } else {
                accept_line(&carry, &mut accept, &mut damaged)?;
            }
            carry.clear();
            skipping = false;
        }
        reader.consume(consumed);
    }
    Ok(damaged)
}
fn accept_line(raw: &[u8], accept: &mut impl FnMut(Value), damaged: &mut usize) -> io::Result<()> {
    if raw.iter().all(u8::is_ascii_whitespace) {
        return Ok(());
    }
    if summary::requires_lossless_strings_bytes(raw) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "History journal requires lossless unpaired UTF-16 strings; original bytes retained",
        ));
    }
    match serde_json::from_slice::<Value>(raw) {
        Ok(v) if v["seq"].as_u64().is_some_and(|n| n > MAX_HISTORY_SEQUENCE) => Err(invalid(
            "History journal contains an unsafe sequence; original bytes preserved",
        )),
        Ok(v) if v["seq"].as_u64().is_some_and(|n| n > 0) && v["kind"].is_string() => {
            accept(v);
            Ok(())
        }
        _ => {
            *damaged += 1;
            Ok(())
        }
    }
}
impl SessionStore {
    pub fn open(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let directory = fs::canonicalize(directory)?;
        fs::create_dir_all(owned_path(&directory, Path::new("sessions"), true)?)?;
        Ok(Self {
            inner: Arc::new(Inner {
                directory,
                entries: Mutex::new(HashMap::new()),
                catalog: Mutex::new(()),
                assets: Mutex::new(Assets {
                    limit: 2 * 1024 * 1024 * 1024,
                    retired: HashSet::new(),
                }),
            }),
        })
    }
    pub fn directory(&self) -> &Path {
        &self.inner.directory
    }
    fn path(&self, id: &str, relative: &str, missing: bool) -> io::Result<PathBuf> {
        if !model::session_id(id) {
            return Err(invalid("Invalid session id"));
        }
        owned_path(
            &self.inner.directory,
            Path::new(&format!("sessions/{id}/{relative}")),
            missing,
        )
    }
    fn canonical(&self, id: &str) -> io::Result<(BTreeMap<String, Value>, bool)> {
        let mut messages = BTreeMap::new();
        let mut canonical_bytes = 0usize;
        let mut aliases_collapsed = false;
        match self
            .path(id, "messages.json", true)
            .and_then(|p| read_json(&p, 256 * 1024 * 1024))
        {
            Ok(raw) => {
                if let Some(map) = raw.as_object() {
                    for (key, event) in map {
                        if message_key(event).as_ref() == Some(key) && event["seq"].is_u64() {
                            budget_canonical_bytes(
                                &mut canonical_bytes,
                                0,
                                serde_json::to_vec(event)?.len(),
                            )?;
                            messages.insert(key.clone(), event.clone());
                        }
                    }
                }
                if messages.len() > MAX_CANONICAL_MESSAGES {
                    return Err(invalid("Canonical history exceeds in-memory record budget"));
                }
            }
            // Malformed legacy maps may be reconstructed from durable shards.
            // Resource-budget violations must not silently drop a map instead.
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::PermissionDenied
                        | io::ErrorKind::InvalidInput
                        | io::ErrorKind::Unsupported
                ) =>
            {
                return Err(e);
            }
            Err(_) => {}
        }
        let shards = self.path(id, "messages", true)?;
        let entries = match fs::read_dir(shards) {
            Ok(e) => Some(e),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        if let Some(entries) = entries {
            for entry in entries {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.len() != 69
                    || !name.ends_with(".json")
                    || !name
                        .get(..64)
                        .unwrap_or("")
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    continue;
                }
                let path = self.path(id, &format!("messages/{name}"), false)?;
                let event = match read_json(&path, MAX_MESSAGE_BYTES as u64) {
                    Ok(event) => event,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::InvalidInput | io::ErrorKind::Unsupported
                        ) =>
                    {
                        return Err(error);
                    }
                    Err(_) => continue,
                };
                if let Some(key) = message_key(&event)
                    && name == format!("{}.json", hash(key.as_bytes()))
                    && event["seq"].is_u64()
                {
                    if messages.len() >= MAX_CANONICAL_MESSAGES && !messages.contains_key(&key) {
                        return Err(invalid("Canonical history exceeds in-memory record budget"));
                    }
                    let next_bytes = serde_json::to_vec(&event)?.len();
                    let previous_bytes = messages
                        .get(&key)
                        .map(serde_json::to_vec)
                        .transpose()?
                        .map_or(0, |bytes| bytes.len());
                    budget_canonical_bytes(&mut canonical_bytes, previous_bytes, next_bytes)?;
                    messages.insert(key, event);
                }
            }
        }
        let mut providers = HashMap::<String, Vec<(String, Value)>>::new();
        for (key, event) in &messages {
            if event["kind"] == "assistant_message"
                && !string(event, "providerMessageId").is_empty()
            {
                providers
                    .entry(string(event, "providerMessageId").into())
                    .or_default()
                    .push((key.clone(), event.clone()));
            }
        }
        for mut group in providers.into_values().filter(|g| g.len() > 1) {
            aliases_collapsed = true;
            group.sort_by_key(|(_, e)| position(e));
            let (first_key, first) = group[0].clone();
            let mut latest = first.clone();
            let mut seq = number(&first, "seq");
            for (key, event) in &group {
                seq = seq.max(number(event, "seq"));
                if terminal(&latest) != terminal(event) {
                    if terminal(event) {
                        latest = event.clone();
                    }
                } else if number(event, "seq") > number(&latest, "seq") {
                    latest = event.clone();
                }
                messages.remove(key);
            }
            latest["messageId"] = first["messageId"].clone();
            latest["origin"] = json!(position(&first));
            latest["time"] = first["time"].clone();
            latest["seq"] = json!(seq);
            if let Some(turn) = group.iter().find_map(|(_, e)| e.get("turnId")) {
                latest["turnId"] = turn.clone();
            }
            if group.iter().any(|(_, e)| e["goalEligible"] == true) {
                latest["goalEligible"] = json!(true);
            }
            messages.insert(first_key, latest);
        }
        // A corrupt shard/map with an unsafe JSON sequence must not drive a
        // replay cursor beyond the range external JS consumers can represent.
        for event in messages.values() {
            if event["seq"]
                .as_u64()
                .is_some_and(|seq| seq > MAX_HISTORY_SEQUENCE)
            {
                return Err(invalid("Canonical message has an unsafe history sequence"));
            }
        }
        Ok((messages, aliases_collapsed))
    }
    fn load(&self, id: &str) -> io::Result<Entry> {
        let meta = self.path(id, "meta.json", true)?;
        let checkpoint = match read_checkpoint(&meta, id) {
            Ok(Some(checkpoint)) => Some(checkpoint),
            Err(error) if error.kind() == io::ErrorKind::Unsupported => return Err(error),
            primary => match read_checkpoint(&self.path(id, "meta.backup.json", true)?, id) {
                Ok(Some(checkpoint)) => Some(checkpoint),
                backup => match (primary, backup) {
                    (Err(error), _) | (_, Err(error)) => return Err(error),
                    _ => None,
                },
            },
        };
        let (messages, aliases_collapsed) = self.canonical(id)?;
        let mut canonical_bytes = 0usize;
        for event in messages.values() {
            budget_canonical_bytes(&mut canonical_bytes, 0, serde_json::to_vec(event)?.len())?;
        }
        let mut max_seq = messages
            .values()
            .map(|v| number(v, "seq"))
            .max()
            .unwrap_or(0);
        let mut summary = model::empty_summary(id, "Recovered session", None, now());
        let mut saw = false;
        let mut identities = Vec::new();
        let mut historical_return_reduction = 0;
        scan(&self.path(id, "events.jsonl", true)?, |event| {
            max_seq = max_seq.max(number(&event, "seq"));
            if message_key(&event).is_some_and(|key| messages.contains_key(&key)) {
                return;
            }
            if checkpoint.as_ref().is_some_and(|c| {
                !c.token_estimate_current
                    && !string(&c.summary, "conversationId").is_empty()
                    && !string(&event["call"], "conversationId").is_empty()
                    && c.summary["conversationId"] != event["call"]["conversationId"]
            }) && event["kind"] == "tool_call"
            {
                historical_return_reduction +=
                    model::stored_tokens(&event["call"]["result"]).saturating_sub(10000);
            }
            if !saw {
                summary["startedAt"] = event["time"].clone();
                summary["updatedAt"] = event["time"].clone();
                saw = true;
            }
            if event["kind"] == "session_start" && !string(&event, "title").is_empty() {
                summary["title"] = event["title"].clone();
            }
            if event["conversationId"].is_string() {
                summary["conversationId"] = event["conversationId"].clone();
                if !summary["chatIds"]
                    .as_array()
                    .unwrap()
                    .contains(&event["conversationId"])
                {
                    summary["chatIds"]
                        .as_array_mut()
                        .unwrap()
                        .push(event["conversationId"].clone());
                }
            }
            model::apply_summary(&mut summary, &event);
            collect_identity(&event, &mut identities);
        })?;
        let mut canonical: Vec<_> = messages.values().collect();
        canonical.sort_by_key(|e| number(e, "seq"));
        for event in canonical {
            if !saw {
                summary["startedAt"] = event["time"].clone();
                summary["updatedAt"] = event["time"].clone();
                saw = true;
            }
            model::apply_summary(&mut summary, event);
            collect_identity(event, &mut identities);
        }
        if !saw && checkpoint.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No recoverable session metadata or history",
            ));
        }
        if saw {
            identities.sort_by_key(position);
            summary["timelineTurns"] = json!({});
            summary["requestTurns"] = json!({});
            summary["nativeQuestion"] = Value::Null;
            for event in identities {
                chronology::apply_identity(&mut summary, &event);
            }
        }
        let mut reconciled = false;
        if let Some(checkpoint) = checkpoint {
            let same_watermark = checkpoint.history_seq == Some(max_seq);
            let current = same_watermark
                && checkpoint.token_estimate_current
                && (!aliases_collapsed || checkpoint.canonical_projection_current)
                && !checkpoint.outcome_counters_missing
                && !checkpoint.activity_boundary_missing
                && [
                    "timelineTurns",
                    "requestTurns",
                    "nativeQuestion",
                    "finishTurn",
                ]
                .into_iter()
                .all(|key| checkpoint.summary.get(key).is_some());
            reconciled = !current || !checkpoint.canonical_projection_current;
            if current {
                summary = checkpoint.summary;
            } else if max_seq == 0 {
                let missing = checkpoint.outcome_counters_missing;
                summary = checkpoint.summary;
                if missing {
                    summary["errors"] = json!(0);
                }
                summary["timelineTurns"] = json!({});
                summary["requestTurns"] = json!({});
                for key in [
                    "nativeQuestion",
                    "lastToolCallAt",
                    "lastAssistantFinalAt",
                    "lastTurnEndAt",
                    "lastFinishReportAt",
                    "finishTurn",
                ] {
                    summary[key] = Value::Null;
                }
            } else {
                let rebuilt = summary;
                summary = checkpoint.summary;
                model::max_time(&mut summary, "updatedAt", &rebuilt["updatedAt"]);
                let context = number(&summary, "contextTokens") as i128
                    + number(&rebuilt, "estimatedTokens") as i128
                    - number(&summary, "estimatedTokens") as i128
                    + historical_return_reduction as i128;
                summary["contextTokens"] = json!(context.max(0) as u64);
                if !same_watermark {
                    summary["activeTurnId"] = rebuilt["activeTurnId"].clone();
                }
                let finish = &rebuilt["finishTurn"];
                summary["finishTurn"] = if finish["conversationId"] == summary["conversationId"] {
                    finish.clone()
                } else {
                    Value::Null
                };
                let mut agents = summary["agents"].as_array().cloned().unwrap_or_default();
                for agent in rebuilt["agents"].as_array().unwrap() {
                    if !agents.contains(agent) {
                        agents.push(agent.clone());
                    }
                }
                summary["agents"] = json!(agents);
                for key in [
                    "events",
                    "timelineTurns",
                    "requestTurns",
                    "nativeQuestion",
                    "userMessages",
                    "toolCalls",
                    "lastToolCallAt",
                    "lastToolActivity",
                    "lastAssistantFinalAt",
                    "lastTurnEndAt",
                    "lastFinishReportAt",
                    "processExitNonzero",
                    "toolRejected",
                    "toolInternalErrors",
                    "errors",
                    "estimatedTokens",
                    "lastHandoffId",
                    "lastHandoffAt",
                    "lastTurnOutcome",
                ] {
                    summary[key] = rebuilt[key].clone();
                }
            }
        } else if saw {
            reconciled = true;
        }
        // Repair only the derived preview. Unsupported UTF-16 slices keep their canonical
        // messages and existing label; global lossless JSON remains an explicit open gate.
        let repaired = title::refresh_user_title(&mut summary, messages.values()).unwrap_or(false);
        if reconciled || repaired {
            self.save_summary(id, &summary, max_seq)?;
        }
        Ok(Entry {
            summary,
            messages,
            canonical_bytes,
            next: max_seq + 1,
            dirty: false,
        })
    }
    fn entry(&self, id: &str) -> io::Result<Arc<Mutex<Entry>>> {
        if !model::session_id(id) {
            return Err(invalid("Invalid session id"));
        }
        let mut entries = lock(&self.inner.entries)?;
        if let Some(entry) = entries.get(id) {
            return Ok(entry.clone());
        }
        let entry = Arc::new(Mutex::new(self.load(id)?));
        entries.insert(id.into(), entry.clone());
        Ok(entry)
    }
    fn save_summary(&self, id: &str, summary: &Value, seq: u64) -> io::Result<()> {
        let target = self.path(id, "meta.json", true)?;
        let mut persisted = summary.clone();
        persisted["__historySeq"] = json!(seq);
        persisted["__canonicalProjection"] = json!(1);
        persisted["__tokenEstimate"] = json!(1);
        if let Ok(previous) = fs::read(&target)
            && serde_json::from_slice::<Value>(&previous).is_ok()
        {
            atomic_write(&self.path(id, "meta.backup.json", true)?, &previous)?;
        }
        atomic_write(&target, &serde_json::to_vec_pretty(&persisted)?)
    }
    pub fn create(&self, options: CreateOptions) -> io::Result<Value> {
        let _catalog = lock(&self.inner.catalog)?;
        let id = options.id.unwrap_or_else(|| {
            format!(
                "{}-{}",
                chrono::Utc::now().format("%Y-%m-%d"),
                &uuid::Uuid::new_v4().to_string()[..8]
            )
        });
        if !model::session_id(&id) {
            return Err(invalid("Invalid session id"));
        }
        match self.summary(&id) {
            Ok(Some(summary)) => {
                if summary["origin"]["kind"] == "desktop" {
                    return Ok(summary);
                }
                return Err(invalid(
                    "Reserved opening session belongs to different work",
                ));
            }
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        // One provider conversation, including retired aliases, has one durable owner.
        // Otherwise a restart can misattribute observed messages to a new epoch.
        if let Some(conversation) = options.conversation_id.as_deref().filter(|s| !s.is_empty())
            && self.list()?.iter().any(|row| {
                row["conversationId"] == conversation
                    || row["chatIds"]
                        .as_array()
                        .is_some_and(|ids| ids.iter().any(|id| id == conversation))
            })
        {
            return Err(invalid("Conversation already belongs to another session"));
        }
        let title = options
            .title
            .as_deref()
            .map(cos_core::config::js_trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("ChatGPT session");
        let mut summary = model::empty_summary(
            &id,
            title,
            options.conversation_id.as_deref(),
            options.now.unwrap_or_else(now),
        );
        summary["origin"] = options.origin.unwrap_or(Value::Null);
        if let Some(source) = options.title_source {
            summary["titleSource"] = json!(source);
        }
        if let Some(source) = summary["origin"]["fromSessionId"].as_str()
            && let Some(parent) = self.summary(source)?
            && let Some(project) = parent.get("projectId")
        {
            summary["projectId"] = project.clone();
        }
        let dir = self.path(&id, "", true)?;
        fs::create_dir_all(dir)?;
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.path(&id, "events.jsonl", true)?)?;
        let map = self.path(&id, "messages.json", true)?;
        match OpenOptions::new().write(true).create_new(true).open(map) {
            Ok(mut file) => {
                file.write_all(b"{}")?;
                file.sync_all()?;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        self.save_summary(&id, &summary, 0)?;
        lock(&self.inner.entries)?.insert(
            id,
            Arc::new(Mutex::new(Entry {
                summary: summary.clone(),
                messages: BTreeMap::new(),
                canonical_bytes: 0,
                next: 1,
                dirty: false,
            })),
        );
        Ok(summary)
    }
    pub fn summary(&self, id: &str) -> io::Result<Option<Value>> {
        match self.entry(id) {
            Ok(entry) => Ok(Some(lock(&entry)?.summary.clone())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub fn append(&self, id: &str, mut event: Value) -> io::Result<Value> {
        model::validate_event(&event)?;
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        event.as_object_mut().unwrap().remove("seq");
        event["seq"] = json!(next_sequence(entry.next)?);
        let mut bytes = serde_json::to_vec(&event)?;
        if bytes.len() > MAX_LINE_BYTES {
            return Err(invalid("History event exceeds line limit"));
        }
        bytes.push(b'\n');
        let target = self.path(id, "events.jsonl", true)?;
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&target)?;
        let len = file.metadata()?.len();
        let mut needs_separator = false;
        if len > 0 {
            file.seek(SeekFrom::Start(len - 1))?;
            let mut last = [0];
            file.read_exact(&mut last)?;
            needs_separator = last[0] != b'\n';
        }
        if len
            .checked_add(bytes.len() as u64)
            .and_then(|total| total.checked_add(u64::from(needs_separator)))
            .is_none_or(|total| total > MAX_HISTORY_JOURNAL_BYTES)
        {
            return Err(invalid(
                "History journal exceeds write budget; original bytes preserved",
            ));
        }
        if needs_separator {
            file.write_all(b"\n")?;
        }
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_data()) {
            let mut committed = false;
            scan(&target, |e| {
                if e == event {
                    committed = true;
                }
            })?;
            if !committed {
                return Err(error);
            }
        }
        entry.next += 1;
        model::apply_summary(&mut entry.summary, &event);
        entry.dirty = true;
        Ok(event)
    }
    fn write_shard(&self, id: &str, key: &str, event: &Value) -> io::Result<()> {
        let bytes = serde_json::to_vec(event)?;
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(invalid("Canonical message is too large"));
        }
        let dir = self.path(id, "messages", true)?;
        fs::create_dir_all(dir)?;
        atomic_write(
            &self.path(id, &format!("messages/{}.json", hash(key.as_bytes())), true)?,
            &bytes,
        )
    }
    pub fn upsert(
        &self,
        id: &str,
        mut event: Value,
        options: UpsertOptions,
    ) -> io::Result<UpsertResult> {
        model::validate_event(&event)?;
        if !matches!(string(&event, "kind"), "user_message" | "assistant_message") {
            return Err(invalid(
                "Canonical prose requires a user or assistant identity",
            ));
        }
        let direct = message_key(&event).ok_or_else(|| invalid("Missing canonical identity"))?;
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        let mut key = direct.clone();
        if !entry.messages.contains_key(&key)
            && event["kind"] == "assistant_message"
            && !string(&event, "providerMessageId").is_empty()
        {
            let aliases: Vec<_> = entry
                .messages
                .iter()
                .filter(|(_, e)| {
                    e["kind"] == "assistant_message"
                        && e["providerMessageId"] == event["providerMessageId"]
                })
                .collect();
            if aliases.len() == 1 {
                key = aliases[0].0.clone();
            }
        }
        let previous = entry.messages.get(&key).cloned();
        if let Some(previous) = &previous {
            if key != direct {
                event["messageId"] = previous["messageId"].clone();
            }
            if previous["kind"] == "assistant_message" && terminal(previous) && !terminal(&event) {
                return Ok(UpsertResult {
                    event: previous.clone(),
                    changed: false,
                    content_changed: false,
                });
            }
        }
        let same = previous
            .as_ref()
            .is_some_and(|p| p["message"] == event["message"]);
        if let Some(previous) = &previous {
            if event["kind"] == "assistant_message" {
                for field in [
                    "authoredAt",
                    "providerMessageId",
                    "resolvedModel",
                    "references",
                    "turnId",
                    "agent",
                ] {
                    if (event.get(field).is_none_or(|v| v.is_null())
                        || matches!(field, "authoredAt" | "turnId") && previous[field].is_string())
                        && let Some(value) = previous.get(field)
                    {
                        event[field] = value.clone();
                    }
                }
                if event.get("renderedHtml").is_none_or(|v| v.is_null())
                    && same
                    && let Some(html) = previous.get("renderedHtml")
                {
                    event["renderedHtml"] = html.clone();
                }
                if previous["goalEligible"] == true {
                    event["goalEligible"] = json!(true);
                }
                event["state"] = json!(if terminal(&event) {
                    "final"
                } else {
                    "streaming"
                });
                event["final"] = json!(terminal(&event));
            } else {
                for field in [
                    "inputId",
                    "authoredAt",
                    "authoredText",
                    "wireTokenEstimate",
                    "reaction",
                    "attachments",
                    "inputDelivery",
                    "model",
                    "reasoningEffort",
                    "assets",
                    "retiredImageAssetIds",
                ] {
                    if (event.get(field).is_none_or(|v| v.is_null())
                        || field == "inputDelivery" && previous[field] == "confirmed"
                        || field == "authoredAt" && previous[field].is_number()
                        || field == "attachments" && previous["inputId"].is_string())
                        && let Some(value) = previous.get(field)
                    {
                        event[field] = value.clone();
                    }
                }
            }
        }
        for field in [
            "seq",
            "origin",
            "contentSeq",
            "finalContentSeq",
            "finalObservedAt",
            "turnOrigin",
        ] {
            event.as_object_mut().unwrap().remove(field);
        }
        self.filter_assets(id, &mut event)?;
        if let Some(previous) = &previous {
            let mut a = previous.clone();
            for field in [
                "seq",
                "origin",
                "contentSeq",
                "finalContentSeq",
                "finalObservedAt",
                "turnOrigin",
            ] {
                a.as_object_mut().unwrap().remove(field);
            }
            if !options.prefer_time || key != direct {
                a.as_object_mut().unwrap().remove("time");
                let mut b = event.clone();
                b.as_object_mut().unwrap().remove("time");
                if a == b {
                    return Ok(UpsertResult {
                        event: previous.clone(),
                        changed: false,
                        content_changed: false,
                    });
                }
            } else if a == event {
                return Ok(UpsertResult {
                    event: previous.clone(),
                    changed: false,
                    content_changed: false,
                });
            }
        }
        let seq = next_sequence(entry.next)?;
        let content_seq = if options.work == Some(false)
            && ((event["kind"] == "user_message" && previous.is_some())
                || (event["kind"] == "assistant_message" && !terminal(&event)))
        {
            previous.as_ref().map(model::work_sequence).unwrap_or(0)
        } else if same
            && previous.as_ref().is_some_and(|p| {
                event["kind"] != "assistant_message" || terminal(p) == terminal(&event)
            })
        {
            model::work_sequence(previous.as_ref().unwrap())
        } else {
            seq
        };
        event["contentSeq"] = json!(content_seq);
        if event["kind"] == "assistant_message" && terminal(&event) {
            let retain = same && previous.as_ref().is_some_and(terminal);
            event["finalContentSeq"] = json!(if retain {
                previous
                    .as_ref()
                    .and_then(|p| p["finalContentSeq"].as_u64())
                    .unwrap_or_else(|| position(previous.as_ref().unwrap()))
            } else {
                seq
            });
            if retain {
                if let Some(at) = previous.as_ref().and_then(|p| p.get("finalObservedAt")) {
                    event["finalObservedAt"] = at.clone();
                }
            } else {
                event["finalObservedAt"] = json!(options.now.unwrap_or_else(now));
            }
        }
        let observed = event["time"].clone();
        if !(options.prefer_time && key == direct)
            && let Some(previous) = &previous
        {
            event["time"] = previous["time"].clone();
        }
        event["origin"] = json!(previous.as_ref().map(position).unwrap_or(seq));
        event["seq"] = json!(seq);
        let budget = admitted_canonical_bytes(&entry, &key, &event)?;
        self.write_shard(id, &key, &event)?;
        entry.messages.insert(key, event.clone());
        entry.canonical_bytes = budget;
        entry.next += 1;
        if event["kind"] == "user_message" {
            let mut preview = entry.summary.clone();
            if title::refresh_user_title(&mut preview, entry.messages.values()).is_ok() {
                entry.summary = preview;
            }
        }
        if let Some(previous) = previous {
            let delta = event_tokens(&event) as i64 - event_tokens(&previous) as i64;
            for key in ["estimatedTokens", "contextTokens"] {
                entry.summary[key] = json!((number(&entry.summary, key) as i64 + delta).max(0));
            }
            model::max_time(&mut entry.summary, "updatedAt", &observed);
            model::note_finish_work(&mut entry.summary, &event);
            if event["kind"] == "assistant_message" && terminal(&event) {
                model::max_time(&mut entry.summary, "lastAssistantFinalAt", &observed);
            }
        } else {
            model::apply_summary(&mut entry.summary, &event);
        }
        entry.dirty = true;
        Ok(UpsertResult {
            event,
            changed: true,
            content_changed: !same,
        })
    }
    pub fn flush(&self) -> io::Result<()> {
        let entries: Vec<_> = lock(&self.inner.entries)?
            .iter()
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect();
        let mut failure = None;
        for (id, owner) in entries {
            let mut entry = lock(&owner)?;
            if entry.dirty {
                match self.save_summary(&id, &entry.summary, entry.next - 1) {
                    Ok(()) => entry.dirty = false,
                    Err(e) => {
                        failure = Some(e);
                    }
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }
    pub fn read(&self, id: &str, options: ReadOptions) -> io::Result<Vec<Value>> {
        let owner = self.entry(id)?;
        let entry = lock(&owner)?;
        let selected = |e: &Value| {
            options.from.is_none_or(|from| number(e, "seq") >= from)
                && (options.kinds.is_empty() || options.kinds.iter().any(|k| e["kind"] == *k))
                && options.agent.as_ref().is_none_or(|a| e["agent"] == *a)
        };
        let mut events = Vec::new();
        let mut exceeded = false;
        scan(&self.path(id, "events.jsonl", true)?, |e| {
            if selected(&e) && !message_key(&e).is_some_and(|k| entry.messages.contains_key(&k)) {
                if events.len() >= MAX_HISTORY_READ_EVENTS {
                    exceeded = true;
                    return;
                }
                events.push(e);
                if options.from.is_some()
                    && options.limit < usize::MAX / 2
                    && events.len() > options.limit.saturating_mul(2)
                {
                    events.sort_by_key(|e| number(e, "seq"));
                    events.truncate(options.limit);
                }
            }
        })?;
        if exceeded {
            return Err(invalid(
                "History read exceeds result budget; use bounded pagination",
            ));
        }
        for event in entry.messages.values().filter(|event| selected(event)) {
            if events.len() >= MAX_HISTORY_READ_EVENTS {
                return Err(invalid(
                    "History read exceeds result budget; use bounded pagination",
                ));
            }
            events.push(event.clone());
        }
        if options.from.is_some() {
            events.sort_by_key(|e| number(e, "seq"));
            events.truncate(options.limit);
        }
        let messages = entry.messages.values().cloned().collect::<Vec<_>>();
        let mut result =
            chronology::chronological(&chronology::project(&events, &entry.summary, &messages));
        result.truncate(options.limit);
        Ok(result)
    }
    pub fn recent(&self, id: &str, limit: usize, before: Option<u64>) -> io::Result<Vec<Value>> {
        if limit > MAX_HISTORY_READ_EVENTS {
            return Err(invalid("History page exceeds result budget"));
        }
        let owner = self.entry(id)?;
        let entry = lock(&owner)?;
        let mut rows = Vec::new();
        scan(&self.path(id, "events.jsonl", true)?, |e| {
            if before.is_none_or(|b| residence(&e) < b)
                && !message_key(&e).is_some_and(|k| entry.messages.contains_key(&k))
            {
                rows.push(e);
                if rows.len() > limit.saturating_mul(2) {
                    rows.sort_by_key(|e| std::cmp::Reverse((residence(e), number(e, "seq"))));
                    rows.truncate(limit);
                }
            }
        })?;
        rows.extend(
            entry
                .messages
                .values()
                .filter(|e| before.is_none_or(|b| residence(e) < b))
                .cloned(),
        );
        rows.sort_by_key(|e| std::cmp::Reverse((residence(e), number(e, "seq"))));
        rows.truncate(limit);
        let messages = entry.messages.values().cloned().collect::<Vec<_>>();
        Ok(chronology::chronological(&chronology::project(
            &rows,
            &entry.summary,
            &messages,
        )))
    }
    pub fn list(&self) -> io::Result<Vec<Value>> {
        let root = owned_path(&self.inner.directory, Path::new("sessions"), false)?;
        let mut result = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let meta = entry.file_type()?;
            if !meta.is_dir() || is_link(&fs::symlink_metadata(entry.path())?) {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if !model::session_id(&id) {
                continue;
            }
            if let Some(summary) = self.summary(&id)? {
                result.push(summary);
            }
        }
        result.sort_by(|a, b| {
            number(b, "updatedAt")
                .cmp(&number(a, "updatedAt"))
                .then_with(|| string(b, "id").cmp(string(a, "id")))
        });
        Ok(result)
    }
    pub fn list_page(
        &self,
        limit: usize,
        cursor: Option<(u64, String)>,
        project: Option<&str>,
    ) -> io::Result<Value> {
        let mut rows = self.list()?;
        rows.retain(|r| {
            project.is_none_or(|p| r["projectId"] == p)
                && cursor.as_ref().is_none_or(|(at, id)| {
                    number(r, "updatedAt") < *at
                        || number(r, "updatedAt") == *at && string(r, "id") < id.as_str()
                })
        });
        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let next = if has_more {
            rows.last()
                .map(|r| json!({"updatedAt":r["updatedAt"],"id":r["id"]}))
        } else {
            None
        };
        Ok(json!({"sessions":rows,"next":next}))
    }
    pub fn find_conversation(&self, conversation: &str) -> io::Result<Option<Value>> {
        let owners: Vec<_> = self
            .list()?
            .into_iter()
            .filter(|r| r["conversationId"] == conversation)
            .collect();
        Ok(if owners.len() == 1 {
            owners.into_iter().next()
        } else {
            None
        })
    }
    pub fn superseded(&self, conversation: &str) -> io::Result<bool> {
        Ok(self.list()?.iter().any(|r| {
            r["conversationId"] != conversation
                && r["chatIds"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| id == conversation))
        }))
    }
    pub fn rebind(
        &self,
        id: &str,
        from: Option<&str>,
        to: &str,
        handoff: Option<&str>,
        at: i64,
    ) -> io::Result<bool> {
        if to.is_empty() || from == Some(to) || handoff.is_some_and(|h| !model::session_id(h)) {
            return Ok(false);
        }
        let _catalog = lock(&self.inner.catalog)?;
        // Provider conversation IDs are not necessarily UUIDs. Both active and
        // retired aliases must remain fenced to their owning session epoch.
        if self.list()?.iter().any(|row| {
            row["id"] != id
                && (row["conversationId"] == to
                    || row["chatIds"]
                        .as_array()
                        .is_some_and(|ids| ids.iter().any(|chat| chat == to)))
        }) {
            return Ok(false);
        }
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        if entry.summary["conversationId"].as_str() != from {
            return Ok(false);
        }
        let mut staged = entry.summary.clone();
        staged["conversationId"] = json!(to);
        if let Some(from) = from {
            if !staged["retiredChatAt"].is_object() {
                staged["retiredChatAt"] = json!({});
            }
            staged["retiredChatAt"][from] = json!(at);
        }
        // A round-trip rebind makes the old alias active again. Clear its
        // retirement marker before publication, not only during cold restore.
        if let Some(retired) = staged["retiredChatAt"].as_object_mut() {
            retired.remove(to);
        }
        if !staged["chatIds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == to)
        {
            staged["chatIds"].as_array_mut().unwrap().push(json!(to));
        }
        staged["contextTokens"] = json!(0);
        staged["activeTurnId"] = Value::Null;
        staged["finishTurn"] = Value::Null;
        staged["updatedAt"] = json!(at);
        staged["endedAt"] = Value::Null;
        staged
            .as_object_mut()
            .unwrap()
            .remove("browserRecoveryDismissedAt");
        if let Some(id) = handoff {
            staged["lastCommittedResumeHandoffId"] = json!(id);
        }
        if self.save_summary(id, &staged, entry.next - 1).is_err() {
            return Ok(false);
        }
        entry.summary = staged;
        entry.dirty = false;
        Ok(true)
    }
    pub fn rename(
        &self,
        id: &str,
        name: &str,
        source: title::TitleSource,
        conversation: Option<&str>,
    ) -> io::Result<Value> {
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        let mut staged = entry.summary.clone();
        title::rename(
            &mut staged,
            entry.messages.values(),
            name,
            source,
            conversation,
        )?;
        if staged != entry.summary {
            self.save_summary(id, &staged, entry.next - 1)?;
            entry.summary = staged;
            entry.dirty = false;
        }
        Ok(entry.summary.clone())
    }
    pub fn clear_name(&self, id: &str) -> io::Result<Value> {
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        let mut staged = entry.summary.clone();
        title::clear(&mut staged, entry.messages.values())?;
        if staged != entry.summary {
            self.save_summary(id, &staged, entry.next - 1)?;
            entry.summary = staged;
            entry.dirty = false;
        }
        Ok(entry.summary.clone())
    }
    pub fn observe_model(
        &self,
        id: &str,
        conversation: &str,
        model_name: &str,
        observed_at: f64,
        reasoning: Option<&str>,
    ) -> io::Result<bool> {
        if !observed_at.is_finite()
            || model_name.is_empty()
            || model_name.len() > 80
            || !model_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" ._-".contains(&b))
        {
            return Ok(false);
        }
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        if entry.summary["conversationId"] != conversation
            || observed_at
                < entry.summary["selectedModel"]["observedAt"]
                    .as_f64()
                    .unwrap_or(0.0)
        {
            return Ok(false);
        }
        let mut selected = model::javascript_numbers(
            json!({"conversationId":conversation,"model":model_name,"observedAt":observed_at}),
        );
        if let Some(reasoning) = reasoning.filter(|r| !r.is_empty()) {
            selected["reasoningEffort"] = json!(reasoning);
        }
        if entry.summary["selectedModel"] == selected {
            return Ok(false);
        }
        let mut staged = entry.summary.clone();
        staged["selectedModel"] = selected;
        self.save_summary(id, &staged, entry.next - 1)?;
        entry.summary = staged;
        entry.dirty = false;
        Ok(true)
    }
    pub fn update_metadata(&self, id: &str, edit: impl FnOnce(&mut Value)) -> io::Result<Value> {
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        let mut staged = entry.summary.clone();
        edit(&mut staged);
        if staged["id"] != entry.summary["id"]
            || staged["conversationId"] != entry.summary["conversationId"]
            || staged["chatIds"] != entry.summary["chatIds"]
        {
            return Err(invalid("Session identity changes require rebind"));
        }
        self.save_summary(id, &staged, entry.next - 1)?;
        entry.summary = staged.clone();
        entry.dirty = false;
        Ok(staged)
    }
    pub fn end(&self, id: &str, expected: Option<&str>, at: i64) -> io::Result<bool> {
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        if expected.is_some_and(|c| entry.summary["conversationId"] != c) {
            return Ok(false);
        }
        let mut staged = entry.summary.clone();
        staged["endedAt"] = json!(at);
        staged["updatedAt"] = json!(at);
        self.save_summary(id, &staged, entry.next - 1)?;
        entry.summary = staged;
        entry.dirty = false;
        Ok(true)
    }
    pub fn reopen(&self, id: &str, observed_at: i64) -> io::Result<bool> {
        let summary = self
            .summary(id)?
            .ok_or_else(|| invalid("Unknown session"))?;
        if summary["browserRecoveryDismissedAt"]
            .as_i64()
            .is_some_and(|dismissed| observed_at <= dismissed)
        {
            return Ok(false);
        }
        self.update_metadata(id, |s| {
            s["endedAt"] = Value::Null;
        })?;
        Ok(true)
    }
    pub fn write_asset(&self, id: &str, data: &[u8], mime: &str) -> io::Result<Value> {
        self.entry(id)?;
        if data.is_empty() || data.len() > MAX_ASSET_BYTES {
            return Err(invalid("Session asset exceeds the per-asset limit"));
        }
        let mut assets = lock(&self.inner.assets)?;
        let extension = match mime {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "text/plain" => "txt",
            _ => "bin",
        };
        let asset_id = format!("{}.{}", &hash(data)[..32], extension);
        let dir = self.path(id, "assets", true)?;
        fs::create_dir_all(&dir)?;
        let target = self.path(id, &format!("assets/{asset_id}"), true)?;
        if !target.exists() {
            let local = directory_bytes(&dir)?;
            let mut global = 0;
            for entry in fs::read_dir(owned_path(
                &self.inner.directory,
                Path::new("sessions"),
                false,
            )?)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() && !is_link(&fs::symlink_metadata(entry.path())?) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if model::session_id(&name) {
                        global += directory_bytes(&self.path(&name, "assets", true)?)?;
                    }
                }
            }
            if local + data.len() as u64 > MAX_SESSION_ASSETS {
                return Err(invalid("Session asset quota exceeded"));
            }
            if global + data.len() as u64 > assets.limit {
                return Err(io::Error::other("Recording storage is full"));
            }
            atomic_write(&target, data)?;
        }
        assets.retired.remove(&format!("{id}/{asset_id}"));
        Ok(json!({"id":asset_id,"mimeType":mime,"bytes":data.len()}))
    }
    pub fn read_asset(
        &self,
        id: &str,
        asset: &str,
        max: Option<usize>,
    ) -> io::Result<Option<Vec<u8>>> {
        if !valid_asset(asset) {
            return Ok(None);
        }
        let path = self.path(id, &format!("assets/{asset}"), true)?;
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let meta = file.metadata()?;
        let bound = max.unwrap_or(MAX_ASSET_BYTES).min(MAX_ASSET_BYTES);
        if !meta.is_file() || meta.len() > bound as u64 {
            return Ok(None);
        }
        let mut bytes = Vec::with_capacity(meta.len() as usize + 1);
        (&mut file).take(bound as u64 + 1).read_to_end(&mut bytes)?;
        Ok((bytes.len() <= bound).then_some(bytes))
    }
    pub fn set_asset_limit(&self, gib: u64) -> io::Result<()> {
        if ![2, 5, 10, 20, 50].contains(&gib) {
            return Err(invalid("Invalid recording storage limit"));
        }
        lock(&self.inner.assets)?.limit = gib * 1024 * 1024 * 1024;
        Ok(())
    }
    fn filter_assets(&self, id: &str, event: &mut Value) -> io::Result<()> {
        let assets = lock(&self.inner.assets)?;
        if let Some(references) = event["assets"].as_array().cloned() {
            let mut retained = Vec::new();
            let mut retired = event["retiredImageAssetIds"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for asset in references {
                let name = string(&asset, "id");
                if !valid_asset(name)
                    || retired.iter().any(|v| v == name)
                    || assets.retired.contains(&format!("{id}/{name}"))
                    || !self.path(id, &format!("assets/{name}"), true)?.is_file()
                {
                    if !name.is_empty() && !retired.iter().any(|v| v == name) {
                        retired.push(json!(name));
                    }
                } else {
                    retained.push(asset);
                }
            }
            let map = event.as_object_mut().unwrap();
            if retained.is_empty() {
                map.remove("assets");
            } else {
                map.insert("assets".into(), json!(retained));
            }
            if !retired.is_empty() {
                map.insert("retiredImageAssetIds".into(), json!(retired));
            }
        }
        Ok(())
    }
    pub fn store_text(&self, id: &str, text: &str, inline: usize) -> io::Result<Value> {
        let clean = cos_core::logging::redact_credentials(text);
        let chars = clean.encode_utf16().count();
        if chars <= inline {
            return Ok(json!({"text":clean,"truncated":false,"chars":chars}));
        }
        let mut out = json!({"text":utf16_prefix(&clean,inline),"truncated":true,"chars":chars});
        if chars <= MAX_ASSET_BYTES
            && let Ok(asset) = self.write_asset(id, clean.as_bytes(), "text/plain")
        {
            out["assetId"] = asset["id"].clone();
        }
        out["digest"] = json!(hash(clean.as_bytes()));
        Ok(out)
    }
    pub fn read_full_text(&self, id: &str, stored: &Value) -> io::Result<String> {
        if let Some(asset) = stored["assetId"].as_str()
            && let Some(bytes) = self.read_asset(id, asset, Some(MAX_ASSET_BYTES))?
        {
            return String::from_utf8(bytes).map_err(|_| invalid("Overflow text is not UTF-8"));
        }
        Ok(string(stored, "text").into())
    }
    pub fn write_document(
        &self,
        id: &str,
        kind: &str,
        name: &str,
        value: &Value,
    ) -> io::Result<()> {
        self.entry(id)?;
        if !model::session_id(name) || !matches!(kind, "traces" | "handoffs") {
            return Err(invalid("Invalid history document identity"));
        }
        let bytes = serde_json::to_vec_pretty(value)?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(invalid("History document too large"));
        }
        atomic_write(
            &self.path(id, &format!("{kind}/{name}.json"), true)?,
            &bytes,
        )
    }
    pub fn read_document(&self, id: &str, kind: &str, name: &str) -> io::Result<Option<Value>> {
        if !model::session_id(name) || !matches!(kind, "traces" | "handoffs") {
            return Err(invalid("Invalid history document identity"));
        }
        match read_json(
            &self.path(id, &format!("{kind}/{name}.json"), true)?,
            8 * 1024 * 1024,
        ) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub fn record_process(&self, id: &str, mut event: Value) -> io::Result<()> {
        model::validate_event(&event)?;
        let key = message_key(&event)
            .filter(|_| event["kind"] == "tool_call")
            .ok_or_else(|| invalid("Invalid process call"))?;
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        if entry.messages.contains_key(&key) {
            return Err(invalid("Process call identity already recorded"));
        }
        let seq = next_sequence(entry.next)?;
        event["seq"] = json!(seq);
        event["origin"] = json!(seq);
        let budget = admitted_canonical_bytes(&entry, &key, &event)?;
        self.write_shard(id, &key, &event)?;
        entry.next += 1;
        model::apply_summary(&mut entry.summary, &event);
        entry.messages.insert(key, event);
        entry.canonical_bytes = budget;
        entry.dirty = true;
        Ok(())
    }
    pub fn complete_process(&self, id: &str, call_id: &str, completion: Value) -> io::Result<bool> {
        if !completion["completedAt"].is_number()
            || !completion["durationMs"].is_number()
            || !(completion["exitCode"].is_null() || completion["exitCode"].is_i64())
        {
            return Err(invalid("Invalid process completion"));
        }
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        let key = format!("tool_call\0{call_id}");
        let Some(previous) = entry.messages.get(&key).cloned().filter(|p| {
            p["call"]["process"].is_object() && p["call"]["process"].get("completedAt").is_none()
        }) else {
            return Ok(false);
        };
        let mut full = previous.clone();
        let exit = completion["exitCode"].as_i64();
        let failed = exit.is_some_and(|n| n != 0) && completion["benignExit"] != true;
        for (key, value) in completion.as_object().unwrap() {
            full["call"]["process"][key] = value.clone();
        }
        let title = string(&previous["call"]["summary"], "title");
        full["call"]["summary"]["title"] =
            json!(if let Some(rest) = title.strip_prefix("Started ") {
                format!(
                    "{}{}",
                    if failed {
                        "Command failed "
                    } else {
                        "Completed "
                    },
                    rest
                )
            } else {
                title.into()
            });
        full["call"]["summary"]["metric"] = json!(match exit {
            None => "finished (exit unknown)".to_owned(),
            Some(code) if failed => format!("✕ exit {code}"),
            _ => "✓ finished".into(),
        });
        full["call"]["summary"]["tone"] = json!(if exit.is_none() {
            "warn"
        } else if failed {
            "bad"
        } else {
            "good"
        });
        full["seq"] = json!(next_sequence(entry.next)?);
        let budget = admitted_canonical_bytes(&entry, &key, &full)?;
        self.write_shard(id, &key, &full)?;
        entry.next += 1;
        let delta = event_tokens(&full) as i64 - event_tokens(&previous) as i64;
        entry.summary["estimatedTokens"] =
            json!((number(&entry.summary, "estimatedTokens") as i64 + delta).max(0));
        if full["call"]["conversationId"] == entry.summary["conversationId"] {
            entry.summary["contextTokens"] =
                json!((number(&entry.summary, "contextTokens") as i64 + delta).max(0));
        }
        entry.messages.insert(key, full);
        entry.canonical_bytes = budget;
        entry.dirty = true;
        Ok(true)
    }
    pub fn upsert_native_image(&self, id: &str, mut event: Value) -> io::Result<UpsertResult> {
        model::validate_event(&event)?;
        let key = message_key(&event)
            .filter(|_| event["kind"] == "native_image")
            .ok_or_else(|| invalid("Invalid native image identity"))?;
        let owner = self.entry(id)?;
        let mut entry = lock(&owner)?;
        let previous = entry.messages.get(&key).cloned();
        if let Some(previous) = &previous {
            if previous["previewError"] == "removed" {
                event.as_object_mut().unwrap().remove("asset");
                event["previewStatus"] = json!("unavailable");
                event["previewError"] = json!("removed");
            }
            let mut comparison = previous.clone();
            comparison
                .as_object_mut()
                .unwrap()
                .retain(|key, _| !matches!(key.as_str(), "seq" | "origin"));
            if comparison == event {
                return Ok(UpsertResult {
                    event: previous.clone(),
                    changed: false,
                    content_changed: false,
                });
            }
            event["time"] = previous["time"].clone();
        }
        let seq = next_sequence(entry.next)?;
        event["seq"] = json!(seq);
        event["origin"] = json!(previous.as_ref().map(position).unwrap_or(seq));
        let budget = admitted_canonical_bytes(&entry, &key, &event)?;
        self.write_shard(id, &key, &event)?;
        entry.next += 1;
        if previous.is_none() {
            model::apply_summary(&mut entry.summary, &event);
        }
        entry.messages.insert(key, event.clone());
        entry.canonical_bytes = budget;
        entry.dirty = true;
        Ok(UpsertResult {
            event,
            changed: true,
            content_changed: previous.is_none(),
        })
    }
    pub fn read_plan(&self, id: &str) -> io::Result<Option<Value>> {
        match read_json(&self.path(id, "plan.json", true)?, 96 * 1024) {
            Ok(value) => Ok(validate_plan(value, true).ok()),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidData
                ) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
    pub fn update_plan(
        &self,
        id: &str,
        conversation: &str,
        input: Value,
        started: i64,
        recovered: Option<i64>,
    ) -> io::Result<bool> {
        if started < 0 {
            return Err(invalid("Invalid plan clock"));
        }
        let mut plan = validate_plan(input, false)?;
        plan["updatedAt"] = json!(started);
        let bytes = serde_json::to_vec(&plan)?;
        if bytes.len() > 96 * 1024 {
            return Err(invalid("Plan exceeds storage budget"));
        }
        let owner = self.entry(id)?;
        let entry = lock(&owner)?;
        if entry.summary["conversationId"] != conversation {
            let retired = entry.summary["retiredChatAt"][conversation].as_i64();
            if recovered
                .is_none_or(|at| at < 0 || retired.is_none_or(|retired| started.max(at) >= retired))
                || !entry.summary["conversationId"].is_string()
                || !entry.summary["chatIds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c == conversation)
            {
                return Ok(false);
            }
        }
        if self
            .read_plan(id)?
            .is_some_and(|p| p["updatedAt"].as_i64().unwrap_or(0) > started)
        {
            return Ok(false);
        }
        atomic_write(&self.path(id, "plan.json", true)?, &bytes)?;
        Ok(true)
    }
    /// Explicit cleanup retires references in canonical overlays before deleting image bytes.
    pub fn clear_images(&self, maximum_bytes: Option<u64>) -> io::Result<Value> {
        let summaries = self.list()?;
        let mut removed = 0_u64;
        let mut freed = 0_u64;
        for summary in summaries {
            if maximum_bytes.is_some_and(|maximum| freed >= maximum) {
                break;
            }
            let id = string(&summary, "id");
            let owner = self.entry(id)?;
            let mut entry = lock(&owner)?;
            let mut assets = lock(&self.inner.assets)?;
            let directory = self.path(id, "assets", true)?;
            let files = match fs::read_dir(directory) {
                Ok(e) => Some(e),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            let mut selected = HashMap::<String, (PathBuf, u64)>::new();
            if let Some(files) = files {
                for file in files {
                    let file = file?;
                    let name = file.file_name().to_string_lossy().into_owned();
                    if valid_asset(&name) && (name.ends_with(".png") || name.ends_with(".jpg")) {
                        let path = self.path(id, &format!("assets/{name}"), false)?;
                        let meta = fs::symlink_metadata(&path)?;
                        if meta.is_file() && !is_link(&meta) {
                            selected.insert(name, (path, meta.len()));
                        }
                    }
                }
            }
            if selected.is_empty() {
                continue;
            }
            if let Some(maximum) = maximum_bytes {
                let mut ordered = selected.into_iter().collect::<Vec<_>>();
                ordered.sort_by(|a, b| a.0.cmp(&b.0));
                let mut selected_bytes = 0;
                selected = ordered
                    .into_iter()
                    .take_while(|(_, (_, bytes))| {
                        if freed + selected_bytes >= maximum {
                            return false;
                        }
                        selected_bytes += bytes;
                        true
                    })
                    .collect();
            }
            let mut records = entry.messages.clone();
            scan(&self.path(id, "events.jsonl", true)?, |event| {
                if let Some(key) = message_key(&event) {
                    records.entry(key).or_insert(event);
                }
            })?;
            for (key, previous) in records {
                let mut next = previous.clone();
                let target = if next["kind"] == "tool_call" {
                    &mut next["call"]
                } else {
                    &mut next
                };
                let mut changed = false;
                if let Some(references) = target["assets"].as_array().cloned() {
                    let mut retired = target["retiredImageAssetIds"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    let kept = references
                        .into_iter()
                        .filter(|asset| {
                            let id = string(asset, "id");
                            if selected.contains_key(id) {
                                if !retired.iter().any(|r| r == id) {
                                    retired.push(json!(id));
                                }
                                changed = true;
                                false
                            } else {
                                true
                            }
                        })
                        .collect::<Vec<_>>();
                    if changed {
                        if kept.is_empty() {
                            target.as_object_mut().unwrap().remove("assets");
                        } else {
                            target["assets"] = json!(kept);
                        }
                        target["retiredImageAssetIds"] = json!(retired);
                    }
                }
                if target["asset"]["id"]
                    .as_str()
                    .is_some_and(|id| selected.contains_key(id))
                {
                    target.as_object_mut().unwrap().remove("asset");
                    target["previewStatus"] = json!("unavailable");
                    target["previewError"] = json!("removed");
                    changed = true;
                }
                if changed {
                    next["origin"] = json!(position(&previous));
                    next["seq"] = json!(next_sequence(entry.next)?);
                    let budget = admitted_canonical_bytes(&entry, &key, &next)?;
                    self.write_shard(id, &key, &next)?;
                    entry.next += 1;
                    entry.messages.insert(key, next);
                    entry.canonical_bytes = budget;
                    entry.dirty = true;
                }
            }
            for (asset, (path, bytes)) in selected {
                assets.retired.insert(format!("{id}/{asset}"));
                fs::remove_file(path)?;
                removed += 1;
                freed += bytes;
            }
        }
        self.flush()?;
        Ok(json!({"freedBytes":freed,"removedFiles":removed}))
    }
    pub fn prune(&self, _retain_days: u64) -> usize {
        0
    }
    pub fn write_trace(
        &self,
        id: &str,
        turn: &str,
        input: &Value,
        question: Option<&str>,
    ) -> io::Result<bool> {
        if turn.is_empty()
            || turn.len() > 100
            || !turn
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Ok(false);
        }
        let Some(mut trace) = normalize_trace(input) else {
            return Ok(false);
        };
        let owner = self.entry(id)?;
        let entry = lock(&owner)?;
        if question.is_some_and(|q| {
            entry.summary["timelineTurns"][turn]["questionId"]
                .as_str()
                .is_some_and(|asked| q != asked)
        }) {
            return Ok(false);
        }
        let path = self.path(id, &format!("traces/{turn}.json"), true)?;
        // A stale malformed trace may be ignored, but never overwrite a trace
        // carrying UTF-16 units that serde_json cannot represent losslessly.
        let previous = match read_json(&path, 512 * 1024) {
            Ok(value) => normalize_trace(&value),
            Err(error) if error.kind() == io::ErrorKind::Unsupported => return Err(error),
            Err(_) => None,
        };
        let known: HashMap<_, _> = previous
            .as_ref()
            .into_iter()
            .flatten()
            .filter(|item| item["kind"] == "call" && item["at"].is_number())
            .map(|item| {
                (
                    string(item, "id").to_owned(),
                    item["at"].as_i64().unwrap_or(0),
                )
            })
            .collect();
        for item in &mut trace {
            if item["kind"] == "call"
                && let Some(at) = known.get(string(item, "id"))
            {
                item["at"] = json!(item["at"].as_i64().unwrap_or(i64::MAX).min(*at));
            }
        }
        if previous.as_ref() == Some(&trace) {
            return Ok(false);
        }
        let bytes = serde_json::to_vec(&trace)?;
        if bytes.len() > 512 * 1024 {
            return Ok(false);
        }
        atomic_write(&path, &bytes)?;
        Ok(true)
    }
    pub fn read_trace(&self, id: &str, turn: &str) -> io::Result<Option<Value>> {
        if turn.is_empty()
            || turn.len() > 100
            || !turn
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Ok(None);
        }
        match read_json(
            &self.path(id, &format!("traces/{turn}.json"), true)?,
            512 * 1024,
        ) {
            Ok(v) => Ok(normalize_trace(&v).map(|v| json!(v))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub fn save_handoff(&self, handoff: &Value) -> io::Result<()> {
        let id = string(handoff, "sessionId");
        let name = string(handoff, "id");
        if !valid_handoff(handoff, id, name) {
            return Err(invalid("Invalid handoff provenance"));
        }
        self.write_document(id, "handoffs", name, handoff)
    }
    pub fn read_handoff(&self, id: &str, name: &str) -> io::Result<Option<Value>> {
        Ok(self
            .read_document(id, "handoffs", name)?
            .filter(|v| valid_handoff(v, id, name)))
    }
    /// These are read-side projections; they do not create/reopen workflow authority.
    pub fn latest_user(&self, id: &str) -> io::Result<Option<Value>> {
        let rows = self.recent(id, 1200, None)?;
        Ok(rows
            .into_iter()
            .filter(|e| e["kind"] == "user_message")
            .max_by_key(position))
    }
    pub fn completed_final(
        &self,
        id: &str,
        conversation: &str,
        turn: &str,
    ) -> io::Result<Option<Value>> {
        let rows = self.read(id, ReadOptions::default())?;
        let completed = rows
            .iter()
            .any(|e| e["kind"] == "turn_end" && e["turnId"] == turn && e["outcome"] == "completed");
        if !completed {
            return Ok(None);
        }
        Ok(rows
            .into_iter()
            .filter(|e| {
                e["kind"] == "assistant_message"
                    && terminal(e)
                    && e["turnId"] == turn
                    && e["conversationId"]
                        .as_str()
                        .is_none_or(|c| c == conversation)
            })
            .max_by_key(model::work_sequence))
    }
}
fn normalize_trace(input: &Value) -> Option<Vec<Value>> {
    let rows = input.as_array()?;
    let mut out = Vec::new();
    let mut room = 128 * 1024;
    let valid_id = |id: &str| {
        !id.is_empty()
            && id.len() <= 200
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_:.-".contains(&b))
    };
    for row in rows.iter().take(400) {
        let id = row["id"].as_str().filter(|id| valid_id(id));
        match string(row, "kind") {
            kind @ ("say" | "recap" | "now") => {
                let text =
                    utf16_prefix(string(row, "text"), if kind == "say" { 8000 } else { 300 });
                let length = text.encode_utf16().count();
                if cos_core::config::js_trim(&text).is_empty() || length > room {
                    continue;
                }
                room -= length;
                let mut item = json!({"kind":kind,"text":text});
                if kind == "say" {
                    item["done"] = json!(row["done"] == true);
                    if let Some(id) = id {
                        item["id"] = json!(id);
                    }
                }
                out.push(item);
            }
            "answer" if id.is_some() => out.push(json!({"kind":"answer","id":id.unwrap()})),
            kind @ ("call" | "exec") if id.is_some() => {
                let mut item = json!({"kind":kind,"id":id.unwrap(),"done":row["done"]==true});
                if kind == "call" {
                    if let Some(tool) = row["tool"].as_str().filter(|tool| {
                        !tool.is_empty()
                            && tool.len() <= 100
                            && tool
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                    }) {
                        item["tool"] = json!(tool);
                    }
                    if let Some(at) = row["at"].as_f64().filter(|at| at.is_finite() && *at > 0.0) {
                        item["at"] = json!(at.round() as i64);
                    }
                }
                out.push(item);
            }
            _ => {}
        }
    }
    (!out.is_empty()).then_some(out)
}
fn valid_handoff(value: &Value, session: &str, name: &str) -> bool {
    if !model::session_id(session) || !model::session_id(name) || !value["text"].is_string() {
        return false;
    }
    if value.get("version").is_none() && value.get("provenance").is_none() {
        return true;
    }
    let p = &value["provenance"];
    value["version"] == 1
        && value["id"] == name
        && value["sessionId"] == session
        && p.is_object()
        && (p["sourceConversationId"].is_null()
            || p["sourceConversationId"]
                .as_str()
                .is_some_and(|s| !s.is_empty() && s.encode_utf16().count() <= 256))
        && (p["sourceGeneration"].is_null()
            || p["sourceGeneration"]
                .as_u64()
                .is_some_and(|n| (1..=9_007_199_254_740_991).contains(&n)))
        && (p["sourceTurnId"].is_null()
            || p["sourceTurnId"]
                .as_str()
                .is_some_and(|s| s.encode_utf16().count() <= 256))
        && (p["continuationId"].is_null()
            || p["continuationId"].as_str().is_some_and(|s| {
                s.len() == 43
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            }))
}
pub fn validate_plan(mut plan: Value, persisted: bool) -> io::Result<Value> {
    let Some(map) = plan.as_object() else {
        return Err(invalid("Invalid plan"));
    };
    if map
        .keys()
        .any(|k| !(matches!(k.as_str(), "plan" | "explanation") || persisted && k == "updatedAt"))
    {
        return Err(invalid("Unknown plan field"));
    }
    if persisted
        && plan["updatedAt"]
            .as_f64()
            .is_none_or(|n| !n.is_finite() || n < 0.0)
    {
        return Err(invalid("Invalid plan clock"));
    }
    if let Some(explanation) = plan.get("explanation") {
        let text = explanation
            .as_str()
            .ok_or_else(|| invalid("Invalid plan explanation"))?;
        let text = cos_core::config::js_trim(text);
        if cos_core::config::js_len(text) > 1000 {
            return Err(invalid("Plan explanation too long"));
        }
        plan["explanation"] = json!(text);
    }
    let steps = plan["plan"]
        .as_array_mut()
        .filter(|steps| steps.len() <= 12)
        .ok_or_else(|| invalid("Invalid plan steps"))?;
    let mut in_progress = 0;
    let mut length = 0;
    for step in steps {
        let Some(map) = step.as_object() else {
            return Err(invalid("Invalid plan step"));
        };
        if map
            .keys()
            .any(|k| !matches!(k.as_str(), "step" | "status" | "details"))
        {
            return Err(invalid("Unknown plan step field"));
        }
        let title = step["step"]
            .as_str()
            .map(cos_core::config::js_trim)
            .filter(|s| !s.is_empty() && cos_core::config::js_len(s) <= 160)
            .ok_or_else(|| invalid("Invalid plan headline"))?
            .to_owned();
        length += title.encode_utf16().count();
        step["step"] = json!(title);
        if let Some(details) = step.get("details") {
            let details = details
                .as_str()
                .map(cos_core::config::js_trim)
                .filter(|s| cos_core::config::js_len(s) <= 2000)
                .ok_or_else(|| invalid("Invalid plan details"))?
                .to_owned();
            length += details.encode_utf16().count();
            step["details"] = json!(details);
        }
        match string(step, "status") {
            "in_progress" => in_progress += 1,
            "pending" | "completed" => {}
            _ => return Err(invalid("Invalid plan status")),
        }
    }
    if in_progress > 1 || length > 12000 {
        return Err(invalid("Invalid complete plan size or active steps"));
    }
    Ok(plan)
}
fn collect_identity(event: &Value, out: &mut Vec<Value>) {
    if matches!(
        string(event, "kind"),
        "user_message" | "turn_start" | "turn_end" | "tool_call"
    ) {
        let mut identity = event.clone();
        identity.as_object_mut().unwrap().remove("message");
        if let Some(call) = identity["call"].as_object_mut() {
            call.remove("args");
            call.remove("result");
        }
        out.push(identity);
    }
}
fn directory_bytes(path: &Path) -> io::Result<u64> {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut total = 0;
    for entry in entries {
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if meta.is_file() && !is_link(&meta) {
            total += meta.len();
        }
    }
    Ok(total)
}
fn valid_asset(asset: &str) -> bool {
    let Some((hash, extension)) = asset.rsplit_once('.') else {
        return false;
    };
    (8..=64).contains(&hash.len())
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && matches!(extension, "png" | "jpg" | "txt" | "bin")
}
fn utf16_prefix(text: &str, limit: usize) -> String {
    let mut units = 0;
    text.chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= limit
        })
        .collect()
}
