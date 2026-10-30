"""Adversarial regression suite: a plan may validate, but must not pass native gates."""
from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_browser_feasibility as gate


class BrowserFeasibilityContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.pristine = gate.load()

    def fresh(self):
        return copy.deepcopy(self.pristine)

    def assert_rejected(self, doc, reason):
        problems = gate.validate(doc)
        self.assertTrue(any(reason in item for item in problems), problems)

    def test_baseline_is_valid_but_explicitly_provisional(self):
        self.assertEqual(gate.validate(self.fresh()), [])
        self.assertEqual(self.pristine["decision"], "PROVISIONAL")
        self.assertTrue(self.pristine["policy"]["prevent_cloud_pass"])
        self.assertTrue(all(c["verification"] == "UNVERIFIED"
                            for c in self.pristine["capabilities"]))

    def test_missing_critical_extension_world_fails(self):
        d = self.fresh()
        d["capabilities"] = [x for x in d["capabilities"] if x["id"] != "extension_worlds"]
        self.assert_rejected(d, "missing capabilities: extension_worlds")

    def test_duplicate_debugger_capability_fails(self):
        d = self.fresh()
        d["capabilities"].append(copy.deepcopy(d["capabilities"][3]))
        self.assert_rejected(d, "duplicate capability id")

    def test_cloud_cannot_claim_browser_runtime_pass(self):
        d = self.fresh()
        d["capabilities"][0]["verification"] = "PASS"
        self.assert_rejected(d, "cannot grant native/browser PASS")

    def test_cloud_cannot_claim_native_verified(self):
        d = self.fresh()
        d["decision"] = "PASS"
        self.assert_rejected(d, "decision: expected")

    def test_debugger_parity_cannot_be_marked_cloud_only(self):
        d = self.fresh()
        next(x for x in d["capabilities"] if x["id"] == "extension_debugger_parity")["first_probe"] = "7P-B"
        self.assert_rejected(d, "cannot reduce native-critical probe to cloud")

    def test_real_profile_access_fail_closed(self):
        d = self.fresh()
        d["policy"]["allow_user_private_profiles"] = True
        self.assert_rejected(d, "no-private-action")

    def test_automatic_send_fail_closed(self):
        d = self.fresh()
        d["policy"]["allow_automatic_send"] = True
        self.assert_rejected(d, "no-private-action")

    def test_rust_only_implicit_assertion_fails(self):
        d = self.fresh()
        d["policy"]["renderer_language"] = "Rust"
        self.assert_rejected(d, "hybrid language policy")

    def test_unreviewed_capability_must_be_explicit(self):
        d = self.fresh()
        d["capabilities"].append(dict(d["capabilities"][0], id="fake-extra"))
        self.assert_rejected(d, "unreviewed capabilities")

    def test_missing_negative_probe_fails(self):
        d = self.fresh()
        d["capabilities"][0]["probe"] = "works"
        self.assert_rejected(d, "needs reproducible probe")

    def test_unsourced_capability_fails(self):
        d = self.fresh()
        d["capabilities"][0]["sources"] = []
        self.assert_rejected(d, "missing evidence pointers")

    def test_unsafe_source_reference_fails(self):
        d = self.fresh()
        d["capabilities"][0]["sources"] = ["file:///C:/private"]
        self.assert_rejected(d, "invalid/unsafe source reference")

    def test_local_gate_cannot_be_skipped(self):
        d = self.fresh()
        d["followup"] = ["7P-B", "7A"]
        self.assert_rejected(d, "preflight order")

    def test_browser_only_external_fallback_does_not_pass(self):
        d = self.fresh()
        next(x for x in d["options"] if x["id"] == "external-browser-only")["status"] = "PROVISIONAL"
        self.assert_rejected(d, "silent external-only browser fallback")

    def test_completed_preflight_retains_distinct_native_bridge_gate(self):
        import task_board
        graph = task_board.load()
        tasks = task_board.index(graph)
        self.assertEqual(tasks["7P-A"]["depends_on"], ["CL-L6"])
        self.assertEqual(tasks["7P-B"]["depends_on"], ["7P-A"])
        self.assertEqual(tasks["7P-L1"]["depends_on"], ["7P-B"])
        self.assertEqual(tasks["7P-L1"]["status"], "done")
        self.assertEqual(tasks["7A"]["depends_on"], ["7P-B"])
        self.assertEqual(tasks["7L1"]["depends_on"], ["7C", "7P-L1"])
        ready, _ = task_board.readiness(graph)
        ready_ids = {task["id"] for task in ready}
        self.assertEqual(tasks["7A"]["status"], "done")
        self.assertNotIn("7A", ready_ids)
        self.assertEqual(tasks["7B"]["status"], "done")
        self.assertNotIn("7B", ready_ids)
        self.assertEqual(tasks["7C"]["status"], "done")
        self.assertNotIn("7C", ready_ids)
        self.assertNotIn("7P-L1", ready_ids)
        self.assertEqual(tasks["7L1"]["status"], "done")
        self.assertNotIn("7L1", ready_ids)
        # A reopened integration gate must remain distinct from completed
        # 7P-L1 preflight and re-block its cloud successor.
        retest = copy.deepcopy(graph)
        next(t for t in retest["tasks"] if t["id"] == "7L1")["status"] = "todo"
        retest_ready, _ = task_board.readiness(retest)
        self.assertIn("7L1", {t["id"] for t in retest_ready})
        self.assertNotIn("7D", {t["id"] for t in retest_ready})
        self.assertNotIn("7D", ready_ids)
        self.assertEqual(tasks["7D"]["status"], "done")
        # Cloud 8B/8C and the separately received physical 8L1 proof do
        # not waive the real provider owner gate or production host repair.
        self.assertEqual(tasks["7E"]["status"], "done")
        self.assertNotIn("7E", ready_ids)
        self.assertEqual(tasks["8A"]["status"], "done")
        self.assertNotIn("8A", ready_ids)
        self.assertNotIn("7L2", ready_ids)  # accepted physical Windows source 74d4460
        self.assertEqual(tasks["7L2"]["status"], "done")
        self.assertIn("7U1-PREP", ready_ids)  # real provider owner runner still unclaimed
        for accepted in ("8B", "8C", "15A"):
            self.assertEqual(tasks[accepted]["status"], "done")
            self.assertNotIn(accepted, ready_ids)
        self.assertEqual(tasks["8L1"]["status"], "done")
        self.assertNotIn("8L1", ready_ids)
        self.assertNotIn("8U1", ready_ids)
        self.assertEqual(task_board.explain(graph, "8U1")["missing_direct"], [])
        self.assertEqual(tasks["8U1"]["status"], "blocked")
        self.assertNotIn("15B", ready_ids)  # 15B cloud implementation accepted
        self.assertEqual(tasks["15B"]["status"], "done")
        self.assertIn("PERF-00", ready_ids)  # P0 methodology first
        self.assertEqual(tasks["7E"]["depends_on"], ["7D"])
        self.assertEqual(tasks["7D"]["depends_on"], ["7C", "7L1"])
        self.assertEqual(tasks["16L3"]["status"], "blocked")
        # Scoped 7P-L1 physical feasibility and 7L1 integrated Rust/MV3
        # acceptance are separately proven; full product release is pending.


if __name__ == "__main__":
    unittest.main()
