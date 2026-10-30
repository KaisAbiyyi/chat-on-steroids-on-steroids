"""Negative-case and source-bound audit regressions for 16F."""
import copy
import hashlib
import json
import unittest

from tools import release_16f as f

SHA = "a" * 40

def fixture():
    sources = {p: (p + ":synthetic").encode("utf-8") for p in f.TRACKED}
    sources["docs/task-registry.json"] = json.dumps({"tasks": [
        {"id": "16D", "depends_on": [], "kind": "C"},
        {"id": "16E", "depends_on": [], "kind": "C"},
        {"id": "16F", "depends_on": ["16D", "16E"], "kind": "C"},
        {"id": "16L1", "depends_on": ["16E"], "kind": "L"},
        {"id": "16U1", "depends_on": ["16E"], "kind": "U",
         "migration_consent": "declined", "owner_decision": "fresh_start"},
        {"id": "16L2", "depends_on": ["16U1", "16E"], "kind": "L",
         "status": "blocked", "private_migration_status": "NOT_RUN / NOT_AUTHORIZED"},
        {"id": "16FS-01", "depends_on": ["16U1", "16L1", "16F"], "kind": "C",
         "status": "done", "accepted_evidence_url": "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/138"},
        {"id": "16FV-01", "depends_on": ["16FS-01", "16U1", "16L1"], "kind": "C",
         "status": "claimed", "resource_locks": ["freshstart-evidence-verifier"]},
        {"id": "16LF-01", "depends_on": ["16FS-01", "16FV-01", "16U1", "16L1"], "kind": "L",
         "status": "blocked", "evidence": None},
        {"id": "16L3", "depends_on": ["16F", "16L1"], "kind": "L",
         "any_of": [["16L2", "16LF-01"]]},
    ]}).encode("utf-8")
    return sources

def make(sources=None, ancestors=None, expected=SHA, head=SHA):
    return f.make_report(expected, head, {k: True for k in f.MERGED} if ancestors is None else ancestors,
                         fixture() if sources is None else sources)

