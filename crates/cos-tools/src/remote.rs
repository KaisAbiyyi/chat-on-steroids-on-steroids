//! Host OpenSSH SFTP transport.
use cos_core::persistence::invalid;
use cos_exec::{ssh, tree::Tree};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io::{self, Cursor, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
const MAX_PACKET: usize = 4 * 1024 * 1024;
fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_be_bytes());
}
fn put64(out: &mut Vec<u8>, v: u64) {
    out.extend(v.to_be_bytes());
}
fn put(out: &mut Vec<u8>, v: &[u8]) {
    put32(out, v.len() as u32);
    out.extend(v);
}
struct Packet {
    bytes: Vec<u8>,
    at: usize,
}
impl Packet {
    fn u32(&mut self) -> io::Result<u32> {
        let bytes = self
            .bytes
            .get(self.at..self.at + 4)
            .ok_or_else(|| invalid("EPROTO: Short SFTP packet"))?;
        self.at += 4;
        Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok((u64::from(self.u32()?) << 32) | u64::from(self.u32()?))
    }
    fn data(&mut self) -> io::Result<Vec<u8>> {
        let n = self.u32()? as usize;
        let bytes = self
            .bytes
            .get(self.at..self.at + n)
            .ok_or_else(|| invalid("EPROTO: Short SFTP packet"))?
            .to_vec();
        self.at += n;
        Ok(bytes)
    }
    fn string(&mut self) -> io::Result<String> {
        Ok(String::from_utf8_lossy(&self.data()?).into_owned())
    }
    fn attrs(&mut self) -> io::Result<Value> {
        let flags = self.u32()?;
        let size = if flags & 1 != 0 { self.u64()? } else { 0 };
        if flags & 2 != 0 {
            self.u32()?;
            self.u32()?;
        }
        let mode = if flags & 4 != 0 { self.u32()? } else { 0 };
        let time = if flags & 8 != 0 {
            self.u32()?;
            self.u32()?
        } else {
            0
        };
        if flags & 0x80000000 != 0 {
            let n = self.u32()?;
            if n > 1024 {
                return Err(invalid("EPROTO: Too many SFTP extended attributes"));
            }
            for _ in 0..n {
                self.data()?;
                self.data()?;
            }
        }
        let kind = mode & 0o170000;
        Ok(
            json!({"size":size,"mode":mode,"isFile":kind==0o100000,"isDirectory":kind==0o040000,"isSymbolicLink":kind==0o120000,"mtimeMs":u64::from(time)*1000}),
        )
    }
}
fn receive(read: &mut dyn Read) -> io::Result<Vec<u8>> {
    let mut size = [0; 4];
    read.read_exact(&mut size)?;
    let size = u32::from_be_bytes(size) as usize;
    if !(1..=MAX_PACKET).contains(&size) {
        return Err(invalid("EPROTO: SFTP packet exceeds limit"));
    }
    let mut bytes = vec![0; size];
    read.read_exact(&mut bytes)?;
    Ok(bytes)
}
struct TimedRead {
    receiver: mpsc::Receiver<io::Result<Vec<u8>>>,
    cursor: Cursor<Vec<u8>>,
}
impl Read for TimedRead {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.cursor.position() as usize == self.cursor.get_ref().len() {
            let packet = self
                .receiver
                .recv_timeout(Duration::from_secs(30))
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::TimedOut, "ECONNRESET: SFTP reply timed out")
                })??;
            let mut frame = (packet.len() as u32).to_be_bytes().to_vec();
            frame.extend(packet);
            self.cursor = Cursor::new(frame);
        }
        self.cursor.read(out)
    }
}
pub struct Sftp {
    read: Box<dyn Read + Send>,
    write: Box<dyn Write + Send>,
    next: u32,
    extensions: BTreeSet<String>,
    closed: bool,
    host: Option<(Child, Tree)>,
    stderr: Arc<Mutex<String>>,
}
struct RemoteReader<'a> {
    client: &'a mut Sftp,
    handle: Vec<u8>,
    offset: u64,
}
impl Read for RemoteReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let size = output.len().min(65536);
        let mut payload = vec![];
        put(&mut payload, &self.handle);
        put64(&mut payload, self.offset);
        put32(&mut payload, size as u32);
        match self.client.checked(5, &payload, 103) {
            Ok(mut p) => {
                let data = p.data()?;
                if data.len() > size {
                    return Err(invalid(
                        "EPROTO: SFTP server exceeded requested read length",
                    ));
                }
                output[..data.len()].copy_from_slice(&data);
                self.offset += data.len() as u64;
                Ok(data.len())
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(0),
            Err(e) => Err(e),
        }
    }
}
impl Drop for Sftp {
    fn drop(&mut self) {
        if let Some((child, tree)) = &mut self.host {
            tree.terminate();
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Sftp {
    pub fn from_io(
        read: impl Read + Send + 'static,
        write: impl Write + Send + 'static,
    ) -> io::Result<Self> {
        let mut s = Self {
            read: Box::new(read),
            write: Box::new(write),
            next: 1,
            extensions: BTreeSet::new(),
            closed: false,
            host: None,
            stderr: Arc::new(Mutex::new(String::new())),
        };
        s.frame(&[1, 0, 0, 0, 3])?;
        let bytes = receive(&mut *s.read)?;
        if bytes[0] != 2 {
            return Err(invalid("EPROTO: SFTP handshake failed"));
        }
        let mut packet = Packet { bytes, at: 1 };
        if packet.u32()? != 3 {
            return Err(invalid("ENOTSUP: SFTP version 3 is required"));
        }
        while packet.at < packet.bytes.len() {
            let name = packet.string()?;
            packet.string()?;
            s.extensions.insert(name);
        }
        Ok(s)
    }
    pub fn connect(remote: &Value) -> io::Result<Self> {
        Self::spawn(
            &ssh::program(),
            &ssh::args(remote, &["-s".into()], &["sftp".into()]),
            None,
        )
    }
    pub fn spawn(
        program: &str,
        args: &[String],
        cwd: Option<&std::path::Path>,
    ) -> io::Result<Self> {
        Self::spawn_environment(program, args, cwd, &std::collections::BTreeMap::new())
    }
    pub fn spawn_environment(
        program: &str,
        args: &[String],
        cwd: Option<&std::path::Path>,
        environment: &std::collections::BTreeMap<String, String>,
    ) -> io::Result<Self> {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        for (key, _) in std::env::vars_os() {
            if cos_exec::process::secret_environment(&key.to_string_lossy()) {
                command.env_remove(key);
            }
        }
        command.envs(environment);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000 | 0x00000004);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn()?;
        let tree = match Tree::attach(child.id(), cfg!(windows)) {
            Ok(t) => t,
            Err(e) => {
                let _ = child.kill();
                return Err(e);
            }
        };
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let stdin: ChildStdin = child.stdin.take().unwrap();
        let errors = Arc::new(Mutex::new(String::new()));
        let copy = errors.clone();
        std::thread::spawn(move || {
            let mut bytes = [0; 1024];
            while let Ok(n) = stderr.read(&mut bytes) {
                if n == 0 {
                    break;
                }
                if let Ok(mut s) = copy.lock()
                    && s.len() < 8192
                {
                    s.push_str(&String::from_utf8_lossy(&bytes[..n]));
                }
            }
        });
        let (tx, rx) = mpsc::sync_channel(16);
        std::thread::spawn(move || {
            loop {
                let r = receive(&mut stdout);
                let failed = r.is_err();
                if tx.send(r).is_err() || failed {
                    break;
                }
            }
        });
        match Self::from_io(
            TimedRead {
                receiver: rx,
                cursor: Cursor::new(vec![]),
            },
            stdin,
        ) {
            Ok(mut s) => {
                s.host = Some((child, tree));
                s.stderr = errors;
                Ok(s)
            }
            Err(e) => {
                tree.terminate();
                let _ = child.kill();
                let _ = child.wait();
                let stderr = errors.lock().unwrap().clone();
                Err(io::Error::new(
                    e.kind(),
                    if stderr.is_empty() {
                        e.to_string()
                    } else {
                        ssh::failure(&stderr)
                    },
                ))
            }
        }
    }
    fn frame(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > MAX_PACKET {
            return Err(invalid("EPROTO: SFTP request exceeds limit"));
        }
        self.write.write_all(&(bytes.len() as u32).to_be_bytes())?;
        self.write.write_all(bytes)?;
        self.write.flush()
    }
    fn request(&mut self, ty: u8, payload: &[u8]) -> io::Result<(u8, Packet)> {
        if self.closed {
            return Err(invalid("ECONNRESET: The SFTP session was closed"));
        }
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        let mut bytes = vec![ty];
        put32(&mut bytes, id);
        bytes.extend(payload);
        let result = (|| {
            self.frame(&bytes)?;
            let bytes = receive(&mut *self.read)?;
            let ty = bytes[0];
            let mut p = Packet { bytes, at: 1 };
            if p.u32()? != id {
                return Err(invalid("EPROTO: SFTP response ID mismatch"));
            }
            Ok((ty, p))
        })();
        if result.is_err() {
            self.closed = true;
        }
        result
    }
    fn checked(&mut self, ty: u8, payload: &[u8], expected: u8) -> io::Result<Packet> {
        let (kind, mut p) = self.request(ty, payload)?;
        if kind == 101 {
            let code = p.u32()?;
            let text = p.string()?;
            if code == 0 && expected == 101 {
                return Ok(p);
            }
            let (name, io_kind) = match code {
                1 => ("EOF", io::ErrorKind::UnexpectedEof),
                2 => ("ENOENT", io::ErrorKind::NotFound),
                3 => ("EACCES", io::ErrorKind::PermissionDenied),
                5 => ("EPROTO", io::ErrorKind::InvalidData),
                6 | 7 => ("ECONNRESET", io::ErrorKind::ConnectionReset),
                8 => ("ENOTSUP", io::ErrorKind::Unsupported),
                _ => ("EIO", io::ErrorKind::Other),
            };
            return Err(io::Error::new(io_kind, format!("{name}: {text}")));
        }
        if kind != expected {
            return Err(invalid("EPROTO: Unexpected SFTP response type"));
        }
        Ok(p)
    }
    pub fn realpath(&mut self, path: &str) -> io::Result<String> {
        let mut payload = vec![];
        put(&mut payload, path.as_bytes());
        let mut p = self.checked(16, &payload, 104)?;
        if p.u32()? != 1 {
            return Err(invalid("EPROTO: Invalid REALPATH response"));
        }
        p.string()
    }
    pub fn stat(&mut self, path: &str) -> io::Result<Value> {
        let mut payload = vec![];
        put(&mut payload, path.as_bytes());
        self.checked(17, &payload, 105)?.attrs()
    }
    fn open(&mut self, path: &str, flags: u32) -> io::Result<Vec<u8>> {
        let mut payload = vec![];
        put(&mut payload, path.as_bytes());
        put32(&mut payload, flags);
        put32(&mut payload, 0);
        self.checked(3, &payload, 102)?.data()
    }
    fn close_handle(&mut self, handle: &[u8]) -> io::Result<()> {
        let mut payload = vec![];
        put(&mut payload, handle);
        self.checked(4, &payload, 101).map(|_| ())
    }
    pub fn read(&mut self, path: &str, limit: usize) -> io::Result<Vec<u8>> {
        let handle = self.open(path, 1)?;
        let result = (|| {
            let mut payload = vec![];
            put(&mut payload, &handle);
            let meta = self.checked(8, &payload, 105)?.attrs()?;
            if meta["isFile"] != true {
                return Err(invalid("Not a regular remote file"));
            }
            if meta["size"].as_u64().unwrap_or(u64::MAX) > limit as u64 {
                return Err(invalid("Remote file exceeds read limit"));
            }
            let mut output = vec![];
            loop {
                let mut payload = vec![];
                put(&mut payload, &handle);
                put64(&mut payload, output.len() as u64);
                put32(&mut payload, 65536);
                match self.checked(5, &payload, 103) {
                    Ok(mut p) => {
                        let data = p.data()?;
                        if data.is_empty() {
                            break;
                        }
                        if output.len() + data.len() > limit {
                            return Err(invalid("Remote file grew beyond read limit"));
                        }
                        output.extend(data);
                    }
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e),
                }
            }
            Ok(output)
        })();
        let close = self.close_handle(&handle);
        result.and_then(|v| close.map(|_| v))
    }
    pub fn preview(
        &mut self,
        path: &str,
        start: usize,
        end: usize,
        budget: usize,
    ) -> io::Result<Value> {
        let handle = self.open(path, 1)?;
        let result = (|| {
            let mut payload = vec![];
            put(&mut payload, &handle);
            let meta = self.checked(8, &payload, 105)?.attrs()?;
            if meta["isFile"] != true {
                return Err(invalid("Not a regular remote file"));
            }
            crate::files::preview(
                RemoteReader {
                    client: self,
                    handle: handle.clone(),
                    offset: 0,
                },
                path,
                meta["size"].as_u64().unwrap_or(0),
                start,
                end,
                budget,
            )
        })();
        let close = self.close_handle(&handle);
        result.and_then(|v| close.map(|_| v))
    }
    /// A bounded caller may inspect regular-file metadata and consume one opened stream.
    /// This retains the trusted-server restriction; FSTAT cannot prove handle containment.
    pub(crate) fn read_stream<T>(
        &mut self,
        path: &str,
        work: impl FnOnce(&mut dyn Read, &Value) -> io::Result<T>,
    ) -> io::Result<T> {
        let handle = self.open(path, 1)?;
        let result = (|| {
            let mut payload = vec![];
            put(&mut payload, &handle);
            let metadata = self.checked(8, &payload, 105)?.attrs()?;
            if metadata["isFile"] != true {
                return Err(invalid("Choose a regular file"));
            }
            work(
                &mut RemoteReader {
                    client: self,
                    handle: handle.clone(),
                    offset: 0,
                },
                &metadata,
            )
        })();
        let closed = self.close_handle(&handle);
        result.and_then(|value| closed.map(|_| value))
    }
    pub fn write_new(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        let handle = self.open(path, 2 | 8 | 32)?;
        let result = (|| {
            for (i, chunk) in bytes.chunks(65536).enumerate() {
                let mut payload = vec![];
                put(&mut payload, &handle);
                put64(&mut payload, (i * 65536) as u64);
                put(&mut payload, chunk);
                self.checked(6, &payload, 101)?;
            }
            Ok(())
        })();
        let close = self.close_handle(&handle);
        result.and(close)
    }
    pub fn rename(&mut self, from: &str, to: &str) -> io::Result<()> {
        let mut payload = vec![];
        let ty = if self.extensions.contains("posix-rename@openssh.com") {
            put(&mut payload, b"posix-rename@openssh.com");
            200
        } else {
            18
        };
        put(&mut payload, from.as_bytes());
        put(&mut payload, to.as_bytes());
        self.checked(ty, &payload, 101).map(|_| ())
    }
    pub fn remove(&mut self, path: &str) -> io::Result<()> {
        let mut payload = vec![];
        put(&mut payload, path.as_bytes());
        self.checked(13, &payload, 101).map(|_| ())
    }
    pub fn mkdir(&mut self, path: &str) -> io::Result<()> {
        let mut payload = vec![];
        put(&mut payload, path.as_bytes());
        put32(&mut payload, 0);
        self.checked(14, &payload, 101).map(|_| ())
    }
    pub fn rmdir(&mut self, path: &str) -> io::Result<()> {
        let mut payload = vec![];
        put(&mut payload, path.as_bytes());
        self.checked(15, &payload, 101).map(|_| ())
    }
    pub fn set_mode(&mut self, path: &str, mode: u32) -> io::Result<()> {
        let mut payload = vec![];
        put(&mut payload, path.as_bytes());
        put32(&mut payload, 4);
        put32(&mut payload, mode & 0o7777);
        self.checked(9, &payload, 101).map(|_| ())
    }
    pub fn list(&mut self, path: &str) -> io::Result<Vec<Value>> {
        let mut entries = self.list_native(path)?;
        entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        Ok(entries)
    }
    pub(crate) fn list_native(&mut self, path: &str) -> io::Result<Vec<Value>> {
        let mut payload = vec![];
        put(&mut payload, path.as_bytes());
        let handle = self.checked(11, &payload, 102)?.data()?;
        let result = (|| {
            let mut entries = vec![];
            loop {
                let mut payload = vec![];
                put(&mut payload, &handle);
                let mut p = match self.checked(12, &payload, 104) {
                    Ok(p) => p,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e),
                };
                let count = p.u32()?;
                if count > 5000 || entries.len() + count as usize > 5000 {
                    return Err(invalid("Remote directory exceeds entry limit"));
                }
                for _ in 0..count {
                    let name = p.string()?;
                    p.string()?;
                    let mut attrs = p.attrs()?;
                    if name != "." && name != ".." {
                        attrs["name"] = json!(name);
                        entries.push(attrs);
                    }
                }
            }
            Ok(entries)
        })();
        let close = self.close_handle(&handle);
        result.and_then(|v| close.map(|_| v))
    }
    pub fn closed(&self) -> bool {
        self.closed
    }
}
/// Low-level adapter for a trusted server filesystem. REALPATH and OPEN are separate
/// SFTP v3 requests; regular-file FSTAT does not attest the handle's canonical path.
/// Callers exposed to untrusted paths must refuse operations unless they have a
/// server-enforced boundary or explicit operator trust (as the MCP kernel does).
pub struct RemoteFiles {
    pub sftp: Sftp,
    pub root: String,
}
impl RemoteFiles {
    pub fn apply_patch(&mut self, source: &str, fail_after: Option<usize>) -> io::Result<Value> {
        crate::patch_budget::check_source(source)?;
        let parsed = crate::patch::parse_typed(source).map_err(invalid)?;
        if parsed.environment_id.is_some() {
            return Err(invalid(
                "apply_patch environment selection is unavailable for this turn",
            ));
        }
        self.apply_hunks(parsed.hunks, fail_after)
    }
    pub fn apply_hunks(
        &mut self,
        hunks: Vec<crate::patch::Hunk>,
        fail_after: Option<usize>,
    ) -> io::Result<Value> {
        self.apply_hunks_checked(hunks, fail_after, &|_| Ok(()))
    }
    pub fn apply_hunks_checked(
        &mut self,
        hunks: Vec<crate::patch::Hunk>,
        fail_after: Option<usize>,
        authorize: &dyn Fn(&str) -> io::Result<()>,
    ) -> io::Result<Value> {
        authorize("")?;
        use crate::patch::Hunk;
        use std::collections::BTreeMap;
        let mut budget = crate::patch_budget::Budget::new(&hunks)?;
        struct Change {
            old: Option<Vec<u8>>,
            new: Option<Vec<u8>>,
            mode: Option<u32>,
        }
        fn at(
            remote: &mut RemoteFiles,
            changes: &mut BTreeMap<String, Change>,
            path: &str,
            budget: &mut crate::patch_budget::Budget,
        ) -> io::Result<String> {
            let real = remote.resolve(path, true)?;
            if !changes.contains_key(&real) {
                let old = match remote.sftp.read(&real, budget.read_limit(&real)?) {
                    Ok(bytes) => Some(bytes),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                    Err(e) => return Err(e),
                };
                budget.bytes(&real, old.as_deref().unwrap_or_default())?;
                let mode = if old.is_some() {
                    remote.sftp.stat(&real)?["mode"].as_u64().map(|m| m as u32)
                } else {
                    None
                };
                changes.insert(
                    real.clone(),
                    Change {
                        new: old.clone(),
                        old,
                        mode,
                    },
                );
            }
            Ok(real)
        }
        let mut changes = BTreeMap::new();
        for hunk in hunks {
            match hunk {
                Hunk::AddFile { path, contents } => {
                    let key = at(self, &mut changes, &path, &mut budget)?;
                    budget.bytes(&key, contents.as_bytes())?;
                    authorize(if changes[&key].new.is_some() {
                        "edit"
                    } else {
                        "create"
                    })?;
                    changes.get_mut(&key).unwrap().new = Some(contents.into_bytes());
                }
                Hunk::DeleteFile { path } => {
                    let key = at(self, &mut changes, &path, &mut budget)?;
                    let c = changes.get_mut(&key).unwrap();
                    if c.new.is_none() {
                        return Err(invalid(format!("Failed to read file to delete {path}")));
                    }
                    c.new = None;
                }
                Hunk::UpdateFile {
                    path,
                    move_path,
                    chunks,
                } => {
                    let key = at(self, &mut changes, &path, &mut budget)?;
                    let old = changes[&key]
                        .new
                        .as_ref()
                        .ok_or_else(|| invalid(format!("Failed to read file to update {path}")))?;
                    let text = std::str::from_utf8(old)
                        .map_err(|_| invalid("stream did not contain valid UTF-8 (EILSEQ)"))?;
                    budget.update(&key, old, &chunks)?;
                    let new = crate::patch::update(text, &path, &chunks)
                        .map_err(invalid)?
                        .into_bytes();
                    if let Some(destination) = move_path {
                        let dest = at(self, &mut changes, &destination, &mut budget)?;
                        budget.bytes(&dest, &new)?;
                        changes.get_mut(&key).unwrap().new = None;
                        changes.get_mut(&dest).unwrap().new = Some(new);
                    } else {
                        changes.get_mut(&key).unwrap().new = Some(new);
                    }
                }
            }
        }
        struct Stage {
            path: String,
            backup: Option<String>,
            temp: Option<String>,
        }
        let mut stages: Vec<Stage> = vec![];
        let mut created = vec![];
        let mut committed = 0;
        let snapshot = |sftp: &mut Sftp, path: &str| -> io::Result<Option<Vec<u8>>> {
            let c = &changes[path];
            let limit = c
                .old
                .as_ref()
                .map_or(0, Vec::len)
                .max(c.new.as_ref().map_or(0, Vec::len));
            match sftp.read(path, limit) {
                Ok(b) => Ok(Some(b)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            }
        };
        let result = (|| -> io::Result<()> {
            for (path, c) in &changes {
                authorize("")?;
                if snapshot(&mut self.sftp, path)? != c.old {
                    return Err(invalid(
                        "FILE_REVISION_CHANGED: no remote edits were applied",
                    ));
                }
                let parent = path
                    .rsplit_once('/')
                    .map(|(p, _)| p)
                    .ok_or_else(|| invalid("Invalid remote file path"))?;
                let tail = parent
                    .strip_prefix(&self.root)
                    .ok_or_else(|| invalid("Remote path escaped approved root"))?;
                let mut folder = self.root.trim_end_matches('/').to_string();
                for segment in tail.split('/').filter(|s| !s.is_empty()) {
                    folder.push('/');
                    folder.push_str(segment);
                    match self.sftp.stat(&folder) {
                        Ok(m) if m["isDirectory"] == true => {}
                        Ok(_) => return Err(invalid("Remote parent is not a directory")),
                        Err(e) if e.kind() == io::ErrorKind::NotFound => {
                            self.sftp.mkdir(&folder)?;
                            created.push(folder.clone());
                        }
                        Err(e) => return Err(e),
                    }
                }
                let backup = if let Some(old) = &c.old {
                    let backup = format!("{path}.cos-rust-{}.backup", uuid::Uuid::new_v4());
                    self.sftp.write_new(&backup, old)?;
                    Some(backup)
                } else {
                    None
                };
                stages.push(Stage {
                    path: path.clone(),
                    backup,
                    temp: None,
                });
                if let Some(new) = &c.new {
                    let temp = format!("{path}.cos-rust-{}.partial", uuid::Uuid::new_v4());
                    self.sftp.write_new(&temp, new)?;
                    if let Some(mode) = c.mode {
                        self.sftp.set_mode(&temp, mode)?;
                    }
                    stages.last_mut().unwrap().temp = Some(temp);
                }
            }
            for (i, stage) in stages.iter().enumerate() {
                authorize("")?;
                if fail_after == Some(i) {
                    return Err(io::Error::other("Injected remote commit failure"));
                }
                if snapshot(&mut self.sftp, &stage.path)? != changes[&stage.path].old {
                    return Err(invalid("FILE_REVISION_CHANGED during remote transaction"));
                }
                if let Some(temp) = &stage.temp {
                    self.sftp.rename(temp, &stage.path)?;
                } else {
                    self.sftp.remove(&stage.path)?;
                }
                committed += 1;
            }
            Ok(())
        })();
        let mut failed_rollback = false;
        if result.is_err() {
            for stage in stages[..committed].iter().rev() {
                let restored = if snapshot(&mut self.sftp, &stage.path).ok().as_ref()
                    != Some(&changes[&stage.path].new)
                {
                    Err(invalid(
                        "Concurrent remote change; original backup retained",
                    ))
                } else if let Some(backup) = &stage.backup {
                    self.sftp.rename(backup, &stage.path)
                } else {
                    self.sftp.remove(&stage.path)
                };
                if restored.is_err() {
                    failed_rollback = true;
                }
            }
        }
        for stage in stages {
            if let Some(temp) = stage.temp {
                let _ = self.sftp.remove(&temp);
            }
            if !failed_rollback && let Some(backup) = stage.backup {
                let _ = self.sftp.remove(&backup);
            }
        }
        if result.is_err() {
            for dir in created.iter().rev() {
                let _ = self.sftp.rmdir(dir);
            }
        }
        if failed_rollback {
            return Err(invalid(
                "Remote transaction rollback incomplete; retained .cos-rust backups for recovery",
            ));
        }
        result?;
        Ok(
            json!({"transaction":"committed","changes":changes.iter().map(|(path,c)|json!({"path":path,"kind":if c.old.is_none(){"add"}else if c.new.is_none(){"delete"}else{"update"},"revision":c.new.as_ref().map(|b|crate::files::digest(b))})).collect::<Vec<_>>()}),
        )
    }
    pub fn connect(remote: &Value) -> io::Result<Self> {
        let mut sftp = Sftp::connect(remote)?;
        let root = sftp.realpath(
            remote["path"]
                .as_str()
                .ok_or_else(|| invalid("Remote root has no path"))?,
        )?;
        if sftp.stat(&root)?["isDirectory"] != true {
            return Err(invalid("Choose a remote project folder"));
        }
        Ok(Self { sftp, root })
    }
    pub fn resolve(&mut self, relative: &str, missing: bool) -> io::Result<String> {
        if relative.chars().any(|c| c < ' ' || c == '\\') || relative.split('/').any(|c| c == "..")
        {
            return Err(invalid("Invalid remote project path"));
        }
        let candidate = format!(
            "{}/{}",
            self.root.trim_end_matches('/'),
            relative.trim_start_matches('/')
        );
        let mut ancestor = candidate.clone();
        let mut suffix = vec![];
        let resolved = loop {
            match self.sftp.realpath(&ancestor) {
                Ok(real) => {
                    break if suffix.is_empty() {
                        real
                    } else {
                        format!(
                            "{}/{}",
                            real.trim_end_matches('/'),
                            suffix.iter().rev().cloned().collect::<Vec<_>>().join("/")
                        )
                    };
                }
                Err(e) if missing && e.kind() == io::ErrorKind::NotFound => {
                    let (parent, name) = ancestor
                        .rsplit_once('/')
                        .ok_or_else(|| invalid("Invalid remote path"))?;
                    suffix.push(name.to_string());
                    ancestor = if parent.is_empty() {
                        "/".into()
                    } else {
                        parent.into()
                    };
                    if suffix.len() > 64 {
                        return Err(invalid("Remote path is too deep"));
                    }
                }
                Err(e) => return Err(e),
            }
        };
        if resolved != self.root
            && !resolved.starts_with(&format!("{}/", self.root.trim_end_matches('/')))
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Remote path escapes approved root",
            ));
        }
        Ok(resolved)
    }
    pub fn read(&mut self, path: &str, limit: usize) -> io::Result<Vec<u8>> {
        let real = self.resolve(path, false)?;
        self.sftp.read(&real, limit)
    }
    pub fn save(&mut self, path: &str, contents: &str, revision: &str) -> io::Result<String> {
        let real = self.resolve(path, false)?;
        let old = self
            .sftp
            .read(&real, crate::files::MAX_EDIT_BYTES as usize)?;
        if crate::files::digest(&old) != revision {
            return Err(invalid(
                "FILE_REVISION_CHANGED: refresh the file before saving",
            ));
        }
        let (_, encoding) = crate::text::decode(&old)?;
        let bytes = crate::text::encode(contents, encoding);
        let temporary = format!("{real}.cos-rust-{}.partial", uuid::Uuid::new_v4());
        self.sftp.write_new(&temporary, &bytes)?;
        let result = (|| {
            if self
                .sftp
                .read(&real, crate::files::MAX_EDIT_BYTES as usize)?
                != old
            {
                return Err(invalid("FILE_REVISION_CHANGED during remote save"));
            }
            self.sftp.rename(&temporary, &real)?;
            Ok(crate::files::digest(&bytes))
        })();
        if result.is_err() {
            let _ = self.sftp.remove(&temporary);
        }
        result
    }
}
