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
                ) and task["status"] == "todo" else "BLOCKED"
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
        self.assertEqual(kinds, {"C": 72, "L": 28, "U": 15})
        self.assertEqual(len(self.tasks), 115)

    def test_owner_deferral_unblocks_cloud_7a_not_native_gate(self):
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
        self.assertEqual(self.tasks["7P-L1"]["status"], "hold")
        self.assertNotIn("7P-L1", ready_ids)
        self.assertEqual(board.explain(self.data, "7P-L1")["state"], "HOLD")
        self.assertIn("physical-windows-host", board.effective_locks(self.tasks["7P-L1"]))
        self.assertEqual(self.tasks["7A"]["status"], "claimed")
        self.assertNotIn("7A", ready_ids)
        candidate = copy.deepcopy(self.data)
        next(t for t in candidate["tasks"] if t["id"] == "7A")["status"] = "todo"
        eligible, _ = board.readiness(candidate)
        self.assertIn("7A", {t["id"] for t in eligible})
        self.assertNotIn("7L1", ready_ids)
        # Reopening architecture must also reopen downstream browser probe;
        # the local gate can never be offered before both are accepted.
        hypothetical = copy.deepcopy(self.data)
        for task in hypothetical["tasks"]:
            if task["id"] in ("7P-A", "7P-B"):
                task["status"] = "todo"
        gated, _ = board.readiness(hypothetical)
        gated_ids = {t["id"] for t in gated}
        self.assertIn("7P-A", gated_ids)
        self.assertNotIn("7P-B", gated_ids)
        self.assertNotIn("7P-L1", gated_ids)

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
            self.assertEqual([t["id"] for t in waves[0]], [ready[0]["id"]])
        # A completed gate is excluded from forecasts. In a hypothetical
        # reopened native gate, every downstream phase must retain its order.
        future = copy.deepcopy(self.data)
        # A synthetic rollback of CL-L6 must also roll back its already
        # accepted dependent 7P-A, or this cannot be a valid readiness DAG.
        for task in future["tasks"]:
            if task["id"] in ("CL-L6", "7P-A", "7P-B", "7P-L1", "7A"):
                task["status"] = "todo"
        waves = board.waves(future)
        flat = [t["id"] for wave in waves for t in wave]
        self.assertLess(flat.index("CL-L6"), flat.index("7P-A"))
        self.assertLess(flat.index("7P-B"), flat.index("7A"))
        self.assertLess(flat.index("7P-L1"), flat.index("7L1"))
        self.assertLess(flat.index("7E"), flat.index("8A"))
        self.assertLess(flat.index("8U1"), flat.index("9A"))

    def test_dangling_dependency_is_rejected(self):
        data = copy.deepcopy(self.data)
        next(t for t in data["tasks"] if t["id"] == "CL-05A")["depends_on"].append("NEVER-EXISTED")
        self.assertTrue(any("dangling" in e for e in board.validate(data)))

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


if __name__ == "__main__":
    unittest.main()
