#!/usr/bin/env python3
"""Build an immutable SHA256 manifest for a clearly labelled 7U1-PREP artifact."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from audit_production_host import audit as audit_production_host
from preflight import PRODUCTION_SOURCE_COPIES, Refused, hash_file, validate_manifest_path, verify_manifest

SYNTHETIC_KIND = "synthetic-7l2-only"
PRODUCTION_HANDSHAKE_KIND = "production-8p-handshake-only-not-7u1"
def package(repo: Path, artifact: Path, kind: str = SYNTHETIC_KIND) -> str:
    repo = repo.absolute()
    artifact = artifact.absolute()
    for base in (repo, artifact):
        for parent in (base, *base.parents):
            if parent.is_symlink() or getattr(parent, "is_junction", lambda: False)():
                raise Refused("reparse point in repository or artifact path")
    if not artifact.is_dir():
        raise Refused("artifact directory is absent")
    if any((artifact / name).exists() for name in
           ("source.sha", "hashes.json", "artifact-kind.json")):
        raise Refused("refuse to replace an existing attestation")
    if kind not in (SYNTHETIC_KIND, PRODUCTION_HANDSHAKE_KIND):
        raise Refused("unsupported artifact kind")

    source_sha = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        check=True, text=True, capture_output=True, timeout=10
    ).stdout.strip().lower()
    if len(source_sha) != 40 or any(char not in "0123456789abcdef" for char in source_sha):
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

    forbidden = ("host/CoSoSProductBrowser.dll", "7u1_product_bridge.exe")
    if any((artifact / path).exists() for path in forbidden):
        raise Refused("invented product-named binary forbidden")

    if kind == SYNTHETIC_KIND:
        if not (artifact / "host" / "WindowsHost.dll").is_file():
            raise Refused("synthetic WebView2 WindowsHost.dll is absent")
        if not (artifact / "7l2_bridge.exe").is_file():
            raise Refused("synthetic Rust 7l2_bridge.exe is absent")
        (artifact / "artifact-kind.json").write_text(
            json.dumps({"schema": "cosos.7u1.artifact-kind.v1", "kind": SYNTHETIC_KIND},
                       sort_keys=True) + "\n", encoding="utf-8"
        )
    else:
        if not (artifact / "host" / "WindowsHost.dll").is_file():
            raise Refused("production WindowsHost.dll publish output is absent")
        if not (artifact / "7l2_bridge.exe").is_file():
            raise Refused("Rust 7l2_bridge.exe publish output is absent")
        for staged, source in PRODUCTION_SOURCE_COPIES.items():
            staged_path = artifact / staged
            source_path = repo / source
            if not staged_path.is_file() or not source_path.is_file():
                raise Refused("missing source-bound production input: " + staged)
            if staged_path.read_bytes() != source_path.read_bytes():
                raise Refused("staged production source differs from repository: " + staged)
        audit_path = artifact / "production-host-audit.json"
        if not audit_path.is_file():
            raise Refused("production host audit receipt is absent")
        audit_receipt = json.loads(audit_path.read_text(encoding="utf-8"))
        if (audit_receipt.get("sourceSha") != source_sha or
                audit_receipt.get("productionObservationStatus") != "BLOCKED" or
                audit_receipt.get("owner7U1Status") != "NOT_RUN" or
                audit_receipt.get("humanConsentSimulated") is not False or
                audit_receipt.get("providerAccessPerformed") is not False):
            raise Refused("production bundle must be labelled handshake-only and blocked")
        current_audit = audit_production_host(repo, expected_sha=source_sha)
        if current_audit["productionObservationStatus"] != "BLOCKED":
            raise Refused("production API surface changed; do not package as handshake-only")

        (artifact / "artifact-kind.json").write_text(
            json.dumps({"schema": "cosos.7u1.artifact-kind.v1",
                        "kind": PRODUCTION_HANDSHAKE_KIND}, sort_keys=True) + "\n",
            encoding="utf-8"
        )

    (artifact / "source.sha").write_text(source_sha + "\n", encoding="utf-8")
    files.extend([artifact / "source.sha", artifact / "artifact-kind.json"])
    entries = [
        {"path": path.relative_to(artifact).as_posix(), "sha256": hash_file(path)}
        for path in sorted(files, key=lambda item: item.relative_to(artifact).as_posix())
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
    parser.add_argument("--kind", choices=(SYNTHETIC_KIND, PRODUCTION_HANDSHAKE_KIND),
                        default=SYNTHETIC_KIND)
    args = parser.parse_args()
    try:
        sha = package(args.repo, args.artifact, args.kind)
    except (Refused, OSError, ValueError, subprocess.SubprocessError) as error:
        print("BLOCKED: " + str(error), file=sys.stderr)
        return 2
    print(args.kind, "sourceSha=" + sha)
    return 0


if __name__ == "__main__":
    sys.exit(main())
