import hashlib
import json
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path
from zipfile import ZipFile

from tools import release_16c as bundle

SHA = "a" * 40


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.pkg = self.root / "dependency"
        self.pkg.mkdir()
        (self.pkg / "LICENSE-MIT").write_text("Synthetic permissive notice\n", encoding="utf-8")
        (self.pkg / "Cargo.toml").write_text("[package]\nname='dummy'\nversion='0.1.0'\n", encoding="utf-8")
        self.exe = self.root / "cos-rust.exe"
        self.exe.write_bytes(b"synthetic-not-a-real-executable")
        self.license = self.root / "LICENSE"
        self.license.write_text("MIT License (synthetic)\n", encoding="utf-8")
        self.meta = {"packages": [{
            "name": "dummy", "version": "0.1.0", "license": "MIT OR Apache-2.0",
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "manifest_path": str(self.pkg / "Cargo.toml")
        }]}
        self.lock = {"version": 4, "package": [{
            "name": "dummy", "version": "0.1.0",
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "checksum": "b" * 64
        }]}
        self.out = self.root / "release.zip"

    def build(self):
        return bundle.build(self.exe, self.license, self.meta, self.lock,
                            self.out, "windows-x64", SHA)

    def test_deterministic_source_and_license_receipt(self):
        result = self.build()
        self.assertEqual(result["status"], "PASS")
        self.assertEqual(result["dependencies"], 1)
        self.assertIn("BLOCKED", result["native_release"])
        with ZipFile(self.out) as z:
            notices = z.read("THIRD_PARTY_NOTICES.txt")
            self.assertIn(b"Synthetic permissive notice", notices)
            index = json.loads(z.read("bundle-index.json"))
            self.assertEqual(index["signature"], "NOT_SIGNED")
            self.assertEqual(index["source_sha"], SHA)
        previous_digest = hashlib.sha256(self.out.read_bytes()).hexdigest()
        self.out.unlink()
        self.build()
        self.assertEqual(hashlib.sha256(self.out.read_bytes()).hexdigest(), previous_digest)

    def test_runtime_dependency_graph_excludes_dev_only(self):
        pkgs = [
            {"id": "app", "name": "cos-app", "source": None},
            {"id": "runtime", "name": "runtime-dep", "source": "registry+ok"},
            {"id": "dev", "name": "dev-dep", "source": "registry+ok"}
        ]
        metadata = {"packages": pkgs, "resolve": {"nodes": [
            {"id": "app", "deps": [
                {"pkg": "runtime", "dep_kinds": [{"kind": None}]},
                {"pkg": "dev", "dep_kinds": [{"kind": "dev"}]}
            ]},
            {"id": "runtime", "deps": []},
            {"id": "dev", "deps": []}
        ]}}
        selected = bundle.selected_runtime_packages(metadata)
        self.assertEqual([p["name"] for p in selected], ["cos-app", "runtime-dep"])

    def test_missing_application_root_refused(self):
        with self.assertRaisesRegex(bundle.Refused, "application root"):
            bundle.selected_runtime_packages({
                "packages": [{"id": "other", "name": "other", "source": None}],
                "resolve": {"nodes": [{"id": "other", "deps": []}]}
            })

    def test_unknown_license_refused(self):
        self.meta["packages"][0]["license"] = "GPL-3.0"
        with self.assertRaisesRegex(bundle.Refused, "SPDX"):
            self.build()

    def test_approved_llvm_exception_is_explicit(self):
        self.meta["packages"][0]["license"] = "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT"
        self.assertEqual(self.build()["status"], "PASS")

    def test_explicit_legacy_spdx_dual_license(self):
        self.meta["packages"][0]["license"] = "MIT/Apache-2.0"
        self.assertEqual(self.build()["status"], "PASS")

    def test_spdx_permissive_alternative_with_lgpl(self):
        self.meta["packages"][0]["license"] = "MIT OR Apache-2.0 OR LGPL-2.1-or-later"
        self.assertEqual(self.build()["status"], "PASS")

    def test_spdx_legacy_unlicense_dual(self):
        self.meta["packages"][0]["license"] = "Unlicense/MIT"
        self.assertEqual(self.build()["status"], "PASS")

    def test_spdx_parenthesized_and_precedence(self):
        self.meta["packages"][0]["license"] = "(MIT OR Apache-2.0) AND Unicode-3.0"
        self.assertEqual(self.build()["status"], "PASS")

    def test_spdx_rejects_mandatory_incompatible_license(self):
        self.meta["packages"][0]["license"] = "MIT AND LGPL-2.1-or-later"
        with self.assertRaisesRegex(bundle.Refused, "license audit blocked"):
            self.build()

    def test_spdx_rejects_unknown_exception_in_or(self):
        self.meta["packages"][0]["license"] = "MIT OR Apache-2.0 WITH Unknown-exception"
        with self.assertRaisesRegex(bundle.Refused, "unapproved license exception"):
            self.build()

    def test_unreviewed_exception_refused(self):
        self.meta["packages"][0]["license"] = "Apache-2.0 WITH Unknown-exception OR MIT"
        with self.assertRaisesRegex(bundle.Refused, "unapproved"):
            self.build()

    def test_workspace_uses_root_license(self):
        self.meta["packages"][0]["source"] = None
        self.lock["package"][0]["source"] = None
        self.lock["package"][0].pop("checksum")
        (self.pkg / "LICENSE-MIT").unlink()
        self.assertEqual(self.build()["status"], "PASS")

    def test_same_source_workspace_sibling_license_notice(self):
        sibling = self.root / "sibling"
        sibling.mkdir()
        (sibling / "Cargo.toml").write_text("[package]\\nname='rquickjs'\\n", encoding="utf-8")
        (sibling / "LICENSE").write_text("Sibling canonical MIT notice", encoding="utf-8")
        self.meta["packages"][0]["repository"] = "https://example.org/canonical"
        self.meta["packages"].append({
            "name": "sibling", "version": "0.1.0", "license": "MIT OR Apache-2.0",
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "manifest_path": str(sibling / "Cargo.toml"),
            "repository": "https://example.org/canonical"
        })
        self.lock["package"].append({
            "name": "sibling", "version": "0.1.0",
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "checksum": "c" * 64
        })
        (self.pkg / "LICENSE-MIT").unlink()
        self.assertEqual(self.build()["status"], "PASS")

    def test_missing_license_text_refused(self):
        (self.pkg / "LICENSE-MIT").unlink()
        with self.assertRaisesRegex(bundle.Refused, "missing license"):
            self.build()

    def test_unpinned_source_refused(self):
        self.lock["package"][0].pop("checksum")
        with self.assertRaisesRegex(bundle.Refused, "checksum"):
            self.build()

    def test_git_dependency_refused(self):
        self.meta["packages"][0]["source"] = "git+https://example.com/a"
        self.lock["package"][0]["source"] = "git+https://example.com/a"
        with self.assertRaisesRegex(bundle.Refused, "non-registry"):
            self.build()

    def test_existing_output_never_overwritten(self):
        self.out.write_bytes(b"preserve me")
        with self.assertRaisesRegex(bundle.Refused, "overwrite"):
            self.build()
        self.assertEqual(self.out.read_bytes(), b"preserve me")

    def test_binary_mismatch_refused(self):
        self.exe.rename(self.root / "wrong.exe")
        with self.assertRaisesRegex(bundle.Refused, "missing"):
            self.build()

    def test_bad_commit_refused(self):
        with self.assertRaises(bundle.Refused):
            bundle.build(self.exe, self.license, self.meta, self.lock,
                         self.out, "windows-x64", "bad")

    def test_undocumented_payload_refused(self):
        self.build()
        with ZipFile(self.out, "a") as z:
            z.writestr("secret.txt", "not allowed")
        with self.assertRaisesRegex(bundle.Refused, "index coverage"):
            bundle.verify(self.out, SHA)

    def test_tampered_content_refused(self):
        self.build()
        contents = {}
        with ZipFile(self.out) as z:
            for name in z.namelist():
                contents[name] = z.read(name)
        contents["cos-rust.exe"] = b"modified"
        self.out.unlink()
        with ZipFile(self.out, "w") as z:
            for name in sorted(contents):
                z.writestr(name, contents[name])
        with self.assertRaisesRegex(bundle.Refused, "checksum"):
            bundle.verify(self.out, SHA)


    def test_forged_index_without_runtime_binary_refused(self):
        self.build()
        with ZipFile(self.out) as z:
            members = {name: z.read(name) for name in z.namelist()}
        index = json.loads(members["bundle-index.json"])
        index["files"] = [item for item in index["files"] if item["path"] != "cos-rust.exe"]
        members.pop("cos-rust.exe")
        members["bundle-index.json"] = json.dumps(index).encode("utf-8")
        self.out.unlink()
        with ZipFile(self.out, "w") as z:
            for name, content in sorted(members.items()):
                z.writestr(name, content)
        with self.assertRaisesRegex(bundle.Refused, "required bundle payload"):
            bundle.verify(self.out, SHA)

    def test_forged_platform_binary_refused(self):
        self.build()
        with ZipFile(self.out) as z:
            members = {name: z.read(name) for name in z.namelist()}
        index = json.loads(members["bundle-index.json"])
        index["platform"] = "macos-arm64"
        members["bundle-index.json"] = json.dumps(index).encode("utf-8")
        self.out.unlink()
        with ZipFile(self.out, "w") as z:
            for name, content in sorted(members.items()):
                z.writestr(name, content)
        with self.assertRaisesRegex(bundle.Refused, "platform/binary"):
            bundle.verify(self.out, SHA)

    def test_competing_file_creation_race_never_deleted(self):
        def competing_writer(*args, **kwargs):
            self.out.write_bytes(b"created by another process")
            raise FileExistsError("exclusive create lost the race")
        with patch.object(bundle, "ZipFile", side_effect=competing_writer):
            with self.assertRaises(FileExistsError):
                self.build()
        self.assertEqual(self.out.read_bytes(), b"created by another process")

    def test_wrong_source_refused(self):
        self.build()
        with self.assertRaisesRegex(bundle.Refused, "provenance"):
            bundle.verify(self.out, "b" * 40)


if __name__ == "__main__":
    unittest.main()
