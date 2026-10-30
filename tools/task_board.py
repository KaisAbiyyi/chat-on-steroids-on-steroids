#!/usr/bin/env python3
"""Offline, stdlib-only prerequisite resolver for the CoSoS canonical task DAG.

Readiness is a CANDIDATE from committed registry data, never a remote lock.
Cloud agents MUST inspect GitHub main CI, open PRs, claims and contract overlap
before taking or merging any task. No private profiles/network access required.
"""
from __future__ import annotations

import argparse
import copy
import json
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

REGISTRY = Path(__file__).resolve().parents[1] / "docs" / "task-registry.json"
EXPECTED = {"C": 80, "L": 31, "U": 15}
STATUSES = {"done", "todo", "claimed", "blocked", "hold"}
PHASES = {"1-6", *(str(n) for n in range(7, 17))}
NOTICE = (
    "READY_CANDIDATE ONLY. Check live GitHub main CI, open PRs/claims, "
    "logical/file locks, review and native/user consent before claiming."
)


def load(path: Path = REGISTRY) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def index(data: dict) -> dict[str, dict]:
    return {task["id"]: task for task in data["tasks"]}



FRESH_CHECKS = (
    "fresh_init", "isolated_storage", "synthetic_only",
    "existing_profiles_untouched", "no_leakage", "rollback",
    "failure_isolation", "owned_cleanup",
)

FRESH_VERIFICATION_FIELDS = (
    "receipt_exists",
    "review_exists",
    "artifact_exists",
    "reviewer_identity_verified",
    "reviewer_independent",
    "review_verdict_pass",
    "reviewed_source_matches",
    "receipt_artifact_digest_matches",
    "artifact_provenance_verified",
    "artifact_source_binding_verified",
    "fresh_start_checks_verified",
    "evidence_complete",
)


@dataclass(frozen=True)
class FreshEvidenceVerification:
    """Facts established by a separately trusted external evidence verifier.

    This value must be produced by trusted verifier code, never deserialized
    from task-registry.json or from the owner-local receipt itself.
    """
    receipt_exists: bool
    review_exists: bool
    artifact_exists: bool
    reviewer_identity_verified: bool
    reviewer_independent: bool
    review_verdict_pass: bool
    reviewed_source_matches: bool
    receipt_artifact_digest_matches: bool
    artifact_provenance_verified: bool
    artifact_source_binding_verified: bool
    fresh_start_checks_verified: bool
    evidence_complete: bool
    verified_receipt_url: str
    verified_review_url: str
    verified_source_sha: str
    verified_reviewed_source_sha: str
    verified_artifact_sha256: str
    verified_receipt_sha256: str

    def authoritative_for(self, receipt: dict) -> bool:
        return (
            all(getattr(self, name) is True for name in FRESH_VERIFICATION_FIELDS)
            and self.verified_receipt_url == receipt.get("receipt_url")
            and self.verified_review_url == receipt.get("review_url")
            and self.verified_source_sha == receipt.get("source_sha")
            and self.verified_reviewed_source_sha == receipt.get("reviewed_source_sha")
            and self.verified_artifact_sha256 == receipt.get("artifact_sha256")
            and self.verified_receipt_sha256 == receipt.get("receipt_sha256")
        )


FreshEvidenceVerifier = Callable[[dict], FreshEvidenceVerification]


def fresh_receipt_structurally_valid(receipt: object) -> bool:
    """Validate receipt shape only; this is never an acceptance decision.

    URLs, hashes and PASS strings are user-controlled claims. In particular,
    this function does not establish that a receipt/review exists, identify its
    author, prove reviewer independence, verify artifact bytes or authenticate
    provenance. Those facts belong to a separately trusted verifier.
    """
    if not isinstance(receipt, dict):
        return False

    def sha(value: object, length: int) -> bool:
        return isinstance(value, str) and len(value) == length and all(
            c in "0123456789abcdef" for c in value
        )

    source = receipt.get("source_sha")
    return (
        receipt.get("schema_version") == 1
        and receipt.get("kind") == "owner-local-synthetic-fresh-start"
        and receipt.get("profile_scope") == "new-disposable-empty"
        and receipt.get("migration") == "none"
        and receipt.get("host") == "owner-local-windows"
        and sha(source, 40)
        and sha(receipt.get("artifact_sha256"), 64)
        and sha(receipt.get("receipt_sha256"), 64)
        and isinstance(receipt.get("local_session_id"), str)
        and len(receipt["local_session_id"]) >= 12
        and isinstance(receipt.get("receipt_url"), str)
        and receipt["receipt_url"].startswith(
            "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/"
        )
        and isinstance(receipt.get("review_url"), str)
        and receipt["review_url"].startswith(
            "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/"
        )
        and receipt.get("reviewed_source_sha") == source
        and receipt.get("independent_review_verdict") == "PASS"
        and isinstance(receipt.get("checks"), dict)
        and set(receipt["checks"]) == set(FRESH_CHECKS)
        and all(receipt["checks"][name] == "PASS" for name in FRESH_CHECKS)
    )


