"""Adversarial tests for the production 16FV-01 trusted evidence verifier."""
import copy
import hashlib
import io
import ntpath
import json
import os
import subprocess
import unittest
import zipfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from tools import fresh_evidence_verifier as v
from tools import task_board as board


REPO = v.REPOSITORY
OWNER_ID = v.OWNER_USER_ID
CREATED = "2026-10-30T12:00:00Z"
FENCE = chr(96) * 3
TEST_ONLY_TRUST_ROOT = json.loads(
    (Path(__file__).parent / "fixtures" / "16fv-trust-root.test.json").read_text(encoding="utf-8")
)


def production_environment(mapping=None, *, owner_controlled=True, include_token=True):
    value = copy.deepcopy(TEST_ONLY_TRUST_ROOT if mapping is None else mapping)
    if owner_controlled:
        if mapping is None:
            dynamic_root, *_ = make_fixture()
            value["verifier_review"] = copy.deepcopy(dynamic_root.verifier_review)
        value["authority"] = "owner-controlled"
    env = {
        v.TRUST_ROOT_ENV: json.dumps(value),
        "GITHUB_ACTIONS": "true",
        "GITHUB_EVENT_NAME": "workflow_dispatch",
        "GITHUB_REF": "refs/heads/main",
        "GITHUB_REPOSITORY": REPO,
        "GITHUB_WORKFLOW_REF": f"{REPO}/{v.PRODUCTION_WORKFLOW_PATH}@refs/heads/main",
    }
    if include_token:
        env["GH_TOKEN"] = "test-token"
    return env


def digest(data):
    return hashlib.sha256(data).hexdigest()


def marked(marker, value):
    return marker + "\n" + FENCE + "json\n" + json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n" + FENCE


def comment(comment_id, body, issue=140, user_id=OWNER_ID, login=v.OWNER_LOGIN, updated=CREATED):
    return {
        "id": comment_id,
        "user": {"id": user_id, "login": login},
        "created_at": CREATED,
        "updated_at": updated,
        "issue_url": f"https://api.github.com/repos/{REPO}/issues/{issue}",
        "body": body,
    }


def observations_for(check_name, receipt_fields):
    values = {
        "fresh_init": {"new_profile_created": True, "profile_was_empty": True, "profile_id": "fresh-profile-001"},
        "isolated_storage": {
            "outside_existing_profile_roots": True,
            "profile_storage_root": receipt_fields["workspace_path"] + r"\fresh-profile",
            "canonical_path": receipt_fields["workspace_path"] + r"\fresh-profile\storage",
            "reparse_point_check_method": "GetFileAttributesW(FILE_ATTRIBUTE_REPARSE_POINT)",
            "reparse_point_checks": _test_reparse_checks(
                receipt_fields["workspace_path"],
                receipt_fields["workspace_path"] + r"\fresh-profile",
                receipt_fields["workspace_path"] + r"\fresh-profile\storage",
            ),
        },
        "synthetic_only": {"artifact_signed": False, "native16L1Accepted": False, "synthetic_input_only": True},
        "existing_profiles_untouched": {"snapshots_match": True, "before_snapshot_sha256": "a" * 64, "after_snapshot_sha256": "a" * 64},
        "no_leakage": {"leakage_detected": False, "cross_profile_access_count": 0, "external_data_reads": 0},
        "rollback": {"rollback_completed": True, "profile_removed": True, "rollback_target": "fresh-profile-001"},
        "failure_isolation": {"injected_failure": True, "failure_contained": True, "existing_profiles_unchanged": True},
        "owned_cleanup": {"workspace_removed": True, "temporary_files_removed": True, "cleanup_owner": v.OWNER_LOGIN},
    }
    return values[check_name]


def make_bundle(receipt_fields, observation_overrides=None, status_overrides=None):
    observation_overrides = observation_overrides or {}
    status_overrides = status_overrides or {}
    checks = {name: status_overrides.get(name, "PASS") for name in board.FRESH_CHECKS}
    check_evidence = {name: "checks/" + name + ".json" for name in board.FRESH_CHECKS}
    files = {"qualification.json": json.dumps({
        "schema": v.QUALIFICATION_SCHEMA,
        **{field: receipt_fields[field] for field in (
            "repository", "source_sha", "artifact_id", "artifact_sha256",
            "contract_source_sha", "contract_merge_commit_sha", "verifier_pr_number",
            "verifier_source_sha", "verifier_merge_commit_sha", "verifier_sha256", "windows_build",
            "local_session_id", "workspace_nonce", "workspace_path",
            "started_at", "finished_at", "commands",
        )},
        "checks": checks,
        "check_evidence": check_evidence,
    }, sort_keys=True, separators=(",", ":")).encode()}
    for name in board.FRESH_CHECKS:
        observations = observations_for(name, receipt_fields)
        observations.update(observation_overrides.get(name, {}))
        record = {
            "schema": v.CHECK_EVIDENCE_SCHEMA,
            "check": name,
            "status": status_overrides.get(name, "PASS"),
            **{field: receipt_fields[field] for field in (
                "repository", "source_sha", "contract_source_sha", "contract_merge_commit_sha",
                "verifier_pr_number", "verifier_source_sha", "verifier_merge_commit_sha",
                "verifier_sha256", "artifact_id", "artifact_sha256", "local_session_id",
                "workspace_nonce", "workspace_path",
            )},
            "captured_at": "2026-10-30T12:15:00Z",
            "observations": observations,
        }
        files[check_evidence[name]] = json.dumps(record, sort_keys=True, separators=(",", ":")).encode()
    manifest = {
        "schema": v.BUNDLE_SCHEMA,
        "repository": receipt_fields["repository"],
        "source_sha": receipt_fields["source_sha"],
        "artifact_id": receipt_fields["artifact_id"],
        "artifact_sha256": receipt_fields["artifact_sha256"],
        "contract_source_sha": receipt_fields["contract_source_sha"],
        "contract_merge_commit_sha": receipt_fields["contract_merge_commit_sha"],
        "verifier_pr_number": receipt_fields["verifier_pr_number"],
        "verifier_source_sha": receipt_fields["verifier_source_sha"],
        "verifier_merge_commit_sha": receipt_fields["verifier_merge_commit_sha"],
        "verifier_sha256": receipt_fields["verifier_sha256"],
        "files": {name: digest(data) for name, data in files.items()},
    }
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in files.items():
            archive.writestr(name, data)
        archive.writestr("evidence-manifest.json", json.dumps(manifest, sort_keys=True, separators=(",", ":")))
    return output.getvalue(), checks, check_evidence


class FakeAPI:
    def __init__(self):
        self.repository = REPO
        self.objects = {}
        self.requested_paths = []
        self.diff = b"source PR 135 diff bytes"
        self.predecessor_diff = (Path(__file__).parent / "fixtures" / "16fs01-pr138.diff").read_bytes()
        self.verifier_diff = b"verifier PR #141 diff bytes"
        self.verifier_file = b"trusted verifier source fixture"
        self.bundle = b""
        self.artifact_zip = b""

    def get_json(self, path):
        self.requested_paths.append(path)
        if path not in self.objects:
            raise v.VerificationError("fake GitHub API record missing: " + path)
        return copy.deepcopy(self.objects[path])

    def get_file_bytes(self, path, ref):
        if path != "tools/fresh_evidence_verifier.py" or ref != "e" * 40:
            raise v.VerificationError("wrong verifier source reference")
        return self.verifier_file

    def get_diff(self, pr_number):
        if pr_number == 138:
            return self.predecessor_diff
        if pr_number == 141:
            return self.verifier_diff
        if pr_number == 135:
            return self.diff
        raise v.VerificationError("unexpected source PR")

    def get_external_bytes(self, url, limit=v.MAX_BUNDLE_BYTES):
        if url != "https://github.com/user-attachments/assets/12345678-1234-1234-1234-123456789abc":
            raise v.VerificationError("unknown attachment")
        if len(self.bundle) > limit:
            raise v.VerificationError("attachment too large")
        return self.bundle

    def download_artifact(self, artifact_id, destination):
        if artifact_id != 12868909286 and artifact_id != 3:
            raise v.VerificationError("unknown artifact id")
        Path(destination).write_bytes(self.artifact_zip)


