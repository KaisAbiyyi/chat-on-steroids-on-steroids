# 7U1-PREP production interface gap — 2026-10-27

**Disposition: BLOCKED.** This document records a source audit, not runtime acceptance. It deliberately does not convert the synthetic reference adapter into a production runner.

## Audited source baseline

- 7U1-PREP HEAD before this evidence update: `7148ae1ef5eea8810f50dc35ba69f50f7b5bd011`.
- Merged 8P-HOST dependency: `6d92ce89edcb2a9afa42143d0cf0bea6b334475a`.
- Scope remains 7U1-PREP only. Do not change the provider-policy or IPC behavior owned by 8P-HOST #124.
- Owner gate #91 remains `BLOCKED / NOT_RUN`; no provider account, credentials, cookies, tokens, simulated consent, or Send were used.

## What is present

- `crates/cos-core/src/browser_tools.rs` contains `DirectBrowserTools` with native debugger attach, bounded snapshot validation, detach, and lifecycle retirement.
- `crates/cos-core/examples/7l2_bridge.rs` exposes `ToolAttach`, `ToolSnapshot`, and `ToolDetach` requests over the private host-owned pipe.
- The production 8P host already binds a fresh WebView2 profile, verifies the installed MV3 worker target, authenticates a Rust channel, and revokes/retires channels on lifecycle changes.
- The existing synthetic 7L2 host has a C# CDP adapter for the Rust direct tools, but it is not a production-provider adapter.

## Concrete production integration gaps

1. **Native C# API absent.** `apps/windows-host/Native8P.cs::Prove8PSessionAsync` calls MV3 `attestSession` and Rust `observe` for the first-proof only. It does not expose a source-owned read-only `ObserveOnlyAttachAsync`, `ObserveOnlySnapshotAsync`, `ObserveOnlyDetachAsync`, or `RevokeObserveOnlyLease` contract.
2. **Production MV3 tool authorization absent.** `probes/8p/extension/sw.mjs` exposes bootstrap/session attestation only. It has no `authorizeTool` / `confirmToolAuthorization` sequence for the Rust `CoSoS/7B/authorization/v1` domain. The separate 7L1 worker is not interchangeable with the 8P worker.
3. **Rust tools are not wired to production WebView2.** `probes/7l1/WindowsHost/Program.cs::RustAsync` dispatches a `cdpCallback` to `Run7L2CdpAsync`. That handler requires the synthetic `https://synthetic.cos.test` origin and exact synthetic `StartUrl`; it cannot safely service a production 8P provider document. Rust core tool availability alone is not integration evidence.
4. **Signed-in owner navigation is not qualified.** The current 8P navigation policy cancels OAuth callback candidates (the code path explicitly says it will not load the code-bearing page). This task must not change #124's provider-policy/IPC behavior. A separately authorized and reviewed 7U1 entrypoint/contract is needed before a signed-in owner runner can be accepted.
5. **Source provenance needed one more input.** The production artifact copied the bridge example and native bridge, but omitted `crates/cos-core/src/browser_tools.rs`, which contains the Rust direct-tool implementation being audited. The artifact source map and workflow must include it.

## Required next implementation before 7U1-PREP can pass

- Add an isolated, explicit-consent 7U1 production entrypoint; do not repurpose the synthetic 7L2 handler.
- Wire native C# attach/snapshot/detach/revoke to the exact production WebView2 profile, tab, document, origin, host generation, and MV3 worker identity.
- Add domain-separated MV3 tool authorization and monotonic sequence confirmation for each operation; do not expose tool authority to page messaging.
- Reuse the Rust direct-tool core only through the private host pipe, with bounded operation deadlines and a 65,536-byte snapshot cap.
- Preserve synchronous lifecycle revocation and fail closed on attach ambiguity, invalid snapshot, source drift, stale/replayed authority, or detach/revoke failure.
- Test missing integration, source drift, attach/snapshot/detach failures, navigation/rebind/crash/worker retirement, and timeout behavior on Windows and Ubuntu. Any static marker audit remains review evidence, not runtime proof.

## Current disposition

- Production interface audit: **BLOCKED**.
- Synthetic preflight/artifact: **BLOCKED for production 7U1**, even when its manifest checks pass.
- Owner gate #91: **BLOCKED / NOT_RUN**.
- PR #128: keep Draft/open/unmerged pending the isolated production integration and independent security review.
