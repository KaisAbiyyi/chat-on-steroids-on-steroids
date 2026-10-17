//! Durable barriers: publication follows successful commit, not a scheduled write.
use fs2::FileExt;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::Duration,
};

pub fn lock<T>(value: &Mutex<T>) -> io::Result<MutexGuard<'_, T>> {
    value
        .lock()
        .map_err(|_| io::Error::other("State owner failed; restart required"))
}
pub fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
pub fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
pub fn owned_path(directory: &Path, relative: &Path, allow_missing: bool) -> io::Result<PathBuf> {
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid("Invalid data-store path"));
    }
    let root = fs::canonicalize(directory)?;
    let mut result = root.clone();
    let mut missing = false;
    for component in relative.components() {
        result.push(component.as_os_str());
        if missing {
            continue;
        }
        match fs::symlink_metadata(&result) {
            Ok(meta) if is_link(&meta) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Data path contains a link",
                ));
            }
            Ok(_) => {
                if !fs::canonicalize(&result)?.starts_with(&root) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Data path escaped its owner",
                    ));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound && allow_missing => missing = true,
            Err(e) => return Err(e),
        }
    }
    Ok(result)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Missing write parent"))?;
    fs::create_dir_all(parent)?;
    if let Ok(meta) = fs::symlink_metadata(path)
        && (is_link(&meta) || !meta.is_file())
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Write target is not a regular file",
        ));
    }
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
struct ConfigState {
    value: Value,
    recovered: bool,
}
pub struct ConfigStore {
    directory: PathBuf,
    state: Mutex<ConfigState>,
}
impl ConfigStore {
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn open(directory: &Path, platform: &str) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let directory = fs::canonicalize(directory)?;
        let path = owned_path(&directory, Path::new("config.json"), true)?;
        let mut backup = false;
        let mut recovered = false;
        let value = match fs::read(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let mut v = crate::fresh_config(platform);
                v["ui"]["lastSeenVersion"] = json!(crate::APP_VERSION);
                v
            }
            Err(_) => {
                recovered = true;
                crate::config::recovery_config()
            }
            Ok(raw) => {
                let parsed = std::str::from_utf8(&raw).ok().and_then(|text| {
                    serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok()
                });
                let result = parsed.and_then(|mut value| {
                    for _ in 0..8 {
                        match crate::config::parse(value.clone()) {
                            Ok(valid) => return Some(crate::config::migrate_loaded(valid)),
                            Err(issues) => {
                                let mut removed = false;
                                for issue in issues {
                                    let key = issue
                                        .path
                                        .strip_prefix("ui.")
                                        .and_then(|p| p.split('.').next())?;
                                    let ui = value.get_mut("ui").and_then(Value::as_object_mut)?;
                                    ui.remove(key)?;
                                    removed = true;
                                    backup = true;
                                }
                                if !removed {
                                    return None;
                                }
                            }
                        }
                    }
                    None
                });
                if backup || result.is_none() {
                    let name = format!(
                        "config.json.unreadable-{}-{}",
                        chrono::Utc::now().format("%Y%m%d-%H%M%S%3f"),
                        uuid::Uuid::new_v4().simple()
                    );
                    let target = owned_path(&directory, Path::new(&name), true)?;
                    atomic_write(&target, &raw)?;
                }
                result.unwrap_or_else(|| {
                    recovered = true;
                    crate::config::recovery_config()
                })
            }
        };
        Ok(Self {
            directory,
            state: Mutex::new(ConfigState { value, recovered }),
        })
    }
    pub fn get(&self) -> Value {
        self.state.lock().expect("config owner").value.clone()
    }
    pub fn recovered(&self) -> bool {
        self.state.lock().expect("config owner").recovered
    }
    pub fn update(&self, edit: impl FnOnce(&mut Value)) -> io::Result<Value> {
        let mut state = lock(&self.state)?;
        let mut proposed = state.value.clone();
        edit(&mut proposed);
        let parsed = crate::config::parse(proposed)
            .map_err(|_| invalid("Invalid settings; nothing saved"))?;
        let path = owned_path(&self.directory, Path::new("config.json"), true)?;
        atomic_write(&path, &serde_json::to_vec_pretty(&parsed)?)?;
        state.value = parsed;
        if state.value["readOnly"] == false {
            state.recovered = false;
        }
        Ok(state.value.clone())
    }
    pub fn save(&self, value: Value) -> io::Result<Value> {
        self.update(|next| *next = value)
    }
}
#[derive(Clone)]
struct Pending {
    generation: u64,
    value: Value,
}
#[derive(Default)]
struct DurableState {
    next: u64,
    pending: HashMap<String, Pending>,
    revisions: HashMap<String, u64>,
    running: HashSet<String>,
}
struct DurableInner {
    directory: PathBuf,
    state: Mutex<DurableState>,
    files: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}
