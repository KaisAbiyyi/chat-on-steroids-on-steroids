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
    raw_tasks = data.get("tasks", []) if isinstance(data, dict) else []
    if not isinstance(raw_tasks, list):
        return {}
    return {
        task["id"]: task
        for task in raw_tasks
        if isinstance(task, dict)
        and isinstance(task.get("id"), str)
        and task["id"]
    }



FRESH_CHECKS = (
    "fresh_init", "isolated_storage", "synthetic_only",
    "existing_profiles_untouched", "no_leakage", "rollback",
    "failure_isolation", "owned_cleanup",
)
FRESH_REVIEW_MODE = "GPT6_OWNER_ATTESTED"
FRESH_RECEIPT_SCHEMA_VERSION = 2

FRESH_VERIFICATION_FIELDS = (
    "receipt_exists",
    "receipt_integrity_verified",
    "review_report_exists",
    "artifact_exists",
    "review_mode_authorized",
    "owner_identity_verified",
    "owner_attestation_exists",
    "owner_attestation_valid",
    "owner_attestation_author_identity_verified",
    "gpt6_review_verdict_pass",
    "reviewed_source_sha_matches",
    "reviewed_diff_digest_matches",
    "receipt_review_artifact_digests_match",
    "artifact_provenance_verified",
    "artifact_source_binding_verified",
    "fresh_start_checks_verified",
    "evidence_complete",
)


def _is_hex_digest(value: object, length: int) -> bool:
    return isinstance(value, str) and len(value) == length and all(
        char in "0123456789abcdef" for char in value
    )


def _is_provider_record_identity(value: object) -> bool:
    """Accept only canonical, provider-native identities supplied by a verifier.

    URLs are locations, not record identities. For GitHub-backed evidence the
    verifier must normalize alternate URL paths/anchors to the same immutable
    provider record identity (for example, github:issue-comment:123).
    """
    return (
        isinstance(value, str)
        and value.startswith("github:")
        and value.count(":") >= 2
        and 1 <= len(value) <= 256
        and value == value.strip()
        and not any(char.isspace() for char in value)
    )


