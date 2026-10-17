"""Measure the qualified headless runtime, separately from the future desktop app."""
import hashlib
import json
import platform
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / "target/release/cos-rust.exe"
OUT = ROOT / "audit/2026-10-16"
PROFILE = OUT / "phase1-3-runtime/release-smoke"


def call(*args):
    start = time.perf_counter()
    result = subprocess.run([str(EXE), "--data-dir", str(PROFILE), *args], capture_output=True, check=True)
    return json.loads(result.stdout), (time.perf_counter() - start) * 1000


info, first_ms = call("info")
smoke, _ = call("self-test")
history, _ = call("history", smoke["sessionId"], "--limit", "10")
search, _ = call("search", "public baseline")
assert info["bridgeProtocol"] == 14 and info["fullApplicationReady"] is False
assert len(history["events"]) == 4 and any(row.get("final") is True for row in history["events"])
assert search["results"]
timings = [call("info")[1] for _ in range(10)]
receipt = {
    "scope": "Windows x64 headless phase 1-3 runtime; complete desktop size/startup/RAM parity NOT RUN",
    "os": platform.platform(),
    "upstreamCommit": info["upstreamCommit"],
    "binary": str(EXE),
    "bytes": EXE.stat().st_size,
    "mib": round(EXE.stat().st_size / 1048576, 3),
    "sha256": hashlib.file_digest(EXE.open("rb"), "sha256").hexdigest(),
    "bridgeProtocol": info["bridgeProtocol"],
    "fullApplicationReady": info["fullApplicationReady"],
    "smoke": {"sessionId": smoke["sessionId"], "historyRows": len(history["events"]), "searchHits": len(search["results"])},
    "startup": {"scope": "New CLI process, warm filesystem, same synthetic profile; subprocess spawn+exit included", "firstMs": first_ms, "samplesMs": timings, "medianMs": statistics.median(timings), "maximumMs": max(timings)},
    "originalDesktopComparison": "NOT RUN",
    "peakMemory": "NOT RUN",
}
(OUT / "phase1-3-release-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
print(json.dumps({key: receipt[key] for key in ("bytes", "mib", "sha256", "smoke")}, indent=2))