#[derive(Clone)]
pub struct DurableStore {
    inner: Arc<DurableInner>,
}
impl DurableStore {
    pub fn open(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let directory = fs::canonicalize(directory)?;
        let state_dir = owned_path(&directory, Path::new("state"), true)?;
        fs::create_dir_all(state_dir)?;
        Ok(Self {
            inner: Arc::new(DurableInner {
                directory,
                state: Mutex::new(DurableState::default()),
                files: Mutex::new(HashMap::new()),
            }),
        })
    }
    fn path(&self, name: &str) -> io::Result<PathBuf> {
        if name.is_empty()
            || name.len() > 40
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(invalid("Invalid durable state name"));
        }
        owned_path(
            &self.inner.directory,
            Path::new(&format!("state/{name}.json")),
            true,
        )
    }
    fn stage(&self, name: &str, value: Value, _background: bool) -> io::Result<Pending> {
        self.path(name)?;
        let mut state = lock(&self.inner.state)?;
        state.next += 1;
        let slot = Pending {
            generation: state.next,
            value,
        };
        state.pending.insert(name.into(), slot.clone());
        Ok(slot)
    }
    fn commit(&self, name: &str, slot: Pending) -> io::Result<()> {
        let fence = self.file_fence(name)?;
        let _guard = lock(&fence)?;
        self.commit_held(name, slot, true)
    }
    fn file_fence(&self, name: &str) -> io::Result<Arc<Mutex<()>>> {
        Ok(lock(&self.inner.files)?
            .entry(name.into())
            .or_default()
            .clone())
    }
    fn commit_held(&self, name: &str, slot: Pending, require_current: bool) -> io::Result<()> {
        if require_current
            && lock(&self.inner.state)?
                .pending
                .get(name)
                .is_none_or(|current| current.generation != slot.generation)
        {
            return Ok(());
        }
        let target = self.path(name)?;
        if slot.value.is_null() {
            match fs::remove_file(&target) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        } else {
            atomic_write(&target, &serde_json::to_vec(&slot.value)?)?;
        }
        let mut state = lock(&self.inner.state)?;
        let next = state.revisions.get(name).copied().unwrap_or(0) + 1;
        state.revisions.insert(name.into(), next);
        if state
            .pending
            .get(name)
            .is_some_and(|current| current.generation == slot.generation)
        {
            state.pending.remove(name);
        }
        Ok(())
    }
    pub fn write_now(&self, name: &str, value: Value) -> io::Result<()> {
        self.path(name)?;
        let fence = self.file_fence(name)?;
        let _guard = lock(&fence)?;
        let slot = self.stage(name, value, false)?;
        let result = self.commit_held(name, slot, false);
        if result.is_err() {
            self.schedule(name)?;
        }
        result
    }
    pub fn queue(&self, name: &str, value: Value) -> io::Result<u64> {
        let slot = self.stage(name, value, true)?;
        self.schedule(name)?;
        Ok(slot.generation)
    }
    fn schedule(&self, name: &str) -> io::Result<()> {
        if !lock(&self.inner.state)?.running.insert(name.into()) {
            return Ok(());
        }
        let weak = Arc::downgrade(&self.inner);
        let name = name.to_string();
        thread::Builder::new()
            .name(format!("cos-state-{name}"))
            .spawn(move || {
                let mut delay = 300;
                loop {
                    thread::sleep(Duration::from_millis(delay));
                    let Some(inner) = weak.upgrade() else {
                        break;
                    };
                    let store = Self { inner };
                    let slot = {
                        let Ok(mut state) = store.inner.state.lock() else {
                            break;
                        };
                        match state.pending.get(&name).cloned() {
                            Some(slot) => slot,
                            None => {
                                state.running.remove(&name);
                                break;
                            }
                        }
                    };
                    delay = if store.commit(&name, slot).is_ok() {
                        300
                    } else {
                        (delay * 2).min(5000)
                    };
                }
            })?;
        Ok(())
    }
    pub fn read(&self, name: &str, strict: bool) -> io::Result<Option<Value>> {
        let path = self.path(name)?;
        match fs::read(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(_) if !strict => Ok(None),
            Err(_) => Err(io::Error::other(format!("Could not read {name} state"))),
            Ok(raw) => match serde_json::from_slice::<Value>(&raw) {
                Ok(value) if !(strict && value.is_null()) => Ok(Some(value)),
                _ if !strict => Ok(None),
                _ => Err(io::Error::other(format!("Could not read {name} state"))),
            },
        }
    }
    pub fn flush(&self) -> io::Result<()> {
        loop {
            let pending: Vec<_> = lock(&self.inner.state)?
                .pending
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if pending.is_empty() {
                return Ok(());
            }
            let results = thread::scope(|scope| {
                pending
                    .into_iter()
                    .map(|(name, slot)| scope.spawn(move || self.commit(&name, slot)))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|job| {
                        job.join()
                            .unwrap_or_else(|_| Err(io::Error::other("Durable worker failed")))
                    })
                    .collect::<Vec<_>>()
            });
            for result in results {
                result?;
            }
        }
    }
    pub fn revision(&self, name: &str) -> u64 {
        self.inner
            .state
            .lock()
            .expect("durable owner")
            .revisions
            .get(name)
            .copied()
            .unwrap_or(0)
    }
    pub fn recover(&self, name: &str, value: Value) -> io::Result<PathBuf> {
        let fence = self.file_fence(name)?;
        let _guard = lock(&fence)?;
        let target = self.path(name)?;
        if !fs::metadata(&target)?.is_file() {
            return Err(invalid("State file is unavailable"));
        }
        let backup = target.with_extension(format!("recovery-{}.json", uuid::Uuid::new_v4()));
        fs::copy(&target, &backup)?;
        let slot = self.stage(name, value, false)?;
        if let Err(error) = self.commit_held(name, slot, false) {
            self.schedule(name)?;
            return Err(error);
        }
        Ok(backup)
    }
}
pub struct InstanceLease {
    _file: File,
}

impl InstanceLease {
    pub fn acquire(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let target = owned_path(directory, Path::new(".cos-rust.lock"), true)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(target)?;
        file.try_lock_exclusive()?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn a_stale_immediate_retry_snapshot_cannot_overwrite_a_newer_commit() {
        let directory = tempfile::tempdir().unwrap();
        let store = DurableStore::open(directory.path()).unwrap();
        let old = store
            .stage("authority", json!({"grant":true}), false)
            .unwrap();
        store
            .write_now("authority", json!({"grant":false}))
            .unwrap();
        // A shutdown flush/worker may already hold this old immediate snapshot.
        store.commit("authority", old).unwrap();
        assert_eq!(
            store.read("authority", true).unwrap(),
            Some(json!({"grant":false}))
        );
    }
}
