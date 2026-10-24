"""Fail-closed 7U1-PREP tests: synthetic fixtures ONLY."""
import hashlib
import json
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from preflight import report


class TestOwnerPreflight(unittest.TestCase):
    SHA = "a" * 40

    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.repo = root / "repo"
        self.repo.mkdir()
        self.art = root / "artifact"
        (self.art / "host").mkdir(parents=True)
        (self.art / "source.sha").write_text(self.SHA, encoding="utf-8")
        (self.art / "host" / "CoSoSProductBrowser.dll").write_bytes(b"test-only")
        (self.art / "7u1_product_bridge.exe").write_bytes(b"test-only")
        (self.repo / "probes" / "7u1").mkdir(parents=True)
        self.contract = dict(
            schema="cosos.7u1.product-integration.v1",
            kind="product-webview2-rust-mv3",
            host="host/CoSoSProductBrowser.dll",
            bridge="7u1_product_bridge.exe",
            allowedOrigins=["https://chatgpt.com"],
            allowedOps=["attach", "snapshot", "detach"],
            captureLimitBytes=65536,
            noContentPersistence=True,
            navigation="owner-only",
            profile="fresh-isolated-only",
            consent="per-session-explicit",
            sendCapability="absent",
        )
        self.save()
        self.rehash()

    def save(self):
        (self.repo / "probes" / "7u1" / "product-integration.json").write_text(
            json.dumps(self.contract), encoding="utf-8")

    def rehash(self):
        entries = []
        for p in sorted(self.art.rglob("*")):
            if p.is_file() and p.name != "hashes.json":
                entries.append({"path": p.relative_to(self.art).as_posix(),
                                "sha256": hashlib.sha256(p.read_bytes()).hexdigest()})
        (self.art / "hashes.json").write_text(json.dumps(entries), encoding="utf-8")

    def gate(self, provider="chatgpt", profile="7u1-fresh001", consent=True, sha=None):
        return report(self.repo, self.art, sha or self.SHA, provider, profile,
                      consent, observed_sha=self.SHA)

    def test_metadata_pass_never_fakes_real_owner(self):
        x = self.gate()
        self.assertEqual(x["preflightStatus"], "PREP_READY_REVIEW_REQUIRED")
        self.assertFalse(x["owner7U1Accepted"])
        self.assertEqual(x["owner7U1Status"], "NOT_RUN")
        self.assertIsNone(x["outgoingSendCount"])

    def test_missing_native_product_integration_blocked(self):
        (self.repo / "probes" / "7u1" / "product-integration.json").unlink()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")

    def test_synthetic_7l2_bridge_not_product(self):
        self.contract["host"] = "host/WindowsHost.dll"
        self.contract["bridge"] = "7l2_bridge.exe"
        self.save()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")

    def test_no_send_or_arbitrary_script(self):
        self.contract["allowedOps"].append("send")
        self.save()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")
        self.contract["allowedOps"] = ["attach", "snapshot", "detach", "evaluate"]
        self.save()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")

    def test_no_foreign_origin(self):
        self.contract["allowedOrigins"] = ["https://attacker.test"]
        self.save()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")
        self.assertEqual(self.gate(provider="attacker")["preflightStatus"], "BLOCKED")

    def test_no_automatic_navigation_or_persistence(self):
        self.contract["navigation"] = "automatic"
        self.save()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")
        self.contract["navigation"] = "owner-only"
        self.contract["noContentPersistence"] = False
        self.save()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")

    def test_no_default_profile_or_implicit_consent(self):
        self.assertEqual(self.gate(profile="Default")["preflightStatus"], "BLOCKED")
        self.assertEqual(self.gate(consent=False)["preflightStatus"], "BLOCKED")

    def test_manifest_file_mutation_and_unlisted_file(self):
        (self.art / "7u1_product_bridge.exe").write_bytes(b"modified")
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")
        self.rehash()
        (self.art / "unknown.dll").write_bytes(b"extra")
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")

    def test_wrong_commit_and_missing_manifest(self):
        self.assertEqual(self.gate(sha="b" * 40)["preflightStatus"], "BLOCKED")
        (self.art / "hashes.json").unlink()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")

    def test_manifest_traversal_and_unknown_fields_denied(self):
        path = self.art / "hashes.json"
        entries = json.loads(path.read_text(encoding="utf-8"))
        entries[0]["path"] = "../outside.exe"
        path.write_text(json.dumps(entries), encoding="utf-8")
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")
        self.rehash()
        entries = json.loads(path.read_text(encoding="utf-8"))
        entries[0]["extra"] = "untrusted"
        path.write_text(json.dumps(entries), encoding="utf-8")
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")

    def test_extra_product_declaration_fields_denied(self):
        self.contract["unsafeOverride"] = True
        self.save()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")

    def test_symlink_artifact_denied(self):
        p = self.art / "host" / "CoSoSProductBrowser.dll"
        p.unlink()
        try:
            p.symlink_to(self.art / "7u1_product_bridge.exe")
        except (OSError, NotImplementedError):
            self.skipTest("Windows runner disallows symlink creation")
        self.rehash()
        self.assertEqual(self.gate()["preflightStatus"], "BLOCKED")


if __name__ == "__main__":
    unittest.main()
