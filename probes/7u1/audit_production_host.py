#!/usr/bin/env python3
"""Static audit of the production 8P host for a missing read-only observation API."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

SHA40 = re.compile(r"[0-9a-f]{40}\Z")

REQUIRED = (
    ("native.attach", "apps/windows-host/Native8P.cs", "ObserveOnlyAttachAsync"),
    ("native.bounded_snapshot", "apps/windows-host/Native8P.cs", "ObserveOnlySnapshotAsync"),
    ("native.detach_revoke", "apps/windows-host/Native8P.cs", "ObserveOnlyDetachAsync"),
    ("native.stale_revoke", "apps/windows-host/Native8P.cs", "RevokeObserveOnlyLease"),
    ("rust.attach", "crates/cos-core/src/native_browser_bridge.rs", "observe_only_attach"),
    ("rust.bounded_snapshot", "crates/cos-core/src/native_browser_bridge.rs", "observe_only_snapshot"),
    ("rust.detach_revoke", "crates/cos-core/src/native_browser_bridge.rs", "observe_only_detach"),
    ("rust.stale_revoke", "crates/cos-core/src/native_browser_bridge.rs", "revoke_observe_only_lease"),
    ("mv3.attach", "probes/8p/extension/sw.mjs", "observeOnlyAttach"),
    ("mv3.bounded_snapshot", "probes/8p/extension/sw.mjs", "observeOnlySnapshot"),
    ("mv3.detach_revoke", "probes/8p/extension/sw.mjs", "observeOnlyDetach"),
    ("mv3.stale_revoke", "probes/8p/extension/sw.mjs", "revokeObserveOnlyLease"),
)

SOURCE_FILES = (
    "apps/windows-host/WindowsHost.csproj",
    "apps/windows-host/Native8P.cs",
    "apps/windows-host/Native8L1.cs",
    "probes/7l1/WindowsHost/Program.cs",
    "probes/7l1/WindowsHost/Native7L2.cs",
    "probes/7l1/WindowsHost/NativeRetirementProof.cs",
    "probes/7l1/WindowsHost/NativeProfileExtensions.cs",
    "probes/7l1/WindowsHost/NativePermissionProgress.cs",
    "Cargo.toml",
    "Cargo.lock",
    "crates/cos-core/Cargo.toml",
    "crates/cos-core/examples/7l2_bridge.rs",
    "crates/cos-core/src/native_browser_bridge.rs",
    "probes/8p/extension/manifest.json",
    "probes/8p/extension/sw.mjs",
)


def audit(repo, expected_sha=None, observed_sha=None):
    repo = Path(repo).absolute()
    blockers = []
    if observed_sha is None:
        try:
            observed_sha = subprocess.run(
                ["git", "-C", str(repo), "rev-parse", "HEAD"],
                check=True, capture_output=True, text=True, timeout=10
            ).stdout.strip().lower()
            status = subprocess.run(
                ["git", "-C", str(repo), "status", "--porcelain", "--untracked-files=all"],
                check=True, capture_output=True, text=True, timeout=10
            )
            if status.stdout.strip():
                blockers.append("checkout is dirty; source attestation is not reproducible")
        except (OSError, subprocess.SubprocessError) as error:
            observed_sha = None
            blockers.append("cannot read Git source revision: " + str(error))
    else:
        observed_sha = str(observed_sha).lower()

    if not SHA40.fullmatch(observed_sha or ""):
        blockers.append("invalid or unavailable source SHA")
    if expected_sha is not None and observed_sha != str(expected_sha).lower():
        blockers.append("source SHA does not match expected HEAD")

    missing_files = []
    source_text = {}
    for relative in SOURCE_FILES:
        path = repo / relative
        if not path.is_file():
            missing_files.append(relative)
        else:
            try:
                source_text[relative] = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError) as error:
                missing_files.append(relative + " (unreadable: " + str(error) + ")")
    if missing_files:
        blockers.append("missing production source: " + ", ".join(missing_files))

    missing_capabilities = []
    capabilities = {}
    for capability, relative, marker in REQUIRED:
        found = marker in source_text.get(relative, "")
        capabilities[capability] = "SOURCE_MARKER_PRESENT" if found else "MISSING"
        if not found:
            missing_capabilities.append(capability + " [" + marker + "]")
    if missing_capabilities:
        blockers.append("production read-only observation contract is absent: " +
                        ", ".join(missing_capabilities))

    status = "BLOCKED" if blockers else "API_SURFACE_PRESENT_REVIEW_REQUIRED"
    return {
        "schema": "cosos.7u1.production-host-audit.v1",
        "sourceSha": observed_sha if SHA40.fullmatch(observed_sha or "") else None,
        "productionObservationStatus": status,
        "capabilities": capabilities,
        "missingCapabilities": missing_capabilities,
        "blockers": blockers,
        "owner7U1Status": "NOT_RUN",
        "owner7U1Accepted": False,
        "humanConsentSimulated": False,
        "providerAccessPerformed": False,
        "outgoingSendCount": None,
        "note": "Static source audit only; this report is not runtime proof or owner acceptance."
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--expected-sha", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    result = audit(args.repo, expected_sha=args.expected_sha)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(result["productionObservationStatus"], "owner7U1Status=NOT_RUN")
    if result["sourceSha"] != args.expected_sha.lower():
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
