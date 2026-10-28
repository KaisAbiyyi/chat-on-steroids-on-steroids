"""Cloud regression for immutable 7U1-PREP artifact packaging."""
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from package_artifact import (
    PRODUCTION_HANDSHAKE_KIND,
    SYNTHETIC_KIND,
    package,
)
from preflight import PRODUCTION_SOURCE_COPIES, Refused, report, verify_manifest


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
                         False)
        self.assertEqual(receipt["preflightStatus"], "BLOCKED")
        self.assertEqual(receipt["checks"]["artifact_manifest"]["status"], "PASS")
        self.assertEqual(receipt["checks"]["product_native_adapter"]["status"], "BLOCKED")
        self.assertFalse(receipt["owner7U1Accepted"])
        self.assertEqual(receipt["owner7U1Status"], "NOT_RUN")
        self.assertIsNone(receipt["outgoingSendCount"])
        self.assertEqual(
            json.loads((self.art / "artifact-kind.json").read_text())["kind"],
            SYNTHETIC_KIND
        )

    def test_production_handshake_bundle_is_source_bound_but_not_7u1(self):
        source_map = PRODUCTION_SOURCE_COPIES
        for relative in source_map.values():
            path = self.repo / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("synthetic source fixture without observe-only API: " + relative,
                            encoding="utf-8")
        subprocess.run(["git", "-C", str(self.repo), "add", "."], check=True)
        subprocess.run(
            ["git", "-C", str(self.repo), "-c", "user.name=CI",
             "-c", "user.email=ci@example.invalid", "commit", "-q",
             "-m", "add synthetic production-source audit fixtures"],
            check=True
        )
        self.art = self.temp.name and Path(self.temp.name) / "production-artifact"
        (self.art / "host").mkdir(parents=True)
        (self.art / "host" / "WindowsHost.dll").write_bytes(b"host build fixture")
        (self.art / "7l2_bridge.exe").write_bytes(b"rust build fixture")
        for staged, source in source_map.items():
            target = self.art / staged
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((self.repo / source).read_bytes())
        sha = subprocess.run(
            ["git", "-C", str(self.repo), "rev-parse", "HEAD"],
            check=True, capture_output=True, text=True
        ).stdout.strip()
        (self.art / "production-host-audit.json").write_text(json.dumps({
            "sourceSha": sha,
            "productionObservationStatus": "BLOCKED",
            "owner7U1Status": "NOT_RUN",
            "humanConsentSimulated": False,
            "providerAccessPerformed": False,
        }), encoding="utf-8")
        packaged_sha = package(self.repo, self.art, PRODUCTION_HANDSHAKE_KIND)
        self.assertEqual(packaged_sha, sha)
        manifest = verify_manifest(self.art, sha)
        self.assertIn("host/WindowsHost.dll", manifest)
        self.assertIn("7l2_bridge.exe", manifest)
        self.assertEqual(json.loads((self.art / "artifact-kind.json").read_text())["kind"],
                         PRODUCTION_HANDSHAKE_KIND)
        receipt = report(self.repo, self.art, sha, "chatgpt", "7u1-isolated-test01",
                         False)
        self.assertEqual(receipt["preflightStatus"], "BLOCKED")
        self.assertEqual(receipt["checks"]["artifact_manifest"]["status"], "PASS")
        self.assertEqual(receipt["checks"]["product_native_adapter"]["status"], "BLOCKED")
        self.assertEqual(receipt["owner7U1Status"], "NOT_RUN")
        self.assertIsNone(receipt["outgoingSendCount"])

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
        with self.assertRaisesRegex(Refused, "invented product-named"):
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
