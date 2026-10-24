#!/usr/bin/env python3
"""PERF-00: privacy-minimal, source-bound total-process-tree performance harness.

No CoSoS app integration, provider sign-in, private profile scan or native PASS.
Collection is opt-in against explicit PIDs. JSON contains only numeric process
metrics and caller-supplied non-secret provenance; never command lines/titles.
"""
from __future__ import annotations

import argparse
import re
import json
import math
import os
import platform
import statistics
import subprocess
import sys
import time
from collections import deque
from pathlib import Path

SCHEMA_VERSION = 1
BYTES_PER_MIB = 1024 * 1024
WORKLOADS = {
    "cold_start": {"chats": 0, "browser": "closed", "history": "empty", "activity": "launch", "thermal": "cold"},
    "warm_start": {"chats": 0, "browser": "closed", "history": "empty", "activity": "launch", "thermal": "warm"},
    "idle_zero_chat": {"chats": 0, "browser": "closed", "history": "empty", "activity": "idle", "thermal": "warm"},
    "one_chat": {"chats": 1, "browser": "required", "history": "small", "activity": "conversation", "thermal": "warm"},
    "multi_chat": {"chats": 8, "browser": "required", "history": "small", "activity": "switch", "thermal": "warm"},
    "large_history": {"chats": 8, "browser": "required", "history": "large-synthetic", "activity": "navigate", "thermal": "warm"},
    "coding_terminal": {"chats": 1, "browser": "required", "history": "small", "activity": "tool-edit-build", "thermal": "warm"},
    "external_browser": {"chats": 1, "browser": "external-reused", "history": "small", "activity": "coding", "thermal": "warm"},
    "embedded_browser": {"chats": 1, "browser": "embedded-ondemand", "history": "small", "activity": "coding", "thermal": "warm"},
    "recovery": {"chats": 1, "browser": "required", "history": "small", "activity": "restart-recover", "thermal": "warm"},
}
ALLOWED_ROLES = {"owned", "external"}
TOP_FIELDS = {"schema_version", "provenance", "workload_id", "workload_params", "samples", "events"}
PROVENANCE_FIELDS = {"app", "source_sha", "binary_sha256", "host_id", "os", "power_mode", "profile_class", "build_id", "logical_cpus", "synthetic"}
PROCESS_FIELDS = {"pid", "ppid", "identity", "rss_bytes", "private_bytes", "peak_rss_bytes", "cpu_seconds", "handles"}


