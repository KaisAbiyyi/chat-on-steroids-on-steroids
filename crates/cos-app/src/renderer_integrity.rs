//! Attestation for the entire runnable native renderer payload.
//! This check is intentionally separate from EXE identity: a .NET apphost
//! executes independently mutable managed IL and dependency resources.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

const MAX_FILES: usize = 128;
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const REQUIRED: &[&str] = &[
    "WindowsRenderer.exe",
    "WindowsRenderer.dll",
    "WindowsRenderer.deps.json",
    "WindowsRenderer.runtimeconfig.json",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    files: Vec<Entry>,
    #[serde(default, rename = "sourceSha")]
    source_sha: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    sha256: String,
}

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Renderer payload integrity verification rejected",
    )
}

fn valid_sha(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Windows CreateFile holds a read-only, non-delete-sharing handle until the
/// native child exits. Runtime DLLs cannot be overwritten, replaced or renamed
/// between the hash check and the Windows loader opening them.
#[cfg(windows)]
fn open_guarded(path: &Path, directory: bool) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let flags = OPEN_REPARSE_POINT | if directory { BACKUP_SEMANTICS } else { 0 };
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(flags)
        .open(path)?;
    use std::os::windows::fs::MetadataExt;
    if file.metadata()?.file_attributes() & 0x0000_0400 != 0 {
        return Err(denied());
    }
    Ok(file)
}
#[cfg(not(windows))]
fn open_guarded(path: &Path, _directory: bool) -> io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

/// The handles live in the launched Rust host for the entire child lifetime.
pub struct VerifiedPayload {
    pub manifest_sha: String,
    pub source_sha: Option<String>,
    _locked_handles: Vec<File>,
}

fn digest_guarded(file: &File) -> io::Result<String> {
    let mut reader = file;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let size = reader.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hasher.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
fn digest_file(path: &Path) -> io::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_FILE_BYTES {
        return Err(denied());
    }
    let file = open_guarded(path, false)?;
    digest_guarded(&file)
}

