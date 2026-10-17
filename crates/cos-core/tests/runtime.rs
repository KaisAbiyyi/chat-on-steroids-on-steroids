use cos_core::{
    logging::{DiagnosticLog, redact_credentials},
    runtime::{Cancellation, ShutdownPhase, shutdown},
    secrets::{Decrypted, SecretCipher, SecretStore, WindowsCipher},
};
use std::{
    fs, io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

struct TestCipher {
    available: AtomicBool,
}
impl SecretCipher for TestCipher {
    fn available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }
    fn encrypt(&self, text: &str) -> io::Result<Vec<u8>> {
        if !self.available() {
            return Err(io::Error::other("unavailable"));
        }
        Ok(text.bytes().map(|b| b ^ 0xaa).collect())
    }
    fn decrypt(&self, bytes: &[u8]) -> io::Result<Decrypted> {
        Ok(Decrypted {
            text: String::from_utf8(bytes.iter().map(|b| b ^ 0xaa).collect())
                .map_err(io::Error::other)?,
            rotate: false,
        })
    }
}
#[test]
fn unavailable_secret_provider_never_erases_existing_ciphertext() {
    let dir = tempfile::tempdir().unwrap();
    let cipher = Arc::new(TestCipher {
        available: AtomicBool::new(true),
    });
    let store = SecretStore::open(dir.path(), "win32", cipher.clone()).unwrap();
    store.set("bridgeToken", "existing").unwrap();
    drop(store);
    let before = fs::read(dir.path().join("secrets.bin")).unwrap();
    cipher.available.store(false, Ordering::SeqCst);
    let store = SecretStore::open(dir.path(), "win32", cipher.clone()).unwrap();
    assert_eq!(store.get("bridgeToken").unwrap(), None);
    assert!(store.set("openaiApiKey", "new").is_err());
    assert_eq!(fs::read(dir.path().join("secrets.bin")).unwrap(), before);
    cipher.available.store(true, Ordering::SeqCst);
    assert_eq!(
        store.get("bridgeToken").unwrap().as_deref(),
        Some("existing")
    );
    store
        .set("openaiApiKey", " \u{ad}new\u{200b}\n key ")
        .unwrap();
    assert_eq!(
        store.get("openaiApiKey").unwrap().as_deref(),
        Some("newkey")
    );
    assert_eq!(
        store.get("bridgeToken").unwrap().as_deref(),
        Some("existing")
    );
}
#[test]
fn malformed_secret_shape_preserved_until_explicit_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let cipher = Arc::new(TestCipher {
        available: AtomicBool::new(true),
    });
    let path = dir.path().join("secrets.bin");
    let raw = cipher.encrypt("{\"future\":34}").unwrap();
    fs::write(&path, &raw).unwrap();
    let store = SecretStore::open(dir.path(), "win32", cipher).unwrap();
    assert_eq!(store.get("future").unwrap(), None);
    assert!(store.set("bridgeToken", "new").is_err());
    assert_eq!(fs::read(&path).unwrap(), raw);
    let backup = store.set_aside_unreadable().unwrap().unwrap();
    assert_eq!(fs::read(backup).unwrap(), raw);
    store.set("bridgeToken", "new").unwrap();
}
#[test]
fn failed_secret_write_does_not_publish_proposed_key() {
    let dir = tempfile::tempdir().unwrap();
    let cipher = Arc::new(TestCipher {
        available: AtomicBool::new(true),
    });
    let store = SecretStore::open(dir.path(), "win32", cipher).unwrap();
    store.set("bridgeToken", "old").unwrap();
    fs::remove_file(dir.path().join("secrets.bin")).unwrap();
    fs::create_dir(dir.path().join("secrets.bin")).unwrap();
    assert!(store.set("bridgeToken", "new").is_err());
    assert_eq!(store.get("bridgeToken").unwrap().as_deref(), Some("old"));
}
#[test]
fn linux_basic_text_blob_refused_even_when_provider_reports_secure() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("secrets.bin"), b"v10insecure").unwrap();
    let store = SecretStore::open(
        dir.path(),
        "linux",
        Arc::new(TestCipher {
            available: AtomicBool::new(true),
        }),
    )
    .unwrap();
    assert_eq!(store.get("bridgeToken").unwrap(), None);
    assert!(store.set("bridgeToken", "new").is_err());
}
#[test]
fn diagnostics_and_exact_recording_have_distinct_redaction_policies() {
    let key = format!("sk-{}", "A".repeat(32));
    let hash = "b".repeat(64);
    assert_eq!(
        redact_credentials(&format!("{key} {hash}")),
        format!("[redacted] {hash}")
    );
    let log = DiagnosticLog::new(2);
    log.push("info", &format!("{key} {hash}"));
    log.push("warn", "b");
    log.push("error", "c");
    assert_eq!(log.snapshot().len(), 2);
    assert!(!log.export().contains(&key));
}
#[test]
fn shutdown_deadline_does_not_skip_later_persistence_phase() {
    let cancellation = Cancellation::default();
    let observed = Arc::new(AtomicBool::new(false));
    let mark = observed.clone();
    let start = Instant::now();
    let report = shutdown(
        &cancellation,
        vec![
            ShutdownPhase::new("hung", Duration::from_millis(10), || {
                std::thread::sleep(Duration::from_millis(100));
                Ok(())
            }),
            ShutdownPhase::new("persist", Duration::from_secs(1), move || {
                mark.store(true, Ordering::SeqCst);
                Ok(())
            }),
        ],
    );
    assert!(cancellation.is_cancelled());
    assert!(observed.load(Ordering::SeqCst));
    assert!(start.elapsed() < Duration::from_millis(90));
    assert_eq!(report[0].status, "timeout");
    assert_eq!(report[1].status, "ok");
}
#[cfg(windows)]
#[test]
fn windows_dpapi_profile_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let cipher = WindowsCipher::open(dir.path(), true).unwrap();
    let encrypted = cipher.encrypt("public-test-value").unwrap();
    assert!(encrypted.starts_with(b"v10"));
    assert_eq!(
        cipher.decrypt(&encrypted).unwrap().text,
        "public-test-value"
    );
    let mut corrupt = encrypted;
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(cipher.decrypt(&corrupt).is_err());
}
