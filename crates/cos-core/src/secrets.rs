//! OS credential custody. All disk/provider operations share one publication fence.
use crate::{
    config::js_trim,
    persistence::{atomic_write, invalid, lock, owned_path},
};
use serde_json::{Map, Value};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;

pub struct Decrypted {
    pub text: String,
    pub rotate: bool,
}
pub trait SecretCipher: Send + Sync {
    fn available(&self) -> bool;
    fn encrypt(&self, text: &str) -> io::Result<Vec<u8>>;
    fn decrypt(&self, bytes: &[u8]) -> io::Result<Decrypted>;
}
fn unavailable() -> io::Error {
    io::Error::other("Secure OS credential storage is unavailable; nothing was saved")
}
fn unreadable() -> io::Error {
    io::Error::other("Saved credentials could not be read; encrypted data was left untouched")
}
pub fn clean_api_key(text: &str) -> String {
    text.chars().filter(|c|!c.is_whitespace()&&!matches!(*c,'\u{ad}'|'\u{180e}'|'\u{200b}'..='\u{200f}'|'\u{2028}'..='\u{202f}'|'\u{205f}'..='\u{2064}'|'\u{2066}'..='\u{206f}'|'\u{feff}')).collect()
}
fn api_key(key: &str) -> bool {
    matches!(
        key,
        "openaiApiKey" | "openRouterApiKey" | "customProviderApiKey"
    ) || key.starts_with("setup:")
}
fn protected(platform: &str, bytes: &[u8]) -> bool {
    !bytes.is_empty() && (platform != "linux" || !bytes.starts_with(b"v10"))
}
#[derive(Default)]
struct State {
    cache: Option<Map<String, Value>>,
    unreadable: bool,
    rotate: bool,
}
pub struct SecretStore {
    directory: PathBuf,
    platform: String,
    cipher: Arc<dyn SecretCipher>,
    state: Mutex<State>,
}
impl SecretStore {
    pub fn open(
        directory: &Path,
        platform: &str,
        cipher: Arc<dyn SecretCipher>,
    ) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        Ok(Self {
            directory: fs::canonicalize(directory)?,
            platform: platform.into(),
            cipher,
            state: Mutex::new(State::default()),
        })
    }
    fn path(&self) -> io::Result<PathBuf> {
        owned_path(&self.directory, Path::new("secrets.bin"), true)
    }
    fn load(&self, state: &mut State) -> io::Result<()> {
        if state.cache.is_some() || !self.cipher.available() {
            return Ok(());
        }
        let result = (|| {
            let bytes = match fs::read(self.path()?) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    return Ok((Map::new(), false));
                }
                Err(e) => return Err(e),
            };
            if !protected(&self.platform, &bytes) {
                return Err(unreadable());
            }
            let plaintext = self.cipher.decrypt(&bytes)?;
            let text = Zeroizing::new(plaintext.text);
            let value: Value = serde_json::from_str(&text).map_err(|_| unreadable())?;
            let Some(map) = value.as_object() else {
                return Err(unreadable());
            };
            if map.values().any(|v| !v.is_string()) {
                return Err(unreadable());
            }
            Ok((map.clone(), plaintext.rotate))
        })();
        match result {
            Ok((map, rotate)) => {
                state.cache = Some(map);
                state.rotate = rotate;
                state.unreadable = false;
            }
            Err(_) => {
                state.unreadable = self.cipher.available();
                state.rotate = false;
            }
        }
        Ok(())
    }
    fn write(&self, state: &mut State, next: Map<String, Value>) -> io::Result<()> {
        if !self.cipher.available() {
            return Err(unavailable());
        }
        let text = Zeroizing::new(serde_json::to_string(&next)?);
        let bytes = self.cipher.encrypt(&text)?;
        if !protected(&self.platform, &bytes) {
            return Err(unavailable());
        }
        atomic_write(&self.path()?, &bytes)?;
        state.cache = Some(next);
        state.rotate = false;
        state.unreadable = false;
        Ok(())
    }
    pub fn get(&self, key: &str) -> io::Result<Option<String>> {
        let mut state = lock(&self.state)?;
        self.load(&mut state)?;
        let stored = state
            .cache
            .as_ref()
            .and_then(|map| map.get(key))
            .and_then(Value::as_str)
            .map(|s| {
                if api_key(key) {
                    clean_api_key(s)
                } else {
                    s.into()
                }
            })
            .filter(|s| !s.is_empty());
        if state.unreadable {
            // An unreadable credential vault must not be confused with an
            // empty vault (in particular by the startup tunnel caller).
            return Err(unreadable());
        }
        if state.rotate
            && let Some(next) = state.cache.clone()
        {
            let _ = self.write(&mut state, next);
        }
        Ok(stored)
    }
    pub fn set(&self, key: &str, value: &str) -> io::Result<()> {
        if key.is_empty() || key.len() > 1024 {
            return Err(invalid("Invalid credential key"));
        }
        let mut state = lock(&self.state)?;
        if !self.cipher.available() {
            return Err(unavailable());
        }
        self.load(&mut state)?;
        let Some(mut next) = state.cache.clone() else {
            return Err(if state.unreadable {
                unreadable()
            } else {
                unavailable()
            });
        };
        let text = if api_key(key) {
            clean_api_key(value)
        } else {
            js_trim(value).into()
        };
        if text.is_empty() {
            next.remove(key);
        } else {
            next.insert(key.into(), Value::String(text));
        }
        self.write(&mut state, next)
    }
    pub fn delete_all(&self) -> io::Result<()> {
        let mut state = lock(&self.state)?;
        state.cache = None;
        state.rotate = false;
        match fs::remove_file(self.path()?) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        state.cache = Some(Map::new());
        state.unreadable = false;
        Ok(())
    }
    pub fn is_unreadable(&self) -> io::Result<bool> {
        let mut state = lock(&self.state)?;
        self.load(&mut state)?;
        Ok(state.unreadable)
    }
    pub fn set_aside_unreadable(&self) -> io::Result<Option<PathBuf>> {
        let mut state = lock(&self.state)?;
        if !self.cipher.available() {
            return Err(unavailable());
        }
        self.load(&mut state)?;
        if state.cache.is_some() || !state.unreadable {
            return Ok(None);
        }
        let backup = owned_path(
            &self.directory,
            Path::new(&format!(
                "secrets.unreadable-{}-{}.bin",
                chrono::Utc::now().format("%Y%m%d-%H%M%S%3f"),
                uuid::Uuid::new_v4()
            )),
            true,
        )?;
        fs::rename(self.path()?, &backup)?;
        state.cache = Some(Map::new());
        state.unreadable = false;
        state.rotate = false;
        Ok(Some(backup))
    }
}

