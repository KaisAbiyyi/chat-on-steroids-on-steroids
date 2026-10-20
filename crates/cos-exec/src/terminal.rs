//! Human renderer terminals have their own lifetime, flow control and IDs; never MCP sessions.
use crate::tree::Tree;
use portable_pty::{Child, MasterPty, PtySize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub shell: String,
    pub identity: Option<String>,
}
pub type Factory = Arc<dyn Fn(Option<&str>) -> io::Result<Launch> + Send + Sync>;
pub type Allowed = Arc<dyn Fn() -> bool + Send + Sync>;
pub type Validate = Arc<dyn Fn(Option<&str>, &Launch) -> io::Result<bool> + Send + Sync>;
pub type Emit = Arc<dyn Fn(Value) + Send + Sync>;
struct Flow {
    live: bool,
    unacked: usize,
    paused: bool,
}
struct Entry {
    project: Option<String>,
    launch: Launch,
    flow: Mutex<Flow>,
    // Serializes operations against close and rechecks live grants at use.
    operation: Mutex<()>,
    wake: Condvar,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    tree: Tree,
}
struct State {
    live: bool,
    pending: HashMap<String, uuid::Uuid>,
    entries: HashMap<String, Arc<Entry>>,
    owed: usize,
}
struct Inner {
    state: Mutex<State>,
    wake: Condvar,
    factory: Factory,
    allowed: Allowed,
    validate: Validate,
    emit: Emit,
}
pub struct Terminals(Arc<Inner>);
fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Terminal command permission or project changed",
    )
}
fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        cols: cols.clamp(2, 500),
        rows: rows.clamp(1, 300),
        pixel_width: 0,
        pixel_height: 0,
    }
}
impl Terminals {
    pub fn new(factory: Factory, allowed: Allowed, validate: Validate, emit: Emit) -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(State {
                live: true,
                pending: HashMap::new(),
                entries: HashMap::new(),
                owed: 0,
            }),
            wake: Condvar::new(),
            factory,
            allowed,
            validate,
            emit,
        }))
    }
    pub fn create(
        &self,
        id: &str,
        project: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> io::Result<Value> {
        if id.is_empty()
            || id.len() > 200
            || !(1..=1000).contains(&cols)
            || !(1..=500).contains(&rows)
            || !(self.0.allowed)()
        {
            return Err(denied());
        }
        let ticket = uuid::Uuid::new_v4();
        {
            let mut state = self.0.state.lock().unwrap();
            if !state.live
                || state.pending.contains_key(id)
                || state.entries.contains_key(id)
                || state.entries.len() + state.pending.len() >= 8
            {
                return Err(io::Error::other(
                    "Terminal limit reached or ID already open",
                ));
            }
            state.pending.insert(id.into(), ticket);
        }
        let result = self.create_pending(id, project, cols, rows, ticket);
        let mut state = self.0.state.lock().unwrap();
        if state.pending.get(id) == Some(&ticket) {
            state.pending.remove(id);
        }
        result
    }
    fn create_pending(
        &self,
        id: &str,
        project: Option<&str>,
        cols: u16,
        rows: u16,
        ticket: uuid::Uuid,
    ) -> io::Result<Value> {
        let launch = (self.0.factory)(project)?;
        {
            let state = self.0.state.lock().unwrap();
            if !state.live || state.pending.get(id) != Some(&ticket) {
                return Err(denied());
            }
        }
        if !(self.0.allowed)() || !(self.0.validate)(project, &launch)? {
            return Err(denied());
        }
        type Spawned = (
            Box<dyn Child + Send + Sync>,
            Box<dyn Read + Send>,
            Box<dyn Write + Send>,
            Box<dyn MasterPty + Send>,
            Tree,
        );
        let (mut child, reader, writer, master, tree): Spawned = {
            #[cfg(windows)]
            {
                let env =
                    std::collections::BTreeMap::from([("TERM".into(), "xterm-256color".into())]);
                let spawned = crate::winpty::spawn(
                    &crate::process::Launch {
                        owner: id,
                        program: &launch.program,
                        args: &launch.args,
                        cwd: &launch.cwd,
                        tty: true,
                        raw_cmd: launch.shell == "cmd",
                    },
                    Some(&env),
                    false,
                )?;
                (
                    spawned.child,
                    spawned.reader,
                    spawned.writer,
                    spawned.master,
                    spawned.tree,
                )
            }
            #[cfg(unix)]
            {
                use portable_pty::{CommandBuilder, native_pty_system};
                let pair = native_pty_system()
                    .openpty(size(cols, rows))
                    .map_err(io::Error::other)?;
                let mut command = CommandBuilder::new(&launch.program);
                command.args(&launch.args);
                command.cwd(&launch.cwd);
                for (key, _) in std::env::vars_os() {
                    if crate::process::secret_environment(&key.to_string_lossy()) {
                        command.env_remove(key);
                    }
                }
                command.env_remove("ELECTRON_RUN_AS_NODE");
                command.env("TERM", "xterm-256color");
                let reader = pair.master.try_clone_reader().map_err(io::Error::other)?;
                let writer = pair.master.take_writer().map_err(io::Error::other)?;
                let mut child = pair
                    .slave
                    .spawn_command(command)
                    .map_err(io::Error::other)?;
                let pid = child
                    .process_id()
                    .ok_or_else(|| io::Error::other("Missing terminal PID"))?;
                let tree = match Tree::attach(pid, false) {
                    Ok(tree) => tree,
                    Err(error) => {
                        let _ = child.kill();
                        return Err(error);
                    }
                };
                drop(pair.slave);
                (child, reader, writer, pair.master, tree)
            }
        };
        let entry = Arc::new(Entry {
            project: project.map(str::to_owned),
            launch: launch.clone(),
            flow: Mutex::new(Flow {
                live: true,
                unacked: 0,
                paused: false,
            }),
            operation: Mutex::new(()),
            wake: Condvar::new(),
            writer: Mutex::new(Some(writer)),
            master: Mutex::new(Some(master)),
            tree,
        });
        // No factory, network or callback runs while the publication lock is held.
        let validation = (|| -> io::Result<bool> {
            let valid = (self.0.allowed)() && (self.0.validate)(project, &launch)?;
            if let Some(master) = entry.master.lock().unwrap().as_ref() {
                master.resize(size(cols, rows)).map_err(io::Error::other)?;
            }
            Ok(valid)
        })();
        if !matches!(validation, Ok(true)) {
            entry.tree.terminate();
            drop(reader);
            entry.master.lock().unwrap().take();
            return Err(validation.err().unwrap_or_else(denied));
        }
        {
            let mut state = self.0.state.lock().unwrap();
            if !state.live || state.pending.get(id) != Some(&ticket) {
                entry.tree.terminate();
                drop(reader);
                entry.master.lock().unwrap().take();
                return Err(denied());
            }
            state.pending.remove(id);
            state.entries.insert(id.into(), entry.clone());
            state.owed += 1;
        }
        let owner = self.0.clone();
        let terminal = id.to_owned();
        let output = entry.clone();
        let (finished, drained) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            pump(reader, &terminal, &output, &owner.emit);
            let _ = finished.send(());
        });
        let owner = self.0.clone();
        let terminal = id.to_owned();
        std::thread::spawn(move || {
            let code = loop {
                match entry
                    .tree
                    .ready_to_reap()
                    .and_then(|ready| if ready { child.try_wait() } else { Ok(None) })
                {
                    Ok(Some(status)) => break status.exit_code(),
                    Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                    Err(_) => break 1,
                }
            };
            entry.tree.terminate();
            entry.writer.lock().unwrap().take();
            // Closing ConPTY sends EOF. A paused reader is resumed by ACK or dispose.
            let master = entry.master.lock().unwrap().take();
            drop(master);
            let _ = drained.recv();
            entry.flow.lock().unwrap().live = false;
            {
                let mut state = owner.state.lock().unwrap();
                if state
                    .entries
                    .get(&terminal)
                    .is_some_and(|e| Arc::ptr_eq(e, &entry))
                {
                    state.entries.remove(&terminal);
                }
            }
            (owner.emit)(json!({"id": terminal, "exitCode": code}));
            {
                let mut state = owner.state.lock().unwrap();
                state.owed = state.owed.saturating_sub(1);
                owner.wake.notify_all();
            }
        });
        Ok(
            json!({"id":id,"shell":launch.shell,"cwd":launch.cwd.to_string_lossy(),"projectId":project}),
        )
    }
    fn entry(&self, id: &str) -> io::Result<Arc<Entry>> {
        self.0
            .state
            .lock()
            .unwrap()
            .entries
            .get(id)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Terminal is not open"))
    }
    pub fn write(&self, id: &str, data: &str) -> io::Result<()> {
        let entry = self.entry(id)?;
        // No stale entry may write after a concurrent close has completed.
        let _operation = entry.operation.lock().unwrap();
        if !(self.0.allowed)()
            || !(self.0.validate)(entry.project.as_deref(), &entry.launch)?
            || !entry.flow.lock().unwrap().live
        {
            return Err(denied());
        }
        let mut writer = entry.writer.lock().unwrap();
        let writer = writer
            .as_mut()
            .ok_or_else(|| io::Error::other("Terminal input is closed"))?;
        writer.write_all(data.as_bytes())?;
        writer.flush()
    }
    pub fn acknowledge(&self, id: &str, count: usize) {
        if let Ok(entry) = self.entry(id) {
            let mut flow = entry.flow.lock().unwrap();
            flow.unacked = flow.unacked.saturating_sub(count);
            if flow.unacked < 65536 {
                flow.paused = false;
                entry.wake.notify_all();
            }
        }
    }
    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> io::Result<()> {
        if !(1..=1000).contains(&cols) || !(1..=500).contains(&rows) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid terminal dimensions",
            ));
        }
        let entry = self.entry(id)?;
        let _operation = entry.operation.lock().unwrap();
        if !(self.0.allowed)()
            || !(self.0.validate)(entry.project.as_deref(), &entry.launch)?
            || !entry.flow.lock().unwrap().live
        {
            return Err(denied());
        }
        if let Some(master) = entry.master.lock().unwrap().as_ref() {
            master.resize(size(cols, rows)).map_err(io::Error::other)?;
        }
        Ok(())
    }
    pub fn close(&self, id: &str) {
        let entry = {
            let mut state = self.0.state.lock().unwrap();
            state.pending.remove(id);
            state.entries.remove(id)
        };
        if let Some(entry) = entry {
            // Wait for any accepted in-flight write/resize before retirement.
            let _operation = entry.operation.lock().unwrap();
            let mut flow = entry.flow.lock().unwrap();
            flow.live = false;
            flow.paused = false;
            entry.wake.notify_all();
            drop(flow);
            entry.tree.terminate();
            entry.writer.lock().unwrap().take();
        }
    }
    pub fn dispose(&self) {
        let ids = {
            let mut state = self.0.state.lock().unwrap();
            state.live = false;
            state.pending.clear();
            state.entries.keys().cloned().collect::<Vec<_>>()
        };
        for id in ids {
            self.close(&id);
        }
    }
    pub fn exited(&self, timeout: Duration) -> bool {
        let until = Instant::now() + timeout;
        let mut state = self.0.state.lock().unwrap();
        while state.owed != 0 {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            state = self.0.wake.wait_timeout(state, left).unwrap().0;
        }
        true
    }
}
impl Drop for Terminals {
    fn drop(&mut self) {
        self.dispose();
        self.exited(Duration::from_secs(3));
    }
}
fn pump(mut reader: Box<dyn Read + Send>, id: &str, entry: &Entry, emit: &Emit) {
    let mut buffer = [0u8; 8192];
    let mut pending = Vec::new();
    loop {
        {
            let mut flow = entry.flow.lock().unwrap();
            while flow.live && flow.paused {
                flow = entry.wake.wait(flow).unwrap();
            }
            if !flow.live {
                break;
            }
        }
        let count = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        pending.extend_from_slice(&buffer[..count]);
        let valid = match std::str::from_utf8(&pending) {
            Ok(_) => pending.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => pending.len(),
        };
        let data = String::from_utf8_lossy(&pending[..valid]).into_owned();
        pending.drain(..valid);
        if data.is_empty() {
            continue;
        }
        {
            let mut flow = entry.flow.lock().unwrap();
            flow.unacked = flow.unacked.saturating_add(data.encode_utf16().count());
            flow.paused = flow.unacked >= 262144;
        }
        emit(json!({"id": id, "data": data}));
    }
    if !pending.is_empty() && entry.flow.lock().unwrap().live {
        emit(json!({"id": id, "data": String::from_utf8_lossy(&pending)}));
    }
}