def make_fixture():
    root_mapping = copy.deepcopy(TEST_ONLY_TRUST_ROOT)
    api = FakeAPI()
    diff_sha = digest(api.diff)
    verifier_report = {
        "schema": "cosos.16fv.verifier-review.v1",
        "repository": REPO,
        "repository_id": v.REPOSITORY_ID,
        "pr_number": v.VERIFIER_PR_NUMBER,
        "source_sha": "e" * 40,
        "diff_sha256": digest(api.verifier_diff),
        "review_mode": v.REVIEW_MODE,
        "review_policy_semantics": "OWNER_ATTESTED_NOT_GITHUB_INDEPENDENT_APPROVAL",
        "model_authorship": "OWNER_ATTESTED_NOT_CRYPTOGRAPHICALLY_AUTHENTICATED",
        "verdict": "PASS",
        "owner_login": v.OWNER_LOGIN,
        "owner_user_id": OWNER_ID,
        "review_report_sha256": "",
    }
    verifier_report["review_report_sha256"] = _test_canonical_digest(
        verifier_report, "review_report_sha256"
    )
    verifier_attestation = {
        "schema": "cosos.16fv.owner-attestation.v1",
        "repository": REPO,
        "repository_id": v.REPOSITORY_ID,
        "pr_number": v.VERIFIER_PR_NUMBER,
        "source_sha": "e" * 40,
        "diff_sha256": digest(api.verifier_diff),
        "review_report_sha256": verifier_report["review_report_sha256"],
        "review_mode": v.REVIEW_MODE,
        "owner_login": v.OWNER_LOGIN,
        "owner_user_id": OWNER_ID,
        "accepted": True,
        "owner_attestation_sha256": "",
    }
    verifier_attestation["owner_attestation_sha256"] = _test_canonical_digest(
        verifier_attestation, "owner_attestation_sha256"
    )
    root_mapping["verifier_review"].update({
        "source_sha": "e" * 40,
        "merge_commit_sha": "d" * 40,
        "diff_sha256": digest(api.verifier_diff),
        "report_sha256": verifier_report["review_report_sha256"],
        "owner_attestation_sha256": verifier_attestation["owner_attestation_sha256"],
    })
    root = v.TrustRoot.from_mapping(root_mapping, production=False)
    nonce = "a" * 16
    payload = {
        "schema_version": board.FRESH_RECEIPT_SCHEMA_VERSION,
        "kind": "owner-local-synthetic-fresh-start",
        "host": "owner-local-windows",
        "profile_scope": "new-disposable-empty",
        "migration": "none",
        "review_mode": v.REVIEW_MODE,
        "owner_login": v.OWNER_LOGIN,
        "repository": REPO,
        "repository_id": v.REPOSITORY_ID,
        "review_pr_number": 135,
        "source_sha": root.artifact["source_sha"],
        "reviewed_source_sha": root.artifact["source_sha"],
        "diff_sha256": diff_sha,
        "reviewed_diff_sha256": diff_sha,
        "artifact_id": root.artifact["artifact_id"],
        "artifact_sha256": root.artifact["installer_sha256"],
        "artifact_source_sha": root.artifact["source_sha"],
        "artifact_provenance_url": f"https://api.github.com/repos/{REPO}/attestations/sha256:{root.artifact['package_receipt_sha256']}",
        "artifact_provenance_sha256": root.artifact["package_receipt_sha256"],
        "evidence_bundle_url": "https://github.com/user-attachments/assets/12345678-1234-1234-1234-123456789abc",
        "evidence_bundle_sha256": "",
        "contract_source_sha": root.predecessor["source_sha"],
        "contract_merge_commit_sha": root.predecessor["merge_commit_sha"],
        "verifier_pr_number": v.VERIFIER_PR_NUMBER,
        "verifier_source_sha": "e" * 40,
        "verifier_merge_commit_sha": "d" * 40,
        "verifier_sha256": digest(api.verifier_file),
        "windows_build": "Windows 11 10.0.26300 x64",
        "local_session_id": "16LF-01-session-20261030-unique",
        "workspace_nonce": nonce,
        "workspace_path": "D:\\CoSoS-16LF-01-Qualification\\" + nonce,
        "started_at": "2026-10-30T12:00:00Z",
        "finished_at": "2026-10-30T12:30:00Z",
        "commands": ["pwsh -File qualification.ps1", "python -m unittest"],
        "checks": {name: "PASS" for name in board.FRESH_CHECKS},
        "check_evidence": {name: "checks/" + name + ".json" for name in board.FRESH_CHECKS},
    }
    api.bundle, payload["checks"], payload["check_evidence"] = make_bundle(payload)
    payload["evidence_bundle_sha256"] = digest(api.bundle)
    receipt_sha = v.object_sha256(payload, "__absent__")
    report = {
        "schema": v.REVIEW_SCHEMA,
        "repository": REPO,
        "repository_id": v.REPOSITORY_ID,
        "pr_number": 135,
        "source_sha": root.artifact["source_sha"],
        "diff_sha256": diff_sha,
        "artifact_sha256": root.artifact["installer_sha256"],
        "receipt_sha256": receipt_sha,
        "verdict": "PASS",
        "security_review": {
            "review_policy": v.REVIEW_MODE,
            "review_policy_semantics": "OWNER_ATTESTED_NOT_GITHUB_INDEPENDENT_APPROVAL",
            "model_authorship": "OWNER_ATTESTED_NOT_CRYPTOGRAPHICALLY_AUTHENTICATED",
        },
        "review_report_sha256": "",
    }
    report["review_report_sha256"] = v.object_sha256(report, "review_report_sha256")
    attestation = {
        "schema": v.ATTESTATION_SCHEMA,
        "repository": REPO,
        "repository_id": v.REPOSITORY_ID,
        "pr_number": 135,
        "source_sha": root.artifact["source_sha"],
        "diff_sha256": diff_sha,
        "artifact_sha256": root.artifact["installer_sha256"],
        "receipt_sha256": receipt_sha,
        "review_report_sha256": report["review_report_sha256"],
        "review_mode": v.REVIEW_MODE,
        "owner_login": v.OWNER_LOGIN,
        "accepted": True,
        "owner_attestation_sha256": "",
    }
    attestation["owner_attestation_sha256"] = v.object_sha256(attestation, "owner_attestation_sha256")
    receipt = {
        **payload,
        "receipt_sha256": receipt_sha,
        "receipt_url": f"https://github.com/{REPO}/issues/140#issuecomment-200",
        "review_report_url": f"https://github.com/{REPO}/issues/140#issuecomment-201",
        "review_report_sha256": report["review_report_sha256"],
        "owner_attestation_url": f"https://github.com/{REPO}/issues/140#issuecomment-202",
        "owner_attestation_sha256": attestation["owner_attestation_sha256"],
    }
    api.objects.update({
        f"/repos/{REPO}": {
            "id": v.REPOSITORY_ID,
            "full_name": REPO,
            "owner": {"login": v.OWNER_LOGIN},
        },
        "/users/" + v.OWNER_LOGIN: {"login": v.OWNER_LOGIN, "id": OWNER_ID},
        f"/repos/{REPO}/pulls/{v.VERIFIER_PR_NUMBER}": {
            "number": v.VERIFIER_PR_NUMBER, "merged": True, "merged_at": CREATED,
            "base": {"ref": "main", "repo": {"full_name": REPO, "id": v.REPOSITORY_ID}},
            "head": {"sha": "e" * 40, "ref": "work/16fv-01-trusted-evidence-verifier-20261030",
                     "repo": {"full_name": REPO, "id": v.REPOSITORY_ID}},
            "merge_commit_sha": "d" * 40,
        },
        "/repos/" + REPO + "/compare/" + "d" * 40 + "...main": {
            "base_commit": {"sha": "d" * 40},
            "merge_base_commit": {"sha": "d" * 40},
            "behind_by": 0, "status": "ahead",
        },
        "/repos/" + REPO + "/compare/" + root.predecessor["merge_commit_sha"] + "...main": {
            "base_commit": {"sha": root.predecessor["merge_commit_sha"]},
            "merge_base_commit": {"sha": root.predecessor["merge_commit_sha"]},
            "behind_by": 0, "status": "identical",
        },
        "/repos/" + REPO + "/compare/" + root.predecessor["merge_commit_sha"] + "..." + "e" * 40: {
            "base_commit": {"sha": root.predecessor["merge_commit_sha"]},
            "merge_base_commit": {"sha": root.predecessor["merge_commit_sha"]},
            "behind_by": 0, "status": "ahead",
        },
        f"/repos/{REPO}/pulls/138": {
            "number": 138, "merged": True, "merged_at": CREATED,
            "base": {"ref": "main", "repo": {"full_name": REPO, "id": v.REPOSITORY_ID}},
            "head": {"sha": root.predecessor["source_sha"], "ref": "work/16fs-01-freshstart-contract-20261029",
                     "repo": {"full_name": REPO, "id": v.REPOSITORY_ID}},
            "merge_commit_sha": root.predecessor["merge_commit_sha"],
        },
        f"/repos/{REPO}/issues/comments/{root.predecessor['report_comment_id']}": comment(
            root.predecessor["report_comment_id"],
            """16FS-01 GPT-6 TECHNICAL REVIEW — PASS (SCOPED; NOT A GITHUB APPROVAL)

Reviewed repository: KaisAbiyyi/chat-on-steroids-on-steroids
PR: #138
Exact source HEAD: c392b076457ca5375e260bb2267950feac31de50
PR-recorded exact diff SHA-256: 9e750f2fbb52eda0bc15275ed5c274cbe3100a988ad628a885b4391143f4aa83

Findings:
- P1 from the prior review is corrected: fresh_receipt_structurally_valid() is shape-only; fresh_receipt_status() returns PENDING_EXTERNAL_VERIFICATION when no trusted verifier is supplied; the production CLI supplies none; accepted(16LF-01) therefore cannot accept a structurally fabricated receipt; 16L3 remains blocked.
- P2 from the prior review is corrected: 16LF-01 is included in the 16F native-owner gate inventory, required to remain blocked with no evidence, and reported NOT_RUN; product_release_ready remains false.
- Owner-attested review semantics are accurately represented as GPT6_OWNER_ATTESTED, not GitHub independent approval. Record identities are required to be canonical and pairwise distinct; the attestation author must match the verified owner.
- AND/OR dependency parsing validates malformed groups before traversal; waves require externally accepted OR alternatives and keep 16L2 disabled.
- The 16U1 fresh-start decision and no-private-profile boundary are preserved. 16L2 remains NOT_RUN / NOT_AUTHORIZED; 16LF-01 and 16L3 remain NOT_RUN; 16U2 remains NOT_RUN.
- Exact-HEAD workflow runs associated with c392b076 are all completed SUCCESS (12 groups), including Task DAG Windows/Ubuntu and 16F provenance Windows/Ubuntu. Full Rust Windows/Linux workflow also completed SUCCESS.

Disposition:
PASS for the bounded 16FS-01 cloud policy/task-graph correction and merge eligibility, subject to separate owner acceptance of this exact report and source revision. This is a GPT-6 technical report, not a GitHub review/approval and not an independently authenticated reviewer identity. The report does not accept 16LF-01, 16L3, migration, installer execution, or product release. 16FV-01 remains a required next cloud task before 16LF-01 can be accepted; no production trust root is implemented by this PR.

GPT-6 report SHA-256: ac0c1581f3c7e8bc32f8c5f109fc8a074069482bdf8b88a68a133fdb9b0ec2dd""",
            issue=138,
        ),
        f"/repos/{REPO}/issues/comments/{root.predecessor['owner_attestation_comment_id']}": comment(
            root.predecessor["owner_attestation_comment_id"],
            "OWNER ACCEPTANCE — 16FS-01 / PR #138\n"
            f"Accepted source HEAD: {root.predecessor['source_sha']}\n"
            f"Accepted PR diff SHA-256: {root.predecessor['diff_sha256']}\n"
            f"Accepted GPT-6 report SHA-256: {root.predecessor['report_sha256']}\n"
            "This is the owner attestation required by GPT6_OWNER_ATTESTED.\n"
            "does NOT claim GitHub independent approval",
            issue=138,
        ),
        f"/repos/{REPO}/issues/comments/200": comment(200, marked(v.RECEIPT_MARKER, {
            "schema": v.RECEIPT_SCHEMA,
            "receipt": payload,
            "receipt_sha256": receipt_sha,
        })),
        f"/repos/{REPO}/issues/comments/201": comment(201, marked(v.REVIEW_MARKER, report)),
        f"/repos/{REPO}/issues/comments/202": comment(202, marked(v.ATTESTATION_MARKER, attestation)),
        f"/repos/{REPO}/issues/comments/301": comment(
            301, marked("<!-- COSOS-16FV-REVIEW-V1 -->", verifier_report), issue=v.VERIFIER_PR_NUMBER
        ),
        f"/repos/{REPO}/issues/comments/302": comment(
            302, marked("<!-- COSOS-16FV-OWNER-ATTESTATION-V1 -->", verifier_attestation),
            issue=v.VERIFIER_PR_NUMBER,
        ),
        f"/repos/{REPO}/pulls/135": {
            "number": 135, "merged": True, "merged_at": CREATED,
            "base": {"ref": "main", "sha": root.artifact["source_base_sha"]},
            "head": {"sha": root.artifact["source_sha"], "ref": root.artifact["branch"]},
            "merge_commit_sha": root.artifact["source_merge_commit_sha"],
        },
    })
    return root, api, receipt, report, attestation