@dataclass(frozen=True)
class FreshEvidenceVerification:
    """Facts established by a separately trusted external evidence verifier.

    GPT6_OWNER_ATTESTED is an owner-attested model-review mode, not a GitHub
    independent reviewer. The verifier must authenticate the owner identity
    and separate owner attestation, and bind that attestation to the exact
    GPT-6 report digest and source revision. Model authorship itself is
    owner-attested, not cryptographically authenticated.

    This value must be produced by trusted verifier code, never deserialized
    from task-registry.json or from the owner-local receipt itself.
    """
    receipt_exists: bool
    receipt_integrity_verified: bool
    review_report_exists: bool
    artifact_exists: bool
    review_mode_authorized: bool
    owner_identity_verified: bool
    owner_attestation_exists: bool
    owner_attestation_valid: bool
    owner_attestation_author_identity_verified: bool
    gpt6_review_verdict_pass: bool
    reviewed_source_sha_matches: bool
    reviewed_diff_digest_matches: bool
    receipt_review_artifact_digests_match: bool
    artifact_provenance_verified: bool
    artifact_source_binding_verified: bool
    fresh_start_checks_verified: bool
    evidence_complete: bool
    verified_review_mode: str
    verified_owner_login: str
    verified_receipt_url: str
    verified_receipt_sha256: str
    verified_review_report_url: str
    verified_review_report_sha256: str
    verified_review_report_verdict: str
    verified_review_report_source_sha: str
    verified_review_report_diff_sha256: str
    verified_review_report_artifact_sha256: str
    verified_owner_attestation_url: str
    verified_receipt_record_identity: str
    verified_review_report_record_identity: str
    verified_owner_attestation_record_identity: str
    verified_owner_attestation_author_login: str
    verified_owner_attestation_sha256: str
    verified_owner_attested_review_report_sha256: str
    verified_owner_attested_source_sha: str
    verified_source_sha: str
    verified_reviewed_source_sha: str
    verified_diff_sha256: str
    verified_reviewed_diff_sha256: str
    verified_artifact_id: int
    verified_artifact_sha256: str
    verified_artifact_source_sha: str
    verified_artifact_provenance_url: str
    verified_artifact_provenance_sha256: str
    verified_fresh_start_checks: tuple[str, ...]

    def authoritative_for(self, receipt: dict) -> bool:
        record_identities = (
            self.verified_receipt_record_identity,
            self.verified_review_report_record_identity,
            self.verified_owner_attestation_record_identity,
        )
        return (
            all(getattr(self, name) is True for name in FRESH_VERIFICATION_FIELDS)
            and self.verified_review_mode == FRESH_REVIEW_MODE
            and self.verified_review_mode == receipt.get("review_mode")
            and self.verified_owner_login == receipt.get("owner_login")
            and isinstance(self.verified_owner_login, str)
            and isinstance(self.verified_owner_attestation_author_login, str)
            and self.verified_owner_attestation_author_login.casefold()
            == self.verified_owner_login.casefold()
            and self.verified_receipt_url == receipt.get("receipt_url")
            and self.verified_receipt_sha256 == receipt.get("receipt_sha256")
            and self.verified_review_report_url == receipt.get("review_report_url")
            and self.verified_review_report_sha256 == receipt.get("review_report_sha256")
            and self.verified_owner_attestation_sha256 == receipt.get("owner_attestation_sha256")
            and self.verified_review_report_verdict == "PASS"
            and self.verified_review_report_source_sha == receipt.get("source_sha")
            and self.verified_review_report_diff_sha256 == receipt.get("diff_sha256")
            and self.verified_review_report_artifact_sha256 == self.verified_artifact_sha256
            and self.verified_owner_attestation_url == receipt.get("owner_attestation_url")
            and all(_is_provider_record_identity(identity) for identity in record_identities)
            and len(set(record_identities)) == 3
            and self.verified_owner_attested_review_report_sha256 == self.verified_review_report_sha256
            and self.verified_owner_attested_source_sha == receipt.get("source_sha")
            and self.verified_source_sha == receipt.get("source_sha")
            and self.verified_reviewed_source_sha == receipt.get("reviewed_source_sha")
            and self.verified_diff_sha256 == receipt.get("diff_sha256")
            and self.verified_reviewed_diff_sha256 == receipt.get("reviewed_diff_sha256")
            and self.verified_artifact_id == receipt.get("artifact_id")
            and self.verified_artifact_sha256 == receipt.get("artifact_sha256")
            and self.verified_artifact_source_sha == receipt.get("artifact_source_sha")
            and self.verified_artifact_provenance_url == receipt.get("artifact_provenance_url")
            and self.verified_artifact_provenance_sha256 == receipt.get("artifact_provenance_sha256")
            and self.verified_fresh_start_checks == FRESH_CHECKS
            and _is_hex_digest(self.verified_receipt_sha256, 64)
            and _is_hex_digest(self.verified_review_report_sha256, 64)
            and _is_hex_digest(self.verified_owner_attestation_sha256, 64)
            and _is_hex_digest(self.verified_artifact_sha256, 64)
            and _is_hex_digest(self.verified_artifact_provenance_sha256, 64)
            and _is_hex_digest(self.verified_source_sha, 40)
            and _is_hex_digest(self.verified_reviewed_source_sha, 40)
            and _is_hex_digest(self.verified_owner_attested_source_sha, 40)
            and _is_hex_digest(self.verified_artifact_source_sha, 40)
            and _is_hex_digest(self.verified_diff_sha256, 64)
            and _is_hex_digest(self.verified_reviewed_diff_sha256, 64)
            and type(self.verified_artifact_id) is int
            and self.verified_artifact_id > 0
        )


FreshEvidenceVerifier = Callable[[dict], FreshEvidenceVerification]


