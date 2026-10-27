"""Production interface audit regressions; source fixtures only, never owner acceptance."""
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from audit_production_host import SOURCE_FILES, audit

SHA = "a" * 40


class ProductionInterfaceAuditTests(unittest.TestCase):
    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        for relative in SOURCE_FILES:
            path = self.repo / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("", encoding="utf-8")

        (self.repo / "crates/cos-core/src/browser_tools.rs").write_text(
            "pub fn attach(\npub fn snapshot(\npub fn detach(\n"
            "pub fn retire_from_native(", encoding="utf-8")
        (self.repo / "crates/cos-core/examples/7l2_bridge.rs").write_text(
            "ToolAttach ToolSnapshot ToolDetach", encoding="utf-8")
        (self.repo / "apps/windows-host/Native8P.cs").write_text(
            "policy.TryCallback; e.Cancel = true; // never load the code-bearing page",
            encoding="utf-8")
        (self.repo / "probes/7l1/WindowsHost/Program.cs").write_text(
            "Run7L2CdpAsync; synthetic.cos.test; StartUrl", encoding="utf-8")
        (self.repo / "probes/8p/extension/sw.mjs").write_text(
            "bootstrap attestSession", encoding="utf-8")

    def test_rust_core_presence_does_not_mask_missing_production_wiring(self):
        result = audit(self.repo, expected_sha=SHA, observed_sha=SHA)
        self.assertEqual(result["productionObservationStatus"], "BLOCKED")
        self.assertEqual(result["capabilities"]["rust.core_attach"], "SOURCE_MARKER_PRESENT")
        self.assertEqual(result["capabilities"]["rust.core_bounded_snapshot"], "SOURCE_MARKER_PRESENT")
        self.assertEqual(result["capabilities"]["rust.core_detach"], "SOURCE_MARKER_PRESENT")
        self.assertEqual(result["capabilities"]["rust.core_lifecycle_revoke"], "SOURCE_MARKER_PRESENT")
        self.assertIn("native.attach [ObserveOnlyAttachAsync]", result["missingCapabilities"])
        self.assertIn("native.callback_dispatch [Run8PObserveOnlyCdpAsync]", result["missingCapabilities"])
        self.assertIn("mv3.tool_authorization [authorizeTool]", result["missingCapabilities"])
        self.assertTrue(any(
            gap["component"] == "Rust DirectBrowserTools" and
            gap["status"] == "PRESENT_BUT_NOT_WIRED"
            for gap in result["interfaceGaps"]))
        self.assertTrue(any(
            gap["component"] == "signed-in owner navigation" and
            gap["status"] == "BLOCKED_BY_CURRENT_8P_POLICY"
            for gap in result["interfaceGaps"]))
        self.assertEqual(result["owner7U1Status"], "NOT_RUN")
        self.assertFalse(result["owner7U1Accepted"])
        self.assertIsNone(result["outgoingSendCount"])


if __name__ == "__main__":
    unittest.main()
