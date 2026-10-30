"""Regression checks for the canonical CoSoS DAG and execution policy."""
from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import task_board as board


class TaskBoardTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = board.load()
        cls.tasks = board.index(cls.data)

    def test_markdown_board_matches_machine_graph_and_candidate_status(self):
        text = (Path(__file__).resolve().parents[1] / "docs" / "PARALLEL-EXECUTION-BOARD.md").read_text(encoding="utf-8")
        table = text.split("## Full 16-phase task registry", 1)[1].split("## Parallel execution lanes", 1)[0]
        rows = {}
        for line in table.splitlines():
            if not line.startswith("| `"):
                continue
            parts = [part.strip() for part in line.strip().strip("|").split("|")]
            task_id = parts[0].strip("`")
            self.assertNotIn(task_id, rows)
            rows[task_id] = parts
        self.assertEqual(set(rows), set(self.tasks))
        for task_id, task in self.tasks.items():
            cols = rows[task_id]
            dependencies = [token.strip("`") for token in cols[2].split(", ")] if cols[2] != "NONE" else []
            self.assertEqual(dependencies, task["depends_on"], task_id)
            self.assertEqual(cols[3], "`" + task["resource_locks"][0] + "`", task_id)
            desired = "DONE" if task["status"] == "done" else ("HOLD" if task["status"] == "hold" else ("CLAIMED" if task["status"] == "claimed" else (
                "**READY · PARALLEL**" if not any(
                    self.tasks[dep]["status"] != "done" for dep in task["depends_on"]
                ) and task["status"] == "todo" and not task.get("any_of") else "BLOCKED"
            )))
            reported_status = cols[5]
            if task_id == "16L2":
                self.assertEqual(reported_status, "BLOCKED / NOT_RUN / NOT_AUTHORIZED")
                reported_status = "BLOCKED"
            self.assertEqual(reported_status, desired, task_id)

    def test_historical_aliases_are_explicit_and_never_reused_as_current_preflight(self):
        aliases = self.data["legacy_pre_2026_10_20_ids"]
        self.assertEqual(aliases["preflight"]["former 8A"], "7P-A")
        self.assertEqual(aliases["full_phase_8"]["former 8C"], "8A")
        self.assertNotEqual(self.tasks["8A"]["title"], self.tasks["7P-A"]["title"])

    def test_registry_is_valid_and_acyclic(self):
        self.assertEqual(board.validate(self.data), [])

    def test_all_work_units_preserved(self):
        kinds = {k: sum(t["kind"] == k for t in self.data["tasks"]) for k in "CLU"}
        self.assertEqual(kinds, {"C": 81, "L": 31, "U": 15})
        self.assertEqual(len(self.tasks), 127)

    def test_completed_native_feasibility_unblocks_7l1_without_waiving_future_gates(self):
        ready, _ = board.readiness(self.data)
        ready_ids = {task["id"] for task in ready}
        for task in ready:
            self.assertEqual(task["status"], "todo")
            self.assertTrue(all(self.tasks[dep]["status"] == "done" for dep in task["depends_on"]))
            if task["kind"] == "L":
                self.assertIn("physical-windows-host", board.effective_locks(task))
            if task["kind"] == "U":
                self.assertIn("human-owner", board.effective_locks(task))
        self.assertEqual(self.tasks["CL-L6"]["status"], "done")
        self.assertEqual(self.tasks["7P-A"]["status"], "done")
        self.assertEqual(self.tasks["7P-A"]["kind"], "C")
        self.assertNotIn("7P-A", ready_ids)
        self.assertNotIn("7P-B", ready_ids)
        self.assertEqual(self.tasks["7P-B"]["kind"], "C")
        self.assertEqual(self.tasks["7P-B"]["status"], "done")
        self.assertEqual(self.tasks["7P-L1"]["status"], "done")
        self.assertNotIn("7P-L1", ready_ids)
        self.assertEqual(board.explain(self.data, "7P-L1")["state"], "DONE")
        self.assertIn("physical-windows-host", board.effective_locks(self.tasks["7P-L1"]))
        self.assertEqual(self.tasks["7A"]["status"], "done")
        self.assertNotIn("7A", ready_ids)
        self.assertEqual(self.tasks["7B"]["status"], "done")
        self.assertNotIn("7B", ready_ids)
        self.assertEqual(self.tasks["7C"]["status"], "done")
        self.assertNotIn("7C", ready_ids)
        candidate = copy.deepcopy(self.data)
        for task in candidate["tasks"]:
            if task["id"] in ("7A", "7B"):
                task["status"] = "todo"
        eligible, _ = board.readiness(candidate)
        self.assertIn("7A", {t["id"] for t in eligible})
        # Native integration reached the independent physical Windows gate.
        # Status is accepted only from its exact-source receipt and review.
        self.assertEqual(self.tasks["7L1"]["status"], "done")
        self.assertNotIn("7L1", ready_ids)
        self.assertEqual(board.explain(self.data, "7L1")["state"], "DONE")
        reopened = copy.deepcopy(self.data)
        next(t for t in reopened["tasks"] if t["id"] == "7L1")["status"] = "todo"
        eligible_after_fix, _ = board.readiness(reopened)
        self.assertIn("7L1", {t["id"] for t in eligible_after_fix})
        self.assertIn("physical-windows-host", board.effective_locks(self.tasks["7L1"]))
        self.assertNotIn("7D", ready_ids)
        self.assertEqual(board.explain(self.data, "7D")["state"], "DONE")
        self.assertNotIn("7E", ready_ids)
        self.assertEqual(board.explain(self.data, "7E")["state"], "DONE")
        self.assertEqual(self.tasks["8A"]["status"], "done")
        self.assertNotIn("8A", ready_ids)
        self.assertEqual(board.explain(self.data, "8A")["state"], "DONE")
        for accepted in ("8B", "8C", "15A"):
            self.assertNotIn(accepted, ready_ids)
            self.assertEqual(board.explain(self.data, accepted)["state"], "DONE")
        self.assertEqual(board.explain(self.data, "8L1")["state"], "DONE")
        self.assertNotIn("8L1", ready_ids)
        self.assertNotIn("8U1", ready_ids)
        self.assertNotIn("15B", ready_ids)
        self.assertEqual(board.explain(self.data, "15B")["state"], "DONE")
        self.assertEqual(self.tasks["16L3"]["status"], "blocked")
        # Reopening architecture must also reopen downstream browser probe;
        # the local gate can never be offered before both are accepted.
        hypothetical = copy.deepcopy(self.data)
        for task in hypothetical["tasks"]:
            if task["id"] in ("7P-A", "7P-B", "7P-L1", "7L1"):
                task["status"] = "todo"
        gated, _ = board.readiness(hypothetical)
        gated_ids = {t["id"] for t in gated}
        self.assertIn("7P-A", gated_ids)
        self.assertNotIn("7P-B", gated_ids)
        self.assertNotIn("7P-L1", gated_ids)
        self.assertNotIn("7L1", gated_ids)

    def test_prerequisite_explanation_is_transitive(self):
        item = board.explain(self.data, "CL-09C")
        self.assertEqual(item["state"], "DONE")
        self.assertEqual(item["missing_direct"], [])
        self.assertIn("CL-L1", self.tasks["CL-09A"]["depends_on"])
        self.assertTrue(item["live_claim_check_required"])
        self.assertEqual(board.explain(self.data, "CL-05A")["state"], "DONE")

    def test_browser_preflight_is_in_phase_seven_not_eight(self):
        for task_id in ("7P-A", "7P-B", "7P-L1"):
            self.assertEqual(self.tasks[task_id]["phase"], "7")
        self.assertEqual(self.tasks["7P-A"]["depends_on"], ["CL-L6"])
        self.assertEqual(self.tasks["7A"]["depends_on"], ["7P-B"])
        self.assertEqual(self.tasks["7L1"]["depends_on"], ["7C", "7P-L1"])
        self.assertEqual(self.tasks["8A"]["depends_on"], ["7E"])
        self.assertEqual(self.tasks["8L1"]["depends_on"], ["8C"])
        self.assertEqual(self.tasks["8U1"]["depends_on"], ["8L1"])
        self.assertNotIn("8E", self.tasks)
        self.assertNotIn("8L2", self.tasks)

    def test_local_and_human_tasks_cannot_be_auto_claimed_as_cloud(self):
        self.assertIn("physical-windows-host", board.effective_locks(self.tasks["CL-L2"]))
        self.assertIn("human-owner", board.effective_locks(self.tasks["8U1"]))
        self.assertNotIn("physical-windows-host", board.effective_locks(self.tasks["CL-05A"]))

    def test_waves_do_not_skip_local_or_user_gates(self):
        waves = board.waves(self.data)
        ready, _ = board.readiness(self.data)
        self.assertEqual(bool(waves), bool(ready))
        if ready:
            # Eligible Cloud and Local tasks may be in the same future wave
            # when logical resource locks are disjoint (owner parallel rule).
            candidates = {task["id"] for task in ready}
            self.assertTrue({task["id"] for task in waves[0]} <= candidates)
            for a, first in enumerate(waves[0]):
                for second in waves[0][a+1:]:
                    self.assertFalse(
                        set(board.effective_locks(first)) & set(board.effective_locks(second))
                    )
        # A completed gate is excluded from forecasts. In a hypothetical
        # reopened native gate, every downstream phase must retain its order.
        future = copy.deepcopy(self.data)
        # Reopen the complete future browser/UI branch, including blocked
        # native gates, for this hypothetical successful forecast only.
        # Live 8L1 remains blocked and is tested separately below.
        for task in future["tasks"]:
            if task["id"] == "CL-L6" or task["phase"] in {str(n) for n in range(7, 17)}:
                task["status"] = "todo"
        # The private-migration alternative stays blocked until a dedicated
        # trusted verifier is implemented; the future forecast uses 16LF-01.
        future_tasks = board.index(future)
        future_tasks["16L2"]["status"] = "blocked"
        # Restore accepted prerequisites that are outside the hypothetical
        # reopened downstream branch.
        for prerequisite in ("16F", "16L1", "16U1"):
            future_tasks[prerequisite]["status"] = "done"
        # The predecessor contract remains merged; the hypothetical future
        # verifier is accepted only through the explicit remote-verification
        # test double, never by a task-registry Done flag alone.
        future_tasks["16FS-01"]["status"] = "done"
        future_tasks["16FS-01"]["accepted_evidence_url"] = "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/138"
        future_tasks["16FV-01"].update({
            "status": "done",
            "accepted_evidence_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/139",
            "accepted_source_sha": "e" * 40,
            "accepted_merge_commit_sha": "d" * 40,
            "accepted_verifier_sha256": "f" * 64,
        })
        verifier = self._trusted_test_verifier()
        self.assertEqual(board.validate(future, trusted_verifier=verifier), [])
        waves = board.waves(future, trusted_verifier=verifier)
        flat = [t["id"] for wave in waves for t in wave]
        self.assertLess(flat.index("CL-L6"), flat.index("7P-A"))
        self.assertLess(flat.index("7P-B"), flat.index("7A"))
        self.assertLess(flat.index("7P-L1"), flat.index("7L1"))
        self.assertLess(flat.index("7E"), flat.index("8A"))
        self.assertLess(flat.index("8U1"), flat.index("9A"))

    def test_blocked_native_gate_never_enters_forecast(self):
        self.assertEqual(self.tasks["8C"]["status"], "done")
        blocked = copy.deepcopy(self.data)
        next(t for t in blocked["tasks"] if t["id"] == "8L1")["status"] = "blocked"
        next(t for t in blocked["tasks"] if t["id"] == "8U1")["status"] = "todo"
        self.assertEqual(board.explain(blocked, "8L1")["state"], "BLOCKED")
        self.assertEqual(board.explain(blocked, "8U1")["missing_direct"], ["8L1"])
        forecast = {task["id"] for wave in board.waves(blocked) for task in wave}
        self.assertTrue({"8L1", "8U1", "9A"}.isdisjoint(forecast))
        self.assertNotIn("15B", forecast)  # 15B cloud scope accepted on merged PR #77
        self.assertIn("PERF-00", forecast)  # 15G cloud scope is now accepted
        self.assertNotIn("16A", forecast)  # 16A source/Notion acceptance is merged

    def test_native_8l1_acceptance_requires_both_physical_round_receipts(self):
        root = Path(__file__).resolve().parents[1] / "audit/2026-10-26/8l1-native-repair"
        accepted = json.loads((root / "qualification-pass.json").read_text(encoding="utf-8"))
        self.assertTrue(accepted["native8L1Accepted"])
        rounds = [json.loads((root / name).read_text(encoding="utf-8-sig"))
                  for name in accepted["roundReceipts"]]
        for receipt in rounds:
            self.assertEqual(receipt["sourceSha"], accepted["sourceSha"])
            self.assertEqual(receipt["hostBinarySha256"], accepted["hostBinarySha256"])
            self.assertEqual(receipt["rustBinarySha256"], accepted["rustBinarySha256"])
            for name in accepted["requiredNativeChecks"]:
                self.assertEqual(receipt["checks"][name]["status"], "PASS", name)
            self.assertFalse(any(v["status"] == "FAIL" for v in receipt["checks"].values()))
            self.assertEqual(receipt["permissionCounters"]["StaleAllowDenied"], 1)
            self.assertEqual(receipt["permissionCounters"]["ReplayDenied"], 1)
            self.assertEqual(receipt["permissionCounters"]["Busy"], 0)
            self.assertEqual(receipt["rustProcessExit"], 0)
        for name in ("hostProcessId", "rustProcessId"):
            self.assertGreater(rounds[0][name], 0)
            self.assertGreater(rounds[1][name], 0)
            self.assertNotEqual(rounds[0][name], rounds[1][name])
        for name in ("profileAPath", "profileBPath", "extensionAId", "extensionBId",
                     "syntheticMarkerA", "syntheticMarkerB"):
            self.assertTrue(rounds[0][name])
            self.assertEqual(rounds[0][name], rounds[1][name])
        restart = json.loads((root / accepted["restartReceipt"]).read_text(encoding="utf-8-sig"))
        for name in ("cleanProcessRelaunch", "retainedProfileAndRustRecovery",
                     "osCrashQualification", "retainedProfileQualification"):
            self.assertEqual(restart[name], "PASS")
        self.assertEqual(self.tasks["8U1"]["status"], "blocked")

    def test_dangling_dependency_is_rejected(self):
        data = copy.deepcopy(self.data)
        next(t for t in data["tasks"] if t["id"] == "CL-05A")["depends_on"].append("NEVER-EXISTED")
        self.assertTrue(any("dangling" in e for e in board.validate(data)))

    def test_dangling_or_dependency_is_rejected(self):
        data = copy.deepcopy(self.data)
        board.index(data)["16L3"]["any_of"] = [["16L2", "UNKNOWN-OR-GATE"]]
        self.assertTrue(any("dangling OR" in e for e in board.validate(data)))

    def test_cycle_is_rejected(self):
        data = copy.deepcopy(self.data)
        next(t for t in data["tasks"] if t["id"] == "CL-09A")["depends_on"].append("CL-09C")
        self.assertTrue(any("cycle" in e.lower() for e in board.validate(data)))

    def test_done_cannot_depend_on_unfinished_task(self):
        data = copy.deepcopy(self.data)
        # Explicitly break the prerequisite regardless of the live acceptance
        # snapshot: CL-L3 and CL-07B can both legitimately become done.
        next(t for t in data["tasks"] if t["id"] == "CL-07B")["status"] = "todo"
        next(t for t in data["tasks"] if t["id"] == "CL-L3")["status"] = "done"
        self.assertTrue(any("cannot be done" in e for e in board.validate(data)))

    def test_legacy_phase_ordering_cannot_regress(self):
        data = copy.deepcopy(self.data)
        next(t for t in data["tasks"] if t["id"] == "7A")["depends_on"] = ["8L1"]
        self.assertTrue(any("required phase ordering" in e for e in board.validate(data)))

    def test_completed_units_are_not_reoffered_as_ready(self):
        ready, _ = board.readiness(self.data)
        offered = {t["id"] for t in ready}
        self.assertTrue({"CL-04A", "CL-04B", "CL-04C", "CL-L1", "CL-05A", "CL-06A", "CL-07A", "CL-08A", "CL-09A", "CL-05B", "CL-06B", "CL-09B", "CL-09C", "CL-07B", "CL-08B"}.isdisjoint(offered))



    def _fresh_scenario(self):
        data = copy.deepcopy(self.data)
        tasks = board.index(data)
        # Well-formed but intentionally fabricated fixture; never authoritative.
        for key in ("16E", "16F", "16PKG-01", "16L1", "16U1", "16FS-01", "16FV-01"):
            tasks[key]["status"] = "done"
        tasks["16FV-01"].update({
            "accepted_evidence_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/139",
            "accepted_source_sha": "e" * 40,
            "accepted_merge_commit_sha": "d" * 40,
            "accepted_verifier_sha256": "f" * 64,
        })
        tasks["16L3"]["status"] = "todo"
        tasks["16LF-01"]["status"] = "done"
        sha = "a" * 40
        diff = "d" * 64
        tasks["16LF-01"]["evidence"] = {
            "schema_version": board.FRESH_RECEIPT_SCHEMA_VERSION,
            "kind": "owner-local-synthetic-fresh-start",
            "host": "owner-local-windows",
            "profile_scope": "new-disposable-empty",
            "migration": "none",
            "review_mode": board.FRESH_REVIEW_MODE,
            "owner_login": "KaisAbiyyi",
            "source_sha": sha,
            "reviewed_source_sha": sha,
            "diff_sha256": diff,
            "reviewed_diff_sha256": diff,
            "artifact_id": 12868909286,
            "artifact_sha256": "b" * 64,
            "artifact_source_sha": sha,
            "artifact_provenance_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/129#issuecomment-1",
            "artifact_provenance_sha256": "f" * 64,
            "receipt_sha256": "c" * 64,
            "local_session_id": "synthetic-test-fixture-only",
            "repository": "KaisAbiyyi/chat-on-steroids-on-steroids",
            "repository_id": 1424422098,
            "review_pr_number": 135,
            "contract_source_sha": "c392b076457ca5375e260bb2267950feac31de50",
            "contract_merge_commit_sha": "cc180575498e9f45d13c05c86a0a3bb4e8e0c1be",
            "verifier_pr_number": 139,
            "verifier_merge_commit_sha": "d" * 40,
            "evidence_bundle_url": "https://github.com/user-attachments/assets/12345678-1234-1234-1234-123456789abc",
            "evidence_bundle_sha256": "3" * 64,
            "windows_build": "Windows 11 10.0.26300 x64",
            "workspace_nonce": "a" * 16,
            "workspace_path": "D:\\CoSoS-16LF-01-Qualification\\" + "a" * 16,
            "verifier_source_sha": "e" * 40,
            "verifier_sha256": "f" * 64,
            "started_at": "2026-10-30T12:00:00Z",
            "finished_at": "2026-10-30T12:30:00Z",
            "commands": ["pwsh -File qualification.ps1", "python -m unittest"],
            "check_evidence": {key: "checks/" + key + ".json" for key in board.FRESH_CHECKS},
            "receipt_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/140#issuecomment-100",
            "review_report_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/140#issuecomment-101",
            "review_report_sha256": "1" * 64,
            "owner_attestation_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/140#issuecomment-102",
            "owner_attestation_sha256": "2" * 64,
            "checks": {key: "PASS" for key in board.FRESH_CHECKS},
        }
        return data, tasks

    def _trusted_test_verifier(self, **overrides):
        # Test-only double for the explicit typed boundary. It cannot be
        # imported by the production CLI and authenticates no real evidence.
        def verify(receipt):
            self.assertTrue(board.fresh_receipt_structurally_valid(receipt))
            report_sha = "1" * 64
            facts = {name: True for name in board.FRESH_VERIFICATION_FIELDS}
            facts.update({
                "verified_review_mode": receipt["review_mode"],
                "verified_owner_login": receipt["owner_login"],
                "verified_receipt_record_identity": "github:issue-comment:100",
                "verified_review_report_record_identity": "github:issue-comment:101",
                "verified_owner_attestation_record_identity": "github:issue-comment:102",
                "verified_owner_attestation_author_login": receipt["owner_login"],
                "verified_receipt_url": receipt["receipt_url"],
                "verified_receipt_sha256": receipt["receipt_sha256"],
                "verified_review_report_url": receipt["review_report_url"],
                "verified_review_report_sha256": report_sha,
                "verified_review_report_verdict": "PASS",
                "verified_review_report_source_sha": receipt["source_sha"],
                "verified_review_report_diff_sha256": receipt["diff_sha256"],
                "verified_review_report_artifact_sha256": receipt["artifact_sha256"],
                "verified_owner_attestation_url": receipt["owner_attestation_url"],
                "verified_owner_attestation_sha256": "2" * 64,
                "verified_owner_attested_review_report_sha256": report_sha,
                "verified_owner_attested_source_sha": receipt["source_sha"],
                "verified_source_sha": receipt["source_sha"],
                "verified_reviewed_source_sha": receipt["reviewed_source_sha"],
                "verified_diff_sha256": receipt["diff_sha256"],
                "verified_reviewed_diff_sha256": receipt["reviewed_diff_sha256"],
                "verified_artifact_id": receipt["artifact_id"],
                "verified_artifact_sha256": receipt["artifact_sha256"],
                "verified_artifact_source_sha": receipt["artifact_source_sha"],
                "verified_artifact_provenance_url": receipt["artifact_provenance_url"],
                "verified_artifact_provenance_sha256": receipt["artifact_provenance_sha256"],
                "verified_fresh_start_checks": board.FRESH_CHECKS,
                "verified_repository_full_name": receipt["repository"],
                "verified_repository_id": receipt["repository_id"],
                "verified_review_pr_number": receipt["review_pr_number"],
                "verified_review_pr_head_sha": receipt["source_sha"],
                "verified_review_pr_diff_sha256": receipt["diff_sha256"],
                "verified_owner_user_id": 93252311,
                "verified_review_report_schema": "cosos.release.cloud-provenance.v2",
                "verified_review_report_author_user_id": 93252311,
                "verified_receipt_author_user_id": 93252311,
                "verified_owner_attestation_author_user_id": 93252311,
                "verified_contract_source_sha": receipt["contract_source_sha"],
                "verified_contract_merge_commit_sha": receipt["contract_merge_commit_sha"],
                "verified_verifier_pr_number": receipt["verifier_pr_number"],
                "verified_verifier_source_sha": receipt["verifier_source_sha"],
                "verified_verifier_merge_commit_sha": receipt["verifier_merge_commit_sha"],
                "verified_verifier_sha256": receipt["verifier_sha256"],
            })
            facts.update(overrides)
            return board.FreshEvidenceVerification(**facts)

        def verify_verifier_task(task):
            return (
                task.get("accepted_evidence_url") == "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/139"
                and task.get("accepted_source_sha") == "e" * 40
                and task.get("accepted_merge_commit_sha") == "d" * 40
                and task.get("accepted_verifier_sha256") == "f" * 64
            )

        verify.verify_verifier_task = verify_verifier_task
        return verify

    def test_16fv_done_requires_remote_merged_pr_and_source_digest(self):
        data, tasks = self._fresh_scenario()
        verifier = self._trusted_test_verifier()
        self.assertTrue(board.accepted(tasks, "16FV-01", trusted_verifier=verifier))
        tasks["16FV-01"]["accepted_merge_commit_sha"] = "0" * 40
        self.assertFalse(board.accepted(tasks, "16FV-01", trusted_verifier=verifier))
        self.assertFalse(board.accepted(tasks, "16FV-01"))
        self.assertTrue(any(
            "16FV-01: Done status requires verified merged PR #139" in error
            for error in board.validate(data)
        ))

    def test_well_formed_fabricated_receipt_is_only_structural(self):
        _, tasks = self._fresh_scenario()
        receipt = tasks["16LF-01"]["evidence"]
        self.assertTrue(board.fresh_receipt_structurally_valid(receipt))
        self.assertEqual(
            board.fresh_receipt_status(receipt),
            "TRUST_ROOT_UNCONFIGURED",
        )
        self.assertFalse(board.accepted(tasks, "16LF-01"))
        self.assertIn(
            "one_of(16L2|16LF-01)",
            board.missing_dependencies(tasks["16L3"], tasks),
        )

    def test_fresh_receipt_rejects_boolean_or_float_schema_version(self):
        _, tasks = self._fresh_scenario()
        good = copy.deepcopy(tasks["16LF-01"]["evidence"])
        for invalid in (True, 2.0):
            broken = copy.deepcopy(good)
            broken["schema_version"] = invalid
            self.assertFalse(board.fresh_receipt_structurally_valid(broken))

    def test_legacy_independent_review_contract_is_rejected(self):
        _, tasks = self._fresh_scenario()
        receipt = copy.deepcopy(tasks["16LF-01"]["evidence"])
        receipt["independent_review_verdict"] = "PASS"
        self.assertFalse(board.fresh_receipt_structurally_valid(receipt))

    def test_fabricated_github_review_url_cannot_establish_acceptance(self):
        _, tasks = self._fresh_scenario()
        receipt = tasks["16LF-01"]["evidence"]
        self.assertIn("/issues/140#issuecomment-", receipt["review_report_url"])
        self.assertTrue(board.fresh_receipt_structurally_valid(receipt))
        self.assertFalse(board.accepted(tasks, "16LF-01"))

    def test_missing_trust_root_fails_closed(self):
        _, tasks = self._fresh_scenario()
        receipt = tasks["16LF-01"]["evidence"]
        self.assertEqual(
            board.fresh_receipt_status(receipt),
            "TRUST_ROOT_UNCONFIGURED",
        )
        self.assertFalse(board.accepted(tasks, "16LF-01"))

    def test_missing_review_report_is_rejected(self):
        _, tasks = self._fresh_scenario()
        receipt = copy.deepcopy(tasks["16LF-01"]["evidence"])
        del receipt["review_report_url"]
        self.assertFalse(board.fresh_receipt_structurally_valid(receipt))
        verifier = self._trusted_test_verifier(review_report_exists=False)
        self.assertFalse(board.accepted(tasks, "16LF-01", trusted_verifier=verifier))

    def test_missing_owner_attestation_is_rejected(self):
        _, tasks = self._fresh_scenario()
        receipt = copy.deepcopy(tasks["16LF-01"]["evidence"])
        del receipt["owner_attestation_url"]
        self.assertFalse(board.fresh_receipt_structurally_valid(receipt))
        for overrides in (
            {"owner_attestation_exists": False},
            {"owner_attestation_valid": False},
        ):
            verifier = self._trusted_test_verifier(**overrides)
            self.assertFalse(board.accepted(
                tasks, "16LF-01", trusted_verifier=verifier
            ))

    def test_wrong_owner_identity_is_rejected(self):
        _, tasks = self._fresh_scenario()
        for overrides in (
            {"owner_identity_verified": False},
            {"verified_owner_login": "unauthorized-owner"},
        ):
            verifier = self._trusted_test_verifier(**overrides)
            self.assertFalse(board.accepted(
                tasks, "16LF-01", trusted_verifier=verifier
            ))

    def test_unsupported_review_mode_is_rejected(self):
        _, tasks = self._fresh_scenario()
        receipt = copy.deepcopy(tasks["16LF-01"]["evidence"])
        receipt["review_mode"] = "GITHUB_INDEPENDENT"
        self.assertFalse(board.fresh_receipt_structurally_valid(receipt))
        verifier = self._trusted_test_verifier(
            review_mode_authorized=False,
            verified_review_mode="GITHUB_INDEPENDENT",
        )
        self.assertFalse(board.accepted(
            tasks, "16LF-01", trusted_verifier=verifier
        ))

    def test_different_reviewed_source_sha_is_rejected(self):
        _, tasks = self._fresh_scenario()
        receipt = copy.deepcopy(tasks["16LF-01"]["evidence"])
        receipt["reviewed_source_sha"] = "d" * 40
        self.assertFalse(board.fresh_receipt_structurally_valid(receipt))
        verifier = self._trusted_test_verifier(
            reviewed_source_sha_matches=False,
            verified_reviewed_source_sha="d" * 40,
            verified_review_report_source_sha="d" * 40,
        )
        self.assertFalse(board.accepted(
            tasks, "16LF-01", trusted_verifier=verifier
        ))

    def test_wrong_reviewed_diff_digest_is_rejected(self):
        _, tasks = self._fresh_scenario()
        receipt = copy.deepcopy(tasks["16LF-01"]["evidence"])
        receipt["reviewed_diff_sha256"] = "e" * 64
        self.assertFalse(board.fresh_receipt_structurally_valid(receipt))
        verifier = self._trusted_test_verifier(
            reviewed_diff_digest_matches=False,
            verified_reviewed_diff_sha256="e" * 64,
            verified_review_report_diff_sha256="e" * 64,
        )
        self.assertFalse(board.accepted(
            tasks, "16LF-01", trusted_verifier=verifier
        ))

    def test_artifact_digest_mismatch_is_rejected(self):
        _, tasks = self._fresh_scenario()
        for overrides in (
            {"receipt_review_artifact_digests_match": False},
            {"verified_review_report_artifact_sha256": "e" * 64},
        ):
            verifier = self._trusted_test_verifier(**overrides)
            self.assertFalse(board.accepted(
                tasks, "16LF-01", trusted_verifier=verifier
            ))

    def test_missing_artifact_is_rejected(self):
        _, tasks = self._fresh_scenario()
        verifier = self._trusted_test_verifier(artifact_exists=False)
        self.assertFalse(board.accepted(
            tasks, "16LF-01", trusted_verifier=verifier
        ))

    def test_missing_provenance_or_source_binding_is_rejected(self):
        _, tasks = self._fresh_scenario()
        for field in ("artifact_provenance_verified", "artifact_source_binding_verified"):
            with self.subTest(field=field):
                verifier = self._trusted_test_verifier(**{field: False})
                self.assertFalse(board.accepted(
                    tasks, "16LF-01", trusted_verifier=verifier
                ))

    def test_self_reported_receipt_fields_never_establish_acceptance(self):
        _, tasks = self._fresh_scenario()
        receipt = tasks["16LF-01"]["evidence"]
        for name in board.FRESH_VERIFICATION_FIELDS:
            receipt[name] = True
        self.assertTrue(board.fresh_receipt_structurally_valid(receipt))
        self.assertEqual(
            board.fresh_receipt_status(receipt),
            "TRUST_ROOT_UNCONFIGURED",
        )
        self.assertFalse(board.accepted(tasks, "16LF-01"))

    def test_gpt6_owner_attested_mode_is_not_github_independent_review(self):
        _, tasks = self._fresh_scenario()
        receipt = tasks["16LF-01"]["evidence"]
        self.assertEqual(receipt["review_mode"], "GPT6_OWNER_ATTESTED")
        self.assertNotIn("reviewer_independent", board.FRESH_VERIFICATION_FIELDS)
        self.assertNotIn("independent_review_verdict", receipt)

    def test_every_trusted_verification_fact_is_mandatory(self):
        _, tasks = self._fresh_scenario()
        for field in board.FRESH_VERIFICATION_FIELDS:
            with self.subTest(field=field):
                verifier = self._trusted_test_verifier(**{field: False})
                self.assertFalse(board.accepted(
                    tasks, "16LF-01", trusted_verifier=verifier
                ))

    def test_trusted_verifier_must_bind_report_owner_attestation_and_artifact(self):
        _, tasks = self._fresh_scenario()
        wrong_url = "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/999#issuecomment-9"
        for field, value in [
            ("verified_review_report_url", wrong_url),
            ("verified_review_report_sha256", "3" * 64),
            ("verified_owner_attestation_sha256", "4" * 64),
            ("verified_review_report_verdict", "FAIL"),
            ("verified_review_report_source_sha", "d" * 40),
            ("verified_review_report_diff_sha256", "e" * 64),
            ("verified_review_report_artifact_sha256", "a" * 64),
            ("verified_owner_attestation_url", wrong_url),
            ("verified_owner_attested_review_report_sha256", "2" * 64),
            ("verified_owner_attested_source_sha", "d" * 40),
            ("verified_receipt_sha256", "f" * 64),
            ("verified_reviewed_source_sha", "d" * 40),
            ("verified_reviewed_diff_sha256", "e" * 64),
            ("verified_artifact_id", 42),
            ("verified_artifact_sha256", "e" * 64),
            ("verified_artifact_source_sha", "d" * 40),
            ("verified_artifact_provenance_url", wrong_url),
            ("verified_artifact_provenance_sha256", "a" * 64),
            ("verified_owner_login", "unauthorized-owner"),
            ("verified_owner_attestation_author_login", "unauthorized-owner"),
        ]:
            with self.subTest(field=field):
                verifier = self._trusted_test_verifier(**{field: value})
                self.assertFalse(board.accepted(
                    tasks, "16LF-01", trusted_verifier=verifier
                ))

    def test_trusted_verifier_errors_fail_closed(self):
        _, tasks = self._fresh_scenario()

        def broken_verifier(_receipt):
            raise RuntimeError("external evidence service unavailable")

        self.assertEqual(
            board.fresh_receipt_status(
                tasks["16LF-01"]["evidence"], trusted_verifier=broken_verifier
            ),
            "VERIFICATION_ERROR",
        )
        self.assertFalse(board.accepted(
            tasks, "16LF-01", trusted_verifier=broken_verifier
        ))

    def test_untrusted_boolean_verifier_result_is_rejected(self):
        _, tasks = self._fresh_scenario()
        self.assertEqual(
            board.fresh_receipt_status(
                tasks["16LF-01"]["evidence"], trusted_verifier=lambda _receipt: True
            ),
            "REJECTED_EXTERNAL_VERIFICATION",
        )

    def test_trusted_verifier_path_accepts_complete_gpt6_owner_attested_evidence(self):
        data, tasks = self._fresh_scenario()
        verifier = self._trusted_test_verifier()
        self.assertEqual(
            board.fresh_receipt_status(
                tasks["16LF-01"]["evidence"], trusted_verifier=verifier
            ),
            "ACCEPTED",
        )
        self.assertTrue(board.accepted(
            tasks, "16LF-01", trusted_verifier=verifier
        ))
        self.assertEqual(
            board.missing_dependencies(
                tasks["16L3"], tasks, trusted_verifier=verifier
            ),
            [],
        )
        self.assertEqual(board.validate(data, trusted_verifier=verifier), [])
        eligible = {t["id"] for wave in board.waves(
            data, trusted_verifier=verifier
        ) for t in wave}
        self.assertIn("16L3", eligible)

    def test_explain_respects_satisfied_or_dependency_group(self):
        data, tasks = self._fresh_scenario()

        unverified = board.explain(data, "16L3")
        self.assertIn("16L2", unverified["missing_transitive"])
        self.assertIn("16LF-01", unverified["missing_transitive"])

        # This test-only verifier exercises the accepted alternative path. It
        # does not authenticate a real GitHub review, artifact or Windows run.
        verified = board.explain(
            data, "16L3", trusted_verifier=self._trusted_test_verifier()
        )
        self.assertEqual(verified["state"], "READY_CANDIDATE")
        self.assertEqual(verified["missing_direct"], [])
        self.assertNotIn("16L2", verified["missing_transitive"])
        self.assertNotIn("16LF-01", verified["missing_transitive"])

    def test_canonical_registry_remains_blocked_without_trusted_acceptance(self):
        self.assertEqual(self.tasks["16LF-01"]["status"], "blocked")
        self.assertFalse(board.accepted(self.tasks, "16LF-01"))
        self.assertEqual(board.explain(self.data, "16L3")["state"], "BLOCKED")
        self.assertEqual(
            board.explain(self.data, "16L3")["fresh_evidence_status"],
            "MISSING",
        )
        self.assertIn(
            "one_of(16L2|16LF-01)",
            board.missing_dependencies(self.tasks["16L3"], self.tasks),
        )
        eligible = {t["id"] for wave in board.waves(self.data) for t in wave}
        self.assertNotIn("16L3", eligible)

    def test_declined_owner_decision_and_migration_status_cannot_drift(self):
        data, tasks = self._fresh_scenario()
        tasks["16U1"]["owner_decision"] = "approved_private_copy"
        self.assertTrue(any("fresh_start owner decision" in e for e in board.validate(data)))

        tasks["16U1"]["owner_decision"] = "fresh_start"
        tasks["16L2"]["private_migration_status"] = "DONE"
        self.assertTrue(any("NOT_RUN / NOT_AUTHORIZED" in e for e in board.validate(data)))

    def test_owner_declined_never_grants_private_profile_access_or_readiness(self):
        data, tasks = self._fresh_scenario()
        self.assertEqual(tasks["16U1"]["migration_consent"], "declined")
        tasks["16L2"]["status"] = "done"
        tasks["16L2"]["private_migration_receipt"] = {
            "scope": "approved-copy-only", "physical_windows": True,
            "independent_review_verdict": "PASS",
        }
        self.assertFalse(board.accepted(tasks, "16L2"))
        self.assertTrue(any("16L2" in e for e in board.validate(data)))

        # A task-state edit must not turn the declined private migration into
        # a ready/forecastable local operation.
        tasks["16L2"]["status"] = "todo"
        self.assertIn(
            "owner_private_migration_consent",
            board.missing_dependencies(tasks["16L2"], tasks),
        )
        ready, blocked = board.readiness(data)
        self.assertNotIn("16L2", {task["id"] for task in ready})
        self.assertIn("16L2", {task["id"] for task in blocked})
        eligible = {
            task["id"] for wave in board.waves(data) for task in wave
        }
        self.assertNotIn("16L2", eligible)
        self.assertTrue(any("16L2:" in e for e in board.validate(data)))

    def test_self_reported_private_migration_receipt_never_establishes_acceptance(self):
        data, tasks = self._fresh_scenario()
        tasks["16U1"]["migration_consent"] = "granted"
        tasks["16U1"]["owner_decision"] = "approved_private_copy"
        tasks["16U1"]["consent_receipt"] = {
            "explicit_owner_approval": True,
            "scope": "approved-copy-only",
            "approval_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/137#issuecomment-1",
        }
        tasks["16L2"]["status"] = "done"
        tasks["16L2"]["private_migration_receipt"] = {
            "scope": "approved-copy-only",
            "physical_windows": True,
            "independent_review_verdict": "PASS",
        }

        self.assertTrue(board.private_migration_authorized(tasks))
        self.assertFalse(board.accepted(tasks, "16L2"))
        self.assertTrue(any("trusted private-migration verifier" in e for e in board.validate(data)))

    def test_private_migration_remains_blocked_until_trusted_verifier_exists(self):
        data, tasks = self._fresh_scenario()
        tasks["16U1"]["migration_consent"] = "granted"
        tasks["16U1"]["owner_decision"] = "approved_private_copy"
        tasks["16U1"]["consent_receipt"] = {
            "explicit_owner_approval": True,
            "scope": "approved-copy-only",
            "approval_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/137#issuecomment-1",
        }
        tasks["16L2"]["status"] = "todo"

        tasks["16U1"]["consent_receipt"]["scope"] = "all-private-profiles"
        self.assertFalse(board.private_migration_authorized(tasks))
        self.assertIn(
            "owner_private_migration_consent",
            board.missing_dependencies(tasks["16L2"], tasks),
        )
        tasks["16U1"]["consent_receipt"]["scope"] = "approved-copy-only"
        self.assertTrue(board.private_migration_authorized(tasks))
        self.assertIn(
            "trusted_private_migration_verifier",
            board.missing_dependencies(tasks["16L2"], tasks),
        )
        ready, blocked = board.readiness(data)
        self.assertNotIn("16L2", {task["id"] for task in ready})
        self.assertIn("16L2", {task["id"] for task in blocked})
        eligible = {
            task["id"] for wave in board.waves(data) for task in wave
        }
        self.assertNotIn("16L2", eligible)
        self.assertTrue(any("trusted private-migration verifier" in e for e in board.validate(data)))

    def test_wave_forecast_requires_trusted_accepted_or_branch(self):
        data, tasks = self._fresh_scenario()
        verifier = self._trusted_test_verifier()
        eligible = {t["id"] for wave in board.waves(
            data, trusted_verifier=verifier
        ) for t in wave}
        self.assertIn("16L3", eligible)
        untrusted = self._trusted_test_verifier(evidence_complete=False)
        blocked = {t["id"] for wave in board.waves(
            data, trusted_verifier=untrusted
        ) for t in wave}
        self.assertNotIn("16L3", blocked)

    def test_missing_freshstart_evidence_and_dependency_bypass_rejected(self):
        data, tasks = self._fresh_scenario()
        tasks["16LF-01"]["evidence"] = None
        self.assertIn("one_of(16L2|16LF-01)", board.missing_dependencies(tasks["16L3"], tasks))
        self.assertTrue(any("16LF-01" in e for e in board.validate(data)))
        tasks["16L3"]["any_of"] = []
        self.assertTrue(any("16L3:" in e for e in board.validate(data)))
        tasks["16L3"]["any_of"] = [["16LF-01", "16L2"]]
        self.assertTrue(any("16L3:" in e for e in board.validate(data)))

    def test_incomplete_and_forged_freshstart_proofs_fail_closed(self):
        _, tasks = self._fresh_scenario()
        good = tasks["16LF-01"]["evidence"]
        for field, value in [
            ("source_sha", "fake"),
            ("reviewed_source_sha", "d" * 40),
            ("artifact_sha256", "broken"),
            ("review_mode", "GITHUB_INDEPENDENT"),
            ("review_report_url", "https://evil.invalid/review"),
            ("host", "github-hosted-windows"),
            ("migration", "private-copy"),
        ]:
            broken = copy.deepcopy(good)
            broken[field] = value
            self.assertFalse(board.fresh_receipt_structurally_valid(broken), field)
        broken = copy.deepcopy(good)
        broken["checks"]["rollback"] = "FAIL"
        self.assertFalse(board.fresh_receipt_structurally_valid(broken))
        broken = copy.deepcopy(good)
        del broken["checks"]["owned_cleanup"]
        self.assertFalse(board.fresh_receipt_structurally_valid(broken))

    def test_release_gate_preserves_16f_16l1_16u2_and_physical_gate(self):
        _, tasks = self._fresh_scenario()
        for prereq in ("16F", "16L1"):
            tasks[prereq]["status"] = "todo"
            self.assertIn(prereq, board.missing_dependencies(tasks["16L3"], tasks))
            tasks[prereq]["status"] = "done"
        tasks["16LF-01"]["status"] = "blocked"
        self.assertIn("one_of(16L2|16LF-01)", board.missing_dependencies(tasks["16L3"], tasks))
        self.assertFalse(board.accepted(tasks, "16LF-01"))
        self.assertEqual(tasks["16U2"]["depends_on"], ["16L3"])
        self.assertEqual(tasks["16L3"]["status"], "todo")
        self.assertEqual(board.explain(self.data, "16U2")["state"], "BLOCKED")

    def test_freshstart_or_edge_cycles_are_rejected(self):
        data = copy.deepcopy(self.data)
        board.index(data)["16LF-01"]["depends_on"].append("16L3")
        self.assertTrue(any("cycle" in e.lower() for e in board.validate(data)))


    def test_review_report_and_owner_attestation_must_be_distinct_records(self):
        _, tasks = self._fresh_scenario()
        verifier = self._trusted_test_verifier(
            verified_owner_attestation_record_identity="github:issue-comment:101"
        )
        self.assertFalse(board.accepted(
            tasks, "16LF-01", trusted_verifier=verifier
        ))

    def test_receipt_and_owner_attestation_must_be_distinct_records(self):
        _, tasks = self._fresh_scenario()
        verifier = self._trusted_test_verifier(
            verified_owner_attestation_record_identity="github:issue-comment:100"
        )
        self.assertFalse(board.accepted(
            tasks, "16LF-01", trusted_verifier=verifier
        ))

    def test_different_url_forms_cannot_disguise_the_same_provider_record(self):
        _, tasks = self._fresh_scenario()
        receipt = tasks["16LF-01"]["evidence"]
        base = "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids"
        receipt["review_report_url"] = f"{base}/pull/138#issuecomment-4242"
        receipt["owner_attestation_url"] = f"{base}/issues/137#issuecomment-4242"
        self.assertNotEqual(
            receipt["review_report_url"], receipt["owner_attestation_url"]
        )
        verifier = self._trusted_test_verifier(
            verified_review_report_record_identity="github:issue-comment:4242",
            verified_owner_attestation_record_identity="github:issue-comment:4242",
        )
        self.assertFalse(board.accepted(
            tasks, "16LF-01", trusted_verifier=verifier
        ))

    def test_owner_attestation_author_must_be_verified_and_match_authorized_owner(self):
        _, tasks = self._fresh_scenario()
        for overrides in (
            {"verified_owner_attestation_author_login": None},
            {"verified_owner_attestation_author_login": ""},
            {"owner_attestation_author_identity_verified": False},
        ):
            with self.subTest(overrides=overrides):
                verifier = self._trusted_test_verifier(**overrides)
                self.assertFalse(board.accepted(
                    tasks, "16LF-01", trusted_verifier=verifier
                ))

    def test_valid_owner_attestation_uses_three_distinct_records_and_authorized_author(self):
        _, tasks = self._fresh_scenario()
        receipt = tasks["16LF-01"]["evidence"]
        verification = self._trusted_test_verifier()(receipt)
        identities = (
            verification.verified_receipt_record_identity,
            verification.verified_review_report_record_identity,
            verification.verified_owner_attestation_record_identity,
        )
        self.assertEqual(len(set(identities)), 3)
        self.assertEqual(
            verification.verified_owner_attestation_author_login,
            verification.verified_owner_login,
        )
        self.assertTrue(verification.authoritative_for(receipt))

    def test_malformed_or_dependency_structures_are_exception_safe_and_never_ready(self):
        malformed_groups = (
            ("non-list any_of null", None),
            ("non-list any_of scalar", "16L2"),
            ("non-list OR group", ["16L2", "16LF-01"]),
            ("null OR group", [None]),
            ("tuple OR group", [("16L2", "16LF-01")]),
            ("null dependency member", [[None, "16LF-01"]]),
            ("non-iterable dependency member", [[42, "16LF-01"]]),
            ("unhashable dependency member", [[["16L2"], "16LF-01"]]),
            ("mapping dependency member", [[{"id": "16L2"}, "16LF-01"]]),
            ("malformed nested structure", [[[["16L2"]], "16LF-01"]]),
            ("duplicate dependencies", [["16L2", "16L2"]]),
            ("unknown dependency", [["UNKNOWN-OR-GATE", "16LF-01"]]),
        )
        for label, malformed in malformed_groups:
            with self.subTest(case=label):
                data = copy.deepcopy(self.data)
                tasks = board.index(data)
                tasks["16L3"]["any_of"] = malformed

                errors = board.validate(data)
                self.assertTrue(errors, label)

                ready, blocked = board.readiness(data)
                ready_ids = {task["id"] for task in ready}
                blocked_ids = {task["id"] for task in blocked}
                self.assertNotIn("16L3", ready_ids, label)
                self.assertIn("16L3", blocked_ids, label)
                self.assertEqual(board.waves(data), [], label)

    def test_cycle_through_a_valid_or_dependency_is_rejected(self):
        data = copy.deepcopy(self.data)
        tasks = board.index(data)
        tasks["16LF-01"]["depends_on"].append("16L3")
        errors = board.validate(data)
        self.assertTrue(any("dependency cycle" in error.lower() for error in errors))

if __name__ == "__main__":
    unittest.main()
