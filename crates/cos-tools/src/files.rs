use crate::{
    patch::{self, Hunk},
    text,
};
use cap_std::fs::{Dir, OpenOptions};
use cos_core::{
    config::Root,
    persistence::{invalid, lock},
    sandbox,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Instant,
};
pub const MAX_READ_BYTES: usize = 512 * 1024;
pub const MAX_EDIT_BYTES: u64 = 512 * 1024 * 1024;
fn stream_line(
    reader: &mut impl BufRead,
    encoding: text::Encoding,
) -> io::Result<Option<(String, Vec<u8>)>> {
    let mut raw = vec![];
    if matches!(encoding, text::Encoding::Utf16Le | text::Encoding::Utf16Be) {
        loop {
            let head = reader.fill_buf()?;
            if head.is_empty() {
                break;
            }
            let mut unit = [0; 2];
            reader.read_exact(&mut unit)?;
            raw.extend(unit);
            let unit = if matches!(encoding, text::Encoding::Utf16Le) {
                u16::from_le_bytes(unit)
            } else {
                u16::from_be_bytes(unit)
            };
            if unit == 10 {
                break;
            }
            if raw.len() > 4 * 1024 * 1024 {
                return Err(invalid("A line exceeds the safe decoded-line limit"));
            }
        }
        if raw.is_empty() {
            return Ok(None);
        }
        let units = raw
            .chunks_exact(2)
            .map(|p| {
                if matches!(encoding, text::Encoding::Utf16Le) {
                    u16::from_le_bytes([p[0], p[1]])
                } else {
                    u16::from_be_bytes([p[0], p[1]])
                }
            })
            .collect::<Vec<_>>();
        let mut line = String::from_utf16_lossy(&units);
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }
        Ok(Some((line, raw)))
    } else {
        loop {
            let head = reader.fill_buf()?;
            if head.is_empty() {
                break;
            }
            let n = head
                .iter()
                .position(|b| *b == 10)
                .map(|n| n + 1)
                .unwrap_or(head.len());
            if raw.len() + n > 2 * 1024 * 1024 {
                return Err(invalid("A line exceeds the safe decoded-line limit"));
            }
            let done = head[n - 1] == 10;
            raw.extend_from_slice(&head[..n]);
            reader.consume(n);
            if done {
                break;
            }
        }
        if raw.is_empty() {
            return Ok(None);
        }
        let mut line = String::from_utf8_lossy(&raw).into_owned();
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }
        Ok(Some((line, raw)))
    }
}
#[derive(Default)]
pub struct Files {
    transaction: Mutex<()>,
}
pub struct WalkResult {
    pub paths: Vec<String>,
    pub truncated: bool,
}
pub struct Location {
    pub root: Dir,
    pub relative: PathBuf,
    pub virtual_path: String,
    pub approved: PathBuf,
}
pub fn handle_path(file: &File) -> io::Result<PathBuf> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
        let mut data = vec![0u16; 32768];
        let n = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                data.as_mut_ptr(),
                data.len() as u32,
                0,
            )
        };
        if n == 0 || n >= data.len() as u32 {
            return Err(io::Error::last_os_error());
        }
        use std::os::windows::ffi::OsStringExt;
        Ok(PathBuf::from(OsString::from_wide(&data[..n as usize])))
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStringExt;
        let mut bytes = vec![0u8; 1024];
        let n = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, bytes.as_mut_ptr()) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        let at = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        bytes.truncate(at);
        Ok(PathBuf::from(OsString::from_vec(bytes)))
    }
}
pub fn locate(
    roots: &[Root],
    input: &str,
    base: Option<&str>,
    missing: bool,
) -> io::Result<Location> {
    let r = sandbox::resolve(roots, input, base, missing)?;
    let approved = std::fs::canonicalize(&r.root.path)?;
    let root = Dir::open_ambient_dir(&approved, cap_std::ambient_authority())?;
    let actual = handle_path(&root.try_clone()?.into_std_file())?;
    if !sandbox::contained(&approved, &actual) || !sandbox::contained(&actual, &approved) {
        return Err(invalid("Approved root changed during open"));
    }
    if !sandbox::contained(&approved, &r.real) {
        return Err(invalid("Path escaped approved root"));
    }
    let relative = r
        .real
        .components()
        .skip(approved.components().count())
        .collect::<PathBuf>();
    let relative = if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    };
    Ok(Location {
        root,
        relative,
        virtual_path: r.virtual_path,
        approved,
    })
}
impl Location {
    pub fn open_handle(&self) -> io::Result<File> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = self.root.open_with(&self.relative, &options)?.into_std();
        if !file.metadata()?.is_file() {
            return Err(invalid("Not a regular file"));
        }
        if !sandbox::contained(&self.approved, &handle_path(&file)?) {
            return Err(invalid("Opened file escaped approved root"));
        }
        Ok(file)
    }
    pub fn open(&self, max: u64) -> io::Result<Vec<u8>> {
        let file = self.open_handle()?;
        if file.metadata()?.len() > max {
            return Err(invalid(format!("File exceeds {max} byte limit")));
        }
        let mut bytes = vec![];
        file.take(max + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max {
            return Err(invalid("File grew beyond the read limit"));
        }
        Ok(bytes)
    }
    fn parent(&self) -> io::Result<(Dir, OsString)> {
        let name = self
            .relative
            .file_name()
            .ok_or_else(|| invalid("Cannot mutate an approved root"))?
            .to_os_string();
        let parent = self.root.open_dir(
            self.relative
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )?;
        let actual = handle_path(&parent.try_clone()?.into_std_file())?;
        if !sandbox::contained(&self.approved, &actual) {
            return Err(invalid("Parent escaped approved root"));
        }
        Ok((parent, name))
    }
    fn optional_limit(&self, limit: u64) -> io::Result<Option<Vec<u8>>> {
        match self.open(limit) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
struct Change {
    location: Location,
    old: Option<Vec<u8>>,
    new: Option<Vec<u8>>,
}
struct Stage {
    parent: Dir,
    name: OsString,
    temp: Option<OsString>,
    backup: Option<OsString>,
    approved: PathBuf,
    file: Option<File>,
    write: WriteProgress,
}
#[derive(Default)]
struct WriteProgress {
    written: usize,
    truncated: bool,
}
impl WriteProgress {
    fn matches(&self, current: &[u8], old: &[u8], new: &[u8]) -> bool {
        if self.truncated {
            return current == new;
        }
        let n = self.written;
        current.len() == old.len().max(n)
            && current.get(..n) == new.get(..n)
            && current.get(n..) == Some(old.get(n..).unwrap_or_default())
    }
}
fn stage_file(parent: &Dir, bytes: &[u8]) -> io::Result<OsString> {
    let name = OsString::from(format!(".cos-rust-{}.partial", uuid::Uuid::new_v4()));
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    let mut file = parent.open_with(&name, &opts)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = parent.remove_file(&name);
        return Err(error);
    }
    Ok(name)
}
fn read_edit(file: &mut File, max: u64) -> io::Result<Vec<u8>> {
    if file.metadata()?.len() > max {
        return Err(invalid("File exceeds edit limit"));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = vec![];
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(invalid("File grew beyond edit limit"));
    }
    Ok(bytes)
}
fn replace_contents(file: &mut File, bytes: &[u8]) -> io::Result<()> {
    write_contents(file, bytes, &mut WriteProgress::default())
}
fn write_contents(file: &mut File, bytes: &[u8], progress: &mut WriteProgress) -> io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    #[cfg(test)]
    let fail_after = FAIL_WRITE_AFTER.with(|f| f.take());
    while progress.written < bytes.len() {
        #[cfg(test)]
        if fail_after.is_some_and(|after| progress.written >= after) {
            return Err(io::Error::other("Injected partial write failure"));
        }
        let end = bytes.len();
        #[cfg(test)]
        let end = fail_after.map_or(end, |after| end.min(after));
        match file.write(&bytes[progress.written..end]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "Partial edit stopped",
                ));
            }
            Ok(n) => progress.written += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    file.set_len(bytes.len() as u64)?;
    progress.truncated = true;
    file.sync_all()
}