def fresh_receipt_structurally_valid(receipt: object) -> bool:
    """Validate receipt shape only; this is never an acceptance decision.

    Receipt URLs, hashes, review mode, owner login and PASS checks are
    self-reported claims. Only a separately trusted verifier can establish
    actual receipt/report/attestation/artifact existence, identity, verdict,
    exact source/diff/digest bindings, provenance and physical checks.
    """
    if not isinstance(receipt, dict):
        return False

    def repo_url(value: object) -> bool:
        return (
            isinstance(value, str)
            and value.startswith(
                "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/"
            )
            and not any(char.isspace() for char in value)
        )

    source = receipt.get("source_sha")
    diff_digest = receipt.get("diff_sha256")
    owner_login = receipt.get("owner_login")
    artifact_id = receipt.get("artifact_id")
    return (
        type(receipt.get("schema_version")) is int
        and receipt["schema_version"] == FRESH_RECEIPT_SCHEMA_VERSION
        and receipt.get("kind") == "owner-local-synthetic-fresh-start"
        and receipt.get("review_mode") == FRESH_REVIEW_MODE
        and isinstance(owner_login, str)
        and 1 <= len(owner_login) <= 39
        and not any(char.isspace() for char in owner_login)
        and receipt.get("profile_scope") == "new-disposable-empty"
        and receipt.get("migration") == "none"
        and receipt.get("host") == "owner-local-windows"
        and _is_hex_digest(source, 40)
        and receipt.get("reviewed_source_sha") == source
        and _is_hex_digest(diff_digest, 64)
        and receipt.get("reviewed_diff_sha256") == diff_digest
        and type(artifact_id) is int
        and artifact_id > 0
        and _is_hex_digest(receipt.get("artifact_sha256"), 64)
        and _is_hex_digest(receipt.get("artifact_source_sha"), 40)
        and repo_url(receipt.get("artifact_provenance_url"))
        and _is_hex_digest(receipt.get("artifact_provenance_sha256"), 64)
        and _is_hex_digest(receipt.get("review_report_sha256"), 64)
        and _is_hex_digest(receipt.get("owner_attestation_sha256"), 64)
        and _is_hex_digest(receipt.get("receipt_sha256"), 64)
        and isinstance(receipt.get("local_session_id"), str)
        and len(receipt["local_session_id"]) >= 12
        and repo_url(receipt.get("receipt_url"))
        and repo_url(receipt.get("review_report_url"))
        and repo_url(receipt.get("owner_attestation_url"))
        and "independent_review_verdict" not in receipt
        and "reviewer_independent" not in receipt
        and isinstance(receipt.get("checks"), dict)
        and set(receipt["checks"]) == set(FRESH_CHECKS)
        and all(receipt["checks"][name] == "PASS" for name in FRESH_CHECKS)
    )


