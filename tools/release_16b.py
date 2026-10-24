#!/usr/bin/env python3
"""CoSoS 16B: inspect a Windows release candidate; never install or update it.

A successful result proves manifest integrity and Windows Authenticode trust
for the staged bytes ONLY. It is not a native install, migration or rollout.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import subprocess
import sys

SCHEMA = "cosos.release.v1"
SHA256 = re.compile(r"[0-9a-f]{64}")
SHA40 = re.compile(r"[0-9a-f]{40}")
THUMBPRINT = re.compile(r"[0-9A-F]{40,64}")
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")
MAX_FILE_BYTES = 512 * 1024 * 1024
MAX_TOTAL_BYTES = 1024 * 1024 * 1024
MAX_FILES = 256


class Refused(ValueError):
    pass


def version(s):
    if not isinstance(s, str) or VERSION.fullmatch(s) is None:
        raise Refused("version must be canonical numeric major.minor.patch")
    return tuple(int(p) for p in s.split("."))


def path_is_safe(root, path):
    if root.is_symlink() or any(parent.is_symlink() for parent in root.parents):
        raise Refused("linked staging directory refused")
    relative = PurePosixPath(path)
    if (
        not path or "\\" in path or ":" in path or path.startswith("/")
        or any(p in (".", "..", "") for p in path.split("/"))
        or relative.as_posix() != path
    ):
        raise Refused("unsafe relative path")
    result = root.joinpath(*relative.parts)
    if result.is_symlink() or any(parent.is_symlink() for parent in result.parents if parent != root):
        raise Refused("linked artifact path refused")
    if not result.is_file():
        raise Refused("missing staged file")
    return result


def digest_file(path):
    total = 0
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            total += len(chunk)
            if total > MAX_FILE_BYTES:
                raise Refused("oversize staged artifact")
            h.update(chunk)
    return h.hexdigest(), total


def windows_signature(path):
    """Windows-owned Authenticode verification: never trusts a JSON claim."""
    if platform.system() != "Windows":
        raise Refused("Windows Authenticode host required")
    # EncodedCommand avoids shell interpolation of untrusted artifact paths.
    quoted = str(path).replace("'", "''")
    script = (
        "$ErrorActionPreference='Stop';"
        "$s=Get-AuthenticodeSignature -LiteralPath '" + quoted + "';"
        "if($s.Status -ne 'Valid' -or $null -eq $s.SignerCertificate){exit 3};"
        "[Console]::Out.Write($s.SignerCertificate.Thumbprint)"
    )
    encoded = __import__("base64").b64encode(script.encode("utf-16le")).decode("ascii")
    try:
        proc = subprocess.run(
            ["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded],
            capture_output=True, text=True, check=False, timeout=30
        )
    except (OSError, subprocess.TimeoutExpired) as ex:
        raise Refused("native signer verifier unavailable") from ex
    if proc.returncode != 0:
        raise Refused("Authenticode signature not valid")
    return proc.stdout.strip().upper()


def inspect(root, expected_source, current_version, trusted_thumbprint, signature_check=None):
    """Fail closed before any publication. signature_check injectable only for tests."""
    root = Path(root).absolute()
    if not root.is_dir() or root.is_symlink():
        raise Refused("missing or linked staging directory")
    thumb = str(trusted_thumbprint).upper()
    if not THUMBPRINT.fullmatch(thumb):
        raise Refused("trusted publisher thumbprint must be preconfigured")
    if not isinstance(expected_source, str) or not SHA40.fullmatch(expected_source):
        raise Refused("expected source commit SHA invalid")
    manifest_path = root / "release.json"
    if manifest_path.is_symlink() or not manifest_path.is_file():
        raise Refused("missing regular release manifest")
    if manifest_path.stat().st_size > 64 * 1024:
        raise Refused("manifest exceeds bounded size")
    try:
        manifest = json.loads(manifest_path.read_text("utf-8"))
    except (UnicodeError, OSError, json.JSONDecodeError) as ex:
        raise Refused("malformed release manifest") from ex
    if not isinstance(manifest, dict) or set(manifest) != {
        "schema", "product", "platform", "version", "source_sha", "files"
    }:
        raise Refused("release manifest schema mismatch")
    if (manifest["schema"] != SCHEMA or manifest["product"] != "CoSoS"
            or manifest["platform"] != "windows-x64"
            or manifest["source_sha"] != expected_source):
        raise Refused("wrong product/platform/source")
    if version(manifest["version"]) <= version(current_version):
        raise Refused("update must increase the existing version (no downgrade/reinstall)")
    files = manifest["files"]
    if not isinstance(files, list) or not 1 <= len(files) <= MAX_FILES:
        raise Refused("manifest must declare a bounded nonempty file list")
    seen, executables, total = set(), [], 0
    for item in files:
        if not isinstance(item, dict) or set(item) != {"path", "sha256", "bytes"}:
            raise Refused("unexpected manifest file fields")
        name, sha, size = item["path"], item["sha256"], item["bytes"]
        if not isinstance(name, str) or name in seen or name == "release.json":
            raise Refused("duplicate or invalid manifest path")
        if not isinstance(sha, str) or SHA256.fullmatch(sha) is None:
            raise Refused("invalid SHA256 manifest digest")
        if type(size) is not int or size < 0 or size > MAX_FILE_BYTES:
            raise Refused("invalid file size")
        target = path_is_safe(root, name)
        actual_sha, actual_size = digest_file(target)
        if actual_sha != sha or actual_size != size:
            raise Refused("file content or byte count mismatch")
        total += size
        if total > MAX_TOTAL_BYTES:
            raise Refused("total package size exceeds safety bound")
        seen.add(name)
        if target.suffix.lower() in (".exe", ".dll", ".msi", ".msix"):
            executables.append(target)
    if not executables:
        raise Refused("no native signed executable/installer candidate")
    actual = set()
    for candidate in root.rglob("*"):
        if candidate.is_symlink():
            raise Refused("linked or external files refused")
        if candidate.is_file():
            actual.add(candidate.relative_to(root).as_posix())
    if actual != seen | {"release.json"}:
        raise Refused("unmanifested or omitted staged file")
    verifier = signature_check if signature_check is not None else windows_signature
    for binary in executables:
        if verifier(binary) != thumb:
            raise Refused("publisher thumbprint mismatch")
    return {
        "schema": SCHEMA, "status": "PASS", "source_sha": expected_source,
        "version": manifest["version"], "files": len(seen),
        "signed_files": len(executables), "bytes": total,
        "limitations": "preflight only; no install, auto-update or native acceptance"
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--staging-dir", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--current-version", required=True)
    parser.add_argument("--trusted-publisher-thumbprint", required=True)
    args = parser.parse_args(argv)
    try:
        print(json.dumps(inspect(
            args.staging_dir, args.source_sha, args.current_version,
            args.trusted_publisher_thumbprint
        ), sort_keys=True))
        return 0
    except (Refused, OSError) as ex:
        print(json.dumps({"status": "BLOCKED", "reason": str(ex)}, sort_keys=True))
        return 2


if __name__ == "__main__":
    sys.exit(main())
