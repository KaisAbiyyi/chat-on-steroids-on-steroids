#!/usr/bin/env python3
"""CL-10: hermetic release-binary integration contract on cloud Windows/Ubuntu.

Runs only against throwaway profiles owned by the GitHub Actions runner.
No real user profiles, external tunnel, credentials, browser, or local workspace.
"""
from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / "target" / "release" / ("cos-rust.exe" if os.name == "nt" else "cos-rust")


def invoke(*args: str, data_dir: Path | None = None, success: bool = True) -> subprocess.CompletedProcess[str]:
    command = [str(EXE)]
    if data_dir is not None:
        command.extend(("--data-dir", str(data_dir)))
    command.extend(args)
    result = subprocess.run(command, capture_output=True, text=True, timeout=35, check=False)
    assert (result.returncode == 0) == success, (
        f"unexpected exit {result.returncode} for {args!r}: {result.stderr[:700]}"
    )
    return result


def response(*args: str, data_dir: Path) -> object:
    return json.loads(invoke(*args, data_dir=data_dir).stdout)


def verify() -> dict[str, object]:
    assert EXE.is_file(), f"release binary missing: {EXE}"
    missing = invoke("info", success=False)
    assert "data-dir" in missing.stderr, "CLI must refuse an implicit personal profile"

    with tempfile.TemporaryDirectory(prefix="cos-cl10-cloud-") as root:
        base = Path(root)
        profile = base / "new-profile"
        info = response("info", data_dir=profile)
        assert isinstance(info, dict)
        assert info["fullApplicationReady"] is False
        assert info["bridgeProtocol"] == 14
        assert Path(info["dataDirectory"]).resolve() == profile.resolve()
        assert (profile / "cos-rust-profile.json").exists()

        # Across independent processes: recorder -> durable session -> history,
        # search, export and usage all reopen the same marked isolated profile.
        probe = response("self-test", data_dir=profile)
        assert isinstance(probe, dict) and probe["publicFixture"] is True
        session = probe["sessionId"]
        assert isinstance(session, str) and session.startswith("probe-")
        history = response("history", session, data_dir=profile)
        assert isinstance(history, dict) and len(history["events"]) >= 4
        events = history["events"]
        assert any(event.get("kind") == "assistant_message" for event in events)
        exported = response("export", session, "--format", "json", data_dir=profile)
        assert session in json.dumps(exported, ensure_ascii=False)
        listing = response("sessions", "list", data_dir=profile)
        assert session in json.dumps(listing, ensure_ascii=False)
        response("search", "Public baseline", data_dir=profile)
        usage = response("usage", data_dir=profile)
        assert isinstance(usage, dict)

        # Explicit tool surfaces, secret-store reporting and the authorization
        # boundary: no model tool execution without a proof/request id.
        tools = response("tools", "list", "core", data_dir=profile)
        assert isinstance(tools, (list, dict)) and bool(tools)
        secrets = response("secrets-status", data_dir=profile)
        assert isinstance(secrets, dict)
        assert secrets["platform"] == ("win32" if os.name == "nt" else "linux")
        assert secrets["qualified"] == (os.name == "nt")
        denied = invoke("tools", "call", "read_file", str(base / "missing.json"),
                        data_dir=profile, success=False)
        assert "request-id" in denied.stderr, "unattributed model calls must fail"

        # Reject a pre-existing non-Rust directory before writing a marker or
        # overwriting the sentinel; keep this profile completely separate.
        unmarked = base / "unmarked"
        unmarked.mkdir()
        sentinel = unmarked / "do-not-touch.txt"
        sentinel.write_bytes(b"source-stays-unchanged")
        refused = invoke("info", data_dir=unmarked, success=False)
        assert "unmarked" in refused.stderr.lower()
        assert sentinel.read_bytes() == b"source-stays-unchanged"
        assert not (unmarked / "cos-rust-profile.json").exists()

        # Real launch/stop of the integrated HTTP + control + kernel service on
        # an ephemeral loopback port; no tunnel, token or production session.
        served = invoke("serve", "--port", "0", "--duration-ms", "200", data_dir=profile)
        ready = [json.loads(line) for line in served.stdout.splitlines()
                 if line.lstrip().startswith("{") and '"event":"ready"' in line]
        assert len(ready) == 1, "one ready event per service process"
        assert isinstance(ready[0]["port"], int) and ready[0]["port"] > 0
        assert ready[0]["fullApplicationReady"] is False
        assert ready[0]["profile"] == str(profile.resolve())

        # A clean post-service restart must retain the earlier durable record.
        again = response("history", session, data_dir=profile)
        assert again["events"] == events
        assert response("info", data_dir=profile)["fullApplicationReady"] is False

    return {
        "scope": "cloud isolated profile, release CLI/process + durable recorder/history, "
                 "model authorization, startup/shutdown",
        "checks": 12,
        "status": "PASS",
        "platform": os.name,
        "limits": "no local Windows native L6, browser, desktop, installer, "
                  "external SSH/tunnel/proxy, private accounts or performance comparison",
    }


if __name__ == "__main__":
    print(json.dumps(verify(), sort_keys=True))
