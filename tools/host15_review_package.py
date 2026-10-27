"""15HOST native PRODUCT-state review-package integrity gate.

Produces candidate-only evidence; it NEVER creates or approves goldens.
All verification is strict and uses only Python stdlib.
"""
import argparse
import hashlib
import json
import pathlib
import struct
import sys

SURFACES = (
    "editor-a", "editor-b", "terminal-a", "terminal-b",
    "pdf-a", "pdf-b", "math-single", "locale-single", "focus-single",
)
PREFIX = "15host-product-"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def verify_folder(directory, source_sha):
    if len(source_sha) != 40 or any(char not in "0123456789abcdef" for char in source_sha):
        raise ValueError("Expected exact lowercase 40-hex source SHA")
    records = []
    for surface in SURFACES:
        name = PREFIX + surface
        receipt_path = directory / (name + ".json")
        png_path = directory / (name + ".png")
        raw_receipt = receipt_path.read_bytes()
        receipt = json.loads(raw_receipt)
        png = png_path.read_bytes()
        if len(png) < 33 or png[:8] != PNG_MAGIC or png[12:16] != b"IHDR":
            raise ValueError("Invalid native PNG: " + surface)
        width, height = struct.unpack(">II", png[16:24])
        if width < 1 or height < 1 or width > 3840 or height > 2160 or len(png) > 16_777_216:
            raise ValueError("Native PNG outside pixel/byte budget: " + surface)
        kind, profile = surface.rsplit("-", 1)
        expected = {
            "sourceSha": source_sha, "surface": kind, "profile": profile,
            "productState": True, "syntheticData": True,
            "approvedGolden": False, "productAccepted": False,
            "pngSha256": sha256(png),
        }
        for key, value in expected.items():
            if receipt.get(key) != value:
                raise ValueError("Candidate evidence mismatch " + surface + ":" + key)
        records.append({
            "surface": kind, "profile": profile, "file": png_path.name,
            "width": width, "height": height, "pngSha256": sha256(png),
            "receiptSha256": sha256(raw_receipt),
        })
    return records


def generate(directory, source_sha, environment, output, compare_dir=None):
    records = verify_folder(directory, source_sha)
    comparison = "NOT_RUN_NO_INDEPENDENT_APPROVED_BASELINE"
    if compare_dir is not None:
        # Strict byte identity is a deterministic regression comparator.
        # Even a match is NOT independent visual approval or a provenance check.
        for entry in records:
            candidate = (directory / entry["file"]).read_bytes()
            baseline = (compare_dir / entry["file"]).read_bytes()
            if candidate != baseline:
                raise ValueError("Native baseline mismatch: " + entry["file"])
        comparison = "MATCHED_EXTERNAL_BASELINE_BYTES_UNAPPROVED"
    index = {
        "schema": "15host-product-review-v1",
        "sourceHead": source_sha,
        "captureEnvironment": environment,
        "captureType": "native C# WebView2 hosted Rust authenticated synthetic product state",
        "specification": "GitHub issue #115; accepted 15F UI snapshot contracts",
        "acceptanceCriteria": [
            "independent authority checks each visual baseline and capture provenance",
            "editor and terminal recorded output (not interactive shell) visible",
            "PDF image is host-authorized pre-rasterized PNG (not native PDF parser)",
            "math is restricted safe MathML (not full TeX)",
            "A/B profile sentinel isolation and captured last-locale native PNG",
            "13 native rendered locales and focused MathML activeElement plus native focus PNG",
            "zero unexpected differences from independently approved PRODUCT-state goldens",
        ],
        "surfaceCount": len(records),
        "surfaces": records,
        "comparison": comparison,
        "approvedGolden": False,
        "productAccepted": False,
        "requiredIndependentReviewer": "ChatGPT independent reviewer, not implementation author",
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return index


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=pathlib.Path)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--environment", default="GitHub Actions windows-latest/WebView2/.NET 8")
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--compare-dir", type=pathlib.Path)
    args = parser.parse_args()
    try:
        index = generate(args.input, args.source_sha, args.environment, args.output, args.compare_dir)
        print(json.dumps({"reviewPackage": str(args.output), "surfaces": index["surfaceCount"],
                          "comparison": index["comparison"], "approvedGolden": False}))
    except (ValueError, OSError, KeyError, json.JSONDecodeError) as exc:
        print("15HOST candidate review gate FAILED: " + str(exc), file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
