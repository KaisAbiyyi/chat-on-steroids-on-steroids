//! First proven request ownership. Commit is the acknowledgement barrier.
use crate::{SessionStore, model};
use cos_core::{
    config::js_len,
    persistence::{DurableStore, invalid, lock},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Correlation {
    pub request_id: String,
    pub conversation_id: String,
    pub session_id: String,
    pub message_id: String,
    pub tool: String,
    pub observed_at: i64,
}
impl Correlation {
    fn valid(&self) -> bool {
        (1..=200).contains(&js_len(&self.request_id))
            && (1..=200).contains(&js_len(&self.conversation_id))
            && model::session_id(&self.session_id)
            && (1..=300).contains(&js_len(&self.message_id))
            && js_len(&self.tool) <= 100
    }
}
#[derive(Clone, Default)]
struct State {
    owners: HashMap<String, Correlation>,
    order: VecDeque<String>,
    windows: HashMap<String, Instant>,
}
fn merge(state: &mut State, input: &Correlation) -> &'static str {
    if !input.valid() {
        return "refused";
    }
    if let Some(previous) = state.owners.get_mut(&input.request_id) {
        if previous.conversation_id != input.conversation_id {
            return "refused";
        }
        if input.observed_at > previous.observed_at {
            previous.observed_at = input.observed_at;
            state.order.retain(|id| id != &input.request_id);
            state.order.push_back(input.request_id.clone());
        }
        return "same";
    }
    state.owners.insert(input.request_id.clone(), input.clone());
    state.order.push_back(input.request_id.clone());
    state.windows.remove(&input.request_id);
    while state.owners.len() > 50000 {
        if let Some(first) = state.order.pop_front() {
            state.owners.remove(&first);
        }
    }
    "stored"
}
fn snapshot(state: &State) -> Value {
    json!({"version":6,"complete":true,"entries":state.order.iter().filter_map(|id|state.owners.get(id)).collect::<Vec<_>>()})
}
pub struct Correlations {
    durable: DurableStore,
    state: Mutex<State>,
    changed: Condvar,
}
impl Correlations {
    pub fn restore(durable: DurableStore, store: &SessionStore) -> io::Result<Self> {
        let saved = durable.read("request-correlations", true)?;
        let mut state = State::default();
        let mut complete = false;
        if let Some(saved) = saved {
            let version = saved["version"].as_u64().unwrap_or(0);
            if !(3..=6).contains(&version) || !saved["entries"].is_array() {
                return Err(invalid(
                    "Unsupported request correlation ledger; history was preserved",
                ));
            }
            let entries = saved["entries"].as_array().unwrap();
            complete = version == 6 && saved["complete"] == true;
            for raw in entries.iter().skip(entries.len().saturating_sub(50000)) {
                let raw = raw.get("value").unwrap_or(raw);
                match serde_json::from_value::<Correlation>(raw.clone()) {
                    Ok(owner) if owner.valid() => {
                        merge(&mut state, &owner);
                    }
                    _ => complete = false,
                }
            }
        }
        if !complete {
            let sessions = store.list()?;
            for session in sessions.into_iter().take(100).rev() {
                let id = model::string(&session, "id");
                for event in store.recent(id, 1024, None)? {
                    let call = &event["call"];
                    if call["attributionMethod"] != "request_id" {
                        continue;
                    }
                    let owner = Correlation {
                        request_id: model::string(call, "requestId").into(),
                        conversation_id: model::string(call, "conversationId").into(),
                        session_id: id.into(),
                        message_id: format!("stored:{}", model::string(call, "callId")),
                        tool: model::string(call, "tool").into(),
                        observed_at: event["time"].as_i64().unwrap_or(0),
                    };
                    merge(&mut state, &owner);
                }
            }
            durable.write_now("request-correlations", snapshot(&state))?;
        }
        Ok(Self {
            durable,
            state: Mutex::new(state),
            changed: Condvar::new(),
        })
    }
    pub fn get(&self, request: &str) -> io::Result<Option<Correlation>> {
        Ok(lock(&self.state)?.owners.get(request).cloned())
    }
    pub fn observe_and_commit(&self, inputs: &[Correlation]) -> io::Result<Vec<&'static str>> {
        let mut state = lock(&self.state)?;
        let mut staged = state.clone();
        let results = inputs
            .iter()
            .map(|input| merge(&mut staged, input))
            .collect::<Vec<_>>();
        if let Err(error) = self
            .durable
            .write_now("request-correlations", snapshot(&staged))
        {
            self.durable
                .queue("request-correlations", snapshot(&state))?;
            return Err(error);
        }
        *state = staged;
        self.changed.notify_all();
        Ok(results)
    }
    pub fn commit(&self) -> io::Result<()> {
        self.durable
            .write_now("request-correlations", snapshot(&*lock(&self.state)?))
    }
    pub fn wait(&self, request: &str, budget: Duration) -> io::Result<Option<Correlation>> {
        if request.is_empty() {
            return Ok(None);
        }
        let mut state = lock(&self.state)?;
        let start = *state
            .windows
            .entry(request.into())
            .or_insert_with(Instant::now);
        if state.windows.len() > 2000
            && let Some(oldest) = state
                .windows
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(id, _)| id.clone())
        {
            state.windows.remove(&oldest);
        }
        loop {
            if let Some(owner) = state.owners.get(request) {
                return Ok(Some(owner.clone()));
            }
            let remaining = budget.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Ok(None);
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| io::Error::other("Correlation owner failed"))?;
            state = next;
        }
    }
}
