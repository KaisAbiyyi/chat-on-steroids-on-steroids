"""Assign every tracked file a migration owner; classification is reviewable, not a semantic proof."""
import collections
import csv
import json
from pathlib import Path

OUT = Path(__file__).resolve().parent
files = list(csv.DictReader((OUT / "tracked-files.csv").open(encoding="utf-8")))
phases = [
    (1, "Freeze baseline and prove architecture", [], "baseline", "PARTIAL: static baseline complete; live baseline and architecture probes not run"),
    (2, "Rust foundation, config, permissions and secure storage", [1], "cos-core", "NOT STARTED"),
    (3, "Sessions, recording, identity and usage", [2], "cos-store", "NOT STARTED"),
    (4, "MCP, connections, tunnels, code mode and control API", [2,3], "cos-mcp", "NOT STARTED"),
    (5, "Files, patching, search, projects, Git and SSH files", [2,3,4], "cos-tools", "NOT STARTED"),
    (6, "Commands, PTY terminals, shell policy and SSH execution", [2,3,4,5], "cos-exec", "NOT STARTED"),
    (7, "Browser companion, bridge and direct browser tools", [2,3,4], "cos-bridge + extension-wasm", "NOT STARTED"),
    (8, "Built-in browser and sign-in compatibility", [3,7], "cos-browser", "NOT STARTED"),
    (9, "Outbox, attachments, steering, finish and generated workflows", [3,4,7], "cos-workflow/outbox", "NOT STARTED"),
    (10, "Compact and Resume, recovery and continuation", [3,7,8,9], "cos-workflow/continuation", "NOT STARTED"),
    (11, "Worker families, inboxes, worktrees and worker archive", [3,5,6,7,9,10], "cos-workflow/agents", "NOT STARTED"),
    (12, "Goal, Loop, planning and helper decisions", [9,10,11], "cos-workflow/goal", "NOT STARTED"),
    (13, "Plugins, Skills, OAuth and optional runtimes", [2,4,5,6,9], "cos-plugins", "NOT STARTED"),
    (14, "Native desktop, capture, dictation, image fidelity and pets", [2,3,4,5,7,11], "cos-desktop", "NOT STARTED"),
    (15, "Complete desktop UI, rich editors and localization", [3,5,6,8,9,10,11,12,13,14], "cos-ui", "NOT STARTED"),
    (16, "Cross-platform conformance, migration, packaging and size gates", list(range(1,16)), "cos-app + release", "NOT STARTED"),
]
phase_rows = [{"phase":n,"title":title,"depends_on":deps,"target_owner":owner,"status":status} for n,title,deps,owner,status in phases]
by_phase = {row["phase"]:row for row in phase_rows}


