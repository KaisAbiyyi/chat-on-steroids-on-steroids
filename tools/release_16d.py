#!/usr/bin/env python3
"""16D: source-bound cloud headless CLI launch, memory and bundle-size evidence.

This is NOT desktop start-to-interactive, whole-process-tree RAM or a release
budget. CI measurements on shared runners are descriptive, never frozen limits.
Only fresh synthetic profiles and the accepted 16C unsigned bundle are used.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from zipfile import ZipFile

try:
    from tools import release_16c
except ImportError:
    import release_16c

SCHEMA = "cosos.release.headless-perf.v1"
MAX_STDOUT = 65536
MAX_RUN_SECONDS = 30
REPEATS = 3


class Refused(ValueError):
    pass


def require(value, message):
    if not value:
        raise Refused(message)


def digest(path):
    return release_16c.sha256_file(path)


def artifact(binary, bundle, platform, source):
    require(platform in ("windows-x64", "linux-x64"), "unsupported platform")
    name = "cos-rust.exe" if platform == "windows-x64" else "cos-rust"
    binary, bundle = Path(binary), Path(bundle)
    require(binary.is_file() and not binary.is_symlink() and binary.name == name,
            "missing or unsafe headless runtime")
    require(bundle.is_file() and not bundle.is_symlink(), "missing or unsafe bundle")
    verified = release_16c.verify(bundle, source)
    require(verified["platform"] == platform, "bundle platform mismatch")
    with ZipFile(bundle) as archive:
        index = json.loads(archive.read("bundle-index.json"))
    declared = next((f for f in index["files"] if f["path"] == name), None)
    binary_sha, binary_bytes = digest(binary)
    require(declared is not None and declared["sha256"] == binary_sha
            and declared["bytes"] == binary_bytes, "built executable differs from signed-off bundle index")
    bundle_sha, bundle_bytes = digest(bundle)
    require(bundle_sha == verified["bundle_sha256"], "bundle digest changed during verification")
    return {"binary_sha256": binary_sha, "binary_bytes": binary_bytes,
            "bundle_sha256": bundle_sha, "bundle_bytes": bundle_bytes}


def working_set(process):
    """Root-process peak resident working set. Never interpreted as GUI/tree RSS."""
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes
        size_t = ctypes.c_size_t

        class Counters(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("page_fault_count", wintypes.DWORD),
                        ("peak_ws", size_t), ("ws", size_t),
                        ("quota_peak_paged", size_t), ("quota_paged", size_t),
                        ("quota_peak_nonpaged", size_t), ("quota_nonpaged", size_t),
                        ("pagefile", size_t), ("peak_pagefile", size_t), ("private", size_t)]
        counters = Counters()
        counters.cb = ctypes.sizeof(Counters)
        api = ctypes.WinDLL("psapi", use_last_error=True).GetProcessMemoryInfo
        api.argtypes = [ctypes.c_void_p, ctypes.POINTER(Counters), wintypes.DWORD]
        api.restype = wintypes.BOOL
        if api(ctypes.c_void_p(int(process._handle)), ctypes.byref(counters), counters.cb):
            return int(counters.peak_ws)
        return None
    if sys.platform.startswith("linux"):
        try:
            raw = (Path("/proc") / str(process.pid) / "status").read_text("ascii")
        except OSError:
            return None
        for line in raw.splitlines():
            if line.startswith("VmHWM:"):
                parts = line.split()
                if len(parts) == 3 and parts[2] == "kB":
                    return int(parts[1]) * 1024
    return None


def sample(binary, profile):
    started = time.perf_counter_ns()
    proc = subprocess.Popen([str(Path(binary).resolve()), "--data-dir", str(profile), "info"],
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE)
    peak = None
    samples = 0
    done = threading.Event()

    def observe():
        nonlocal peak, samples
        while not done.is_set():
            amount = working_set(proc)
            if amount is not None and amount > 0:
                peak = max(peak or 0, amount)
                samples += 1
            done.wait(0.005)

    observer = threading.Thread(target=observe, daemon=True)
    observer.start()
    try:
        out, err = proc.communicate(timeout=MAX_RUN_SECONDS)
    except subprocess.TimeoutExpired as exc:
        proc.kill()
        proc.communicate()
        raise Refused("headless CLI launch timed out") from exc
    finally:
        done.set()
        observer.join(timeout=2)
    elapsed = (time.perf_counter_ns() - started) / 1_000_000
    require(proc.returncode == 0, "headless CLI returned nonzero exit")
    require(len(out) <= MAX_STDOUT and len(err) <= MAX_STDOUT,
            "unexpected oversized CLI output")
    try:
        payload = json.loads(out)
    except (UnicodeError, ValueError) as exc:
        raise Refused("headless CLI did not emit JSON") from exc
    require(isinstance(payload, dict), "headless CLI info must return object")
    require(math.isfinite(elapsed) and elapsed > 0, "invalid launch measurement")
    return {"launch_to_cli_json_ms": round(elapsed, 3),
            "root_peak_working_set_bytes": peak,
            "memory_samples": samples}


def distribution(runs, field):
    values = [r[field] for r in runs if r[field] is not None]
    require(bool(values), "no observed " + field)
    ordered = sorted(values)
    index = (len(ordered) - 1) * .95
    low = int(index)
    p95 = ordered[low] + (ordered[min(low + 1, len(ordered) - 1)] - ordered[low]) * (index - low)
    return {"median": round(statistics.median(values), 3),
            "p95": round(p95, 3), "observed_runs": len(values)}


def collect(binary, bundle, platform, source):
    identity = artifact(binary, bundle, platform, source)
    fresh, warm = [], []
    with tempfile.TemporaryDirectory(prefix="cosos-16d-ci-") as base:
        root = Path(base)
        # Each fresh-profile run starts with an empty directory. 'Fresh' is NOT OS cold-boot.
        for i in range(REPEATS):
            profile = root / ("fresh-" + str(i))
            profile.mkdir()
            fresh.append(sample(binary, profile))
        reusable = root / "reused"
        reusable.mkdir()
        sample(binary, reusable)  # primer excluded from warm statistics
        for _ in range(REPEATS):
            warm.append(sample(binary, reusable))
    require(all(r["memory_samples"] > 0 for r in fresh + warm),
            "root RAM measurement unavailable for one or more launches")
    result = {
        "schema": SCHEMA, "status": "PASS", "source_sha": source,
        "platform": platform, **identity,
        "workload": "isolated-headless-info-cli; fresh-profile vs reused-profile",
        "classification": "hosted-cloud-provisional-observation",
        "process_scope": "headless-root-only-no-renderer-or-browser",
        "fresh_profile": {"runs": fresh,
                          "launch_ms": distribution(fresh, "launch_to_cli_json_ms"),
                          "root_peak_ws_bytes": distribution(fresh, "root_peak_working_set_bytes")},
        "reused_profile": {"runs": warm,
                           "launch_ms": distribution(warm, "launch_to_cli_json_ms"),
                           "root_peak_ws_bytes": distribution(warm, "root_peak_working_set_bytes")},
        "bundle_size_gate": "PASS_16C_MAX_ARCHIVE",
        "release_performance_budget": "NOT_FROZEN",
        "native_product_acceptance": "NOT_RUN",
        "physical_windows_acceptance": "NOT_RUN",
        "original_cos_comparison": "NOT_RUN",
    }
    validate(result, binary, bundle, platform, source)
    return result


def validate(receipt, binary, bundle, platform, source):
    require(isinstance(receipt, dict) and receipt.get("schema") == SCHEMA,
            "wrong performance receipt schema")
    require(receipt.get("source_sha") == source and receipt.get("platform") == platform,
            "performance receipt provenance mismatch")
    require(receipt.get("status") == "PASS"
            and receipt.get("classification") == "hosted-cloud-provisional-observation"
            and receipt.get("process_scope") == "headless-root-only-no-renderer-or-browser"
            and receipt.get("release_performance_budget") == "NOT_FROZEN"
            and receipt.get("native_product_acceptance") == "NOT_RUN"
            and receipt.get("physical_windows_acceptance") == "NOT_RUN"
            and receipt.get("original_cos_comparison") == "NOT_RUN"
            and receipt.get("bundle_size_gate") == "PASS_16C_MAX_ARCHIVE",
            "receipt may not assert native, comparison or frozen budget PASS")
    for key, value in artifact(binary, bundle, platform, source).items():
        require(receipt.get(key) == value, "artifact identity or size differs: " + key)
    for name in ("fresh_profile", "reused_profile"):
        group = receipt.get(name)
        require(isinstance(group, dict) and len(group.get("runs", [])) == REPEATS,
                "missing launch samples")
        runs = group["runs"]
        require(all(isinstance(r, dict) and set(r) ==
                    {"launch_to_cli_json_ms", "root_peak_working_set_bytes", "memory_samples"}
                    for r in runs), "unexpected run fields")
        for run in runs:
            require(type(run["launch_to_cli_json_ms"]) in (int, float)
                    and math.isfinite(run["launch_to_cli_json_ms"])
                    and run["launch_to_cli_json_ms"] > 0, "invalid launch value")
            require(type(run["root_peak_working_set_bytes"]) is int
                    and run["root_peak_working_set_bytes"] > 0
                    and type(run["memory_samples"]) is int
                    and run["memory_samples"] > 0, "invalid RAM observation")
        for summary, key in (("launch_ms", "launch_to_cli_json_ms"),
                             ("root_peak_ws_bytes", "root_peak_working_set_bytes")):
            require(group.get(summary) == distribution(runs, key),
                    "inconsistent reported distribution")
    return {"status": "PASS", "source_sha": source, "platform": platform,
            "cloud_scope": "headless-cli-only", "native_product_acceptance": "NOT_RUN"}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    for action in ("measure", "verify"):
        cmd = sub.add_parser(action)
        cmd.add_argument("--binary", required=True)
        cmd.add_argument("--bundle", required=True)
        cmd.add_argument("--platform", choices=("windows-x64", "linux-x64"), required=True)
        cmd.add_argument("--source-sha", required=True)
        if action == "measure":
            cmd.add_argument("--out", required=True)
        else:
            cmd.add_argument("--receipt", required=True)
    args = parser.parse_args(argv)
    try:
        if args.action == "measure":
            receipt = collect(args.binary, args.bundle, args.platform, args.source_sha)
            output = Path(args.out)
            require(output.parent.is_dir() and not output.is_symlink(), "unsafe output destination")
            with output.open("x", encoding="utf-8") as stream:
                json.dump(receipt, stream, indent=2, sort_keys=True)
                stream.write("\n")
            print(json.dumps({"status": "PASS", "scope": "headless-cli-cloud",
                              "report": output.name, "native_product_acceptance": "NOT_RUN"}))
        else:
            path = Path(args.receipt)
            require(path.stat().st_size <= 32768 and not path.is_symlink(),
                    "unsafe oversized performance receipt")
            print(json.dumps(validate(json.loads(path.read_text("utf-8")),
                                      args.binary, args.bundle, args.platform, args.source_sha)))
        return 0
    except (Refused, release_16c.Refused, OSError, ValueError, KeyError,
            subprocess.SubprocessError) as exc:
        print(json.dumps({"status": "BLOCKED", "reason": str(exc)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