#[cfg(test)]
thread_local! { static FAIL_WRITE_AFTER: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }

impl Files {
    pub fn read(
        &self,
        roots: &[Root],
        path: &str,
        start: usize,
        end: usize,
        budget: usize,
    ) -> io::Result<Value> {
        if start == 0 || end < start {
            return Err(invalid(
                "endLine must be greater than or equal to startLine",
            ));
        }
        let l = locate(roots, path, None, false)?;
        let meta = l.root.metadata(&l.relative)?;
        if meta.is_dir() {
            return self.list(roots, path, 200);
        }
        let file = l.open_handle()?;
        let size = file.metadata()?.len();
        preview(file, &l.virtual_path, size, start, end, budget)
    }
    pub fn list(&self, roots: &[Root], path: &str, limit: usize) -> io::Result<Value> {
        let l = locate(roots, path, None, false)?;
        let dir = l.root.open_dir(&l.relative)?;
        let mut entries = vec![];
        for entry in dir.entries()?.take(5001) {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry.metadata()?;
            entries.push(json!({"name":name,"directory":metadata.is_dir(),"size":metadata.len()}));
        }
        entries.sort_by(|a, b| {
            b["directory"]
                .as_bool()
                .cmp(&a["directory"].as_bool())
                .then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
        });
        let truncated = entries.len() > limit;
        entries.truncate(limit.min(200));
        Ok(json!({"path":l.virtual_path,"entries":entries,"truncated":truncated}))
    }
    pub fn revision(&self, roots: &[Root], path: &str) -> io::Result<String> {
        Ok(digest(
            &locate(roots, path, None, false)?.open(MAX_EDIT_BYTES)?,
        ))
    }
    pub fn save(
        &self,
        roots: &[Root],
        path: &str,
        contents: &str,
        revision: &str,
    ) -> io::Result<String> {
        let _guard = lock(&self.transaction)?;
        let location = locate(roots, path, None, false)?;
        let old = location.open(MAX_EDIT_BYTES)?;
        if digest(&old) != revision {
            return Err(invalid(
                "FILE_REVISION_CHANGED: refresh the file before saving",
            ));
        }
        let (_, encoding) = text::decode(&old)?;
        let new = text::encode(contents, encoding);
        let rev = digest(&new);
        self.commit(
            vec![Change {
                location,
                old: Some(old),
                new: Some(new),
            }],
            None,
        )?;
        Ok(rev)
    }
    pub fn apply(&self, roots: &[Root], base: &str, source: &str) -> io::Result<Value> {
        self.apply_with_failure(roots, base, source, None)
    }
    /// Deterministic failure injection used by transaction qualification, never exposed by MCP.
    pub fn apply_with_failure(
        &self,
        roots: &[Root],
        base: &str,
        source: &str,
        fail_after: Option<usize>,
    ) -> io::Result<Value> {
        self.apply_checked(roots, base, source, fail_after, &|_| Ok(()))
    }
    pub fn apply_checked(
        &self,
        roots: &[Root],
        base: &str,
        source: &str,
        fail_after: Option<usize>,
        authorize: &dyn Fn(&str) -> io::Result<()>,
    ) -> io::Result<Value> {
        self.apply_checked_mode(
            roots,
            base,
            source,
            fail_after,
            patch::FileUpdateMode::PreserveLineEndings,
            authorize,
        )
    }
    pub fn apply_checked_mode(
        &self,
        roots: &[Root],
        base: &str,
        source: &str,
        fail_after: Option<usize>,
        mode: patch::FileUpdateMode,
        authorize: &dyn Fn(&str) -> io::Result<()>,
    ) -> io::Result<Value> {
        let _guard = lock(&self.transaction)?;
        authorize("")?;
        crate::patch_budget::check_source(source)?;
        let parsed = patch::parse_typed(source).map_err(invalid)?;
        if parsed.environment_id.is_some() {
            return Err(invalid(
                "apply_patch environment selection is unavailable for this turn",
            ));
        }
        let mut changes = BTreeMap::<String, Change>::new();
        let mut budget = crate::patch_budget::Budget::new(&parsed.hunks)?;
        fn get<'a>(
            changes: &'a mut BTreeMap<String, Change>,
            roots: &[Root],
            path: &str,
            base: &str,
            budget: &mut crate::patch_budget::Budget,
        ) -> io::Result<&'a mut Change> {
            let location = locate(roots, path, Some(base), true)?;
            let key = location.virtual_path.clone();
            if !changes.contains_key(&key) {
                let old = location.optional_limit(budget.read_limit(&key)? as u64)?;
                budget.bytes(&key, old.as_deref().unwrap_or_default())?;
                changes.insert(
                    key.clone(),
                    Change {
                        location,
                        new: old.clone(),
                        old,
                    },
                );
            }
            Ok(changes.get_mut(&key).unwrap())
        }
        for hunk in parsed.hunks {
            match hunk {
                Hunk::AddFile { path, contents } => {
                    let change = get(&mut changes, roots, &path, base, &mut budget)?;
                    budget.bytes(&change.location.virtual_path, contents.as_bytes())?;
                    authorize(if change.new.is_some() {
                        "edit"
                    } else {
                        "create"
                    })?;
                    change.new = Some(contents.into_bytes());
                }
                Hunk::DeleteFile { path } => {
                    let c = get(&mut changes, roots, &path, base, &mut budget)?;
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
                    let c = get(&mut changes, roots, &path, base, &mut budget)?;
                    let raw = c
                        .new
                        .as_ref()
                        .ok_or_else(|| invalid(format!("Failed to read file to update {path}")))?;
                    let contents = std::str::from_utf8(raw)
                        .map_err(|_| invalid("stream did not contain valid UTF-8 (EILSEQ)"))?;
                    budget.update(&c.location.virtual_path, raw, &chunks)?;
                    let new = patch::update_with_mode(contents, &path, &chunks, mode)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
                        .into_bytes();
                    if &new != raw {
                        authorize("edit")?;
                    }
                    if let Some(destination) = move_path {
                        let source_path = c.location.virtual_path.clone();
                        c.new = None;
                        let target = get(&mut changes, roots, &destination, base, &mut budget)?;
                        let same_path = if cfg!(windows) {
                            target.location.virtual_path.to_lowercase()
                                == source_path.to_lowercase()
                        } else {
                            target.location.virtual_path == source_path
                        };
                        if target.new.is_some() && !same_path {
                            authorize("edit")?;
                        }
                        budget.bytes(&target.location.virtual_path, &new)?;
                        target.new = Some(new);
                    } else {
                        c.new = Some(new);
                    }
                }
            }
        }
        let report=changes.iter().map(|(path,c)|json!({"path":path,"kind":if c.old.is_none(){"add"}else if c.new.is_none(){"delete"}else{"update"},"revision":c.new.as_ref().map(|v|digest(v))})).collect::<Vec<_>>();
        self.commit_checked(changes.into_values().collect(), fail_after, &|| {
            authorize("")
        })?;
        Ok(
            json!({"changes":report,"text":"Success. Updated the following files:","transaction":"committed"}),
        )
    }
    fn commit(&self, changes: Vec<Change>, fail_after: Option<usize>) -> io::Result<()> {
        self.commit_checked(changes, fail_after, &|| Ok(()))
    }
    fn commit_checked(
        &self,
        changes: Vec<Change>,
        fail_after: Option<usize>,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        let mut stages: Vec<Stage> = vec![];
        let mut created: Vec<(Dir, PathBuf)> = vec![];
        let prepare = (|| -> io::Result<()> {
            for c in &changes {
                authorize()?;
                if c.location
                    .optional_limit(c.old.as_ref().map_or(0, |b| b.len()) as u64)?
                    != c.old
                {
                    return Err(invalid("FILE_REVISION_CHANGED: no edits were applied"));
                }
                if let Some(parent) = c.location.relative.parent() {
                    let mut part = PathBuf::new();
                    for component in parent.components() {
                        part.push(component);
                        if c.location.root.open_dir(&part).is_err() {
                            c.location.root.create_dir(&part)?;
                            created.push((c.location.root.try_clone()?, part.clone()));
                        }
                    }
                }
                let (parent, name) = c.location.parent()?;
                stages.push(Stage {
                    parent,
                    name,
                    temp: None,
                    backup: None,
                    approved: c.location.approved.clone(),
                    file: None,
                    write: WriteProgress::default(),
                });
                let stage = stages.last_mut().unwrap();
                if let Some(old) = &c.old {
                    let mut options = OpenOptions::new();
                    options.read(true).write(c.new.is_some());
                    #[cfg(windows)]
                    {
                        use cap_std::fs::OpenOptionsExt;
                        options.share_mode(if c.new.is_some() { 0 } else { 4 });
                    }
                    #[cfg(unix)]
                    {
                        use cap_std::fs::OpenOptionsExt;
                        options.custom_flags(libc::O_NONBLOCK);
                    }
                    let mut file = stage.parent.open_with(&stage.name, &options)?.into_std();
                    if !file.metadata()?.is_file()
                        || !sandbox::contained(&stage.approved, &handle_path(&file)?)
                    {
                        return Err(invalid("Opened edit handle escaped approved root"));
                    }
                    if read_edit(&mut file, old.len() as u64)? != *old {
                        return Err(invalid("FILE_REVISION_CHANGED: no edits were applied"));
                    }
                    stage.file = Some(file);
                    if c.new.is_none() {
                        stage.backup =
                            Some(format!(".cos-rust-{}.backup", uuid::Uuid::new_v4()).into());
                    } else {
                        stage.backup = match stage_file(&stage.parent, old) {
                            Ok(name) => Some(name),
                            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => None,
                            Err(e) => return Err(e),
                        };
                    }
                } else if let Some(new) = &c.new {
                    stage.temp = Some(stage_file(&stage.parent, new)?);
                }
            }
            Ok(())
        })();
        let mut committed = 0;
        let result = prepare.and_then(|_| {
            for (i, s) in stages.iter_mut().enumerate() {
                authorize()?;
                if fail_after == Some(i) {
                    return Err(io::Error::other("Injected transaction commit failure"));
                }
                if !sandbox::contained(
                    &s.approved,
                    &handle_path(&s.parent.try_clone()?.into_std_file())?,
                ) {
                    return Err(invalid(
                        "Parent moved outside approved root during transaction",
                    ));
                }
                if let Some(file) = s.file.as_mut() {
                    if !sandbox::contained(&s.approved, &handle_path(file)?)
                        || read_edit(file, changes[i].old.as_ref().unwrap().len() as u64)?
                            != *changes[i].old.as_ref().unwrap()
                    {
                        return Err(invalid("FILE_REVISION_CHANGED during transaction"));
                    }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        let current = s.parent.open(&s.name)?.into_std().metadata()?;
                        let held = file.metadata()?;
                        if (current.dev(), current.ino()) != (held.dev(), held.ino()) {
                            return Err(invalid("FILE_REVISION_CHANGED: edit target was replaced"));
                        }
                    }
                    if let Some(new) = &changes[i].new {
                        committed = i + 1; // A partial write also enters guarded rollback.
                        write_contents(file, new, &mut s.write)?;
                    } else {
                        s.parent
                            .rename(&s.name, &s.parent, s.backup.as_ref().unwrap())?;
                        committed = i + 1;
                    }
                } else if let Some(temp) = &s.temp {
                    // hard_link publishes only if the destination is still absent.
                    s.parent.hard_link(temp, &s.parent, &s.name)?;
                    committed = i + 1;
                } else {
                    return Err(invalid("Invalid transaction stage"));
                }
            }
            Ok(())
        });
        let mut rollback_errors = vec![];
        if result.is_err() {
            for (i, s) in stages[..committed].iter_mut().enumerate().rev() {
                let rollback = (|| -> io::Result<()> {
                    if let (Some(old), Some(new), Some(file)) =
                        (&changes[i].old, &changes[i].new, s.file.as_mut())
                    {
                        let current = read_edit(file, old.len().max(new.len()) as u64)?;
                        if !sandbox::contained(&s.approved, &handle_path(file)?) {
                            return Err(invalid(
                                "Edit target moved outside approved root; backup retained",
                            ));
                        }
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::MetadataExt;
                            let target = s.parent.open(&s.name)?.into_std().metadata()?;
                            let held = file.metadata()?;
                            if (target.dev(), target.ino()) != (held.dev(), held.ino()) {
                                return Err(invalid(
                                    "Concurrent replacement; original backup retained",
                                ));
                            }
                        }
                        if current == *old {
                            return Ok(());
                        }
                        if !s.write.matches(&current, old, new) {
                            return Err(invalid(
                                "Concurrent file change; original backup retained",
                            ));
                        }
                        replace_contents(file, old)
                    } else if changes[i].old.is_some() {
                        match s.parent.symlink_metadata(&s.name) {
                            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                            _ => {
                                return Err(invalid(
                                    "Concurrent replacement; delete rollback backup retained",
                                ));
                            }
                        }
                        s.parent
                            .rename(s.backup.as_ref().unwrap(), &s.parent, &s.name)
                    } else {
                        let mut current = s.parent.open(&s.name)?.into_std();
                        if read_edit(&mut current, changes[i].new.as_ref().unwrap().len() as u64)?
                            != *changes[i].new.as_ref().unwrap()
                        {
                            return Err(invalid("Concurrent change; newly created file retained"));
                        }
                        s.parent.remove_file(&s.name)
                    }
                })();
                if let Err(e) = rollback {
                    rollback_errors.push(e.to_string());
                }
            }
        }
        // Failed rollback backups remain beside their files as recovery evidence.
        for s in &stages {
            if let Some(temp) = &s.temp {
                let _ = s.parent.remove_file(temp);
            }
            if rollback_errors.is_empty()
                && let Some(backup) = &s.backup
            {
                let _ = s.parent.remove_file(backup);
            }
        }
        drop(stages); // Windows directory handles must settle before removing newly created parents.
        if result.is_err() {
            for (dir, path) in created.iter().rev() {
                let _ = dir.remove_dir(path);
            }
        }
        if !rollback_errors.is_empty() {
            return Err(io::Error::other(format!(
                "Transaction rollback incomplete; retained .cos-rust backups: {}",
                rollback_errors.join("; ")
            )));
        }
        result
    }
    pub fn glob(&self, roots: &[Root], pattern: &str) -> io::Result<Vec<String>> {
        let at = pattern.find(['*', '?']).unwrap_or(pattern.len());
        let prefix = &pattern[..at];
        let base = prefix
            .rsplit_once('/')
            .map(|(head, _)| head)
            .unwrap_or(prefix);
        let base = if base.is_empty() { pattern } else { base };
        let glob = glob_regex(pattern)?;
        let paths = self.walk(
            roots,
            base,
            &[],
            5000,
            Instant::now() + std::time::Duration::from_secs(10),
        )?;
        Ok(paths
            .into_iter()
            .filter(|p| glob.is_match(p))
            .take(20)
            .collect())
    }
    pub fn walk(
        &self,
        roots: &[Root],
        base: &str,
        excludes: &[String],
        limit: usize,
        deadline: Instant,
    ) -> io::Result<Vec<String>> {
        Ok(self
            .walk_bounded(roots, base, excludes, limit, deadline)?
            .paths)
    }
    pub fn walk_bounded(
        &self,
        roots: &[Root],
        base: &str,
        excludes: &[String],
        limit: usize,
        deadline: Instant,
    ) -> io::Result<WalkResult> {
        self.walk_checked(roots, base, excludes, limit, deadline, &|| Ok(()))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn walk_checked(
        &self,
        roots: &[Root],
        base: &str,
        excludes: &[String],
        limit: usize,
        deadline: Instant,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<WalkResult> {
        authorize()?;
        let start = locate(roots, base, None, false)?;
        let mut todo = vec![(
            start.relative.clone(),
            start.virtual_path.trim_end_matches('/').to_string(),
        )];
        let mut files = vec![];
        let mut scanned = 0;
        let mut truncated = false;
        while let Some((path, virtual_path)) = todo.pop() {
            authorize()?;
            if scanned >= limit || Instant::now() > deadline {
                truncated = true;
                break;
            }
            let dir = start.root.open_dir(&path)?;
            let remaining = limit.saturating_sub(scanned);
            let mut entries = vec![];
            for entry in dir.entries()?.take(remaining.saturating_add(1)) {
                authorize()?;
                if Instant::now() > deadline {
                    truncated = true;
                    break;
                }
                entries.push(entry?);
            }
            if entries.len() > remaining {
                truncated = true;
                entries.truncate(remaining);
            }
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                authorize()?;
                scanned += 1;
                if scanned > limit || Instant::now() > deadline {
                    truncated = true;
                    break;
                }
                let meta = entry.metadata()?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let child = path.join(entry.file_name());
                let virtual_child = format!("{virtual_path}/{name}");
                if meta.is_dir() {
                    if !excludes.iter().any(|e| {
                        if let Some(p) = e.strip_suffix('*') {
                            name.to_lowercase().starts_with(&p.to_lowercase())
                        } else {
                            name.eq_ignore_ascii_case(e)
                        }
                    }) && !entry.file_type()?.is_symlink()
                    {
                        todo.push((child, virtual_child));
                    }
                } else if meta.is_file() {
                    files.push(virtual_child);
                }
            }
        }
        files.sort();
        authorize()?;
        Ok(WalkResult {
            paths: files,
            truncated,
        })
    }
}
pub fn glob_regex(pattern: &str) -> io::Result<regex::Regex> {
    let mut out = String::from("^");
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    if chars.peek() == Some(&'/') {
                        chars.next();
                        out.push_str("(?:.*/)?");
                    } else {
                        out.push_str(".*");
                    }
                } else {
                    out.push_str("[^/]*");
                }
            }
            '?' => out.push_str("[^/]"),
            _ => out.push_str(&regex::escape(&c.to_string())),
        }
    }
    out.push('$');
    regex::Regex::new(&out).map_err(|e| invalid(e.to_string()))
}

