"""16E synthetic install, upgrade, recovery, downgrade and rollback matrix."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from zipfile import ZipFile, ZIP_STORED

from tools import release_16b, release_16c, release_16e

SHA = "a" * 40
CERT = "B" * 40


class MatrixTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="cosos-16e-fixture-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.matrix = release_16e.DisposableMatrix(SHA, CERT, lambda _: CERT)
        self.addCleanup(self.matrix.close)
        self.n = 0

    def stage(self, version="0.1.0", *, source=SHA, extra=None):
        self.n += 1
        root = self.base / ("release-" + str(self.n))
        root.mkdir()
        (root / "bin").mkdir()
        payload = b"synthetic-exe-not-installable-" + version.encode("ascii")
        (root / "bin" / "cos-rust.exe").write_bytes(payload)
        paths = [{"path": "bin/cos-rust.exe", "bytes": len(payload),
                  "sha256": hashlib.sha256(payload).hexdigest()}]
        if extra:
            (root / "LICENSE.txt").write_text(extra, encoding="utf-8")
            raw = extra.encode("utf-8")
            paths.append({"path": "LICENSE.txt", "bytes": len(raw),
                          "sha256": hashlib.sha256(raw).hexdigest()})
        manifest = {"schema": release_16b.SCHEMA, "product": "CoSoS",
                    "platform": "windows-x64", "version": version,
                    "source_sha": source, "files": paths}
        (root / "release.json").write_text(json.dumps(manifest), encoding="utf-8")
        return root

    def test_clean_install_preserves_stage_and_disposable_state(self):
        src = self.stage()
        assert not self.matrix.selector.exists()
        receipt = self.matrix.install_or_upgrade(src)
        self.assertEqual(receipt["status"], "PASS")
        self.assertEqual(receipt["version"], "0.1.0")
        self.assertIsNone(self.matrix.state()["previous"])
        self.assertTrue((src / "bin/cos-rust.exe").exists())
        self.assertIn("cosos-16e-synthetic-", str(self.matrix.root))

    def test_upgrade_and_verified_rollback_selects_retained_release(self):
        self.matrix.install_or_upgrade(self.stage("0.1.0"))
        old = self.matrix.state()["active"]
        self.matrix.install_or_upgrade(self.stage("0.2.0", extra="fixture notice"))
        latest = self.matrix.state()["active"]
        self.assertEqual(self.matrix.state()["previous"], old)
        self.assertNotEqual(old, latest)
        self.assertEqual(self.matrix.rollback()["version"], "0.1.0")
        self.assertEqual(self.matrix.state(), {"active": old, "previous": latest})
        self.assertEqual(self.matrix.current()["version"], "0.1.0")
        # This is explicit slot selection, not a 16B downgrade installation.

    def test_downgrade_or_same_version_update_rejected(self):
        self.matrix.install_or_upgrade(self.stage("0.2.0"))
        before = self.matrix.state()
        for version in ("0.1.0", "0.2.0"):
            with self.subTest(version=version):
                with self.assertRaisesRegex(release_16e.RehearsalRefused, "increase"):
                    self.matrix.install_or_upgrade(self.stage(version))
                self.assertEqual(self.matrix.state(), before)

    def test_rollback_without_known_previous_refused(self):
        with self.assertRaisesRegex(release_16e.RehearsalRefused, "no known"):
            self.matrix.rollback()
        self.matrix.install_or_upgrade(self.stage())
        with self.assertRaises(release_16e.RehearsalRefused):
            self.matrix.rollback()

    def test_failure_injection_before_publication_keeps_active(self):
        self.matrix.install_or_upgrade(self.stage())
        old = self.matrix.state()
        for failpoint in ("after_preflight", "after_copy", "before_publish"):
            with self.subTest(point=failpoint):
                with self.assertRaises(release_16e.InjectedCrash):
                    self.matrix.install_or_upgrade(self.stage("0.2.0"), fail_at=failpoint)
                self.assertEqual(self.matrix.state(), old)
                self.assertEqual(self.matrix.current()["version"], "0.1.0")

    def test_failure_after_atomic_publication_recovers_new_or_old_complete(self):
        self.matrix.install_or_upgrade(self.stage())
        with self.assertRaises(release_16e.InjectedCrash):
            self.matrix.install_or_upgrade(self.stage("0.2.0"), fail_at="after_publish")
        self.assertEqual(self.matrix.current()["version"], "0.2.0")
        self.assertEqual(self.matrix.rollback()["version"], "0.1.0")
        self.assertEqual(self.matrix.current()["version"], "0.1.0")

    def test_rollback_crashes_have_unambiguous_selector(self):
        self.matrix.install_or_upgrade(self.stage("0.1.0"))
        self.matrix.install_or_upgrade(self.stage("0.2.0"))
        with self.assertRaises(release_16e.InjectedCrash):
            self.matrix.rollback(fail_at="before_publish")
        self.assertEqual(self.matrix.current()["version"], "0.2.0")
        with self.assertRaises(release_16e.InjectedCrash):
            self.matrix.rollback(fail_at="after_publish")
        self.assertEqual(self.matrix.current()["version"], "0.1.0")

    def test_invalid_product_source_and_hash_rejected(self):
        for change in ("source", "content", "extra"):
            with self.subTest(change=change):
                src = self.stage()
                if change == "source":
                    obj = json.loads((src / "release.json").read_text("utf-8"))
                    obj["source_sha"] = "c" * 40
                    (src / "release.json").write_text(json.dumps(obj), encoding="utf-8")
                elif change == "content":
                    (src / "bin/cos-rust.exe").write_bytes(b"changed")
                else:
                    (src / "unmanifested.dll").write_bytes(b"unmanifested")
                with self.assertRaises(release_16e.RehearsalRefused):
                    self.matrix.install_or_upgrade(src)
                self.assertFalse(self.matrix.selector.exists())

    def test_bad_signer_refuses_even_synthetic_install(self):
        matrix = release_16e.DisposableMatrix(SHA, CERT, lambda _: "C" * 40)
        self.addCleanup(matrix.close)
        with self.assertRaisesRegex(release_16e.RehearsalRefused, "thumbprint"):
            matrix.install_or_upgrade(self.stage())

    def test_corrupted_previous_release_cannot_rollback(self):
        self.matrix.install_or_upgrade(self.stage("0.1.0"))
        self.matrix.install_or_upgrade(self.stage("0.2.0"))
        original = self.matrix.state()
        old = self.matrix.slots / original["previous"] / "bin" / "cos-rust.exe"
        old.write_bytes(b"tamper")
        with self.assertRaises(release_16e.RehearsalRefused):
            self.matrix.rollback()
        self.assertEqual(self.matrix.state(), original)
        self.assertEqual(self.matrix.current()["version"], "0.2.0")

    def test_selector_tamper_refused_before_operation(self):
        self.matrix.install_or_upgrade(self.stage())
        for bad in (
            '{"active":"../../outside","previous":null}',
            '{"active":"slot-123","previous":null}',
            '{"active":null,"previous":null}',
            '{"active":"slot-123","previous":null,"unexpected":1}',
            "{this is not json}",
        ):
            with self.subTest(selector=bad):
                self.matrix.selector.write_text(bad, encoding="utf-8")
                with self.assertRaises(release_16e.RehearsalRefused):
                    self.matrix.install_or_upgrade(self.stage("0.2.0"))

    def test_symlink_in_stage_refused_when_supported(self):
        src = self.stage()
        alias = src / "linked.dll"
        try:
            alias.symlink_to(src / "bin" / "cos-rust.exe")
        except (OSError, NotImplementedError):
            self.skipTest("OS does not permit symlink fixture")
        with self.assertRaises(release_16e.RehearsalRefused):
            self.matrix.install_or_upgrade(src)

    def test_real_unsigned_authenticode_is_not_mocked_by_default(self):
        # No real signing key, native publisher, or end-user installer in CI.
        src = self.stage()
        from unittest import mock
        with mock.patch.object(release_16b.platform, "system", return_value="Linux"):
            with self.assertRaisesRegex(release_16b.Refused, "Windows Authenticode"):
                release_16b.inspect(src, SHA, "0.0.0", CERT)

    def test_headless_bundle_contract_cannot_be_native_installed(self):
        members = {"cos-rust.exe": b"fake-runtime", "LICENSE": b"MIT license",
                   "THIRD_PARTY_NOTICES.txt": b"synthetic-only notices"}
        index = {"schema": release_16c.SCHEMA, "source_sha": SHA,
                 "platform": "windows-x64", "binary": "cos-rust.exe",
                 "signature": "NOT_SIGNED", "native_host": "NOT_INCLUDED",
                 "webview2_runtime": "EXTERNAL_NOT_VERIFIED",
                 "packages": [{"name": "fixture"}],
                 "files": [{"path": k, "bytes": len(v),
                            "sha256": hashlib.sha256(v).hexdigest()}
                           for k, v in members.items()]}
        zip_path = self.base / "headless.zip"
        with ZipFile(zip_path, "w", ZIP_STORED) as archive:
            for name, raw in members.items():
                archive.writestr(name, raw)
            archive.writestr("bundle-index.json", json.dumps(index))
        result = release_16c.verify(zip_path, SHA)
        self.assertEqual(result["status"], "PASS")
        self.assertIn("BLOCKED", result["native_release"])
        with self.assertRaises(release_16e.RehearsalRefused):
            self.matrix.install_or_upgrade(zip_path)
        with self.assertRaises(release_16c.Refused):
            release_16c.verify(zip_path, "c" * 40)

    def test_after_publish_selector_is_not_a_profile_migration(self):
        src = self.stage()
        self.matrix.install_or_upgrade(src)
        self.assertFalse(any((self.matrix.root / name).exists()
                             for name in ("sessions", "state", "secrets.bin", "Local State")))
        # Real source/target profile backup/restore regression is Rust 16A,
        # run separately in this task's hosted workflow.


if __name__ == "__main__":
    unittest.main()
