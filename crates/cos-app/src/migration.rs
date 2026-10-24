//! Phase 16A: bounded, explicit, non-destructive backup and restore.
//! No private profile is selected by default. Restores create a new profile;
//! they never replace or delete the source, backup or an existing target.
use crate::profile;
use cos_core::persistence::{atomic_write, invalid, owned_path};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const MANIFEST: &str = "cos-rust-backup-manifest.json";
const SCHEMA_VERSION: u64 = 1;
const MAX_BYTES: u64 = 64 * 1024 * 1024 * 1024;

type Inventory = BTreeMap<PathBuf, (u64, String)>;

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn manifest(inventory: &Inventory) -> io::Result<Value> {
    let files = inventory
        .iter()
        .map(|(path, (bytes, hash))| {
            Ok(json!({
                "path": path.to_str().ok_or_else(|| invalid("Backup path cannot be represented as UTF-8"))?,
                "bytes": bytes,
                "sha256": hash
            }))
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(json!({
        "schemaVersion": SCHEMA_VERSION,
        "kind": "cos-rust-profile-backup",
        "sourceFormat": 1,
        "upstreamCommit": cos_core::UPSTREAM_COMMIT,
        "files": files
    }))
}

// This is a fresh-destination transaction; there is never an overwrite or
// cleanup of an existing target. The staging directory is owned by TempDir.
fn destinations(source: &Path, target: &Path) -> io::Result<(PathBuf, PathBuf)> {
    if !source.is_absolute() || !target.is_absolute() {
        return Err(invalid(
            "Source and destination must be explicit absolute paths",
        ));
    }
    let source = fs::canonicalize(source)?;
    if fs::symlink_metadata(target).is_ok() {
        return Err(invalid(
            "Destination already exists; overwrite is forbidden",
        ));
    }
    let parent = target
        .parent()
        .ok_or_else(|| invalid("Destination has no parent"))?;
    fs::create_dir_all(parent)?;
    let parent = fs::canonicalize(parent)?;
    let leaf = target
        .file_name()
        .ok_or_else(|| invalid("Invalid destination"))?;
    let target = parent.join(leaf);
    if source.starts_with(&target) || target.starts_with(&source) {
        return Err(invalid("Source and destination must be disjoint"));
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(invalid(
            "Destination already exists; overwrite is forbidden",
        ));
    }
    Ok((source, target))
}

fn copy_verified(source: &Path, stage: &Path, original: &Inventory) -> io::Result<u64> {
    let mut total = 0u64;
    for (relative, (size, expected)) in original {
        total = total
            .checked_add(*size)
            .filter(|total| *total <= MAX_BYTES)
            .ok_or_else(|| invalid("Backup is larger than the 64 GiB safety bound"))?;
        let input = owned_path(source, relative, false)?;
        let output = stage.join(relative);
        fs::create_dir_all(
            output
                .parent()
                .ok_or_else(|| invalid("Invalid staged path"))?,
        )?;
        let mut reader = fs::File::open(&input)?;
        let mut writer = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?;
        let count = io::copy(&mut reader, &mut writer)?;
        writer.flush()?;
        writer.sync_all()?;
        if count != *size || profile::digest(&output)? != *expected {
            return Err(io::Error::other(
                "Source changed during backup; no snapshot published",
            ));
        }
    }
    if profile::inventory(source)? != *original {
        return Err(io::Error::other(
            "Source changed during backup verification; no snapshot published",
        ));
    }
    Ok(total)
}

pub fn backup(source: &Path, target: &Path) -> io::Result<Value> {
    let (source, target) = destinations(source, target)?;
    let original = profile::inventory(&source)?;
    let stage = tempfile::Builder::new()
        .prefix(".cos-rust-backup-")
        .tempdir_in(
            target
                .parent()
                .ok_or_else(|| invalid("Missing backup parent"))?,
        )?;
    let total = copy_verified(&source, stage.path(), &original)?;
    let manifest_bytes = serde_json::to_vec_pretty(&manifest(&original)?)?;
    if manifest_bytes.len() > 32 * 1024 * 1024 {
        return Err(invalid(
            "Backup manifest exceeds 32 MiB; snapshot not published",
        ));
    }
    atomic_write(&stage.path().join(MANIFEST), &manifest_bytes)?;
    // A final snapshot verification prevents publishing copies damaged after write.
    if profile::inventory(stage.path())? != original {
        return Err(io::Error::other("Staged backup verification failed"));
    }
    let hash = sha256(&manifest_bytes);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(invalid(
            "Destination appeared during backup; overwrite refused",
        ));
    }
    profile::publish_new(stage.path(), &target)?;
    Ok(json!({
        "backup": target, "schemaVersion": SCHEMA_VERSION, "files": original.len(),
        "bytes": total, "manifestSha256": hash, "verified": true,
        "sourceUnchanged": true, "restorableWith": "restore --backup <absolute backup> --manifest-sha256 <receipt hash>"
    }))
}

pub fn restore(backup: &Path, target: &Path, expected_manifest_sha256: &str) -> io::Result<Value> {
    if expected_manifest_sha256.len() != 64
        || !expected_manifest_sha256
            .bytes()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(invalid(
            "Restore requires the exact 64-character backup manifest SHA-256",
        ));
    }
    let (source, target) = destinations(backup, target)?;
    let manifest_path = owned_path(&source, Path::new(MANIFEST), false)?;
    let mut file = fs::File::open(manifest_path)?;
    if file.metadata()?.len() > 32 * 1024 * 1024 {
        return Err(invalid("Backup manifest exceeds 32 MiB"));
    }
    let mut raw = Vec::new();
    (&mut file)
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > 32 * 1024 * 1024 {
        return Err(invalid("Backup manifest exceeds 32 MiB"));
    }
    if sha256(&raw) != expected_manifest_sha256.to_ascii_lowercase() {
        return Err(invalid("Backup manifest hash mismatch; restore refused"));
    }
    let parsed: Value =
        serde_json::from_slice(&raw).map_err(|_| invalid("Unreadable backup manifest"))?;
    if parsed["schemaVersion"] != SCHEMA_VERSION
        || parsed["kind"] != "cos-rust-profile-backup"
        || parsed["sourceFormat"] != 1
        || parsed["upstreamCommit"] != cos_core::UPSTREAM_COMMIT
    {
        return Err(invalid(
            "Unsupported backup schema, source format or upstream identity",
        ));
    }
    let actual = profile::inventory(&source)?;
    if manifest(&actual)? != parsed {
        return Err(invalid(
            "Backup file inventory differs from verified manifest",
        ));
    }
    // import itself re-inventories the source, hashes each copied file,
    // validates the synthetic copied session/config and atomically publishes.
    let mut receipt = profile::import_expected(&source, &target, Some(&actual))?;
    receipt["restoredFromBackup"] = json!(true);
    receipt["backupSchemaVersion"] = json!(SCHEMA_VERSION);
    receipt["manifestSha256"] = json!(expected_manifest_sha256.to_ascii_lowercase());
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> io::Result<(tempfile::TempDir, PathBuf)> {
        let home = tempfile::tempdir()?;
        let source = home.path().join("source");
        fs::create_dir_all(source.join("state"))?;
        fs::write(source.join("config.json"), b"{}")?;
        fs::write(
            source.join("state").join("example.json"),
            b"{\"hello\":\"world\"}",
        )?;
        Ok((home, source))
    }

    #[test]
    fn backup_and_restore_are_copy_only_and_versioned() -> io::Result<()> {
        let (home, source) = fixture()?;
        let backup_dir = home.path().join("backup");
        let receipt = backup(&source, &backup_dir)?;
        assert_eq!(receipt["verified"], true);
        assert!(!source.join(MANIFEST).exists());
        let hash = receipt["manifestSha256"].as_str().unwrap();
        let restored = home.path().join("restored");
        let result = restore(&backup_dir, &restored, hash)?;
        assert_eq!(result["restoredFromBackup"], true);
        assert_eq!(
            fs::read(source.join("state/example.json"))?,
            b"{\"hello\":\"world\"}"
        );
        assert_eq!(
            fs::read(restored.join("state/example.json"))?,
            b"{\"hello\":\"world\"}"
        );
        assert!(restored.join("cos-rust-profile.json").exists());
        assert!(backup_dir.join(MANIFEST).exists());
        Ok(())
    }

    #[test]
    fn changed_payload_refuses_restore_without_creating_target() -> io::Result<()> {
        let (home, source) = fixture()?;
        let backup_dir = home.path().join("backup");
        let receipt = backup(&source, &backup_dir)?;
        fs::write(backup_dir.join("config.json"), b"{\"altered\":true}")?;
        let restored = home.path().join("restored");
        assert!(
            restore(
                &backup_dir,
                &restored,
                receipt["manifestSha256"].as_str().unwrap()
            )
            .is_err()
        );
        assert!(!restored.exists());
        Ok(())
    }

    #[test]
    fn rejects_wrong_digest_and_future_schema_without_side_effect() -> io::Result<()> {
        let (home, source) = fixture()?;
        let backup_dir = home.path().join("backup");
        let receipt = backup(&source, &backup_dir)?;
        let dest = home.path().join("restore");
        assert!(restore(&backup_dir, &dest, &"0".repeat(64)).is_err());
        let path = backup_dir.join(MANIFEST);
        let mut changed: Value = serde_json::from_slice(&fs::read(&path)?)?;
        changed["schemaVersion"] = json!(SCHEMA_VERSION + 1);
        let bytes = serde_json::to_vec_pretty(&changed)?;
        fs::write(&path, &bytes)?;
        assert!(restore(&backup_dir, &dest, &sha256(&bytes)).is_err());
        assert!(!dest.exists());
        assert!(source.join("config.json").exists());
        assert_eq!(receipt["verified"], true);
        Ok(())
    }

    #[test]
    fn destination_and_nesting_are_refused() -> io::Result<()> {
        let (home, source) = fixture()?;
        let same = source.clone();
        assert!(backup(&source, &same).is_err());
        assert!(backup(&source, &source.join("nested")).is_err());
        let existing = home.path().join("existing");
        fs::create_dir_all(&existing)?;
        assert!(backup(&source, &existing).is_err());
        Ok(())
    }

    #[test]
    fn verified_import_refuses_replaced_snapshot_even_if_self_consistent() -> io::Result<()> {
        let (home, source) = fixture()?;
        let before = profile::inventory(&source)?;
        fs::write(source.join("config.json"), b"{\"changed\":true}")?;
        let target = home.path().join("restored");
        assert!(profile::import_expected(&source, &target, Some(&before)).is_err());
        assert!(!target.exists());
        Ok(())
    }

    #[test]
    fn atomic_publication_does_not_replace_an_existing_empty_directory() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        let staged = home.path().join("staged");
        let occupied = home.path().join("occupied");
        fs::create_dir(&staged)?;
        fs::create_dir(&occupied)?;
        assert!(profile::publish_new(&staged, &occupied).is_err());
        assert!(staged.is_dir());
        assert!(occupied.is_dir());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_data_refuses_backup() -> io::Result<()> {
        use std::os::unix::fs::symlink;
        let (home, source) = fixture()?;
        symlink(home.path(), source.join("sessions"))?;
        assert!(backup(&source, &home.path().join("backup")).is_err());
        Ok(())
    }
}
