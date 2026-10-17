"""Verify local receipts without promoting them to complete phase acceptance."""
import base64
import hashlib
import json
import re
import subprocess
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
AUDIT = ROOT / "audit/2026-10-16"
UPSTREAM = ROOT / "upstream/chat-on-steroids"
COMMIT = "024731d8083d92c46e8248d64e7e0b36ab0a470f"


def sha(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=UPSTREAM, text=True).strip() == COMMIT
assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=UPSTREAM, text=True).strip()
for name, hashes_key in [("baseline-manifest.json", "sourceHashes"), ("history-manifest.json", "sources")]:
    manifest = read(ROOT / "fixtures/upstream" / name)
    for relative, expected in manifest[hashes_key].items():
        assert sha(UPSTREAM / relative) == expected, relative

case = read(ROOT / "fixtures/upstream/history-case.json")
for relative, content in case["files"].items():
    path = PurePosixPath(relative)
    assert not path.is_absolute() and ".." not in path.parts
    raw = base64.b64decode(content, validate=True)
    if relative.startswith("messages/"):
        event = json.loads(raw)
        key = f"{event['kind']}\0{event['messageId']}"
        assert hashlib.sha256(key.encode()).hexdigest() + ".json" == path.name

log = (AUDIT / "phase1-3-tests-final.log").read_text(encoding="utf-8")
counts = [tuple(map(int, row)) for row in re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", log)]
assert counts and sum(row[0] for row in counts) >= 46
assert all(row[1] == 0 for row in counts)
assert "test result: FAILED" not in log
lint = (AUDIT / "phase1-3-clippy-final.log").read_text(encoding="utf-8")
assert "Finished" in lint and "error:" not in lint and "warning:" not in lint
release = read(AUDIT / "phase1-3-release-receipt.json")
binary = Path(release["binary"])
if not binary.exists() or sha(binary) != release["sha256"]:
    # A later phase may rebuild the same target. Keep the historical receipt immutable.
    binary = AUDIT / "phase1-3-release/cos-rust.exe"
assert sha(binary) == release["sha256"] and binary.stat().st_size == release["bytes"]
assert release["bridgeProtocol"] == 14 and release["fullApplicationReady"] is False
crypto = read(AUDIT / "phase1-3-crypto-receipt.json")
assert crypto["electron"]["rustRead"] and crypto["electron"]["electronRoundTrip"] and crypto["rust"]["electronRead"]
assert "phase1-3-runtime" in crypto["electron"]["userData"]
assert crypto["electron"]["userData"] == crypto["electron"]["sessionData"]

receipt = {
    "upstreamCommit": COMMIT,
    "upstreamTrackedAndUntrackedClean": True,
    "sourceOracleHashes": "PASS",
    "historyShardIdentity": "PASS",
    "rustTests": {"passed": sum(row[0] for row in counts), "failed": 0, "ignored": sum(row[2] for row in counts)},
    "strictClippy": "PASS",
    "releaseBinaryReceipt": "PASS",
    "windowsSecretsInterop": "PASS",
    "wholePhaseAcceptance": {"1": "PARTIAL", "2": "PARTIAL", "3": "PARTIAL"},
    "unqualified": ["GUI/browser/JS/Wasm architecture", "macOS/Linux/native credential providers", "complete original owner branches", "signed-in provider and long workflows", "full desktop size/startup/RAM parity"],
}
(AUDIT / "phase1-3-verification.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
print(json.dumps(receipt, indent=2))
