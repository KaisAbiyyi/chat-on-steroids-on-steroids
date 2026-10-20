use crate::{buffer::HeadTail, shell, tree::Tree};
use cos_core::persistence::{invalid, lock};
#[cfg(unix)]
use portable_pty::{CommandBuilder, native_pty_system};
use portable_pty::{MasterPty, PtySize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

pub const MAX_PROCESSES: usize = 64;
pub const OUTPUT_BYTES: usize = 1024 * 1024;
thread_local! { static PAGES: std::cell::RefCell<Option<Vec<Delivery>>> = const { std::cell::RefCell::new(None) }; }
struct Delivery {
    session: Arc<Session>,
    unread: HeadTail,
    cursor: u64,
    exit_read: bool,
}
pub struct Publication {
    pages: Vec<Delivery>,
}
impl Publication {
    pub fn commit(&mut self) {
        for page in self.pages.drain(..) {
            if let Ok(mut state) = page.session.state.lock() {
                state.publication_pending = false;
                page.session.changed.notify_all();
            }
        }
    }
}
impl Drop for Publication {
    fn drop(&mut self) {
        for page in self.pages.drain(..) {
            if let Ok(mut state) = page.session.state.lock() {
                state.unread.prepend(&page.unread);
                state.cursor = page.cursor;
                state.exit_read = page.exit_read;
                state.publication_pending = false;
                page.session.changed.notify_all();
            }
        }
    }
}
/// Thread-local scope around synchronous tool dispatch, transferred to the HTTP body.
pub struct PublicationScope {
    active: bool,
    _thread: std::marker::PhantomData<*mut ()>,
}
impl PublicationScope {
    pub fn begin() -> Self {
        PAGES.with(|p| {
            let mut p = p.borrow_mut();
            assert!(p.is_none(), "Nested publication scope");
            *p = Some(vec![]);
        });
        Self {
            active: true,
            _thread: std::marker::PhantomData,
        }
    }
    pub fn finish(mut self) -> Publication {
        self.active = false;
        Publication {
            pages: PAGES.with(|p| p.borrow_mut().take().unwrap_or_default()),
        }
    }
}
impl Drop for PublicationScope {
    fn drop(&mut self) {
        if self.active {
            drop(Publication {
                pages: PAGES.with(|p| p.borrow_mut().take().unwrap_or_default()),
            });
        }
    }
}
pub fn secret_environment(key: &str) -> bool {
    matches!(
        key.to_ascii_uppercase().as_str(),
        "CONTROL_PLANE_API_KEY"
            | "OPENAI_API_KEY"
            | "OPENAI_ADMIN_KEY"
            | "CLOUDFLARED_TOKEN"
            | "CLOUDFLARED_TUNNEL_TOKEN"
    )
}
pub struct Launch<'a> {
    pub owner: &'a str,
    pub program: &'a str,
    pub args: &'a [String],
    pub cwd: &'a Path,
    pub tty: bool,
    pub raw_cmd: bool,
}
enum Process {
    Pipe(Child),
    Pty(Box<dyn portable_pty::Child + Send + Sync>),
}
impl Process {
    fn status(&mut self) -> io::Result<Option<i32>> {
        match self {
            Self::Pipe(c) => c.try_wait().map(|o| o.map(|s| s.code().unwrap_or(-1))),
            Self::Pty(c) => c.try_wait().map(|o| o.map(|s| s.exit_code() as i32)),
        }
    }
}
struct State {
    display: Option<crate::batch::Display>,
    owner: String,
    unread: HeadTail,
    retained: HeadTail,
    exit: Option<i32>,
    readers: usize,
    exit_read: bool,
    publication_pending: bool,
    retired: bool,
    total: u64,
    cursor: u64,
}
struct Session {
    script: Mutex<Option<tempfile::TempPath>>,
    interaction: Mutex<()>,
    state: Mutex<State>,
    changed: Condvar,
    child: Mutex<Process>,
    input: Mutex<Option<Box<dyn Write + Send>>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    tree: Tree,
    created: Instant,
}
struct Inner {
    sessions: Mutex<BTreeMap<i32, Arc<Session>>>,
    stopped: std::sync::atomic::AtomicBool,
}
impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(s) = self.sessions.lock() {
            for p in s.values() {
                p.tree.terminate();
            }
        }
    }
}
#[derive(Clone)]
pub struct Manager(Arc<Inner>);
impl Default for Manager {
    fn default() -> Self {
        Self(Arc::new(Inner {
            sessions: Mutex::new(BTreeMap::new()),
            stopped: std::sync::atomic::AtomicBool::new(false),
        }))
    }
}
fn pump(mut reader: Box<dyn Read + Send>, session: Arc<Session>) {
    thread::spawn(move || {
        let mut bytes = [0u8; 8192];
        let mut query = Vec::new();
        while let Ok(n) = reader.read(&mut bytes) {
            if n == 0 {
                break;
            }
            // ConPTY inherits the cursor only after a terminal answers DSR. A model-facing
            // terminal has no renderer to answer it; respond here, including split packets.
            if cfg!(windows) {
                query.extend_from_slice(&bytes[..n]);
                if query.windows(4).any(|w| w == b"\x1b[6n")
                    && let Ok(mut input) = session.input.lock()
                    && let Some(w) = input.as_mut()
                {
                    let _ = w.write_all(b"\x1b[1;1R");
                    let _ = w.flush();
                }
                if query.len() > 3 {
                    query.drain(..query.len() - 3);
                }
            }
            if let Ok(mut s) = session.state.lock() {
                let display = if let Some(display) = &mut s.display {
                    display.push(&bytes[..n], false)
                } else {
                    bytes[..n].to_vec()
                };
                s.unread.push(&display);
                s.retained.push(&display);
                s.total += display.len() as u64;
                session.changed.notify_all();
            } else {
                break;
            }
        }
        if let Ok(mut s) = session.state.lock() {
            s.readers = s.readers.saturating_sub(1);
            if s.readers == 0
                && let Some(display) = &mut s.display
            {
                let tail = display.push(&[], true);
                s.unread.push(&tail);
                s.retained.push(&tail);
                s.total += tail.len() as u64;
            }
            session.changed.notify_all();
        }
    });
}
impl Manager {
    pub fn launch_with_limit(
        &self,
        launch: Launch<'_>,
        wait: Duration,
        marker: Option<&str>,
        max_tokens: usize,
    ) -> io::Result<Value> {
        self.launch_env(launch, wait, None, marker, max_tokens)
    }
    pub fn launch_batch(
        &self,
        launch: Launch<'_>,
        wait: Duration,
        marker: &str,
    ) -> io::Result<Value> {
        self.launch_env(launch, wait, None, Some(marker), 10000)
    }
    pub fn launch_environment(
        &self,
        launch: Launch<'_>,
        wait: Duration,
        environment: &BTreeMap<String, String>,
    ) -> io::Result<Value> {
        self.launch_env(launch, wait, Some(environment), None, 10000)
    }
    pub fn launch(&self, launch: Launch<'_>, wait: Duration) -> io::Result<Value> {
        self.launch_env(launch, wait, None, None, 10000)
    }
    fn launch_env(
        &self,
        launch: Launch<'_>,
        wait: Duration,
        environment: Option<&BTreeMap<String, String>>,
        batch_marker: Option<&str>,
        max_tokens: usize,
    ) -> io::Result<Value> {
        if batch_marker.is_some_and(|m| m.len() != 24 || !m.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(invalid("Invalid command batch marker"));
        }
        let mut script = None;
        let mut override_args = None;
        if cfg!(windows) && launch.raw_cmd && launch.args.last().is_some_and(|a| a.contains('\n')) {
            let mut file = tempfile::Builder::new()
                .prefix("cos-rust-command-")
                .suffix(".cmd")
                .tempfile()?;
            file.write_all(b"@echo off\r\nchcp 65001 >nul\r\n")?;
            file.write_all(launch.args.last().unwrap().as_bytes())?;
            file.flush()?;
            let path = file.into_temp_path();
            let mut args = launch.args[..launch.args.len() - 1].to_vec();
            args.push(format!(
                "\"{}\"",
                path.to_string_lossy().trim_start_matches("\\\\?\\")
            ));
            override_args = Some(args);
            script = Some(path);
        }
        let launch = Launch {
            args: override_args.as_deref().unwrap_or(launch.args),
            ..launch
        };
        if let Some(env) = environment
            && (env.len() > 64
                || env.iter().any(|(k, v)| {
                    k.is_empty()
                        || k.len() > 128
                        || v.len() > 8192
                        || k.contains(['=', '\0'])
                        || v.contains('\0')
                }))
        {
            return Err(invalid("Invalid child environment"));
        }
        use std::sync::atomic::Ordering;
        if launch.owner.is_empty() {
            return Err(invalid("Execution needs a request or session owner"));
        }
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(invalid("Execution manager is shutting down"));
        }
        #[cfg(windows)]
        let prepared_environment = crate::toolchain::child_overrides(environment)?;
        #[cfg(windows)]
        let environment = Some(&prepared_environment);
        let mut registry = lock(&self.0.sessions)?;
        if registry.len() >= MAX_PROCESSES {
            let evict = registry
                .iter()
                .filter(|(_, p)| {
                    p.state
                        .lock()
                        .is_ok_and(|s| s.exit.is_some() && s.exit_read && !s.publication_pending)
                })
                .min_by_key(|(_, p)| p.created)
                .map(|(id, _)| *id);
            if let Some(id) = evict {
                registry.remove(&id);
            } else {
                return Err(invalid(
                    "Too many running processes or unread completed results",
                ));
            }
        }
        type Spawned = (
            Process,
            Option<Box<dyn Write + Send>>,
            Option<Box<dyn MasterPty + Send>>,
            Vec<Box<dyn Read + Send>>,
            Tree,
        );
        let (child, input, master, readers, tree): Spawned = if launch.tty {
            #[cfg(windows)]
            {
                let p = crate::winpty::spawn(&launch, environment, true)?;
                (
                    Process::Pty(p.child),
                    Some(p.writer),
                    Some(p.master),
                    vec![p.reader],
                    p.tree,
                )
            }
            #[cfg(unix)]
            {
                let pair = native_pty_system()
                    .openpty(PtySize {
                        rows: 24,
                        cols: 80,
                        pixel_width: 0,
                        pixel_height: 0,
                    })
                    .map_err(io::Error::other)?;
                let mut command = CommandBuilder::new(launch.program);
                command.args(launch.args);
                command.cwd(launch.cwd);
                for (key, _) in std::env::vars_os() {
                    if secret_environment(&key.to_string_lossy()) {
                        command.env_remove(key);
                    }
                }
                if let Some(environment) = environment {
                    for (key, value) in environment {
                        command.env(key, value);
                    }
                }
                for (k, v) in shell::environment().as_object().unwrap() {
                    command.env(k, v.as_str().unwrap());
                }
                let mut child = pair
                    .slave
                    .spawn_command(command)
                    .map_err(io::Error::other)?;
                let pid = child
                    .process_id()
                    .ok_or_else(|| invalid("PTY has no process identity"))?;
                let tree = match Tree::attach(pid, false) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ = child.kill();
                        return Err(e);
                    }
                };
                // A spawned PTY owns a live child even when its host-side
                // reader/writer setup fails; never orphan that child.
                let read = match pair.master.try_clone_reader() {
                    Ok(reader) => reader,
                    Err(error) => {
                        tree.terminate();
                        let _ = child.kill();
                        return Err(io::Error::other(error));
                    }
                };
                let input = match pair.master.take_writer() {
                    Ok(writer) => writer,
                    Err(error) => {
                        tree.terminate();
                        let _ = child.kill();
                        return Err(io::Error::other(error));
                    }
                };
                drop(pair.slave);
                (
                    Process::Pty(child),
                    Some(input),
                    Some(pair.master),
                    vec![read],
                    tree,
                )
            }
        } else {
            let mut command = Command::new(launch.program);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000 | 0x00000004);
                if launch.raw_cmd {
                    if let Some((last, rest)) = launch.args.split_last() {
                        command.args(rest);
                        command.raw_arg(last);
                    }
                } else {
                    command.args(launch.args);
                }
            }
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.args(launch.args);
                command.process_group(0);
            }
            command
                .current_dir(launch.cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            for (key, _) in std::env::vars_os() {
                if secret_environment(&key.to_string_lossy()) {
                    command.env_remove(key);
                }
            }
            if let Some(environment) = environment {
                command.envs(environment);
            }
            for (k, v) in shell::environment().as_object().unwrap() {
                command.env(k, v.as_str().unwrap());
            }
            let mut child = command.spawn()?;
            let tree = match Tree::attach(child.id(), cfg!(windows)) {
                Ok(t) => t,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e);
                }
            };
            let readers: Vec<Box<dyn Read + Send>> = vec![
                Box::new(child.stdout.take().unwrap()),
                Box::new(child.stderr.take().unwrap()),
            ];
            (Process::Pipe(child), None, None, readers, tree)
        };
        let mut id = (uuid::Uuid::new_v4().as_u128() as i32) & i32::MAX;
        while id == 0 || registry.contains_key(&id) {
            id = (uuid::Uuid::new_v4().as_u128() as i32) & i32::MAX;
        }
        let session = Arc::new(Session {
            state: Mutex::new(State {
                display: batch_marker.map(crate::batch::Display::new),
                owner: launch.owner.into(),
                unread: HeadTail::new(OUTPUT_BYTES),
                publication_pending: false,
                retired: false,
                retained: HeadTail::new(256 * 1024),
                exit: None,
                readers: readers.len(),
                exit_read: false,
                total: 0,
                cursor: 0,
            }),
            changed: Condvar::new(),
            script: Mutex::new(script),
            interaction: Mutex::new(()),
            child: Mutex::new(child),
            input: Mutex::new(input),
            master: Mutex::new(master),
            tree,
            created: Instant::now(),
        });
        registry.insert(id, session.clone());
        drop(registry);
        for reader in readers {
            pump(reader, session.clone());
        }
        let waiting = session.clone();
        thread::spawn(move || {
            loop {
                let status = match waiting.child.lock() {
                    Ok(mut c) => waiting
                        .tree
                        .ready_to_reap()
                        .and_then(|ready| if ready { c.status() } else { Ok(None) }),
                    Err(_) => Err(io::Error::other("Child lock poisoned")),
                };
                match status {
                    Ok(Some(exit)) => {
                        waiting.tree.terminate();
                        if let Ok(mut input) = waiting.input.lock() {
                            input.take();
                        }
                        let master = waiting.master.lock().ok().and_then(|mut m| m.take());
                        drop(master);
                        if let Ok(mut script) = waiting.script.lock() {
                            script.take();
                        }
                        if let Ok(mut s) = waiting.state.lock() {
                            s.exit = Some(exit);
                            waiting.changed.notify_all();
                        }
                        break;
                    }
                    Err(_) => {
                        // An unobservable native exit is still a custody failure: close the
                        // writers/PTY and temporary script before publishing terminal state.
                        // Tree::terminate() is PID-reuse safe on Unix after lost ownership.
                        waiting.tree.terminate();
                        if let Ok(mut input) = waiting.input.lock() {
                            input.take();
                        }
                        if let Ok(mut master) = waiting.master.lock() {
                            master.take();
                        }
                        if let Ok(mut script) = waiting.script.lock() {
                            script.take();
                        }
                        if let Ok(mut s) = waiting.state.lock() {
                            s.exit = Some(-1);
                            waiting.changed.notify_all();
                        }
                        break;
                    }
                    _ => {}
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        Self::response(id, &session, wait, false, max_tokens)
    }
    // Registry lookup is only an initial authority check: a returned Arc may
    // outlive a concurrent promotion or retirement. Recheck under interaction
    // custody immediately before every mutation, input delivery or publication.
    fn require_live_owner(p: &Session, owner: &str) -> io::Result<()> {
        let state = lock(&p.state)?;
        if state.retired {
            return Err(invalid(
                "EXEC_SESSION_RETIRED: process custody was released",
            ));
        }
        if state.owner != owner {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "EXEC_SESSION_NOT_OWNED: this process belongs to another session or request",
            ));
        }
        Ok(())
    }
    fn owned(&self, id: i32, owner: &str) -> io::Result<Arc<Session>> {
        let p = lock(&self.0.sessions)?
            .get(&id)
            .cloned()
            .ok_or_else(|| invalid(format!("Unknown process ID {id}")))?;
        Self::require_live_owner(&p, owner)?;
        Ok(p)
    }
    pub fn promote(&self, request: &str, session: &str) -> io::Result<()> {
        for p in lock(&self.0.sessions)?.values() {
            // Serialize a rebind against a pending write_stdin, resize, interrupt
            // or retirement. The old principal cannot act after this handoff.
            let _interaction = lock(&p.interaction)?;
            let mut s = lock(&p.state)?;
            if !s.retired && s.owner == format!("request:{request}") {
                s.owner = format!("session:{session}");
            }
        }
        Ok(())
    }
    pub fn provisional_requests(&self) -> io::Result<Vec<String>> {
        let mut requests = std::collections::BTreeSet::new();
        for session in lock(&self.0.sessions)?.values() {
            let s = lock(&session.state)?;
            if let Some(request) = s.owner.strip_prefix("request:") {
                requests.insert(request.into());
            }
        }
        Ok(requests.into_iter().collect())
    }
    fn response(
        id: i32,
        p: &Arc<Session>,
        wait: Duration,
        replay: bool,
        max_tokens: usize,
    ) -> io::Result<Value> {
        let _interaction = lock(&p.interaction)?;
        Self::respond(id, p, wait, replay, max_tokens)
    }
    fn respond(
        id: i32,
        p: &Arc<Session>,
        wait: Duration,
        replay: bool,
        max_tokens: usize,
    ) -> io::Result<Value> {
        let started = Instant::now();
        let deadline = started + wait.min(Duration::from_secs(300));
        let mut s = lock(&p.state)?;
        if s.publication_pending {
            return Err(invalid(
                "EXEC_OUTPUT_PENDING: the previous output publication has not settled; no input was sent",
            ));
        }
        while (s.exit.is_none() || s.readers > 0) && Instant::now() < deadline {
            if !s.unread.is_empty() && started.elapsed() >= Duration::from_millis(150) {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (next, _) = p
                .changed
                .wait_timeout(s, remaining.min(Duration::from_millis(150)))
                .map_err(|_| io::Error::other("Process wait poisoned"))?;
            s = next;
        }
        PAGES.with(|pages| {
            if let Some(pages) = pages.borrow_mut().as_mut() {
                pages.push(Delivery {
                    session: p.clone(),
                    unread: if replay {
                        HeadTail::new(OUTPUT_BYTES)
                    } else {
                        s.unread.clone()
                    },
                    cursor: s.cursor,
                    exit_read: s.exit_read,
                });
                s.publication_pending = true;
            }
        });
        let text = if replay {
            s.retained.snapshot()
        } else {
            s.unread.take()
        };
        s.cursor = s.total;
        let original = text.len().div_ceil(4);
        let output = truncate(&text, max_tokens.min(OUTPUT_BYTES / 4) * 4);
        let mut r = json!({"chunk_id":uuid::Uuid::new_v4().simple().to_string()[..6].to_owned(),"wall_time_seconds":started.elapsed().as_secs_f64(),"output":output});
        if original > max_tokens.min(OUTPUT_BYTES / 4) {
            r["original_token_count"] = json!(original);
        }
        if let Some(exit) = s.exit.filter(|_| s.readers == 0) {
            r["exit_code"] = json!(exit);
            r["completed_session_id"] = json!(id);
            s.exit_read = true;
        } else {
            r["session_id"] = json!(id);
        }
        if replay {
            r["output_replayed"] = json!(true);
        }
        Ok(r)
    }
    pub fn poll(&self, id: i32, owner: &str, chars: &str, wait: Duration) -> io::Result<Value> {
        self.poll_with_limit(id, owner, chars, wait, 10000)
    }
    pub fn poll_with_limit(
        &self,
        id: i32,
        owner: &str,
        chars: &str,
        wait: Duration,
        max_tokens: usize,
    ) -> io::Result<Value> {
        let p = self.owned(id, owner)?;
        let _interaction = lock(&p.interaction)?;
        Self::require_live_owner(&p, owner)?;
        let completed = {
            let s = lock(&p.state)?;
            if s.publication_pending {
                return Err(invalid(
                    "EXEC_OUTPUT_PENDING: the previous output publication has not settled; no input was sent",
                ));
            }
            s.exit.is_some() && s.readers == 0 && s.exit_read
        };
        if completed {
            if !chars.is_empty() {
                return Err(invalid(
                    "Process already completed; no input was sent. Use empty chars to read its retained output.",
                ));
            }
            return Self::respond(id, &p, Duration::ZERO, true, max_tokens);
        }
        if !chars.is_empty() && lock(&p.state)?.exit.is_some() {
            return Err(invalid(
                "Process already completed; no input was sent. Use empty chars to read its retained output.",
            ));
        }
        if !chars.is_empty() {
            let mut input = lock(&p.input)?;
            if let Some(w) = input.as_mut() {
                w.write_all(chars.as_bytes())?;
                w.flush()?;
            } else if chars == "\u{3}" {
                #[cfg(unix)]
                p.tree.interrupt();
                #[cfg(windows)]
                {
                    p.tree.terminate();
                }
            } else {
                return Err(invalid(
                    "stdin is closed; run exec_command with tty=true to keep it open",
                ));
            }
        }
        Self::respond(id, &p, wait, false, max_tokens)
    }
    pub fn reread(&self, id: i32, owner: &str) -> io::Result<Value> {
        let p = self.owned(id, owner)?;
        let _interaction = lock(&p.interaction)?;
        Self::require_live_owner(&p, owner)?;
        Self::respond(id, &p, Duration::ZERO, true, 10000)
    }
    pub fn resize(&self, id: i32, owner: &str, rows: u16, cols: u16) -> io::Result<()> {
        if rows == 0 || cols == 0 || rows > 500 || cols > 1000 {
            return Err(invalid("Invalid terminal dimensions"));
        }
        let p = self.owned(id, owner)?;
        let _interaction = lock(&p.interaction)?;
        Self::require_live_owner(&p, owner)?;
        let m = lock(&p.master)?;
        m.as_ref()
            .ok_or_else(|| invalid("Not a PTY session"))?
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io::Error::other)
    }
    pub fn terminate(&self, id: i32, owner: &str) -> io::Result<()> {
        let p = self.owned(id, owner)?;
        let _interaction = lock(&p.interaction)?;
        Self::require_live_owner(&p, owner)?;
        p.tree.terminate();
        Ok(())
    }
    /// Explicit host retirement discards only this owned process's settled results.
    /// It waits on child/pipe state, independently of any response publication lock.
    /// Admission and ordinary model polling never use this destructive retirement path.
    pub fn retire(&self, id: i32, owner: &str, wait: Duration) -> io::Result<()> {
        self.retire_inner(id, owner, wait, false)
    }
    /// Explicit internal sidecar shutdown: discard output pending HTTP publication
    /// only after the owned child and all pipe readers have settled. Ordinary
    /// terminal sessions must use retire() to preserve unread completion.
    pub fn retire_discard_pending(&self, id: i32, owner: &str, wait: Duration) -> io::Result<()> {
        self.retire_inner(id, owner, wait, true)
    }
    fn retire_inner(
        &self,
        id: i32,
        owner: &str,
        wait: Duration,
        discard_pending: bool,
    ) -> io::Result<()> {
        let p = self.owned(id, owner)?;
        let interaction = lock(&p.interaction)?;
        Self::require_live_owner(&p, owner)?;
        p.tree.terminate();
        let deadline = Instant::now() + wait.min(Duration::from_secs(30));
        let mut state = lock(&p.state)?;
        // A publication belongs to the HTTP response until commit/rollback.
        // Never evict custody or lose unread output while it is still pending.
        while state.exit.is_none()
            || state.readers > 0
            || (state.publication_pending && !discard_pending)
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "Owned process {id} did not settle: exit={:?}, open_readers={}, publication_pending={}",
                        state.exit, state.readers, state.publication_pending
                    ),
                ));
            }
            state = p
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| io::Error::other("Process wait poisoned"))?
                .0;
        }
        // Mark the Arc itself retired before releasing the interaction lock:
        // callers that looked it up before removal must not act on stale custody.
        state.retired = true;
        drop(state);
        drop(interaction);
        // No interaction mutex is held while acquiring registry (promote uses
        // registry -> interaction); this avoids a cross-lock deadlock.
        let mut sessions = lock(&self.0.sessions)?;
        if sessions
            .get(&id)
            .is_some_and(|current| Arc::ptr_eq(current, &p))
        {
            sessions.remove(&id);
        }
        Ok(())
    }
    pub fn snapshot(&self, owner: &str) -> io::Result<Value> {
        let mut result = vec![];
        for (id, p) in lock(&self.0.sessions)?.iter() {
            let s = lock(&p.state)?;
            if s.owner == owner {
                result.push(json!({"session_id":id,"exit_code":s.exit,"unread":s.total>s.cursor||!s.exit_read||s.publication_pending,"publicationPending":s.publication_pending,"tty":lock(&p.master)?.is_some(),"outputBytes":s.total}));
            }
        }
        Ok(json!(result))
    }
    pub fn shutdown(&self) {
        self.0
            .stopped
            .store(true, std::sync::atomic::Ordering::Release);
        if let Ok(s) = self.0.sessions.lock() {
            for p in s.values() {
                p.tree.terminate();
            }
        }
    }
}
pub fn truncate(text: &str, budget: usize) -> String {
    if text.len() <= budget {
        return text.into();
    }
    let mut left = budget / 2;
    while !text.is_char_boundary(left) {
        left -= 1;
    }
    let mut right = text.len() - (budget - budget / 2);
    while !text.is_char_boundary(right) {
        right += 1;
    }
    format!(
        "Warning: truncated output (original token count: {})\nTotal output lines: {}\n\n{}…{} tokens truncated…{}",
        text.len().div_ceil(4),
        text.lines().count(),
        &text[..left],
        (text.len() - budget).div_ceil(4),
        &text[right..]
    )
}

