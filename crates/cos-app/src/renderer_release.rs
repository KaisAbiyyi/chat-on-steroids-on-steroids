//! 15HOST release-issuer trust gate. Payload SHA-256 alone is not issuer auth.
//!
//! Production requires a GitHub OIDC/Sigstore SLSA attestation of the EXACT
//! renderer-integrity.json bytes, bound to the protected main source ref,
//! exact source commit and dedicated workflow identity. Every runnable file is
//! separately hashed and guarded by renderer_integrity while the child runs.
//!
//! Trust boundary: the GitHub CLI installation at Program Files must be OS /
//! administrator protected. PATH, caller supplied gh paths and bundled gh.exe
//! are NEVER trusted. No gh, offline API, invalid certificate or mismatched
//! identity => no renderer launch. This is an online fail-closed release gate.
use std::{
    io,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const REPO: &str = "KaisAbiyyi/chat-on-steroids-on-steroids";
const SIGNER: &str =
    "KaisAbiyyi/chat-on-steroids-on-steroids/.github/workflows/15host-renderer.yml";
const AUTHORIZED_SOURCE_REF: &str = "refs/heads/main";
const GH_EXE: &str = r"C:\Program Files\GitHub CLI\gh.exe";
const VERIFY_TIMEOUT: Duration = Duration::from_secs(35);

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Native renderer release issuer verification rejected",
    )
}

fn sha_is_valid(source: &str) -> bool {
    source.len() == 40 && source.bytes().all(|value| value.is_ascii_hexdigit())
}

fn arguments(manifest: &Path, source_sha: &str) -> Vec<String> {
    vec![
        "attestation".into(),
        "verify".into(),
        manifest.to_string_lossy().into_owned(),
        "--repo".into(),
        REPO.into(),
        "--signer-workflow".into(),
        SIGNER.into(),
        "--source-ref".into(),
        AUTHORIZED_SOURCE_REF.into(),
        "--source-digest".into(),
        source_sha.into(),
    ]
}

pub fn verify_trusted_release(
    renderer: &Path,
    manifest_source_sha: Option<&str>,
    expected_source_sha: Option<&str>,
) -> io::Result<()> {
    let source = expected_source_sha
        .filter(|v| sha_is_valid(v))
        .ok_or_else(denied)?;
    if manifest_source_sha != Some(source) {
        return Err(denied());
    }
    let gh_path = Path::new(GH_EXE);
    // Fixed OS-protected program path, never PATH resolution. A writable
    // attacker-controlled CLI would invalidate all provenance guarantees.
    let gh_metadata = std::fs::symlink_metadata(gh_path).map_err(|_| denied())?;
    if !gh_metadata.is_file() || gh_metadata.file_type().is_symlink() {
        return Err(denied());
    }
    let manifest = renderer
        .parent()
        .ok_or_else(denied)?
        .join("renderer-integrity.json");
    let mut child = Command::new(gh_path)
        .args(arguments(&manifest, source))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| denied())?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(denied())
                };
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(denied());
            }
            Ok(None) if start.elapsed() >= VERIFY_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(denied());
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_policy_is_pinned_not_caller_selected() {
        let args = arguments(
            Path::new("renderer-integrity.json"),
            "a".repeat(40).as_str(),
        );
        assert!(args.windows(2).any(|w| w == ["--repo", REPO]));
        assert!(args.windows(2).any(|w| w == ["--signer-workflow", SIGNER]));
        assert!(
            args.windows(2)
                .any(|w| w == ["--source-ref", "refs/heads/main"])
        );
        assert!(
            !args
                .iter()
                .any(|a| a == "refs/heads/work/15host-production-renderer-20261026")
        );
    }

    #[test]
    fn no_release_proof_and_mismatched_source_fail_closed() {
        let renderer = Path::new("C:/absent/WindowsRenderer.exe");
        let sha = "a".repeat(40);
        assert!(verify_trusted_release(renderer, None, Some(&sha)).is_err());
        assert!(verify_trusted_release(renderer, Some(&sha), None).is_err());
        assert!(verify_trusted_release(renderer, Some(&sha), Some(&"b".repeat(40))).is_err());
        assert!(verify_trusted_release(renderer, Some(&sha), Some("forged")).is_err());
    }
}
