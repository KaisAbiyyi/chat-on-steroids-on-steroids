"""Release candidate regression fixtures, never product signing acceptance."""
import tempfile
import unittest
from pathlib import Path
from tools import release_16pkg as pkg

SHA = "a" * 40

class PackageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        self.renderer = root / "renderer"
        self.renderer.mkdir()
        for name in ("WindowsRenderer.exe","WindowsRenderer.dll","WindowsRenderer.deps.json","WindowsRenderer.runtimeconfig.json"):
            (self.renderer / name).write_bytes(name.encode())
        self.runtime = root / "cos-rust.exe"
        self.runtime.write_bytes(b"fixture-runtime")
        self.license = root / "LICENSE"
        self.license.write_bytes(b"fixture-license")
        self.bundle = root / "candidate.zip"

    def build(self):
        return pkg.build(self.runtime,self.renderer,self.license,self.bundle,SHA,"0.1.0")

    def test_deterministic_unsigned_candidate(self):
        result = self.build()
        self.assertEqual(result["status"],"UNSIGNED_CANDIDATE_ONLY")
        self.assertFalse(result["signer_checked"])
        original = self.bundle.read_bytes()
        self.bundle.unlink()
        self.build()
        self.assertEqual(self.bundle.read_bytes(),original)

    def test_missing_renderer_refuses(self):
        (self.renderer / "WindowsRenderer.dll").unlink()
        with self.assertRaises(pkg.Refused):
            self.build()

    def test_version_and_source_refuse(self):
        self.build()
        with self.assertRaises(pkg.Refused):
            pkg.verify(self.bundle,"b" * 40)
        with self.assertRaises(pkg.Refused):
            self.build()