class TraceError(ValueError):
    """Invalid provenance, sampling or accounting; refuse misleading output."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise TraceError(message)


def nonnegative(value: object, name: str, *, optional: bool = False) -> int | float | None:
    if value is None and optional:
        return None
    require(isinstance(value, (int, float)) and not isinstance(value, bool)
            and math.isfinite(value) and value >= 0, "invalid " + name)
    return value


def percentile(numbers: list[float], percent: float) -> float | None:
    if not numbers:
        return None
    ordered = sorted(numbers)
    index = (len(ordered) - 1) * percent
    low = int(index)
    upper = min(low + 1, len(ordered) - 1)
    return ordered[low] + (ordered[upper] - ordered[low]) * (index - low)


def _process(p: dict) -> None:
    require(isinstance(p, dict) and not (set(p) - PROCESS_FIELDS), "invalid/unsafe process fields")
    for k in ("pid", "ppid"):
        require(type(p.get(k)) is int and p[k] >= 0, "invalid " + k)
    require(p["pid"] > 0 and isinstance(p.get("identity"), str)
            and 0 < len(p["identity"]) <= 96, "missing stable process identity")
    for key in ("rss_bytes", "private_bytes", "peak_rss_bytes", "cpu_seconds", "handles"):
        nonnegative(p.get(key), key, optional=key != "rss_bytes")
    if p.get("peak_rss_bytes") is not None:
        require(p["peak_rss_bytes"] >= p["rss_bytes"], "peak below current RSS")


def validate_trace(trace: dict) -> None:
    require(isinstance(trace, dict) and trace.get("schema_version") == SCHEMA_VERSION, "unsupported schema")
    require(not (set(trace) - TOP_FIELDS), "unrecognized trace fields; privacy boundary")
    provenance = trace.get("provenance")
    require(isinstance(provenance, dict) and not (set(provenance) - PROVENANCE_FIELDS),
            "unrecognized provenance fields; privacy boundary")
    for k in ("app", "source_sha", "host_id", "os", "power_mode", "profile_class", "build_id"):
        require(isinstance(provenance.get(k), str) and 0 < len(provenance[k]) <= 128,
                "missing/invalid provenance " + k)
        if k != "source_sha":
            require(re.fullmatch(r"[A-Za-z0-9_.-]+", provenance[k]) is not None,
                    "non-anonymous provenance characters in " + k)
    require(provenance["app"] in ("cos-original", "cosos"), "unknown application kind")
    require(provenance["source_sha"] == "SYNTHETIC" or
            (len(provenance["source_sha"]) == 40 and all(ch in "0123456789abcdef"
            for ch in provenance["source_sha"])), "source SHA must be exact or SYNTHETIC")
    require(type(provenance.get("synthetic")) is bool, "synthetic flag required")
    if provenance["synthetic"]:
        require(provenance["source_sha"] == "SYNTHETIC", "synthetic source mismatch")
    else:
        require(provenance["source_sha"] != "SYNTHETIC", "observed trace cannot be synthetic")
        require(isinstance(provenance.get("binary_sha256"), str) and
                len(provenance["binary_sha256"]) == 64 and
                all(c in "0123456789abcdef" for c in provenance["binary_sha256"]),
                "real capture requires binary digest")
    name = trace.get("workload_id")
    require(name in WORKLOADS, "unknown workload")
    require(trace.get("workload_params") == WORKLOADS[name], "workload definition changed")
    cores = provenance.get("logical_cpus")
    require(type(cores) is int and cores > 0, "invalid logical_cpus")
    samples = trace.get("samples")
    require(isinstance(samples, list) and 1 <= len(samples) <= 20000, "missing/excessive samples")
    last_time = -1
    for sample in samples:
        require(isinstance(sample, dict), "invalid sample")
        t = nonnegative(sample.get("elapsed_ms"), "elapsed_ms")
        require(t > last_time, "timestamps must be strictly increasing")
        last_time = t
        nonnegative(sample.get("collector_ms"), "collector_ms")
        processes, roots = sample.get("processes"), sample.get("roots")
        require(isinstance(processes, list) and len(processes) <= 10000, "invalid process count")
        require(not (set(sample) - {"elapsed_ms", "collector_ms", "processes", "roots"}),
                "unrecognized sample fields; privacy boundary")
        require(isinstance(roots, list) and 1 <= len(roots) <= 128, "invalid root count")
        by_pid: dict[int, dict] = {}
        for p in processes:
            _process(p)
            require(p["pid"] not in by_pid, "duplicate process pid")
            by_pid[p["pid"]] = p
        seen_root_ids = set()
        for root in roots:
            require(isinstance(root, dict) and not (set(root) - {"pid", "identity", "role"})
                    and type(root.get("pid")) is int
                    and root["pid"] > 0 and isinstance(root.get("identity"), str)
                    and root.get("role") in ALLOWED_ROLES, "invalid root")
            require(root["pid"] not in seen_root_ids, "duplicate root pid")
            seen_root_ids.add(root["pid"])
    events = trace.get("events", {})
    require(isinstance(events, dict) and not (set(events) - {"startup_ms", "tool_latencies_ms"}),
            "unrecognized events fields; privacy boundary")
    nonnegative(events.get("startup_ms"), "startup_ms", optional=True)
    latencies = events.get("tool_latencies_ms", [])
    require(isinstance(latencies, list) and len(latencies) <= 20000, "invalid latencies")
    for number in latencies:
        nonnegative(number, "tool latency")


def sample_tree(sample: dict) -> dict:
    """Traverse explicit roots only; owner wins if external roots overlap.

    A parent chain is accepted only while the tracked root identities match.
    PID reuse never silently reattaches a different root.
    """
    processes = {p["pid"]: p for p in sample["processes"]}
    children: dict[int, list[int]] = {}
    for p in sample["processes"]:
        children.setdefault(p["ppid"], []).append(p["pid"])
    included: dict[int, str] = {}
    missing = []
    for role in ("owned", "external"):
        for root in sample["roots"]:
            if root["role"] != role:
                continue
            start = processes.get(root["pid"])
            if not start or start["identity"] != root["identity"]:
                missing.append(root["pid"])
                continue
            queue = deque([root["pid"]])
            while queue:
                pid = queue.popleft()
                if pid in included:
                    continue
                included[pid] = role
                queue.extend(children.get(pid, []))
    owned = [processes[pid] for pid, role in included.items() if role == "owned"]
    external = [processes[pid] for pid, role in included.items() if role == "external"]
    included_processes = owned + external
    private_values = [p.get("private_bytes") for p in included_processes]
    return {
        "complete": not missing,
        "missing_root_pids": sorted(set(missing)),
        "owned_process_count": len(owned),
        "external_process_count": len(external),
        "owned_rss_bytes": sum(p["rss_bytes"] for p in owned),
        "external_rss_bytes": sum(p["rss_bytes"] for p in external),
        "total_rss_bytes": sum(p["rss_bytes"] for p in included_processes),
        "total_private_bytes": sum(private_values) if all(v is not None for v in private_values) else None,
        "identity_cpu_seconds": {str(p["pid"]) + ":" + p["identity"]: p.get("cpu_seconds")
                                 for p in included_processes},
        "identity_handles": {str(p["pid"]) + ":" + p["identity"]: p.get("handles")
                             for p in included_processes},
    }


def summarize(trace: dict) -> dict:
    validate_trace(trace)
    samples = []
    previous_cpu = None
    previous_ms = None
    warnings = []
    expected_root_identities = {}
    for point in trace["samples"]:
        entry = sample_tree(point)
        current_by_pid = {p["pid"]: p for p in point["processes"]}
        for root in point["roots"]:
            pid = root["pid"]
            actual = current_by_pid.get(pid)
            if actual is not None and pid not in expected_root_identities:
                expected_root_identities[pid] = actual["identity"]
            if actual is not None and expected_root_identities.get(pid) != actual["identity"]:
                entry["complete"] = False
                entry["missing_root_pids"] = sorted(set(entry["missing_root_pids"] + [pid]))
        entry["elapsed_ms"] = point["elapsed_ms"]
        entry["collector_ms"] = point["collector_ms"]
        entry["cpu_percent_of_machine_lower_bound"] = None
        current_cpu = entry.pop("identity_cpu_seconds")
        entry["handle_count"] = (sum(entry.pop("identity_handles").values())
                                 if all(p.get("handles") is not None for p in point["processes"])
                                 else None)
        if previous_cpu is not None and entry["complete"] and previous_ms is not None:
            elapsed = (point["elapsed_ms"] - previous_ms) / 1000
            if elapsed > 0:
                cpu_delta = sum(max(0, value - previous_cpu[identity])
                                for identity, value in current_cpu.items()
                                if value is not None and previous_cpu.get(identity) is not None)
                entry["cpu_percent_of_machine_lower_bound"] = round(
                    100 * cpu_delta / (elapsed * trace["provenance"]["logical_cpus"]), 3)
        if not entry["complete"]:
            warnings.append("root absent or PID identity changed at elapsed_ms=" + str(point["elapsed_ms"]))
        previous_cpu = current_cpu
        previous_ms = point["elapsed_ms"]
        samples.append(entry)
    complete = [s for s in samples if s["complete"]]
    require(bool(complete), "no complete root samples")
    rss = [s["total_rss_bytes"] for s in complete]
    private = [s["total_private_bytes"] for s in complete]
    cpu = [s["cpu_percent_of_machine_lower_bound"] for s in samples
           if s["cpu_percent_of_machine_lower_bound"] is not None]
    tool = trace.get("events", {}).get("tool_latencies_ms", [])
    if any(s["collector_ms"] >= (samples[i]["elapsed_ms"] - samples[i-1]["elapsed_ms"])
           for i, s in enumerate(samples) if i > 0):
        warnings.append("collector time exceeded sample period; results may be perturbed")
    return {
        "schema_version": SCHEMA_VERSION,
        "classification": "synthetic-fixture" if trace["provenance"]["synthetic"] else "provisional-observation",
        "native_product_acceptance": "NOT_RUN",
        "provenance": trace["provenance"],
        "workload_id": trace["workload_id"],
        "workload_params": trace["workload_params"],
        "sample_count": len(samples),
        "complete_sample_count": len(complete),
        "all_roots_observed": len(complete) == len(samples),
        "whole_tree_peak_rss_bytes": max(rss),
        "whole_tree_median_rss_bytes": statistics.median(rss),
        "whole_tree_final_rss_bytes": rss[-1],
        "whole_tree_median_private_bytes": statistics.median(private) if all(p is not None for p in private) else None,
        "cpu_p50_percent_machine_lower_bound": percentile(cpu, .5),
        "cpu_p95_percent_machine_lower_bound": percentile(cpu, .95),
        "startup_ms": trace.get("events", {}).get("startup_ms"),
        "tool_latency_p50_ms": percentile(tool, .5),
        "tool_latency_p95_ms": percentile(tool, .95),
        "sample_series": [{k: v for k, v in sample.items()
                           if k not in ("handle_count",)} for sample in samples],
        "warnings": warnings,
    }


def compare(baseline: dict, candidate: dict) -> dict:
    """Refuse comparability claims for synthetic/mismatched/incomplete sources."""
    required = ("host_id", "os", "power_mode", "profile_class", "logical_cpus")
    a, b = baseline["provenance"], candidate["provenance"]
    require(a["app"] == "cos-original" and b["app"] == "cosos", "baseline must be original CoS")
    require(not a["synthetic"] and not b["synthetic"], "synthetic data cannot assert savings")
    require(baseline["classification"] == candidate["classification"] == "provisional-observation",
            "comparison requires observed traces")
    require(baseline["workload_id"] == candidate["workload_id"]
            and baseline["workload_params"] == candidate["workload_params"], "workload mismatch")
    require(all(a[k] == b[k] for k in required), "machine/profile/power mismatch")
    require(baseline["all_roots_observed"] and candidate["all_roots_observed"],
            "incomplete process trees")
    require(baseline["sample_count"] >= 2 and candidate["sample_count"] >= 2,
            "comparison requires multiple samples")
    old, new = baseline["whole_tree_median_rss_bytes"], candidate["whole_tree_median_rss_bytes"]
    require(old > 0, "baseline RSS is zero")
    return {
        "classification": "provisional-matched-observation-only",
        "native_product_acceptance": "NOT_RUN",
        "metric": "median_total_process_tree_rss_bytes",
        "original_bytes": old,
        "cosos_bytes": new,
        "reduction_percent": round(100 * (old - new) / old, 2),
        "qualification": "Requires independent provenance, attribution and physical Windows review",
    }


def _linux_rows() -> list[dict]:
    ticks = os.sysconf("SC_CLK_TCK")
    rows = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text(encoding="ascii")
            fields = stat[stat.rfind(")") + 2:].split()
            pid = int(entry.name)
            ppid, identity = int(fields[1]), fields[19]
            cpu = (int(fields[11]) + int(fields[12])) / ticks
            status = (entry / "status").read_text(encoding="ascii")
            data = {}
            for line in status.splitlines():
                if line.startswith(("VmRSS:", "VmHWM:")):
                    name, value, *_ = line.split()
                    data[name[:-1]] = int(value) * 1024
            if "VmRSS" not in data:
                continue
            rows.append({"pid": pid, "ppid": ppid, "identity": identity,
                         "rss_bytes": data["VmRSS"],
                         "private_bytes": None,  # Linux RSS is not Windows private bytes.
                         "peak_rss_bytes": max(data.get("VmHWM", data["VmRSS"]), data["VmRSS"]),
                         "cpu_seconds": cpu, "handles": None})
        except (OSError, ValueError, IndexError):
            continue  # Process exited or is inaccessible; roots must still pass completeness.
    return rows


_WINDOWS_SCRIPT = r"""
$ErrorActionPreference = 'Stop'
$parents = @{}
Get-CimInstance Win32_Process | ForEach-Object {
  $parents[[int]$_.ProcessId] = @{
    ppid=[int]$_.ParentProcessId
    identity=[string]$_.CreationDate
  }
}
$rows = @(
  Get-Process | ForEach-Object {
    $idNumber = [int]$_.Id
    if ($parents.ContainsKey($idNumber)) {
      try {
        @{
          pid=$idNumber
          ppid=$parents[$idNumber].ppid
          identity=$parents[$idNumber].identity
          rss_bytes=[long]$_.WorkingSet64
          private_bytes=[long]$_.PrivateMemorySize64
          peak_rss_bytes=[long]$_.PeakWorkingSet64
          cpu_seconds=$(if ($null -ne $_.CPU) { [double]$_.CPU } else { $null })
          handles=[long]$_.HandleCount
        }
      } catch { }
    }
  }
)
ConvertTo-Json -InputObject @($rows) -Depth 3 -Compress
"""


def _windows_rows() -> list[dict]:
    # Expensive and version-sensitive; collector_ms makes overhead visible.
    result = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive",
                             "-Command", _WINDOWS_SCRIPT], check=True,
                            capture_output=True, text=True, timeout=60)
    rows = json.loads(result.stdout)
    require(isinstance(rows, list), "Windows collector did not return an array")
    return rows


def collect_sample(root_pids: list[int], external_pids: list[int], elapsed_ms: float) -> dict:
    before = time.perf_counter()
    if sys.platform == "win32":
        processes = _windows_rows()
    elif sys.platform.startswith("linux"):
        processes = _linux_rows()
    else:
        raise TraceError("live collection only supported on Windows/Linux")
    by_pid = {p["pid"]: p for p in processes}
    children = {}
    for p in processes:
        children.setdefault(p["ppid"], []).append(p["pid"])
    retained = set()
    for root in root_pids + external_pids:
        todo = [root]
        while todo:
            pid = todo.pop()
            if pid in retained:
                continue
            retained.add(pid)
            todo.extend(children.get(pid, []))
    # Roots absent on this tick keep their identity marker, so aggregates fail closed.
    roots = []
    for role, pids in (("owned", root_pids), ("external", external_pids)):
        for pid in pids:
            p = by_pid.get(pid)
            roots.append({"pid": pid, "identity": p["identity"] if p else "ABSENT",
                          "role": role})
    return {"elapsed_ms": round(elapsed_ms, 3),
            "collector_ms": round((time.perf_counter() - before) * 1000, 3),
            "roots": roots, "processes": [p for p in processes if p["pid"] in retained]}


def capture(args: argparse.Namespace) -> dict:
    require(args.duration > 0 and args.duration <= 14400 and args.interval >= .2
            and args.interval <= 300, "unsafe duration/interval")
    require(len(set(args.root_pid + args.external_root_pid)) ==
            len(args.root_pid + args.external_root_pid), "overlapping or duplicate roots")
    require(all(pid > 0 for pid in args.root_pid + args.external_root_pid), "invalid PID")
    require(len(args.root_pid) > 0 and len(args.root_pid + args.external_root_pid) <= 128, "root count")
    require(len(args.source_sha) == 40 and all(c in "0123456789abcdef" for c in args.source_sha),
            "observed source SHA required")
    require(len(args.binary_sha256) == 64 and all(c in "0123456789abcdef" for c in args.binary_sha256),
            "observed binary digest required")
    provenance = {
        "app": args.app, "source_sha": args.source_sha, "binary_sha256": args.binary_sha256,
        "build_id": args.build_id, "host_id": args.host_id, "os": platform.system(),
        "power_mode": args.power_mode, "profile_class": args.profile_class,
        "logical_cpus": os.cpu_count() or 1, "synthetic": False,
    }
    samples = []
    started = time.perf_counter()
    step = 0
    while True:
        elapsed = (time.perf_counter() - started) * 1000
        samples.append(collect_sample(args.root_pid, args.external_root_pid, elapsed))
        step += 1
        deadline = started + step * args.interval
        if deadline - started > args.duration:
            break
        time.sleep(max(0, deadline - time.perf_counter()))
    return {"schema_version": SCHEMA_VERSION, "provenance": provenance,
            "workload_id": args.workload, "workload_params": WORKLOADS[args.workload],
            "samples": samples, "events": {}}


def write_json(path: str, data: dict) -> None:
    destination = Path(path)
    require(not destination.is_symlink(), "refuse symlink output")
    require(destination.parent.exists(), "output directory must already exist")
    destination.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def read_json(path: str) -> dict:
    target = Path(path)
    require(target.stat().st_size <= 12 * 1024 * 1024, "trace exceeds 12MiB cap")
    return json.loads(target.read_text(encoding="utf-8"))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    cmd = parser.add_subparsers(dest="action", required=True)
    cmd.add_parser("matrix")
    capture_cmd = cmd.add_parser("capture", help="Opt-in numerical PID capture; never runs CoS or scans profiles")
    capture_cmd.add_argument("--app", choices=("cos-original", "cosos"), required=True)
    capture_cmd.add_argument("--workload", choices=tuple(WORKLOADS), required=True)
    capture_cmd.add_argument("--root-pid", type=int, action="append", required=True)
    capture_cmd.add_argument("--external-root-pid", type=int, action="append", default=[])
    capture_cmd.add_argument("--duration", type=float, default=30)
    capture_cmd.add_argument("--interval", type=float, default=1)
    capture_cmd.add_argument("--source-sha", required=True)
    capture_cmd.add_argument("--binary-sha256", required=True)
    capture_cmd.add_argument("--build-id", required=True)
    capture_cmd.add_argument("--host-id", required=True, help="Pseudonym, not machine name")
    capture_cmd.add_argument("--power-mode", required=True)
    capture_cmd.add_argument("--profile-class", required=True, help="Synthetic/restricted/consented; not a path")
    capture_cmd.add_argument("--output", required=True)
    summ = cmd.add_parser("summarize")
    summ.add_argument("--input", required=True)
    summ.add_argument("--output")
    comp = cmd.add_parser("compare")
    comp.add_argument("--baseline", required=True)
    comp.add_argument("--candidate", required=True)
    comp.add_argument("--output")
    args = parser.parse_args(argv)
    try:
        if args.action == "matrix":
            print(json.dumps(WORKLOADS, indent=2, sort_keys=True))
            return 0
        if args.action == "capture":
            result = capture(args)
            validate_trace(result)
            write_json(args.output, result)
            print(json.dumps({"samples": len(result["samples"]), "output": args.output,
                              "native_product_acceptance": "NOT_RUN"}))
            return 0
        if args.action == "summarize":
            result = summarize(read_json(args.input))
        else:
            result = compare(summarize(read_json(args.baseline)),
                             summarize(read_json(args.candidate)))
        if args.output:
            write_json(args.output, result)
        else:
            print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (TraceError, OSError, json.JSONDecodeError, subprocess.SubprocessError) as exc:
        print("PERF-00 validation error: " + str(exc), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
