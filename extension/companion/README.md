# 7C — Extension companion claim / ACK / repair (cloud contract)

Cloud-scoped, standalone ES modules for a future **privileged MV3 extension adapter**. This is not a running browser feature, signed-in provider workflow or real extension installation.

## Contract

- `ledger.mjs` owns a bounded (1024) per-session `chrome.storage.session` journal of exact browser-channel provenance and intent claim/ACK state.
- The consumer MUST provide an async `verifyHostEvent(nativeProof, event)` which independently validates the **7B** authenticated native channel, original sender, bound origin/profile/tab/frame/document/session and channel/worker generation. No page-supplied identity is trusted. The module rejects unknown protocol versions, extra event fields, invalid IDs/digests, wrong sequence, ownership changes, stale generations, duplicate/conflicting receipts and unsafe after-retire resurrection.
- The caller invokes `dispatch(event, nativeProof)` with a version-14 bounded envelope (kind `attach`, `claim`, `ack`, `repair`, `retire`). The binding is intentionally **not** `chrome.runtime.onMessage` or an unauthenticated content script. Event authorizations of the actual 7B adapter and provider-specific intent digest/wire mapping remain future integration work.
- `claim` reserves an opaque intent ID plus fixed SHA-256 digest. This module **never executes that intent**, never sends input, and never automatically retries. `ack` binds the exact claim incarnation and receipt. `repair` only returns durable status and `resendAllowed: false` even after worker restart; an old claim is ambiguous until native reconciliation.
- Every mutation is serialized per worker instance and persisted **before success is returned**. `onSuspend` does no asynchronous flush; MV3 may stop at any time. Resuming requires new native host generation and a fresh verified channel. Retired generation tombstones are persistent to refuse old-worker re-election. Browser shutdown and multiple concurrent worker-process writes are **not** proven; the host must fence native generations across workers and supply at-most-one writer.

## Test

```sh
node --check extension/companion/ledger.mjs
node --check extension/companion/worker.mjs
node --test extension/companion/tests/ledger.test.mjs
```

GitHub Actions workflow `7c-companion.yml` executes on Windows and Ubuntu. Task DAG and Rust CI remain mandatory before merging.

## Acceptance boundary

This task accepts only source and **synthetic cloud** tests for the extension-side state machine and privileged adapter contract. It does **not** prove real MV3 worker suspension, WebView2 extension loading, native 7B proof delivery, upstream byte wire parity, true provider origin or session authority, debugger, real user action, installer qualification or `7P-L1` physical Windows PASS. Specifically, `7L1` is **blocked** until both `7C` is accepted and the separate held `7P-L1` has passed on a real authorized isolated Windows host. `7D` also waits for `7L1`.
