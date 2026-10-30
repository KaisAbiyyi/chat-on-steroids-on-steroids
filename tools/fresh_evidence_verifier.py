"""Production verifier for the 16LF-01 owner-attested fresh-start evidence contract.

Production authority is loaded only from COSOS_FRESH_EVIDENCE_TRUST_ROOT_JSON.
Task-registry fields and test fixtures are never production trust roots.
"""
from __future__ import annotations

import base64
import hashlib
import ntpath
from datetime import datetime
import io
import json
import os
import re
import stat
import subprocess
import tempfile
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Callable
from urllib.parse import quote, urlparse
from urllib.request import Request, urlopen

try:
    from . import task_board as board
except ImportError:
    import task_board as board

try:
    from .fresh_evidence_contract import (
        HISTORICAL_16L1_PR_NUMBER, HISTORICAL_TOOLING_PR_NUMBER,
        OWNER_LOGIN, OWNER_USER_ID, PREDECESSOR_PR_NUMBER,
        PRODUCTION_WORKFLOW_PATH, RECEIPT_ISSUE_NUMBER, REPOSITORY,
        REPOSITORY_ID, REVIEW_MODE, TRUST_ROOT_ENV, TRUST_ROOT_SCHEMA,
        VERIFIER_ATTESTATION_MARKER, VERIFIER_ATTESTATION_SCHEMA,
        VERIFIER_REVIEW_MARKER, VERIFIER_REVIEW_SCHEMA, VERIFIER_PR_NUMBER,
    )
except ImportError:
    from fresh_evidence_contract import (
        HISTORICAL_16L1_PR_NUMBER, HISTORICAL_TOOLING_PR_NUMBER,
        OWNER_LOGIN, OWNER_USER_ID, PREDECESSOR_PR_NUMBER,
        PRODUCTION_WORKFLOW_PATH, RECEIPT_ISSUE_NUMBER, REPOSITORY,
        REPOSITORY_ID, REVIEW_MODE, TRUST_ROOT_ENV, TRUST_ROOT_SCHEMA,
        VERIFIER_ATTESTATION_MARKER, VERIFIER_ATTESTATION_SCHEMA,
        VERIFIER_REVIEW_MARKER, VERIFIER_REVIEW_SCHEMA, VERIFIER_PR_NUMBER,
    )
RECEIPT_MARKER = "<!-- COSOS-16LF01-RECEIPT-V1 -->"
REVIEW_MARKER = "<!-- COSOS-GPT6-REVIEW-V2 -->"
ATTESTATION_MARKER = "<!-- COSOS-OWNER-ATTESTATION-V1 -->"
REVIEW_SCHEMA = "cosos.release.cloud-provenance.v2"
RECEIPT_SCHEMA = "cosos.freshstart.receipt.v1"
ATTESTATION_SCHEMA = "cosos.freshstart.owner-attestation.v1"
BUNDLE_SCHEMA = "cosos.16lf01.evidence-bundle.v1"
QUALIFICATION_SCHEMA = "cosos.16lf01.qualification.v1"
CHECK_EVIDENCE_SCHEMA = "cosos.16lf01.check-evidence.v1"
MAX_ARTIFACT_BYTES = 200 * 1024 * 1024
MAX_BUNDLE_BYTES = 100 * 1024 * 1024
MAX_UNPACKED_BYTES = 500 * 1024 * 1024

PREDECESSOR_TRUST_ROOT_FIELDS = frozenset({
    "pr_number", "source_sha", "merge_commit_sha", "diff_sha256",
    "report_comment_id", "owner_attestation_comment_id", "report_sha256",
})
ARTIFACT_TRUST_ROOT_FIELDS = frozenset({
    "pr_number", "source_sha", "source_base_sha", "source_merge_commit_sha",
    "branch", "original_event_sha", "run_id", "run_attempt", "workflow_id",
    "workflow_path", "artifact_id", "artifact_name", "archive_sha256",
    "installer_sha256", "package_receipt_sha256", "candidate_zip_sha256",
    "update02_setup_sha256", "update03_setup_sha256", "update02_zip_sha256",
    "update03_zip_sha256", "release_manifest_sha256", "renderer_manifest_sha256",
    "rust_sha256", "renderer_host_sha256", "renderer_dll_sha256",
    "tooling_pr_number", "tooling_head_sha", "tooling_merge_commit_sha",
})
ARTIFACT_COMMIT_FIELDS = (
    "source_sha", "source_base_sha", "source_merge_commit_sha",
    "original_event_sha", "tooling_head_sha", "tooling_merge_commit_sha",
)
ARTIFACT_DIGEST_FIELDS = (
    "archive_sha256", "installer_sha256", "package_receipt_sha256",
    "candidate_zip_sha256", "update02_setup_sha256", "update03_setup_sha256",
    "update02_zip_sha256", "update03_zip_sha256", "release_manifest_sha256",
    "renderer_manifest_sha256", "rust_sha256", "renderer_host_sha256",
    "renderer_dll_sha256",
)
ARTIFACT_INTEGER_FIELDS = (
    "pr_number", "run_id", "run_attempt", "workflow_id", "artifact_id",
    "tooling_pr_number",
)
RECEIPT_PAYLOAD_FIELDS = (
    "schema_version", "kind", "host", "profile_scope", "migration",
    "review_mode", "owner_login", "repository", "repository_id",
    "review_pr_number", "source_sha", "reviewed_source_sha", "diff_sha256",
    "reviewed_diff_sha256", "contract_source_sha", "contract_merge_commit_sha",
    "verifier_pr_number", "verifier_source_sha", "verifier_merge_commit_sha",
    "verifier_sha256", "artifact_id", "artifact_sha256", "artifact_source_sha",
    "artifact_provenance_url", "artifact_provenance_sha256",
    "evidence_bundle_url", "evidence_bundle_sha256", "windows_build",
    "local_session_id", "workspace_nonce", "workspace_path",
    "started_at", "finished_at", "commands", "checks", "check_evidence",
)


class VerificationError(RuntimeError):
    """Evidence is missing, inconsistent, unauthenticated or unverifiable."""


class TrustRootError(VerificationError):
    """Owner-controlled trust-root configuration is absent or invalid."""


