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
            self.assertEqual(cols[5], desired, task_id)

    def test_historical_aliases_are_explicit_and_never_reused_as_current_preflight(self):
        aliases = self.data["legacy_pre_2026_10_20_ids"]
        self.assertEqual(aliases["preflight"]["former 8A"], "7P-A")
        self.assertEqual(aliases["full_phase_8"]["former 8C"], "8A")
        self.assertNotEqual(self.tasks["8A"]["title"], self.tasks["7P-A"]["title"])

    def test_registry_is_valid_and_acyclic(self):
        self.assertEqual(board.validate(self.data), [])

    def test_all_work_units_preserved(self):
        kinds = {k: sum(t["kind"] == k for t in self.data["tasks"]) for k in "CLU"}
        self.assertEqual(kinds, {"C": 80, "L": 31, "U": 15})
        self.assertEqual(len(self.tasks), 126)

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
        self.assertEqual(board.validate(future), [])
        waves = board.waves(future)
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
        # Purely hypothetical source-pinned fixture. Never a native acceptance.
        for key in ("16E", "16F", "16PKG-01", "16L1", "16U1", "16FS-01"):
            tasks[key]["status"] = "done"
        # This fixture is for conditional readiness only, not graph validator,
        # because historical registry prerequisites still need reconciling.
        tasks["16L3"]["status"] = "todo"
        tasks["16LF-01"]["status"] = "done"
        sha = "a" * 40
        tasks["16LF-01"]["evidence"] = {
            "schema_version": 1, "kind": "owner-local-synthetic-fresh-start",
            "host": "owner-local-windows", "profile_scope": "new-disposable-empty",
            "migration": "none", "source_sha": sha, "reviewed_source_sha": sha,
            "artifact_sha256": "b" * 64, "receipt_sha256": "c" * 64,
            "local_session_id": "synthetic-test-fixture-only",
            "receipt_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/137#issuecomment-1",
            "review_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/999#pullrequestreview-1",
            "independent_review_verdict": "PASS",
            "checks": {k: "PASS" for k in board.FRESH_CHECKS},
        }
        return data, tasks

    def test_fresh_start_positive_hypothetical_fixture_only(self):
        data, tasks = self._fresh_scenario()
        self.assertTrue(board.fresh_receipt_valid(tasks["16LF-01"]["evidence"]))
        self.assertFalse(board.accepted(tasks, "16L2"))
        self.assertEqual(board.missing_dependencies(tasks["16L3"], tasks), [])
        # Actual canonical release remains blocked and is never auto-promoted.
        self.assertEqual(board.explain(self.data, "16L3")["state"], "BLOCKED")

    def test_owner_declined_never_grants_private_profile_access(self):
        data, tasks = self._fresh_scenario()
        self.assertEqual(tasks["16U1"]["migration_consent"], "declined")
        tasks["16L2"]["status"] = "done"
        tasks["16L2"]["private_migration_receipt"] = {
            "scope": "approved-copy-only", "physical_windows": True,
            "independent_review_verdict": "PASS",
        }
        self.assertFalse(board.accepted(tasks, "16L2"))
        self.assertTrue(any("16L2" in e for e in board.validate(data)))

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
        data, tasks = self._fresh_scenario()
        good = tasks["16LF-01"]["evidence"]
        for field, value in [
            ("source_sha", "fake"),
            ("reviewed_source_sha", "d" * 40),
            ("artifact_sha256", "broken"),
            ("independent_review_verdict", "PENDING"),
            ("review_url", "https://evil.invalid/review"),
            ("host", "github-hosted-windows"),
            ("migration", "private-copy"),
        ]:
            broken = copy.deepcopy(good)
            broken[field] = value
            self.assertFalse(board.fresh_receipt_valid(broken), field)
        broken = copy.deepcopy(good)
        broken["checks"]["rollback"] = "FAIL"
        self.assertFalse(board.fresh_receipt_valid(broken))
        broken = copy.deepcopy(good)
        del broken["checks"]["owned_cleanup"]
        self.assertFalse(board.fresh_receipt_valid(broken))

    def test_release_gate_preserves_16f_16l1_16u2_and_physical_gate(self):
        data, tasks = self._fresh_scenario()
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

if __name__ == "__main__":
    unittest.main()
