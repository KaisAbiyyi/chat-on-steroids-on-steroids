#!/usr/bin/env python3
"""CoSoS 16C: deterministic headless bundle, dependency/license gate and audit.

Produces a source-bound, NOT-SIGNED test bundle. The full native desktop host,
WebView2 runtime, installer and owner profile are deliberately not packaged.
"""
import argparse
import hashlib
import json
import re
import sys
import tomllib
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_STORED

SCHEMA = "cosos.bundle.headless.v1"
APPROVED = frozenset({
    "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC",
    "Unicode-3.0", "Unicode-DFS-2016", "Zlib", "0BSD", "Unlicense",
    "CC0-1.0", "BSL-1.0", "MPL-2.0"
})
SHA40 = re.compile(r"[0-9a-f]{40}")
SHA64 = re.compile(r"[0-9a-f]{64}")
MAX_ARCHIVE = 256 * 1024 * 1024
MAX_NOTICE = 128 * 1024
LICENSE_NAMES = ("LICENSE", "COPYING", "NOTICE")


class Refused(ValueError):
    pass


def licenses(expr):
    if not isinstance(expr, str) or not expr.strip():
        raise Refused("missing SPDX license identifier")
    # Only the LLVM exception applied to Apache-2.0 is approved explicitly.
    # All OR/AND alternatives are still audited, never silently dropped.
    normalized = re.sub(r"[()]", " ", expr)
    tokens = normalized.split()
    if (not tokens or
            re.sub(r"\\s+", " ", normalized).strip() != " ".join(tokens)):
        raise Refused("malformed SPDX license expression")
    i = 0
    while i < len(tokens):
        token = tokens[i]
        if token in ("AND", "OR"):
            i += 1
            continue
        if token not in APPROVED:
            raise Refused("unsupported/unknown SPDX license: " + expr)
        if i + 1 < len(tokens) and tokens[i + 1] == "WITH":
            if i + 2 >= len(tokens) or (token, tokens[i + 2]) != ("Apache-2.0", "LLVM-exception"):
                raise Refused("unapproved license exception: " + expr)
            i += 3
        else:
            i += 1
    if tokens[0] in ("AND", "OR") or tokens[-1] in ("AND", "OR"):
        raise Refused("malformed SPDX conjunction")
    return expr

def read_notice(folder, explicit=None):
    if not isinstance(folder, Path) or not folder.is_dir():
        raise Refused("dependency source unavailable for license audit")
    paths = []
    if explicit:
        candidate = (folder / explicit).resolve()
        if not candidate.is_relative_to(folder.resolve()):
            raise Refused("license_file escapes package")
        paths.append(candidate)
    paths.extend(p for p in sorted(folder.iterdir())
                 if p.is_file() and p.name.upper().startswith(LICENSE_NAMES))
    seen = set()
    chunks = []
    for path in paths:
        if path in seen or path.is_symlink() or not path.is_file():
            continue
        seen.add(path)
        if path.stat().st_size > MAX_NOTICE:
            raise Refused("oversize third party license text")
        try:
            content = path.read_text("utf-8")
        except (UnicodeError, OSError) as ex:
            raise Refused("non-readable third party license text") from ex
        if content.strip():
            chunks.append("=== " + path.name + " ===\n" + content.strip())
    if not chunks:
        raise Refused("missing license text for dependency: " + folder.name)
    return "\n\n".join(chunks)