def good_artifact_result(root, _api):
    artifact = root.artifact
    return {
        "artifact_exists": True,
        "artifact_provenance_verified": True,
        "artifact_source_binding_verified": True,
        "verified_artifact_id": artifact["artifact_id"],
        "verified_artifact_sha256": artifact["installer_sha256"],
        "verified_artifact_source_sha": artifact["source_sha"],
        "verified_artifact_provenance_url": f"https://api.github.com/repos/{root.repository_full_name}/attestations/sha256:{artifact['package_receipt_sha256']}",
        "verified_artifact_provenance_sha256": artifact["package_receipt_sha256"],
    }


class ProductionWorkflowBoundaryTests(unittest.TestCase):
    def test_production_secret_is_main_only_and_absent_from_pr_workflow(self):
        root = Path(__file__).resolve().parents[1]
        production = (
            root / ".github" / "workflows" / "16fv-production-verify.yml"
        ).read_text(encoding="utf-8")
        pull_request = (
            root / ".github" / "workflows" / "16fv-trusted-evidence-verifier.yml"
        ).read_text(encoding="utf-8")

        self.assertIn("workflow_dispatch:", production)
        self.assertNotIn("\n  pull_request:", production)
        self.assertNotIn("\n  pull_request_target:", production)
        self.assertNotIn("\n  push:", production)
        self.assertIn(
            "if: github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/main'",
            production,
        )
        self.assertIn("environment: fresh-evidence-production", production)
        for permission in (
            "contents: read", "pull-requests: read", "issues: read",
            "actions: read", "attestations: read",
        ):
            self.assertIn(permission, production)
        self.assertNotRegex(production, r":\s*write\b")
        self.assertEqual(
            production.count("secrets.COSOS_FRESH_EVIDENCE_TRUST_ROOT_JSON"), 1
        )
        self.assertNotIn("COSOS_FRESH_EVIDENCE_TRUST_ROOT_JSON", pull_request)
        self.assertNotIn("pull_request_target", pull_request)


