# 8A — Chrome API / extension worker world (cloud implementation)

The `chrome-worker.mjs` adapter connects an externally authenticated **7B/7L1 native host** to the existing **7C claim/ACK journal** and to the MV3 `chrome.scripting` API. This is a bounded cloud adapter contract; it is **not** a real WebView2 profile/sign-in implementation or a browser extension install proof.

## Integration

The privileged host injects all four dependencies: `subscribeVerifiedNativeEvent` (native-authenticated event subscription), `verifyHostEvent` (native 7B verification for the 7C ledger), `authorizeNativeTarget` (independently verifies current host ownership and returns exactly the eleven context keys, including exact allowed origin), and `allowedOrigins` (explicit HTTPS origin allowlist, maximum 16). Never construct proofs using page JSON, `runtime.onMessage`, tab ID alone, `sender.url`, or the results of a scripting operation. The 7B host must fence concurrent MV3 worker instances. The adapter supplies no key, credential or native authority of its own.

`inject({nativeProof, world:"ISOLATED" | "MAIN"})` injects only fixed, audited, **inert** files under `adapter/world/`, with `target.documentIds` (not `frameIds`), and cross-checks authenticated native context, tab URL, active frame document ID, frame URL, and generation both before and after the browser API call. Chrome validates frameId and documentId in the getFrame REQUEST; the response does not promise a frameId property. Distinct tab/profile ownership is not inferred from JavaScript. Browser APIs that cannot preserve the required document binding fail closed. A same-document injection is allowed more than once; concurrent injections in the same profile/tab/frame are refused. An unsuccessful or uncertain injection yields no success receipt. The checked `documentId` must be surfaced in a Chrome/WebView2 implementation's `InjectionResult` before reporting success.

The two shipped scripts do nothing and deliberately communicate no data between MAIN and ISOLATED. The future production integration must introduce provider-specific, separately reviewed, origin-bound capabilities, not general arbitrary-script injection or page-mediated native RPC. No `chrome.debugger.attach` is performed here: exclusive authenticated debugging belongs to the 7D native `DirectBrowserTools` layer. No Send, clipboard, cookies, OAuth, profile transfer or user action occurs.

## Cloud tests

```sh
node --check extension/adapter/chrome-worker.mjs
node --test extension/adapter/tests/chrome-worker.test.mjs
node --test extension/companion/tests/ledger.test.mjs
```

Dedicated `8a-chrome-adapter.yml` runs Windows and Ubuntu plus Rust/Task DAG CI on every PR. This synthetic browser API harness tests navigation races, stale attestations, cross-origin rejection, malformed identity, callback lastError, Promise APIs, worker-store claim/ACK and privilege boundaries. Real WebView2/chrome.scripting parity, browser extension install, physical worker restart, native profile isolation and owner sign-in remain **NOT RUN** and belong to later explicit native/owner gates. A post-injection check detects uncertain navigation but cannot undo already-executed JavaScript, hence **inert** audited scripts are mandatory.