def inventory(metadata, lockfile, project_license=None):
    if (not isinstance(metadata, dict) or
            not isinstance(metadata.get("packages"), list) or
            not isinstance(lockfile, dict) or not isinstance(lockfile.get("package"), list)):
        raise Refused("invalid cargo metadata or lockfile")
    locked = {(p["name"], p["version"], p.get("source")): p
              for p in lockfile["package"]}
    if len(locked) != len(lockfile["package"]):
        raise Refused("ambiguous lockfile packages")
    result, notices, seen = [], [], set()
    for p in metadata["packages"]:
        if not isinstance(p, dict):
            raise Refused("invalid cargo package")
        name, ver, source = p.get("name"), p.get("version"), p.get("source")
        key = (name, ver, source)
        if key not in locked or key in seen:
            raise Refused("metadata/lockfile mismatch")
        seen.add(key)
        license_id = licenses(p.get("license"))
        if source and not source.startswith("registry+"):
            raise Refused("unreviewed non-registry cargo source")
        checksum = locked[key].get("checksum")
        if source and (not isinstance(checksum, str) or not SHA64.fullmatch(checksum)):
            raise Refused("registry source is not checksum pinned")
        manifest = p.get("manifest_path")
        if not isinstance(manifest, str):
            raise Refused("missing dependency source manifest")
        directory = Path(manifest).parent
        entry = {
            "name": name, "version": ver, "license": license_id,
            "source": "registry" if source else "workspace",
            "checksum": checksum if source else None
        }
        if source is None and not any(
            item.is_file() and item.name.upper().startswith(LICENSE_NAMES)
            for item in directory.iterdir()
        ):
            if project_license is None:
                raise Refused("workspace license text unavailable")
            text = "=== PROJECT LICENSE ===\\n" + Path(project_license).read_text("utf-8")
        else:
            text = read_notice(directory, p.get("license_file"))
        notices.append("## " + name + " " + ver + " [" + license_id + "]\n" + text)
        result.append(entry)
    if not result:
        raise Refused("no Cargo dependencies discovered")
    result.sort(key=lambda x: (x["name"], x["version"], x["source"]))
    notices.sort()
    return result, "THIRD PARTY LICENSE TEXTS — source release receipt\n\n" + "\n\n".join(notices) + "\n"


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def sha256_file(path):
    h = hashlib.sha256()
    total = 0
    with Path(path).open("rb") as inp:
        for block in iter(lambda: inp.read(1024 * 1024), b""):
            total += len(block)
            if total > MAX_ARCHIVE:
                raise Refused("oversize binary or archive")
            h.update(block)
    return h.hexdigest(), total


def zip_member(name, executable=False):
    info = ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    info.compress_type = ZIP_STORED
    info.external_attr = (0o100755 if executable else 0o100644) << 16
    return info


def build(binary, project_license, meta, lock, output, os_target, source):
    if not isinstance(source, str) or not SHA40.fullmatch(source):
        raise Refused("source commit must be a full SHA-1")
    if os_target not in ("windows-x64", "linux-x64"):
        raise Refused("unsupported platform")
    binary = Path(binary)
    if binary.is_symlink() or not binary.is_file():
        raise Refused("missing or linked headless binary")
    name = "cos-rust.exe" if os_target.startswith("windows") else "cos-rust"
    if binary.name != name or binary.stat().st_size > MAX_ARCHIVE:
        raise Refused("unexpected/oversize headless binary")
    project_license = Path(project_license)
    if project_license.is_symlink() or not project_license.is_file():
        raise Refused("project LICENSE missing")
    packages, notices = inventory(meta, lock, project_license)
    binary_sha, binary_len = sha256_file(binary)
    members = {
        "LICENSE": project_license.read_bytes(),
        "THIRD_PARTY_NOTICES.txt": notices.encode("utf-8")
    }
    items = [{"path": name, "bytes": binary_len, "sha256": binary_sha}]
    items.extend({"path": key, "bytes": len(value), "sha256": sha256(value)}
                 for key, value in sorted(members.items()))
    index = {
        "schema": SCHEMA, "source_sha": source, "platform": os_target,
        "binary": name, "signature": "NOT_SIGNED",
        "native_host": "NOT_INCLUDED", "webview2_runtime": "EXTERNAL_NOT_VERIFIED",
        "files": sorted(items, key=lambda x: x["path"]),
        "packages": packages
    }
    if binary_len + sum(len(v) for v in members.values()) > MAX_ARCHIVE:
        raise Refused("bundle over size limit")
    members["bundle-index.json"] = (json.dumps(index, sort_keys=True, indent=2) + "\n").encode("utf-8")
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise Refused("refusing to overwrite an existing bundle")
    if not output.parent.is_dir():
        raise Refused("bundle parent must already exist")
    try:
        with ZipFile(output, "x", compression=ZIP_STORED, allowZip64=False) as z:
            for filename, data in sorted(members.items()):
                z.writestr(zip_member(filename), data)
            with z.open(zip_member(name, executable=True), "w", force_zip64=False) as dest:
                with binary.open("rb") as source_file:
                    for block in iter(lambda: source_file.read(1024 * 1024), b""):
                        dest.write(block)
        return verify(output, source)
    except Exception:
        output.unlink(missing_ok=True)
        raise