def fresh_receipt_status(
    receipt: object,
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> str:
    """Return MISSING, INVALID, PENDING_EXTERNAL_VERIFICATION,
    REJECTED_EXTERNAL_VERIFICATION, VERIFICATION_ERROR, or ACCEPTED.

    The only accepted review mode in this contract is GPT6_OWNER_ATTESTED.
    The future 16FV-01 verifier must use an owner-controlled trust root to
    authenticate the owner identity and separate attestation, verify the
    actual GPT-6 report's PASS verdict and exact source/diff bindings, and
    check receipt/report/artifact digests plus 16L1 provenance. This offline
    task-board module deliberately has no production trust root or verifier.
    Its normal CLI supplies none, so it remains fail-closed. A test-only
    verifier double is not production authority.
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
        # User-editable receipt fields cannot establish physical acceptance.
        # No trusted private-migration verifier is integrated in this phase.
        return False
    if task_id == "16LF-01":
        return fresh_receipt_status(
            task.get("evidence"), trusted_verifier=trusted_verifier
        ) == "ACCEPTED"
    return True


def _validated_and_dependencies(
    task: dict,
    tasks: dict[str, dict],
) -> tuple[list[str], bool]:
    raw = task.get("depends_on")
    if not isinstance(raw, list):
        return [], False
    if any(not isinstance(dep, str) or not dep for dep in raw):
        return [], False
    if len(raw) != len(set(raw)):
        return [], False
    if any(dep not in tasks or dep == task.get("id") for dep in raw):
        return [], False
    return list(raw), True


def _validated_or_groups(
    task: dict,
    tasks: dict[str, dict],
) -> tuple[list[list[str]], bool]:
    raw_groups = task.get("any_of", [])
    if not isinstance(raw_groups, list):
        return [], False

    validated: list[list[str]] = []
    all_valid = True
    for group in raw_groups:
        if not isinstance(group, list) or len(group) < 2:
            all_valid = False
            continue
        # Check types before set()/membership operations: malformed JSON may
        # contain nulls, nested arrays or objects that are not hashable IDs.
        if any(not isinstance(dep, str) or not dep for dep in group):
            all_valid = False
            continue
        if len(group) != len(set(group)):
            all_valid = False
            continue
        if any(dep not in tasks or dep == task.get("id") for dep in group):
            all_valid = False
            continue
        validated.append(list(group))
    return validated, all_valid


def missing_dependencies(
    task: dict,
    tasks: dict[str, dict],
    *,
    trusted_verifier: FreshEvidenceVerifier | None = None,
) -> list[str]:
    missing: list[str] = []
    dependencies, dependencies_valid = _validated_and_dependencies(task, tasks)
    if not dependencies_valid:
        missing.append("invalid_prerequisites")
    else:
        missing.extend(
            dep for dep in dependencies
            if not accepted(tasks, dep, trusted_verifier=trusted_verifier)
        )

    # The private-migration path stays disabled in this phase. Owner consent
    # is necessary but insufficient: no trusted private-migration verifier is
    # integrated, so the task must not become ready or satisfy the release OR.
    if task.get("id") == "16L2":
        if not private_migration_authorized(tasks):
            missing.append("owner_private_migration_consent")
        missing.append("trusted_private_migration_verifier")

    groups, groups_valid = _validated_or_groups(task, tasks)
    if not groups_valid:
        missing.append("invalid_or_prerequisites")
    for group in groups:
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
    if not isinstance(data, dict):
        return ["Invalid schema/version or tasks array"]
    tasks = data.get("tasks", [])
    if data.get("schema_version") != 1 or not isinstance(tasks, list):
        return ["Invalid schema/version or tasks array"]

    problems: list[str] = []
    task_rows: list[dict] = []
    ids: list[str] = []
    for position, task in enumerate(tasks):
        if not isinstance(task, dict):
            problems.append(f"Invalid task entry at index {position}")
            continue
        task_rows.append(task)
        task_id = task.get("id")
        if not isinstance(task_id, str) or not task_id:
            problems.append(f"Invalid task id: {task_id!r}")
        else:
            ids.append(task_id)

    counts = Counter(ids)
    for task_id, count in counts.items():
        if count != 1:
            problems.append(f"Duplicate task id: {task_id}")

    tasks_by_id = index(data)
    validated_and_by_id: dict[str, list[str]] = {}
    validated_or_by_id: dict[str, list[list[str]]] = {}

    for task in task_rows:
        task_id = task.get("id")
        if not isinstance(task_id, str) or not task_id:
            continue

        kind = task.get("kind")
        status = task.get("status")
        phase = task.get("phase")
        if not isinstance(kind, str) or kind not in EXPECTED:
            problems.append(f"{task_id}: invalid task kind")
        if not isinstance(status, str) or status not in STATUSES:
            problems.append(f"{task_id}: invalid status")
        if not isinstance(phase, str) or phase not in PHASES:
            problems.append(f"{task_id}: invalid phase")

        locks = task.get("resource_locks")
        if not isinstance(locks, list) or not locks or not all(
            isinstance(lock, str) and lock for lock in locks
        ):
            problems.append(f"{task_id}: missing resource locks")

        raw_dependencies = task.get("depends_on")
        dependencies_valid = (
            isinstance(raw_dependencies, list)
            and all(isinstance(dep, str) and dep for dep in raw_dependencies)
        )
        if dependencies_valid:
            dependencies_valid = len(raw_dependencies) == len(set(raw_dependencies))
        if not dependencies_valid:
            problems.append(f"{task_id}: duplicated or invalid prerequisites")

        validated_dependencies: list[str] = []
        for dependency in raw_dependencies if isinstance(raw_dependencies, list) else []:
            if not isinstance(dependency, str) or not dependency:
                continue
            if dependency not in tasks_by_id:
                problems.append(f"{task_id}: dangling prerequisite {dependency}")
                dependencies_valid = False
                continue
            if dependency == task_id:
                problems.append(f"{task_id}: self dependency")
                dependencies_valid = False
                continue
            validated_dependencies.append(dependency)
            if status == "done" and not accepted(
                tasks_by_id, dependency, trusted_verifier=trusted_verifier
            ):
                problems.append(f"{task_id}: cannot be done before {dependency}")
        validated_and_by_id[task_id] = (
            validated_dependencies if dependencies_valid else []
        )

    # This OR group is intentional. Every other dependency remains mandatory.
    l3 = tasks_by_id.get("16L3", {})
    if (
        l3.get("depends_on") != ["16F", "16L1"]
        or l3.get("any_of") != [["16L2", "16LF-01"]]
    ):
        problems.append("16L3: required 16F + 16L1 AND (16L2 OR 16LF-01) gate changed")

    owner_task = tasks_by_id.get("16U1", {})
    private_task = tasks_by_id.get("16L2", {})
    if owner_task.get("migration_consent") == "declined":
        if owner_task.get("owner_decision") != "fresh_start":
            problems.append(
                "16U1: declined private migration must preserve the explicit fresh_start owner decision"
            )
        if private_task.get("private_migration_status") != "NOT_RUN / NOT_AUTHORIZED":
            problems.append(
                "16L2: owner-declined migration status must remain NOT_RUN / NOT_AUTHORIZED"
            )

    private_status = private_task.get("status")
    if isinstance(private_status, str) and private_status in {"todo", "claimed", "done"}:
        if not private_migration_authorized(tasks_by_id):
            problems.append(
                "16L2: cannot become ready, claimed, or accepted without explicit owner private-copy consent"
            )
        problems.append(
            "16L2: keep blocked until a trusted private-migration verifier is implemented and configured"
        )

    for protected in ("16L2", "16LF-01"):
        protected_task = tasks_by_id.get(protected)
        if isinstance(protected_task, dict) and protected_task.get("status") == "done":
            if not accepted(tasks_by_id, protected, trusted_verifier=trusted_verifier):
                problems.append(f"{protected}: missing authorized, reviewed physical evidence")

    for task in task_rows:
        task_id = task.get("id")
        if not isinstance(task_id, str) or not task_id:
            continue
        groups = task.get("any_of", [])
        if task_id != "16L3" and groups:
            problems.append(f"{task_id}: unexpected OR prerequisite")
        if not isinstance(groups, list):
            problems.append(f"{task_id}: invalid OR prerequisites")
            validated_or_by_id[task_id] = []
            continue

        validated_groups: list[list[str]] = []
        for group in groups:
            if not isinstance(group, list) or len(group) < 2:
                problems.append(f"{task_id}: invalid OR group")
                continue
            if any(not isinstance(dep, str) or not dep for dep in group):
                problems.append(f"{task_id}: invalid OR group")
                continue
            if len(group) != len(set(group)):
                problems.append(f"{task_id}: invalid OR group")
                continue

            group_valid = True
            for dependency in group:
                if dependency not in tasks_by_id:
                    problems.append(
                        f"{task_id}: dangling OR prerequisite {dependency}"
                    )
                    group_valid = False
                elif dependency == task_id:
                    problems.append(f"{task_id}: self OR dependency")
                    group_valid = False
            if group_valid:
                validated_groups.append(list(group))

        # Cycle traversal below receives only fully validated OR groups.
        validated_or_by_id[task_id] = validated_groups
        if task.get("status") == "done" and missing_dependencies(
            task, tasks_by_id, trusted_verifier=trusted_verifier
        ):
            problems.append(f"{task_id}: cannot be done before all required release gates")

    kind_counts = Counter(
        task.get("kind")
        for task in task_rows
        if isinstance(task.get("kind"), str)
    )
    if kind_counts != Counter(EXPECTED):
        problems.append(f"Unexpected task counts: {dict(kind_counts)}")
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
        if sorted(validated_and_by_id.get(task_id, [])) != sorted(expected):
            problems.append(f"{task_id}: required phase ordering or gate changed")

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
        edges = [
            *validated_and_by_id.get(name, []),
            *(
                dependency
                for group in validated_or_by_id.get(name, [])
                for dependency in group
            ),
        ]
        for dependency in edges:
            if dependency in tasks_by_id:
                visit(dependency, [*chain, name])
        color[name] = 2

    for name in ids:
        if name in tasks_by_id:
            visit(name, [])

    for task in task_rows:
        task_id = task.get("id")
        if (
            task.get("phase") == "8"
            and isinstance(task_id, str)
            and task_id.startswith("7P-")
        ):
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
    raw_tasks = data.get("tasks", []) if isinstance(data, dict) else []
    if not isinstance(raw_tasks, list):
        return [], []
    tasks = index(data)
    ready: list[dict] = []
    blocked: list[dict] = []
    for task in raw_tasks:
        if not isinstance(task, dict):
            continue
        if task.get("status") == "done":
            continue
        missing = missing_dependencies(
            task, tasks, trusted_verifier=trusted_verifier
        )
        item = {**task, "missing_prerequisites": missing}
        if task.get("status") == "todo" and not missing:
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
        current = tasks.get(node)
        if not isinstance(current, dict):
            return
        dependencies, dependencies_valid = _validated_and_dependencies(
            current, tasks
        )
        if not dependencies_valid:
            return
        for dep in dependencies:
            if not accepted(tasks, dep, trusted_verifier=trusted_verifier) and dep not in visited:
                visited.add(dep)
                blockers(dep)
        # An OR group is a satisfied prerequisite when any alternative is
        # accepted. Malformed groups are not traversed or treated as gates.
        groups, groups_valid = _validated_or_groups(current, tasks)
        if not groups_valid:
            return
        for group in groups:
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

    Malformed dependency graphs are never projected into executable waves.
    L and U actions are not performed by this script.
    """
    raw_tasks = data.get("tasks", []) if isinstance(data, dict) else []
    if not isinstance(raw_tasks, list):
        return []
    tasks = index(data)
    if len(tasks) != len(raw_tasks):
        return []

    normalized: dict[str, tuple[list[str], list[list[str]]]] = {}
    for task in raw_tasks:
        if not isinstance(task, dict):
            return []
        task_id = task.get("id")
        if not isinstance(task_id, str) or not task_id:
            return []
        dependencies, dependencies_valid = _validated_and_dependencies(
            task, tasks
        )
        groups, groups_valid = _validated_or_groups(task, tasks)
        locks = task.get("resource_locks")
        if (
            not dependencies_valid
            or not groups_valid
            or not isinstance(locks, list)
            or not locks
            or not all(isinstance(lock, str) and lock for lock in locks)
        ):
            return []
        normalized[task_id] = (dependencies, groups)

    pending = {
        task["id"]: task for task in raw_tasks if task.get("status") == "todo"
    }
    # Only physically evidenced, accepted local branches may satisfy an OR.
    # Forecasted future work is never evidence for a 16L2 / 16LF-01 gate.
    externally_qualified = {
        task_id for task_id in tasks
        if accepted(tasks, task_id, trusted_verifier=trusted_verifier)
    }
    projected = set(externally_qualified)
    result: list[list[dict]] = []
    while pending:
        candidates = [
            task for task in pending.values()
            if all(
                dependency in projected
                for dependency in normalized[task["id"]][0]
            )
            and all(
                any(dependency in externally_qualified for dependency in group)
                for group in normalized[task["id"]][1]
            )
            and task["id"] != "16L2"
        ]
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
