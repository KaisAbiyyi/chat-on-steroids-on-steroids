//! Explicit, non-destructive legacy copy; never opens the source as a writable owner.
use cos_core::persistence::{atomic_write, invalid, is_link, owned_path};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
const MARKER: &str = "cos-rust-profile.json";
fn marker() -> Value {
    json!({"format":1,"upstreamCommit":cos_core::UPSTREAM_COMMIT,"runtimeVersion":env!("CARGO_PKG_VERSION")})
}
pub fn ensure_marked(directory: &Path) -> io::Result<()> {
    let path = owned_path(directory, Path::new(MARKER), true)?;
    if path.exists() {
        let value: Value = serde_json::from_slice(&fs::read(path)?)
            .map_err(|_| invalid("Unreadable Rust profile marker"))?;
        if value["format"] != 1 || value["upstreamCommit"] != cos_core::UPSTREAM_COMMIT {
            return Err(invalid("Unsupported Rust profile identity"));
        }
        return Ok(());
    }
    let entries = fs::read_dir(directory)?;
    for entry in entries {
        let entry = entry?;
        if entry.file_name() != ".cos-rust.lock" {
            return Err(invalid(
                "Existing unmarked profile: use import --source to copy it into a new directory",
            ));
        }
    }
    atomic_write(&path, &serde_json::to_vec_pretty(&marker())?)
}
pub(super) fn digest(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub(super) fn inventory(root: &Path) -> io::Result<BTreeMap<PathBuf, (u64, String)>> {
    let mut files = BTreeMap::new();
    fn walk(
        root: &Path,
        relative: &Path,
        depth: usize,
        files: &mut BTreeMap<PathBuf, (u64, String)>,
    ) -> io::Result<()> {
        if depth > 8 {
            return Err(invalid(
                "Legacy data contains an unexpected directory depth",
            ));
        }
        let path = owned_path(root, relative, true)?;
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        if is_link(&meta) {
            return Err(invalid("Legacy data contains a link; copy refused"));
        }
        if meta.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                walk(root, &relative.join(entry.file_name()), depth + 1, files)?;
            }
        } else if meta.is_file() {
            files.insert(relative.into(), (meta.len(), digest(&path)?));
            if files.len() > 200000 {
                return Err(invalid("Legacy file count exceeds the import bound"));
            }
        } else {
            return Err(invalid("Legacy data contains a nonregular file"));
        }
        Ok(())
    }
    for name in [
        "config.json",
        "secrets.bin",
        "Local State",
        "state",
        "sessions",
    ] {
        walk(root, Path::new(name), 0, &mut files)?;
    }
    Ok(files)
}
pub fn import(source: &Path, target: &Path) -> io::Result<Value> {
    if !source.is_absolute() || !target.is_absolute() {
        return Err(invalid("Import paths must be absolute"));
    }
    let source = fs::canonicalize(source)?;
    if target.exists() {
        return Err(invalid("Import target must not already exist"));
    }
    let parent = target
        .parent()
        .ok_or_else(|| invalid("Import target has no parent"))?;
    fs::create_dir_all(parent)?;
    let parent = fs::canonicalize(parent)?;
    let leaf = target
        .file_name()
        .ok_or_else(|| invalid("Invalid import target"))?;
    let target = parent.join(leaf);
    if target.starts_with(&source) || source.starts_with(&target) {
        return Err(invalid(
            "Import source and target must be separate directories",
        ));
    }
    let original = inventory(&source)?;
    let staging = tempfile::Builder::new()
        .prefix(".cos-rust-import-")
        .tempdir_in(&parent)?;
    let mut bytes = 0_u64;
    for (relative, (size, expected)) in &original {
        let input = owned_path(&source, relative, false)?;
        let output = staging.path().join(relative);
        fs::create_dir_all(output.parent().unwrap())?;
        let mut reader = fs::File::open(input)?;
        let mut writer = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&output)?;
        let count = io::copy(&mut reader, &mut writer)?;
        writer.flush()?;
        writer.sync_all()?;
        if count != *size || digest(&output)? != *expected {
            return Err(io::Error::other(
                "Legacy data changed during copy; import was not published",
            ));
        }
        bytes += count;
    }
    if inventory(&source)? != original {
        return Err(io::Error::other(
            "Legacy data changed during verification; import was not published",
        ));
    }
    let store = cos_store::SessionStore::open(staging.path())?;
    let summaries = store.list()?;
    cos_core::persistence::ConfigStore::open(
        staging.path(),
        if cfg!(windows) {
            "win32"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        },
    )?;
    atomic_write(
        &staging.path().join(MARKER),
        &serde_json::to_vec_pretty(&marker())?,
    )?;
    let receipt = json!({"source":source,"target":target,"files":original.len(),"bytes":bytes,"sessions":summaries.len(),"verified":true,"sourceUnchanged":true,"scope":"config, secrets + Local State, durable state and complete sessions","upstreamCommit":cos_core::UPSTREAM_COMMIT});
    atomic_write(
        &staging.path().join("cos-rust-import-receipt.json"),
        &serde_json::to_vec_pretty(&receipt)?,
    )?;
    fs::rename(staging.path(), &target)?;
    Ok(receipt)
}
