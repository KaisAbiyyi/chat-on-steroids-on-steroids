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
    # Parse SPDX AND/OR (AND binds tighter). An OR branch with an approved
    # permissive choice is acceptable; AND needs all components approved.
    # Unknown WITH exceptions never get silent fallback through OR.
    # Legacy slash spellings are limited to explicit known dual-license pairs.
    normalized = expr
    for old, new in (
        ("MIT/Apache-2.0", "MIT OR Apache-2.0"),
        ("Apache-2.0/MIT", "Apache-2.0 OR MIT"),
        ("Unlicense/MIT", "Unlicense OR MIT"),
        ("MIT/Unlicense", "MIT OR Unlicense"),
    ):
        normalized = normalized.replace(old, new)
    tokens = normalized.replace("(", " ( ").replace(")", " ) ").split()
    if not tokens:
        raise Refused("malformed SPDX license expression")
    pos = 0

    def primary():
        nonlocal pos
        if pos >= len(tokens):
            raise Refused("incomplete SPDX license expression")
        tok = tokens[pos]
        pos += 1
        if tok == "(":
            value = either()
            if pos >= len(tokens) or tokens[pos] != ")":
                raise Refused("unclosed SPDX parentheses")
            pos += 1
            return value
        if not re.fullmatch("[A-Za-z0-9.+-]+", tok) or tok in ("AND", "OR", "WITH"):
            raise Refused("invalid SPDX token: " + tok)
        if pos < len(tokens) and tokens[pos] == "WITH":
            pos += 1
            if pos >= len(tokens) or (tok, tokens[pos]) != ("Apache-2.0", "LLVM-exception"):
                raise Refused("unapproved license exception: " + expr)
            pos += 1
        return tok in APPROVED

    def together():
        nonlocal pos
        result = primary()
        while pos < len(tokens) and tokens[pos] == "AND":
            pos += 1
            rhs = primary()
            result = result and rhs
        return result

    def either():
        nonlocal pos
        result = together()
        while pos < len(tokens) and tokens[pos] == "OR":
            pos += 1
            rhs = together()
            result = result or rhs
        return result

    approved = either()
    if pos != len(tokens):
        raise Refused("unexpected SPDX operator or token: " + tokens[pos])
    if not approved:
        raise Refused("unsupported/unknown SPDX license: " + expr)
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
    # r-efi publishes full original MIT/Apache/LGPL license and copyright
    # notices in AUTHORS rather than a LICENSE-prefixed file.
    if folder.name.startswith("r-efi-"):
        authors = folder / "AUTHORS"
        if authors.is_file() and not authors.is_symlink():
            text = authors.read_text("utf-8")
            if "LICENSE:" in text and "Permission is hereby granted" in text and "COPYRIGHT:" in text:
                paths.append(authors)
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


def selected_runtime_packages(metadata):
    packages = metadata["packages"]
    resolution = metadata.get("resolve")
    if resolution is None:
        return packages  # Only small synthetic tests omit Cargo's resolve graph.
    nodes = {node["id"]: node for node in resolution.get("nodes", [])}
    roots = [pkg["id"] for pkg in packages
             if pkg.get("name") == "cos-app" and pkg.get("source") is None]
    if len(roots) != 1 or roots[0] not in nodes:
        raise Refused("missing unique Cargo application root in lock graph")
    visited, pending = set(), roots[:]
    while pending:
        key = pending.pop()
        if key in visited:
            continue
        visited.add(key)
        node = nodes.get(key)
        if node is None:
            raise Refused("unresolved Cargo runtime dependency node")
        for dep in node.get("deps", []):
            kinds = dep.get("dep_kinds", [])
            if not isinstance(kinds, list):
                raise Refused("invalid Cargo dependency kind declaration")
            # Build-time macro dependencies are part of the resulting binary
            # provenance. Dev-only test crates are not shipped.
            if not kinds or any(k.get("kind") != "dev" for k in kinds):
                pending.append(dep["pkg"])
    subset = [pkg for pkg in packages if pkg.get("id") in visited]
    if len(subset) != len(visited):
        raise Refused("Cargo package set and resolved graph disagree")
    return subset


def inventory(metadata, lockfile, project_license=None):
    if (not isinstance(metadata, dict) or
            not isinstance(metadata.get("packages"), list) or
            not isinstance(lockfile, dict) or not isinstance(lockfile.get("package"), list)):
        raise Refused("invalid cargo metadata or lockfile")
    packages = selected_runtime_packages(metadata)
    license_errors = []
    for pkg in packages:
        try:
            licenses(pkg.get("license"))
        except Refused as ex:
            license_errors.append(str(pkg.get("name")) + "@" + str(pkg.get("version")) + ": " + str(ex))
    if license_errors:
        raise Refused("license audit blocked: " + "; ".join(license_errors[:30]))
    locked = {(p["name"], p["version"], p.get("source")): p
              for p in lockfile["package"]}
    if len(locked) != len(lockfile["package"]):
        raise Refused("ambiguous lockfile packages")
    result, notices, seen = [], [], set()
    for p in packages:
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
            try:
                text = read_notice(directory, p.get("license_file"))
            except Refused as err:
                if "missing license text for dependency" not in str(err) or not p.get("repository"):
                    raise
                # Cargo workspaces sometimes publish sibling crates without
                # COPYING while their sibling package carries the exact
                # upstream LICENSE. Require same upstream, version AND SPDX.
                mirrors = sorted((other for other in packages
                                  if other is not p
                                  and other.get("repository") == p.get("repository")
                                  and other.get("version") == ver
                                  and other.get("license") == license_id
                                  and other.get("source") == source),
                                 key=lambda q: q["name"])
                text = None
                for other in mirrors:
                    mirror_dir = Path(other["manifest_path"]).parent
                    try:
                        notice = read_notice(mirror_dir, other.get("license_file"))
                    except Refused:
                        continue
                    text = "=== Same-upstream license source: " + other["name"] + "@" + ver + " ===\\n" + notice
                    break
                if text is None and (
                    name == "tree-sitter" and ver == "0.25.1"
                    and license_id == "MIT"
                    and p["repository"] == "https://github.com/tree-sitter/tree-sitter"
                ):
                    # Reproducibly carry the exact v0.25.1 upstream original
                    # (git blob 451fe1...) when the crates.io package omits it.
                    pinned = (Path(__file__).resolve().parent.parent / "docs"
                              / "third-party-licenses" / "tree-sitter-0.25.1-LICENSE")
                    if pinned.is_symlink() or not pinned.is_file():
                        raise Refused("missing pinned upstream Tree-sitter notice")
                    raw = pinned.read_bytes()
                    actual = hashlib.sha1(
                        b"blob " + str(len(raw)).encode("ascii") + b"\x00" + raw
                    ).hexdigest()
                    if actual != "451fe1d26ee6043ac94f6c8a81a6143f17748b4d":
                        raise Refused("pinned Tree-sitter source notice altered")
                    text = (
                        "=== Upstream tree-sitter/tree-sitter tag v0.25.1 LICENSE "
                        "(git blob 451fe1d26ee6043ac94f6c8a81a6143f17748b4d) ===\\n"
                        + raw.decode("utf-8")
                    )
                if text is None:
                    raise err
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
