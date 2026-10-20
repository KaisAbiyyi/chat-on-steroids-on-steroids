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
            desired = "DONE" if task["status"] == "done" else (
                "**READY · PARALLEL**" if not any(
                    self.tasks[dep]["status"] != "done" for dep in task["depends_on"]
                ) and task["status"] == "todo" else "BLOCKED"
            )
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

    def test_current_ready_wave_is_five_disjoint_cloud_tasks(self):
        ready, _ = board.readiness(self.data)
        self.assertEqual(
            {task["id"] for task in ready},
            {"CL-05A", "CL-06B", "CL-07A", "CL-08A", "CL-09A"},
        )
        self.assertTrue(all(task["kind"] == "C" for task in ready))
        self.assertEqual(len(set().union(*(board.effective_locks(t) for t in ready))), 5)

    def test_prerequisite_explanation_is_transitive(self):
        item = board.explain(self.data, "CL-09C")
        self.assertEqual(item["state"], "BLOCKED")
        self.assertEqual(item["missing_direct"], ["CL-09A", "CL-05A"])
        self.assertIn("CL-L1", self.tasks["CL-09A"]["depends_on"])
        self.assertTrue(item["live_claim_check_required"])
        self.assertEqual(board.explain(self.data, "CL-05A")["state"], "READY_CANDIDATE")

    def test_browser_preflight_is_in_phase_seven_not_eight(self):
        for task_id in ("7P-A", "7P-B", "7P-L1"):
            self.assertEqual(self.tasks[task_id]["phase"], "7")
        self.assertEqual(self.tasks["7P-A"]["depends_on"], ["CL-L6"])
        self.assertEqual(self.tasks["7A"]["depends_on"], ["7P-L1"])
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
        self.assertEqual({t["id"] for t in waves[0]}, {"CL-05A", "CL-06B", "CL-07A", "CL-08A", "CL-09A"})
        flat = [t["id"] for wave in waves for t in wave]
        self.assertLess(flat.index("CL-L6"), flat.index("7P-A"))
        self.assertLess(flat.index("7P-L1"), flat.index("7A"))
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
        next(t for t in data["tasks"] if t["id"] == "CL-05B")["status"] = "done"
        self.assertTrue(any("cannot be done" in e for e in board.validate(data)))

    def test_legacy_phase_ordering_cannot_regress(self):
        data = copy.deepcopy(self.data)
        next(t for t in data["tasks"] if t["id"] == "7A")["depends_on"] = ["8L1"]
        self.assertTrue(any("required phase ordering" in e for e in board.validate(data)))

    def test_completed_units_are_not_reoffered_as_ready(self):
        ready, _ = board.readiness(self.data)
        offered = {t["id"] for t in ready}
        self.assertTrue({"CL-04A", "CL-04B", "CL-04C", "CL-L1", "CL-06A"}.isdisjoint(offered))


if __name__ == "__main__":
    unittest.main()
