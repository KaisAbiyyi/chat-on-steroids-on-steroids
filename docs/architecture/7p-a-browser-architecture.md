# 7P-A — Embedded browser architecture feasibility (cloud preflight)

**Decision state:** PROVISIONAL architecture selected; not a device/browser compatibility PASS.  
**Date:** 2026-10-22 (Asia/Jakarta). **Source baseline:** `39cfa042478e33acfff41e27f3a41428042f0c03`; after native backend `CL-L6` accepted in [#39](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/39).  
**Execution:** `7P-A` cloud only, exclusive resource `browser-architecture`; no machine-local work, native device, real profile or sign-in touched.  
**Machine-checkable contract:** [7p-a-feasibility-gates.json](7p-a-feasibility-gates.json) and [CI validator](../../tools/check_browser_feasibility.py).

## Source ownership and hard constraints

The [pinned source scan](../research/2026-10-16-chat-on-steroids-deep-scan-v1.0.md) records Electron + Chromium at **385.17 MB / 63.44%** of the analyzed app payload, bridge protocol 14, and native/browser responsibilities: `bridge.ts`, extension `background.js`, `content.js`, `fiber.js`, built-in `cos-browser/host.ts`, `chrome-api.ts`, `extension-worker.ts`, `tab-model.ts`. Runtime must retain authenticated browser communication, epoch-fenced election, command/receipt/ACK sequencing, debugger ownership and no duplicate user input. UI requires rich editor/code/PDF/math/terminal rather than a plaintext approximation. These source observations are from the *committed deep-scan report*, not newly executed upstream probes.

Original [16-phase gate](../implementation-plan/2026-10-16-rust-migration-phases-v1.0.md) requires browser and UI feasibility before the bridge rewrite. [Cloud unit matrix](../implementation-plan/2026-10-18-cloud-first-units.md) now makes the order explicit: `CL-L6 → 7P-A → 7P-B → 7P-L1 → 7A`. Real extension/host evidence is **not** a cloud-only acceptance for `7P-A`.

## Architecture decision: Windows-first WebView2 hybrid, conditional

1. **Rust owns** app/service authorization, tab/profile/document identity, lifetime and CDP target ownership, and the backend bridge. A host-specific adapter supplies WebView2 environment/profile and CDP capabilities; the core must never blindly treat browser-origin JS as local authority.
2. **Windows embedded content** provisionally uses the system **Evergreen WebView2 runtime**, ideally through a thin Wry/Tauri 2 or native WebView2 binding where direct extension/CDP/target access is necessary. Runtime availability/version is checked; shipping fixed WebView2 or CEF is *not* a free distribution-size win.
3. **Renderer** retains web/TypeScript/JavaScript for the feature-equivalent rich editor, KaTeX, PDF.js, xterm and their event/IME/accessibility behavior. This is an **explicit non-Rust deviation**, not a false claim of 100% Rust. Native Rust UI is not selected without screenshot, interaction and feature-parity proof.
4. **Companion extension** retains the minimal packaged MV3 JavaScript glue demanded by browser/DOM APIs; Rust/Wasm can own only tested portable logic. Do not replace content-script MAIN/ISOLATED world and MV3 lifecycle semantics on assumption. The upstream companion is an oracle; preserving its exact behavior remains a separate implementation task.
5. **Profile isolation:** app-owned synthetic profiles and an isolated user-data location. Absolutely no copying an installed personal Edge/Chrome profile, no use of authenticated provider cookies and no signed-in user flows in this preflight.
6. **Other platforms:** macOS/Linux remain in the total product scope but native proof is deferred per project policy. Windows success cannot be extrapolated to WKWebView/WebKitGTK or Chrome extension parity elsewhere.

This is a **conditional design choice**, not final engine certification. Microsoft provides [unpacked extension install on WebView2 profiles](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2profile7?view=webview2-1.0.4258.31), but documents missing browser-UI extension entry points. The [environment option is disabled by default](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2environmentoptions6?view=webview2-1.0.2849.39), and [WebView2 is not identical to the Edge browser](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/browser-features). Its [host CDP API](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2?view=webview2-1.0.2651.64) does **not** prove the Chrome extension [`chrome.debugger` API](https://developer.chrome.com/docs/extensions/reference/api/debugger) behaves identically. The official [MV3 service-worker lifecycle](https://developer.chrome.com/docs/extensions/develop/concepts/service-workers/lifecycle) is discontinuous and must be exercised after worker sleep/restart. The [Tauri WebView2 settings](https://v2.tauri.app/reference/config/) are a wrapper capability, not proof that the original CoSoS extension and renderer run correctly.

## Candidate comparison and pivot condition

| Candidate | Browser/UI potential | Critical unknown | Decision |
| --- | --- | --- | --- |
| Rust host + shared WebView2 + existing web renderer | Windows shared engine, unpacked extension and host CDP documented | Actual `chrome.debugger`, background MV3, MAIN/ISOLATED worlds, tab identity, provider sign-in | **PROVISIONAL** — first spike |
| Rust host + CEF/custom Chromium runtime | Potential for more Chromium embedding surface | Browser API shims, binary footprint, maintenance/security patching and license details | **Fallback; NOT VERIFIED** |
| External browser alone | Low embedded-bundle complexity | Not a replacement for owned built-in tabs/profiles/sign-in | **Reject as silent parity reduction** |
| Rust-only GUI without web engine | Rust-native UI ownership | Cannot load actual provider pages or extension DOM worlds | **Reject as browser replacement** |

**Mandatory pivot:** if any *hard blocker* in the JSON matrix fails and there is no strictly parity-preserving, demonstrable adapter, mark `7P-L1 FAIL`, hold `7A` and downstream browser phases, and open a new reviewed architecture decision. Do not invent extension behavior, disable debugger workflows, or quietly launch an external browser to simulate PASS.

## Executable acceptance ledger

Every matrix capability is intentionally `UNVERIFIED` with an explicit observer and first test stage. Static/API documentation is **documentation-only evidence**. `7P-A` cloud regression checks assert no unmeasured capability is recorded as passed. Future synthetic `7P-B` harness reports must list exact executable, SDK/runtime, OS runner, source SHA, fixture, raw check and exact failure outputs. Real `7P-L1` must reproduce on the user's isolated physical Windows host, record runtime/extension version, attach/detach and MV3 suspension, tab/profile boundaries, renderer interaction and resource measurements, and link independently inspectable SHA-bound logs.

**7P-B cloud handoff — implement a minimal host and differential probe, not the product bridge:**
- Enable extension option before creating environment; load unpacked synthetic MV3 package and detect explicit feature/installation errors.
- Prove service worker/background event and MAIN vs ISOLATED script behavior on *synthetic pages*, with browser CSP; require negative tests for unsupported APIs and privilege separation.
- Test host CDP event ordering and per-tab/frame/worker targeting against actual `chrome.debugger` calls/semantics; no silent substitute.
- Probe two named synthetic profiles, duplicate tab election and stale-target rejection, and build renderer interaction fixtures for editor/PDF/math/terminal.
- Produce run receipts and a crisp READY/STOP verdict for isolated real Windows `7P-L1`.

**7P-L1 native handoff:** user-initiated separate local gate; no real signed-in account or private profile. Verify installed host API/runtime, MV3 lifecycle, CDP ownership, restart/tab/profile separation, refusal cases, renderer quality, and measured synthetic-probe installer/payload, cold/warm full-batch runtime and sampled RAM on D: only. Output `PASS/FAIL/BLOCKED` for each critical gate with the separate native qualifier receipt; a raw synthetic host's `NOT_RUN` umbrella flags do not themselves decide physical acceptance. Cloud CI must never mark this native gate PASS. Candidate probe size limits are feasibility targets, not approved production app budgets.

**Excluded until future gates:** signed-in provider authentication (`7U1` and `8U1` as scheduled), real-account consent, automatic Send, complete Phase 7 protocol 14 bridge, Phase 8 tab/profile product parity, **production** packaged installer/signing, cold-start-to-interactive and private-memory release qualification (`16L3`) and macOS/Linux native acceptance.

## Cloud review checklist

- Source expectations and JS exception explicitly documented; no unsupported "100% Rust" or native-verified claims.
- All mandatory capabilities unique, fail-closed, source-linked and owned, with first-probe and native gating.
- Regression tests reject missing, duplicate, falsely verified and ungrounded capabilities. CI runs on Windows/Ubuntu.
- Dependency DAG stays intact: no `7A` while `7P-L1` unfinished.
- Exact PR HEAD tested, independent review documented; no claim that a coding agent is third-party security accreditation.