def classify(path):
    stem = Path(path).stem
    if path.startswith("extension/"):
        return 7
    if path.startswith("native/") or path.startswith("pets/"):
        return 14
    if path.startswith("src/renderer/"):
        if stem.startswith("cos-browser"):
            return 8
        if stem.startswith("pet") or "/pet-assets/" in path:
            return 14
        return 15
    if path.startswith("src/preload/"):
        return 8 if stem.startswith("cos-browser") else 14 if stem == "pet-overlay" else 15
    if path.startswith("src/main/"):
        if "/cos-browser/" in path:
            return 8
        if "/computer/" in path or stem.startswith("pet-") or stem in {"dictation", "image-export"}:
            return 14
        if "/plugins/" in path or "skill" in stem or stem in {"codex-plugin-runtime", "plugin-refresh", "plugins-ipc"}:
            return 13
        if "/worktrees/" in path or stem in {"agents", "agent-admission", "worker-archive"}:
            return 11
        if stem.startswith("goal") or stem == "task-request":
            return 12
        if "/session/" in path:
            if stem.startswith("input") or stem in {"start-input", "prompt", "finish", "request-plans"}:
                return 9
            if stem in {"continuation", "resume-gate", "handoff", "handoff-prompt"}:
                return 10
            return 3
        if stem in {"turn-rotation", "approval-wait"}:
            return 9
        if stem.startswith("browser") or stem.startswith("bridge") or stem in {"extension-path", "chat-models"}:
            return 7
        if "/remote/" in path:
            return 6 if stem == "ssh" else 5
        if "/codex/apply-patch/" in path or stem in {"filesystem", "read-backend", "view-image"}:
            return 5
        if "/codex/" in path or stem in {"exec", "exec-hints", "workspace-terminal", "workspace-terminal-ipc"}:
            return 6
        if "/mcp/" in path or "/tunnel/" in path or stem.startswith("control-") or stem in {"connection", "connector-identity", "connector-proof", "app-proxy", "connection-loss-notice"}:
            return 4
        if stem in {"rawfs", "fsops", "search", "ripgrep", "workspace", "projects", "project-files", "project-file-watcher", "project-git", "text-match", "diffstat", "toolchain"}:
            return 5
        if stem in {"provider-usage", "usage-tool"}:
            return 3
        if stem.startswith("diagnostics") or stem.startswith("update") or stem == "version":
            return 16
        if stem in {"ipc", "main-texts", "appearance-schema", "edit-context-menu", "window-icon", "window-layout", "window-lifecycle", "tray-image", "turn-notifications", "stuck-notice"}:
            return 15
        return 2
    if path.startswith("src/shared/"):
        if stem.startswith("goal"):
            return 12
        if stem.startswith("skill") or stem in {"recommended-skills", "plugins", "plugin-refresh"}:
            return 13
        if stem in {"pets", "pet-activity", "windows-computer", "dictation"}:
            return 14
        if stem in {"agent-health", "worker-brief"}:
            return 11
        if stem in {"agent-plan", "input", "user-prompt", "finish", "task-progress", "approval-wait"}:
            return 9
        if stem in {"handoff", "recovery"}:
            return 10
        if stem == "cos-browser-sites":
            return 8
        if stem.startswith("browser") or stem in {"firefox-addon", "chat-models"}:
            return 7
        if stem in {"control-api", "connector-names", "tunnel-ids"}:
            return 4
        if stem.startswith("project"):
            return 5
        if stem in {"workspace-terminal", "background-exec", "command-allowlist"}:
            return 6
        if stem in {"session", "session-activity", "chronology", "usage", "provider-usage", "turn-trace", "markdown-export", "content-reference", "dil-tables"}:
            return 3
        if stem in {"appearance", "ui-language", "whats-new", "main-texts", "message-reaction", "stop-notice", "chat-error", "external-link"}:
            return 15
        return 2
    if path.startswith(("docs/", "artwork/")) or path in {"AGENTS.md", "CLAUDE.md", "README.md", "SECURITY.md", "CONTRIBUTING.md", "CHANGELOG.md", "CONTRIBUTORS.md", "LICENSE"}:
        return 1
    return 16


mapping = []
for row in files:
    path = row["path"]
    phase = classify(path)
    if path.startswith("test/"):
        handling = "PORT_OR_RETAIN_ORACLE_TEST"
    elif path.startswith(("docs/", "artwork/")) or phase == 1:
        handling = "REFERENCE_AND_BASELINE"
    elif path.startswith("extension/") and row["extension"] in {".js", ".ts"}:
        handling = "RUST_WASM_CANDIDATE_REQUIRES_JS_BROWSER_ENTRY"
    elif row["extension"] in {".png", ".webp", ".woff", ".woff2", ".json", ".svg"} and path.startswith(("src/renderer/", "extension/", "pets/")):
        handling = "PRESERVE_ASSET_OR_DATA"
    elif path.startswith(("scripts/", ".github/", ".githooks/")) or phase == 16:
        handling = "REPLACE_BUILD_OR_RELEASE_TOOLING"
    else:
        handling = "PORT_PRODUCT_BEHAVIOR_TO_RUST"
    mapping.append({"path":path, "phase":phase, "phase_title":by_phase[phase]["title"], "target_owner":by_phase[phase]["target_owner"],
                    "handling":handling, "bytes":int(row["bytes"]), "physical_lines":int(row["lines"] or 0), "sha256":row["sha256"]})
with (OUT / "migration-file-map.csv").open("w", newline="", encoding="utf-8") as stream:
    writer = csv.DictWriter(stream, fieldnames=list(mapping[0]))
    writer.writeheader()
    writer.writerows(mapping)
counts = collections.Counter(row["phase"] for row in mapping)
product_counts = collections.Counter(row["phase"] for row in mapping if row["path"].startswith(("src/", "extension/", "native/")))
for phase in phase_rows:
    phase["tracked_files_owned"] = counts[phase["phase"]]
    phase["product_files_owned"] = product_counts[phase["phase"]]
product = [row for row in mapping if row["path"].startswith(("src/", "extension/", "native/"))]
result = {"phase_count":16,"phases":phase_rows,"tracked_file_count":len(files),"mapped_file_count":len(mapping),
          "product_file_count":len(product),"unassigned_files":[],"mapping_policy":"One primary owner per file; integration and secondary ownership remain necessary. Classification is a roadmap aid, not proof that all behavior is understood or ported."}
(OUT / "migration-phase-map.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"phases":16,"tracked":len(files),"mapped":len(mapping),"product_files":len(product),"unassigned":0,"by_phase":dict(sorted(product_counts.items()))}, indent=2))