def verify(bundle, source):
    bundle = Path(bundle)
    if bundle.is_symlink() or not bundle.is_file() or bundle.stat().st_size > MAX_ARCHIVE + 2 * 1024 * 1024:
        raise Refused("unsafe or oversize archive")
    with ZipFile(bundle, "r") as z:
        infos = z.infolist()
        names = [x.filename for x in infos]
        if len(names) != len(set(names)) or not 1 <= len(names) <= 5:
            raise Refused("duplicate or excessive bundle members")
        for info in infos:
            if ("/" in info.filename or "\\" in info.filename or ":" in info.filename
                    or info.filename.startswith(".") or info.file_size > MAX_ARCHIVE
                    or info.compress_type != ZIP_STORED
                    or (info.external_attr >> 16) & 0o170000 not in (0, 0o100000)):
                raise Refused("unsafe bundle member")
        if "bundle-index.json" not in names:
            raise Refused("missing index")
        index = json.loads(z.read("bundle-index.json"))
        if (index.get("schema") != SCHEMA or index.get("source_sha") != source
                or index.get("signature") != "NOT_SIGNED"
                or index.get("native_host") != "NOT_INCLUDED"
                or index.get("webview2_runtime") != "EXTERNAL_NOT_VERIFIED"):
            raise Refused("bundle provenance/schema mismatch")
        files = index.get("files")
        if not isinstance(files, list) or set(names) != {x["path"] for x in files} | {"bundle-index.json"}:
            raise Refused("bundle index coverage mismatch")
        for item in files:
            h = hashlib.sha256()
            size = 0
            with z.open(item["path"]) as stream:
                for block in iter(lambda: stream.read(1024 * 1024), b""):
                    size += len(block)
                    if size > MAX_ARCHIVE:
                        raise Refused("oversize ZIP content")
                    h.update(block)
            if size != item["bytes"] or h.hexdigest() != item["sha256"]:
                raise Refused("bundle checksum mismatch")
        return {
            "status": "PASS", "schema": SCHEMA, "source_sha": source,
            "platform": index["platform"], "files": len(files),
            "dependencies": len(index["packages"]), "bundle_sha256": sha256_file(bundle)[0],
            "native_release": "BLOCKED: unsigned headless-only, native host absent"
        }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="cmd", required=True)
    build_cmd = commands.add_parser("build")
    build_cmd.add_argument("--binary", required=True)
    build_cmd.add_argument("--project-license", required=True)
    build_cmd.add_argument("--metadata-json", required=True)
    build_cmd.add_argument("--cargo-lock", required=True)
    build_cmd.add_argument("--out", required=True)
    build_cmd.add_argument("--platform", required=True)
    build_cmd.add_argument("--source-sha", required=True)
    check = commands.add_parser("verify")
    check.add_argument("--bundle", required=True)
    check.add_argument("--source-sha", required=True)
    args = parser.parse_args(argv)
    try:
        if args.cmd == "build":
            meta = json.loads(Path(args.metadata_json).read_text("utf-8"))
            lock = tomllib.loads(Path(args.cargo_lock).read_text("utf-8"))
            receipt = build(args.binary, args.project_license, meta, lock, args.out, args.platform, args.source_sha)
        else:
            receipt = verify(args.bundle, args.source_sha)
        print(json.dumps(receipt, sort_keys=True))
        return 0
    except (Refused, OSError, KeyError, TypeError, ValueError) as ex:
        print(json.dumps({"status": "BLOCKED", "reason": str(ex)}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
