use cos_core::{
    config::{self, Root},
    sandbox,
    secrets::{SecretCipher, WindowsCipher},
};
use serde_json::json;
use std::{fs, path::Path};
#[cfg(windows)]
#[test]
fn native_raw_identity_preserves_windows_case_and_wsl_linux_case() {
    assert!(sandbox::contained(
        Path::new(r"C:\Public"),
        Path::new(r"\\?\c:\PUBLIC\a")
    ));
    assert!(!sandbox::contained(
        Path::new(r"C:\Public"),
        Path::new(r"C:\Public-sibling\a")
    ));
    assert!(sandbox::contained(
        Path::new(r"\\wsl$\Ubuntu\Home"),
        Path::new(r"\\wsl.localhost\UBUNTU\Home\a")
    ));
    assert!(!sandbox::contained(
        Path::new(r"\\wsl$\Ubuntu\Home"),
        Path::new(r"\\wsl.localhost\Ubuntu\home\a")
    ));
}

fn junction(link: &Path, target: &Path) {
    #[cfg(windows)]
    {
        let script = format!(
            "New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null",
            link.display()
                .to_string()
                .trim_start_matches(r"\\?\")
                .replace('\'', "''"),
            target
                .display()
                .to_string()
                .trim_start_matches(r"\\?\")
                .replace('\'', "''")
        );
        let result = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).unwrap();
    }
}
#[test]
fn approved_root_replacement_and_nested_junction_cannot_move_authority() {
    let temp = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(temp.path()).unwrap();
    let approved = base.join("approved");
    let outside = base.join("outside");
    fs::create_dir(&approved).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("public.txt"), b"outside").unwrap();
    let root = Root {
        name: "project".into(),
        path: approved.to_string_lossy().into_owned(),
        remote: None,
    };
    junction(&approved.join("escape"), &outside);
    assert_eq!(
        fs::read(approved.join("escape/public.txt")).unwrap(),
        b"outside"
    );
    assert!(
        sandbox::resolve(
            std::slice::from_ref(&root),
            "/project/escape/public.txt",
            None,
            false
        )
        .is_err()
    );
    fs::rename(&approved, base.join("original-approved")).unwrap();
    junction(&approved, &outside);
    assert_eq!(fs::read(approved.join("public.txt")).unwrap(), b"outside");
    assert!(
        sandbox::resolve(
            std::slice::from_ref(&root),
            "/project/public.txt",
            None,
            false
        )
        .is_err()
    );
    assert!(
        sandbox::resolve(
            &[root],
            outside.join("public.txt").to_str().unwrap(),
            None,
            false
        )
        .is_err()
    );
}
#[cfg(unix)]
#[test]
fn non_utf8_paths_cannot_alias_through_replacement_characters() {
    use std::os::unix::ffi::OsStringExt;
    let a = std::path::PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/a-\xfe".to_vec()));
    let b = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        b"/tmp/a-\xff/private".to_vec(),
    ));
    assert!(!sandbox::contained(&a, &b));
}
#[test]
fn legacy_darwin_clipboard_mask_keeps_browser_screen_control() {
    let config = cos_core::fresh_config("win32");
    let old = config::effective_capabilities_for_release(&config, "darwin", Some("21.3.0"));
    assert_eq!(old["clipboardRead"], false);
    assert_eq!(old["clipboardWrite"], false);
    assert_eq!(old["screen"], true);
    assert_eq!(old["control"], true);
    let current = config::effective_capabilities_for_release(&config, "darwin", Some("21.4.0"));
    assert_eq!(
        current["clipboardRead"],
        config["capabilities"]["clipboardRead"]
    );
}
#[cfg(windows)]
#[test]
fn first_secret_initializes_existing_keyless_electron_local_state_without_erasing_preferences() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("Local State"),
        br#"{"publicPreference":"keep","os_crypt":{"audit_enabled":true}}"#,
    )
    .unwrap();
    let cipher = WindowsCipher::open(dir.path(), true).unwrap();
    assert_eq!(
        cipher
            .decrypt(&cipher.encrypt("public").unwrap())
            .unwrap()
            .text,
        "public"
    );
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("Local State")).unwrap()).unwrap();
    assert_eq!(state["publicPreference"], "keep");
    assert_eq!(state["os_crypt"]["audit_enabled"], json!(true));
}
