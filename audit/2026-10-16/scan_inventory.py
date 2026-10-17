"""Read-only, reproducible inventory of the pinned CoS checkout (stdlib only)."""
from __future__ import annotations

import collections
import csv
import hashlib
import json
import re
import subprocess
from pathlib import Path

BASE = Path(__file__).resolve().parents[2]
ROOT = BASE / "upstream" / "chat-on-steroids"
OUT = Path(__file__).resolve().parent
CODE_EXTENSIONS = {".ts", ".tsx", ".js", ".mjs", ".cjs", ".rs", ".c", ".cc", ".cpp", ".h", ".mm", ".swift", ".py", ".ps1", ".sh"}
TEXT_EXTENSIONS = CODE_EXTENSIONS | {".md", ".txt", ".json", ".yml", ".yaml", ".html", ".css", ".toml", ".nix", ".nsh", ".xml"}


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True, encoding="utf-8").strip()


def write_json(name: str, value: object) -> None:
    (OUT / name).write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


files = []
imports = []
ipc = []
tests = []
file_texts = {}
import_re = re.compile(r"(?:\bfrom\s*|\bimport\s*\(|\brequire\s*\()\s*['\"]([^'\"]+)['\"]|\bimport\s+['\"]([^'\"]+)['\"]")
ipc_re = re.compile(r"\b(ipcMain|ipcRenderer)\s*\.\s*(handle|on|invoke|send|once|removeHandler|removeListener)\s*\(\s*['\"]([^'\"]+)['\"]")
for rel in git("ls-files", "-z").split("\0"):
    if not rel:
        continue
    path = ROOT / rel
    if not path.is_file():
        continue
    data = path.read_bytes()
    ext = path.suffix.lower() or "[none]"
    text = data.decode("utf-8", errors="replace") if ext in TEXT_EXTENSIONS else None
    lines = len(text.splitlines()) if text is not None else None
    row = {"path": rel, "bytes": len(data), "lines": lines, "extension": ext,
           "top_directory": rel.split("/")[0] if "/" in rel else "(root)",
           "sha256": hashlib.sha256(data).hexdigest()}
    files.append(row)
    if text is None:
        continue
    file_texts[rel] = text
    if ext in CODE_EXTENSIONS:
        for match in import_re.finditer(text):
            imports.append({"path": rel, "line": text.count("\n", 0, match.start()) + 1,
                            "module": match.group(1) or match.group(2)})
        for match in ipc_re.finditer(text):
            ipc.append({"path": rel, "line": text.count("\n", 0, match.start()) + 1,
                        "side": match.group(1), "method": match.group(2), "channel": match.group(3)})
    if rel.startswith("test/") and rel.endswith(".test.ts"):
        # Static lexical test declarations; runtime-generated tests and .each expansion differ.
        declarations = re.findall(r"(?m)\b(?:it|test)\s*(?:\.\s*(?:skip|only|todo|concurrent|fails|each)\b[^\n]*?)?\s*\(", text)
        tests.append({"path": rel, "lines": lines, "lexical_test_declarations": len(declarations)})


def summarize(key: str) -> list[dict]:
    groups = collections.defaultdict(lambda: {"files": 0, "bytes": 0, "text_lines": 0, "code_lines": 0})
    for row in files:
        group = groups[row[key]]
        group["files"] += 1
        group["bytes"] += row["bytes"]
        group["text_lines"] += row["lines"] or 0
        if row["extension"] in CODE_EXTENSIONS:
            group["code_lines"] += row["lines"] or 0
    return [{key: name, **value} for name, value in sorted(groups.items(), key=lambda item: -item[1]["bytes"])]


package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
lock = json.loads((ROOT / "package-lock.json").read_text(encoding="utf-8"))
locked = lock.get("packages", {})
prod = {key: value for key, value in locked.items() if key and not value.get("dev", False)}
optional = {key: value for key, value in prod.items() if value.get("optional", False)}
node_apis = collections.Counter(row["module"] for row in imports if row["module"].startswith("node:"))
external = collections.Counter(row["module"] for row in imports if not row["module"].startswith((".", "/", "node:")))
internal = [row for row in imports if row["module"].startswith(".")]
fan_in = collections.Counter()
for row in internal:
    stem = (ROOT / row["path"]).parent / row["module"]
    candidates = [stem]
    if stem.suffix in (".js", ".mjs"):
        candidates.extend(stem.with_suffix(suffix) for suffix in (".ts", ".tsx", ".mts"))
    candidates.extend([*[Path(str(stem) + suffix) for suffix in (".ts", ".tsx", ".js", ".mjs")], stem / "index.ts"])
    for candidate in candidates:
        if candidate.is_file():
            fan_in[candidate.resolve().relative_to(ROOT.resolve()).as_posix()] += 1
            row["resolved"] = candidate.resolve().relative_to(ROOT.resolve()).as_posix()
            break

