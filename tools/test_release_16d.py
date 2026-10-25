"""16D synthetic negative contracts; never infer desktop or CoS RAM savings."""
import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools import release_16c as bundle
from tools import release_16d as perf

SHA = "a" * 40


class ReleasePerfTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.target = "windows-x64"
        self.exe = self.root / "cos-rust.exe"
        self.exe.write_bytes(b"synthetic-headless-not-executable")
        self.pkg = self.root / "dummy"
        self.pkg.mkdir()
        (self.pkg / "LICENSE-MIT").write_text("Synthetic permissive notice\n")
        (self.pkg / "Cargo.toml").write_text("[package]\nname='dummy'\nversion='0.1.0'\n")
        self.license = self.root / "LICENSE"
        self.license.write_text("MIT synthetic project notice\n")
        self.out = self.root / "release-16c.zip"
        metadata = {"packages": [{"name": "dummy", "version": "0.1.0",
                    "license": "MIT", "source": None,
                    "manifest_path": str(self.pkg / "Cargo.toml")}]}
        lock = {"package": [{"name": "dummy", "version": "0.1.0", "source": None}]}
        bundle.build(self.exe, self.license, metadata, lock, self.out, self.target, SHA)

    def fake_sample(self, _binary, _profile):
        self.n += 1
        return {"launch_to_cli_json_ms": float(self.n + 10),
                "root_peak_working_set_bytes": 2_000_000 + self.n,
                "memory_samples": 3}

    def receipt(self):
        self.n = 0
        with patch.object(perf, "sample", side_effect=self.fake_sample):
            return perf.collect(self.exe, self.out, self.target, SHA)

    def test_valid_bundle_pinned_to_binary_identity(self):
        identity = perf.artifact(self.exe, self.out, self.target, SHA)
        self.assertEqual(identity["binary_bytes"], self.exe.stat().st_size)
        self.assertEqual(len(identity["bundle_sha256"]), 64)

    def test_cloud_receipt_is_bounded_and_cannot_claim_native_pass(self):
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "PASS")
        self.assertEqual(receipt["release_performance_budget"], "NOT_FROZEN")
        self.assertEqual(receipt["native_product_acceptance"], "NOT_RUN")
        self.assertEqual(receipt["original_cos_comparison"], "NOT_RUN")
        self.assertEqual(receipt["fresh_profile"]["launch_ms"]["observed_runs"], 3)
        self.assertEqual(receipt["reused_profile"]["root_peak_ws_bytes"]["observed_runs"], 3)
        self.assertEqual(perf.validate(receipt, self.exe, self.out, self.target, SHA)["status"], "PASS")

    def test_modified_or_replaced_binary_is_refused(self):
        self.exe.write_bytes(b"not-the-same-as-bundle")
        with self.assertRaisesRegex(perf.Refused, "differs"):
            perf.artifact(self.exe, self.out, self.target, SHA)

    def test_mismatched_source_platform_are_refused(self):
        with self.assertRaises(bundle.Refused):
            perf.artifact(self.exe, self.out, self.target, "b" * 40)
        linux_binary = self.root / "cos-rust"
        linux_binary.write_bytes(self.exe.read_bytes())
        with self.assertRaisesRegex(perf.Refused, "bundle platform mismatch"):
            perf.artifact(linux_binary, self.out, "linux-x64", SHA)

    def test_corrupted_bundle_payload_is_refused(self):
        from zipfile import BadZipFile
        raw = self.out.read_bytes()
        self.assertIn(b"synthetic-headless-not-executable", raw)
        self.out.write_bytes(raw.replace(b"synthetic-headless-not-executable",
                                         b"synthetic-headless-not-executablX", 1))
        with self.assertRaises((bundle.Refused, BadZipFile)):
            perf.artifact(self.exe, self.out, self.target, SHA)

    def test_tampered_receipt_size_or_hash_is_refused(self):
        baseline = self.receipt()
        for field, bad_value in (("bundle_bytes", 0), ("binary_sha256", "0" * 64)):
            receipt = copy.deepcopy(baseline)
            receipt[field] = bad_value
            with self.subTest(field=field), self.assertRaisesRegex(perf.Refused, "artifact identity"):
                perf.validate(receipt, self.exe, self.out, self.target, SHA)

    def test_budget_native_or_comparison_promotion_refused(self):
        baseline = self.receipt()
        for key, value in (("release_performance_budget", "PASS"),
                           ("native_product_acceptance", "PASS"),
                           ("original_cos_comparison", "PASS"),
                           ("process_scope", "full-desktop")):
            receipt = copy.deepcopy(baseline)
            receipt[key] = value
            with self.subTest(field=key), self.assertRaisesRegex(perf.Refused, "may not assert"):
                perf.validate(receipt, self.exe, self.out, self.target, SHA)

    def test_private_extra_receipt_fields_are_refused(self):
        baseline = self.receipt()
        for key, value in (("private_profile_path", "C:/Users/secret"),
                           ("provider_token", "secret")):
            receipt = copy.deepcopy(baseline)
            receipt[key] = value
            with self.subTest(field=key), self.assertRaisesRegex(perf.Refused, "private"):
                perf.validate(receipt, self.exe, self.out, self.target, SHA)
        receipt = copy.deepcopy(baseline)
        receipt["fresh_profile"]["user"] = "secret"
        with self.assertRaisesRegex(perf.Refused, "private"):
            perf.validate(receipt, self.exe, self.out, self.target, SHA)

    def test_forged_workload_is_refused(self):
        receipt = self.receipt()
        receipt["workload"] = "fully-interactive-gui"
        with self.assertRaisesRegex(perf.Refused, "unrecognized synthetic workload"):
            perf.validate(receipt, self.exe, self.out, self.target, SHA)

    def test_forged_distribution_is_refused(self):
        receipt = self.receipt()
        receipt["fresh_profile"]["launch_ms"]["median"] = 0
        with self.assertRaisesRegex(perf.Refused, "inconsistent"):
            perf.validate(receipt, self.exe, self.out, self.target, SHA)

    def test_missing_nan_negative_memory_and_boolean_values_refused(self):
        baseline = self.receipt()
        for field, invalid in (("launch_to_cli_json_ms", float("nan")),
                               ("root_peak_working_set_bytes", -1),
                               ("root_peak_working_set_bytes", True),
                               ("memory_samples", 0)):
            receipt = copy.deepcopy(baseline)
            receipt["reused_profile"]["runs"][0][field] = invalid
            with self.subTest(field=field, invalid=invalid), self.assertRaises(perf.Refused):
                perf.validate(receipt, self.exe, self.out, self.target, SHA)

    def test_no_memory_samples_fail_closed(self):
        self.n = 0
        def no_sample(_exe, _profile):
            return {"launch_to_cli_json_ms": 10.0,
                    "root_peak_working_set_bytes": None, "memory_samples": 0}
        with patch.object(perf, "sample", side_effect=no_sample):
            with self.assertRaisesRegex(perf.Refused, "measurement unavailable"):
                perf.collect(self.exe, self.out, self.target, SHA)

    def test_unexpected_cli_exit_and_private_output_are_refused(self):
        class Process:
            returncode = 1
            pid = 424
            def communicate(self, timeout=None):
                return b"", b"do not publish private command"
            def poll(self):
                return 1
        with patch.object(perf.subprocess, "Popen", return_value=Process()):
            with patch.object(perf, "working_set", return_value=1234):
                with self.assertRaisesRegex(perf.Refused, "nonzero"):
                    perf.sample(self.exe, self.root)

    def test_distribution_rejects_absence_of_observations(self):
        with self.assertRaisesRegex(perf.Refused, "no observed"):
            perf.distribution([{"rss": None}], "rss")


if __name__ == "__main__":
    unittest.main()