@dataclass(frozen=True)
class TrustRoot:
    authority: str
    repository_full_name: str
    repository_id: int
    owner_login: str
    owner_user_id: int
    review_mode: str
    receipt_issue_number: int
    predecessor: dict[str, Any]
    verifier_review: dict[str, Any]
    artifact: dict[str, Any]

    @classmethod
    def from_mapping(cls, value: object, *, production: bool) -> "TrustRoot":
        if not isinstance(value, dict):
            raise TrustRootError("TRUST_ROOT_INVALID: root must be a JSON object")
        expected_keys = {
            "schema", "authority", "repository_full_name", "repository_id",
            "owner_login", "owner_user_id", "review_mode",
            "receipt_issue_number", "predecessor", "verifier_review", "artifact",
        }
        if set(value) != expected_keys or value.get("schema") != TRUST_ROOT_SCHEMA:
            raise TrustRootError("TRUST_ROOT_INVALID: schema or fields mismatch")
        expected_authority = "owner-controlled" if production else "test-only"
        if value.get("authority") != expected_authority:
            raise TrustRootError("TRUST_ROOT_INVALID: trust-root authority does not match the execution mode")

        # Identity anchors are fixed; historical pins are supplied independently
        # by the owner-controlled payload and verified against actual GitHub data.
        if (
            value.get("repository_full_name") != REPOSITORY
            or type(value.get("repository_id")) is not int
            or value.get("repository_id") != REPOSITORY_ID
            or value.get("owner_login") != OWNER_LOGIN
            or type(value.get("owner_user_id")) is not int
            or value.get("owner_user_id") != OWNER_USER_ID
            or value.get("review_mode") != REVIEW_MODE
            or type(value.get("receipt_issue_number")) is not int
            or value.get("receipt_issue_number") != RECEIPT_ISSUE_NUMBER
        ):
            raise TrustRootError("TRUST_ROOT_INVALID: repository, owner, receipt or review-policy identity mismatch")

        predecessor = value.get("predecessor")
        if (
            not isinstance(predecessor, dict)
            or set(predecessor) != PREDECESSOR_TRUST_ROOT_FIELDS
            or type(predecessor.get("pr_number")) is not int
            or predecessor.get("pr_number") != PREDECESSOR_PR_NUMBER
            or type(predecessor.get("report_comment_id")) is not int
            or predecessor.get("report_comment_id") <= 0
            or type(predecessor.get("owner_attestation_comment_id")) is not int
            or predecessor.get("owner_attestation_comment_id") <= 0
            or any(
                not isinstance(predecessor.get(field), str)
                or re.fullmatch(rf"[0-9a-f]{{{length}}}", predecessor[field]) is None
                for field, length in (
                    ("source_sha", 40), ("merge_commit_sha", 40),
                    ("diff_sha256", 64), ("report_sha256", 64),
                )
            )
        ):
            raise TrustRootError("TRUST_ROOT_INVALID: predecessor contract pins are malformed")


        verifier_review = value.get("verifier_review")
        if (
            not isinstance(verifier_review, dict)
            or set(verifier_review) != {
                "pr_number", "source_sha", "merge_commit_sha", "diff_sha256",
                "report_comment_id", "report_sha256", "owner_attestation_comment_id",
                "owner_attestation_sha256", "review_mode",
            }
            or type(verifier_review.get("pr_number")) is not int
            or verifier_review.get("pr_number") != VERIFIER_PR_NUMBER
            or type(verifier_review.get("report_comment_id")) is not int
            or verifier_review.get("report_comment_id") <= 0
            or type(verifier_review.get("owner_attestation_comment_id")) is not int
            or verifier_review.get("owner_attestation_comment_id") <= 0
            or verifier_review.get("report_comment_id") == verifier_review.get("owner_attestation_comment_id")
            or verifier_review.get("review_mode") != REVIEW_MODE
            or any(
                not isinstance(verifier_review.get(field), str)
                or re.fullmatch(rf"[0-9a-f]{{{length}}}", verifier_review[field]) is None
                for field, length in (
                    ("source_sha", 40), ("merge_commit_sha", 40),
                    ("diff_sha256", 64), ("report_sha256", 64),
                    ("owner_attestation_sha256", 64),
                )
            )
        ):
            raise TrustRootError("TRUST_ROOT_INVALID: verifier-review contract pins are malformed")

        artifact = value.get("artifact")
        if (
            not isinstance(artifact, dict)
            or set(artifact) != ARTIFACT_TRUST_ROOT_FIELDS
            or any(type(artifact.get(field)) is not int or artifact[field] <= 0 for field in ARTIFACT_INTEGER_FIELDS)
            or artifact.get("pr_number") != HISTORICAL_16L1_PR_NUMBER
            or artifact.get("tooling_pr_number") != HISTORICAL_TOOLING_PR_NUMBER
            or artifact.get("workflow_path") != ".github/workflows/16pkg-native-installer.yml"
            or any(
                not isinstance(artifact.get(field), str)
                or re.fullmatch(r"[0-9a-f]{40}", artifact[field]) is None
                for field in ARTIFACT_COMMIT_FIELDS
            )
            or any(
                not isinstance(artifact.get(field), str)
                or re.fullmatch(r"[0-9a-f]{64}", artifact[field]) is None
                for field in ARTIFACT_DIGEST_FIELDS
            )
            or any(
                not isinstance(artifact.get(field), str) or not artifact[field].strip()
                for field in ("branch", "workflow_path", "artifact_name")
            )
        ):
            raise TrustRootError("TRUST_ROOT_INVALID: historical 16L1 artifact/provenance pins are malformed")

        return cls(
            authority=value["authority"],
            repository_full_name=value["repository_full_name"],
            repository_id=value["repository_id"],
            owner_login=value["owner_login"],
            owner_user_id=value["owner_user_id"],
            review_mode=value["review_mode"],
            receipt_issue_number=value["receipt_issue_number"],
            predecessor=dict(predecessor),
            verifier_review=dict(verifier_review),
            artifact=dict(artifact),
        )


class GitHubAPI:
    """Small read-only GitHub REST client; write operations are intentionally absent."""

    def __init__(self, token: str, repository: str = REPOSITORY, timeout: int = 30):
        if not token:
            raise VerificationError("GitHub read token is unavailable")
        self.token = token
        self.repository = repository
        self.timeout = timeout

    def _request(self, url: str, accept: str = "application/vnd.github+json", limit: int = 12 * 1024 * 1024) -> bytes:
        request = Request(url, headers={
            "Authorization": "Bearer " + self.token,
            "Accept": accept,
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "CoSoS-16FV-01-Trusted-Evidence-Verifier",
        })
        with urlopen(request, timeout=self.timeout) as response:
            data = response.read(limit + 1)
            if len(data) > limit:
                raise VerificationError("GitHub response exceeded the configured size limit")
            return data

    def get_json(self, path: str) -> dict[str, Any]:
        raw = self._request("https://api.github.com" + path)
        value = json.loads(raw.decode("utf-8"))
        if not isinstance(value, dict):
            raise VerificationError("GitHub API returned a non-object response")
        return value

    def get_file_bytes(self, path: str, ref: str) -> bytes:
        encoded_path = quote(path, safe="/")
        record = self.get_json(f"/repos/{self.repository}/contents/{encoded_path}?ref={quote(ref, safe='')}")
        if record.get("encoding") != "base64" or not isinstance(record.get("content"), str):
            raise VerificationError("GitHub did not return the pinned verifier source as base64 content")
        try:
            return base64.b64decode(record["content"], validate=False)
        except (ValueError, TypeError) as exc:
            raise VerificationError("Pinned verifier source content is malformed") from exc

    def get_diff(self, pr_number: int) -> bytes:
        return self._request(
            f"https://api.github.com/repos/{self.repository}/pulls/{pr_number}",
            accept="application/vnd.github.diff",
            limit=32 * 1024 * 1024,
        )

    def get_external_bytes(self, url: str, limit: int = MAX_BUNDLE_BYTES) -> bytes:
        parsed = urlparse(url)
        if (
            parsed.scheme != "https"
            or not (
                parsed.hostname == "github.com" and parsed.path.startswith("/user-attachments/assets/")
                or parsed.hostname == "private-user-images.githubusercontent.com"
            )
        ):
            raise VerificationError("Evidence bundle URL is outside the permitted GitHub attachment hosts")
        request = Request(url, headers={"User-Agent": "CoSoS-16FV-01-Trusted-Evidence-Verifier"})
        with urlopen(request, timeout=self.timeout) as response:
            final_host = urlparse(response.geturl()).hostname
            if final_host not in {"github.com", "private-user-images.githubusercontent.com"}:
                raise VerificationError("Evidence bundle redirected outside GitHub attachment hosts")
            data = response.read(limit + 1)
            if len(data) > limit:
                raise VerificationError("Evidence bundle exceeds the configured size limit")
            return data

    def download_artifact(self, artifact_id: int, destination: Path) -> None:
        command = [
            "gh", "api",
            f"repos/{self.repository}/actions/artifacts/{artifact_id}/zip",
            "--output", str(destination),
        ]
        env = dict(os.environ)
        env["GH_TOKEN"] = self.token
        result = subprocess.run(command, capture_output=True, text=True, timeout=240, env=env, check=False)
        if result.returncode != 0 or not destination.is_file():
            raise VerificationError("GitHub Actions artifact download failed: " + (result.stderr or "no output")[-1200:])


def canonical_json_bytes(value: dict[str, Any]) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def object_sha256(value: dict[str, Any], omit: str) -> str:
    return hashlib.sha256(canonical_json_bytes({key: item for key, item in value.items() if key != omit})).hexdigest()


def _hex(value: object, length: int) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{" + str(length) + r"}", value) is not None


def _comment_id_from_url(value: object, repository: str, issue_number: int) -> int:
    if not isinstance(value, str):
        raise VerificationError("GitHub record URL is missing")
    parsed = urlparse(value)
    expected_path = f"/{repository}/issues/{issue_number}"
    match = re.fullmatch(r"issuecomment-([1-9][0-9]*)", parsed.fragment)
    if parsed.scheme != "https" or parsed.hostname != "github.com" or parsed.path != expected_path or not match:
        raise VerificationError("GitHub record URL is not a canonical issue-comment URL")
    return int(match.group(1))


def _check_unchanged_comment(comment: dict[str, Any], expected_id: int, issue_number: int, root: TrustRoot) -> str:
    user = comment.get("user")
    if (
        type(comment.get("id")) is not int
        or comment["id"] != expected_id
        or not isinstance(user, dict)
        or user.get("login", "").casefold() != root.owner_login.casefold()
        or type(user.get("id")) is not int
        or user["id"] != root.owner_user_id
        or comment.get("created_at") != comment.get("updated_at")
        or not isinstance(comment.get("body"), str)
    ):
        raise VerificationError("GitHub record identity, owner authorship or unchanged-record check failed")
    expected_issue_url = (
        f"https://api.github.com/repos/{root.repository_full_name}/issues/{issue_number}"
    )
    if comment.get("issue_url") != expected_issue_url:
        raise VerificationError("GitHub comment belongs to the wrong canonical repository or issue")
    return "github:issue-comment:" + str(expected_id)


def _record_payload(body: str, marker: str) -> dict[str, Any]:
    if body.count(marker) != 1:
        raise VerificationError("Evidence comment marker is missing or duplicated")
    remainder = body.split(marker, 1)[1].strip()
    fence = chr(96) * 3
    if not (remainder.startswith(fence + "json") and remainder.endswith(fence)):
        raise VerificationError("Evidence comment must contain one canonical fenced JSON record")
    encoded = remainder[len(fence + "json"): -len(fence)].strip()
    value = json.loads(encoded)
    if not isinstance(value, dict):
        raise VerificationError("Evidence record payload must be a JSON object")
    return value