class ProductionTrustRootTests(unittest.TestCase):
    def test_missing_root_is_explicitly_unconfigured_and_fails_closed(self):
        verifier, status = v.production_verifier_from_environment({})
        self.assertIsNone(verifier)
        self.assertEqual(status, "TRUST_ROOT_UNCONFIGURED")
        _, _, receipt, _, _ = make_fixture()
        self.assertEqual(board.fresh_receipt_status(receipt, trusted_verifier=verifier), "TRUST_ROOT_UNCONFIGURED")

    def test_test_only_fixture_cannot_be_loaded_as_production_authority(self):
        verifier, status = v.production_verifier_from_environment(
            production_environment(owner_controlled=False)
        )
        self.assertIsNone(verifier)
        self.assertEqual(status, "TRUST_ROOT_INVALID")

    def test_modified_owner_or_malformed_historical_pin_invalidates_root(self):
        mapping = copy.deepcopy(TEST_ONLY_TRUST_ROOT)
        mapping["authority"] = "owner-controlled"
        mapping["owner_user_id"] = OWNER_ID + 1
        verifier, status = v.production_verifier_from_environment(production_environment(mapping))
        self.assertIsNone(verifier)
        self.assertEqual(status, "TRUST_ROOT_INVALID")

        mapping = copy.deepcopy(TEST_ONLY_TRUST_ROOT)
        mapping["authority"] = "owner-controlled"
        mapping["artifact"]["installer_sha256"] = "0" * 63
        verifier, status = v.production_verifier_from_environment(production_environment(mapping))
        self.assertIsNone(verifier)
        self.assertEqual(status, "TRUST_ROOT_INVALID")

        mapping = copy.deepcopy(TEST_ONLY_TRUST_ROOT)
        mapping["authority"] = "owner-controlled"
        mapping["predecessor"]["source_sha"] = "not-a-source-sha"
        verifier, status = v.production_verifier_from_environment(production_environment(mapping))
        self.assertIsNone(verifier)
        self.assertEqual(status, "TRUST_ROOT_INVALID")

    def test_secret_is_rejected_outside_protected_main_dispatch_context(self):
        env = production_environment()
        env["GITHUB_EVENT_NAME"] = "pull_request"
        verifier, status = v.production_verifier_from_environment(env)
        self.assertIsNone(verifier)
        self.assertEqual(status, "TRUST_ROOT_INVALID")

        env = production_environment()
        env["GITHUB_REF"] = "refs/heads/work/16fv-01-trusted-evidence-verifier-20261030"
        verifier, status = v.production_verifier_from_environment(env)
        self.assertIsNone(verifier)
        self.assertEqual(status, "TRUST_ROOT_INVALID")

    def test_16fv_task_acceptance_rechecks_merged_16fs_and_pr141_source(self):
        _, api, receipt, _, _ = make_fixture()
        task = {
            "status": "done",
            "accepted_evidence_url": f"https://github.com/{REPO}/pull/{v.VERIFIER_PR_NUMBER}",
            "accepted_source_sha": receipt["verifier_source_sha"],
            "accepted_merge_commit_sha": receipt["verifier_merge_commit_sha"],
            "accepted_verifier_sha256": receipt["verifier_sha256"],
        }
        with patch.dict(os.environ, production_environment(), clear=True):
            with patch("tools.fresh_evidence_verifier.GitHubAPI", return_value=api):
                with patch("tools.fresh_evidence_verifier.verify_historical_16l1_artifact", return_value={}):
                    verifier, status = v.production_verifier_from_environment()
                    self.assertEqual(status, "CONFIGURED")
                    self.assertTrue(verifier.verify_verifier_task(task))
                    task["accepted_source_sha"] = "0" * 40
                    self.assertFalse(verifier.verify_verifier_task(task))
                    task["accepted_source_sha"] = receipt["verifier_source_sha"]
                    task["status"] = "claimed"
                    self.assertFalse(verifier.verify_verifier_task(task))

    def test_missing_github_token_does_not_construct_a_verifier(self):
        verifier, status = v.production_verifier_from_environment(
            production_environment(include_token=False)
        )
        self.assertIsNone(verifier)
        self.assertEqual(status, "VERIFICATION_ERROR")


