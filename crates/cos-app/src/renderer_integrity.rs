//! Attestation for the entire runnable native renderer payload.
//! This check is intentionally separate from EXE identity: a .NET apphost
//! executes independently mutable managed IL and dependency resources.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
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

fn digest_file(path: &Path) -> io::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_FILE_BYTES {
        return Err(denied());
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hasher.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hasher.finalize()))
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

fn inventory(directory: &Path, prefix: &str, found: &mut BTreeSet<String>) -> io::Result<()> {
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
        if ty.is_dir() {
            inventory(&item.path(), &relative, found)?;
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
pub fn verify(executable: &Path, exe_sha: &str, manifest_sha: &str) -> io::Result<String> {
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
    let bytes = fs::read(&manifest_path)?;
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
        if !metadata.file_type().is_file() {
            return Err(denied());
        }
        total = total.checked_add(metadata.len()).ok_or_else(denied)?;
        if total > MAX_TOTAL_BYTES {
            return Err(denied());
        }
        if !digest_file(&path)?.eq_ignore_ascii_case(&item.sha256) {
            return Err(denied());
        }
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
    inventory(root, "", &mut actual)?;
    if expected != actual {
        return Err(denied());
    }
    Ok(actual_manifest_sha)
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