def _safe_zip_members(archive: zipfile.ZipFile, max_unpacked: int) -> dict[str, zipfile.ZipInfo]:
    members: dict[str, zipfile.ZipInfo] = {}
    total = 0
    for info in archive.infolist():
        name = info.filename
        path = PurePosixPath(name)
        mode = info.external_attr >> 16
        if (
            not name or name in members or path.is_absolute()
            or "\\" in name or any(part in {"", ".", ".."} for part in path.parts)
            or stat.S_ISLNK(mode)
        ):
            raise VerificationError("Evidence ZIP contains duplicate, linked or unsafe paths")
        total += info.file_size
        if total > max_unpacked:
            raise VerificationError("Evidence ZIP exceeds the unpacked size limit")
        members[name] = info
    return members




def _canonical_windows_path(value: object, field: str, expected_drive: str | None = None) -> tuple[str, list[str]]:
    """Return a case-normalized absolute Windows path and every component path."""
    if not isinstance(value, str) or not value or value != value.strip():
        raise VerificationError("Windows storage path is missing or malformed: " + field)
    raw = value.replace("/", "\\")
    if raw.startswith("\\\\") or raw.startswith("\\??\\"):
        raise VerificationError("UNC and device paths are forbidden for isolated storage: " + field)
    drive, tail = ntpath.splitdrive(raw)
    if re.fullmatch(r"[A-Za-z]:", drive) is None or not tail.startswith("\\"):
        raise VerificationError("Windows storage path must be absolute on a local drive: " + field)
    if expected_drive is not None and ntpath.normcase(drive) != ntpath.normcase(expected_drive):
        raise VerificationError("Windows storage path changed drives: " + field)
    segments = tail[1:].split("\\")
    if segments and segments[-1] == "":
        segments.pop()
    if any(part in {"", ".", ".."} for part in segments):
        raise VerificationError("Windows storage path contains an unsafe component: " + field)
    normalized = ntpath.normpath(raw)
    if (
        not ntpath.isabs(normalized)
        or ntpath.splitdrive(normalized)[0] == ""
        or ntpath.normcase(ntpath.splitdrive(normalized)[0]) != ntpath.normcase(drive)
    ):
        raise VerificationError("Windows storage path normalization changed its drive: " + field)
    canonical = ntpath.normcase(normalized)
    components = [ntpath.normcase(drive + "\\")]
    current = drive + "\\"
    for part in segments:
        current = ntpath.join(current, part)
        components.append(ntpath.normcase(ntpath.normpath(current)))
    return canonical, components


def _require_strict_windows_descendant(
    candidate: object, parent: object, field: str, expected_drive: str
) -> tuple[str, list[str]]:
    parent_path, _ = _canonical_windows_path(parent, field + " parent", expected_drive)
    candidate_path, candidate_components = _canonical_windows_path(candidate, field, expected_drive)
    try:
        common = ntpath.commonpath([parent_path, candidate_path])
    except ValueError as exc:
        raise VerificationError("Windows storage paths are not comparable: " + field) from exc
    if candidate_path == parent_path or common != parent_path:
        raise VerificationError("Windows storage path is not a strict descendant: " + field)
    return candidate_path, candidate_components


def _verify_reparse_point_evidence(
    observations: dict[str, Any], workspace: str, profile_root: str, canonical_path: str
) -> None:
    if observations.get("reparse_point_check_method") != "GetFileAttributesW(FILE_ATTRIBUTE_REPARSE_POINT)":
        raise VerificationError("Isolated storage lacks an accepted Windows reparse-point check method")
    checks = observations.get("reparse_point_checks")
    if not isinstance(checks, list):
        raise VerificationError("Isolated storage lacks explicit reparse-point path evidence")
    expected_paths: list[str] = []
    seen: set[str] = set()
    for value in (workspace, profile_root, canonical_path):
        _, components = _canonical_windows_path(value, "reparse-point path")
        for component in components:
            if component not in seen:
                seen.add(component)
                expected_paths.append(component)
    actual_paths: list[str] = []
    for check in checks:
        if (
            not isinstance(check, dict)
            or set(check) != {"path", "is_reparse_point"}
            or type(check.get("is_reparse_point")) is not bool
        ):
            raise VerificationError("Windows reparse-point check record is malformed")
        checked_path, _ = _canonical_windows_path(check.get("path"), "reparse-point checked path")
        if check["is_reparse_point"] is not False:
            raise VerificationError("A Windows storage ancestor is a reparse point or junction")
        actual_paths.append(checked_path)
    if actual_paths != expected_paths:
        raise VerificationError("Windows reparse-point evidence does not cover the exact storage path chain")


def _validate_check_evidence(name: str, record: dict[str, Any], receipt: dict[str, Any]) -> None:
    if not isinstance(record, dict) or record.get("schema") != CHECK_EVIDENCE_SCHEMA:
        raise VerificationError("Raw check evidence schema is missing or invalid: " + name)
    binding_fields = (
        "repository", "source_sha", "contract_source_sha", "contract_merge_commit_sha",
        "verifier_pr_number", "verifier_source_sha", "verifier_merge_commit_sha",
        "verifier_sha256", "artifact_id", "artifact_sha256", "local_session_id",
        "workspace_nonce", "workspace_path",
    )
    if record.get("check") != name or record.get("status") != "PASS":
        raise VerificationError("Raw check evidence does not record PASS for " + name)
    if any(record.get(field) != receipt.get(field) for field in binding_fields):
        raise VerificationError("Raw check evidence is bound to another source/session/artifact: " + name)
    try:
        captured_at = datetime.fromisoformat(record.get("captured_at", "").replace("Z", "+00:00"))
        started_at = datetime.fromisoformat(receipt["started_at"].replace("Z", "+00:00"))
        finished_at = datetime.fromisoformat(receipt["finished_at"].replace("Z", "+00:00"))
    except (TypeError, ValueError, KeyError) as exc:
        raise VerificationError("Raw check evidence timestamp is malformed: " + name) from exc
    if (
        captured_at.tzinfo is None or started_at.tzinfo is None or finished_at.tzinfo is None
        or not started_at <= captured_at <= finished_at
    ):
        raise VerificationError("Raw check evidence timestamp is outside the owner session: " + name)
    observations = record.get("observations")
    if not isinstance(observations, dict):
        raise VerificationError("Raw check evidence observations are missing: " + name)
    requirements = {
        "fresh_init": {
            "new_profile_created": True, "profile_was_empty": True,
        },
        "isolated_storage": {
            "outside_existing_profile_roots": True,
        },
        "synthetic_only": {
            "artifact_signed": False, "native16L1Accepted": False, "synthetic_input_only": True,
        },
        "existing_profiles_untouched": {
            "snapshots_match": True,
        },
        "no_leakage": {
            "leakage_detected": False, "cross_profile_access_count": 0, "external_data_reads": 0,
        },
        "rollback": {
            "rollback_completed": True, "profile_removed": True,
        },
        "failure_isolation": {
            "injected_failure": True, "failure_contained": True, "existing_profiles_unchanged": True,
        },
        "owned_cleanup": {
            "workspace_removed": True, "temporary_files_removed": True,
            "cleanup_owner": OWNER_LOGIN,
        },
    }
    required_keys = {
        "fresh_init": {"new_profile_created", "profile_was_empty", "profile_id"},
        "isolated_storage": {
            "outside_existing_profile_roots", "profile_storage_root", "canonical_path",
            "reparse_point_check_method", "reparse_point_checks",
        },
        "synthetic_only": {"artifact_signed", "native16L1Accepted", "synthetic_input_only"},
        "existing_profiles_untouched": {"snapshots_match", "before_snapshot_sha256", "after_snapshot_sha256"},
        "no_leakage": {"leakage_detected", "cross_profile_access_count", "external_data_reads"},
        "rollback": {"rollback_completed", "profile_removed", "rollback_target"},
        "failure_isolation": {"injected_failure", "failure_contained", "existing_profiles_unchanged"},
        "owned_cleanup": {"workspace_removed", "temporary_files_removed", "cleanup_owner"},
    }
    expected = requirements.get(name)
    if expected is None or set(observations) != required_keys.get(name):
        raise VerificationError("Raw check evidence has an unknown check or incomplete observation set: " + name)
    for key, value in expected.items():
        actual = observations.get(key)
        if type(actual) is not type(value) or actual != value:
            raise VerificationError("Raw check observation did not pass: " + name + "." + key)
    if name == "fresh_init":
        if not isinstance(observations.get("profile_id"), str) or len(observations["profile_id"]) < 8:
            raise VerificationError("Fresh profile identity is missing")
    elif name == "isolated_storage":
        workspace_path, _ = _canonical_windows_path(receipt.get("workspace_path"), "workspace_path")
        workspace_drive = ntpath.splitdrive(workspace_path)[0]
        if workspace_path == ntpath.normcase(workspace_drive + "\\"):
            raise VerificationError("Disposable workspace must be below a Windows drive root")
        profile_storage_root, _ = _require_strict_windows_descendant(
            observations.get("profile_storage_root"), workspace_path,
            "profile_storage_root", workspace_drive,
        )
        canonical_path, _ = _require_strict_windows_descendant(
            observations.get("canonical_path"), profile_storage_root,
            "canonical_path", workspace_drive,
        )
        _verify_reparse_point_evidence(observations, workspace_path, profile_storage_root, canonical_path)
    elif name == "existing_profiles_untouched":
        before = observations.get("before_snapshot_sha256")
        after = observations.get("after_snapshot_sha256")
        if not _hex(before, 64) or before != after:
            raise VerificationError("Existing profile before/after snapshot hashes do not match")
    elif name == "rollback":
        if not isinstance(observations.get("rollback_target"), str) or not observations["rollback_target"]:
            raise VerificationError("Rollback target evidence is missing")


