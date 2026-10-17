"""Download and hash the public installer for static package inspection; never execute it."""
import hashlib
import json
import time
import urllib.request
from pathlib import Path

OUT = Path(__file__).resolve().parent
release = json.loads((OUT / "github-latest-release.json").read_text(encoding="utf-8-sig"))
asset = next(x for x in release["assets"] if x["name"] == "Chat-On-Steroids-Setup-x64.exe")
directory = OUT / "release-artifact"
directory.mkdir(exist_ok=True)
target = directory / asset["name"]
expected = asset["digest"].removeprefix("sha256:")

def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for data in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(data)
    return h.hexdigest()

if not target.is_file() or target.stat().st_size != asset["size"] or digest(target) != expected:
    partial = target.with_suffix(".exe.partial")
    request = urllib.request.Request(asset["browser_download_url"], headers={"User-Agent": "CoS-Rust-read-only-audit"})
    start = time.monotonic()
    with urllib.request.urlopen(request, timeout=90) as response, partial.open("wb") as stream:
        while data := response.read(1024 * 1024):
            stream.write(data)
            if stream.tell() > asset["size"]:
                raise ValueError("Download exceeds published asset size")
    actual = digest(partial)
    if partial.stat().st_size != asset["size"] or actual != expected:
        raise ValueError("Installer does not match published size and SHA256")
    partial.replace(target)
    print(f"Downloaded verified asset in {time.monotonic() - start:.1f}s", flush=True)
record = {"tag": release["tag_name"], "url": asset["browser_download_url"], "file": str(target),
          "bytes": target.stat().st_size, "sha256": digest(target), "published_digest": asset["digest"],
          "verification": "PASS: exact published size and SHA256", "executed_or_installed": False}
(OUT / "release-artifact-verification.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
print(json.dumps(record, indent=2), flush=True)