class FreshEvidenceVerificationTests(unittest.TestCase):
    def verify(self, fixture):
        root, api, receipt, _, _ = fixture
        return v.verify_evidence(receipt, root, api, artifact_verifier=good_artifact_result)

    def test_complete_valid_evidence_produces_all_17_authoritative_facts(self):
        fixture = make_fixture()
        verification = self.verify(fixture)
        self.assertTrue(verification.authoritative_for(fixture[2]))
        self.assertTrue(all(getattr(verification, field) is True for field in board.FRESH_VERIFICATION_FIELDS))
        self.assertEqual(len(board.FRESH_VERIFICATION_FIELDS), 17)
        self.assertEqual(verification.verified_review_report_schema, v.REVIEW_SCHEMA)
        self.assertEqual(verification.verified_owner_user_id, OWNER_ID)
        self.assertEqual(
            len({
                verification.verified_receipt_record_identity,
                verification.verified_review_report_record_identity,
                verification.verified_owner_attestation_record_identity,
            }),
            3,
        )

    def test_missing_receipt_record_rejected(self):
        fixture = make_fixture()
        del fixture[1].objects[f"/repos/{REPO}/issues/comments/200"]
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_tampered_receipt_record_rejected(self):
        fixture = make_fixture()
        path = f"/repos/{REPO}/issues/comments/200"
        record = v._record_payload(fixture[1].objects[path]["body"], v.RECEIPT_MARKER)
        record["receipt"]["host"] = "owner-local-linux"
        fixture[1].objects[path]["body"] = marked(v.RECEIPT_MARKER, record)
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_missing_review_report_rejected(self):
        fixture = make_fixture()
        del fixture[1].objects[f"/repos/{REPO}/issues/comments/201"]
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_non_pass_verdict_rejected(self):
        fixture = make_fixture()
        path = f"/repos/{REPO}/issues/comments/201"
        report = v._record_payload(fixture[1].objects[path]["body"], v.REVIEW_MARKER)
        report["verdict"] = "FAIL"
        report["review_report_sha256"] = v.object_sha256(report, "review_report_sha256")
        fixture[1].objects[path]["body"] = marked(v.REVIEW_MARKER, report)
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_legacy_or_wrong_review_schema_rejected(self):
        for schema in ("cosos.release.cloud-provenance.v1", "cosos.legacy-review.v1"):
            fixture = make_fixture()
            path = f"/repos/{REPO}/issues/comments/201"
            report = v._record_payload(fixture[1].objects[path]["body"], v.REVIEW_MARKER)
            report["schema"] = schema
            report["review_report_sha256"] = v.object_sha256(report, "review_report_sha256")
            fixture[1].objects[path]["body"] = marked(v.REVIEW_MARKER, report)
            with self.subTest(schema=schema), self.assertRaises(v.VerificationError):
                self.verify(fixture)

    def test_legacy_independent_review_field_is_rejected(self):
        fixture = make_fixture()
        path = f"/repos/{REPO}/issues/comments/201"
        report = v._record_payload(fixture[1].objects[path]["body"], v.REVIEW_MARKER)
        report["independent_review"] = {"verdict": "APPROVED"}
        report["review_report_sha256"] = v.object_sha256(report, "review_report_sha256")
        fixture[1].objects[path]["body"] = marked(v.REVIEW_MARKER, report)
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_wrong_review_mode_rejected(self):
        fixture = make_fixture()
        path = f"/repos/{REPO}/issues/comments/201"
        report = v._record_payload(fixture[1].objects[path]["body"], v.REVIEW_MARKER)
        report["security_review"]["review_policy"] = "GITHUB_INDEPENDENT_APPROVAL"
        report["review_report_sha256"] = v.object_sha256(report, "review_report_sha256")
        fixture[1].objects[path]["body"] = marked(v.REVIEW_MARKER, report)
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_wrong_review_governance_semantics_rejected(self):
        fixture = make_fixture()
        path = f"/repos/{REPO}/issues/comments/201"
        report = v._record_payload(fixture[1].objects[path]["body"], v.REVIEW_MARKER)
        report["security_review"]["review_policy_semantics"] = "GITHUB_INDEPENDENT_APPROVAL"
        report["review_report_sha256"] = v.object_sha256(report, "review_report_sha256")
        fixture[1].objects[path]["body"] = marked(v.REVIEW_MARKER, report)
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_missing_owner_attestation_rejected(self):
        fixture = make_fixture()
        del fixture[1].objects[f"/repos/{REPO}/issues/comments/202"]
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_unauthorized_owner_identity_rejected(self):
        fixture = make_fixture()
        fixture[1].objects["/users/" + v.OWNER_LOGIN]["id"] = OWNER_ID + 1
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_attestation_author_mismatch_rejected(self):
        fixture = make_fixture()
        fixture[1].objects[f"/repos/{REPO}/issues/comments/202"]["user"]["id"] = OWNER_ID + 1
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_modified_attestation_record_rejected(self):
        fixture = make_fixture()
        fixture[1].objects[f"/repos/{REPO}/issues/comments/202"]["updated_at"] = "2026-10-30T12:01:00Z"
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_wrong_source_sha_rejected(self):
        fixture = make_fixture()
        path = f"/repos/{REPO}/issues/comments/201"
        report = v._record_payload(fixture[1].objects[path]["body"], v.REVIEW_MARKER)
        report["source_sha"] = "0" * 40
        report["review_report_sha256"] = v.object_sha256(report, "review_report_sha256")
        fixture[1].objects[path]["body"] = marked(v.REVIEW_MARKER, report)
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_predecessor_diff_and_review_report_body_are_hashed_from_actual_sources(self):
        fixture = make_fixture()
        fixture[1].predecessor_diff += b"tampered"
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)
        fixture = make_fixture()
        report_path = f"/repos/{REPO}/issues/comments/{fixture[0].predecessor['report_comment_id']}"
        fixture[1].objects[report_path]["body"] = fixture[1].objects[report_path]["body"].replace(
            "P2 from the prior review is corrected", "P2 report text was altered"
        )
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_receipt_must_bind_the_exact_merged_16fs_contract(self):
        fixture = make_fixture()
        fixture[2]["contract_source_sha"] = "0" * 40
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)
        fixture = make_fixture()
        fixture[2]["contract_merge_commit_sha"] = "0" * 40
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_predecessor_review_and_owner_attestation_records_are_required(self):
        fixture = make_fixture()
        root, api = fixture[0], fixture[1]
        del api.objects[f"/repos/{REPO}/issues/comments/{root.predecessor['report_comment_id']}"]
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)
        fixture = make_fixture()
        root, api = fixture[0], fixture[1]
        api.objects[f"/repos/{REPO}/issues/comments/{root.predecessor['owner_attestation_comment_id']}"]["updated_at"] = "2026-10-30T12:01:00Z"
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_wrong_verifier_source_or_file_digest_rejected(self):
        fixture = make_fixture()
        fixture[2]["verifier_source_sha"] = "0" * 40
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)
        fixture = make_fixture()
        fixture[1].verifier_file = b"changed verifier source"
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_issue_139_cannot_be_mistaken_for_verifier_pr_141(self):
        fixture = make_fixture()
        api, receipt = fixture[1], fixture[2]
        canonical_path = f"/repos/{REPO}/pulls/{v.VERIFIER_PR_NUMBER}"
        issue_number_path = f"/repos/{REPO}/pulls/139"
        self.assertEqual(v.VERIFIER_PR_NUMBER, 141)
        self.assertEqual(receipt["verifier_pr_number"], 141)
        self.assertIn(canonical_path, api.objects)
        self.assertNotIn(issue_number_path, api.objects)

        v.verify_16fv_source(receipt, api)
        self.assertIn(canonical_path, api.requested_paths)
        self.assertNotIn(issue_number_path, api.requested_paths)

        forged = copy.deepcopy(receipt)
        forged["verifier_pr_number"] = 139
        before = len(api.requested_paths)
        with self.assertRaises(v.VerificationError):
            v.verify_16fv_source(forged, api)
        self.assertEqual(len(api.requested_paths), before)

    def test_canonical_pr_141_route_rejects_wrong_actual_pr_number(self):
        fixture = make_fixture()
        fixture[1].objects[f"/repos/{REPO}/pulls/{v.VERIFIER_PR_NUMBER}"]["number"] = 139
        with self.assertRaises(v.VerificationError):
            v.verify_16fv_source(fixture[2], fixture[1])

    def test_unmerged_16fv_source_rejected(self):
        fixture = make_fixture()
        fixture[1].objects[f"/repos/{REPO}/pulls/{v.VERIFIER_PR_NUMBER}"]["merged"] = False
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_wrong_actual_pr_diff_digest_rejected(self):
        fixture = make_fixture()
        fixture[1].diff = b"tampered diff returned by provider"
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_duplicate_record_identities_rejected(self):
        fixture = make_fixture()
        fixture[2]["review_report_url"] = fixture[2]["receipt_url"]
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_forged_github_provenance_issue_identity_rejected(self):
        fixture = make_fixture()
        fixture[1].objects[f"/repos/{REPO}/issues/comments/200"]["issue_url"] = (
            f"https://api.github.com/repos/{REPO}/issues/141"
        )
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_artifact_missing_or_provenance_failure_rejected(self):
        fixture = make_fixture()
        def missing(_root, _api):
            raise v.VerificationError("artifact not found")
        with self.assertRaises(v.VerificationError):
            v.verify_evidence(fixture[2], fixture[0], fixture[1], artifact_verifier=missing)
        def untrusted(root, _api):
            result = good_artifact_result(root, None)
            result["artifact_provenance_verified"] = False
            return result
        with self.assertRaises(v.VerificationError):
            v.verify_evidence(fixture[2], fixture[0], fixture[1], artifact_verifier=untrusted)

    def test_artifact_digest_mismatch_rejected(self):
        fixture = make_fixture()
        fixture[2]["artifact_sha256"] = "0" * 64
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_missing_fresh_start_check_rejected(self):
        fixture = make_fixture()
        del fixture[2]["checks"][board.FRESH_CHECKS[0]]
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)

    def test_incomplete_or_malformed_receipt_rejected(self):
        for field, value in (
            ("commands", []),
            ("windows_build", ""),
            ("started_at", "not-a-date"),
            ("workspace_path", "D:\\untrusted"),
            ("check_evidence", {"fresh_init": "checks/fresh_init.log"}),
            ("evidence_bundle_sha256", "z" * 64),
        ):
            fixture = make_fixture()
            fixture[2][field] = value
            with self.subTest(field=field), self.assertRaises(v.VerificationError):
                self.verify(fixture)

    def test_api_failure_is_not_acceptance(self):
        fixture = make_fixture()
        def unavailable(_receipt):
            raise v.VerificationError("GitHub API unavailable")
        self.assertEqual(
            board.fresh_receipt_status(fixture[2], trusted_verifier=unavailable),
            "VERIFICATION_ERROR",
        )

    def test_raw_check_evidence_observations_must_prove_each_check(self):
        fixture = make_fixture()
        receipt = fixture[2]
        fixture[1].bundle, _, _ = make_bundle(
            receipt,
            observation_overrides={"no_leakage": {"cross_profile_access_count": 1}},
        )
        receipt["evidence_bundle_sha256"] = digest(fixture[1].bundle)
        with self.assertRaises(v.VerificationError):
            v.verify_evidence_bundle(receipt, fixture[1])

    def test_existing_profile_snapshot_mismatch_is_rejected(self):
        fixture = make_fixture()
        receipt = fixture[2]
        fixture[1].bundle, _, _ = make_bundle(
            receipt,
            observation_overrides={"existing_profiles_untouched": {"after_snapshot_sha256": "b" * 64}},
        )
        receipt["evidence_bundle_sha256"] = digest(fixture[1].bundle)
        with self.assertRaises(v.VerificationError):
            v.verify_evidence_bundle(receipt, fixture[1])

    def test_raw_check_evidence_status_and_paths_cannot_be_reused(self):
        fixture = make_fixture()
        receipt = fixture[2]
        fixture[1].bundle, _, _ = make_bundle(receipt, status_overrides={"rollback": "FAIL"})
        receipt["evidence_bundle_sha256"] = digest(fixture[1].bundle)
        with self.assertRaises(v.VerificationError):
            v.verify_evidence_bundle(receipt, fixture[1])
        fixture = make_fixture()
        receipt = fixture[2]
        receipt["check_evidence"][board.FRESH_CHECKS[1]] = receipt["check_evidence"][board.FRESH_CHECKS[0]]
        with self.assertRaises(v.VerificationError):
            v.verify_evidence_bundle(receipt, fixture[1])

    def test_bundle_digest_or_internal_file_tampering_rejected(self):
        fixture = make_fixture()
        fixture[1].bundle += b"tamper"
        with self.assertRaises(v.VerificationError):
            self.verify(fixture)