/// Unsupported hosts fail closed until their native provider and migration are qualified.
pub struct UnavailableCipher;
impl SecretCipher for UnavailableCipher {
    fn available(&self) -> bool {
        false
    }
    fn encrypt(&self, _: &str) -> io::Result<Vec<u8>> {
        Err(unavailable())
    }
    fn decrypt(&self, _: &[u8]) -> io::Result<Decrypted> {
        Err(unavailable())
    }
}

/// Chromium Windows v10: DPAPI-wrapped AES-256 key in Local State, 12-byte nonce,
/// authenticated ciphertext with a 16-byte tag. Older direct DPAPI blobs remain readable.
pub struct WindowsCipher {
    key: Zeroizing<Vec<u8>>,
}
impl WindowsCipher {
    #[cfg(windows)]
    pub fn open(profile: &Path, create: bool) -> io::Result<Self> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        fs::create_dir_all(profile)?;
        let path = owned_path(profile, Path::new("Local State"), true)?;
        let mut state: Value = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| unreadable())?,
            Err(e) if e.kind() == io::ErrorKind::NotFound && create => serde_json::json!({}),
            Err(e) => return Err(e),
        };
        if !state.is_object() {
            return Err(unreadable());
        }
        let key = if let Some(encoded) = state
            .pointer("/os_crypt/encrypted_key")
            .and_then(Value::as_str)
        {
            let raw = STANDARD.decode(encoded).map_err(|_| unreadable())?;
            if !raw.starts_with(b"DPAPI") {
                return Err(unreadable());
            }
            dpapi(&raw[5..], false)?
        } else {
            if !create || state.pointer("/os_crypt/encrypted_key").is_some() {
                return Err(unreadable());
            }
            // Legacy Windows profiles may have direct DPAPI credentials but
            // no Chromium AES key yet. Only issue a new key after proving that
            // the existing blob really is readable direct-DPAPI JSON. Never
            // replace missing key material for v10 AES or damaged credentials.
            let legacy_path = owned_path(profile, Path::new("secrets.bin"), true)?;
            match fs::read(&legacy_path) {
                Ok(blob) => {
                    if blob.is_empty() || blob.starts_with(b"v10") || blob.len() > 8 * 1024 * 1024 {
                        return Err(unreadable());
                    }
                    let decoded = Zeroizing::new(dpapi(&blob, false).map_err(|_| unreadable())?);
                    let value: Value =
                        serde_json::from_slice(&decoded).map_err(|_| unreadable())?;
                    if value
                        .as_object()
                        .is_none_or(|entries| entries.values().any(|v| !v.is_string()))
                    {
                        return Err(unreadable());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => return Err(unreadable()),
            }
            let mut key = Zeroizing::new(vec![0; 32]);
            getrandom::fill(&mut key).map_err(|_| unavailable())?;
            let mut protected = b"DPAPI".to_vec();
            protected.extend(dpapi(&key, true)?);
            if state
                .get("os_crypt")
                .is_some_and(|value| !value.is_object())
            {
                return Err(unreadable());
            }
            if !state["os_crypt"].is_object() {
                state["os_crypt"] = serde_json::json!({});
            }
            state["os_crypt"]["encrypted_key"] = serde_json::json!(STANDARD.encode(protected));
            atomic_write(&path, &serde_json::to_vec(&state)?)?;
            key.to_vec()
        };
        if key.len() != 32 {
            return Err(unreadable());
        }
        Ok(Self {
            key: Zeroizing::new(key),
        })
    }
    #[cfg(not(windows))]
    pub fn open(_: &Path, _: bool) -> io::Result<Self> {
        Err(unavailable())
    }
}
impl SecretCipher for WindowsCipher {
    fn available(&self) -> bool {
        cfg!(windows) && self.key.len() == 32
    }
    fn encrypt(&self, text: &str) -> io::Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| unavailable())?;
        let mut nonce = [0; 12];
        getrandom::fill(&mut nonce).map_err(|_| unavailable())?;
        let encrypted = cipher
            .encrypt(Nonce::from_slice(&nonce), text.as_bytes())
            .map_err(|_| unavailable())?;
        let mut bytes = b"v10".to_vec();
        bytes.extend(nonce);
        bytes.extend(encrypted);
        Ok(bytes)
    }
    fn decrypt(&self, bytes: &[u8]) -> io::Result<Decrypted> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        let (plaintext, rotate) = if bytes.starts_with(b"v10") {
            if bytes.len() < 31 {
                return Err(unreadable());
            }
            let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| unavailable())?;
            (
                cipher
                    .decrypt(Nonce::from_slice(&bytes[3..15]), &bytes[15..])
                    .map_err(|_| unreadable())?,
                false,
            )
        } else {
            #[cfg(windows)]
            {
                (dpapi(bytes, false)?, true)
            }
            #[cfg(not(windows))]
            {
                return Err(unavailable());
            }
        };
        Ok(Decrypted {
            text: String::from_utf8(plaintext).map_err(|_| unreadable())?,
            rotate,
        })
    }
}
#[cfg(windows)]
fn dpapi(bytes: &[u8], encrypt: bool) -> io::Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    let count = u32::try_from(bytes.len())
        .map_err(|_| invalid("Credential blob exceeds supported size"))?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: count,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // DPAPI owns output; it is copied and freed exactly once on success. UI is forbidden.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(unavailable());
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}