def fresh_receipt_status(
    receipt: object,
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> str:
    """Return MISSING, INVALID, PENDING_EXTERNAL_VERIFICATION, REJECTED_EXTERNAL_VERIFICATION,
    VERIFICATION_ERROR, or ACCEPTED.

    No production trust root currently exists in this offline task-board
    module for GitHub review identity, independent-review status, owner-local
    receipt existence, artifact digest/provenance and source binding. Therefore
    the normal CLI supplies no verifier and remains fail-closed. A future
    integration may pass a verifier only when its trust root is pinned outside
    the task registry and it verifies every FreshEvidenceVerification fact.
    """
    if receipt is None:
        return "MISSING"
    if not fresh_receipt_structurally_valid(receipt):
        return "INVALID"
    if trusted_verifier is None:
        return "PENDING_EXTERNAL_VERIFICATION"
    try:
        verification = trusted_verifier(copy.deepcopy(receipt))
    except Exception:
        return "VERIFICATION_ERROR"
    if type(verification) is not FreshEvidenceVerification:
        return "REJECTED_EXTERNAL_VERIFICATION"
    if not verification.authoritative_for(receipt):
        return "REJECTED_EXTERNAL_VERIFICATION"
    return "ACCEPTED"


def private_migration_authorized(tasks: dict[str, dict]) -> bool:
    owner = tasks.get("16U1")
    if not isinstance(owner, dict):
        return False
    consent = owner.get("consent_receipt")
    return (
        owner.get("migration_consent") == "granted"
        and owner.get("owner_decision") == "approved_private_copy"
        and isinstance(consent, dict)
        and consent.get("explicit_owner_approval") is True
        # 16L2 is strictly copy-only; a nonempty but broader scope must
        # never authorize this task.
        and consent.get("scope") == "approved-copy-only"
        and isinstance(consent.get("approval_url"), str)
        and consent["approval_url"].startswith("https://")
    )


def accepted(
    tasks: dict[str, dict],
    task_id: str,
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> bool:
    task = tasks.get(task_id)
    if task is None or task.get("status") != "done":
        return False
    if task_id == "16L2":
        # Consent can authorize starting the copy-only operation, but a
        # user-editable receipt cannot establish physical/reviewer acceptance.
        # No trusted private-migration verifier is integrated in this phase.
        return False
    if task_id == "16LF-01":
        return fresh_receipt_status(
            task.get("evidence"), trusted_verifier=trusted_verifier
        ) == "ACCEPTED"
    return True


def missing_dependencies(
    task: dict,
    tasks: dict[str, dict],
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> list[str]:
    missing = [
        dep for dep in task["depends_on"]
        if not accepted(tasks, dep, trusted_verifier=trusted_verifier)
    ]
    # The private-migration path stays disabled in this phase. Owner consent
    # is necessary but insufficient: no trusted private-migration verifier is
    # integrated, so the task must not become ready or satisfy the release OR.
    if task.get("id") == "16L2":
        if not private_migration_authorized(tasks):
            missing.append("owner_private_migration_consent")
        missing.append("trusted_private_migration_verifier")
    for group in task.get("any_of", []):
        if not any(
            accepted(tasks, dep, trusted_verifier=trusted_verifier)
            for dep in group
        ):
            missing.append("one_of(" + "|".join(group) + ")")
    return missing


def validate(
    data: dict,
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> list[str]:
    problems: list[str] = []
    tasks = data.get("tasks", [])
    if data.get("schema_version") != 1 or not isinstance(tasks, list):
        return ["Invalid schema/version or tasks array"]
    ids = [t.get("id") for t in tasks]
    counts = Counter(ids)
    for task_id, count in counts.items():
        if count != 1:
            problems.append(f"Duplicate task id: {task_id}")
    tasks_by_id = index(data)
    for t in tasks:
        task_id = t.get("id")
        deps = t.get("depends_on")
        locks = t.get("resource_locks")
        if not isinstance(task_id, str) or not task_id:
            problems.append(f"Invalid task id: {task_id!r}")
        if t.get("kind") not in EXPECTED:
            problems.append(f"{task_id}: invalid task kind")
        if t.get("status") not in STATUSES:
            problems.append(f"{task_id}: invalid status")
        if t.get("phase") not in PHASES:
            problems.append(f"{task_id}: invalid phase")
        if not isinstance(deps, list) or len(deps) != len(set(deps)):
            problems.append(f"{task_id}: duplicated or invalid prerequisites")
            deps = []
        if not isinstance(locks, list) or not locks or not all(isinstance(x, str) and x for x in locks):
            problems.append(f"{task_id}: missing resource locks")
        for dep in deps:
            if dep not in tasks_by_id:
                problems.append(f"{task_id}: dangling prerequisite {dep}")
            if dep == task_id:
                problems.append(f"{task_id}: self dependency")
            if t.get("status") == "done" and not accepted(
                tasks_by_id, dep, trusted_verifier=trusted_verifier
            ):
                problems.append(f"{task_id}: cannot be done before {dep}")
    # This OR group is intentional. Every other dependency remains mandatory.
    l3 = tasks_by_id.get("16L3", {})
    if l3.get("depends_on") != ["16F", "16L1"] or l3.get("any_of") != [["16L2", "16LF-01"]]:
        problems.append("16L3: required 16F + 16L1 AND (16L2 OR 16LF-01) gate changed")
    if tasks_by_id.get("16L2", {}).get("status") in {"todo", "claimed", "done"}:
        if not private_migration_authorized(tasks_by_id):
            problems.append(
                "16L2: cannot become ready, claimed, or accepted without explicit owner private-copy consent"
            )
        problems.append(
            "16L2: keep blocked until a trusted private-migration verifier is implemented and configured"
        )
    for protected in ("16L2", "16LF-01"):
        if protected in tasks_by_id and tasks_by_id[protected]["status"] == "done":
            if not accepted(tasks_by_id, protected, trusted_verifier=trusted_verifier):
                problems.append(f"{protected}: missing authorized, reviewed physical evidence")
    for t in tasks:
        groups = t.get("any_of", [])
        if t["id"] != "16L3" and groups:
            problems.append(f"{t['id']}: unexpected OR prerequisite")
        if not isinstance(groups, list):
            problems.append(f"{t['id']}: invalid OR prerequisites")
            continue
        for group in groups:
            if not isinstance(group, list) or len(group) < 2 or len(group) != len(set(group)):
                problems.append(f"{t['id']}: invalid OR group")
                continue
            for dep in group:
                if dep not in tasks_by_id:
                    problems.append(f"{t['id']}: dangling OR prerequisite {dep}")
                elif dep == t["id"]:
                    problems.append(f"{t['id']}: self OR dependency")
        if t["status"] == "done" and missing_dependencies(
            t, tasks_by_id, trusted_verifier=trusted_verifier
        ):
            problems.append(f"{t['id']}: cannot be done before all required release gates")
    color: dict[str, int] = {}
    def visit(name: str, chain: list[str]) -> None:
        if color.get(name) == 2:
            return
        if color.get(name) == 1:
            problems.append("Dependency cycle: " + " -> ".join([*chain, name]))
            return
        if name not in tasks_by_id:
            return
        color[name] = 1
        edges = tasks_by_id[name].get("depends_on", []) + [
            dep for group in tasks_by_id[name].get("any_of", []) for dep in group
        ]
        for dep in edges:
            if dep in tasks_by_id:
                visit(dep, [*chain, name])
        color[name] = 2
    for name in ids:
        if isinstance(name, str):
            visit(name, [])
    if Counter(t.get("kind") for t in tasks) != Counter(EXPECTED):
        problems.append(f"Unexpected task counts: {dict(Counter(t.get('kind') for t in tasks))}")
    for obsolete in ("7P-C", "8D", "8E", "8L2"):
        if obsolete in tasks_by_id:
            problems.append(f"Obsolete runtime task id: {obsolete}")
    expected_edges = {
        "7P-A": ["CL-L6"], "7P-B": ["7P-A"], "7P-L1": ["7P-B"],
        "7A": ["7P-B"], "7L1": ["7C", "7P-L1"], "8A": ["7E"], "8B": ["8A"], "8C": ["8B"],
        "8L1": ["8C"], "8U1": ["8L1"],
        "7U1-PREP": ["7L2"], "7U1": ["7L2", "7U1-PREP"],
        "PERF-00": ["15G"], "PERF-01": ["PERF-00"],
        "PERF-02": ["PERF-00"], "PERF-03": ["PERF-00"], "PERF-04": ["PERF-00"],
        "PERF-05": ["PERF-01", "PERF-02", "PERF-03", "PERF-04", "8L1", "15L3"],
        "PERF-06": ["PERF-05", "16D"],
        "CL-09C": ["CL-09A", "CL-05A"],
        "CL-L3": ["CL-06B", "CL-07B"],
        "CL-10": ["CL-04B", "CL-04C", "CL-L2", "CL-L3", "CL-L4", "CL-L5"],
        "CL-L6": ["CL-10"],
        "16FS-01": ["16U1", "16L1", "16F"],
        "16LF-01": ["16FS-01", "16U1", "16L1"],
        "16U2": ["16L3"],
    }
    for task_id, expected in expected_edges.items():
        if sorted(tasks_by_id.get(task_id, {}).get("depends_on", [])) != sorted(expected):
            problems.append(f"{task_id}: required phase ordering or gate changed")
    for task in tasks:
        if task["phase"] == "8" and task["id"].startswith("7P-"):
            problems.append("Phase 7 preflight cannot be counted as Phase 8")
    return problems


def effective_locks(task: dict) -> set[str]:
    locks = set(task["resource_locks"])
    if task["kind"] == "L":
        locks.add("physical-windows-host")
    if task["kind"] == "U":
        locks.add("human-owner")
    return locks


def readiness(
    data: dict,
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> tuple[list[dict], list[dict]]:
    tasks = index(data)
    ready, blocked = [], []
    for task in data["tasks"]:
        if task["status"] == "done":
            continue
        missing = missing_dependencies(
            task, tasks, trusted_verifier=trusted_verifier
        )
        item = {**task, "missing_prerequisites": missing}
        if task["status"] == "todo" and not missing:
            ready.append(item)
        else:
            blocked.append(item)
    return ready, blocked


def explain(
    data: dict,
    task_id: str,
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> dict:
    tasks = index(data)
    if task_id not in tasks:
        raise KeyError(task_id)
    task = tasks[task_id]
    visited: set[str] = set()
    def blockers(node: str) -> None:
        current = tasks[node]
        for dep in current["depends_on"]:
            if not accepted(tasks, dep, trusted_verifier=trusted_verifier) and dep not in visited:
                visited.add(dep)
                blockers(dep)
        # An OR group is a satisfied prerequisite when any alternative is
        # accepted. Only expose every alternative as a blocker when none is.
        for group in current.get("any_of", []):
            if any(
                accepted(tasks, dep, trusted_verifier=trusted_verifier)
                for dep in group
            ):
                continue
            for dep in group:
                if dep not in visited:
                    visited.add(dep)
                    blockers(dep)
    blockers(task_id)
    missing_direct = missing_dependencies(
        task, tasks, trusted_verifier=trusted_verifier
    )
    return {
        **task,
        "state": "DONE" if accepted(
            tasks, task_id, trusted_verifier=trusted_verifier
        ) else
                 "HOLD" if task["status"] == "hold" else
                 "CLAIMED" if task["status"] == "claimed" else
                 "READY_CANDIDATE" if task["status"] == "todo" and not missing_direct else
                 "BLOCKED",
        "missing_direct": missing_direct,
        "missing_transitive": sorted(visited),
        "fresh_evidence_status": fresh_receipt_status(
            tasks.get("16LF-01", {}).get("evidence"),
            trusted_verifier=trusted_verifier,
        ),
        "effective_locks": sorted(effective_locks(task)),
        "live_claim_check_required": True,
    }


def waves(
    data: dict,
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> list[list[dict]]:
    """Potential independent implementation waves, ignoring live locks and time.
    L and U actions are not performed by this script.
    """
    pending = {t["id"]: t for t in data["tasks"] if t["status"] == "todo"}
    tasks = index(data)
    # Only physically evidenced, accepted local branches may satisfy an OR.
    # Forecasted future work is never evidence for a 16L2 / 16LF-01 gate.
    externally_qualified = {
        task_id for task_id in tasks
        if accepted(tasks, task_id, trusted_verifier=trusted_verifier)
    }
    projected = set(externally_qualified)
    result = []
    while pending:
        candidates = [t for t in pending.values() if
                      all(d in projected for d in t["depends_on"]) and
                      all(any(d in externally_qualified for d in group)
                          for group in t.get("any_of", [])) and
                      t["id"] != "16L2"]
        if not candidates:
            break  # blocked/claimed/stale snapshot, never assume accepted
        # Each wave is a resource-conflict-free possible simultaneous set.
        chosen, held = [], set()
        for task in candidates:
            locks = effective_locks(task)
            if held.isdisjoint(locks):
                chosen.append(task)
                held.update(locks)
        result.append(chosen)
        for task in chosen:
            projected.add(task["id"])
            del pending[task["id"]]
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "ready", "task", "waves"])
    parser.add_argument("task_id", nargs="?")
    parser.add_argument("--json", action="store_true", dest="as_json")
    args = parser.parse_args()
    data = load()
    errors = validate(data)
    if errors:
        for error in errors:
            print("ERROR " + error, file=sys.stderr)
        return 1
    if args.command == "check":
        print(f"VALID: {len(data['tasks'])} nodes; validated AND/OR gates and no dependency cycles")
        return 0
    if args.command == "ready":
        ready, blocked = readiness(data)
        payload = {
            "readiness": "CANDIDATE_NOT_CLAIMED",
            "ready_candidates": [
                {"id": t["id"], "phase": t["phase"], "kind": t["kind"], "locks": sorted(effective_locks(t))}
                for t in ready
            ],
            "blocked_count": len(blocked),
            "live_claim_check_required": True,
            "notice": NOTICE,
        }
    elif args.command == "task":
        if not args.task_id:
            parser.error("task requires a task ID")
        try:
            payload = explain(data, args.task_id)
        except KeyError:
            print("Unknown task ID: " + args.task_id, file=sys.stderr)
            return 2
        payload["notice"] = NOTICE
    else:
        payload = {
            "potential_waves": [[t["id"] for t in wave] for wave in waves(data)],
            "notice": NOTICE,
            "gate_warning": "Wave forecasts are NOT accepted work; L needs real Windows and U needs explicit owner consent.",
        }
    if args.as_json:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    elif args.command == "ready":
        print("READY_CANDIDATE (pending fresh remote claim/lock screening):")
        for task in payload["ready_candidates"]:
            print(f"  {task['id']:<9} phase={task['phase']:<4} kind={task['kind']} locks={','.join(task['locks'])}")
        print(f"BLOCKED / claimed / not yet eligible: {payload['blocked_count']}")
        print(NOTICE)
    elif args.command == "task":
        print(f"{payload['id']}: {payload['state']} | phase {payload['phase']} | {payload['title']}")
        print("Direct prerequisites:", ", ".join(payload["depends_on"]) or "NONE")
        print("Missing direct:", ", ".join(payload["missing_direct"]) or "NONE")
        print("Transitive blockers:", ", ".join(payload["missing_transitive"]) or "NONE")
        print("16LF-01 evidence verification:", payload["fresh_evidence_status"])
        print("Effective locks:", ", ".join(payload["effective_locks"]))
        print(NOTICE)
    else:
        for i, wave in enumerate(payload["potential_waves"], 1):
            print(f"Wave {i:02d}: " + ", ".join(wave))
        print(NOTICE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
