use cos_core::secrets::{SecretCipher, WindowsCipher};
use std::{fs, path::Path};
fn main() -> std::io::Result<()> {
    let directory = std::env::args()
        .nth(1)
        .expect("explicit isolated probe path");
    if !directory.contains("phase1-3-runtime") {
        return Err(std::io::Error::other("Probe directory required"));
    }
    let directory = Path::new(&directory);
    let cipher = WindowsCipher::open(&directory.join("electron-profile"), false)?;
    let original = fs::read(directory.join("electron-secrets.bin"))?;
    let text = cipher.decrypt(&original)?.text;
    let expected = r#"{"bridgeToken":"public-test-token","openaiApiKey":"public-test-key","future:key":"preserved"}"#;
    if text != expected {
        return Err(std::io::Error::other("Oracle plaintext mismatch"));
    }
    fs::write(
        directory.join("rust-secrets.bin"),
        cipher.encrypt(expected)?,
    )?;
    fs::write(
        directory.join("rust-crypto-probe.json"),
        b"{\"electronRead\":true,\"authenticatedRoundTrip\":true}",
    )?;
    Ok(())
}
