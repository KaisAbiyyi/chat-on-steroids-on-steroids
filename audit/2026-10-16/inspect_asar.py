import collections
import json
import struct
from pathlib import Path

OUT = Path(__file__).resolve().parent
path = OUT / "release-artifact" / "extracted" / "resources" / "app.asar"
with path.open("rb") as stream:
    prefix = stream.read(16)
    header_sizes = struct.unpack("<4I", prefix)
    if header_sizes[0] != 4 or not 0 < header_sizes[3] < 100 * 1024 * 1024:
        raise ValueError("Unexpected ASAR header")
    header = json.loads(stream.read(header_sizes[3]))
entries = []


def walk(tree, prefix=""):
    for name, entry in tree.get("files", {}).items():
        rel = f"{prefix}/{name}".lstrip("/")
        if "files" in entry:
            walk(entry, rel)
        elif "size" in entry:
            entries.append({"path": rel, "bytes": entry["size"], "unpacked": entry.get("unpacked", False)})


walk(header)
groups = collections.defaultdict(lambda: {"files": 0, "packed_file_bytes": 0, "unpacked_file_bytes": 0})
packages = collections.defaultdict(int)
for entry in entries:
    rel = entry["path"]
    category = "node_modules" if rel.startswith("node_modules/") else "/".join(rel.split("/")[:2])
    groups[category]["files"] += 1
    groups[category]["unpacked_file_bytes" if entry["unpacked"] else "packed_file_bytes"] += entry["bytes"]
    if rel.startswith("node_modules/"):
        segments = rel.split("/")
        package = "/".join(segments[1:3]) if segments[1].startswith("@") else segments[1]
        packages[package] += entry["bytes"]
summary = {"asar_file_bytes": path.stat().st_size, "header_sizes": header_sizes,
           "entry_count": len(entries), "packed_content_bytes": sum(e["bytes"] for e in entries if not e["unpacked"]),
           "unpacked_content_bytes_referenced": sum(e["bytes"] for e in entries if e["unpacked"]),
           "groups": dict(groups), "largest_dependency_trees_bytes_including_unpacked": sorted(packages.items(), key=lambda kv:-kv[1])[:25],
           "largest_packed_files": sorted([e for e in entries if not e["unpacked"]], key=lambda e:-e["bytes"])[:25],
           "notes": ["Unpacked entries are references to separately counted app.asar.unpacked; do not add them to the ASAR file size.",
                     "Dependency tree totals are packaged bytes, not npm registry unpackedSize or source size."]}
(OUT / "asar-size-breakdown.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
print(json.dumps(summary, indent=2))