def make_synthetic_artifact():
    mapping = copy.deepcopy(TEST_ONLY_TRUST_ROOT)
    mapping["authority"] = "test-only"
    artifact = mapping["artifact"]
    artifact.update({
        "pr_number": 135,
        "source_sha": "a" * 40,
        "source_base_sha": "b" * 40,
        "source_merge_commit_sha": "c" * 40,
        "branch": "fix/test-artifact",
        "original_event_sha": "d" * 40,
        "run_id": 1,
        "run_attempt": 1,
        "workflow_id": 2,
        "artifact_id": 3,
        "artifact_name": "cosos-16pkg-unsigned-candidate-" + "a" * 40,
        "tooling_pr_number": 136,
        "tooling_head_sha": "e" * 40,
        "tooling_merge_commit_sha": "f" * 40,
    })
    payload_files = {
        "installer-package/CoSoSInstaller.exe": b"installer-binary-fixture",
        "candidate.zip": b"candidate-payload-fixture",
        "installer-unsigned-0.2.0/CoSoSInstaller.exe": b"updater-02-fixture",
        "installer-unsigned-0.3.0/CoSoSInstaller.exe": b"updater-03-fixture",
        "candidate-0.2.0.zip": b"updater-02-zip-fixture",
        "candidate-0.3.0.zip": b"updater-03-zip-fixture",
        "installer-release-manifest.json": b'{"release":"fixture"}',
        "installer-renderer-integrity.json": b'{"renderer":"fixture"}',
    }
    artifact["installer_sha256"] = digest(payload_files["installer-package/CoSoSInstaller.exe"])
    artifact["candidate_zip_sha256"] = digest(payload_files["candidate.zip"])
    artifact["update02_setup_sha256"] = digest(payload_files["installer-unsigned-0.2.0/CoSoSInstaller.exe"])
    artifact["update03_setup_sha256"] = digest(payload_files["installer-unsigned-0.3.0/CoSoSInstaller.exe"])
    artifact["update02_zip_sha256"] = digest(payload_files["candidate-0.2.0.zip"])
    artifact["update03_zip_sha256"] = digest(payload_files["candidate-0.3.0.zip"])
    artifact["release_manifest_sha256"] = digest(payload_files["installer-release-manifest.json"])
    artifact["renderer_manifest_sha256"] = digest(payload_files["installer-renderer-integrity.json"])
    artifact["rust_sha256"] = digest(b"rust-fixture")
    artifact["renderer_host_sha256"] = digest(b"renderer-host-fixture")
    artifact["renderer_dll_sha256"] = digest(b"renderer-dll-fixture")
    receipt = {
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
    payload_files["package-receipt.json"] = json.dumps(receipt, sort_keys=True, separators=(",", ":")).encode()
    artifact["package_receipt_sha256"] = digest(payload_files["package-receipt.json"])
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in payload_files.items():
            archive.writestr(name, data)
    archive_bytes = output.getvalue()
    artifact["archive_sha256"] = digest(archive_bytes)
    root = v.TrustRoot.from_mapping(mapping, production=False)
    api = FakeAPI()
    api.artifact_zip = archive_bytes
    metadata = {
        "id": artifact["artifact_id"], "name": artifact["artifact_name"],
        "digest": "sha256:" + artifact["archive_sha256"], "expired": False,
        "size_in_bytes": len(archive_bytes),
        "workflow_run": {"id": artifact["run_id"], "repository_id": v.REPOSITORY_ID,
                         "head_sha": artifact["source_sha"]},
    }
    run = {
        "id": artifact["run_id"], "run_attempt": artifact["run_attempt"],
        "status": "completed", "conclusion": "success", "event": "pull_request",
        "path": artifact["workflow_path"], "workflow_id": artifact["workflow_id"],
        "head_branch": artifact["branch"], "head_sha": artifact["source_sha"],
        "head_commit": {"id": artifact["source_sha"]},
        "repository": {"full_name": REPO, "id": v.REPOSITORY_ID},
        "head_repository": {"full_name": REPO, "id": v.REPOSITORY_ID},
    }
    source_pr = {
        "number": 135, "merged": True, "merged_at": CREATED,
        "base": {"ref": "main", "sha": artifact["source_base_sha"]},
        "head": {"sha": artifact["source_sha"], "ref": artifact["branch"]},
        "merge_commit_sha": artifact["source_merge_commit_sha"],
    }
    tooling_pr = {
        "number": 136, "merged": True, "merged_at": CREATED,
        "base": {"ref": "main"},
        "head": {"sha": artifact["tooling_head_sha"]},
        "merge_commit_sha": artifact["tooling_merge_commit_sha"],
    }
    compare_source = {
        "base_commit": {"sha": artifact["source_sha"]},
        "merge_base_commit": {"sha": artifact["source_sha"]},
        "behind_by": 0, "status": "ahead",
    }
    compare_tooling = {
        "base_commit": {"sha": artifact["tooling_merge_commit_sha"]},
        "merge_base_commit": {"sha": artifact["tooling_merge_commit_sha"]},
        "behind_by": 0, "status": "ahead",
    }
    api.objects.update({
        f"/repos/{REPO}/actions/artifacts/{artifact['artifact_id']}": metadata,
        f"/repos/{REPO}/actions/runs/{artifact['run_id']}": run,
        f"/repos/{REPO}/actions/runs/{artifact['run_id']}/artifacts?per_page=100": {
            "artifacts": [{
                "id": artifact["artifact_id"], "name": artifact["artifact_name"],
                "digest": "sha256:" + artifact["archive_sha256"], "expired": False,
                "workflow_run": {"id": artifact["run_id"], "head_sha": artifact["source_sha"]},
            }]
        },
        f"/repos/{REPO}/pulls/135": source_pr,
        f"/repos/{REPO}/pulls/136": tooling_pr,
        f"/repos/{REPO}/commits/{artifact['source_merge_commit_sha']}": {
            "sha": artifact["source_merge_commit_sha"],
            "parents": [{"sha": artifact["source_sha"]}, {"sha": artifact["source_base_sha"]}],
        },
        f"/repos/{REPO}/compare/{artifact['source_sha']}...main": compare_source,
        f"/repos/{REPO}/compare/{artifact['tooling_merge_commit_sha']}...main": compare_tooling,
    })
    invocation = f"https://github.com/{REPO}/actions/runs/{artifact['run_id']}/attempts/{artifact['run_attempt']}"
    proof = [{
        "verificationResult": {
            "signature": {"certificate": {
                "runInvocationURI": invocation,
                "sourceRepositoryIdentifier": str(v.REPOSITORY_ID),
                "sourceRepositoryRef": "refs/pull/135/merge",
                "githubWorkflowTrigger": "pull_request",
                "sourceRepositoryDigest": artifact["original_event_sha"],
            }},
            "statement": {
                "predicateType": "https://slsa.dev/provenance/v1",
                "predicate": {"runDetails": {"metadata": {"invocationId": invocation}}},
            },
        }
    }]
    def runner(command, *_args, **_kwargs):
        subject_path = Path(command[3])
        signed_proof = copy.deepcopy(proof[0])
        signed_proof["attestation"] = {"bundle": {"dsseEnvelope": {}}}
        signed_proof["verificationResult"]["statement"]["subject"] = [{
            "name": subject_path.name,
            "digest": {"sha256": digest(subject_path.read_bytes())},
        }]
        return SimpleNamespace(returncode=0, stdout=json.dumps([signed_proof]), stderr="")
    return root, api, runner


class HistoricalArtifactTests(unittest.TestCase):
    def test_actual_archive_hashes_and_five_sigstore_proofs_are_required(self):
        root, api, runner = make_synthetic_artifact()
        result = v.verify_historical_16l1_artifact(root, api, runner=runner)
        self.assertTrue(result["artifact_exists"])
        self.assertTrue(result["artifact_provenance_verified"])
        self.assertTrue(result["artifact_source_binding_verified"])
        self.assertEqual(result["verified_artifact_id"], 3)

    def test_missing_or_tampered_archive_rejected(self):
        root, api, runner = make_synthetic_artifact()
        api.artifact_zip = b"tampered archive"
        with self.assertRaises(v.VerificationError):
            v.verify_historical_16l1_artifact(root, api, runner=runner)

    def test_wrong_signed_event_provenance_rejected(self):
        root, api, runner = make_synthetic_artifact()
        def bad_runner(*_args, **_kwargs):
            result = runner(*_args, **_kwargs)
            proof = json.loads(result.stdout)
            proof[0]["verificationResult"]["signature"]["certificate"]["sourceRepositoryDigest"] = "0" * 40
            return SimpleNamespace(returncode=0, stdout=json.dumps(proof), stderr="")
        with self.assertRaises(v.VerificationError):
            v.verify_historical_16l1_artifact(root, api, runner=bad_runner)

    def test_expired_artifact_rejected(self):
        root, api, runner = make_synthetic_artifact()
        api.objects[f"/repos/{REPO}/actions/artifacts/3"]["expired"] = True
        with self.assertRaises(v.VerificationError):
            v.verify_historical_16l1_artifact(root, api, runner=runner)


class ProductionCliFailClosedTests(unittest.TestCase):
    def test_cli_cannot_accept_16lf_when_production_root_is_missing(self):
        data = board.load()
        tasks = board.index(data)
        tasks["16FS-01"]["status"] = "done"
        tasks["16FV-01"]["status"] = "done"
        tasks["16FV-01"]["accepted_evidence_url"] = "https://github.com/" + REPO + "/pull/" + str(v.VERIFIER_PR_NUMBER)
        _, _, receipt, _, _ = make_fixture()
        tasks["16LF-01"]["status"] = "done"
        tasks["16LF-01"]["evidence"] = receipt
        with patch.object(board, "load", return_value=data), patch.dict(
            os.environ, {v.TRUST_ROOT_ENV: ""}, clear=False
        ), patch("sys.argv", ["task_board.py", "check"]):
            result = board.main()
        self.assertEqual(result, 1)

    def test_production_status_command_reports_unconfigured_root(self):
        _, status = v.production_verifier_from_environment({})
        self.assertEqual(status, "TRUST_ROOT_UNCONFIGURED")




def _test_canonical_digest(value, omitted):
    encoded = json.dumps(
        {key: item for key, item in value.items() if key != omitted},
        ensure_ascii=False, sort_keys=True, separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _test_windows_path_chain(path):
    normalized = ntpath.normpath(path.replace("/", "\\"))
    drive, tail = ntpath.splitdrive(normalized)
    current = drive + "\\"
    result = [current]
    for part in tail.strip("\\").split("\\"):
        if part:
            current = ntpath.join(current, part)
            result.append(current)
    return result


def _test_reparse_checks(*paths):
    checks = []
    seen = set()
    for path in paths:
        for component in _test_windows_path_chain(path):
            key = ntpath.normcase(component)
            if key not in seen:
                seen.add(key)
                checks.append({"path": component, "is_reparse_point": False})
    return checks


def _test_isolated_record(receipt, observations):
    binding_fields = (
        "repository", "source_sha", "contract_source_sha", "contract_merge_commit_sha",
        "verifier_pr_number", "verifier_source_sha", "verifier_merge_commit_sha",
        "verifier_sha256", "artifact_id", "artifact_sha256", "local_session_id",
        "workspace_nonce", "workspace_path",
    )
    return {
        "schema": v.CHECK_EVIDENCE_SCHEMA,
        "check": "isolated_storage",
        "status": "PASS",
        **{field: receipt[field] for field in binding_fields},
        "captured_at": "2026-10-30T12:15:00Z",
        "observations": observations,
    }


def make_verifier_review_test_fixture():
    mapping = copy.deepcopy(TEST_ONLY_TRUST_ROOT)
    mapping["schema"] = "cosos.fresh-evidence-trust-root.v2"
    mapping["artifact"].pop("attestations", None)
    _, api, _receipt, _, _ = make_fixture()
    verifier_diff = b"exact PR #141 diff bytes for the owner-attested verifier review"
    original_get_diff = api.get_diff

    def get_diff(pr_number):
        if pr_number == v.VERIFIER_PR_NUMBER:
            return api.verifier_diff
        return original_get_diff(pr_number)

    api.get_diff = get_diff
    source_sha = "e" * 40
    merge_sha = "d" * 40
    diff_sha = digest(verifier_diff)
    report = {
        "schema": "cosos.16fv.verifier-review.v1",
        "repository": REPO,
        "repository_id": v.REPOSITORY_ID,
        "pr_number": v.VERIFIER_PR_NUMBER,
        "source_sha": source_sha,
        "diff_sha256": diff_sha,
        "review_mode": v.REVIEW_MODE,
        "review_policy_semantics": "OWNER_ATTESTED_NOT_GITHUB_INDEPENDENT_APPROVAL",
        "model_authorship": "OWNER_ATTESTED_NOT_CRYPTOGRAPHICALLY_AUTHENTICATED",
        "verdict": "PASS",
        "owner_login": v.OWNER_LOGIN,
        "owner_user_id": OWNER_ID,
        "review_report_sha256": "",
    }
    report["review_report_sha256"] = _test_canonical_digest(report, "review_report_sha256")
    attestation = {
        "schema": "cosos.16fv.owner-attestation.v1",
        "repository": REPO,
        "repository_id": v.REPOSITORY_ID,
        "pr_number": v.VERIFIER_PR_NUMBER,
        "source_sha": source_sha,
        "diff_sha256": diff_sha,
        "review_report_sha256": report["review_report_sha256"],
        "review_mode": v.REVIEW_MODE,
        "owner_login": v.OWNER_LOGIN,
        "owner_user_id": OWNER_ID,
        "accepted": True,
        "owner_attestation_sha256": "",
    }
    attestation["owner_attestation_sha256"] = _test_canonical_digest(
        attestation, "owner_attestation_sha256"
    )
    mapping["verifier_review"] = {
        "pr_number": v.VERIFIER_PR_NUMBER,
        "source_sha": source_sha,
        "merge_commit_sha": merge_sha,
        "diff_sha256": diff_sha,
        "report_comment_id": 301,
        "report_sha256": report["review_report_sha256"],
        "owner_attestation_comment_id": 302,
        "owner_attestation_sha256": attestation["owner_attestation_sha256"],
        "review_mode": v.REVIEW_MODE,
    }
    api.objects[f"/repos/{REPO}/pulls/{v.VERIFIER_PR_NUMBER}"] = {
        "number": v.VERIFIER_PR_NUMBER,
        "merged": True,
        "merged_at": CREATED,
        "base": {"ref": "main", "repo": {"full_name": REPO, "id": v.REPOSITORY_ID}},
        "head": {"sha": source_sha, "ref": "work/16fv-01-trusted-evidence-verifier-20261030",
                 "repo": {"full_name": REPO, "id": v.REPOSITORY_ID}},
        "merge_commit_sha": merge_sha,
    }
    predecessor_merge = mapping["predecessor"]["merge_commit_sha"]
    api.objects[f"/repos/{REPO}/compare/{predecessor_merge}...main"] = {
        "base_commit": {"sha": predecessor_merge},
        "merge_base_commit": {"sha": predecessor_merge},
        "behind_by": 0,
        "status": "identical",
    }
    api.objects[f"/repos/{REPO}/compare/{predecessor_merge}...{source_sha}"] = {
        "base_commit": {"sha": predecessor_merge},
        "merge_base_commit": {"sha": predecessor_merge},
        "behind_by": 0,
        "status": "ahead",
    }
    api.objects[f"/repos/{REPO}/issues/comments/301"] = comment(
        301,
        marked("<!-- COSOS-16FV-REVIEW-V1 -->", report),
        issue=v.VERIFIER_PR_NUMBER,
    )
    api.objects[f"/repos/{REPO}/issues/comments/302"] = comment(
        302,
        marked("<!-- COSOS-16FV-OWNER-ATTESTATION-V1 -->", attestation),
        issue=v.VERIFIER_PR_NUMBER,
    )
    task = {
        "status": "done",
        "accepted_evidence_url": f"https://github.com/{REPO}/pull/{v.VERIFIER_PR_NUMBER}",
        "accepted_source_sha": source_sha,
        "accepted_merge_commit_sha": merge_sha,
        "accepted_verifier_sha256": digest(api.verifier_file),
    }
    return mapping, api, task


class FinalSecurityCorrectionRedTests(unittest.TestCase):
    def _load_verifier(self, mapping, api):
        with patch.dict(os.environ, production_environment(mapping), clear=True):
            with patch("tools.fresh_evidence_verifier.GitHubAPI", return_value=api):
                with patch(
                    "tools.fresh_evidence_verifier.verify_historical_16l1_artifact",
                    return_value={},
                ):
                    return v.production_verifier_from_environment()

    def test_trust_root_v2_accepts_a_separate_verifier_review_contract(self):
        mapping, _api, _task = make_verifier_review_test_fixture()
        try:
            root = v.TrustRoot.from_mapping(mapping, production=False)
        except Exception as exc:
            self.fail(f"v2 trust root with a complete verifier-review contract was rejected: {exc}")
        self.assertEqual(root.verifier_review["pr_number"], 141)
        self.assertEqual(root.verifier_review["review_mode"], "GPT6_OWNER_ATTESTED")

    def test_merged_verifier_task_rejects_missing_review_or_owner_acceptance(self):
        for missing_comment_id in (301, 302):
            with self.subTest(missing_comment_id=missing_comment_id):
                mapping, api, task = make_verifier_review_test_fixture()
                del api.objects[f"/repos/{REPO}/issues/comments/{missing_comment_id}"]
                verifier, status = self._load_verifier(mapping, api)
                self.assertEqual(status, "CONFIGURED")
                self.assertFalse(verifier.verify_verifier_task(task))

    def test_verifier_review_is_bound_to_actual_diff_bytes_and_predecessor_ancestry(self):
        mapping, api, task = make_verifier_review_test_fixture()
        api.verifier_diff += b"changed after the owner-attested review"
        verifier, status = self._load_verifier(mapping, api)
        self.assertEqual(status, "CONFIGURED")
        self.assertFalse(verifier.verify_verifier_task(task))

        mapping, api, task = make_verifier_review_test_fixture()
        predecessor_merge = mapping["predecessor"]["merge_commit_sha"]
        source_sha = mapping["verifier_review"]["source_sha"]
        api.objects[f"/repos/{REPO}/compare/{predecessor_merge}...{source_sha}"][
            "merge_base_commit"
        ]["sha"] = "0" * 40
        verifier, status = self._load_verifier(mapping, api)
        self.assertEqual(status, "CONFIGURED")
        self.assertFalse(verifier.verify_verifier_task(task))

    def test_trust_root_status_checks_remote_verifier_review_records(self):
        mapping, api, _task = make_verifier_review_test_fixture()
        del api.objects[f"/repos/{REPO}/issues/comments/301"]
        with patch.dict(os.environ, production_environment(mapping), clear=True):
            with patch("tools.fresh_evidence_verifier.GitHubAPI", return_value=api):
                with patch(
                    "tools.fresh_evidence_verifier.verify_historical_16l1_artifact",
                    return_value={},
                ):
                    self.assertEqual(v.trust_root_status(), "VERIFICATION_ERROR")

    def test_same_issue_number_from_another_repository_is_rejected(self):
        root, _api, _receipt, _, _ = make_fixture()
        foreign = comment(777, "foreign comment", issue=138)
        foreign["issue_url"] = "https://api.github.com/repos/another-owner/another-repo/issues/138"
        with self.assertRaises(v.VerificationError):
            v._check_unchanged_comment(foreign, 777, 138, root)

    def test_predecessor_merge_commit_must_be_an_ancestor_of_main(self):
        root, api, _receipt, _, _ = make_fixture()
        merge_sha = root.predecessor["merge_commit_sha"]
        api.objects[f"/repos/{REPO}/compare/{merge_sha}...main"] = {
            "base_commit": {"sha": merge_sha},
            "merge_base_commit": {"sha": "0" * 40},
            "behind_by": 1,
            "status": "diverged",
        }
        with self.assertRaises(v.VerificationError):
            v._verify_predecessor(root, api)

    def test_pr141_base_and_head_must_belong_to_the_canonical_repository(self):
        _root, api, receipt, _, _ = make_fixture()
        pr_path = f"/repos/{REPO}/pulls/{v.VERIFIER_PR_NUMBER}"
        api.objects[pr_path]["base"]["repo"] = {"full_name": "another-owner/another-repo", "id": 99}
        with self.assertRaises(v.VerificationError):
            v.verify_16fv_source(receipt, api)

        _root, api, receipt, _, _ = make_fixture()
        api.objects[pr_path]["head"]["repo"]["id"] = 99
        with self.assertRaises(v.VerificationError):
            v.verify_16fv_source(receipt, api)

    def test_windows_storage_rejects_dotdot_traversal_before_normalization(self):
        _root, _api, receipt, _, _ = make_fixture()
        observations = observations_for("isolated_storage", receipt)
        observations["profile_storage_root"] = (
            receipt["workspace_path"] + r"\fresh-profile\..\..\Users\ExistingProfile"
        )
        observations["canonical_path"] = observations["profile_storage_root"] + r"\storage"
        with self.assertRaises(v.VerificationError):
            v._validate_check_evidence(
                "isolated_storage", _test_isolated_record(receipt, observations), receipt
            )

    def test_windows_storage_rejects_mixed_separator_nested_traversal(self):
        _root, _api, receipt, _, _ = make_fixture()
        observations = observations_for("isolated_storage", receipt)
        observations["profile_storage_root"] = (
            receipt["workspace_path"] + r"\fresh-profile/../../Users/ExistingProfile"
        )
        observations["canonical_path"] = observations["profile_storage_root"] + r"\storage"
        with self.assertRaises(v.VerificationError):
            v._validate_check_evidence(
                "isolated_storage", _test_isolated_record(receipt, observations), receipt
            )

    def test_windows_storage_rejects_nested_escape_after_normalization(self):
        _root, _api, receipt, _, _ = make_fixture()
        observations = observations_for("isolated_storage", receipt)
        observations["canonical_path"] = (
            observations["profile_storage_root"] + r"\storage\..\..\outside"
        )
        with self.assertRaises(v.VerificationError):
            v._validate_check_evidence(
                "isolated_storage", _test_isolated_record(receipt, observations), receipt
            )

    def test_windows_storage_requires_explicit_reparse_point_evidence(self):
        _root, _api, receipt, _, _ = make_fixture()
        observations = observations_for("isolated_storage", receipt)
        observations.pop("reparse_point_check_method")
        observations.pop("reparse_point_checks")
        with self.assertRaises(v.VerificationError):
            v._validate_check_evidence(
                "isolated_storage", _test_isolated_record(receipt, observations), receipt
            )

    def test_windows_storage_case_variations_are_valid_under_windows_semantics(self):
        _root, _api, receipt, _, _ = make_fixture()
        workspace = receipt["workspace_path"]
        profile = workspace.swapcase() + r"\Fresh-Profile"
        canonical = profile + r"\Storage"
        observations = observations_for("isolated_storage", receipt)
        observations["profile_storage_root"] = profile
        observations["canonical_path"] = canonical
        observations["reparse_point_check_method"] = "GetFileAttributesW(FILE_ATTRIBUTE_REPARSE_POINT)"
        observations["reparse_point_checks"] = _test_reparse_checks(workspace, profile, canonical)
        try:
            v._validate_check_evidence(
                "isolated_storage", _test_isolated_record(receipt, observations), receipt
            )
        except Exception as exc:
            self.fail(f"case-insensitive Windows containment rejected a valid path: {exc}")

    def test_windows_storage_rejects_unc_different_drive_and_sibling_prefix_paths(self):
        _root, _api, receipt, _, _ = make_fixture()
        workspace = receipt["workspace_path"]
        invalid_roots = (
            r"\\server\share\fresh-profile",
            r"\\?\D:\CoSoS-16LF-01-Qualification\fresh-profile",
            r"\\.\D:\CoSoS-16LF-01-Qualification\fresh-profile",
            r"C:\CoSoS-16LF-01-Qualification\fresh-profile",
            workspace + r"-sibling\fresh-profile",
        )
        for invalid_root in invalid_roots:
            with self.subTest(invalid_root=invalid_root):
                observations = observations_for("isolated_storage", receipt)
                observations["profile_storage_root"] = invalid_root
                observations["canonical_path"] = invalid_root + r"\storage"
                with self.assertRaises(v.VerificationError):
                    v._validate_check_evidence(
                        "isolated_storage", _test_isolated_record(receipt, observations), receipt
                    )

    def test_sigstore_proof_must_bind_the_exact_subject_file_digest(self):
        root, api, runner = make_synthetic_artifact()

        def mismatched_subject(command, **kwargs):
            result = runner(command, **kwargs)
            proof = json.loads(result.stdout)
            proof[0]["verificationResult"]["statement"]["subject"] = [{
                "name": "unrelated.bin",
                "digest": {"sha256": "0" * 64},
            }]
            proof[0]["attestation"] = {"bundle": {"dsseEnvelope": {}}}
            return SimpleNamespace(returncode=0, stdout=json.dumps(proof), stderr="")

        with self.assertRaises(v.VerificationError):
            v.verify_historical_16l1_artifact(root, api, runner=mismatched_subject)

    def test_duplicate_sigstore_proof_records_are_rejected(self):
        root, api, runner = make_synthetic_artifact()

        def duplicate_proofs(command, **kwargs):
            result = runner(command, **kwargs)
            proof = json.loads(result.stdout)[0]
            proof["verificationResult"]["statement"]["subject"] = [{
                "name": Path(command[3]).name,
                "digest": {"sha256": digest(Path(command[3]).read_bytes())},
            }]
            proof["attestation"] = {"bundle": {"dsseEnvelope": {}}}
            return SimpleNamespace(returncode=0, stdout=json.dumps([proof, copy.deepcopy(proof)]), stderr="")

        with self.assertRaises(v.VerificationError):
            v.verify_historical_16l1_artifact(root, api, runner=duplicate_proofs)

    def test_historical_trust_root_does_not_pin_unverifiable_attestation_ids(self):
        self.assertNotIn("attestations", TEST_ONLY_TRUST_ROOT["artifact"])



if __name__ == "__main__":
    unittest.main()
