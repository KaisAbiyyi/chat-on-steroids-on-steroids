# 8C — Native browser lifecycle and permission hardening (cloud contract)

Claim: [Issue #64](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/64). Base: accepted 8B PR #61, main `1ecc3ba388997d51eb2c31fe467431b3228e327e`. Lock: `browser-lifecycle`. Environment: **GitHub cloud only, local file access blocked**.

## Security boundary

`browser-lifecycle.mjs` is a **host-only, synthetic JavaScript integration contract**. It must be constructed only by an authenticated native Windows/WebView2 host, never exposed through page scripts, extension MAIN/ISOLATED worlds, IPC, untrusted `runtime.onMessage`, or renderer permissions. The module does **not** implement actual WebView2 permission handlers, process events, storage partitions or persistent recovery. No real account session/cookie/token access is performed or claimed.

The native host provides four independently privileged callbacks:

1. `authorizeNativeTarget(nativeProof)`: attest the exact eleven-field 8A/8B tab/frame/document/profile/session/epoch/host-generation identity. Proofs are opaque privileged handles, not page JSON or tab IDs.
2. `resolveNativePartition(nativeProof)`: attest an exclusive 8B native storage partition for that profile. Partition IDs are **opaque handles**, never filesystem paths; the native host independently owns partition isolation.
3. `verifyNativeLifecycleEvent(eventProof)`: authenticate `{kind, sequence, before}` from the native host's source/event journal. An untrusted page or extension must never mint native event proofs.
4. `authorizeNativePermission({nativeProof,consentProof,capability,expectedContext,expectedPartition})`: **atomically** validate latest document/profile, origin, partition and one-use explicit owner consent *before* issuing any underlying browser permission, and return exactly `{granted:true}` only on a completed grant. Consume consent at the native boundary, not just in JavaScript. An uncertain result is not an allow and must never be blindly retried.

`track({nativeProof})` registers an initially live scope after double attestation; each scope is tied to a profile, tab, frame, document, origin and native partition. It refuses identity drift, partition rebound/collision and excess scopes. `transition({eventProof,nativeProof?})` handles authenticated navigation, crash, close, host restart and explicit rebind. All registered tabs become suspended after a host restart, not merely the reporting tab. Rebinding demands a fresh host generation, increased navigation epoch, new document, same trusted origin/session/partition; a changed application session must be handed off via **8B**, never inferred as a browser lifecycle event.

`requestPermission({nativeProof,capability,consentProof})` denies by default, serializes by frame scope, cross-checks live host context/partition and state before and after native consent/grant. Capabilities are narrowly enumerated: camera, microphone, geolocation, notifications, clipboard-read, clipboard-write and downloads. Debugger, cookies, arbitrary JavaScript, Send, arbitrary external schemes and profile storage migration are **not** exposed. On unexpected results, errors, changed documents, crash, close, restart, duplicate attempts or uncertain grant it fails closed. The native code **must** enforce atomicity across its actual permission side effect; postchecks in JS cannot undo a permission already granted by a buggy host.

The controller is ephemeral and never writes credentials, cookies, logs containing consentProof, disk state, or partition paths. State is bounded (256 frame scopes, 64 partitions). A new host process must construct a new controller and reattest every scope; it must never restore a previous permission grant from this in-memory controller.

## Cloud validation

```sh
node --check extension/adapter/browser-lifecycle.mjs
node --check extension/adapter/tests/browser-lifecycle.test.mjs
node --test extension/adapter/tests/browser-lifecycle.test.mjs
node --test extension/adapter/tests/profile-session.test.mjs
node --test extension/adapter/tests/chrome-worker.test.mjs
node --test extension/companion/tests/ledger.test.mjs
node --test extension/companion/tests/transport-differential.test.mjs
```

`.github/workflows/8c-browser-lifecycle.yml` runs Node 22 synthetic regressions on both Windows/Ubuntu; Rust CI and Task DAG remain separately required. Review the exact PR-head source diff, native authority delegation, default-deny path, life-cycle interleavings, grant atomicity, cross-profile isolation, and all exact-head check statuses. Do not claim PASS until green checks are evidenced. No unrelated Rust native implementation is claimed by 8C.

**NOT_RUN / deliberately separate gates:** real Windows WebView2 process/restart/multi-tab/profile persistence and live permission handler tests (8L1); owner/provider sign-in and consent UX (8U1). Synthetic CI is not these physical/owner acceptance receipts. 7L2 browser-debugger native gap is a separate tracked dependency and cannot be certified by this task.
