"""Tests for exact-HEAD GitHub Actions qualification evidence."""
import hashlib
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from tools import record_16fv_qualification as recorder


class QualificationRecorderTests(unittest.TestCase):
    def invoke(self, args, actual_head):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "tools").mkdir()
            verifier = b"production verifier fixture"
            (root / "tools" / "fresh_evidence_verifier.py").write_bytes(verifier)
            previous = Path.cwd()
            try:
                os.chdir(root)
                with patch.object(sys, "argv", ["record_16fv_qualification.py", *args]):
                    with patch.object(
                        recorder.subprocess,
                        "run",
                        return_value=SimpleNamespace(stdout=actual_head + "\n"),
                    ):
                        result = recorder.main()
                output = json.loads((root / "16fv-qualification.json").read_text(encoding="utf-8"))
                return result, output, hashlib.sha256(verifier).hexdigest()
            finally:
                os.chdir(previous)

    def args(self, source_sha, *, status="success", dag="success"):
        return [
            "--source-sha", source_sha,
            "--platform", "Linux",
            "--status", status,
            "--compile-outcome", "success",
            "--unit-outcome", "success",
            "--dag-outcome", dag,
            "--workflow-run-id", "123",
            "--run-attempt", "1",
            "--run-url", "https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/123",
            "--event", "pull_request",
        ]

    def test_records_only_the_actual_checked_out_head_and_verifier_hash(self):
        sha = "a" * 40
        result, evidence, verifier_sha = self.invoke(self.args(sha), sha)
        self.assertEqual(result, 0)
        self.assertEqual(evidence["tested_source_sha"], sha)
        self.assertEqual(evidence["verifier_source_sha"], sha)
        self.assertEqual(evidence["verifier_sha256"], verifier_sha)
        self.assertEqual(evidence["status"], "PASS")
        self.assertEqual(evidence["production_trust_root"], "NOT_CONFIGURED_IN_CI")
        self.assertEqual(evidence["production_acceptance"], "NOT_AUTHORIZED_BY_CI")

    def test_refuses_a_claimed_sha_that_differs_from_checkout(self):
        with self.assertRaises(SystemExit):
            self.invoke(self.args("a" * 40), "b" * 40)

    def test_failed_dag_outcome_cannot_produce_pass_evidence(self):
        result, evidence, _ = self.invoke(self.args("a" * 40, dag="failure"), "a" * 40)
        self.assertEqual(result, 1)
        self.assertEqual(evidence["status"], "FAIL")


if __name__ == "__main__":
    unittest.main()
