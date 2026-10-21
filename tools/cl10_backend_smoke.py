#!/usr/bin/env python3
"""CL-10: hermetic release-binary integration contract on cloud Windows/Ubuntu.

Runs only against throwaway profiles owned by the GitHub Actions runner.
No real user profiles, external tunnel, credentials, browser, or local workspace.
"""
from __future__ import annotations

import http.client
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

        # Live integrated transport checks: MCP bearer path, hostile Host/Origin,
        # control bearer, disabled writes; then graceful shutdown/restart.
        response("config", "set", "controlApi.enabled", "true", data_dir=profile)
        service = subprocess.Popen(
            [str(EXE), "--data-dir", str(profile), "serve", "--port", "0",
             "--duration-ms", "3500"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        try:
            assert service.stdout is not None
            event = json.loads(service.stdout.readline())
            assert event["event"] == "ready" and event["fullApplicationReady"] is False
            assert event["profile"] == str(profile.resolve())
            port = event["port"]
            control = event["controlPort"]
            assert isinstance(port, int) and port > 0
            assert isinstance(control, int) and control > 0
            from urllib.parse import urlsplit
            path = urlsplit(event["urls"]["core"]).path

            def check_http(port: int, method: str, path: str, headers: dict[str, str],
                           expected: int, body: str | None = None) -> dict:
                client = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
                try:
                    client.request(method, path, body=body, headers=headers)
                    result = client.getresponse()
                    payload = json.loads(result.read())
                    assert result.status == expected, (method, path, result.status, payload)
                    assert result.getheader("cache-control") == "no-store"
                    return payload
                finally:
                    client.close()

            check_http(port, "POST", path, {"Host": "evil.example"}, 403, "{}")
            check_http(port, "POST", path, {"Origin": "https://evil.example"}, 403, "{}")
            check_http(port, "GET", "/mcp/core/not-a-token", {}, 404)
            check_http(control, "GET", "/v1/health", {}, 401)
            bearer = (profile / "control-api" / "token").read_text(encoding="utf-8")
            health = check_http(control, "GET", "/v1/health",
                                {"Authorization": f"Bearer {bearer}"}, 200)
            assert health["ok"] is True
            assert health["actions"]["enabled"] is False
            check_http(control, "POST", "/v1/inputs",
                       {"Authorization": f"Bearer {bearer}",
                        "Content-Type": "application/json"}, 403, "{}")
            service.wait(timeout=20)
            assert service.returncode == 0, "integrated service exited abnormally"
        finally:
            if service.poll() is None:
                service.kill()
                service.wait(timeout=10)
            if service.stdout:
                service.stdout.close()
            if service.stderr:
                service.stderr.close()

        # A clean post-service restart must retain the earlier durable record.
        again = response("history", session, data_dir=profile)
        assert again["events"] == events
        assert response("info", data_dir=profile)["fullApplicationReady"] is False

    return {
        "scope": "cloud isolated profile, release CLI/process + durable recorder/history, "
                 "model authorization, startup/shutdown",
        "checks": 18,
        "status": "PASS",
        "platform": os.name,
        "limits": "no local Windows native L6, browser, desktop, installer, "
                  "external SSH/tunnel/proxy, private accounts or performance comparison",
    }


if __name__ == "__main__":
    print(json.dumps(verify(), sort_keys=True))