def verify_evidence_bundle(receipt: dict[str, Any], api: Any) -> dict[str, Any]:
    bundle = api.get_external_bytes(receipt["evidence_bundle_url"], MAX_BUNDLE_BYTES)
    bundle_sha = hashlib.sha256(bundle).hexdigest()
    if bundle_sha != receipt["evidence_bundle_sha256"]:
        raise VerificationError("Evidence bundle SHA-256 mismatch")
    try:
        with zipfile.ZipFile(io.BytesIO(bundle), "r") as archive:
            members = _safe_zip_members(archive, MAX_UNPACKED_BYTES)
            if "evidence-manifest.json" not in members or "qualification.json" not in members:
                raise VerificationError("Evidence bundle is missing its manifest or qualification receipt")
            manifest = json.loads(archive.read("evidence-manifest.json").decode("utf-8"))
            qualification = json.loads(archive.read("qualification.json").decode("utf-8"))
            if not isinstance(manifest, dict) or not isinstance(qualification, dict):
                raise VerificationError("Evidence bundle JSON records are malformed")
            if manifest.get("schema") != BUNDLE_SCHEMA or qualification.get("schema") != QUALIFICATION_SCHEMA:
                raise VerificationError("Evidence bundle schema mismatch")
            files = manifest.get("files")
            if not isinstance(files, dict) or set(files) != set(members) - {"evidence-manifest.json"}:
                raise VerificationError("Evidence bundle manifest does not cover every file exactly once")
            for name, digest in files.items():
                if not _hex(digest, 64) or hashlib.sha256(archive.read(name)).hexdigest() != digest:
                    raise VerificationError("Evidence bundle file digest mismatch: " + str(name))
            check_evidence_in_archive = receipt.get("check_evidence")
            if not isinstance(check_evidence_in_archive, dict):
                raise VerificationError("Evidence bundle check-evidence map is malformed")
            raw_check_records = {}
            for check_name, path in check_evidence_in_archive.items():
                if not isinstance(path, str) or path not in files:
                    raise VerificationError("Raw check evidence path is missing: " + str(check_name))
                raw_check_records[check_name] = json.loads(archive.read(path).decode("utf-8"))
    except (zipfile.BadZipFile, KeyError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise VerificationError("Evidence bundle is malformed or incomplete") from exc

    checks = receipt.get("checks")
    check_evidence = receipt.get("check_evidence")
    if (
        checks != {name: "PASS" for name in board.FRESH_CHECKS}
        or qualification.get("checks") != checks
        or not isinstance(check_evidence, dict)
        or set(check_evidence) != set(board.FRESH_CHECKS)
        or qualification.get("check_evidence") != check_evidence
        or any(not isinstance(path, str) or path not in files for path in check_evidence.values())
        or len(set(check_evidence.values())) != len(board.FRESH_CHECKS)
    ):
        raise VerificationError("Fresh-start checks are incomplete or lack per-check evidence files")

    for field in (
        "repository", "source_sha", "artifact_id", "artifact_sha256",
        "contract_source_sha", "contract_merge_commit_sha", "verifier_pr_number",
        "verifier_source_sha", "verifier_merge_commit_sha", "verifier_sha256",
        "windows_build", "local_session_id", "workspace_nonce", "workspace_path",
        "started_at", "finished_at", "commands",
    ):
        if qualification.get(field) != receipt.get(field):
            raise VerificationError("Evidence bundle qualification mismatch: " + field)
    if (
        manifest.get("repository") != receipt["repository"]
        or manifest.get("source_sha") != receipt["source_sha"]
        or manifest.get("artifact_id") != receipt["artifact_id"]
        or manifest.get("artifact_sha256") != receipt["artifact_sha256"]
        or manifest.get("contract_source_sha") != receipt["contract_source_sha"]
        or manifest.get("contract_merge_commit_sha") != receipt["contract_merge_commit_sha"]
        or manifest.get("verifier_pr_number") != receipt["verifier_pr_number"]
        or manifest.get("verifier_source_sha") != receipt["verifier_source_sha"]
        or manifest.get("verifier_merge_commit_sha") != receipt["verifier_merge_commit_sha"]
        or manifest.get("verifier_sha256") != receipt["verifier_sha256"]
        or not isinstance(qualification.get("commands"), list)
        or not qualification["commands"]
        or not all(isinstance(command, str) and command.strip() for command in qualification["commands"])
    ):
        raise VerificationError("Evidence bundle does not prove the exact source/artifact/verifier and executed commands")
    verified_checks = {}
    for check_name, raw_record in raw_check_records.items():
        _validate_check_evidence(check_name, raw_record, receipt)
        verified_checks[check_name] = "PASS"
    if checks != verified_checks or qualification.get("checks") != verified_checks:
        raise VerificationError("Qualification PASS flags do not match verified raw check evidence")
    return {"sha256": bundle_sha, "manifest": manifest, "qualification": qualification, "verified_checks": verified_checks}


def verify_repository_identity(root: TrustRoot, api: Any) -> None:
    repository = api.get_json(f"/repos/{root.repository_full_name}")
    owner = api.get_json("/users/" + root.owner_login)
    if (
        repository.get("full_name") != root.repository_full_name
        or repository.get("id") != root.repository_id
        or repository.get("owner", {}).get("login") != root.owner_login
    ):
        raise VerificationError("GitHub repository identity does not match the owner-controlled trust root")
    if (
        owner.get("login", "").casefold() != root.owner_login.casefold()
        or owner.get("id") != root.owner_user_id
    ):
        raise VerificationError("GitHub owner identity does not match the owner-controlled trust root")
    if root.review_mode != REVIEW_MODE:
        raise VerificationError("Trust root does not authorize GPT6_OWNER_ATTESTED review policy")


def _verify_predecessor(root: TrustRoot, api: Any) -> None:
    predecessor = root.predecessor
    repo = root.repository_full_name
    pr = api.get_json(f"/repos/{repo}/pulls/{predecessor['pr_number']}")
    base_repo = pr.get("base", {}).get("repo", {})
    head_repo = pr.get("head", {}).get("repo", {})
    if (
        pr.get("number") != predecessor["pr_number"]
        or pr.get("merged") is not True or not pr.get("merged_at")
        or pr.get("base", {}).get("ref") != "main"
        or base_repo.get("full_name") != repo or base_repo.get("id") != root.repository_id
        or pr.get("head", {}).get("sha") != predecessor["source_sha"]
        or head_repo.get("full_name") != repo or head_repo.get("id") != root.repository_id
        or pr.get("merge_commit_sha") != predecessor["merge_commit_sha"]
    ):
        raise VerificationError("Merged 16FS-01 predecessor identity does not match the trusted contract")

    predecessor_main = api.get_json(f"/repos/{repo}/compare/{predecessor['merge_commit_sha']}...main")
    if (
        predecessor_main.get("base_commit", {}).get("sha") != predecessor["merge_commit_sha"]
        or predecessor_main.get("merge_base_commit", {}).get("sha") != predecessor["merge_commit_sha"]
        or predecessor_main.get("behind_by") != 0
        or predecessor_main.get("status") not in {"ahead", "identical"}
    ):
        raise VerificationError("Merged 16FS-01 merge commit is not an ancestor of main")

    report = api.get_json(f"/repos/{repo}/issues/comments/{predecessor['report_comment_id']}")
    attestation = api.get_json(f"/repos/{repo}/issues/comments/{predecessor['owner_attestation_comment_id']}")
    report_id = _check_unchanged_comment(report, predecessor["report_comment_id"], predecessor["pr_number"], root)
    attestation_id = _check_unchanged_comment(attestation, predecessor["owner_attestation_comment_id"], predecessor["pr_number"], root)
    if report_id == attestation_id:
        raise VerificationError("Predecessor review and owner acceptance must be separate records")
    report_body = report["body"]
    attestation_body = attestation["body"]
    actual_predecessor_diff = hashlib.sha256(api.get_diff(predecessor["pr_number"])).hexdigest()
    report_hash_line = re.search(r"\n\nGPT-6 report SHA-256: ([0-9a-f]{64})\s*$", report_body)
    if (
        actual_predecessor_diff != predecessor["diff_sha256"]
        or report_hash_line is None
        or report_hash_line.group(1) != predecessor["report_sha256"]
        or hashlib.sha256(report_body[:report_hash_line.start()].encode("utf-8")).hexdigest()
        != predecessor["report_sha256"]
    ):
        raise VerificationError("Merged 16FS-01 report content or actual PR diff digest does not match the trusted pin")
    required_report = (
        "16FS-01 GPT-6 TECHNICAL REVIEW — PASS",
        "Reviewed repository: " + repo,
        "PR: #138",
        "Exact source HEAD: " + predecessor["source_sha"],
        "PR-recorded exact diff SHA-256: " + predecessor["diff_sha256"],
        "GPT-6 report SHA-256: " + predecessor["report_sha256"],
        "GPT6_OWNER_ATTESTED",
        "NOT A GITHUB APPROVAL",
    )
    required_attestation = (
        "OWNER ACCEPTANCE — 16FS-01 / PR #138",
        "Accepted source HEAD: " + predecessor["source_sha"],
        "Accepted PR diff SHA-256: " + predecessor["diff_sha256"],
        "Accepted GPT-6 report SHA-256: " + predecessor["report_sha256"],
        "This is the owner attestation required by GPT6_OWNER_ATTESTED.",
        "does NOT claim GitHub independent approval",
    )
    if not all(token in report_body for token in required_report) or not all(token in attestation_body for token in required_attestation):
        raise VerificationError("Merged 16FS-01 report or owner-attestation contract is incompatible")


def verify_16fv_source(receipt: dict[str, Any], api: Any) -> dict[str, Any]:
    """Bind the receipt to the exact merged 16FV-01 PR source and verifier file bytes."""
    if receipt.get("verifier_pr_number") != VERIFIER_PR_NUMBER:
        raise VerificationError("Fresh-start receipt names the wrong verifier PR")
    source_sha = receipt.get("verifier_source_sha")
    merge_sha = receipt.get("verifier_merge_commit_sha")
    expected_file_sha = receipt.get("verifier_sha256")
    if not _hex(source_sha, 40) or not _hex(merge_sha, 40) or not _hex(expected_file_sha, 64):
        raise VerificationError("16FV-01 source/merge/file digests are malformed")
    repo = api.repository
    if repo != REPOSITORY:
        raise VerificationError("Verifier API client is bound to the wrong repository")
    pr = api.get_json(f"/repos/{repo}/pulls/{VERIFIER_PR_NUMBER}")
    if (
        pr.get("number") != VERIFIER_PR_NUMBER
        or pr.get("merged") is not True or not pr.get("merged_at")
        or pr.get("base", {}).get("ref") != "main"
        or pr.get("base", {}).get("repo", {}).get("full_name") != repo
        or pr.get("base", {}).get("repo", {}).get("id") != REPOSITORY_ID
        or pr.get("head", {}).get("sha") != source_sha
        or pr.get("merge_commit_sha") != merge_sha
        or pr.get("head", {}).get("repo", {}).get("full_name") != repo
        or pr.get("head", {}).get("repo", {}).get("id") != REPOSITORY_ID
    ):
        raise VerificationError("16FV-01 PR/source/merge identity is not the merged accepted verifier")
    compare = api.get_json(f"/repos/{repo}/compare/{merge_sha}...main")
    if (
        compare.get("base_commit", {}).get("sha") != merge_sha
        or compare.get("merge_base_commit", {}).get("sha") != merge_sha
        or compare.get("behind_by") != 0
        or compare.get("status") not in {"ahead", "identical"}
    ):
        raise VerificationError("16FV-01 merge commit is not an ancestor of main")
    verifier_bytes = api.get_file_bytes("tools/fresh_evidence_verifier.py", source_sha)
    actual_file_sha = hashlib.sha256(verifier_bytes).hexdigest()
    if actual_file_sha != expected_file_sha:
        raise VerificationError("16FV-01 verifier source file SHA-256 mismatch")
    return {
        "verified_verifier_pr_number": VERIFIER_PR_NUMBER,
        "verified_verifier_source_sha": source_sha,
        "verified_verifier_merge_commit_sha": merge_sha,
        "verified_verifier_sha256": actual_file_sha,
    }


def _verify_verifier_review_contract(root: TrustRoot, api: Any) -> dict[str, Any]:
    """Verify the GPT6_OWNER_ATTESTED report and separate owner acceptance for PR #141."""
    review = root.verifier_review
    repo = root.repository_full_name
    pr_number = VERIFIER_PR_NUMBER
    candidate_file = api.get_file_bytes("tools/fresh_evidence_verifier.py", review["source_sha"])
    candidate_file_sha = hashlib.sha256(candidate_file).hexdigest()
    source_result = verify_16fv_source({
        "verifier_pr_number": pr_number,
        "verifier_source_sha": review["source_sha"],
        "verifier_merge_commit_sha": review["merge_commit_sha"],
        "verifier_sha256": candidate_file_sha,
    }, api)
    actual_diff_sha = hashlib.sha256(api.get_diff(pr_number)).hexdigest()
    if actual_diff_sha != review["diff_sha256"]:
        raise VerificationError("PR #141 actual diff bytes do not match the owner-controlled verifier-review pin")
    source_ancestry = api.get_json(
        f"/repos/{repo}/compare/{root.predecessor['merge_commit_sha']}...{review['source_sha']}"
    )
    if (
        source_ancestry.get("base_commit", {}).get("sha") != root.predecessor["merge_commit_sha"]
        or source_ancestry.get("merge_base_commit", {}).get("sha") != root.predecessor["merge_commit_sha"]
        or source_ancestry.get("behind_by") != 0
        or source_ancestry.get("status") not in {"ahead", "identical"}
    ):
        raise VerificationError("Merged 16FS-01 predecessor is not included in PR #141 source ancestry")

    report_comment = api.get_json(f"/repos/{repo}/issues/comments/{review['report_comment_id']}")
    attestation_comment = api.get_json(f"/repos/{repo}/issues/comments/{review['owner_attestation_comment_id']}")
    report_identity = _check_unchanged_comment(report_comment, review["report_comment_id"], pr_number, root)
    attestation_identity = _check_unchanged_comment(attestation_comment, review["owner_attestation_comment_id"], pr_number, root)
    if report_identity == attestation_identity:
        raise VerificationError("PR #141 technical review and owner acceptance must be distinct GitHub comments")

    report = _record_payload(report_comment["body"], VERIFIER_REVIEW_MARKER)
    attestation = _record_payload(attestation_comment["body"], VERIFIER_ATTESTATION_MARKER)
    if set(report) != {
        "schema", "repository", "repository_id", "pr_number", "source_sha", "diff_sha256",
        "review_mode", "review_policy_semantics", "model_authorship", "verdict",
        "owner_login", "owner_user_id", "review_report_sha256",
    }:
        raise VerificationError("PR #141 GPT-6 review report has an unexpected schema or field set")
    if (
        report.get("schema") != VERIFIER_REVIEW_SCHEMA
        or report.get("verdict") != "PASS"
        or report.get("repository") != repo
        or report.get("repository_id") != root.repository_id
        or report.get("pr_number") != pr_number
        or report.get("source_sha") != review["source_sha"]
        or report.get("diff_sha256") != actual_diff_sha
        or report.get("review_mode") != REVIEW_MODE
        or report.get("review_policy_semantics") != "OWNER_ATTESTED_NOT_GITHUB_INDEPENDENT_APPROVAL"
        or report.get("model_authorship") != "OWNER_ATTESTED_NOT_CRYPTOGRAPHICALLY_AUTHENTICATED"
        or report.get("owner_login") != root.owner_login
        or type(report.get("owner_user_id")) is not int
        or report.get("owner_user_id") != root.owner_user_id
        or report.get("review_report_sha256") != review["report_sha256"]
        or report.get("review_report_sha256") != object_sha256(report, "review_report_sha256")
    ):
        raise VerificationError("PR #141 GPT-6 report verdict, owner identity or exact source/diff digest binding failed")
    if set(attestation) != {
        "schema", "repository", "repository_id", "pr_number", "source_sha", "diff_sha256",
        "review_report_sha256", "review_mode", "owner_login", "owner_user_id",
        "accepted", "owner_attestation_sha256",
    }:
        raise VerificationError("PR #141 owner attestation has an unexpected schema or field set")
    if (
        attestation.get("schema") != VERIFIER_ATTESTATION_SCHEMA
        or attestation.get("repository") != repo
        or attestation.get("repository_id") != root.repository_id
        or attestation.get("pr_number") != pr_number
        or attestation.get("source_sha") != review["source_sha"]
        or attestation.get("diff_sha256") != actual_diff_sha
        or attestation.get("review_report_sha256") != review["report_sha256"]
        or attestation.get("review_mode") != REVIEW_MODE
        or attestation.get("owner_login") != root.owner_login
        or type(attestation.get("owner_user_id")) is not int
        or attestation.get("owner_user_id") != root.owner_user_id
        or attestation.get("accepted") is not True
        or attestation.get("owner_attestation_sha256") != review["owner_attestation_sha256"]
        or attestation.get("owner_attestation_sha256") != object_sha256(attestation, "owner_attestation_sha256")
    ):
        raise VerificationError("PR #141 separate owner acceptance does not bind the exact PASS report, source and diff")
    return {
        **source_result,
        "verified_verifier_review_report_sha256": review["report_sha256"],
        "verified_verifier_owner_attestation_sha256": review["owner_attestation_sha256"],
        "verified_verifier_review_report_identity": report_identity,
        "verified_verifier_owner_attestation_identity": attestation_identity,
    }



def _verify_gh_attestation(
    path: Path,
    root: TrustRoot,
    expected_digest: str,
    runner: Callable[..., Any],
) -> None:
    """Require one cryptographically verified proof for this exact subject digest.

    GitHub's supported verification output does not expose a stable attestation
    ID. The production guarantee is therefore digest-scoped, not UI-ID-pinned.
    """
    if not _hex(expected_digest, 64) or not path.is_file():
        raise VerificationError("Sigstore subject file or expected SHA-256 is missing")
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected_digest:
        raise VerificationError("Sigstore subject file does not match its independently pinned digest")
    artifact = root.artifact
    workflow = root.repository_full_name + "/" + artifact["workflow_path"]
    invocation = f"https://github.com/{root.repository_full_name}/actions/runs/{artifact['run_id']}/attempts/{artifact['run_attempt']}"
    command = [
        "gh", "attestation", "verify", str(path),
        "--repo", root.repository_full_name,
        "--signer-workflow", workflow,
        "--source-ref", "refs/pull/135/merge",
        "--source-digest", artifact["original_event_sha"],
        "--format", "json",
    ]
    result = runner(command, capture_output=True, text=True, timeout=120, check=False)
    if result.returncode != 0:
        raise VerificationError("GitHub Sigstore attestation verification failed: " + path.name)
    try:
        verified = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise VerificationError("GitHub Sigstore verifier returned malformed JSON") from exc
    if isinstance(verified, dict):
        verified = [verified]
    if not isinstance(verified, list) or len(verified) != 1:
        raise VerificationError("Expected exactly one verified Sigstore proof for the pinned subject digest")
    item = verified[0]
    attestation = item.get("attestation") if isinstance(item, dict) else None
    vr = item.get("verificationResult", {}) if isinstance(item, dict) else {}
    signature = vr.get("signature", {})
    certificate = signature.get("certificate", {}) if isinstance(signature, dict) else {}
    statement = vr.get("statement", {})
    predicate = statement.get("predicate", {}) if isinstance(statement, dict) else {}
    subjects = statement.get("subject") if isinstance(statement, dict) else None
    invocation_id = predicate.get("runDetails", {}).get("metadata", {}).get("invocationId")
    if (
        not isinstance(attestation, dict)
        or not isinstance(subjects, list) or len(subjects) != 1
        or not isinstance(subjects[0], dict)
        or not isinstance(subjects[0].get("name"), str) or not subjects[0]["name"]
        or not isinstance(subjects[0].get("digest"), dict)
        or subjects[0]["digest"].get("sha256") != expected_digest
        or certificate.get("runInvocationURI") != invocation
        or str(certificate.get("sourceRepositoryIdentifier")) != str(root.repository_id)
        or certificate.get("sourceRepositoryRef") != "refs/pull/135/merge"
        or certificate.get("githubWorkflowTrigger") != "pull_request"
        or certificate.get("sourceRepositoryDigest") != artifact["original_event_sha"]
        or statement.get("predicateType") != "https://slsa.dev/provenance/v1"
        or invocation_id != invocation
    ):
        raise VerificationError("Sigstore proof is missing, duplicated, unauthorized or not bound to the exact subject/run/workflow")



def verify_historical_16l1_artifact(
    root: TrustRoot,
    api: Any,
    *,
    runner: Callable[..., Any] = subprocess.run,
) -> dict[str, Any]:
    """Verify actual historical 16L1 bytes, Actions provenance and merged source ancestry."""
    artifact = root.artifact
    repo = root.repository_full_name
    metadata = api.get_json(f"/repos/{repo}/actions/artifacts/{artifact['artifact_id']}")
    run = api.get_json(f"/repos/{repo}/actions/runs/{artifact['run_id']}")
    artifacts = api.get_json(f"/repos/{repo}/actions/runs/{artifact['run_id']}/artifacts?per_page=100")
    source_pr = api.get_json(f"/repos/{repo}/pulls/{artifact['pr_number']}")
    tooling_pr = api.get_json(f"/repos/{repo}/pulls/{artifact['tooling_pr_number']}")
    source_merge = api.get_json(f"/repos/{repo}/commits/{artifact['source_merge_commit_sha']}")
    source_compare = api.get_json(f"/repos/{repo}/compare/{artifact['source_sha']}...main")
    tooling_compare = api.get_json(f"/repos/{repo}/compare/{artifact['tooling_merge_commit_sha']}...main")

    if (
        metadata.get("id") != artifact["artifact_id"]
        or metadata.get("name") != artifact["artifact_name"]
        or metadata.get("digest") != "sha256:" + artifact["archive_sha256"]
        or metadata.get("expired") is not False
        or metadata.get("size_in_bytes", 0) <= 0
        or metadata.get("workflow_run", {}).get("id") != artifact["run_id"]
        or metadata.get("workflow_run", {}).get("repository_id") != root.repository_id
        or metadata.get("workflow_run", {}).get("head_sha") != artifact["source_sha"]
    ):
        raise VerificationError("Historical 16L1 artifact metadata/identity/digest/expiry mismatch")
    run_repo = run.get("repository", {})
    head_repo = run.get("head_repository", {})
    if (
        run.get("id") != artifact["run_id"]
        or run.get("run_attempt") != artifact["run_attempt"]
        or run.get("status") != "completed" or run.get("conclusion") != "success"
        or run.get("event") != "pull_request"
        or run.get("path") != artifact["workflow_path"]
        or run.get("workflow_id") != artifact["workflow_id"]
        or run.get("head_branch") != artifact["branch"]
        or run.get("head_sha") != artifact["source_sha"]
        or run.get("head_commit", {}).get("id") != artifact["source_sha"]
        or run_repo.get("full_name") != repo or run_repo.get("id") != root.repository_id
        or head_repo.get("full_name") != repo or head_repo.get("id") != root.repository_id
    ):
        raise VerificationError("Historical 16L1 workflow run is not the pinned successful PR event")
    if (
        source_pr.get("number") != artifact["pr_number"]
        or source_pr.get("merged") is not True or not source_pr.get("merged_at")
        or source_pr.get("base", {}).get("ref") != "main"
        or source_pr.get("base", {}).get("sha") != artifact["source_base_sha"]
        or source_pr.get("head", {}).get("sha") != artifact["source_sha"]
        or source_pr.get("head", {}).get("ref") != artifact["branch"]
        or source_pr.get("merge_commit_sha") != artifact["source_merge_commit_sha"]
    ):
        raise VerificationError("Historical 16L1 source PR was not merged at the pinned source")
    if (
        tooling_pr.get("number") != artifact["tooling_pr_number"]
        or tooling_pr.get("merged") is not True or not tooling_pr.get("merged_at")
        or tooling_pr.get("base", {}).get("ref") != "main"
        or tooling_pr.get("head", {}).get("sha") != artifact["tooling_head_sha"]
        or tooling_pr.get("merge_commit_sha") != artifact["tooling_merge_commit_sha"]
    ):
        raise VerificationError("Accepted 16L1 verifier-tooling PR #136 is not the pinned merged revision")
    if (
        source_merge.get("sha") != artifact["source_merge_commit_sha"]
        or sum(parent.get("sha") == artifact["source_sha"] for parent in source_merge.get("parents", [])) != 1
        or source_compare.get("base_commit", {}).get("sha") != artifact["source_sha"]
        or source_compare.get("merge_base_commit", {}).get("sha") != artifact["source_sha"]
        or source_compare.get("behind_by") != 0
        or source_compare.get("status") not in {"ahead", "identical"}
        or tooling_compare.get("base_commit", {}).get("sha") != artifact["tooling_merge_commit_sha"]
        or tooling_compare.get("merge_base_commit", {}).get("sha") != artifact["tooling_merge_commit_sha"]
        or tooling_compare.get("behind_by") != 0
        or tooling_compare.get("status") not in {"ahead", "identical"}
    ):
        raise VerificationError("Historical installer or accepted verifier tooling is not on main ancestry")
    listed = artifacts.get("artifacts", [])
    matching = [
        item for item in listed
        if item.get("id") == artifact["artifact_id"]
        and item.get("name") == artifact["artifact_name"]
        and item.get("digest") == "sha256:" + artifact["archive_sha256"]
        and item.get("workflow_run", {}).get("id") == artifact["run_id"]
        and item.get("workflow_run", {}).get("head_sha") == artifact["source_sha"]
        and item.get("expired") is False
    ]
    if len(matching) != 1:
        raise VerificationError("Historical 16L1 run does not contain exactly one pinned artifact")

    with tempfile.TemporaryDirectory(prefix="cosos-16fv-artifact-") as directory:
        archive_path = Path(directory) / "historical-16l1.zip"
        api.download_artifact(artifact["artifact_id"], archive_path)
        if not archive_path.is_file() or archive_path.stat().st_size > MAX_ARTIFACT_BYTES:
            raise VerificationError("Historical 16L1 artifact download is missing or oversized")
        if hashlib.sha256(archive_path.read_bytes()).hexdigest() != artifact["archive_sha256"]:
            raise VerificationError("Downloaded historical 16L1 archive digest mismatch")
        try:
            with zipfile.ZipFile(archive_path, "r") as archive:
                members = _safe_zip_members(archive, MAX_UNPACKED_BYTES)
                expected_files = {
                    "package-receipt.json": artifact["package_receipt_sha256"],
                    "installer-package/CoSoSInstaller.exe": artifact["installer_sha256"],
                    "candidate.zip": artifact["candidate_zip_sha256"],
                    "installer-unsigned-0.2.0/CoSoSInstaller.exe": artifact["update02_setup_sha256"],
                    "installer-unsigned-0.3.0/CoSoSInstaller.exe": artifact["update03_setup_sha256"],
                    "candidate-0.2.0.zip": artifact["update02_zip_sha256"],
                    "candidate-0.3.0.zip": artifact["update03_zip_sha256"],
                    "installer-release-manifest.json": artifact["release_manifest_sha256"],
                    "installer-renderer-integrity.json": artifact["renderer_manifest_sha256"],
                }
                if not set(expected_files).issubset(members):
                    raise VerificationError("Historical 16L1 artifact is missing required files")
                for name, expected in expected_files.items():
                    if hashlib.sha256(archive.read(name)).hexdigest() != expected:
                        raise VerificationError("Historical 16L1 file digest mismatch: " + name)
                package_receipt = json.loads(archive.read("package-receipt.json").decode("utf-8-sig"))
                expected_receipt = {
                    "sourceSha": artifact["source_sha"],
                    "signed": False,
                    "native16L1Accepted": False,
                    "setupSha256": artifact["installer_sha256"],
                    "zipSha256": artifact["candidate_zip_sha256"],
                    "update02SetupSha256": artifact["update02_setup_sha256"],
                    "update03SetupSha256": artifact["update03_setup_sha256"],
                    "update02ZipSha256": artifact["update02_zip_sha256"],
                    "update03ZipSha256": artifact["update03_zip_sha256"],
                    "releaseManifestSha256": artifact["release_manifest_sha256"],
                    "rendererManifestSha256": artifact["renderer_manifest_sha256"],
                    "rustSha256": artifact["rust_sha256"],
                    "rendererHostSha256": artifact["renderer_host_sha256"],
                    "rendererDllSha256": artifact["renderer_dll_sha256"],
                }
                if any(package_receipt.get(key) != value for key, value in expected_receipt.items()):
                    raise VerificationError("Historical 16L1 package receipt does not bind all pinned payload digests")
                proof_files = {
                    "package-receipt.json": artifact["package_receipt_sha256"],
                    "installer-package/CoSoSInstaller.exe": artifact["installer_sha256"],
                    "candidate.zip": artifact["candidate_zip_sha256"],
                    "installer-unsigned-0.2.0/CoSoSInstaller.exe": artifact["update02_setup_sha256"],
                    "installer-unsigned-0.3.0/CoSoSInstaller.exe": artifact["update03_setup_sha256"],
                }
                for name, expected_digest in proof_files.items():
                    target = Path(directory) / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(archive.read(name))
                    _verify_gh_attestation(target, root, expected_digest, runner)
        except (zipfile.BadZipFile, KeyError, UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise VerificationError("Historical 16L1 artifact archive is malformed") from exc

    return {
        "artifact_exists": True,
        "artifact_provenance_verified": True,
        "artifact_source_binding_verified": True,
        "verified_artifact_id": artifact["artifact_id"],
        "verified_artifact_sha256": artifact["installer_sha256"],
        "verified_artifact_source_sha": artifact["source_sha"],
        "verified_artifact_provenance_url": f"https://api.github.com/repos/{repo}/attestations/sha256:{artifact['package_receipt_sha256']}",
        "verified_artifact_provenance_sha256": artifact["package_receipt_sha256"],
    }


def verify_evidence(
    receipt: dict[str, Any],
    root: TrustRoot,
    api: Any,
    *,
    artifact_verifier: Callable[[TrustRoot, Any], dict[str, Any]] = verify_historical_16l1_artifact,
) -> board.FreshEvidenceVerification:
    if not board.fresh_receipt_structurally_valid(receipt):
        raise VerificationError("Fresh-start receipt is structurally invalid")
    if root.review_mode != REVIEW_MODE:
        raise VerificationError("Only GPT6_OWNER_ATTESTED review governance is authorized")
    repo = root.repository_full_name
    verify_repository_identity(root, api)

    _verify_predecessor(root, api)
    _verify_verifier_review_contract(root, api)
    if (
        receipt.get("contract_source_sha") != root.predecessor["source_sha"]
        or receipt.get("contract_merge_commit_sha") != root.predecessor["merge_commit_sha"]
    ):
        raise VerificationError("Receipt does not bind the exact merged 16FS-01 source and merge commit")
    verifier_source = verify_16fv_source(receipt, api)

    receipt_id = _comment_id_from_url(receipt.get("receipt_url"), repo, root.receipt_issue_number)
    report_id = _comment_id_from_url(receipt.get("review_report_url"), repo, root.receipt_issue_number)
    attestation_id = _comment_id_from_url(receipt.get("owner_attestation_url"), repo, root.receipt_issue_number)
    identities = tuple("github:issue-comment:" + str(item) for item in (receipt_id, report_id, attestation_id))
    if len(set(identities)) != 3:
        raise VerificationError("Receipt, review report and owner attestation must be three distinct GitHub records")

    receipt_comment = api.get_json(f"/repos/{repo}/issues/comments/{receipt_id}")
    report_comment = api.get_json(f"/repos/{repo}/issues/comments/{report_id}")
    attestation_comment = api.get_json(f"/repos/{repo}/issues/comments/{attestation_id}")
    receipt_identity = _check_unchanged_comment(receipt_comment, receipt_id, root.receipt_issue_number, root)
    report_identity = _check_unchanged_comment(report_comment, report_id, root.receipt_issue_number, root)
    attestation_identity = _check_unchanged_comment(attestation_comment, attestation_id, root.receipt_issue_number, root)
    if len({receipt_identity, report_identity, attestation_identity}) != 3:
        raise VerificationError("Duplicate canonical GitHub evidence record identities")

    receipt_record = _record_payload(receipt_comment["body"], RECEIPT_MARKER)
    report = _record_payload(report_comment["body"], REVIEW_MARKER)
    attestation = _record_payload(attestation_comment["body"], ATTESTATION_MARKER)
    if (
        receipt_record.get("schema") != RECEIPT_SCHEMA
        or not isinstance(receipt_record.get("receipt"), dict)
        or receipt_record.get("receipt_sha256") != receipt.get("receipt_sha256")
        or object_sha256(receipt_record["receipt"], "__no_field__") != receipt.get("receipt_sha256")
    ):
        raise VerificationError("Actual GitHub receipt record is missing or its SHA-256 is invalid")
    actual_payload = receipt_record["receipt"]
    expected_payload = {field: receipt[field] for field in RECEIPT_PAYLOAD_FIELDS if field in receipt}
    if actual_payload != expected_payload or set(actual_payload) != set(RECEIPT_PAYLOAD_FIELDS):
        raise VerificationError("Task registry receipt differs from the actual immutable GitHub receipt record")

    artifact = root.artifact
    source_pr_number = artifact["pr_number"]
    source_pr = api.get_json(f"/repos/{repo}/pulls/{source_pr_number}")
    if (
        source_pr.get("number") != source_pr_number
        or source_pr.get("base", {}).get("ref") != "main"
        or source_pr.get("head", {}).get("sha") != artifact["source_sha"]
        or source_pr.get("merge_commit_sha") != artifact["source_merge_commit_sha"]
        or source_pr.get("merged") is not True
    ):
        raise VerificationError("Review PR identity/source SHA does not match the trusted 16L1 source")
    diff_digest = hashlib.sha256(api.get_diff(source_pr_number)).hexdigest()

    if (
        report.get("schema") != REVIEW_SCHEMA
        or report.get("verdict") != "PASS"
        or report.get("repository") != repo
        or report.get("repository_id") != root.repository_id
        or report.get("pr_number") != source_pr_number
        or report.get("source_sha") != source_pr["head"]["sha"]
        or report.get("diff_sha256") != diff_digest
        or report.get("artifact_sha256") != artifact["installer_sha256"]
        or report.get("receipt_sha256") != receipt["receipt_sha256"]
        or report.get("security_review", {}).get("review_policy") != REVIEW_MODE
        or report.get("security_review", {}).get("review_policy_semantics") != "OWNER_ATTESTED_NOT_GITHUB_INDEPENDENT_APPROVAL"
        or report.get("security_review", {}).get("model_authorship") != "OWNER_ATTESTED_NOT_CRYPTOGRAPHICALLY_AUTHENTICATED"
        or "independent_review" in report
        or "reviewer_independent" in report
        or "independent_review" in report.get("security_review", {})
        or "reviewer_independent" in report.get("security_review", {})
        or report.get("review_report_sha256") != object_sha256(report, "review_report_sha256")
        or receipt.get("review_report_sha256") != report.get("review_report_sha256")
    ):
        raise VerificationError("GPT-6 review report schema, verdict, identity or digest binding is invalid")

    if (
        attestation.get("schema") != ATTESTATION_SCHEMA
        or attestation.get("repository") != repo
        or attestation.get("repository_id") != root.repository_id
        or attestation.get("pr_number") != source_pr_number
        or attestation.get("source_sha") != source_pr["head"]["sha"]
        or attestation.get("diff_sha256") != diff_digest
        or attestation.get("artifact_sha256") != artifact["installer_sha256"]
        or attestation.get("receipt_sha256") != receipt["receipt_sha256"]
        or attestation.get("review_report_sha256") != report.get("review_report_sha256")
        or attestation.get("review_mode") != REVIEW_MODE
        or attestation.get("owner_login", "").casefold() != root.owner_login.casefold()
        or attestation.get("accepted") is not True
        or attestation.get("owner_attestation_sha256") != object_sha256(attestation, "owner_attestation_sha256")
        or receipt.get("owner_attestation_sha256") != attestation.get("owner_attestation_sha256")
    ):
        raise VerificationError("Separate owner attestation does not accept the exact report, receipt and source revision")

    artifact_result = artifact_verifier(root, api)
    if (
        artifact_result.get("artifact_exists") is not True
        or artifact_result.get("artifact_provenance_verified") is not True
        or artifact_result.get("artifact_source_binding_verified") is not True
        or artifact_result.get("verified_artifact_id") != receipt.get("artifact_id")
        or artifact_result.get("verified_artifact_sha256") != receipt.get("artifact_sha256")
        or artifact_result.get("verified_artifact_source_sha") != receipt.get("artifact_source_sha")
        or artifact_result.get("verified_artifact_provenance_url") != receipt.get("artifact_provenance_url")
        or artifact_result.get("verified_artifact_provenance_sha256") != receipt.get("artifact_provenance_sha256")
    ):
        raise VerificationError("Historical 16L1 artifact, provenance or receipt digest binding failed")

    bundle_result = verify_evidence_bundle(receipt, api)
    if (
        receipt.get("repository") != repo
        or receipt.get("repository_id") != root.repository_id
        or receipt.get("review_pr_number") != source_pr_number
        or receipt.get("source_sha") != source_pr["head"]["sha"]
        or receipt.get("reviewed_source_sha") != source_pr["head"]["sha"]
        or receipt.get("diff_sha256") != diff_digest
        or receipt.get("reviewed_diff_sha256") != diff_digest
        or receipt.get("review_mode") != REVIEW_MODE
        or receipt.get("owner_login", "").casefold() != root.owner_login.casefold()
        or receipt.get("artifact_source_sha") != artifact["source_sha"]
        or receipt.get("evidence_bundle_sha256") != bundle_result["sha256"]
    ):
        raise VerificationError("Receipt repository/PR/source/diff/artifact/evidence binding mismatch")

    verification_values = {name: True for name in board.FRESH_VERIFICATION_FIELDS}
    verification_values.update({
        "verified_review_mode": REVIEW_MODE,
        "verified_owner_login": root.owner_login,
        "verified_receipt_url": receipt["receipt_url"],
        "verified_receipt_sha256": receipt["receipt_sha256"],
        "verified_review_report_url": receipt["review_report_url"],
        "verified_review_report_sha256": report["review_report_sha256"],
        "verified_review_report_verdict": report["verdict"],
        "verified_review_report_source_sha": report["source_sha"],
        "verified_review_report_diff_sha256": report["diff_sha256"],
        "verified_review_report_artifact_sha256": report["artifact_sha256"],
        "verified_owner_attestation_url": receipt["owner_attestation_url"],
        "verified_receipt_record_identity": receipt_identity,
        "verified_review_report_record_identity": report_identity,
        "verified_owner_attestation_record_identity": attestation_identity,
        "verified_owner_attestation_author_login": attestation_comment["user"]["login"],
        "verified_owner_attestation_sha256": attestation["owner_attestation_sha256"],
        "verified_owner_attested_review_report_sha256": attestation["review_report_sha256"],
        "verified_owner_attested_source_sha": attestation["source_sha"],
        "verified_source_sha": receipt["source_sha"],
        "verified_reviewed_source_sha": receipt["reviewed_source_sha"],
        "verified_diff_sha256": receipt["diff_sha256"],
        "verified_reviewed_diff_sha256": receipt["reviewed_diff_sha256"],
        "verified_artifact_id": artifact_result["verified_artifact_id"],
        "verified_artifact_sha256": artifact_result["verified_artifact_sha256"],
        "verified_artifact_source_sha": artifact_result["verified_artifact_source_sha"],
        "verified_artifact_provenance_url": artifact_result["verified_artifact_provenance_url"],
        "verified_artifact_provenance_sha256": artifact_result["verified_artifact_provenance_sha256"],
        "verified_fresh_start_checks": tuple(board.FRESH_CHECKS),
        "verified_repository_full_name": repo,
        "verified_repository_id": root.repository_id,
        "verified_review_pr_number": source_pr_number,
        "verified_review_pr_head_sha": source_pr["head"]["sha"],
        "verified_review_pr_diff_sha256": diff_digest,
        "verified_owner_user_id": root.owner_user_id,
        "verified_review_report_schema": report["schema"],
        "verified_review_report_author_user_id": report_comment["user"]["id"],
        "verified_receipt_author_user_id": receipt_comment["user"]["id"],
        "verified_owner_attestation_author_user_id": attestation_comment["user"]["id"],
        "verified_contract_source_sha": root.predecessor["source_sha"],
        "verified_contract_merge_commit_sha": root.predecessor["merge_commit_sha"],
        **verifier_source,
    })
    verification = board.FreshEvidenceVerification(**verification_values)
    if not verification.authoritative_for(receipt):
        raise VerificationError("The typed 17-fact contract rejected the verified evidence")
    return verification


def _trusted_production_environment(env: dict[str, str]) -> bool:
    expected_workflow_ref = f"{REPOSITORY}/{PRODUCTION_WORKFLOW_PATH}@refs/heads/main"
    return (
        env.get("GITHUB_ACTIONS") == "true"
        and env.get("GITHUB_EVENT_NAME") == "workflow_dispatch"
        and env.get("GITHUB_REF") == "refs/heads/main"
        and env.get("GITHUB_REPOSITORY") == REPOSITORY
        and env.get("GITHUB_WORKFLOW_REF") == expected_workflow_ref
    )


def production_verifier_from_environment(environ: dict[str, str] | None = None):
    """Load owner authority only inside the protected main-only production workflow."""
    env = os.environ if environ is None else environ
    raw = env.get(TRUST_ROOT_ENV)
    if not raw:
        return None, "TRUST_ROOT_UNCONFIGURED"
    if not _trusted_production_environment(env):
        return None, "TRUST_ROOT_INVALID"
    try:
        mapping = json.loads(raw)
        root = TrustRoot.from_mapping(mapping, production=True)
    except (json.JSONDecodeError, TrustRootError, TypeError, ValueError):
        return None, "TRUST_ROOT_INVALID"
    token = env.get("GH_TOKEN") or env.get("GITHUB_TOKEN")
    if not token:
        return None, "VERIFICATION_ERROR"
    api = GitHubAPI(token, root.repository_full_name)

    def verify(receipt: dict[str, Any]) -> board.FreshEvidenceVerification:
        return verify_evidence(receipt, root, api)

    def verify_trust_root() -> bool:
        verify_repository_identity(root, api)
        _verify_predecessor(root, api)
        _verify_verifier_review_contract(root, api)
        verify_historical_16l1_artifact(root, api)
        return True

    def verify_verifier_task(task: dict[str, Any]) -> bool:
        if (
            not isinstance(task, dict)
            or task.get("status") != "done"
            or task.get("accepted_evidence_url") != f"https://github.com/{root.repository_full_name}/pull/{VERIFIER_PR_NUMBER}"
        ):
            return False
        candidate = {
            "verifier_pr_number": VERIFIER_PR_NUMBER,
            "verifier_source_sha": task.get("accepted_source_sha"),
            "verifier_merge_commit_sha": task.get("accepted_merge_commit_sha"),
            "verifier_sha256": task.get("accepted_verifier_sha256"),
        }
        if (
            candidate["verifier_source_sha"] != root.verifier_review["source_sha"]
            or candidate["verifier_merge_commit_sha"] != root.verifier_review["merge_commit_sha"]
        ):
            return False
        try:
            verify_trust_root()
            verify_16fv_source(candidate, api)
        except Exception:
            return False
        return True

    setattr(verify, "verify_verifier_task", verify_verifier_task)
    setattr(verify, "verify_trust_root", verify_trust_root)
    return verify, "CONFIGURED"


def trust_root_status() -> str:
    verifier, status = production_verifier_from_environment()
    if verifier is None:
        return status
    try:
        if verifier.verify_trust_root() is not True:
            return "VERIFICATION_ERROR"
    except Exception:
        return "VERIFICATION_ERROR"
    return "CONFIGURED"


if __name__ == "__main__":
    print(json.dumps({
        "task": "16FV-01",
        "trust_root_status": trust_root_status(),
        "production_acceptance": "FAIL_CLOSED_UNTIL_OWNER_TRUST_ROOT_AND_EVIDENCE",
        "review_mode": REVIEW_MODE,
    }, ensure_ascii=False, indent=2))