fn safe_parts(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && !path
            .bytes()
            .any(|value| matches!(value, b'\\' | b':' | b'\0'))
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn inventory(
    directory: &Path,
    prefix: &str,
    found: &mut BTreeSet<String>,
    locked_dirs: &mut Vec<File>,
) -> io::Result<()> {
    locked_dirs.push(open_guarded(directory, true)?);
    for item in fs::read_dir(directory)? {
        let item = item?;
        let name = item.file_name().into_string().map_err(|_| denied())?;
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if !safe_parts(&relative) {
            return Err(denied());
        }
        let ty = item.file_type()?;
        if ty.is_symlink() {
            return Err(denied());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if fs::symlink_metadata(item.path())?.file_attributes() & 0x0000_0400 != 0 {
                return Err(denied());
            }
        }
        if ty.is_dir() {
            inventory(&item.path(), &relative, found, locked_dirs)?;
        } else if ty.is_file() {
            if relative != "renderer-integrity.json" {
                found.insert(relative);
                if found.len() > MAX_FILES {
                    return Err(denied());
                }
            }
        } else {
            return Err(denied());
        }
    }
    Ok(())
}

/// The manifest digest is an independent caller-supplied acceptance input,
/// NOT a digest read from the untrusted renderer directory.
pub fn verify(executable: &Path, exe_sha: &str, manifest_sha: &str) -> io::Result<VerifiedPayload> {
    if !valid_sha(exe_sha)
        || !valid_sha(manifest_sha)
        || executable
            .file_name()
            .is_none_or(|x| x != "WindowsRenderer.exe")
    {
        return Err(denied());
    }
    let root = executable.parent().ok_or_else(denied)?;
    let manifest_path: PathBuf = root.join("renderer-integrity.json");
    let meta = fs::symlink_metadata(&manifest_path)?;
    if !meta.file_type().is_file() || meta.len() == 0 || meta.len() > MAX_MANIFEST_BYTES {
        return Err(denied());
    }
    let mut locked = Vec::new();
    // Hold the manifest handle through launch just like executable/DLL handles.
    let mut manifest_file = open_guarded(&manifest_path, false)?;
    let mut bytes = Vec::new();
    manifest_file.read_to_end(&mut bytes)?;
    locked.push(manifest_file);
    let actual_manifest_sha = format!("{:x}", Sha256::digest(&bytes));
    if !actual_manifest_sha.eq_ignore_ascii_case(manifest_sha) {
        return Err(denied());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| denied())?;
    if manifest.format != 1
        || manifest.files.len() < REQUIRED.len()
        || manifest.files.len() > MAX_FILES
    {
        return Err(denied());
    }
    if manifest.source_sha.as_deref().is_some_and(|sha| {
        sha.len() != 40 || !sha.bytes().all(|value| value.is_ascii_hexdigit())
    }) {
        return Err(denied());
    }
    let source_sha = manifest.source_sha.clone();
    let mut expected = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut total = 0u64;
    let mut verified_exe = false;
    for item in &manifest.files {
        if !safe_parts(&item.path)
            || !valid_sha(&item.sha256)
            || !expected.insert(item.path.clone())
            || !folded.insert(item.path.to_ascii_lowercase())
            || item.path == "renderer-integrity.json"
        {
            return Err(denied());
        }
        let path = root.join(&item.path);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_FILE_BYTES
        {
            return Err(denied());
        }
        total = total.checked_add(metadata.len()).ok_or_else(denied)?;
        if total > MAX_TOTAL_BYTES {
            return Err(denied());
        }
        let file = open_guarded(&path, false)?;
        if !digest_guarded(&file)?.eq_ignore_ascii_case(&item.sha256) {
            return Err(denied());
        }
        locked.push(file);
        if item.path == "WindowsRenderer.exe" {
            if !item.sha256.eq_ignore_ascii_case(exe_sha) {
                return Err(denied());
            }
            verified_exe = true;
        }
    }
    if !verified_exe || REQUIRED.iter().any(|item| !expected.contains(*item)) {
        return Err(denied());
    }
    let mut actual = BTreeSet::new();
    inventory(root, "", &mut actual, &mut locked)?;
    if expected != actual {
        return Err(denied());
    }
    Ok(VerifiedPayload {
        manifest_sha: actual_manifest_sha,
        source_sha,
        _locked_handles: locked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(root: &Path) -> (String, String) {
        for (name, data) in [
            ("WindowsRenderer.exe", b"apphost".as_slice()),
            ("WindowsRenderer.dll", b"managed-IL".as_slice()),
            ("WindowsRenderer.deps.json", b"deps".as_slice()),
            ("WindowsRenderer.runtimeconfig.json", b"runtime".as_slice()),
        ] {
            fs::write(root.join(name), data).unwrap();
        }
        let names = REQUIRED
            .iter()
            .map(|path| json!({"path": path, "sha256": digest_file(&root.join(path)).unwrap()}))
            .collect::<Vec<_>>();
        let bytes = serde_json::to_vec(&json!({"format":1,"files":names})).unwrap();
        fs::write(root.join("renderer-integrity.json"), &bytes).unwrap();
        (
            digest_file(&root.join("WindowsRenderer.exe")).unwrap(),
            format!("{:x}", Sha256::digest(bytes)),
        )
    }

    #[test]
    fn unchanged_exe_with_tampered_managed_dll_is_denied() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, manifest) = fixture(temp.path());
        assert!(verify(&temp.path().join("WindowsRenderer.exe"), &exe, &manifest).is_ok());
        fs::write(
            temp.path().join("WindowsRenderer.dll"),
            b"malicious managed IL",
        )
        .unwrap();
        assert!(verify(&temp.path().join("WindowsRenderer.exe"), &exe, &manifest).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn post_verification_managed_dll_swap_and_rename_are_denied_while_guard_is_alive() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, manifest) = fixture(temp.path());
        let executable = temp.path().join("WindowsRenderer.exe");
        let dll = temp.path().join("WindowsRenderer.dll");
        let guarded = verify(&executable, &exe, &manifest).unwrap();
        // Attacker has retained the unchanged, previously approved EXE digest.
        assert_eq!(guarded.manifest_sha, manifest);
        assert!(fs::write(&dll, b"modified AFTER verification").is_err());
        assert!(fs::rename(&dll, temp.path().join("swapped.dll")).is_err());
        assert!(fs::remove_file(&dll).is_err());
        assert!(fs::write(&executable, b"changed executable").is_err());
        assert!(fs::rename(&executable, temp.path().join("swapped.exe")).is_err());
        drop(guarded);
        // Regression is meaningful only if the same attacker can mutate it once
        // the launcher releases the guard; otherwise ACLs masked the gap.
        fs::write(&dll, b"modified AFTER guard release").unwrap();
        assert!(verify(&executable, &exe, &manifest).is_err());
    }

    #[test]
    fn added_dependency_and_modified_manifest_are_denied() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, manifest) = fixture(temp.path());
        fs::write(temp.path().join("Additional.dll"), b"injected").unwrap();
        assert!(verify(&temp.path().join("WindowsRenderer.exe"), &exe, &manifest).is_err());
        fs::remove_file(temp.path().join("Additional.dll")).unwrap();
        fs::write(temp.path().join("renderer-integrity.json"), b"{}").unwrap();
        assert!(verify(&temp.path().join("WindowsRenderer.exe"), &exe, &manifest).is_err());
    }
}
