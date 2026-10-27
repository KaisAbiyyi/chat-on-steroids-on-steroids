"""Cloud regression for immutable synthetic 7L2 build packaging."""
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from package_artifact import package
from preflight import Refused, report, verify_manifest


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.repo = root / "repo"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        subprocess.run(
            ["git", "-C", str(self.repo), "-c", "user.name=CI",
             "-c", "user.email=ci@example.invalid", "commit", "-q",
             "--allow-empty", "-m", "synthetic fixture"],
            check=True
        )
        self.art = root / "artifact"
        (self.art / "host").mkdir(parents=True)
        (self.art / "host" / "WindowsHost.dll").write_bytes(b"synthetic webview2")
        (self.art / "7l2_bridge.exe").write_bytes(b"synthetic rust")

    def test_immutable_synthetic_manifest_never_qualifies_product(self):
        sha = package(self.repo, self.art)
        self.assertEqual(verify_manifest(self.art, sha), {
            "source.sha", "artifact-kind.json", "host/WindowsHost.dll",
            "7l2_bridge.exe",
        })
        receipt = report(self.repo, self.art, sha, "chatgpt", "7u1-isolated-test01",
                         True)
        self.assertEqual(receipt["preflightStatus"], "BLOCKED")
        self.assertEqual(receipt["checks"]["artifact_manifest"]["status"], "PASS")
        self.assertEqual(receipt["checks"]["product_native_adapter"]["status"], "BLOCKED")
        self.assertFalse(receipt["owner7U1Accepted"])
        self.assertEqual(receipt["owner7U1Status"], "NOT_RUN")
        self.assertIsNone(receipt["outgoingSendCount"])
        self.assertEqual(
            json.loads((self.art / "artifact-kind.json").read_text())["kind"],
            "synthetic-7l2-only"
        )

    def test_rerun_refuses_overwrite(self):
        package(self.repo, self.art)
        with self.assertRaises(Refused):
            package(self.repo, self.art)

    def test_dirty_source_refused(self):
        (self.repo / "apps").mkdir()
        (self.repo / "apps" / "dirty.cs").write_text("unexpected", encoding="utf-8")
        with self.assertRaisesRegex(Refused, "clean"):
            package(self.repo, self.art)

    def test_forged_product_named_binary_refused(self):
        (self.art / "host" / "CoSoSProductBrowser.dll").write_bytes(b"fake")
        with self.assertRaisesRegex(Refused, "forbidden"):
            package(self.repo, self.art)

    def test_missing_synthetic_component_refused(self):
        (self.art / "7l2_bridge.exe").unlink()
        with self.assertRaises(Refused):
            package(self.repo, self.art)

    def test_package_detects_mutation(self):
        sha = package(self.repo, self.art)
        (self.art / "host" / "WindowsHost.dll").write_bytes(b"tampered")
        with self.assertRaises(Refused):
            verify_manifest(self.art, sha)


if __name__ == "__main__":
    unittest.main()
