import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import release_16b as release

SHA = "a" * 40
CERT = "B" * 40


class ReleasePreflightTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.payload = self.root / "cos-rust.exe"
        self.payload.write_bytes(b"synthetic-native-not-a-real-binary")
        self.manifest = {
            "schema": release.SCHEMA, "product": "CoSoS",
            "platform": "windows-x64", "version": "0.2.0", "source_sha": SHA,
            "files": [self.file_entry("cos-rust.exe")],
        }
        self.write_manifest()

    def file_entry(self, name):
        data = (self.root / name).read_bytes()
        return {"path": name, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}

    def write_manifest(self):
        (self.root / "release.json").write_text(json.dumps(self.manifest), encoding="utf-8")

    def inspect(self, **overrides):
        return release.inspect(
            self.root, overrides.get("source", SHA),
            overrides.get("current", "0.1.0"),
            overrides.get("cert", CERT),
            overrides.get("checker", lambda _: CERT),
        )

    def test_synthetic_interface_happy_path_never_installs(self):
        receipt = self.inspect()
        self.assertEqual(receipt["status"], "PASS")
        self.assertEqual(receipt["signed_files"], 1)
        self.assertEqual(self.payload.read_bytes(), b"synthetic-native-not-a-real-binary")

    def test_changed_file_refused(self):
        self.payload.write_bytes(b"changed")
        with self.assertRaisesRegex(release.Refused, "mismatch"):
            self.inspect()

    def test_downgrade_and_equal_versions_refused(self):
        for current in ("0.2.0", "0.3.0"):
            with self.subTest(current=current), self.assertRaisesRegex(release.Refused, "increase"):
                self.inspect(current=current)

    def test_invalid_semver_refused(self):
        self.manifest["version"] = "1.2.3-rc1"
        self.write_manifest()
        with self.assertRaises(release.Refused):
            self.inspect()

    def test_wrong_source_and_target_refused(self):
        with self.assertRaisesRegex(release.Refused, "wrong product"):
            self.inspect(source="b" * 40)
        self.manifest["platform"] = "linux-x64"
        self.write_manifest()
        with self.assertRaisesRegex(release.Refused, "wrong product"):
            self.inspect()

    def test_unmanifested_binary_refused(self):
        (self.root / "extra.dll").write_bytes(b"unsigned")
        with self.assertRaisesRegex(release.Refused, "unmanifested"):
            self.inspect()

    def test_traversal_and_windows_drive_refused(self):
        for evil in ("../escape.exe", "C:/bad.exe", "sub\\bad.exe", "/outside.exe"):
            with self.subTest(evil=evil):
                self.manifest["files"][0]["path"] = evil
                self.write_manifest()
                with self.assertRaises(release.Refused):
                    self.inspect()

    def test_duplicate_manifest_entry_refused(self):
        self.manifest["files"].append(self.manifest["files"][0].copy())
        self.write_manifest()
        with self.assertRaisesRegex(release.Refused, "duplicate"):
            self.inspect()

    def test_unsigned_or_wrong_publisher_refused(self):
        with self.assertRaisesRegex(release.Refused, "thumbprint"):
            self.inspect(checker=lambda _: "C" * 40)
        with mock.patch.object(release.platform, "system", return_value="Linux"):
            with self.assertRaisesRegex(release.Refused, "Windows Authenticode"):
                release.inspect(self.root, SHA, "0.1.0", CERT)

    def test_no_executable_refused(self):
        self.payload.rename(self.root / "LICENSE.txt")
        self.manifest["files"] = [self.file_entry("LICENSE.txt")]
        self.write_manifest()
        with self.assertRaisesRegex(release.Refused, "no native"):
            self.inspect()

    def test_link_refused_if_supported(self):
        alias = self.root / "fake.dll"
        try:
            alias.symlink_to(self.payload)
        except (OSError, NotImplementedError):
            self.skipTest("symlink privilege unavailable")
        with self.assertRaisesRegex(release.Refused, "linked"):
            self.inspect()


if __name__ == "__main__":
    unittest.main()
