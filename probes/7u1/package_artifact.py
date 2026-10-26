#!/usr/bin/env python3
"""Build an exact-commit SHA256 manifest for a synthetic Windows 7L2 artifact.

No product integration is created or asserted; this script neither launches a
browser nor accesses provider accounts. Run only against a clean CI checkout.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from preflight import Refused, hash_file, validate_manifest_path, verify_manifest


def package(repo: Path, artifact: Path) -> str:
    repo = repo.absolute()
    artifact = artifact.absolute()
    for base in (repo, artifact):
        for parent in (base, *base.parents):
            if parent.is_symlink() or getattr(parent, "is_junction", lambda: False)():
                raise Refused("reparse point in repository or artifact path")
    if not artifact.is_dir():
        raise Refused("artifact directory is absent")
    if (artifact / "source.sha").exists() or (artifact / "hashes.json").exists():
        raise Refused("refuse to replace an existing attestation")

    source_sha = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        check=True, text=True, capture_output=True, timeout=10
    ).stdout.strip().lower()
    if len(source_sha) != 40 or any(c not in "0123456789abcdef" for c in source_sha):
        raise Refused("invalid Git HEAD")
    status = subprocess.run(
        ["git", "-C", str(repo), "status", "--porcelain", "--untracked-files=all"],
        check=True, text=True, capture_output=True, timeout=10
    )
    if status.stdout.strip():
        raise Refused("checkout must be clean before packaging")

    original = list(artifact.rglob("*"))
    files = []
    names = set()
    for path in original:
        if path.is_symlink() or getattr(path, "is_junction", lambda: False)():
            raise Refused("reparse point inside artifact")
        if path.is_file():
            raw = path.relative_to(artifact).as_posix()
            folded = validate_manifest_path(raw)
            if folded in names:
                raise Refused("case-folded artifact path collision")
            names.add(folded)
            files.append(path)
        elif not path.is_dir():
            raise Refused("unexpected artifact entry")
    if not files or len(files) > 4094:
        raise Refused("artifact missing or too many files")

    # This manifest only qualifies existing 7L2 *synthetic* source-owned builds.
    # In particular, never rename these to CoSoSProductBrowser or 7u1_product_bridge.
    if not (artifact / "host" / "WindowsHost.dll").is_file():
        raise Refused("synthetic WebView2 WindowsHost.dll is absent")
    if not (artifact / "7l2_bridge.exe").is_file():
        raise Refused("synthetic Rust 7l2_bridge.exe is absent")
    for forbidden in ("host/CoSoSProductBrowser.dll", "7u1_product_bridge.exe"):
        if (artifact / forbidden).exists():
            raise Refused("product-named binary forbidden in synthetic bundle")

    (artifact / "source.sha").write_text(source_sha + "\n", encoding="utf-8")
    (artifact / "artifact-kind.json").write_text(
        json.dumps({"schema": "cosos.7u1.artifact-kind.v1", "kind": "synthetic-7l2-only"},
                   sort_keys=True) + "\n", encoding="utf-8"
    )
    files.extend([artifact / "source.sha", artifact / "artifact-kind.json"])
    entries = [
        {"path": path.relative_to(artifact).as_posix(), "sha256": hash_file(path)}
        for path in sorted(files, key=lambda x: x.relative_to(artifact).as_posix())
    ]
    (artifact / "hashes.json").write_text(
        json.dumps(entries, indent=2) + "\n", encoding="utf-8"
    )
    verify_manifest(artifact, source_sha)
    return source_sha


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--artifact", type=Path, required=True)
    args = parser.parse_args()
    try:
        sha = package(args.repo, args.artifact)
    except (Refused, OSError, ValueError, subprocess.SubprocessError) as error:
        print("BLOCKED: " + str(error), file=sys.stderr)
        return 2
    print("SYNTHETIC ONLY / NOT 7U1: sourceSha=" + sha)
    return 0


if __name__ == "__main__":
    sys.exit(main())
