#!/usr/bin/env python3
"""16F: bounded, source-linked cloud release security/provenance report.

This generator proves pinned merges are ancestors of the checked-out commit and
hashes tracked contracts. It does NOT authenticate a signer, attest GitHub CI
statuses, prove a native install, or authorize reading a private profile.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

SCHEMA = "cosos.release.cloud-provenance.v1"
SHA40 = re.compile(r"[0-9a-f]{40}\Z")
MAX_RECEIPT_BYTES = 128 * 1024
MAX_SOURCE_BYTES = 2 * 1024 * 1024
MERGED = {
    "16A": (103, "a2d3781dd499378ef53e022b543c891fe12df93f"),
    "16B": (110, "3b8e6d2fe006035dda98ab9fa38dfe73ec1f1a68"),
    "16C": (111, "326b0fca66c8deefce85161e298ff20262a57358"),
    "16D": (119, "9ab33009aafdd24ebd4ec093e7c46f4e45985846"),
    "16E": (118, "c9f17ed5654e42cd4497f01309d9b6cfcf3dd0f1"),
}
TRACKED = (
    "LICENSE",
    "Cargo.lock",
    "docs/task-registry.json",
    "tools/release_16b.py",
    "tools/release_16c.py",
    "tools/release_16d.py",
    "tools/release_16e.py",
    "docs/implementation-summary/2026-10-24-16a-reversible-migration.md",
    "docs/implementation-summary/2026-10-24-16b-updater-interface.md",
    "docs/implementation-summary/2026-10-24-16c-bundle-licenses.md",
    "docs/implementation-summary/2026-10-25-16d-cloud-headless-perf.md",
    "docs/implementation-summary/2026-10-25-16e-install-matrix.md",
    "docs/CURRENT-PHASE.md",
    "tools/release_16f.py",
    "tools/test_release_16f.py",
    ".github/workflows/16f-release-report.yml",
    "docs/implementation-summary/2026-10-25-16f-cloud-release-security.md",
)
GATES = (
    "16L1_native_installer",
    "16U1_owner_consent",
    "16L2_owner_approved_copy_migration",
    "16LF-01_owner_local_fresh_start",
    "16L3_native_full_release_performance",
    "16U2_owner_release_decision",
    "PERF-05_native_product_comparison",
    "PERF-06_product_performance_budget",
)
class Refused(ValueError):
    pass

def require(ok, reason):
    if not ok:
        raise Refused(reason)

def valid_sha(value):
    return isinstance(value, str) and SHA40.fullmatch(value) is not None

def make_report(expected_head, actual_head, ancestors, sources):
    """Pure validator: caller provides checked-out HEAD's immutable ancestry/blobs."""
    require(valid_sha(expected_head) and actual_head == expected_head, "source HEAD mismatch")
    require(isinstance(ancestors, dict) and set(ancestors) == set(MERGED), "incomplete predecessor ancestry evidence")
    require(all(ancestors[k] is True for k in MERGED), "required accepted merge missing from source ancestry")
    require(isinstance(sources, dict) and set(sources) == set(TRACKED), "missing or extra source audit file")
    hashes = {}
    for path in TRACKED:
        data = sources[path]
        require(isinstance(data, bytes) and 0 < len(data) <= MAX_SOURCE_BYTES, "missing or oversized tracked source: " + path)
        hashes[path] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    try:
        registry = json.loads(sources["docs/task-registry.json"])
    except (UnicodeError, ValueError) as exc:
        raise Refused("invalid canonical task registry") from exc
    require(isinstance(registry, dict) and isinstance(registry.get("tasks"), list), "invalid task registry structure")
    tasks = {item["id"]: item for item in registry["tasks"] if isinstance(item, dict) and isinstance(item.get("id"), str)}
    require(len(tasks) == len(registry["tasks"]), "invalid or duplicate task IDs")
    require(set(tasks["16F"]["depends_on"]) == {"16D", "16E"}, "16F dependency contract changed")
    require(set(tasks["16L3"]["depends_on"]) == {"16F", "16L1"} and
            tasks["16L3"].get("any_of") == [["16L2", "16LF-01"]],
            "16L3 required AND/OR native gate dependencies changed")
    require(tasks.get("16FS-01", {}).get("kind") == "C" and
            tasks["16FS-01"].get("depends_on") == ["16U1", "16L1", "16F"] and
            tasks.get("16LF-01", {}).get("kind") == "L" and
            tasks["16LF-01"].get("depends_on") == ["16FS-01", "16U1", "16L1"] and
            tasks["16LF-01"].get("status") == "blocked" and
            tasks["16LF-01"].get("evidence") is None,
            "16FS-01 / 16LF-01 independent fresh-start gate changed or is no longer NOT_RUN")
    require(tasks.get("16U1", {}).get("migration_consent") == "declined" and
            tasks.get("16L2", {}).get("status") == "blocked",
            "owner no-migration decision or private 16L2 block changed")
    require(tasks["16F"]["kind"] == "C" and tasks["16L3"]["kind"] == "L", "task environment gate changed")
    return {
        "schema": SCHEMA,
        "source_sha": expected_head,
        "scope": "GITHUB_HOSTED_SOURCE_AND_SYNTHETIC_CONTRACTS_ONLY",
        "merged_predecessors": {
            task: {
                "pull_request": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/" + str(number),
                "merged_commit_sha": commit,
                "merge_ancestor_at_source": True,
                "conclusion": "SOURCE_ANCESTRY_ONLY",
            }
            for task, (number, commit) in MERGED.items()
        },
        "tracked_contracts": hashes,
        "security_review": {
            "updater_signer": "16B_OPTIONAL_SIGNED_COMPATIBILITY_NOT_ATTESTED_HERE",
            "unsigned_installer_trust": "16PKG-01_MANUAL_SHA256_AND_EXTERNAL_GITHUB_ATTESTATION_REQUIRED",
            "bundle_signature": "16C_UNSIGNED_HEADLESS_ONLY",
            "license_inventory": "16C_LOCKED_DEPENDENCY_CONTRACT_NOT_EXTERNAL_LEGAL_CERTIFICATION",
            "migration": "16A_COPY_ONLY_NO_PRIVATE_DATA",
            "installer_transaction": "16E_SYNTHETIC_DISPOSABLE_ONLY",
            "startup_memory": "16D_HEADLESS_ROOT_PROCESS_ONLY_NO_GUI_BUDGET",
            "github_ci_verdict": "REQUIRES_EXTERNAL_EXACT_HEAD_CHECKS",
            "independent_review": "REQUIRES_HUMAN_OR_SEPARATE_REVIEW_RECEIPT",
        },
        "native_owner_gates": {gate: "NOT_RUN" for gate in GATES},
        "product_release_ready": False,
        "cloud_closure": "PROVISIONAL_UNTIL_EXACT_HEAD_CI_AND_REVIEW",
    }

