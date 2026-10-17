//! Explicit grants are durable owners. Worker and resume trust are derived per call.
use crate::{SessionStore, model};
use cos_core::persistence::{DurableStore, invalid, lock};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    sync::Mutex,
};
#[derive(Default, Clone, PartialEq, Eq)]
pub struct WorkerOwner {
    pub owned: bool,
    pub prime: Option<String>,
}
#[derive(Default)]
struct State {
    trusted: BTreeSet<String>,
    blocked: BTreeMap<String, i64>,
}
pub struct ConversationPolicy {
    durable: DurableStore,
    state: Mutex<State>,
}
impl ConversationPolicy {
    pub fn restore(durable: DurableStore) -> io::Result<Self> {
        let mut state = State::default();
        for name in ["trusted-chats", "blocked-chats"] {
            if let Some(saved) = durable.read(name, true)? {
                if saved["version"] != 1 || !saved["entries"].is_array() {
                    return Err(invalid("Unsupported conversation authority ledger"));
                }
                let entries = saved["entries"].as_array().unwrap();
                if entries.len() > 200 {
                    return Err(invalid("Conversation authority ledger exceeds its bound"));
                }
                for entry in entries {
                    let id = if name == "trusted-chats" {
                        entry.as_str()
                    } else {
                        entry["conversationId"].as_str()
                    }
                    .filter(|id| model::session_id(id))
                    .ok_or_else(|| invalid("Invalid conversation authority entry"))?;
                    if name == "trusted-chats" {
                        state.trusted.insert(id.into());
                    } else {
                        state.blocked.insert(
                            id.into(),
                            entry["blockedAt"]
                                .as_i64()
                                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
                        );
                    }
                }
            }
        }
        Ok(Self {
            durable,
            state: Mutex::new(state),
        })
    }
    pub fn is_trusted(&self, id: &str) -> io::Result<bool> {
        Ok(lock(&self.state)?.trusted.contains(id))
    }
    pub fn is_blocked(&self, id: &str) -> io::Result<bool> {
        Ok(lock(&self.state)?.blocked.contains_key(id))
    }
    pub fn snapshot(&self) -> io::Result<Value> {
        let state = lock(&self.state)?;
        Ok(json!({"trusted":state.trusted,"blocked":state.blocked}))
    }
    pub fn set_trusted(&self, ids: &[String], next: bool) -> io::Result<()> {
        validate(ids)?;
        let mut state = lock(&self.state)?;
        let mut updated = state.trusted.clone();
        for id in ids {
            if next {
                updated.insert(id.clone());
            } else {
                updated.remove(id);
            }
        }
        if updated.len() > 200 {
            return Err(invalid("Too many trusted chats (200)"));
        }
        let saved = json!({"version":1,"entries":updated});
        if let Err(error) = self.durable.write_now("trusted-chats", saved) {
            self.durable.queue(
                "trusted-chats",
                json!({"version":1,"entries":state.trusted}),
            )?;
            return Err(error);
        }
        state.trusted = updated;
        Ok(())
    }
    pub fn set_blocked(&self, ids: &[String], next: bool, at: i64) -> io::Result<()> {
        validate(ids)?;
        let mut state = lock(&self.state)?;
        let mut updated = state.blocked.clone();
        for id in ids {
            if next {
                updated.insert(id.clone(), at);
            } else {
                updated.remove(id);
            }
        }
        if updated.len() > 200 {
            return Err(invalid("Too many blocked chats (200)"));
        }
        let snap = |blocked: &BTreeMap<String, i64>| json!({"version":1,"entries":blocked.iter().map(|(id,at)|json!({"conversationId":id,"blockedAt":at})).collect::<Vec<_>>()});
        if let Err(error) = self.durable.write_now("blocked-chats", snap(&updated)) {
            self.durable.queue("blocked-chats", snap(&state.blocked))?;
            return Err(error);
        }
        state.blocked = updated;
        Ok(())
    }
    fn exact_trust(&self, id: &str, store: &SessionStore) -> io::Result<bool> {
        if self.is_blocked(id)? {
            return Ok(false);
        }
        let summary = store.find_conversation(id)?;
        let state = lock(&self.state)?;
        if state.blocked.contains_key(id) {
            return Ok(false);
        }
        if summary
            .as_ref()
            .is_some_and(|s| s["origin"]["kind"] == "worker")
        {
            return Ok(false);
        }
        if state.trusted.contains(id) {
            return Ok(true);
        }
        if let Some(summary) = summary
            && summary["lastCommittedResumeHandoffId"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
        {
            let ids = summary["chatIds"].as_array().cloned().unwrap_or_default();
            if ids.last().is_some_and(|last| last == id) {
                for ancestor in ids.iter().rev().skip(1).filter_map(Value::as_str) {
                    if state.blocked.contains_key(ancestor) {
                        return Ok(false);
                    }
                    if state.trusted.contains(ancestor) {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
    pub fn allowed(
        &self,
        conversation: Option<&str>,
        strict: bool,
        store: &SessionStore,
        worker: impl Fn(&str) -> WorkerOwner,
    ) -> io::Result<bool> {
        if conversation.is_some_and(|id| self.is_blocked(id).unwrap_or(true)) {
            return Ok(false);
        }
        if !strict {
            return Ok(true);
        }
        let Some(id) = conversation.filter(|id| !id.is_empty()) else {
            return Ok(false);
        };
        let owner = worker(id);
        if owner.owned {
            let Some(prime) = owner.prime.as_deref() else {
                return Ok(false);
            };
            let trusted = self.exact_trust(prime, store)?;
            if worker(id) != owner || self.is_blocked(id)? {
                return Ok(false);
            }
            return Ok(trusted);
        }
        self.exact_trust(id, store)
    }
}
fn validate(ids: &[String]) -> io::Result<()> {
    if ids.is_empty() || ids.iter().any(|id| !model::session_id(id)) {
        Err(invalid("Not a ChatGPT conversation id"))
    } else {
        Ok(())
    }
}
