# 7B — Native authenticated bridge authority (cloud unit)

**Date:** 2026-10-22 (Asia/Jakarta). **Agent:** ChatGPT GPT-6 (GitHub Cloud only).  
**PR:** [#45](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/45) · **base:** accepted 7A merge `505a5c9d7c1851e4e2e4182ac2c8521c5a58f968`.  
**Exclusive resource:** `browser-bridge`. **Owner-authenticated physical Windows:** NOT RUN.

## Native Rust implementation (bounded 7B)

`crates/cos-core/src/native_browser_bridge.rs` implements a host-attested authority on top of the versioned `IdentityFence` from 7A:

1. **Trusted host evidence:** profile/tab/frame, normalized document/session/worker identity, exact HTTPS origin and monotonic native host generation must come from a native browser callback. Renderer, page URL or content script cannot manufacture host evidence through this API. Reject untrusted origins, weak/missing generations and identity mismatch.
2. **Explicit channel enrollment:** one live channel per native scope, 32-byte cryptographic key and independent 32-byte challenge from OS randomness. The returned privileged bootstrap is NOT serializable as a public JSON frame and MUST be delivered exclusively to an isolated native-to-extension channel, never a page's MAIN world or local storage.
3. **Handshake:** HMAC-SHA256 challenge response with domain separation and channel ID/generation binding. Invalid proof does not authenticate; a consumed handshake cannot be replayed.
4. **Event admission:** HMAC-SHA256 per identity frame, strict one-by-one sequence numbers and distinct observation versus current-identity admission domains. Exact trusted host identity must equal the versioned normalized frame before 7A first-proof observation or current-proof authorization. This grants **identity-only admission**, not a permission to execute any user command; any future action must separately bind its actual command bytes and enforce owner/tool policy.
5. **Generation replacement:** advancing the native host generation retires the prior channel and requires fresh challenge authentication; old events are refused. The 7A session first-proof fence is preserved across channel rotations, so a fresh channel does not become an implicit session-rebind bypass. Stale retire events cannot evict successor channels. Bounded at 2,048 channels.

## Adversarial cloud regressions

Embedded Rust tests cover successful enrollment/proof/identity admission, unauthenticated frames, wrong HMAC, wrong signing domain, replay and gap refusal, mismatched origin/profile/tab/frame/generation, protocol downgrade, forged document, unaccepted new channel, stale channel retirement, cross-scope same-session theft, and unsafe origins/epoch rejection.

CI acceptance requires **exact PR head** Windows/Ubuntu Rust format, strict Clippy, complete workspace tests/release, separate task DAG validation and code review. Earlier failed CI runs due exclusively to formatter, Clippy argument layout, a test assertion requiring secret Debug, and stale DAG readiness expectations were corrected on the branch and are **not acceptance receipts**. See the final PR checks for the accurate outcome. No Rust host/browser process or user-machine files were accessed by the implementing agent.

## Hard safety boundary / deferred qualification

This is a native Rust *authority module*, not a running WebView2 transport. The source does not claim upstream v14 byte-level adapter parity, actual extension injection proof, physical Windows `7P-L1`, MV3 restart or native `7L1`, CDP target ownership, browser UI/editor, signed-in provider workflow, or automatic Send. Their owners are subsequent `7C/7L1/7D/7E` and separate owner gates. In particular **7L1 requires 7C AND accepted physical 7P-L1**; the native HOLD remains visible. Never deploy this module as an end-to-end authenticated browser bridge without a host adapter that enforces the documented trusted-source contract.

## Acceptance / handoff contract

- **Cloud 7B becomes Done ONLY after:** exact-head Windows+Ubuntu CI PASS, source review, PR #45 merge to main and Notion Status/Claim Owner/board update.
- **After accepted merge:** 7C Ready / Now (unclaimed), 7P-L1 Hold / Now; next native 7L1 remains Blocked / Up Next and never runs without the held local gate.
- **No physical OS or real-account proof** is provided by this cloud unit.