stable_tag = json.loads((OUT / "github-latest-release.json").read_text(encoding="utf-8-sig"))["tag_name"]
stable_sha = git("rev-parse", stable_tag + "^{commit}")
identity = {"repository": git("remote", "get-url", "origin"), "branch": git("branch", "--show-current"),
            "head": git("rev-parse", "HEAD"), "head_commit": git("log", "-1", "--format=%cI %s"),
            "package_version": package["version"], "latest_release_tag": stable_tag,
            "latest_release_commit": stable_sha, "commits_since_release": int(git("rev-list", "--count", stable_tag + "..HEAD")),
            "working_tree_status": git("status", "--porcelain=v1"),
            "analysis_date": "2026-10-16", "analysis_timezone": "Asia/Jakarta"}
summary = {"identity": identity, "tracked_files": len(files), "tracked_bytes": sum(row["bytes"] for row in files),
           "code_files": sum(row["extension"] in CODE_EXTENSIONS for row in files),
           "code_lines_including_tests_scripts": sum((row["lines"] or 0) for row in files if row["extension"] in CODE_EXTENSIONS),
           "directories": summarize("top_directory"), "extensions": summarize("extension"),
           "largest_files_by_bytes": sorted(files, key=lambda row: -row["bytes"])[:25],
           "largest_production_code_by_lines": sorted([row for row in files if row["extension"] in CODE_EXTENSIONS and row["top_directory"] in ("src", "extension", "native")], key=lambda row: -(row["lines"] or 0))[:30],
           "direct_production_dependencies": len(package.get("dependencies", {})),
           "direct_development_dependencies": len(package.get("devDependencies", {})),
           "lock_package_records": len(locked) - int("" in locked),
           "lock_production_records_including_optional": len(prod), "lock_optional_production_records": len(optional),
           "lexical_test_files": len(tests), "lexical_test_declarations": sum(row["lexical_test_declarations"] for row in tests),
           "electron_importing_files": sorted(set(row["path"] for row in imports if row["module"] == "electron")),
           "node_api_imports": dict(node_apis.most_common()), "external_imports": dict(external.most_common()),
           "top_internal_import_fan_in": fan_in.most_common(30),
           "ipc_unique_literal_main_handles": sorted(set(row["channel"] for row in ipc if row["side"] == "ipcMain" and row["method"] == "handle")),
           "counting_notes": ["Code lines are physical lines, include blank lines/comments; not SLOC.",
                              "Lock records include target-specific optional packages; not measured packaged modules or disk size.",
                              "Imports and IPC are regex-extracted and can miss wrappers/dynamic cases.",
                              "Test declarations are lexical counts, not executed/expanded cases.",
                              "Tracked bytes exclude .git, downloaded dependencies, runtime data and release binaries."]}
write_json("source-identity.json", identity)
write_json("inventory.json", summary)
write_json("imports.json", imports)
write_json("ipc-channels.json", ipc)
write_json("test-inventory.json", tests)
write_json("production-lock-records.json", prod)
with (OUT / "tracked-files.csv").open("w", newline="", encoding="utf-8") as stream:
    writer = csv.DictWriter(stream, fieldnames=list(files[0]))
    writer.writeheader()
    writer.writerows(files)
(OUT / "main-vs-release.diffstat.txt").write_text(git("diff", "--stat", stable_tag + "..HEAD") + "\n", encoding="utf-8")
(OUT / "main-vs-release.commits.txt").write_text(git("log", "--format=%h %cI %s", stable_tag + "..HEAD") + "\n", encoding="utf-8")
print(json.dumps({key: summary[key] for key in ("identity", "tracked_files", "tracked_bytes", "code_files", "code_lines_including_tests_scripts", "direct_production_dependencies", "lock_production_records_including_optional", "lexical_test_files", "lexical_test_declarations", "directories")}, indent=2))
