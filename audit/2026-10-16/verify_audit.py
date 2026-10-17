"""Fresh verification of this investigation; never starts or installs the original app."""
import ast
import csv
import hashlib
import json
import re
import subprocess
from pathlib import Path

OUT = Path(__file__).resolve().parent
BASE = OUT.parents[1]
ROOT = BASE / "upstream/chat-on-steroids"
checks = []


def record(name, operation):
    try:
        detail = operation()
        checks.append({"check":name,"status":"PASS","detail":detail})
    except Exception as error:
        checks.append({"check":name,"status":"FAIL","detail":str(error)})


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def command(args, cwd=ROOT):
    result = subprocess.run(args,cwd=cwd,capture_output=True,text=True,encoding="utf-8",errors="replace")
    require(result.returncode == 0, f"{args}: {result.stderr or result.stdout}")
    return result.stdout.strip()


def file_hash(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for data in iter(lambda:stream.read(1024*1024),b""):
            value.update(data)
    return value.hexdigest()


def identity():
    value = read_json(OUT / "source-identity.json")
    require(command(["git","rev-parse","HEAD"]) == value["head"],"HEAD changed")
    require(value["head"] == "024731d8083d92c46e8248d64e7e0b36ab0a470f","Unexpected pinned commit")
    require(command(["git","status","--porcelain=v1"]) == "","Upstream clone has untracked or modified files")
    require(value["working_tree_status"] == "","Identity receipt predates clean state")
    return {"head":value["head"],"working_tree":"clean"}


def file_coverage():
    inventory = list(csv.DictReader((OUT / "tracked-files.csv").open(encoding="utf-8")))
    mapped = list(csv.DictReader((OUT / "migration-file-map.csv").open(encoding="utf-8")))
    tracked = command(["git","ls-files","-z"]).split("\0")
    tracked = {x for x in tracked if x}
    require(len(inventory)==1442 and len(mapped)==1442,"Wrong tracked/map count")
    require({x["path"] for x in inventory} == tracked == {x["path"] for x in mapped},"Coverage mismatch")
    for row in inventory:
        path = ROOT / row["path"]
        require(path.is_file() and path.stat().st_size == int(row["bytes"]),f"File size changed: {row['path']}")
        require(file_hash(path) == row["sha256"],f"File hash changed: {row['path']}")
    phases = read_json(OUT / "migration-phase-map.json")
    require(phases["phase_count"]==16 and phases["product_file_count"]==539,"Wrong phase/product count")
    require([x["phase"] for x in phases["phases"]] == list(range(1,17)),"Wrong phase IDs")
    require(not phases["unassigned_files"],"Unassigned files")
    for phase in phases["phases"]:
        require(all(dep < phase["phase"] for dep in phase["depends_on"]),"Cyclic or forward phase dependency")
        require(phase["tracked_files_owned"] == sum(int(x["phase"]) == phase["phase"] for x in mapped),"Phase count mismatch")
    return {"tracked_files":1442,"all_hashes_match":True,"product_files":539,"phases":16,"unassigned":0}


def package_accounting():
    verified = read_json(OUT / "release-artifact-verification.json")
    release = read_json(OUT / "github-latest-release.json")
    asset = next(x for x in release["assets"] if x["name"]=="Chat-On-Steroids-Setup-x64.exe")
    installer = Path(verified["file"])
    require(installer.stat().st_size==asset["size"]==173637457,"Installer size mismatch")
    require("sha256:"+file_hash(installer)==asset["digest"],"Installer digest mismatch")
    package = read_json(OUT / "package-size-breakdown.json")
    asar = read_json(OUT / "asar-size-breakdown.json")
    require(sum(x["bytes"] for x in package["buckets"])==package["uncompressed_archive_payload_bytes"]==607145786,"Payload bucket mismatch")
    require(sum(x["bytes"] for x in package["all_entries"])==607145786,"Payload entries mismatch")
    by_name = {x["category"]:x["bytes"] for x in package["buckets"]}
    require(asar["asar_file_bytes"]==by_name["app_asar"],"ASAR size mismatch")
    require(asar["unpacked_content_bytes_referenced"]==by_name["native_node_modules"],"Unpacked reference mismatch")
    require(8 + asar["header_sizes"][1] + asar["packed_content_bytes"] == asar["asar_file_bytes"],"ASAR header/content accounting mismatch")
    require(not verified["executed_or_installed"],"Unexpected installer execution claim")
    return {"installer_bytes":173637457,"payload_bytes":607145786,"sha256":verified["sha256"],"asar_accounting":"consistent"}


def syntax():
    scripts = list(OUT.glob("*.py"))
    for path in scripts:
        ast.parse(path.read_text(encoding="utf-8"),filename=str(path))
    extension = sorted((ROOT / "extension").glob("*.js"))
    for path in extension:
        command(["node","--check",str(path)])
    return {"analysis_python_scripts":len(scripts),"extension_js_files":len(extension),"scope":"syntax only"}


def data_and_doc_links():
    parsed = 0
    for path in OUT.glob("*.json"):
        read_json(path)
        parsed += 1
    docs = [BASE / "README.md", *sorted((BASE / "docs").rglob("*.md"))]
    links = 0
    for path in docs:
        text = path.read_text(encoding="utf-8")
        for href in re.findall(r"\[[^\]]*\]\(([^)]+)\)",text):
            href = href.strip().strip("<>")
            if re.match(r"(?:https?://|#|mailto:)",href):
                continue
            target = (path.parent / href.split("#",1)[0]).resolve()
            if target == OUT / "verification.json":
                continue
            require(target.exists(),f"Broken local link in {path.name}: {href}")
            links += 1
    return {"parsed_audit_json":parsed,"documents":len(docs),"existing_local_links":links}


def semantic_and_graph():
    graph_dir = BASE / "graphify-out"
    sem = read_json(graph_dir / "semantic-docs.json")
    node_ids = {x["id"] for x in sem["nodes"]}
    require(len(node_ids)==len(sem["nodes"]),"Duplicate semantic node IDs")
    sources = set()
    for node in sem["nodes"]:
        path = Path(node["source_file"])
        require(path.is_file(),"Missing semantic source")
        lines = len(path.read_text(encoding="utf-8").splitlines())
        for number in re.findall(r"L(\d+)",node.get("source_location") or ""):
            require(1<=int(number)<=lines,"Semantic line outside file")
        sources.add(node["source_file"])
    for edge in sem["edges"]:
        require(edge["source"] in node_ids and edge["target"] in node_ids,"Dangling semantic edge")
        require(0<float(edge["confidence_score"])<=1,"Missing/bad semantic confidence")
    require(len(sources)==20,"Semantic document coverage mismatch")
    graph = read_json(graph_dir / "graph.json")
    diag = read_json(graph_dir / "diagnostics.json")
    require(len(graph["nodes"])==diag["post_build_node_count"],"Graph node count mismatch")
    require((graph_dir / "graph.html").stat().st_size > 1000,"Missing interactive HTML")
    require("unmetered" in (graph_dir / "GRAPH_REPORT.md").read_text(encoding="utf-8"),"Token metering limit not disclosed")
    report = (BASE / "docs/research/2026-10-16-chat-on-steroids-deep-scan-v1.0.md").read_text(encoding="utf-8")
    require(str(diag["dangling_endpoint_edges"]) in report and str(diag["directed_same_endpoint_collapsed_edges"]) in report,"Graph limits differ from report")
    return {"semantic_documents":20,"semantic_nodes":len(sem["nodes"]),"graph_nodes":len(graph["nodes"]),"graph_integrity":"PARTIAL; disclosed unresolved and collapsed edges"}


record("pinned_source_identity_and_clean_clone",identity)
record("all_tracked_files_hashes_and_phase_coverage",file_coverage)
record("published_release_and_payload_accounting",package_accounting)
record("analysis_and_extension_syntax",syntax)
record("audit_json_and_local_artifact_links",data_and_doc_links)
record("semantic_sources_and_graph_disclosures",semantic_and_graph)
record("native_source_pin_inventory",lambda:command(["node","scripts/package-native-sources.mjs","--check"]))
result = {"date":"2026-10-16","timezone":"Asia/Jakarta","investigation_checks":"PASS" if all(x["status"]=="PASS" for x in checks) else "FAIL", "checks":checks,
          "graph_as_complete_call_graph":"PARTIAL", "original_app_test_suite":"NOT RUN", "original_app_build":"NOT RUN", "live_browser_or_desktop":"NOT RUN",
          "runtime_memory_and_latency":"NOT RUN", "rust_product_implementation":"NOT STARTED", "rust_parity_acceptance":"NOT RUN"}
(OUT / "verification.json").write_text(json.dumps(result,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
print(json.dumps(result,indent=2,ensure_ascii=False))
raise SystemExit(0 if result["investigation_checks"]=="PASS" else 1)
