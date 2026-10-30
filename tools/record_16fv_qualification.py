"""Write machine-readable evidence for one exact-HEAD 16FV-01 CI matrix job."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

try:
    from .fresh_evidence_contract import VERIFIER_PR_NUMBER
except ImportError:
    from fresh_evidence_contract import VERIFIER_PR_NUMBER


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--platform", required=True, choices=("Windows", "Linux"))
    parser.add_argument("--status", required=True, choices=("success", "failure", "cancelled"))
    parser.add_argument("--compile-outcome", required=True)
    parser.add_argument("--unit-outcome", required=True)
    parser.add_argument("--dag-outcome", required=True)
    parser.add_argument("--workflow-run-id", required=True, type=int)
    parser.add_argument("--run-attempt", required=True, type=int)
    parser.add_argument("--run-url", required=True)
    parser.add_argument("--event", required=True)
    args = parser.parse_args()

    if re.fullmatch(r"[0-9a-f]{40}", args.source_sha) is None:
        parser.error("source SHA must be a full lowercase Git commit SHA")
    if args.workflow_run_id <= 0 or args.run_attempt <= 0:
        parser.error("workflow run ID and attempt must be positive")
    expected_run_url = f"https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/{args.workflow_run_id}"
    if args.run_url != expected_run_url:
        parser.error("run URL must identify the exact GitHub Actions run ID")
    try:
        actual_head = subprocess.run(
            ["git", "rev-parse", "HEAD"], check=True, text=True, capture_output=True
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as exc:
        parser.error(f"cannot establish checked-out Git HEAD: {exc}")
    if actual_head != args.source_sha:
        parser.error("reported source SHA does not match the actual checked-out Git HEAD")
    verifier_path = Path("tools/fresh_evidence_verifier.py")
    if not verifier_path.is_file():
        parser.error("production verifier source is missing from the tested checkout")
    verifier_sha256 = hashlib.sha256(verifier_path.read_bytes()).hexdigest()
    outcomes = {
        "compile": args.compile_outcome,
        "focused_verifier_tests": args.unit_outcome,
        "task_board_and_dag_tests": args.dag_outcome,
    }
    passed = args.status == "success" and all(value == "success" for value in outcomes.values())
    evidence = {
        "schema": "cosos.16fv01.github-actions-qualification.v1",
        "task": "16FV-01",
        "tested_source_sha": args.source_sha,
        "verifier_pr_number": VERIFIER_PR_NUMBER,
        "verifier_source_sha": args.source_sha,
        "verifier_sha256": verifier_sha256,
        "platform": args.platform,
        "status": "PASS" if passed else "FAIL",
        "workflow_run_id": args.workflow_run_id,
        "run_attempt": args.run_attempt,
        "run_url": args.run_url,
        "event": args.event,
        "checks": outcomes,
        "production_trust_root": "NOT_CONFIGURED_IN_CI",
        "production_acceptance": "NOT_AUTHORIZED_BY_CI",
    }
    Path("16fv-qualification.json").write_text(
        json.dumps(evidence, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
