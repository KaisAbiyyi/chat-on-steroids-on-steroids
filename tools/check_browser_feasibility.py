"""Fail-closed 7P-A browser architecture preflight contract validator.

Static vendor documentation and cloud tests are *not* native browser proof.
This deliberately cannot emit PASS for 7P-L1 or Phase 7A.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "docs" / "architecture" / "7p-a-feasibility-gates.json"

REQUIRED = {
    "extension_install",
    "extension_worlds",
    "mv3_lifecycle",
    "debugger_target",
    "extension_debugger_parity",
    "tab_profile_isolation",
    "browser_navigation",
    "bridge_auth_identity",
    "sign_in_constraints",
    "rich_editor",
    "pdf_math_terminal",
    "distribution_budgets",
}
NATIVE_CRITICAL = {
    "mv3_lifecycle",
    "extension_debugger_parity",
    "tab_profile_isolation",
    "browser_navigation",
    "bridge_auth_identity",
    "sign_in_constraints",
    "distribution_budgets",
}
OWNERS = {"browser-host", "extension", "bridge", "renderer", "release"}
STAGES = {"7P-B", "7P-L1"}
EXPECTED_PATHS = (
    ROOT / "docs" / "architecture" / "7p-a-browser-architecture.md",
    ROOT / "docs" / "implementation-plan" / "2026-10-18-cloud-first-units.md",
)


def validate(document: dict) -> list[str]:
    """Validate the *planning* contract without granting device qualification."""
    errors: list[str] = []
    if not isinstance(document, dict):
        return ["manifest must be a JSON object"]
    for name, expected in {
        "schema_version": 1,
        "task_id": "7P-A",
        "decision": "PROVISIONAL",
        "selected_architecture": "windows-webview2-rust-host-hybrid-renderer",
        "prereq": "CL-L6",
        "phase7_bridge_guard": "7P-L1",
    }.items():
        if document.get(name) != expected:
            errors.append(f"{name}: expected {expected!r}")
    if document.get("supported_for_probe") != ["windows-x64"]:
        errors.append("Windows x64 must be the explicit active native target")
    if set(document.get("deferred_targets", [])) != {"macos", "linux"}:
        errors.append("deferred platforms must be visible and not silently dropped")
    if document.get("followup") != ["7P-B", "7P-L1"]:
        errors.append("preflight order 7P-B -> 7P-L1 cannot be skipped")

    policy = document.get("policy", {})
    if not isinstance(policy, dict) or (
        policy.get("native_review_required") is not True
        or policy.get("prevent_cloud_pass") is not True
        or policy.get("allow_user_private_profiles") is not False
        or policy.get("allow_automatic_send") is not False
        or "javascript" not in policy.get("extension_glue_language", "").lower()
        or "typescript" not in policy.get("renderer_language", "").lower()
    ):
        errors.append("explicit native-review, no-private-action and hybrid language policy required")

    options = document.get("options", [])
    if not isinstance(options, list) or len(options) < 3:
        errors.append("architecture alternatives and a rejected silent fallback are required")
    else:
        identities = [o.get("id") for o in options if isinstance(o, dict)]
        if len(set(identities)) != len(identities):
            errors.append("duplicate architecture option")
        selected = [o for o in options if isinstance(o, dict)
                    and o.get("id") == document.get("selected_architecture")]
        if len(selected) != 1 or selected[0].get("status") != "PROVISIONAL":
            errors.append("selected architecture must be exactly one PROVISIONAL option")
        rejected = {o.get("id"): o.get("status") for o in options if isinstance(o, dict)}
        if rejected.get("external-browser-only") != "REJECTED_AS_PARITY_REPLACEMENT":
            errors.append("silent external-only browser fallback must be rejected")

    caps = document.get("capabilities", [])
    if not isinstance(caps, list):
        return errors + ["capabilities must be a list"]
    found: dict[str, int] = {}
    for i, cap in enumerate(caps):
        where = f"capabilities[{i}]"
        if not isinstance(cap, dict):
            errors.append(f"{where}: must be object")
            continue
        ident = cap.get("id")
        if not isinstance(ident, str):
            errors.append(f"{where}: id must be string")
            continue
        found[ident] = found.get(ident, 0) + 1
        if cap.get("hard_blocker") is not True:
            errors.append(f"{ident}: all critical preflight capabilities must block")
        if cap.get("verification") != "UNVERIFIED":
            errors.append(f"{ident}: cloud preflight cannot grant native/browser PASS")
        if cap.get("owner") not in OWNERS:
            errors.append(f"{ident}: missing/unknown logical owner")
        if cap.get("first_probe") not in STAGES:
            errors.append(f"{ident}: first probe must be assigned to 7P-B or 7P-L1")
        if ident in NATIVE_CRITICAL and cap.get("first_probe") != "7P-L1":
            errors.append(f"{ident}: cannot reduce native-critical probe to cloud")
        if not isinstance(cap.get("probe"), str) or len(cap["probe"].strip()) < 35:
            errors.append(f"{ident}: needs reproducible probe and negative case")
        refs = cap.get("sources")
        if not isinstance(refs, list) or not refs:
            errors.append(f"{ident}: missing evidence pointers")
        else:
            for ref in refs:
                parsed = urlparse(ref) if isinstance(ref, str) else None
                if not parsed or parsed.scheme != "https" or not parsed.netloc:
                    errors.append(f"{ident}: invalid/unsafe source reference")

    missing = REQUIRED - found.keys()
    additional = found.keys() - REQUIRED
    if missing:
        errors.append("missing capabilities: " + ", ".join(sorted(missing)))
    if additional:
        errors.append("unreviewed capabilities: " + ", ".join(sorted(additional)))
    for ident, count in found.items():
        if count != 1:
            errors.append(f"{ident}: duplicate capability id")

    bans = document.get("must_not_claim", [])
    if not isinstance(bans, list) or not any("Native Windows" in x for x in bans):
        errors.append("native Windows not-verified disclaimer required")
    if not any("Rust-only" in x for x in bans):
        errors.append("hybrid/non-Rust exception must remain visible")
    if any(not path.is_file() for path in EXPECTED_PATHS):
        errors.append("required source-backed architecture or phase contract missing")
    return errors


def load(path: Path = DEFAULT) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        data = load(args.manifest)
        errors = validate(data)
    except (OSError, ValueError) as exc:
        errors = [f"cannot parse manifest: {exc}"]
    result = {
        "task": "7P-A",
        "contract": "PASS" if not errors else "FAIL",
        "architecture": "PROVISIONAL",
        "native_7P_L1": "NOT_RUN",
        "browser_runtime": "NOT_RUN",
        "errors": errors,
    }
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"7P-A contract: {result['contract']}; native browser: NOT RUN")
        for error in errors:
            print(f"  FAIL: {error}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