class SourceBoundReportTests(unittest.TestCase):
    def test_valid_report_is_strictly_cloud_only(self):
        r = make()
        self.assertEqual(r["schema"], f.SCHEMA)
        self.assertEqual(f.SCHEMA, "cosos.release.cloud-provenance.v2")
        self.assertEqual(r["source_sha"], SHA)
        self.assertFalse(r["product_release_ready"])
        self.assertEqual(set(r["merged_predecessors"]), set(f.MERGED))
        self.assertTrue(all(g == "NOT_RUN" for g in r["native_owner_gates"].values()))
        self.assertEqual(set(r["native_owner_gates"]), set(f.GATES))
        self.assertEqual(
            r["security_review"]["review_policy"],
            "GPT6_OWNER_ATTESTED",
        )
        self.assertEqual(
            r["security_review"]["review_policy_semantics"],
            "OWNER_ATTESTED_NOT_GITHUB_INDEPENDENT_APPROVAL",
        )
        self.assertEqual(
            r["security_review"]["model_authorship"],
            "OWNER_ATTESTED_NOT_CRYPTOGRAPHICALLY_AUTHENTICATED",
        )
        self.assertEqual(
            r["native_owner_gates"]["16LF-01_owner_local_fresh_start"], "NOT_RUN"
        )
        dag_contract_sources = {
            "tools/task_board.py",
            "tools/test_task_board.py",
            "tools/fresh_evidence_verifier.py",
            "tools/fixtures/16fs01-pr138.diff",
            "tools/test_fresh_evidence_verifier.py",
            "tools/record_16fv_qualification.py",
            "tools/test_record_16fv_qualification.py",
            ".github/workflows/16fv-trusted-evidence-verifier.yml",
            ".github/workflows/task-graph.yml",
            "docs/PARALLEL-EXECUTION-BOARD.md",
            "docs/implementation-plan/2026-10-18-cloud-first-units.md",
            "docs/implementation-summary/2026-10-29-16fs01-freshstart-contract.md",
        }
        self.assertTrue(dag_contract_sources.issubset(set(f.TRACKED)))
        self.assertTrue(dag_contract_sources.issubset(set(r["tracked_contracts"])))
        self.assertEqual(set(f.GATES), {
            "16L1_native_installer",
            "16U1_owner_consent",
            "16L2_owner_approved_copy_migration",
            "16LF-01_owner_local_fresh_start",
            "16L3_native_full_release_performance",
            "16U2_owner_release_decision",
            "PERF-05_native_product_comparison",
            "PERF-06_product_performance_budget",
        })
        self.assertEqual(r["tracked_contracts"]["LICENSE"]["sha256"],
                         hashlib.sha256(b"LICENSE:synthetic").hexdigest())
        self.assertTrue(f.validate_receipt(json.loads(json.dumps(r)), r))

    def test_source_sha_mismatch_and_malformed_sha_fail_closed(self):
        for expect, actual in [("b" * 40, SHA), ("invalid", "invalid"), (SHA.upper(), SHA)]:
            with self.subTest(expect=expect), self.assertRaises(f.Refused):
                make(expected=expect, head=actual)

    def test_missing_ancestor_rejected_even_if_other_tasks_pass(self):
        for missing in f.MERGED:
            with self.subTest(missing=missing), self.assertRaises(f.Refused):
                make(ancestors={k: k != missing for k in f.MERGED})

    def test_incomplete_ancestor_map_rejected(self):
        with self.assertRaises(f.Refused):
            make(ancestors={"16D": True, "16E": True})

    def test_malformed_and_missing_sources_denied(self):
        sources = fixture()
        del sources["LICENSE"]
        with self.assertRaises(f.Refused):
            make(sources=sources)
        sources = fixture()
        sources["extra.txt"] = b"extra"
        with self.assertRaises(f.Refused):
            make(sources=sources)
        sources = fixture()
        sources["LICENSE"] = b""
        with self.assertRaises(f.Refused):
            make(sources=sources)

    def test_dependency_or_kind_drift_denied(self):
        for task, key, value in [
            ("16F", "depends_on", ["16D"]), ("16L3", "depends_on", ["16F"]),
            ("16F", "kind", "L"), ("16L3", "kind", "C"),
            ("16L3", "any_of", []), ("16L3", "any_of", [["16L2", "16L1"]]),
            ("16LF-01", "depends_on", ["16U1"]), ("16FS-01", "kind", "L"),
            ("16FV-01", "depends_on", ["16FS-01"]), ("16FV-01", "kind", "L"),
            ("16FV-01", "resource_locks", []),
            ("16U1", "migration_consent", "granted"),
            ("16U1", "owner_decision", "approved_private_copy"),
            ("16L2", "status", "done"),
            ("16L2", "private_migration_status", "DONE")
        ]:
            sources = fixture()
            registry = json.loads(sources["docs/task-registry.json"])
            next(t for t in registry["tasks"] if t["id"] == task)[key] = value
            sources["docs/task-registry.json"] = json.dumps(registry).encode()
            with self.subTest(task=task, key=key), self.assertRaises(f.Refused):
                make(sources=sources)

    def test_forged_product_release_native_pass_or_review_denied(self):
        expected = make()
        for mutate in [
            lambda r: r.update(product_release_ready=True),
            lambda r: r["native_owner_gates"].update({"16L1_native_installer": "PASS"}),
            lambda r: r["native_owner_gates"].pop("16U1_owner_consent"),
            lambda r: r["native_owner_gates"].update({"16LF-01_owner_local_fresh_start": "PASS"}),
            lambda r: r["security_review"].update({"review_policy": "PASS"}),
            lambda r: r["merged_predecessors"]["16E"].update({"merged_commit_sha": "0" * 40}),
            lambda r: r["tracked_contracts"]["LICENSE"].update({"sha256": "0" * 64}),
            lambda r: r.update(source_sha="b" * 40),
            lambda r: r.update(unexpected_claim="release authorized"),
        ]:
            altered = copy.deepcopy(expected)
            mutate(altered)
            with self.assertRaises(f.Refused):
                f.validate_receipt(altered, expected)

    def test_16lf_gate_must_remain_blocked_and_not_run_in_source_registry(self):
        for field, value in [("status", "done"), ("evidence", {"independent_review_verdict": "PASS"})]:
            sources = fixture()
            registry = json.loads(sources["docs/task-registry.json"])
            next(t for t in registry["tasks"] if t["id"] == "16LF-01")[field] = value
            sources["docs/task-registry.json"] = json.dumps(registry).encode()
            with self.subTest(field=field), self.assertRaises(f.Refused):
                make(sources=sources)

    def test_bad_registry_and_oversize_file_denied(self):
        src = fixture()
        src["docs/task-registry.json"] = b"not-json"
        with self.assertRaises(f.Refused):
            make(sources=src)
        src = fixture()
        src["LICENSE"] = b"x" * (f.MAX_SOURCE_BYTES + 1)
        with self.assertRaises(f.Refused):
            make(sources=src)

if __name__ == "__main__":
    unittest.main()
