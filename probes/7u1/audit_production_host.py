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
    ("native.callback_dispatch", "probes/7l1/WindowsHost/Program.cs", "Run8PObserveOnlyCdpAsync"),
    ("mv3.tool_authorization", "probes/8p/extension/sw.mjs", "authorizeTool"),
    ("mv3.authorization_sequence_commit", "probes/8p/extension/sw.mjs", "confirmToolAuthorization"),
    # Rust core tools exist, but the production 8P host route must actually wire them.
    ("rust.core_attach", "crates/cos-core/src/browser_tools.rs", "pub fn attach("),
    ("rust.core_bounded_snapshot", "crates/cos-core/src/browser_tools.rs", "pub fn snapshot("),
    ("rust.core_detach", "crates/cos-core/src/browser_tools.rs", "pub fn detach("),
    ("rust.core_lifecycle_revoke", "crates/cos-core/src/browser_tools.rs", "pub fn retire_from_native("),
    ("rust.bridge_attach_request", "crates/cos-core/examples/7l2_bridge.rs", "ToolAttach"),
    ("rust.bridge_snapshot_request", "crates/cos-core/examples/7l2_bridge.rs", "ToolSnapshot"),
    ("rust.bridge_detach_request", "crates/cos-core/examples/7l2_bridge.rs", "ToolDetach"),
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
    "crates/cos-core/src/browser_tools.rs",
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

    native8p = source_text.get("apps/windows-host/Native8P.cs", "")
    callback_dispatch_missing = capabilities.get("native.callback_dispatch") == "MISSING"
    native_api_missing = any(capabilities.get(key) == "MISSING" for key in (
        "native.attach", "native.bounded_snapshot", "native.detach_revoke",
        "native.stale_revoke"))
    mv3_authorization_missing = any(capabilities.get(key) == "MISSING" for key in (
        "mv3.tool_authorization", "mv3.authorization_sequence_commit"))
    rust_core_present = all(capabilities.get(key) == "SOURCE_MARKER_PRESENT" for key in (
        "rust.core_attach", "rust.core_bounded_snapshot", "rust.core_detach",
        "rust.core_lifecycle_revoke", "rust.bridge_attach_request",
        "rust.bridge_snapshot_request", "rust.bridge_detach_request"))
    owner_auth_blocked = ("policy.TryCallback" in native8p and
                          "never load the code-bearing page" in native8p)
    if owner_auth_blocked:
        blockers.append(
            "signed-in owner flow is not qualified: current 8P policy cancels OAuth callback candidates; "
            "a separately authorized 7U1 entrypoint is required without changing #124 policy/IPC")

    interface_gaps = [
        {
            "component": "production 8P C# read-only API",
            "status": "MISSING" if native_api_missing else "SOURCE_MARKERS_PRESENT_REVIEW_REQUIRED",
            "details": "The 8P C# source must expose attach, bounded snapshot, detach, and stale-lease revoke."
        },
        {
            "component": "Rust DirectBrowserTools",
            "status": ("PRESENT_BUT_NOT_WIRED" if rust_core_present and
                       (native_api_missing or callback_dispatch_missing) else
                       ("PRESENT" if rust_core_present else "MISSING")),
            "details": "Rust core attach/snapshot/detach/retire and bridge request variants exist; this is not proof that production 8P dispatches them to its WebView2 page."
        },
        {
            "component": "production 8P MV3 tool authorization",
            "status": "MISSING" if mv3_authorization_missing else "SOURCE_MARKERS_PRESENT_REVIEW_REQUIRED",
            "details": "The production worker needs domain-separated authorization MACs and sequence confirmation for each bounded operation."
        },
        {
            "component": "production WebView2 callback route",
            "status": "MISSING" if callback_dispatch_missing else "SOURCE_MARKERS_PRESENT_REVIEW_REQUIRED",
            "details": "The existing 7L2 callback is synthetic-origin-bound and cannot be reused as a production provider adapter."
        },
        {
            "component": "signed-in owner navigation",
            "status": "BLOCKED_BY_CURRENT_8P_POLICY" if owner_auth_blocked else "REVIEW_REQUIRED",
            "details": "OAuth callback candidates are currently cancelled by 8P; do not change #124 policy/IPC from this task."
        }
    ]

    status = "BLOCKED" if blockers else "API_SURFACE_PRESENT_REVIEW_REQUIRED"
    return {
        "schema": "cosos.7u1.production-host-audit.v1",
        "sourceSha": observed_sha if SHA40.fullmatch(observed_sha or "") else None,
        "productionObservationStatus": status,
        "capabilities": capabilities,
        "missingCapabilities": missing_capabilities,
        "interfaceGaps": interface_gaps,
        "blockers": blockers,
        "owner7U1Status": "NOT_RUN",
        "owner7U1Accepted": False,
        "humanConsentSimulated": False,
        "providerAccessPerformed": False,
        "outgoingSendCount": None,
        "note": "Static audit only: Rust direct tools exist but are not wired to the production 8P C#/MV3/WebView2 path; no runtime proof or owner acceptance is implied."
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