/// Shared bounded decoder for local and opened SFTP handles.
pub fn preview(
    input: impl Read,
    path: &str,
    size: u64,
    start: usize,
    end: usize,
    budget: usize,
) -> io::Result<Value> {
    if start == 0 || end < start {
        return Err(invalid("Invalid line range"));
    }
    let mut reader = BufReader::with_capacity(8192, input);
    let head = reader.fill_buf()?;
    let (encoding, bom) = if head.starts_with(&[255, 254]) {
        (text::Encoding::Utf16Le, 2)
    } else if head.starts_with(&[254, 255]) {
        (text::Encoding::Utf16Be, 2)
    } else if head.starts_with(&[239, 187, 191]) {
        (text::Encoding::Utf8Bom, 3)
    } else {
        (text::Encoding::Utf8, 0)
    };
    if bom == 0 && head.contains(&0) {
        return Err(invalid("Binary file; text decoding is unavailable"));
    }
    let mut hasher = Sha256::new();
    hasher.update(&head[..bom]);
    reader.consume(bom);
    let mut output = vec![];
    let mut bytes = 0;
    let mut last = start.saturating_sub(1);
    let budget = budget.clamp(1, MAX_READ_BYTES);
    let mut line_no = 0;
    let mut scanned = bom;
    let mut stopped = false;
    let mut exhausted = false;
    let mut more = false;
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if Instant::now() > deadline {
            stopped = true;
            more = true;
            break;
        }
        let Some((line, raw)) = stream_line(&mut reader, encoding)? else {
            break;
        };
        scanned += raw.len();
        hasher.update(&raw);
        line_no += 1;
        if line_no < start {
            continue;
        }
        if line_no > end || exhausted {
            more = true;
            if size > 4 * 1024 * 1024 || scanned > 4 * 1024 * 1024 {
                stopped = true;
                break;
            }
            continue;
        }
        if bytes + line.len() + 1 > budget {
            if output.is_empty() {
                return Err(invalid(format!(
                    "Line {line_no} is larger than max_bytes={budget}. Increase max_bytes (up to {MAX_READ_BYTES}) or use exec_command to transform it."
                )));
            }
            exhausted = true;
            more = true;
            if size > 4 * 1024 * 1024 || scanned > 4 * 1024 * 1024 {
                stopped = true;
                break;
            }
            continue;
        }
        bytes += line.len() + 1;
        last = line_no;
        output.push(line);
    }
    if stopped && line_no < start {
        return Err(invalid("Read deadline reached before the requested range"));
    }
    let total = if stopped { None } else { Some(line_no) };
    Ok(
        json!({"path":path,"size":size,"text":output.join("\n"),"firstLine":start,"lastLine":last,"totalLines":total,"truncated":exhausted,"more":more,"encoding":format!("{encoding:?}"),"revision":if stopped{None}else{Some(format!("{:x}",hasher.finalize()))}}),
    )
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn partial_write_failure_restores_original_contents() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a"), "original contents\n").unwrap();
        let roots = [Root {
            name: "p".into(),
            path: d.path().to_string_lossy().into_owned(),
            remote: None,
        }];
        FAIL_WRITE_AFTER.with(|f| f.set(Some(4)));
        let error = Files::default().apply(&roots, "/p", "*** Begin Patch\n*** Update File: a\n@@\n-original contents\n+replacement text\n*** End Patch").unwrap_err();
        assert!(
            error.to_string().contains("Injected partial write"),
            "{error}"
        );
        assert_eq!(
            std::fs::read(d.path().join("a")).unwrap(),
            b"original contents\n"
        );
        assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
    }
}
