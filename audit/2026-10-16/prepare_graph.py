"""Scope the migration graph to product code and current behavioral documents."""
import json
from pathlib import Path
from graphify.cache import check_semantic_cache

BASE = Path(__file__).resolve().parents[2]
ROOT = BASE / "upstream" / "chat-on-steroids"
OUT = BASE / "graphify-out"
full_path = OUT / "full-corpus-detection.json"
full = json.loads((full_path if full_path.exists() else OUT / ".graphify_detect.json").read_text(encoding="utf-8"))
(OUT / "full-corpus-detection.json").write_text(json.dumps(full, ensure_ascii=False), encoding="utf-8")
code = [f for f in full["files"]["code"] if Path(f).relative_to(ROOT).parts[0] in {"src", "extension", "native"}]
# Native N-API .cc is a required compatibility boundary even if detector support changes.
addon = (ROOT / "native/macos-desktop-addon/addon.cc").as_posix()
if addon not in [Path(f).as_posix() for f in code]:
    code.append(addon)
docs = ["README.md", "SECURITY.md", "CONTRIBUTING.md", "docs/setup.md", "docs/plugins.md",
        "docs/tool-surface.md", "docs/chatgpt-turn-signals.md", "docs/codex-desktop-bridge.md",
        "docs/parity/FEATURE_PARITY_MATRIX.md", "docs/parity/UPSTREAM_SOURCES.md", "docs/recovery-inventory.md",
        "docs/recovery-usage-2026-09-12.md", "docs/usage-model-attribution.md", "docs/windows-dictation.md",
        "docs/workspace-integration-2026-09-12.md", "docs/workspace-recovery-dependencies-2026-09-13.md",
        "docs/pet/ACCEPTANCE.md", "docs/pet/PRODUCTION.md", "docs/plugin-licenses.md", "docs/plugin-notice-audit.md"]
docs = [(ROOT / name).as_posix() for name in docs if (ROOT / name).is_file()]
scoped = dict(full)
scoped["files"] = {"code": code, "document": docs, "paper": [], "image": [], "video": []}
scoped["total_files"] = len(code) + len(docs)
scoped["total_words"] = sum(len(Path(f).read_text(encoding="utf-8", errors="replace").split()) for f in code + docs)
scoped["warning"] = None
scoped["migration_graph_scope"] = "Product code in src, extension, native; 20 selected behavioral documents. Assets, generated notices, test code, build scripts and historical worklogs are inventoried and reviewed separately, not represented as full semantic graph nodes."
(OUT / ".graphify_detect.json").write_text(json.dumps(scoped, ensure_ascii=False), encoding="utf-8")
(OUT / "scope.json").write_text(json.dumps({k:scoped[k] for k in ("total_files", "total_words", "migration_graph_scope")}, indent=2), encoding="utf-8")
nodes, edges, hyperedges, uncached = check_semantic_cache(docs, root=str(BASE))
if nodes or edges or hyperedges:
    (OUT / ".graphify_cached.json").write_text(json.dumps({"nodes": nodes, "edges": edges, "hyperedges": hyperedges}), encoding="utf-8")
(OUT / ".graphify_uncached.txt").write_text("\n".join(uncached), encoding="utf-8")
print(json.dumps({"product_code_files": len(code), "semantic_docs": len(docs), "words": scoped["total_words"], "uncached_docs": len(uncached)}))
