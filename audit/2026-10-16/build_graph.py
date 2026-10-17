"""Normalize AST provenance, merge documentary evidence and build the migration map."""
import collections
import json
import re
from pathlib import Path
from graphify.build import build_from_json
from graphify.cluster import cluster, score_all
from graphify.analyze import god_nodes, surprising_connections, suggest_questions
from graphify.report import generate
from graphify.export import to_json
from graphify.diagnostics import diagnose_extraction, format_diagnostic_report
from graphify.cache import save_semantic_cache
from graphify.detect import save_manifest

BASE = Path(__file__).resolve().parents[2]
ROOT = BASE / "upstream" / "chat-on-steroids"
OUT = BASE / "graphify-out"
ast = json.loads((OUT / ".graphify_ast.json").read_text(encoding="utf-8"))
sem = json.loads((OUT / "semantic-docs.json").read_text(encoding="utf-8-sig"))
detection = json.loads((OUT / ".graphify_detect.json").read_text(encoding="utf-8"))
detection["warning"] = None  # The full-corpus warning is retained in full-corpus-detection.json.
(OUT / ".graphify_detect.json").write_text(json.dumps(detection, ensure_ascii=False), encoding="utf-8")
id_prefix = "upstream_chat_on_steroids_"
mapping = {n["id"]: n["id"].removeprefix(id_prefix) for n in ast["nodes"]}
if len(set(mapping.values())) != len(mapping):
    raise ValueError("AST ID normalization would collide")


def absolute_source(record):
    source = record.get("source_file")
    if source and not Path(source).is_absolute():
        candidate = BASE / source
        if candidate.is_file():
            record["source_file"] = candidate.as_posix()


for node in ast["nodes"]:
    node["id"] = mapping[node["id"]]
    absolute_source(node)
for edge in ast["edges"]:
    edge["source"] = mapping.get(edge["source"], edge["source"].removeprefix(id_prefix))
    edge["target"] = mapping.get(edge["target"], edge["target"].removeprefix(id_prefix))
    absolute_source(edge)
    edge.setdefault("confidence_score", {"EXTRACTED": 1.0, "INFERRED": 0.95, "AMBIGUOUS": 0.2}.get(edge.get("confidence"), 0.2))
seen = {n["id"] for n in ast["nodes"]}
nodes = ast["nodes"] + [n for n in sem["nodes"] if n["id"] not in seen]
edges = ast["edges"] + sem["edges"]
file_nodes = {n["source_file"]:n["id"] for n in ast["nodes"] if n.get("source_file") and n.get("label") == Path(n["source_file"]).name}
# Only explicit source paths in assigned documents create documentary references.
for source in detection["files"]["document"]:
    path = Path(source)
    doc_id = re.sub(r"[^a-z0-9]", "_", path.relative_to(ROOT).with_suffix("").as_posix().lower())
    if doc_id not in seen:
        nodes.append({"id":doc_id, "label":path.name, "file_type":"document", "source_file":source, "source_location":"L1"})
        seen.add(doc_id)
    for node in sem["nodes"]:
        if node["source_file"] == source:
            edges.append({"source":doc_id, "target":node["id"], "relation":"contains", "confidence":"EXTRACTED", "confidence_score":1.0, "source_file":source, "source_location":node.get("source_location"), "weight":1.0})
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        for match in re.finditer(r"(?:src|extension|native)/[A-Za-z0-9_./-]+\.(?:ts|js|swift|cc)", line):
            target = (ROOT / match.group()).as_posix()
            if target in file_nodes:
                edges.append({"source":doc_id, "target":file_nodes[target], "relation":"references", "confidence":"EXTRACTED", "confidence_score":1.0, "source_file":source, "source_location":f"L{line_number}", "weight":1.0})
merged = {"nodes":nodes, "edges":edges, "hyperedges":sem.get("hyperedges", []), "input_tokens":0, "output_tokens":0}
(OUT / ".graphify_extract.json").write_text(json.dumps(merged, indent=2, ensure_ascii=False), encoding="utf-8")
(OUT / ".graphify_semantic.json").write_text(json.dumps(sem, indent=2, ensure_ascii=False), encoding="utf-8")
save_semantic_cache(sem["nodes"], sem["edges"], sem.get("hyperedges", []), root=str(BASE))
graph = build_from_json(merged, root=str(ROOT), directed=True)
if not graph.number_of_nodes():
    raise ValueError("Graph is empty")
