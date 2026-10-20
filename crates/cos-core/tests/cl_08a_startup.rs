use cos_core::{
    persistence::ConfigStore,
    secrets::{Decrypted, SecretCipher, SecretStore},
};
#[cfg(windows)]
use cos_core::secrets::WindowsCipher;
use std::{fs, io, sync::Arc};

struct LockedCipher;
impl SecretCipher for LockedCipher {
    fn available(&self) -> bool {
        true
    }
    fn encrypt(&self, _: &str) -> io::Result<Vec<u8>> {
        Err(io::Error::other("blocked"))
    }
    fn decrypt(&self, _: &[u8]) -> io::Result<Decrypted> {
        Err(io::Error::other("locked"))
    }
}

#[test]
fn broken_vault_does_not_appear_empty_on_startup() {
    let profile = tempfile::tempdir().unwrap();
    fs::write(profile.path().join("secrets.bin"), b"sealed-fixture").unwrap();
    let store = SecretStore::open(profile.path(), "win32", Arc::new(LockedCipher)).unwrap();
    let error = store.get("openaiApiKey").unwrap_err().to_string();
    assert!(error.contains("left untouched"), "{error}");
    assert!(store.is_unreadable().unwrap());
    assert_eq!(
        fs::read(profile.path().join("secrets.bin")).unwrap(),
        b"sealed-fixture"
    );
    assert!(store.set("openaiApiKey", "replacement").is_err());
    assert_eq!(
        fs::read(profile.path().join("secrets.bin")).unwrap(),
        b"sealed-fixture"
    );
}

#[test]
fn config_io_failure_is_not_equivalent_to_invalid_json_recovery() {
    let profile = tempfile::tempdir().unwrap();
    // A directory blocks file reads on Linux and Windows. This is not corrupt
    // JSON and must not be replaced by a clean-looking recovery profile.
    fs::create_dir(profile.path().join("config.json")).unwrap();
    assert!(ConfigStore::open(profile.path(), "win32").is_err());
    assert!(profile.path().join("config.json").is_dir());
    assert_eq!(fs::read_dir(profile.path()).unwrap().count(), 1);
}

#[cfg(windows)]
fn protect_legacy(bytes: &[u8]) -> Vec<u8> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData},
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).unwrap(),
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    assert_ne!(ok, 0);
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    result
}

#[cfg(windows)]
#[test]
fn legacy_dpapi_vault_migrates_without_erasing_other_local_state_fields() {
    let profile = tempfile::tempdir().unwrap();
    let raw = br#"{"openaiApiKey":" public-legacy-secret ","setup:alpha":"public-setup"}"#;
    let encrypted = protect_legacy(raw);
    fs::write(profile.path().join("secrets.bin"), &encrypted).unwrap();
    fs::write(
        profile.path().join("Local State"),
        br#"{"publicSetting":"keep","os_crypt":{"audit_enabled":true}}"#,
    )
    .unwrap();
    let cipher = Arc::new(WindowsCipher::open(profile.path(), true).unwrap());
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(profile.path().join("Local State")).unwrap()).unwrap();
    assert_eq!(state["publicSetting"], "keep");
    assert_eq!(state["os_crypt"]["audit_enabled"], true);
    assert!(state["os_crypt"]["encrypted_key"].is_string());

    let store = SecretStore::open(profile.path(), "win32", cipher).unwrap();
    assert_eq!(
        store.get("openaiApiKey").unwrap().as_deref(),
        Some("public-legacy-secret"),
    );
    assert_eq!(
        store.get("setup:alpha").unwrap().as_deref(),
        Some("public-setup"),
    );
    let migrated = fs::read(profile.path().join("secrets.bin")).unwrap();
    assert!(migrated.starts_with(b"v10"));
    assert_ne!(migrated, encrypted);
    assert!(!store.is_unreadable().unwrap());

    // Cold restart restores the same secrets via the newly persisted key.
    let reopened = SecretStore::open(
        profile.path(),
        "win32",
        Arc::new(WindowsCipher::open(profile.path(), false).unwrap()),
    )
    .unwrap();
    assert_eq!(
        reopened.get("openaiApiKey").unwrap().as_deref(),
        Some("public-legacy-secret"),
    );
}

#[cfg(windows)]
#[test]
fn keyless_aes_vault_or_invalid_dpapi_never_creates_replacement_key() {
    for fixture in [
        b"v10opaque-ciphertext".as_slice(),
        b"not-a-valid-dpapi-blob".as_slice(),
    ] {
        let profile = tempfile::tempdir().unwrap();
        fs::write(profile.path().join("secrets.bin"), fixture).unwrap();
        fs::write(
            profile.path().join("Local State"),
            br#"{"publicSetting":"keep"}"#,
        )
        .unwrap();
        assert!(WindowsCipher::open(profile.path(), true).is_err());
        assert_eq!(
            fs::read(profile.path().join("secrets.bin")).unwrap(),
            fixture
        );
        let state: serde_json::Value =
            serde_json::from_slice(&fs::read(profile.path().join("Local State")).unwrap()).unwrap();
        assert!(state.pointer("/os_crypt/encrypted_key").is_none());
        assert_eq!(state["publicSetting"], "keep");
    }
}
