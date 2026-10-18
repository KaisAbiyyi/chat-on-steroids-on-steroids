//! No tool-name, tab, timing or agent payload may select a request owner.
use crate::{
    CreateOptions, ReadOptions, SessionStore, UpsertOptions,
    correlation::Correlations,
    model::{self, string},
};
use cos_core::persistence::{invalid, lock};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
pub struct Recorder {
    store: SessionStore,
    correlations: Arc<Correlations>,
    bindings: Mutex<HashMap<String, String>>,
    enabled: AtomicBool,
    gate: Mutex<()>,
}
pub struct ToolRecordContext {
    pub call_id: String,
    pub parent_call_id: Option<String>,
    pub duration_ms: u64,
}
impl Recorder {
    pub fn new(store: SessionStore, correlations: Arc<Correlations>) -> Self {
        Self {
            store,
            correlations,
            bindings: Mutex::new(HashMap::new()),
            enabled: AtomicBool::new(true),
            gate: Mutex::new(()),
        }
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
    fn session_locked(&self, conversation: &str, at: i64) -> io::Result<Option<String>> {
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(None);
        }
        if conversation.is_empty() || conversation.len() > 200 {
            return Err(invalid("Invalid observed conversation id"));
        }
        let bound = { lock(&self.bindings)?.get(conversation).cloned() };
        if let Some(id) = bound {
            let summary = self.store.summary(&id)?;
            if summary
                .as_ref()
                .is_some_and(|s| s["conversationId"] == conversation)
            {
                return Ok(Some(id));
            }
            lock(&self.bindings)?.remove(conversation);
        }
        if let Some(summary) = self.store.find_conversation(conversation)? {
            let id = string(&summary, "id").to_owned();
            self.store.reopen(&id, at)?;
            lock(&self.bindings)?.insert(conversation.into(), id.clone());
            return Ok(Some(id));
        }
        if self.store.superseded(conversation)? {
            return Ok(None);
        }
        let summary = self.store.create(CreateOptions {
            conversation_id: Some(conversation.into()),
            now: Some(at),
            ..Default::default()
        })?;
        let id = string(&summary, "id").to_owned();
        lock(&self.bindings)?.insert(conversation.into(), id.clone());
        Ok(Some(id))
    }
    pub fn session_for(&self, conversation: &str, at: i64) -> io::Result<Option<String>> {
        let _guard = lock(&self.gate)?;
        self.session_locked(conversation, at)
    }
    pub fn observe(
        &self,
        conversation: &str,
        observations: &[Value],
        at: i64,
    ) -> io::Result<Option<String>> {
        let _guard = lock(&self.gate)?;
        let Some(id) = self.session_locked(conversation, at)? else {
            return Ok(None);
        };
        let history = self.store.read(
            &id,
            ReadOptions {
                kinds: vec!["turn_start".into(), "turn_end".into()],
                ..Default::default()
            },
        )?;
        let mut starts = HashSet::<String>::new();
        let mut ends = HashSet::<String>::new();
        for event in history {
            let turn = string(&event, "turnId").to_owned();
            if event["kind"] == "turn_start" {
                starts.insert(turn.clone());
                ends.remove(&turn);
            } else {
                ends.insert(turn);
            }
        }
        let mut page_title = None;
        for observation in observations {
            let mut event = observation.clone();
            event["source"] = json!("extension");
            event["conversationId"] = json!(conversation);
            let turn = string(&event, "turnId").to_owned();
            match string(&event, "kind") {
                "conversation_title" => {
                    page_title = observation["text"].as_str();
                }
                "model_selection" => {
                    if let Some(model) = event["model"].as_str() {
                        self.store.observe_model(
                            &id,
                            conversation,
                            model,
                            event["time"].as_f64().unwrap_or(f64::NAN),
                            event["reasoningEffort"].as_str(),
                        )?;
                    }
                }
                "turn_start" => {
                    if !turn.is_empty() && !starts.contains(&turn) && !ends.contains(&turn) {
                        self.store.append(&id, event)?;
                        starts.insert(turn);
                    }
                }
                "turn_end" => {
                    if starts.contains(&turn) && !ends.contains(&turn) {
                        self.store.append(&id, event)?;
                        ends.insert(turn);
                    }
                }
                "user_message" | "assistant_message" => {
                    let prefer_time = event["authoredTime"] == true;
                    if event.get("message").is_none() {
                        if string(&event, "messageId").is_empty() {
                            continue;
                        }
                        if event.get("text").is_some_and(|text| !text.is_string()) {
                            return Err(invalid("Invalid native message text"));
                        }
                        let text = event["text"].as_str().unwrap_or("");
                        event["message"] = json!({"text":text,"truncated":false,"chars":text.encode_utf16().count()});
                        event.as_object_mut().unwrap().remove("text");
                        event.as_object_mut().unwrap().remove("authoredTime");
                    }
                    let final_message =
                        event["kind"] == "assistant_message" && model::terminal(&event);
                    if let Some(text) = event["message"]["text"].as_str()
                        && event["message"]["truncated"] != true
                    {
                        event["message"] = self.store.store_text(
                            &id,
                            text,
                            if event["kind"] == "user_message" {
                                256000
                            } else {
                                12000
                            },
                        )?;
                    }
                    self.store.upsert(
                        &id,
                        event,
                        UpsertOptions {
                            prefer_time,
                            now: Some(at),
                            work: Some(!ends.contains(&turn)),
                        },
                    )?;
                    if final_message && starts.contains(&turn) && !ends.contains(&turn) {
                        self.store.append(&id,json!({"kind":"turn_end","source":"extension","time":at,"turnId":turn,"conversationId":conversation,"outcome":"completed"}))?;
                        ends.insert(turn);
                    }
                }
                "progress" | "page_tool" | "chat_error" => {
                    self.store.append(&id, event)?;
                }
                _ => return Err(invalid("Unsupported browser observation kind")),
            }
        }
        if let Some(title) = page_title
            .map(cos_core::config::js_trim)
            .filter(|s| !s.is_empty())
        {
            self.store.rename(
                &id,
                title,
                crate::title::TitleSource::Provider,
                Some(conversation),
            )?;
        }
        Ok(Some(id))
    }
    pub fn record_call(
        &self,
        request: &str,
        tool: &str,
        args: Value,
        result: &str,
        started_at: i64,
        outcome: &str,
    ) -> io::Result<Value> {
        self.record_call_details(request, tool, args, result, started_at, outcome, None)
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "Retains the phase-3 recording API while accepting nested-call provenance"
    )]
    pub fn record_call_details(
        &self,
        request: &str,
        tool: &str,
        args: Value,
        result: &str,
        started_at: i64,
        outcome: &str,
        context: Option<&ToolRecordContext>,
    ) -> io::Result<Value> {
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(Value::Null);
        }
        if !matches!(
            outcome,
            "ok" | "process_exit_nonzero"
                | "tool_rejected"
                | "tool_execution_error"
                | "tool_internal_error"
        ) {
            return Err(invalid("Invalid tool outcome"));
        }
        let _guard = lock(&self.gate)?;
        let owner = self.correlations.get(request)?;
        let (id, conversation, method) = if let Some(owner) = owner {
            if self.store.summary(&owner.session_id)?.is_none() {
                return Err(invalid("Proven request epoch is unavailable"));
            }
            (owner.session_id, Some(owner.conversation_id), "request_id")
        } else {
            let id = "unattributed-activity";
            if self.store.summary(id)?.is_none() {
                self.store.create(CreateOptions {
                    id: Some(id.into()),
                    title: Some("Unattributed activity".into()),
                    origin: Some(json!({"kind":"unattributed"})),
                    now: Some(started_at),
                    ..Default::default()
                })?;
            }
            (id.into(), None, "unattributed")
        };
        // Sanitise BEFORE store_text: its overflow path persists a full asset.
        // Raw data never enters the event, returned call, caption or asset store.
        let safe_args = crate::redaction::arguments(tool, &args);
        let safe_result = crate::redaction::result(tool, &args, result);
        let args = self
            .store
            .store_text(&id, &serde_json::to_string(&safe_args)?, 8000)?;
        let result = self.store.store_text(&id, &safe_result, 8000)?;
        let mut call = json!({"callId":context.map(|c|c.call_id.clone()).unwrap_or_else(||uuid::Uuid::new_v4().to_string()),"tool":tool,"requestId":if request.is_empty(){None}else{Some(request)},"conversationId":conversation,"attributionMethod":method,"attribution":if method=="request_id"{"request_id"}else{"unattributed"},"startedAt":started_at,"durationMs":context.map(|c|c.duration_ms).unwrap_or(0),"args":args,"result":result,"outcome":outcome,"summary":{"kind":"tool","title":tool}});
        if let Some(parent) = context.and_then(|c| c.parent_call_id.as_deref()) {
            call["nested"] = json!(true);
            call["parentCallId"] = json!(parent);
        }
        let event = self.store.append(
            &id,
            json!({"kind":"tool_call","source":"mcp","time":started_at,"call":call}),
        )?;
        Ok(json!({"sessionId":id,"call":call,"event":event}))
    }
    pub fn flush(&self) -> io::Result<()> {
        self.store.flush()
    }
}