communities = cluster(graph)
cohesion = score_all(graph, communities)
gods = god_nodes(graph)
surprises = surprising_connections(graph, communities)


def community_label(node_ids):
    counts = collections.Counter()
    for node_id in node_ids:
        source = str(graph.nodes[node_id].get("source_file", "")).replace("\\", "/")
        if "/session/" in source:
            label = "Session State and Delivery"
        elif "/mcp/" in source:
            label = "MCP Dispatch and Contracts"
        elif "/cos-browser/" in source:
            label = "Built In Browser"
        elif "/codex/" in source:
            label = "File and Terminal Tools"
        elif "/computer/" in source or "/native/" in source:
            label = "Native Desktop Control"
        elif "/plugins/" in source or "skill" in Path(source).stem:
            label = "Plugins and Skills"
        elif "/extension/" in source:
            label = "Browser Companion"
        elif "/renderer/" in source:
            label = "Renderer Workspace"
        elif "/shared/" in source:
            label = "Shared Wire Contracts"
        elif "/docs/" in source:
            label = "Documented Behavior"
        elif "agents" in source or "worktrees" in source:
            label = "Workers and Worktrees"
        elif "goal" in source:
            label = "Goal and Loop"
        elif "bridge" in source or "browser" in source:
            label = "Browser Bridge"
        else:
            label = "Core Host Services"
        counts[label] += 1
    return counts.most_common(1)[0][0]


labels = {cid:community_label(ids) for cid, ids in communities.items()}
questions = suggest_questions(graph, communities, labels)
identity = json.loads((BASE / "audit/2026-10-16/source-identity.json").read_text(encoding="utf-8"))
if not to_json(graph, communities, str(OUT / "graph.json"), built_at_commit=identity["head"], community_labels=labels):
    raise ValueError("Graph export refused")
report = generate(graph, communities, cohesion, labels, gods, surprises, detection,
                  {"input":0,"output":0}, str(ROOT), suggested_questions=questions, built_at_commit=identity["head"])
report = report.replace("- Token cost: 0 input · 0 output", "- Token usage: unmetered; zero counters in raw metadata are placeholders.")
report += "\n## Migration audit scope and evidence limits\n\n" + detection["migration_graph_scope"] + "\n\nThe graph is an AST/semantic navigation aid, not a complete dynamic call graph. The 26 locale/animation JSON inputs have no structural AST nodes and remain covered by file inventory and the migration file map. AST IDs were normalized to the checkout root; original extraction is preserved. Token usage is unmetered by this host; zero counters are placeholders, not a claim of zero cost.\n"
(OUT / "GRAPH_REPORT.md").write_text(report, encoding="utf-8")
(OUT / ".graphify_labels.json").write_text(json.dumps({str(k):v for k,v in labels.items()}), encoding="utf-8")
(OUT / ".graphify_analysis.json").write_text(json.dumps({"communities":{str(k):v for k,v in communities.items()},"cohesion":{str(k):v for k,v in cohesion.items()},"gods":gods,"surprises":surprises,"questions":questions}, indent=2), encoding="utf-8")
diagnostic = diagnose_extraction(merged, directed=True, root=ROOT)
(OUT / "diagnostics.json").write_text(json.dumps(diagnostic, indent=2), encoding="utf-8")
(OUT / "diagnostics.txt").write_text(format_diagnostic_report(diagnostic), encoding="utf-8")
with (OUT / "GRAPH_REPORT.md").open("a", encoding="utf-8") as stream:
    stream.write(f"\n## Graph health warning\n\n{diagnostic.get('dangling_endpoint_edges', 0)} raw edges name unresolved endpoints; {diagnostic.get('directed_same_endpoint_collapsed_edges', 0)} parallel endpoint edges collapse in the directed projection. The raw extraction and diagnostics are retained. Generic inferred indirect calls can connect unrelated same-name functions across the extension and desktop runtime; use verified source imports and current owner code for migration decisions. This graph is PARTIAL as a complete call graph.\n")
save_manifest(detection["files"], root=str(ROOT))
(OUT / "cost.json").write_text(json.dumps({"metered":False,"note":"Host subagent token usage unavailable. Zeros are placeholders.","runs":[{"date":"2026-10-16","input_tokens":0,"output_tokens":0,"files":detection["total_files"]}],"total_input_tokens":0,"total_output_tokens":0}, indent=2), encoding="utf-8")
print(f"Graph: {graph.number_of_nodes()} nodes, {graph.number_of_edges()} edges, {len(communities)} communities")
print(format_diagnostic_report(diagnostic))
