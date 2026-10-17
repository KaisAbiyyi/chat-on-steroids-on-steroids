//! Durable native-project creation leases. No browser action is performed by this owner.
use super::*;
use std::{
    thread::JoinHandle,
    time::{SystemTime, UNIX_EPOCH},
};
const LEASE_MS: u64 = 60_000;
const COLORS: [&str; 6] = ["blue", "green", "amber", "purple", "rose", "teal"];
type Listeners = Arc<Mutex<Vec<mpsc::SyncSender<()>>>>;
pub(super) struct Sweep {
    wake: mpsc::SyncSender<()>,
    task: JoinHandle<()>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn native_id(id: &str) -> bool {
    id.len() == 36
        && id.starts_with("g-p-")
        && id[4..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn uuid(id: &str) -> bool {
    id.len() == 36 && uuid::Uuid::parse_str(id).is_ok()
}
fn bounded(value: &Value, min: usize, max: usize) -> bool {
    value
        .as_str()
        .is_some_and(|s| (min..=max).contains(&cos_core::config::js_len(s)))
}
pub(super) fn valid_project(p: &Value) -> bool {
    if !bounded(&p["path"], 1, 32768)
        || p.get("color")
            .is_some_and(|c| c.as_str().is_none_or(|s| !COLORS.contains(&s)))
        || p.get("ungrouped").is_some_and(|v| !v.is_boolean())
    {
        return false;
    }
    if let Some(r) = p.get("remote")
        && (!bounded(&r["host"], 1, 255)
            || !bounded(&r["path"], 1, 4096)
            || r.get("port")
                .is_some_and(|v| v.as_u64().is_none_or(|n| !(1..=65535).contains(&n))))
    {
        return false;
    }
    let Some(link) = p.get("chatgpt") else {
        return true;
    };
    if link["updatedAt"]
        .as_f64()
        .is_none_or(|n| !n.is_finite() || n < 0.)
    {
        return false;
    }
    match link["state"].as_str() {
        Some("linked") => link["id"].as_str().is_some_and(native_id),
        Some(state @ ("requested" | "claimed" | "creating" | "failed" | "uncertain")) => {
            link["requestId"].as_str().is_some_and(uuid)
                && (!matches!(state, "claimed" | "creating") || bounded(&link["owner"], 1, 64))
                && (!matches!(state, "failed" | "uncertain") || bounded(&link["error"], 0, 500))
        }
        _ => false,
    }
}
fn emit(listeners: &Listeners) {
    if let Ok(mut listeners) = listeners.lock() {
        listeners.retain(|tx| !matches!(tx.try_send(()), Err(mpsc::TrySendError::Disconnected(_))));
    }
}
fn lapsed(link: &Value, at: u64) -> bool {
    link["state"] == "claimed"
        && at.saturating_sub(link["updatedAt"].as_u64().unwrap_or(u64::MAX)) >= LEASE_MS
}
impl Projects {
    pub fn set_color(&self, id: &str, color: Option<&str>) -> io::Result<Value> {
        if !uuid(id) || color.is_some_and(|c| !COLORS.contains(&c)) {
            return Err(invalid("Invalid project color or identity"));
        }
        let _guard = lock(&self.mutation)?;
        let mut rows = self.list()?.as_array().unwrap().clone();
        let p = rows
            .iter_mut()
            .find(|p| p["id"] == id)
            .ok_or_else(|| invalid("Project not found"))?;
        if p["color"].as_str() == color {
            return Ok(p.clone());
        }
        p.as_object_mut().unwrap().remove("color");
        if let Some(color) = color {
            p["color"] = json!(color);
        }
        let result = p.clone();
        self.durable.write_now("projects", json!(rows))?;
        Ok(result)
    }
    pub fn remove(&self, id: &str) -> io::Result<Value> {
        if !uuid(id) {
            return Err(invalid("Invalid project identity"));
        }
        // Soft removal retains all durable workspace and request identities.
        self.update(id, None, Some(true))
    }
    pub fn subscribe_link_changes(&self) -> io::Result<mpsc::Receiver<()>> {
        let mut listeners = lock(&self.link_watchers)?;
        if listeners.len() >= 64 {
            return Err(invalid("Project link listener limit reached"));
        }
        let (tx, rx) = mpsc::sync_channel(1);
        listeners.push(tx);
        Ok(rx)
    }
    fn change_link(
        &self,
        matching: impl Fn(&Value, &Value) -> bool,
        next: impl Fn(&Value) -> Option<Value>,
    ) -> io::Result<Option<Value>> {
        let _guard = lock(&self.mutation)?;
        let mut rows = self.list()?.as_array().unwrap().clone();
        let Some(project) = rows
            .iter_mut()
            .find(|p| p.get("chatgpt").is_some_and(|l| matching(l, p)))
        else {
            return Ok(None);
        };
        let Some(changed) = next(&project["chatgpt"]) else {
            return Ok(None);
        };
        project["chatgpt"] = changed;
        if !valid_project(project) {
            return Err(invalid("Project link update is invalid"));
        }
        let result = project.clone();
        self.durable.write_now("projects", json!(rows))?;
        emit(&self.link_watchers);
        Ok(Some(result))
    }
    pub fn create_transition(
        &self,
        request: &str,
        action: &str,
        owner: &str,
        value: &str,
        at: u64,
    ) -> io::Result<bool> {
        if !matches!(action, "claim" | "arm" | "complete" | "fail") {
            return Err(invalid("Unknown project creation action"));
        }
        if owner.is_empty() || cos_core::config::js_len(owner) > 64 {
            return Err(invalid("Invalid project creation owner"));
        }
        if action == "complete" && !native_id(value) {
            return Ok(false);
        }
        let changed=self.change_link(|link,_|link["requestId"]==request,|link| {
            let owned=link["owner"]==owner;
            match action {
                "claim" if link["state"]=="requested" || (link["state"]=="claimed"&&(owned||lapsed(link,at)))=>Some(json!({"state":"claimed","requestId":request,"owner":owner,"updatedAt":at})),
                "arm" if link["state"]=="claimed"&&owned&&!lapsed(link,at)=>Some(json!({"state":"creating","requestId":request,"owner":owner,"updatedAt":at})),
                "complete" if link["state"]=="creating"&&owned=>Some(json!({"state":"linked","id":value,"updatedAt":at})),
                "fail" if owned&&(link["state"]=="claimed"||link["state"]=="creating")=>Some(json!({"state":if link["state"]=="creating"{"uncertain"}else{"failed"},"requestId":request,"error":if value.is_empty(){"unknown error".to_owned()}else{String::from_utf16_lossy(&value.encode_utf16().take(500).collect::<Vec<_>>())},"updatedAt":at})),
                _=>None,
            }
        })?.is_some();
        if changed && action == "arm" {
            self.expire_armed(at)?;
            self.resume_expiry()?;
        }
        Ok(changed)
    }
    pub fn retry_create(&self, id: &str, at: u64) -> io::Result<Option<Value>> {
        if !uuid(id) {
            return Err(invalid("Invalid project identity"));
        }
        self.change_link(|_,p|p["id"]==id,|link|matches!(link["state"].as_str(),Some("failed"|"uncertain")).then(||json!({"state":"requested","requestId":uuid::Uuid::new_v4().to_string(),"updatedAt":at})))
    }
    pub fn mark_gone(&self, id: &str, native: &str, at: u64) -> io::Result<bool> {
        Ok(self.change_link(|link,p|p["id"]==id&&link["state"]=="linked"&&link["id"]==native,|_|Some(json!({"state":"failed","requestId":uuid::Uuid::new_v4().to_string(),"error":"ChatGPT no longer opens this Project","updatedAt":at})))?.is_some())
    }
    pub fn pending_creates(&self, at: u64) -> io::Result<Value> {
        self.expire_armed(at)?;
        Ok(json!(self.list()?.as_array().unwrap().iter().filter_map(|p|{
            let l=&p["chatgpt"];
            if p["ungrouped"]==true||!matches!(l["state"].as_str(),Some("requested"|"claimed"|"creating")){return None;}
            Some(json!({"id":l["requestId"],"projectId":p["id"],"name":p["name"],"state":if lapsed(l,at){json!("requested")}else{l["state"].clone()}}))
        }).collect::<Vec<_>>()))
    }
    pub fn expire_armed(&self, at: u64) -> io::Result<()> {
        expire(&self.durable, &self.mutation, &self.link_watchers, at).map(|_| ())
    }
    pub(super) fn resume_expiry(&self) -> io::Result<()> {
        if !self
            .list()?
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["chatgpt"]["state"] == "creating")
        {
            return Ok(());
        }
        let mut sweep = lock(&self.sweep)?;
        if let Some(s) = sweep.as_ref()
            && !s.task.is_finished()
            && !matches!(
                s.wake.try_send(()),
                Err(mpsc::TrySendError::Disconnected(_))
            )
        {
            return Ok(());
        }
        if let Some(s) = sweep.take() {
            drop(s.wake);
            let _ = s.task.join();
        }
        let durable = self.durable.clone();
        let mutation = self.mutation.clone();
        let listeners = self.link_watchers.clone();
        let (wake, rx) = mpsc::sync_channel(1);
        let task = std::thread::spawn(move || {
            loop {
                match expire(&durable, &mutation, &listeners, now()) {
                    Ok(Some(next)) => {
                        if matches!(
                            rx.recv_timeout(Duration::from_millis(
                                next.saturating_sub(now()).saturating_add(50)
                            )),
                            Err(mpsc::RecvTimeoutError::Disconnected)
                        ) {
                            break;
                        }
                    }
                    Ok(None) => {
                        if rx.recv().is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        *sweep = Some(Sweep { wake, task });
        Ok(())
    }
}
fn expire(
    durable: &DurableStore,
    mutation: &Mutex<()>,
    listeners: &Listeners,
    at: u64,
) -> io::Result<Option<u64>> {
    let _guard = lock(mutation)?;
    let mut rows = durable.read("projects", true)?.unwrap_or(json!([]));
    let rows = rows
        .as_array_mut()
        .ok_or_else(|| invalid("Project catalog is invalid"))?;
    let mut next = None;
    let mut changed = 0;
    for p in rows.iter_mut() {
        if !valid_project(p) {
            return Err(invalid("Project catalog is invalid"));
        }
        let Some(link) = p.get_mut("chatgpt") else {
            continue;
        };
        if link["state"] != "creating" {
            continue;
        }
        let deadline = link["updatedAt"]
            .as_u64()
            .unwrap_or(u64::MAX)
            .saturating_add(LEASE_MS);
        if at >= deadline {
            *link = json!({"state":"uncertain","requestId":link["requestId"],"error":"ChatGPT did not show the new Project in time","updatedAt":at});
            changed += 1;
        } else {
            next = Some(next.map_or(deadline, |n: u64| n.min(deadline)));
        }
    }
    if changed > 0 {
        durable.write_now("projects", json!(rows))?;
        for _ in 0..changed {
            emit(listeners);
        }
    }
    Ok(next)
}
impl Drop for Projects {
    fn drop(&mut self) {
        if let Ok(sweep) = self.sweep.get_mut()
            && let Some(s) = sweep.take()
        {
            drop(s.wake);
            let _ = s.task.join();
        }
    }
}
