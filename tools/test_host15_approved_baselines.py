"""Fail-closed negative regression for independently approved native baselines."""
import copy
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

from tools import host15_approved_baselines as gate


def pixel_png(red):
    raw = b"\x00" + bytes([red, 0, 0, 255])
    def chunk(t, data):
        return struct.pack(">I", len(data)) + t + data + struct.pack(">I", zlib.crc32(t + data) & 0xffffffff)
    return (gate.MAGIC + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def fixture_manifest():
    return {"schema": "15host-independent-approved-baseline-v1", "repo": gate.REPO,
            "approvedSourceSha": gate.SOURCE, "artifactId": gate.ARTIFACT_ID,
            "artifactZipSha256": gate.ARCHIVE_SHA,
            "approvalCommentId": gate.COMMENT_ID,
            "approvalAuthor": gate.REVIEWER, "approvalCreatedAt": gate.REVIEWED_AT,
            "approvalCommentUrl": "https://github.com/" + gate.REPO +
                                  "/pull/127#issuecomment-" + str(gate.COMMENT_ID),
            "screenshots": {s: "a" * 64 for s in gate.SURFACES}}


def approval(m):
    return {"id": gate.COMMENT_ID, "html_url": m["approvalCommentUrl"],
            "user": {"login": gate.REVIEWER}, "author_association": "OWNER",
            "created_at": gate.REVIEWED_AT,
            "body": "**Independent visual inspection & baseline adoption: APPROVED" +
                    " " + gate.SOURCE + " pixel-identical\n" +
                    "\n".join("- " + s + " " + chr(96) + m["screenshots"][s] + chr(96)
                              for s in gate.SURFACES)}


class TestApprovedBaseline(unittest.TestCase):
    def test_reference_manifest_source_and_pin(self):
        m = fixture_manifest()
        gate.verify_manifest(m)
        for key, value in (("artifactId", 1), ("approvedSourceSha", "0"*40),
                           ("approvalCommentId", 12), ("artifactZipSha256", "f"*64)):
            mutated = copy.deepcopy(m)
            mutated[key] = value
            with self.assertRaises(ValueError, msg=key):
                gate.verify_manifest(mutated)

    def test_forged_approval_rejected(self):
        m = fixture_manifest()
        gate.verify_approval(approval(m), m)
        for field, val in (("id", 111), ("author_association", "CONTRIBUTOR"),
                           ("created_at", "2000-01-01T00:00:00Z"),
                           ("body", "I approve unrelated screenshots")):
            wrong = approval(m)
            wrong[field] = val
            with self.assertRaises(ValueError, msg=field):
                gate.verify_approval(wrong, m)
        wrong = approval(m)
        wrong["user"]["login"] = "attacker"
        with self.assertRaises(ValueError):
            gate.verify_approval(wrong, m)

    def test_pixel_mismatch_rejected(self):
        a, b = pixel_png(0), pixel_png(1)
        self.assertEqual(gate.exact_match(a, a, "editor-a")["comparison"],
                         "PASS_EXACT_BYTES")
        with self.assertRaisesRegex(ValueError, "pixel drift"):
            gate.exact_match(a, b, "editor-a")

    def test_known_single_lsb_noncontent_chrome_only(self):
        def large_png(changes):
            w, h = 1028, 749
            rows = bytearray()
            for y in range(h):
                row = bytearray(w * 3)
                for (x1, y1), delta in changes.items():
                    if y == y1:
                        row[x1 * 3] = delta
                rows.extend(b"\x00" + row)
            def chunk(tag, body):
                return (struct.pack(">I", len(body)) + tag + body +
                        struct.pack(">I", zlib.crc32(tag + body) & 0xffffffff))
            return (gate.MAGIC + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) +
                    chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))
        base = large_png({})
        harmless = large_png({(9, 395): 1})
        self.assertEqual(gate.exact_match(base, harmless, "editor-b")["comparison"],
                         "PASS_BOUNDED_1LSB_EDGE")
        for changed in ({(9, 395): 2}, {(20, 395): 1}, {(9, 391): 1}):
            with self.assertRaisesRegex(ValueError, "pixel drift"):
                gate.exact_match(base, large_png(changed), "editor-b")

    def test_missing_baselines_archive_and_wrong_sha_rejected(self):
        m = fixture_manifest()
        fake = io.BytesIO()
        with __import__("zipfile").ZipFile(fake, "w") as z:
            z.writestr("15host-product-editor-a.png", pixel_png(1))
        with self.assertRaisesRegex(ValueError, "SHA/size"):
            gate.verify_reference_archive(fake.getvalue(), m)
        with self.assertRaisesRegex(ValueError, "SHA/size"):
            gate.verify_reference_archive(b"", m)

    def test_wrong_source_or_missing_candidate_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            gate.ensure_distinct(root / "ref", root / "candidate")
            with self.assertRaisesRegex(ValueError, "Same-directory"):
                gate.ensure_distinct(root, root)
            with self.assertRaisesRegex(ValueError, "nested"):
                gate.ensure_distinct(root, root / "capture")
            with self.assertRaises(ValueError):
                gate.compare({}, root / "candidate", "wrong")
            with self.assertRaises(FileNotFoundError):
                gate.compare({}, root / "candidate", "1"*40)

    def test_checksum_and_focus_pinning(self):
        m = fixture_manifest()
        m["screenshots"]["focus-single"] = "b"*64
        with self.assertRaisesRegex(ValueError, "Focus"):
            gate.verify_manifest(m)
        m = fixture_manifest()
        m["screenshots"]["math-single"] = "b"*64
        with self.assertRaisesRegex(ValueError, "Focus"):
            gate.verify_manifest(m)


if __name__ == "__main__":
    unittest.main()
