import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const read = (p) => readFileSync(resolve(root, p), "utf8");
const manifest = JSON.parse(read("extension/manifest.json"));
const packageSpec = JSON.parse(read("renderer/package.json"));
const architecture = JSON.parse(read("../../docs/architecture/7p-a-feasibility-gates.json"));
assert.equal(manifest.manifest_version, 3);
assert.equal(manifest.background.service_worker, "sw.js");
assert.deepEqual(manifest.host_permissions, ["https://synthetic.cos.test/*"]);
assert.equal(manifest.content_scripts.length, 2);
assert.deepEqual(manifest.content_scripts.map(x => x.world).sort(), ["ISOLATED", "MAIN"]);
assert.ok(!manifest.permissions.includes("cookies"));
assert.ok(!manifest.permissions.includes("nativeMessaging"));
const worker = read("extension/sw.js");
assert.ok(worker.includes("chrome.debugger.getTargets"));
assert.ok(worker.includes("sender.tab") && worker.includes("senderUrl.startsWith(SYNTHETIC_ORIGIN)"),
  "debugger must be bound to its own synthetic sender");
assert.ok(worker.includes("t.tabId === tabId") && worker.includes("t.url.startsWith(SYNTHETIC_ORIGIN)"),
  "never attach a target without exact synthetic-origin ownership");
assert.ok(worker.includes("chrome.debugger.attach(") && worker.includes("chrome.debugger.detach("),
  "synthetic debugger must test both attach and detach");
assert.ok(worker.includes("Runtime.consoleAPICalled"),
  "synthetic debugger must check event arrival, not only API existence");
assert.ok(read("WindowsHost/Program.cs").includes("AllowSingleSignOnUsingOSPrimaryAccount = false"));
assert.ok(read("WindowsHost/Program.cs").includes("AreBrowserExtensionsEnabled = enabled"));
assert.ok(read("WindowsHost/Program.cs").includes("native7PL1Accepted = false"));
assert.ok(read("WindowsHost/Program.cs").includes("CoreWebView2HostResourceAccessKind.DenyCors"));
assert.ok(read("ProbePolicy/LeaseBook.cs").includes("_active.TryGetValue"));
for (const dep of ["codemirror", "katex", "pdfjs-dist", "@xterm/xterm"]) {
  assert.ok(packageSpec.dependencies[dep], "actual renderer dependency missing: " + dep);
}
assert.equal(architecture.decision, "PROVISIONAL");
assert.ok(architecture.capabilities.every(c => c.verification === "UNVERIFIED"),
  "7P-A must not be rewritten as a native PASS by cloud probes");
console.log("PASS: fail-closed 7P-B synthetic contract checks (not native browser tests)");
