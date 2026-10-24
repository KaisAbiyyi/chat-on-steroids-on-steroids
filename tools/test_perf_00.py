"""PERF-00 isolated synthetic contracts; no private accounts or native host claims."""
from __future__ import annotations

import copy
import json
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import perf_00 as perf

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "fixtures" / "perf-00" / "synthetic-trace.json"


class PerfMeasurementTests(unittest.TestCase):
    def setUp(self):
        self.trace = json.loads(FIXTURE.read_text(encoding="utf-8"))

    def test_workload_matrix_has_required_modes_and_no_secrets(self):
        self.assertEqual(len(perf.WORKLOADS), 10)
        self.assertEqual(perf.WORKLOADS["multi_chat"]["chats"], 8)
        self.assertEqual(perf.WORKLOADS["external_browser"]["browser"], "external-reused")
        self.assertEqual(perf.WORKLOADS["embedded_browser"]["browser"], "embedded-ondemand")
        self.assertIn("recovery", perf.WORKLOADS)
        self.assertNotIn("email", json.dumps(perf.WORKLOADS).lower())
        self.assertNotIn("password", json.dumps(perf.WORKLOADS).lower())

    def test_fixture_process_tree_totals_exclude_unrelated_and_prevent_double_count(self):
        summary = perf.summarize(self.trace)
        self.assertEqual(summary["classification"], "synthetic-fixture")
        self.assertEqual(summary["native_product_acceptance"], "NOT_RUN")
        self.assertEqual(summary["whole_tree_peak_rss_bytes"], 248 * perf.BYTES_PER_MIB)
        self.assertEqual(summary["whole_tree_final_rss_bytes"], 248 * perf.BYTES_PER_MIB)
        self.assertEqual(summary["whole_tree_median_rss_bytes"], 239 * perf.BYTES_PER_MIB)
        self.assertEqual(summary["whole_tree_median_private_bytes"], 146 * perf.BYTES_PER_MIB)
        self.assertEqual(summary["sample_series"][0]["owned_process_count"], 2)
        self.assertEqual(summary["sample_series"][0]["external_process_count"], 2)
        self.assertEqual(summary["sample_series"][1]["owned_rss_bytes"], 143 * perf.BYTES_PER_MIB)
        self.assertEqual(summary["sample_series"][1]["external_rss_bytes"], 105 * perf.BYTES_PER_MIB)
        self.assertEqual(summary["tool_latency_p95_ms"], 48.0)

    def test_externally_tagged_descendant_is_counted_only_once(self):
        first = self.trace["samples"][0]
        first["roots"].append({"pid": 102, "identity": "a:11", "role": "external"})
        perf.validate_trace(self.trace)
        result = perf.sample_tree(first)
        self.assertEqual(result["owned_process_count"], 2)
        self.assertEqual(result["external_process_count"], 2)
        self.assertEqual(result["total_rss_bytes"], 230 * perf.BYTES_PER_MIB)

    def test_missing_root_and_pid_reuse_never_become_complete_samples(self):
        first = self.trace["samples"][0]
        first["roots"][0]["identity"] = "different-start"
        summary = perf.summarize(self.trace)
        self.assertFalse(summary["all_roots_observed"])
        self.assertEqual(summary["complete_sample_count"], 1)
        self.assertTrue(any("root absent" in x for x in summary["warnings"]))
        self.trace["samples"][1]["roots"][0]["identity"] = "different-start"
        with self.assertRaises(perf.TraceError):
            perf.summarize(self.trace)

    def test_duplicate_pids_and_roots_are_invalid(self):
        point = self.trace["samples"][0]
        point["processes"].append(copy.deepcopy(point["processes"][0]))
        with self.assertRaisesRegex(perf.TraceError, "duplicate process"):
            perf.validate_trace(self.trace)
        point["processes"].pop()
        point["roots"].append(copy.deepcopy(point["roots"][0]))
        with self.assertRaisesRegex(perf.TraceError, "duplicate root"):
            perf.validate_trace(self.trace)

    def test_negative_nan_and_nonmonotonic_samples_are_invalid(self):
        for field, invalid in [("rss_bytes", -1), ("cpu_seconds", float("nan")),
                               ("rss_bytes", True)]:
            bad = copy.deepcopy(self.trace)
            bad["samples"][0]["processes"][0][field] = invalid
            with self.subTest(field=field, invalid=str(invalid)), self.assertRaises(perf.TraceError):
                perf.validate_trace(bad)
        self.trace["samples"][1]["elapsed_ms"] = 0
        with self.assertRaisesRegex(perf.TraceError, "strictly increasing"):
            perf.validate_trace(self.trace)

    def test_no_private_bytes_when_missing_any_process(self):
        self.trace["samples"][0]["processes"][0]["private_bytes"] = None
        report = perf.summarize(self.trace)
        self.assertIsNone(report["whole_tree_median_private_bytes"])

    def test_no_comparison_of_synthetic_data(self):
        baseline = copy.deepcopy(self.trace)
        baseline["provenance"]["app"] = "cos-original"
        with self.assertRaisesRegex(perf.TraceError, "synthetic data"):
            perf.compare(perf.summarize(baseline), perf.summarize(self.trace))

    def test_observed_comparison_requires_same_host_workload_and_complete_roots(self):
        a, b = copy.deepcopy(self.trace), copy.deepcopy(self.trace)
        for record, name in ((a, "cos-original"), (b, "cosos")):
            record["provenance"]["app"] = name
            record["provenance"]["synthetic"] = False
            record["provenance"]["source_sha"] = "a" * 40
            record["provenance"]["binary_sha256"] = "b" * 64
        result = perf.compare(perf.summarize(a), perf.summarize(b))
        self.assertEqual(result["reduction_percent"], 0)
        self.assertEqual(result["native_product_acceptance"], "NOT_RUN")
        b["provenance"]["host_id"] = "different-machine"
        with self.assertRaisesRegex(perf.TraceError, "machine/profile/power mismatch"):
            perf.compare(perf.summarize(a), perf.summarize(b))
        b["provenance"]["host_id"] = a["provenance"]["host_id"]
        b["samples"][0]["roots"][0]["identity"] = "unmatched"
        with self.assertRaisesRegex(perf.TraceError, "incomplete process trees"):
            perf.compare(perf.summarize(a), perf.summarize(b))

    def test_source_identity_and_workload_cannot_be_forged_by_fixture_flag(self):
        self.trace["provenance"]["synthetic"] = False
        with self.assertRaises(perf.TraceError):
            perf.validate_trace(self.trace)
        self.trace["provenance"]["synthetic"] = True
        self.trace["workload_params"]["chats"] = 1000
        with self.assertRaisesRegex(perf.TraceError, "workload definition changed"):
            perf.validate_trace(self.trace)

    def test_cli_matrix_and_fixture_summary_are_portable(self):
        run = subprocess.run([sys.executable, str(ROOT / "tools" / "perf_00.py"),
                              "matrix"], capture_output=True, text=True, check=True)
        self.assertEqual(json.loads(run.stdout), perf.WORKLOADS)
        run = subprocess.run([sys.executable, str(ROOT / "tools" / "perf_00.py"),
                              "summarize", "--input", str(FIXTURE)],
                             capture_output=True, text=True, check=True)
        summary = json.loads(run.stdout)
        self.assertEqual(summary["sample_count"], 2)
        self.assertEqual(summary["native_product_acceptance"], "NOT_RUN")

    def test_cli_compare_fails_closed_on_synthetic_trace(self):
        run = subprocess.run([sys.executable, str(ROOT / "tools" / "perf_00.py"),
                              "compare", "--baseline", str(FIXTURE),
                              "--candidate", str(FIXTURE)],
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 2)
        self.assertIn("baseline must be original CoS", run.stderr)


if __name__ == "__main__":
    unittest.main()
