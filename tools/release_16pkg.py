#!/usr/bin/env python3
"""16PKG: deterministic source-bound *unsigned candidate* Windows desktop bundle.

This builds the actual Rust runtime and real 15HOST renderer files provided by CI.
An intact ZIP is NOT a trusted signed installer; release_16b signer checks and
issuer-authorized main-ref release attestation remain required downstream.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import stat
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED

SOURCE = re.compile(r"[a-f0-9]{40}\Z")
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z")
HEXDIGEST = re.compile(r"[a-f0-9]{64}\Z")
REQUIRED = {"bin/cos-rust.exe", "renderer/WindowsRenderer.exe",
            "renderer/WindowsRenderer.dll", "renderer/WindowsRenderer.deps.json",
            "renderer/WindowsRenderer.runtimeconfig.json", "renderer/renderer-integrity.json",
            "LICENSE.txt"}
MAX_FILES = 250
MAX_FILE = 512 * 1024 * 1024
MAX_TOTAL = 1024 * 1024 * 1024
MAX_MANIFEST = 128 * 1024


class Refused(ValueError):
    pass


def safe_name(name):
    if not isinstance(name, str) or not 0 < len(name) <= 240 or name.startswith("/"):
        return False
    for part in name.split("/"):
        if (part in ("", ".", "..") or part.endswith((".", " "))
                or any(ord(c) < 32 or c in '<>:"\\|?*' for c in part)
                or re.fullmatch(r"(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])",
                                part.split(".", 1)[0], re.IGNORECASE)):
            return False
    return True


def digest(data):
    return hashlib.sha256(data).hexdigest()


def is_link(path):
    return path.is_symlink() or getattr(path, "is_junction", lambda: False)()


def source_file(path):
    path = Path(path)
    if any(is_link(p) for p in (path, *path.parents)):
        raise Refused("linked source path")
    if not path.is_file() or not 0 < path.stat().st_size <= MAX_FILE:
        raise Refused("missing or oversized source file")
    return path.read_bytes()


def renderer_payload(renderer_dir, source_sha):
    root = Path(renderer_dir).resolve(strict=True)
    if is_link(Path(renderer_dir)):
        raise Refused("linked renderer root")
    entries = {}
    existing_manifest = None
    for path in sorted(root.rglob("*")):
        if is_link(path):
            raise Refused("linked renderer member")
        if path.is_dir():
            continue
        if not path.is_file():
            raise Refused("nonfile renderer member")
        relative = path.relative_to(root).as_posix()
        if relative == "renderer-integrity.json":
            existing_manifest = source_file(path)
            continue
        if not safe_name(relative):
            raise Refused("unsafe renderer path")
        entries[relative] = source_file(path)
    for name in ("WindowsRenderer.exe", "WindowsRenderer.dll",
                 "WindowsRenderer.deps.json", "WindowsRenderer.runtimeconfig.json"):
        if name not in entries:
            raise Refused("missing actual renderer " + name)
    if len(entries) > 127:
        raise Refused("renderer exceeds Rust payload bound")
    manifest = {"format": 1, "sourceSha": source_sha,
                "files": [{"path": name, "sha256": digest(contents)}
                          for name, contents in sorted(entries.items())]}
    if existing_manifest is not None:
        try:
            supplied = json.loads(existing_manifest)
            if (not isinstance(supplied, dict)
                    or set(supplied) != {"format", "sourceSha", "files"}
                    or supplied["format"] != 1 or supplied["sourceSha"] != source_sha
                    or not isinstance(supplied["files"], list)
                    or len(supplied["files"]) != len(entries)):
                raise Refused("supplied renderer manifest identity mismatch")
            seen = set()
            for item in supplied["files"]:
                if (not isinstance(item, dict) or set(item) != {"path", "sha256"}
                        or item["path"] in seen or item["path"] not in entries
                        or digest(entries[item["path"]]) != item["sha256"]):
                    raise Refused("supplied renderer manifest content mismatch")
                seen.add(item["path"])
        except (KeyError, TypeError, ValueError) as ex:
            raise Refused("invalid supplied renderer manifest") from ex
        entries["renderer-integrity.json"] = existing_manifest
    else:
        # This generated manifest has NO issuer attestation and is a CI candidate.
        entries["renderer-integrity.json"] = json.dumps(
            manifest, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return {"renderer/" + name: data for name, data in entries.items()}


def _zip_entry(name, data):
    info = ZipInfo(name, (1980, 1, 1, 0, 0, 0))
    info.compress_type = ZIP_DEFLATED
    info.external_attr = 0o100644 << 16
    return info, data


def build(binary, renderer_dir, license_path, out, source_sha, version):
    if not SOURCE.fullmatch(source_sha) or not VERSION.fullmatch(version):
        raise Refused("invalid exact source or version")
    output = Path(out)
    if output.exists() or is_link(output) or any(is_link(p) for p in output.parents):
        raise Refused("output exists or linked parent")
    contents = {"bin/cos-rust.exe": source_file(binary),
                "LICENSE.txt": source_file(license_path)}
    contents.update(renderer_payload(renderer_dir, source_sha))
    if not REQUIRED.issubset(contents) or len(contents) > MAX_FILES:
        raise Refused("incomplete product runtime or file count exceeded")
    if sum(len(data) for data in contents.values()) > MAX_TOTAL:
        raise Refused("total product bytes exceeded")
    if len({name.casefold() for name in contents}) != len(contents):
        raise Refused("case-folded package collision")
    files = [{"path": name, "bytes": len(data), "sha256": digest(data)}
             for name, data in sorted(contents.items())]
    manifest = {"schema": "cosos.release.v1", "product": "CoSoS",
                "platform": "windows-x64", "source_sha": source_sha,
                "version": version, "files": files}
    contents["release.json"] = json.dumps(
        manifest, sort_keys=True, separators=(",", ":")).encode("utf-8")
    if len(contents["release.json"]) > MAX_MANIFEST:
        raise Refused("release manifest too large")
    output.parent.mkdir(parents=True, exist_ok=True)
    # Exclusive creation prevents replacing someone else's artifact.
    with output.open("xb") as sink:
        with ZipFile(sink, "w") as archive:
            for name, data in sorted(contents.items()):
                archive.writestr(*_zip_entry(name, data))
    try:
        return verify(output, source_sha)
    except Exception:
        output.unlink()
        raise


def verify(bundle, source_sha):
    if not SOURCE.fullmatch(source_sha):
        raise Refused("invalid source")
    bundle = Path(bundle)
    if not bundle.is_file() or is_link(bundle):
        raise Refused("bundle is missing/linked")
    names, data = set(), {}
    total = 0
    with ZipFile(bundle) as archive:
        infos = archive.infolist()
        if not 1 <= len(infos) <= MAX_FILES + 1:
            raise Refused("too many archive entries")
        for entry in infos:
            name = entry.filename
            if (not safe_name(name) or name.casefold() in names
                    or entry.is_dir() or entry.file_size > MAX_FILE
                    or entry.flag_bits & 0x1
                    or (entry.external_attr >> 16) & stat.S_IFMT(0o170000) == stat.S_IFLNK):
                raise Refused("unsafe archive entry")
            names.add(name.casefold())
            total += entry.file_size
            if total > MAX_TOTAL:
                raise Refused("expanded content cap exceeded")
            data[name] = archive.read(entry)
            if len(data[name]) != entry.file_size:
                raise Refused("decompressed byte length mismatch")
    if not REQUIRED.issubset(data):
        raise Refused("missing real desktop runtime")
    if len(data["release.json"]) > MAX_MANIFEST:
        raise Refused("release manifest oversized")
    manifest = json.loads(data["release.json"])
    if (not isinstance(manifest, dict)
            or set(manifest) != {"schema", "product", "platform", "source_sha", "version", "files"}
            or manifest["schema"] != "cosos.release.v1"
            or manifest["product"] != "CoSoS"
            or manifest["platform"] != "windows-x64"
            or manifest["source_sha"] != source_sha
            or not VERSION.fullmatch(manifest["version"])):
        raise Refused("release schema/source mismatch")
    indexed = manifest["files"]
    if not isinstance(indexed, list) or len(indexed) != len(data) - 1:
        raise Refused("release index count mismatch")
    expected = {}
    for item in indexed:
        if (not isinstance(item, dict) or set(item) != {"path", "bytes", "sha256"}
                or not safe_name(item["path"]) or item["path"] in expected
                or type(item["bytes"]) is not int or item["bytes"] < 1
                or not isinstance(item["sha256"], str) or not HEXDIGEST.fullmatch(item["sha256"])):
            raise Refused("invalid indexed member")
        expected[item["path"]] = item
    if set(data) != set(expected) | {"release.json"}:
        raise Refused("archive has omitted/unlisted bytes")
    for name, item in expected.items():
        if len(data[name]) != item["bytes"] or digest(data[name]) != item["sha256"]:
            raise Refused("package payload tamper")
    inner = json.loads(data["renderer/renderer-integrity.json"])
    if (not isinstance(inner, dict) or set(inner) != {"format", "sourceSha", "files"}
            or inner["format"] != 1 or inner["sourceSha"] != source_sha):
        raise Refused("renderer identity mismatch")
    actual_renderers = {name[len("renderer/"):]: raw for name, raw in data.items()
                        if name.startswith("renderer/") and name != "renderer/renderer-integrity.json"}
    if (not isinstance(inner["files"], list)
            or len(inner["files"]) != len(actual_renderers)
            or {x["path"] for x in inner["files"]} != set(actual_renderers)
            or any(digest(actual_renderers[x["path"]]) != x["sha256"] for x in inner["files"])):
        raise Refused("Rust renderer integrity index mismatch")
    return {"status": "UNSIGNED_CANDIDATE_ONLY", "source_sha": source_sha,
            "version": manifest["version"], "files": len(expected),
            "sha256": digest(bundle.read_bytes()),
            "signer_checked": False, "native_install_accepted": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    b = sub.add_parser("build")
    for key in ("binary", "renderer-dir", "license", "out", "source-sha", "version"):
        b.add_argument("--" + key, required=True)
    v = sub.add_parser("verify")
    v.add_argument("--bundle", required=True)
    v.add_argument("--source-sha", required=True)
    args = parser.parse_args()
    result = (build(args.binary, args.renderer_dir, args.license, args.out, args.source_sha, args.version)
              if args.command == "build" else verify(args.bundle, args.source_sha))
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (Refused, OSError, ValueError, KeyError, TypeError) as ex:
        parser_message = type(ex).__name__ + ": " + str(ex)
        raise SystemExit("16PKG refused: " + parser_message)
