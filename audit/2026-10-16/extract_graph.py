import json
from pathlib import Path
from graphify.extract import extract

BASE = Path(__file__).resolve().parents[2]
ROOT = BASE / "upstream" / "chat-on-steroids"
OUT = BASE / "graphify-out"
if __name__ == "__main__":
    detection = json.loads((OUT / ".graphify_detect.json").read_text(encoding="utf-8"))
    result = extract([Path(f) for f in detection["files"]["code"]], cache_root=BASE, max_workers=4)
    (OUT / ".graphify_ast.json").write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"AST: {len(result['nodes'])} nodes, {len(result['edges'])} edges")
