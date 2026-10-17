import collections
import json
import re
from pathlib import Path

BASE = Path(__file__).resolve().parents[2]
ROOT = BASE / "upstream" / "chat-on-steroids"
OUT = Path(__file__).resolve().parent
ipc = []
state = []
pattern = re.compile(r"(?:\bipcMain\s*\.\s*handle|(?<![\w.])handle)\s*\(\s*['\"]([^'\"]+)['\"]")
for path in (ROOT / "src/main").rglob("*.ts"):
    text = path.read_text(encoding="utf-8")
    rel = path.relative_to(ROOT).as_posix()
    for match in pattern.finditer(text):
        ipc.append({"path":rel,"line":text.count("\n",0,match.start())+1,"channel":match.group(1)})
    consts = dict(re.findall(r"(?:const|let)\s+(\w+)\s*=\s*['\"]([a-z0-9-]+)['\"]", text))
    for match in re.finditer(r"\b(readDurable|writeDurableNow|writeDurableSoon|writeDurableSnapshotSoon)(?:<[^>]+>)?\s*\(\s*(['\"][a-z0-9-]+['\"]|[A-Z][A-Z0-9_]+)\s*[,)]",text):
        name = match.group(2)
        resolved = name[1:-1] if name[0] in "'\"" else consts.get(name)
        state.append({"path":rel,"line":text.count("\n",0,match.start())+1,"operation":match.group(1),"ledger":resolved,"literal_or_constant":name})
types = (ROOT / "src/shared/types.ts").read_text(encoding="utf-8")
cap = re.search(r"export const CAPABILITIES = \[([\s\S]+?)\]",types).group(1)
langs = (ROOT / "src/shared/ui-language.ts").read_text(encoding="utf-8")
core = (ROOT / "src/main/mcp/surfaces.ts").read_text(encoding="utf-8")
core_tools = re.search(r"tools: \[([^\]]+)\]",core).group(1)
limits = (ROOT / "src/main/mcp/code-mode-runtime.ts").read_text(encoding="utf-8")
limits = re.search(r"export const CODE_MODE_LIMITS = Object.freeze\(\{([\s\S]+?)\}\);",limits).group(1).strip()
imports = json.loads((OUT / "imports.json").read_text(encoding="utf-8"))
fanin = collections.Counter(x["resolved"] for x in imports if x.get("resolved") and x["path"].startswith(("src/", "extension/", "native/")))
result = {"bridge_protocol":14,"capabilities":re.findall(r"'([^']+)'",cap),"ui_languages":re.findall(r"'([^']+)'",langs.split("as const")[0]),
          "core_declared_tool_union":re.findall(r"'([^']+)'",core_tools),"ipc_handler_declarations":ipc,"unique_ipc_handler_channels":sorted(set(x["channel"] for x in ipc)),
          "durable_ledger_calls":state,"unique_literal_durable_ledgers":sorted(set(x["ledger"] for x in state if x["ledger"])),
          "code_mode_limits_source":limits,"product_only_internal_import_fan_in":fanin.most_common(25),
          "notes":["Declarations are static regex extractions, not runtime tools/list or network conformance.","Wrapped handle() calls are included; other registration wrappers and dynamic routes can remain absent.","State ledger list excludes computed names not resolvable from a local string constant."]}
(OUT / "contracts.json").write_text(json.dumps(result, indent=2) + "\n",encoding="utf-8")
print(json.dumps({"capabilities":len(result["capabilities"]),"ui_languages":len(result["ui_languages"]),"literal_ipc_channels":len(result["unique_ipc_handler_channels"]),"literal_durable_ledgers":len(result["unique_literal_durable_ledgers"]),"top_product_fan_in":result["product_only_internal_import_fan_in"][:10]},indent=2))
