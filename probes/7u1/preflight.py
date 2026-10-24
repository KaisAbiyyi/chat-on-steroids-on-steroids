#!/usr/bin/env python3
"""7U1-PREP: fail-closed source/artifact preflight; NEVER opens a provider."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys

PROVIDERS = {"chatgpt": "https://chatgpt.com", "claude": "https://claude.ai",
             "gemini": "https://gemini.google.com"}
SHA40 = re.compile(r"[0-9a-f]{40}")
SHA64 = re.compile(r"[0-9a-f]{64}")


class Refused(Exception):
    pass


def hash_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_manifest(artifact, expected_sha):
    artifact = artifact.absolute()
    source = artifact / "source.sha"
    manifest = artifact / "hashes.json"
    if not source.is_file() or not manifest.is_file():
        raise Refused("missing source.sha or hashes.json")
    if source.read_text(encoding="utf-8").strip() != expected_sha:
        raise Refused("artifact is not pinned to this commit")
    items = json.loads(manifest.read_text(encoding="utf-8"))
    if not isinstance(items, list) or not items:
        raise Refused("invalid SHA256 manifest")
    seen = set()
    for item in items:
        if not isinstance(item, dict) or set(item) != {"path", "sha256"}:
            raise Refused("unexpected manifest fields")
        raw = item["path"]
        if (not isinstance(raw, str) or not raw or "\\" in raw or
                ":" in raw or PurePosixPath(raw).is_absolute() or
                any(x in (".", "..", "") for x in raw.split("/")) or
                PurePosixPath(raw).as_posix() != raw or raw in seen):
            raise Refused("unsafe or duplicate manifest path")
        if not isinstance(item["sha256"], str) or not SHA64.fullmatch(item["sha256"]):
            raise Refused("invalid SHA256")
        seen.add(raw)
        file = artifact / raw
        if not file.is_file() or hash_file(file) != item["sha256"]:
            raise Refused("missing or mutated artifact")
    actual = set()
    for path in artifact.rglob("*"):
        if path.is_symlink() or getattr(path, "is_junction", lambda: False)():
            raise Refused("symlink/junction artifact is forbidden")
        if path.is_file():
            actual.add(path.relative_to(artifact).as_posix())
    if actual - {"hashes.json"} != seen or "source.sha" not in seen:
        raise Refused("unmanifested or missing native loadable file")
    return seen


def verify_product_adapter(repo, files, provider):
    declaration = repo / "probes" / "7u1" / "product-integration.json"
    if not declaration.is_file():
        raise Refused("real CoSoS product WebView2/Rust adapter is missing (8L1 #71)")
    value = json.loads(declaration.read_text(encoding="utf-8"))
    expected = {
        "schema": "cosos.7u1.product-integration.v1",
        "kind": "product-webview2-rust-mv3",
        "host": "host/CoSoSProductBrowser.dll",
        "bridge": "7u1_product_bridge.exe",
        "allowedOrigins": [PROVIDERS[provider]],
        "allowedOps": ["attach", "snapshot", "detach"],
        "captureLimitBytes": 65536,
        "noContentPersistence": True,
        "navigation": "owner-only",
        "profile": "fresh-isolated-only",
        "consent": "per-session-explicit",
        "sendCapability": "absent",
    }
    if value != expected:
        raise Refused("unsupported/insecure product adapter declaration")
    if expected["host"] not in files or expected["bridge"] not in files:
        raise Refused("real product host and Rust bridge absent from SHA256 manifest")
    # Metadata is never accepted as proof of the native implementation.


def report(repo, artifact, sha, provider, profile, consent, observed_sha=None):
    checks = {}
    def check(key, status, evidence):
        checks[key] = {"status": status, "evidence": evidence}

    check("origin_allowlist", "PASS" if provider in PROVIDERS else "FAIL",
          "Exact provider origin" if provider in PROVIDERS else "Unknown origin")
    ok_profile = bool(re.fullmatch(r"7u1-[a-zA-Z0-9_-]{8,48}", profile or ""))
    check("isolated_profile_id", "PASS" if ok_profile else "FAIL",
          "Dedicated test profile name" if ok_profile else "Default/unsafe test profile")
    check("explicit_owner_opt_in", "PASS" if consent else "BLOCKED",
          "Opt-in flag recorded" if consent else "Owner consent not given")
    try:
        head = (observed_sha or subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "HEAD"],
            capture_output=True, text=True, check=True, timeout=10
        ).stdout.strip()).lower()
        if not SHA40.fullmatch(sha or "") or head != sha:
            raise Refused("source SHA mismatch")
        check("source_provenance", "PASS", "Git HEAD matches expected source SHA")
    except (Refused, OSError, subprocess.SubprocessError) as e:
        check("source_provenance", "FAIL", str(e))
    files = None
    try:
        if checks["source_provenance"]["status"] != "PASS":
            raise Refused("source not attested")
        files = verify_manifest(artifact, sha)
        check("artifact_manifest", "PASS", "Full SHA256 coverage and source pin")
    except (Refused, OSError, ValueError) as e:
        check("artifact_manifest", "BLOCKED", str(e))
    try:
        if files is None or provider not in PROVIDERS:
            raise Refused("source and artifact proof required")
        verify_product_adapter(repo, files, provider)
        check("product_native_adapter", "PASS", "Metadata only; native review required")
    except (Refused, OSError, ValueError) as e:
        check("product_native_adapter", "BLOCKED", str(e))

    for key, why in (
        ("native_8l1_acceptance", "Real integrated Windows host gate not run"),
        ("signed_in_owner_observation", "Only owner can authorize provider sign-in"),
        ("stale_handle_denial", "Navigation/worker retirement not physically tested"),
        ("zero_outgoing_send", "No provider launched: zero Send is NOT proven"),
    ):
        check(key, "NOT_RUN", why)
    prep = all(checks[k]["status"] == "PASS" for k in (
        "origin_allowlist", "isolated_profile_id", "explicit_owner_opt_in",
        "source_provenance", "artifact_manifest", "product_native_adapter"))
    return {
        "schema": "cosos.7u1.preflight.v1",
        "sourceSha": sha if SHA40.fullmatch(sha or "") else None,
        "provider": provider if provider in PROVIDERS else None,
        "preflightStatus": "PREP_READY_REVIEW_REQUIRED" if prep else "BLOCKED",
        "owner7U1Status": "NOT_RUN", "owner7U1Accepted": False,
        "outgoingSendCount": None, "checks": checks
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for arg in ("repo", "artifact", "receipt"):
        p.add_argument("--" + arg, required=True, type=Path)
    for arg in ("expected-sha", "provider", "profile"):
        p.add_argument("--" + arg, required=True)
    p.add_argument("--owner-consent", action="store_true")
    a = p.parse_args()
    result = report(a.repo, a.artifact, a.expected_sha, a.provider,
                    a.profile, a.owner_consent)
    a.receipt.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(result["preflightStatus"], "owner7U1Status=NOT_RUN")
    return 0 if result["preflightStatus"] == "PREP_READY_REVIEW_REQUIRED" else 2


if __name__ == "__main__":
    sys.exit(main())