def validate_receipt(receipt, expected):
    require(isinstance(receipt, dict) and receipt == expected, "receipt differs from reproducible source audit")
    require(receipt.get("product_release_ready") is False, "forged release approval")
    require(set(receipt.get("native_owner_gates", {})) == set(GATES), "native gate omission")
    require(all(value == "NOT_RUN" for value in receipt["native_owner_gates"].values()), "forged native or owner acceptance")
    return True

def git(repo, *argv):
    try:
        run = subprocess.run(["git", "-C", str(repo), *argv], capture_output=True,
                             check=False, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise Refused("git evidence unavailable") from exc
    require(run.returncode == 0, "git evidence failed: " + " ".join(argv[:2]))
    return run.stdout.strip()

def collect(repo, expected_head):
    root = Path(repo).resolve(strict=True)
    actual_head = git(root, "rev-parse", "HEAD").decode("ascii")
    require(valid_sha(expected_head) and actual_head == expected_head, "checkout not at expected PR HEAD")
    ancestors = {}
    for task, (_, commit) in MERGED.items():
        try:
            ancestors[task] = git(root, "merge-base", "--is-ancestor", commit, expected_head) == b""
        except Refused:
            ancestors[task] = False
    blobs = {}
    for path in TRACKED:
        local = root / path
        require(local.is_file() and not local.is_symlink(), "unsafe or missing tracked file: " + path)
        require(not any(parent.is_symlink() for parent in local.parents if parent != root),
                "linked source parent forbidden: " + path)
        # Preserve blob bytes exactly; never strip whitespace from provenance bytes.
        proc = subprocess.run(["git", "-C", str(root), "show", "HEAD:" + path],
                              capture_output=True, check=False, timeout=30)
        require(proc.returncode == 0 and len(proc.stdout) <= MAX_SOURCE_BYTES, "unavailable tracked blob: " + path)
        require(local.read_bytes() == proc.stdout, "dirty contract source: " + path)
        blobs[path] = proc.stdout
    return make_report(expected_head, actual_head, ancestors, blobs)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("report", "verify"))
    parser.add_argument("--repo", default=".")
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--receipt", required=True)
    args = parser.parse_args()
    try:
        expected = collect(args.repo, args.expected_head)
        receipt = Path(args.receipt)
        if args.action == "report":
            require(not receipt.is_symlink(), "linked output forbidden")
            receipt.write_text(json.dumps(expected, indent=2, sort_keys=True) + "\n", "utf-8")
        else:
            require(receipt.is_file() and not receipt.is_symlink()
                    and receipt.stat().st_size <= MAX_RECEIPT_BYTES, "missing or unsafe receipt")
            validate_receipt(json.loads(receipt.read_text("utf-8")), expected)
        print(json.dumps({"schema": SCHEMA, "source_sha": args.expected_head,
                          "cloud_audit": "PROVISIONAL", "product_release_ready": False}))
    except (Refused, OSError, ValueError, KeyError, TypeError) as exc:
        parser.exit(2, "16F REFUSED: " + str(exc) + "\n")

if __name__ == "__main__":
    main()