#[cfg(test)]
mod retirement_tests {
    use super::*;
    #[test]
    fn stale_arc_cannot_publish_after_rebind_or_retirement() {
        let d = tempfile::tempdir().unwrap();
        let (program, args) = if cfg!(windows) {
            (
                "cmd.exe",
                vec!["/d".into(), "/c".into(), "echo stable".into()],
            )
        } else {
            ("sh", vec!["-c".into(), "printf stable".into()])
        };
        let manager = Manager::default();
        let response = manager
            .launch(
                Launch {
                    owner: "request:old",
                    program,
                    args: &args,
                    cwd: d.path(),
                    tty: false,
                    raw_cmd: false,
                },
                Duration::from_secs(3),
            )
            .unwrap();
        let id = response["completed_session_id"].as_i64().unwrap() as i32;
        let stale = manager.owned(id, "request:old").unwrap();
        manager.promote("old", "new").unwrap();
        assert_eq!(
            Manager::require_live_owner(&stale, "request:old")
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        manager
            .retire(id, "session:new", Duration::from_secs(2))
            .unwrap();
        assert!(
            Manager::require_live_owner(&stale, "session:new")
                .unwrap_err()
                .to_string()
                .contains("EXEC_SESSION_RETIRED")
        );
    }
    #[test]
    fn retirement_timeout_preserves_owned_custody_until_a_successful_retry() {
        let d = tempfile::tempdir().unwrap();
        let (program, args) = if cfg!(windows) {
            (
                "cmd.exe",
                vec!["/d".into(), "/c".into(), "ping -n 30 127.0.0.1 >nul".into()],
            )
        } else {
            ("sh", vec!["-c".into(), "sleep 30".into()])
        };
        let manager = Manager::default();
        let result = manager
            .launch(
                Launch {
                    owner: "sidecar",
                    program,
                    args: &args,
                    cwd: d.path(),
                    tty: false,
                    raw_cmd: false,
                },
                Duration::ZERO,
            )
            .unwrap();
        let id = result["session_id"].as_i64().unwrap() as i32;
        let process = manager.owned(id, "sidecar").unwrap();
        // Hold the real reaper's child lock to exercise a delayed native exit,
        // without replacing the process or the production custody state.
        let child = process.child.lock().unwrap();
        assert_eq!(
            manager
                .retire(id, "foreign", Duration::ZERO)
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            manager
                .retire(id, "sidecar", Duration::from_millis(20))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(manager.owned(id, "sidecar").is_ok());
        drop(child);
        manager
            .retire(id, "sidecar", Duration::from_secs(2))
            .unwrap();
        assert_eq!(manager.snapshot("sidecar").unwrap(), json!([]));
    }
}
