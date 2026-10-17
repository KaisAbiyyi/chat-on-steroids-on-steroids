"""Account for uncompressed release payload using 7-Zip's static archive listing."""
import collections
import json
from pathlib import Path, PurePosixPath

OUT = Path(__file__).resolve().parent
raw = (OUT / "installer-archive-listing.txt").read_text(encoding="utf-8-sig")
if "----------" not in raw:
    raise ValueError("Missing 7-Zip entry section")
entries = []
for block in raw.split("----------", 1)[1].strip().split("\n\n"):
    record = {}
    for line in block.splitlines():
        if " = " in line:
            key, value = line.split(" = ", 1)
            record[key.strip()] = value.strip()
    if "Path" not in record or "Size" not in record:
        continue
    path = record["Path"].replace("\\", "/")
    parts = PurePosixPath(path).parts
    if PurePosixPath(path).is_absolute() or ".." in parts or ":" in path:
        raise ValueError("Unsafe archive member")
    if record.get("Attributes", "").startswith("D"):
        continue
    entries.append({"path": path, "bytes": int(record["Size"]), "packed_bytes": int(record.get("Packed Size", "0") or "0")})


def category(path):
    if path.startswith("resources/tunnel/"):
        return "tunnel_sidecars"
    if path.startswith("resources/rg/"):
        return "ripgrep"
    if path == "resources/app.asar":
        return "app_asar"
    if path.startswith("resources/app.asar.unpacked/"):
        return "native_node_modules"
    if path.startswith("resources/extension/"):
        return "extension"
    if path.startswith("resources/pets/"):
        return "pets"
    if path.startswith("resources/"):
        return "resource_notices_and_misc"
    return "electron_chromium_runtime"


groups = collections.defaultdict(lambda: {"files": 0, "bytes": 0, "packed_bytes": 0})
for entry in entries:
    entry["category"] = category(entry["path"])
    group = groups[entry["category"]]
    group["files"] += 1
    group["bytes"] += entry["bytes"]
    group["packed_bytes"] += entry["packed_bytes"]
total = sum(x["bytes"] for x in entries)
summary = {"release": "v2.1.39", "platform": "win32-x64", "installer_bytes": 173637457,
           "uncompressed_archive_payload_bytes": total, "payload_file_count": len(entries),
           "buckets": [{"category": k, **v, "share_percent": round(100 * v["bytes"] / total, 3)} for k, v in sorted(groups.items(), key=lambda item: -item[1]["bytes"])],
           "largest_files": sorted(entries, key=lambda x:-x["bytes"])[:25],
           "all_entries": entries,
           "notes": ["Uncompressed archive payload, not allocated installed disk space or runtime RAM.",
                     "7-Zip detects the app's embedded 7z and reports wrapper tail bytes; the complete installer SHA256 matches GitHub.",
                     "Buckets classify packaged files; Electron executable contains Chromium, Node and Electron together.",
                     "Packed totals may differ from installer size because headers, wrapper and compression layout are separate."]}
(OUT / "package-size-breakdown.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
print(json.dumps({k:summary[k] for k in ("uncompressed_archive_payload_bytes", "payload_file_count", "buckets", "largest_files")}, indent=2))
