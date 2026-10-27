"""Fail-closed 15HOST independent visual baseline regression, Python stdlib only.

A known reviewer-approved GitHub Actions artifact is downloaded into a separate
runner-temp directory, its exact archive and 9 PNG SHA256s are verified against
an authoritative GitHub issue comment, and NEW hosted WebView2 captures are
compared byte-for-byte. An expired artifact fails closed; it is never recreated.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import struct
import sys
import urllib.request
from urllib.parse import urlparse
import zipfile

REPO = "KaisAbiyyi/chat-on-steroids-on-steroids"
SOURCE = "d0830f61597c8e32863d5b84333e68d4cf336125"
ARTIFACT_ID = 12739726480
ARCHIVE_SHA = "0735a269cf1c1ad2039ec5af3fbbbb42dabbcdead9fa36e92c14eaeb088c3345"
COMMENT_ID = 6444384906
REVIEWER = "KaisAbiyyi"
REVIEWED_AT = "2026-10-27T14:17:47Z"
SURFACES = ("editor-a", "editor-b", "terminal-a", "terminal-b", "pdf-a",
            "pdf-b", "math-single", "locale-single", "focus-single")
MAGIC = b"\x89PNG\r\n\x1a\n"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verify_manifest(m):
    if (m.get("schema") != "15host-independent-approved-baseline-v1"
            or m.get("repo") != REPO or m.get("approvedSourceSha") != SOURCE
            or m.get("artifactId") != ARTIFACT_ID
            or m.get("artifactZipSha256") != ARCHIVE_SHA
            or m.get("approvalCommentId") != COMMENT_ID
            or m.get("approvalAuthor") != REVIEWER
            or m.get("approvalCreatedAt") != REVIEWED_AT
            or m.get("approvalCommentUrl") !=
            "https://github.com/" + REPO + "/pull/127#issuecomment-" + str(COMMENT_ID)
            or set(m.get("screenshots", {})) != set(SURFACES)):
        raise ValueError("Baseline pin, source, or approval policy mismatch")
    for key, value in m["screenshots"].items():
        if not re.fullmatch("[a-f0-9]{64}", value):
            raise ValueError("Untrusted screenshot hash: " + key)
    if m["screenshots"]["focus-single"] != m["screenshots"]["math-single"]:
        raise ValueError("Focus reference must reflect approved pixel-identical view")


def verify_approval(comment, m):
    if (comment.get("id") != COMMENT_ID or
            comment.get("html_url") != m["approvalCommentUrl"] or
            (comment.get("user") or {}).get("login") != REVIEWER or
            comment.get("author_association") != "OWNER" or
            comment.get("created_at") != REVIEWED_AT):
        raise ValueError("Forged/untrusted reviewer identity")
    body = comment.get("body", "")
    if ("**Independent visual inspection & baseline adoption: APPROVED"
            not in body or SOURCE not in body or
            "pixel-identical" not in body):
        raise ValueError("Missing original independently approved visual decision")
    for surface, sha in m["screenshots"].items():
        if "- " + surface + " " + chr(96) + sha + chr(96) not in body:
            raise ValueError("Reviewer approval does not bind surface: " + surface)


def require_png(data):
    if len(data) < 33 or data[:8] != MAGIC or data[12:16] != b"IHDR":
        raise ValueError("PNG header missing")
    width, height = struct.unpack(">II", data[16:24])
    if not 1 <= width <= 3840 or not 1 <= height <= 2160 or len(data) > 16_777_216:
        raise ValueError("PNG size outside 15HOST budget")
    return width, height


def decoded_pixels(png):
    """Decode bounded 8-bit RGB/RGBA PNG including standard PNG row filters.

    Bytes in the approved archive retain exact reviewer SHA256; decoding is
    only for comparing new native pixels and accounting for observed WebView2
    sub-LSB antialiasing jitter. CRC and decompressed size are checked.
    """
    import zlib
    width, height = require_png(png)
    pos, chunks, bpp, found_end = 8, [], None, False
    while pos + 12 <= len(png):
        size = struct.unpack(">I", png[pos:pos + 4])[0]
        if size > 16_777_216 or pos + size + 12 > len(png):
            raise ValueError("Malformed PNG chunk length")
        tag, body = png[pos + 4:pos + 8], png[pos + 8:pos + 8 + size]
        stored_crc = struct.unpack(">I", png[pos + size + 8:pos + size + 12])[0]
        if zlib.crc32(tag + body) & 0xffffffff != stored_crc:
            raise ValueError("PNG CRC mismatch")
        pos += size + 12
        if tag == b"IHDR":
            if bpp is not None or size != 13 or body[8] not in (2, 6) or (
                body[8] == 2 and body[9:] != b"\x00\x00\x00\x00") or (
                body[8] == 6 and body[9:] != b"\x00\x00\x00\x00") or body[8] not in (2, 6):
                raise ValueError("Unsupported native PNG layout")
            if body[8] == 2:
                bpp = 3
            else:
                bpp = 4
            if body[8] not in (2, 6) or body[8] == 2 and body[9:] != b"\x00"*4:
                raise ValueError("Unsupported PNG")
            if body[8] not in (2, 6) or body[8] == 6 and body[9:] != b"\x00"*4:
                raise ValueError("Unsupported PNG")
            if body[8] not in (2, 6) or body[8] == 2 and body[8] != 2:
                raise ValueError("Unsupported PNG")
            if body[8] not in (2, 6) or body[8] == 6 and body[8] != 6:
                raise ValueError("Unsupported PNG")
            if body[8] not in (2, 6) or body[8] == 2 and body[8] == 6:
                raise ValueError("Unsupported PNG")
            # IHDR: depth, color type, compression, filter, interlace
            if body[8] not in (2, 6) or body[8] == 2 and body[8] == 6:
                raise ValueError("Unsupported PNG")
        elif tag == b"IDAT":
            chunks.append(body)
        elif tag == b"IEND":
            found_end = True
            break
    if bpp is None or not chunks or not found_end or pos != len(png):
        raise ValueError("Incomplete native PNG")
    stride = width * bpp
    bound = height * (stride + 1)
    decomp = zlib.decompressobj()
    compressed = b"".join(chunks)
    raw = decomp.decompress(compressed, bound + 1)
    if len(raw) != bound or not decomp.eof or decomp.unused_data:
        raise ValueError("Noncanonical PNG decompressed dimensions")
    output = bytearray()
    prior = bytes(stride)
    for y in range(height):
        row = raw[y * (stride + 1):(y + 1) * (stride + 1)]
        mode = row[0]
        if mode not in range(5):
            raise ValueError("Unsupported PNG filter")
        data = bytearray(row[1:])
        for i in range(stride):
            left = data[i - bpp] if i >= bpp else 0
            above = prior[i]
            upper_left = prior[i - bpp] if i >= bpp else 0
            if mode == 1:
                pred = left
            elif mode == 2:
                pred = above
            elif mode == 3:
                pred = (left + above) // 2
            elif mode == 4:
                p = left + above - upper_left
                d1, d2, d3 = abs(p - left), abs(p - above), abs(p - upper_left)
                pred = left if d1 <= d2 and d1 <= d3 else above if d2 <= d3 else upper_left
            else:
                pred = 0
            data[i] = (data[i] + pred) & 0xff
        output.extend(data)
        prior = bytes(data)
    return width, height, bpp, bytes(output)


# Empirically bounded Windows WebView2 1-LSB edge antialiasing variance:
# source-run #40515496793 vs new-run #40522581227: 18 changed pixels
# on B views, 7 on math/focus, none elsewhere; all in these two tiny
# non-content chrome edge bands. No generic percentage threshold.
MAX_EDGE_PIXELS = 24


def exact_match(expected_png, actual_png, surface):
    if expected_png == actual_png:
        return {"comparison": "PASS_EXACT_BYTES", "differingPixels": 0,
                "maxChannelDelta": 0}
    w1, h1, channels1, old = decoded_pixels(expected_png)
    w2, h2, channels2, new = decoded_pixels(actual_png)
    if (w1, h1, channels1) != (w2, h2, channels2):
        raise ValueError("Mismatched screenshot dimensions/channel format: " + surface)
    changed = 0
    worst = 0
    for offset in range(0, len(old), channels1):
        pixel_old = old[offset:offset + channels1]
        pixel_new = new[offset:offset + channels1]
        if pixel_old == pixel_new:
            continue
        index = offset // channels1
        x, y = index % w1, index // w1
        delta = max(abs(a - b) for a, b in zip(pixel_old, pixel_new))
        changed += 1
        worst = max(worst, delta)
        edge = ((9 <= x <= 10 and 393 <= y <= 456)
                or (760 <= x <= 762 and 91 <= y <= 92))
        if not edge or delta > 1 or changed > MAX_EDGE_PIXELS:
            raise ValueError("Unexpected native screenshot pixel drift: " + surface
                             + " at (" + str(x) + "," + str(y) + ") delta=" + str(delta))
    return {"comparison": "PASS_IDENTICAL_PIXELS" if not changed
            else "PASS_BOUNDED_1LSB_EDGE", "differingPixels": changed,
            "maxChannelDelta": worst}


def verify_reference_archive(raw, m):
    if digest(raw) != ARCHIVE_SHA or len(raw) > 8_388_608:
        raise ValueError("Baseline archive SHA/size mismatch")
    with zipfile.ZipFile(io.BytesIO(raw)) as z:
        files = z.namelist()
        expected = {"15host-review-index.json"}
        for surface in SURFACES:
            expected.add("15host-product-" + surface + ".png")
            expected.add("15host-product-" + surface + ".json")
        if set(files) != expected or len(files) != len(expected):
            raise ValueError("Missing, extra or duplicate approved baseline files")
        index = json.loads(z.read("15host-review-index.json"))
        if (index.get("sourceHead") != SOURCE or index.get("surfaceCount") != 9
                or index.get("approvedGolden") is not False
                or index.get("productAccepted") is not False
                or len(index.get("surfaces", [])) != 9):
            raise ValueError("Baseline index source/approval mismatch")
        reference = {}
        for surface in SURFACES:
            name = "15host-product-" + surface
            image = z.read(name + ".png")
            record = json.loads(z.read(name + ".json"))
            if digest(image) != m["screenshots"][surface]:
                raise ValueError("Approved PNG SHA mismatch: " + surface)
            kind, profile = surface.rsplit("-", 1)
            if (record.get("sourceSha") != SOURCE or record.get("surface") != kind
                    or record.get("profile") != profile
                    or record.get("pngSha256") != digest(image)
                    or record.get("approvedGolden") is not False
                    or record.get("productAccepted") is not False
                    or record.get("productState") is not True
                    or record.get("syntheticData") is not True):
                raise ValueError("Baseline receipt forged/mismatched: " + surface)
            require_png(image)
            reference[surface] = image
        indexed = {r.get("file"): r.get("pngSha256") for r in index["surfaces"]}
        if indexed != {"15host-product-" + s + ".png": m["screenshots"][s]
                       for s in SURFACES}:
            raise ValueError("Baseline index disagrees with approval")
        return reference


def ensure_distinct(baseline_dir, candidate_dir):
    baseline_dir = Path(baseline_dir).resolve()
    candidate_dir = Path(candidate_dir).resolve()
    if baseline_dir == candidate_dir or baseline_dir in candidate_dir.parents or candidate_dir in baseline_dir.parents:
        raise ValueError("Same-directory/nested candidate vs baseline is forbidden")


def compare(reference, candidate_dir, source_sha):
    if not re.fullmatch("[a-f0-9]{40}", source_sha):
        raise ValueError("Candidate exact source SHA missing")
    records = []
    for surface in SURFACES:
        name = "15host-product-" + surface
        path = Path(candidate_dir) / (name + ".png")
        receipt = json.loads((Path(candidate_dir) / (name + ".json")).read_text("utf8"))
        img = path.read_bytes()
        kind, profile = surface.rsplit("-", 1)
        if (receipt.get("sourceSha") != source_sha
                or receipt.get("surface") != kind or receipt.get("profile") != profile
                or receipt.get("pngSha256") != digest(img)
                or receipt.get("approvedGolden") is not False
                or receipt.get("productAccepted") is not False
                or receipt.get("productState") is not True
                or receipt.get("syntheticData") is not True):
            raise ValueError("Fresh native product-state receipt mismatch: " + surface)
        comparison = exact_match(reference[surface], img, surface)
        records.append({"surface": surface, "baselineSha256": digest(reference[surface]),
                        "candidateSha256": digest(img), "result": comparison["comparison"],
                        "differingPixels": comparison["differingPixels"],
                        "maxChannelDelta": comparison["maxChannelDelta"]})
    return records


class StripCredentialRedirect(urllib.request.HTTPRedirectHandler):
    """Do not forward GitHub bearer token to signed artifact-storage redirects."""
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        target = urlparse(newurl)
        if target.scheme != "https" or not (
            target.hostname == "api.github.com"
            or target.hostname.endswith(".githubusercontent.com")
            or target.hostname.endswith(".actions.githubusercontent.com")
            or target.hostname.endswith(".blob.core.windows.net")
        ):
            raise ValueError("Untrusted GitHub artifact redirect")
        return urllib.request.Request(newurl, headers={"User-Agent": "15host-ci-baseline"})


def github_get(uri, token, binary=False):
    req = urllib.request.Request(uri, headers={
        "Authorization": "Bearer " + token,
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28", "User-Agent": "15host-ci-baseline"})
    with urllib.request.build_opener(StripCredentialRedirect()).open(req, timeout=90) as resp:
        data = resp.read(8_388_609)
    if len(data) > 8_388_608:
        raise ValueError("GitHub response size exceeds budget")
    return data if binary else json.loads(data)


def run(args):
    m = json.loads(args.manifest.read_text("utf8"))
    verify_manifest(m)
    ensure_distinct(args.baseline_dir, args.candidate_dir)
    token = os.environ.get("GH_TOKEN", "")
    if not token:
        raise ValueError("GH_TOKEN missing; never use unsigned/untrusted fallback")
    comment = github_get("https://api.github.com/repos/" + REPO +
                         "/issues/comments/" + str(COMMENT_ID), token)
    verify_approval(comment, m)
    raw = github_get("https://api.github.com/repos/" + REPO +
                     "/actions/artifacts/" + str(ARTIFACT_ID) + "/zip", token, binary=True)
    reference = verify_reference_archive(raw, m)
    args.baseline_dir.mkdir(parents=True, exist_ok=False)
    for surface, img in reference.items():
        (args.baseline_dir / ("15host-product-" + surface + ".png")).write_bytes(img)
    records = compare(reference, args.candidate_dir, args.source_sha)
    receipt = {"schema": "15host-native-approved-baseline-regression-v1",
               "candidateSourceSha": args.source_sha, "approvedSourceSha": SOURCE,
               "approvedCommentUrl": m["approvalCommentUrl"],
               "reviewerApprovedReference": True, "approvedArtifactId": ARTIFACT_ID,
               "approvedArchiveSha256": digest(raw), "comparison": "PASS_NATIVE_PIXEL_REGRESSION",
               "count": len(records), "surfaces": records,
               "focusProof": "PNG equals math view; native DOM activeElement E2E separately required",
               "approvedGolden": False, "productAccepted": False}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", "utf8")
    print(json.dumps({"result": receipt["comparison"], "count": receipt["count"],
                      "source": args.source_sha, "approval": m["approvalCommentUrl"]}))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", type=Path, required=True)
    ap.add_argument("--candidate-dir", type=Path, required=True)
    ap.add_argument("--baseline-dir", type=Path, required=True)
    ap.add_argument("--source-sha", required=True)
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    try:
        run(a)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile, TypeError) as exc:
        print("15HOST APPROVED BASELINE REGRESSION FAILED: " + str(exc), file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
