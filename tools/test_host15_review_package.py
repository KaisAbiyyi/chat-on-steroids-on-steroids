"""Negative checks for 15HOST screenshot candidate provenance."""
import hashlib
import json
import pathlib
import struct
import tempfile
import unittest
from tools import host15_review_package as reviewer

HEAD = "a" * 40


class ReviewEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = pathlib.Path(self.tmp.name)
        self.png = b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">IIBBBBB", 800, 600, 8, 6, 0, 0, 0) + b"\x00" * 4
        for surface in reviewer.SURFACES:
            kind, profile = surface.rsplit("-", 1)
            path = self.root / ("15host-product-" + surface)
            path.with_suffix(".png").write_bytes(self.png)
            path.with_suffix(".json").write_text(json.dumps({
                "sourceSha": HEAD, "surface": kind, "profile": profile,
                "pngSha256": hashlib.sha256(self.png).hexdigest(),
                "productState": True, "syntheticData": True,
                "approvedGolden": False, "productAccepted": False,
            }), encoding="utf-8")

    def run_review(self, compare=None):
        return reviewer.generate(self.root, HEAD, "windows-latest", self.root / "index.json", compare)

    def test_seven_candidates_are_validated_but_not_approved(self):
        result = self.run_review()
        self.assertEqual(result["surfaceCount"], 7)
        self.assertFalse(result["approvedGolden"])
        self.assertFalse(result["productAccepted"])
        self.assertEqual(result["comparison"], "NOT_RUN_NO_INDEPENDENT_APPROVED_BASELINE")

    def test_tampered_candidate_png_rejected(self):
        (self.root / "15host-product-pdf-b.png").write_bytes(self.png + b"x")
        with self.assertRaisesRegex(ValueError, "pngSha256"):
            self.run_review()

    def test_forged_approval_flags_rejected(self):
        receipt = self.root / "15host-product-editor-a.json"
        data = json.loads(receipt.read_text())
        data["approvedGolden"] = True
        receipt.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "approvedGolden"):
            self.run_review()

    def test_wrong_source_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "sourceSha"):
            reviewer.generate(self.root, "b" * 40, "win", self.root / "index.json")

    def test_missing_candidate_is_rejected(self):
        (self.root / "15host-product-math-single.png").unlink()
        with self.assertRaises(FileNotFoundError):
            self.run_review()

    def test_comparator_is_not_approval(self):
        result = self.run_review(compare=self.root)
        self.assertEqual(result["comparison"], "MATCHED_EXTERNAL_BASELINE_BYTES_UNAPPROVED")
        self.assertFalse(result["productAccepted"])


if __name__ == "__main__":
    unittest.main()
