//! Human project file-panel owner; separate from model-facing file tools.
use crate::{files, project::Projects};
use base64::{Engine, engine::general_purpose::STANDARD};
use cap_std::fs::{Dir, OpenOptions};
use cos_core::{
    config::{Root, js_len},
    persistence::{invalid, lock},
    sandbox,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
mod native;
mod ordering;
mod server;
// Git review shares the same native identity/change stamp as optimistic saves.
pub(crate) fn file_stamp(file: &File) -> io::Result<Value> {
    native::stamp(file)
}
const TEXT_BYTES: usize = 256 * 1024;
const IMAGE_BYTES: usize = 5 * 1024 * 1024;
const PDF_BYTES: usize = 20 * 1024 * 1024;
const STALE: &str = "File changed on disk. Reload it before saving your edits.";
#[derive(Default)]
pub struct ProjectFiles {
    saves: Mutex<BTreeSet<String>>,
    mutations: Mutex<()>,
}
pub struct Edit<'a> {
    pub text: &'a str,
    pub modified_at: &'a str,
    pub bytes: u64,
    pub revision: &'a str,
}
pub struct ServerSnapshot {
    pub file: PathBuf,
    _directory: tempfile::TempDir,
}
struct Context<'a> {
    projects: &'a Projects,
    roots: &'a [Root],
    id: &'a str,
    name: String,
    real: PathBuf,
    virtual_path: String,
    dir: Dir,
    identity: same_file::Handle,
}
fn relative(input: &str) -> io::Result<String> {
    if js_len(input) > 4096 {
        return Err(invalid("Project path is invalid"));
    }
    if input.is_empty() {
        return Ok(String::new());
    }
    if input.starts_with(['/', '\\'])
        || input
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && input.as_bytes().get(1) == Some(&b':')
            && input.as_bytes().get(2).is_some_and(|b| b"/\\".contains(b))
    {
        return Err(invalid("Project paths must be relative"));
    }
    let parts = input.split(['/', '\\']).collect::<Vec<_>>();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err(invalid("Project path contains an invalid segment"));
    }
    Ok(parts.join("/"))
}
fn leaf(input: &str) -> io::Result<&str> {
    if input.is_empty() || js_len(input) > 255 {
        return Err(invalid("Name is invalid"));
    }
    if matches!(input, "." | "..") || input.contains(['/', '\\', '\0']) {
        return Err(invalid("Name must be one file or folder name"));
    }
    Ok(input)
}
fn child(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.into()
    } else {
        format!("{parent}/{name}")
    }
}
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}
fn refuse_server_edit(projects: &Projects, id: &str) -> io::Result<()> {
    if projects
        .list()?
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == id && p.get("remote").is_some())
    {
        return Err(invalid(
            "Files in a server folder cannot be changed from the app yet; ChatGPT can still change them with its tools.",
        ));
    }
    Ok(())
}
fn kind(meta: &cap_std::fs::Metadata) -> &'static str {
    if meta.is_dir() {
        "directory"
    } else if meta.is_file() {
        "file"
    } else {
        "other"
    }
}
fn is_link(meta: &cap_std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
impl<'a> Context<'a> {
    fn open(projects: &'a Projects, roots: &'a [Root], id: &'a str) -> io::Result<Self> {
        let workspace = projects.workspace(roots, id)?;
        let name = projects
            .list()?
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .and_then(|p| p["name"].as_str())
            .ok_or_else(|| invalid("Project not found"))?
            .to_owned();
        let dir = Dir::open_ambient_dir(&workspace.real, cap_std::ambient_authority())?;
        let identity = same_file::Handle::from_file(dir.try_clone()?.into_std_file())?;
        let context = Self {
            projects,
            roots,
            id,
            name,
            real: workspace.real,
            virtual_path: workspace.virtual_path,
            dir,
            identity,
        };
        context.validate()?;
        Ok(context)
    }
    fn validate(&self) -> io::Result<()> {
        let current = self.projects.workspace(self.roots, self.id)?;
        let reopened = Dir::open_ambient_dir(&current.real, cap_std::ambient_authority())?;
        if !sandbox::contained(&current.real, &self.real)
            || !sandbox::contained(&self.real, &current.real)
            || same_file::Handle::from_file(reopened.into_std_file())? != self.identity
            || files::handle_path(&self.dir.try_clone()?.into_std_file())? != self.real
        {
            return Err(invalid(
                "The project folder changed location. Reload it before continuing.",
            ));
        }
        Ok(())
    }
    fn target(&self, path: &str, missing: bool, allow_root: bool) -> io::Result<(String, PathBuf)> {
        let path = relative(path)?;
        if path.is_empty() && !allow_root {
            return Err(invalid("The project root cannot be changed"));
        }
        self.validate()?;
        let mut tail = PathBuf::new();
        let parts = path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        for (index, part) in parts.iter().enumerate() {
            tail.push(part);
            match self.dir.symlink_metadata(&tail) {
                Ok(meta) if is_link(&meta) => {
                    return Err(invalid("Files does not follow symbolic links or junctions"));
                }
                Err(e)
                    if missing
                        && index + 1 == parts.len()
                        && e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
                _ => {}
            }
        }
        let virtual_path = if path.is_empty() {
            self.virtual_path.clone()
        } else {
            child(&self.virtual_path, &path)
        };
        let resolved = sandbox::resolve(self.roots, &virtual_path, None, missing)?;
        if !sandbox::contained(&self.real, &resolved.real) {
            return Err(invalid("Path leaves the selected project folder"));
        }
        Ok((path, resolved.real))
    }
    fn directory(&self, path: &str) -> io::Result<Dir> {
        let (path, real) = self.target(path, false, true)?;
        let dir = self
            .dir
            .open_dir(if path.is_empty() { "." } else { &path })?;
        if files::handle_path(&dir.try_clone()?.into_std_file())? != real {
            return Err(invalid(
                "The project folder changed location. Reload it before continuing.",
            ));
        }
        Ok(dir)
    }
    fn open_file(&self, path: &str) -> io::Result<File> {
        let (path, real) = self.target(path, false, false)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = self.dir.open_with(&path, &options)?.into_std();
        if !file.metadata()?.is_file() {
            return Err(invalid("Choose a regular file"));
        }
        if files::handle_path(&file)? != real {
            return Err(invalid(
                "The project file changed location. Reload it before continuing.",
            ));
        }
        Ok(file)
    }
    fn check_parent(&self, path: &str, held: &Dir) -> io::Result<()> {
        let current = self.directory(path)?;
        if same_file::Handle::from_file(current.into_std_file())?
            != same_file::Handle::from_file(held.try_clone()?.into_std_file())?
        {
            return Err(invalid(
                "The project folder changed location. Reload it before continuing.",
            ));
        }
        Ok(())
    }
    fn check_file(&self, path: &str, file: &File, stamp: &Value) -> io::Result<()> {
        let current = self.open_file(path)?;
        if native::stamp(file)? != *stamp
            || native::stamp(&current)? != *stamp
            || same_file::Handle::from_file(file.try_clone()?)?
                != same_file::Handle::from_file(current)?
        {
            return Err(invalid(STALE));
        }
        self.validate()
    }
    fn bytes(&self, path: &str, limit: usize) -> io::Result<Snapshot> {
        let mut file = self.open_file(path)?;
        let stamp = native::stamp(&file)?;
        let size = file.metadata()?.len();
        if size > limit as u64 {
            return Err(invalid("This file is too large for the editor"));
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != size {
            return Err(invalid("File changed while being read"));
        }
        self.check_file(path, &file, &stamp)?;
        let revision = snapshot_revision(&bytes, &stamp)?;
        Ok(Snapshot {
            file,
            bytes,
            revision,
        })
    }
    fn text(&self, path: &str) -> io::Result<(Snapshot, String)> {
        let snapshot = self.bytes(path, TEXT_BYTES)?;
        let text = String::from_utf8(snapshot.bytes.clone())
            .map_err(|_| invalid("This file is not UTF-8 text and cannot be edited here"))?;
        if text
            .chars()
            .any(|c| matches!(c,'\0'..='\u{8}'|'\u{b}'|'\u{c}'|'\u{e}'..='\u{1f}'|'\u{7f}'))
        {
            return Err(invalid("Binary files cannot be edited here"));
        }
        Ok((snapshot, text))
    }
}
struct Snapshot {
    file: File,
    bytes: Vec<u8>,
    revision: String,
}
fn snapshot_revision(bytes: &[u8], stamp: &Value) -> io::Result<String> {
    // Source Stats fields are JS Numbers. Serializing an inode as an exact Rust
    // u64 produces a different digest above 2^53, even for the same f64 value.
    let mut encoded = String::from("[");
    let mut buffer = ryu_js::Buffer::new();
    for (index, value) in stamp
        .as_array()
        .ok_or_else(|| invalid("Invalid file stamp"))?
        .iter()
        .enumerate()
    {
        if index != 0 {
            encoded.push(',');
        }
        let number = value
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| invalid("Invalid file stamp number"))?;
        encoded.push_str(buffer.format_finite(number));
    }
    encoded.push(']');
    let mut hash = Sha256::new();
    hash.update(bytes);
    hash.update(encoded.as_bytes());
    Ok(format!("{:x}", hash.finalize()))
}
fn modified(file: &File) -> io::Result<String> {
    // Node Stats lazily constructs its Date using Math.round(mtimeMs).
    // Keep revision precision separate from the rounded display/save timestamp.
    let ms = native::stamp(file)?[3]
        .as_f64()
        .ok_or_else(|| invalid("Invalid file timestamp"))?;
    let rounded = ms.floor() + f64::from(ms - ms.floor() >= 0.5);
    Ok(
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(rounded as i64)
            .ok_or_else(|| invalid("File timestamp is out of range"))?
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )
}
fn metadata_modified(file: &File) -> io::Result<String> {
    let ms = native::stamp(file)?[3]
        .as_f64()
        .ok_or_else(|| invalid("Invalid file timestamp"))?;
    Ok(
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms.max(0.).trunc() as i64)
            .ok_or_else(|| invalid("File timestamp is out of range"))?
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )
}
pub(crate) fn sniff(head: &[u8]) -> bool {
    if head.is_empty()
        || head.starts_with(&[255, 254])
        || head.starts_with(&[254, 255])
        || head.starts_with(&[239, 187, 191])
    {
        return false;
    }
    head.contains(&0)
        || head
            .iter()
            .filter(|b| **b < 32 && !b"\t\n\r".contains(b) || **b == 127)
            .count() as f64
            / head.len() as f64
            > 0.3
}
fn image_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" | "ico"
            )
        })
}
fn thumbnail(bytes: &[u8]) -> io::Result<Vec<u8>> {
    use image::{ImageDecoder, ImageFormat, ImageReader, Limits, imageops::FilterType};
    let mut reader = ImageReader::new(io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(io::Error::other)?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > 16 * 1024 * 1024 {
        return Err(invalid("Image exceeds pixel limit"));
    }
    let orientation = decoder.orientation().map_err(io::Error::other)?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(io::Error::other)?;
    image.apply_orientation(orientation);
    if image.width() > 1600 || image.height() > 1600 {
        image = image.resize(1600, 1600, FilterType::Lanczos3);
    }
    let mut output = io::Cursor::new(vec![]);
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(io::Error::other)?;
    Ok(output.into_inner())
}
impl ProjectFiles {
    pub fn list(
        &self,
        projects: &Projects,
        roots: &[Root],
        id: &str,
        input: &str,
    ) -> io::Result<Value> {
        let context = Context::open(projects, roots, id)?;
        let path = relative(input)?;
        let dir = context.directory(&path)?;
        let collator = ordering::collator()?;
        let mut entries = BTreeMap::new();
        let mut truncated = false;
        // Retain the sorted prefix without materializing the whole directory.
        for (index, entry) in dir.entries()?.enumerate() {
            if index >= 100_000 {
                return Err(invalid("Project directory exceeds the scan limit"));
            }
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("Project filename is not valid Unicode"))?;
            let meta = dir.symlink_metadata(&name)?;
            let kind = if is_link(&meta) { "other" } else { kind(&meta) };
            entries.insert(
                ordering::Key {
                    collator: &collator,
                    non_directory: kind != "directory",
                    name: name.clone(),
                    index,
                },
                json!({"name":name,"path":child(&path,&name),"kind":kind,"bytes":null}),
            );
            if entries.len() > 500 {
                entries.pop_last();
                truncated = true;
            }
        }
        let entries = entries
            .into_values()
            .filter(|entry| !matches!(entry["name"].as_str(), Some(".git" | ".DS_Store")))
            .collect::<Vec<_>>();
        context.target(&path, false, true)?;
        Ok(
            json!({"projectId":id,"projectName":context.name,"directory":path,"entries":entries,"truncated":truncated}),
        )
    }
    pub fn preview(
        &self,
        projects: &Projects,
        roots: &[Root],
        id: &str,
        input: &str,
    ) -> io::Result<Value> {
        let context = Context::open(projects, roots, id)?;
        let (path, real) = context.target(input, false, false)?;
        let mut file = context.open_file(&path)?;
        let size = file.metadata()?.len();
        let stamp = native::stamp(&file)?;
        let mut result = json!({"projectId":id,"projectName":context.name,"path":path,"name":real.file_name().and_then(|s|s.to_str()).ok_or_else(||invalid("Project filename is invalid"))?,"bytes":size,"modifiedAt":metadata_modified(&file)?,"binary":true,"text":null,"truncated":false});
        if image_extension(&path) && size <= IMAGE_BYTES as u64 {
            result["modifiedAt"] = json!(modified(&file)?);
            let snapshot = context.bytes(&path, IMAGE_BYTES)?;
            match thumbnail(&snapshot.bytes) {
                Ok(image) => {
                    result["imageMimeType"] = json!("image/png");
                    result["imageDataUrl"] =
                        json!(format!("data:image/png;base64,{}", STANDARD.encode(image)));
                }
                Err(_) => {
                    result["note"] = json!(
                        "Image preview unavailable. The file is invalid, unsupported or exceeds the pixel limit."
                    )
                }
            }
        } else if real
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("pdf"))
        {
            result["modifiedAt"] = json!(modified(&file)?);
            if size > PDF_BYTES as u64 {
                result["truncated"] = json!(true);
                result["note"] = json!("PDF is too large for the bounded in-app preview.");
            } else {
                let snapshot = context.bytes(&path, PDF_BYTES)?;
                if snapshot.bytes[..snapshot.bytes.len().min(1024)]
                    .windows(5)
                    .any(|w| w == b"%PDF-")
                {
                    result["pdfDataBase64"] = json!(STANDARD.encode(&snapshot.bytes));
                } else {
                    result["note"] = json!("This .pdf file does not contain a PDF header.");
                }
            }
        } else {
            let mut head = vec![];
            Read::by_ref(&mut file).take(8192).read_to_end(&mut head)?;
            file.seek(SeekFrom::Start(0))?;
            if sniff(&head) {
                if image_extension(&path) && size > IMAGE_BYTES as u64 {
                    result["truncated"] = json!(true);
                    result["note"] = json!("Image is too large for the bounded preview.");
                }
            } else if size <= TEXT_BYTES as u64 {
                match context.text(&path) {
                    Ok((snapshot, text)) => {
                        result["modifiedAt"] = json!(modified(&snapshot.file)?);
                        result["binary"] = json!(false);
                        result["text"] = json!(text);
                        result["revision"] = json!(snapshot.revision);
                    }
                    Err(e)
                        if e.to_string().contains("not UTF-8")
                            || e.to_string().contains("Binary files") =>
                    {
                        result["note"] = json!(e.to_string())
                    }
                    Err(e) => return Err(e),
                }
            } else {
                result["binary"] = json!(false);
                match files::preview(file.try_clone()?, &path, size, 1, usize::MAX, TEXT_BYTES) {
                    Ok(read) => {
                        result["text"] = read["text"].clone();
                        result["truncated"] =
                            json!(read["truncated"] == true || read["more"] == true);
                    }
                    Err(e) if e.to_string().starts_with("Line ") => {
                        result["truncated"] = json!(true);
                        result["note"] = json!(
                            "This file contains a line that is too large for the bounded preview."
                        );
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        context.check_file(&path, &file, &stamp)?;
        Ok(result)
    }
    pub fn save(
        &self,
        projects: &Projects,
        roots: &[Root],
        id: &str,
        input: &str,
        edit: &Edit<'_>,
    ) -> io::Result<Value> {
        refuse_server_edit(projects, id)?;
        if edit.text.len() > TEXT_BYTES {
            return Err(invalid(format!(
                "Edited files must be {TEXT_BYTES} bytes or smaller"
            )));
        }
        let context = Context::open(projects, roots, id)?;
        let (path, real) = context.target(input, false, false)?;
        let key = sandbox::native_identity(&real);
        {
            let mut saves = lock(&self.saves)?;
            if saves.len() >= 32 || !saves.insert(key.clone()) {
                return Err(invalid(
                    "A file save is already in progress. Try again when it finishes.",
                ));
            }
        }
        struct Saving<'a> {
            owner: &'a ProjectFiles,
            key: String,
        }
        impl Drop for Saving<'_> {
            fn drop(&mut self) {
                if let Ok(mut saves) = self.owner.saves.lock() {
                    saves.remove(&self.key);
                }
            }
        }
        let _saving = Saving { owner: self, key };
        let (original, _) = context.text(&path)?;
        if original.revision != edit.revision
            || original.bytes.len() as u64 != edit.bytes
            || modified(&original.file)? != edit.modified_at
        {
            return Err(invalid(STALE));
        }
        let directory = context.directory(parent(&path))?;
        let name = Path::new(&path).file_name().unwrap();
        let mut stage = Stage::new(directory, original.file.metadata()?.permissions())?;
        stage.file.write_all(edit.text.as_bytes())?;
        stage.file.sync_all()?;
        let (latest, _) = context.text(&path)?;
        if latest.revision != original.revision {
            return Err(invalid(STALE));
        }
        context.target(&path, false, false)?;
        context.check_parent(parent(&path), &stage.parent)?;
        drop(latest);
        drop(original);
        native::rename(
            &stage.file,
            &stage.parent,
            &stage.name,
            &stage.parent,
            name,
            true,
        )?;
        stage.published = true;
        Ok(json!({"preview":self.preview(projects,roots,id,&path)?}))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        projects: &Projects,
        roots: &[Root],
        id: &str,
        input: &str,
        name: &str,
        kind: &str,
    ) -> io::Result<Value> {
        refuse_server_edit(projects, id)?;
        let _guard = lock(&self.mutations)?;
        let context = Context::open(projects, roots, id)?;
        let name = leaf(name)?;
        let directory = relative(input)?;
        let path = child(&directory, name);
        context.target(&path, true, false)?;
        let parent = context.directory(&directory)?;
        match parent.symlink_metadata(name) {
            Ok(_) => return Err(invalid("A file or folder with that name already exists")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        #[cfg(test)]
        before_native();
        context.check_parent(&directory, &parent)?;
        match kind {
            "directory" => parent.create_dir(name)?,
            "file" => {
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                drop(parent.open_with(name, &options)?);
            }
            _ => return Err(invalid("Entry kind must be file or directory")),
        }
        Ok(json!({"projectId":id,"path":path,"kind":kind}))
    }
    pub fn rename(
        &self,
        projects: &Projects,
        roots: &[Root],
        id: &str,
        input: &str,
        name: &str,
    ) -> io::Result<Value> {
        refuse_server_edit(projects, id)?;
        let _guard = lock(&self.mutations)?;
        let context = Context::open(projects, roots, id)?;
        let (path, _) = context.target(input, false, false)?;
        let name = leaf(name)?;
        let destination = child(parent(&path), name);
        let metadata = context.dir.symlink_metadata(&path)?;
        let kind = kind(&metadata);
        if !matches!(kind, "file" | "directory") {
            return Err(invalid("Only regular files and folders can be renamed"));
        }
        if destination == path {
            return Ok(json!({"projectId":id,"path":path,"kind":kind}));
        }
        context.target(&destination, true, false)?;
        let dir = context.directory(parent(&path))?;
        let old = Path::new(&path).file_name().unwrap();
        let file = native::rename_handle(&dir, old)?;
        match dir.symlink_metadata(name) {
            Ok(_) => {
                let other = native::rename_handle(&dir, Path::new(name).as_os_str())?;
                if same_file::Handle::from_file(file.try_clone()?)?
                    != same_file::Handle::from_file(other)?
                {
                    return Err(invalid("A file or folder with that name already exists"));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        #[cfg(test)]
        before_native();
        let (_, current_real) = context.target(&path, false, false)?;
        context.check_parent(parent(&path), &dir)?;
        let current = native::rename_handle(&dir, old)?;
        if files::handle_path(&file)? != current_real
            || same_file::Handle::from_file(current)?
                != same_file::Handle::from_file(file.try_clone()?)?
        {
            return Err(invalid(
                "The project file changed location. Reload it before continuing.",
            ));
        }
        native::rename(&file, &dir, old, &dir, Path::new(name).as_os_str(), false)?;
        Ok(json!({"projectId":id,"path":destination,"kind":kind}))
    }
}
struct Stage {
    parent: Dir,
    name: OsString,
    file: File,
    published: bool,
}
impl Stage {
    fn new(parent: Dir, permissions: std::fs::Permissions) -> io::Result<Self> {
        let name = OsString::from(format!(".cos-save-{}.tmp", uuid::Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.access_mode(0xC0010000);
        }
        let file = parent.open_with(&name, &options)?.into_std();
        let stage = Self {
            parent,
            name,
            file,
            published: false,
        };
        stage.file.set_permissions(permissions)?;
        Ok(stage)
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        let owned = (|| -> io::Result<bool> {
            let current = self.parent.open(&self.name)?.into_std();
            Ok(same_file::Handle::from_file(current)?
                == same_file::Handle::from_file(self.file.try_clone()?)?)
        })()
        .unwrap_or(false);
        if owned {
            let _ = self.parent.remove_file(&self.name);
        }
    }
}

#[cfg(test)]
thread_local! {static BEFORE_NATIVE:std::cell::RefCell<Option<Box<dyn FnOnce()>>>=const{std::cell::RefCell::new(None)};}
#[cfg(test)]
fn before_native() {
    let hook = BEFORE_NATIVE.with(|h| h.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}
#[cfg(test)]
mod custody_tests {
    use super::*;
    #[test]
    fn large_native_file_identity_uses_source_javascript_number_serialization() {
        // Captured from the actual textSnapshot on a public Windows fixture.
        // The f64 inode is exactly 61361544922983112, whose JS spelling ends in 110.
        let stamp = json!([
            3836455792u64,
            61361544922983112u64,
            15,
            1792256233400.4734,
            1792256233400.4734
        ]);
        assert_eq!(
            snapshot_revision("\u{feff}public\r\nlast".as_bytes(), &stamp).unwrap(),
            "223c1dc39d09d8172031a42de665ccc92acb735ded136f6e03fe8d384628ed35"
        );
    }
    #[test]
    fn actual_upstream_path_name_and_binary_vectors_match() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/upstream/phase1-6-closure/project-files.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let result = match case["kind"].as_str().unwrap() {
                "relative" => relative(case["value"].as_str().unwrap()).map(Value::String),
                "leaf" => leaf(case["value"].as_str().unwrap()).map(|s| json!(s)),
                "sniff" => Ok(json!(sniff(
                    &serde_json::from_value::<Vec<u8>>(case["bytes"].clone()).unwrap()
                ))),
                _ => panic!("Unknown fixture case"),
            };
            match result {
                Ok(value) => assert_eq!(value, case["expected"], "{case}"),
                Err(error) => assert_eq!(
                    Some(error.to_string()).as_deref(),
                    case["error"].as_str(),
                    "{case}"
                ),
            }
        }
    }
    #[test]
    fn parent_replacement_cannot_redirect_creation_outside_selected_project() {
        let root = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        std::fs::create_dir_all(project.join("nested")).unwrap();
        let roots = [Root {
            name: "r".into(),
            path: std::fs::canonicalize(root.path())
                .unwrap()
                .to_string_lossy()
                .into(),
            remote: None,
        }];
        let projects =
            Projects::open(cos_core::persistence::DurableStore::open(profile.path()).unwrap());
        let p = projects.add(&roots, "/r/project").unwrap();
        let id = p["id"].as_str().unwrap();
        let original = project.join("nested");
        let moved = root.path().join("moved-outside-project");
        let destination = moved.clone();
        let relocated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = relocated.clone();
        BEFORE_NATIVE.with(|h| {
            *h.borrow_mut() = Some(Box::new(move || {
                match std::fs::rename(&original, &destination) {
                    Ok(()) => {
                        flag.store(true, std::sync::atomic::Ordering::Release);
                        std::fs::create_dir(&original).unwrap();
                    }
                    Err(e) if cfg!(windows) && e.raw_os_error() == Some(32) => {}
                    Err(e) => panic!("Unable to exercise native directory custody: {e}"),
                }
            }))
        });
        let result =
            ProjectFiles::default().create(&projects, &roots, id, "nested", "new.txt", "file");
        if relocated.load(std::sync::atomic::Ordering::Acquire) {
            assert!(
                result.is_err(),
                "A held parent moved outside the selected project was still writable"
            );
            assert!(!moved.join("new.txt").exists());
            assert!(!project.join("nested/new.txt").exists());
        } else {
            assert!(
                result.is_ok(),
                "The OS refused relocation of the held directory; its original target remains usable"
            );
            assert!(project.join("nested/new.txt").is_file());
            assert!(!moved.exists());
        }
    }
    #[test]
    fn source_replacement_cannot_rename_a_different_file_or_relocated_handle() {
        let root = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(project.join("old.txt"), "original public data").unwrap();
        let roots = [Root {
            name: "r".into(),
            path: std::fs::canonicalize(root.path())
                .unwrap()
                .to_string_lossy()
                .into(),
            remote: None,
        }];
        let projects =
            Projects::open(cos_core::persistence::DurableStore::open(profile.path()).unwrap());
        let p = projects.add(&roots, "/r/project").unwrap();
        let id = p["id"].as_str().unwrap();
        let original = project.join("old.txt");
        let moved = root.path().join("moved.txt");
        let destination = moved.clone();
        BEFORE_NATIVE.with(|h| {
            *h.borrow_mut() = Some(Box::new(move || {
                std::fs::rename(&original, &destination).unwrap();
                std::fs::write(&original, "replacement public data").unwrap();
            }))
        });
        let result = ProjectFiles::default().rename(&projects, &roots, id, "old.txt", "new.txt");
        assert!(result.is_err(), "A replaced source was renamed");
        assert_eq!(
            std::fs::read_to_string(&moved).unwrap(),
            "original public data"
        );
        assert_eq!(
            std::fs::read_to_string(project.join("old.txt")).unwrap(),
            "replacement public data"
        );
        assert!(!project.join("new.txt").exists());
    }
}
