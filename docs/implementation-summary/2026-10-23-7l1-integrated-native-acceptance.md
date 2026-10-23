# 7L1 — physical Windows Rust ↔ WebView2 ↔ MV3 integration acceptance

**Date:** 2026-10-23

**Task:** `7L1` — Real extension handshake / MV3 restart

**Implementation:** [PR #51](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/51), merged into `main` as `83da1f1ce7c5116314e1b658a03831b461cf97ef`
**Physically tested source:** `2590decb7c618ace066795c2aeb6b4e5e2612405` (the PR's exact head, preserved as an ancestor of its merge commit)

## Acceptance evidence

- [Raw physical Windows receipt](../../audit/2026-10-23/7l1-2590dec-integrated-native.json): `native7L1Accepted=true`, **21 PASS, 0 FAIL**, native Rust child exit code `0`. The Windows host verified Git HEAD, clean authority files, source stamp and SHA256 of all 14 manifest-listed native artifacts before granting authority.
- The authentic Rust channel completed an extension MV3 `service_worker` handshake; real separately provisioned WebView2 profiles and tab controllers were checked. The same-profile rival and cross-profile rival each completed a Rust authenticated handshake before the Rust first-proof fence returned precisely `ConflictingAttribution`. The original owner reauthenticated at a higher generation.
- Actual 7C worker claim, signed authorization, receipt and ACK were committed and recovered from `chrome.storage.session`. After an observed 65-second detached idle, the live worker target was forcibly stopped and a new worker incarnation proven. Persisted claims repaired without duplicate Send; the cross-generation ACK had a valid Rust authorization but was rejected by the ledger with precisely `CompanionError: stale-claim-generation` and no persisted state change.
- Real physical WebView2 navigation and tab disposal revoked privileged CDP access and retired the Rust channel. Spoofed page-origin events and CDP page fallbacks did not gain access to the worker adapter.
- `native_mv3_natural_idle` is **NOT_RUN**, because the worker remained alive during the 65-second idle observation. The documented `native_mv3_idle_or_forced_restart` alternative **PASS** demonstrates an actual worker reincarnation after forced stop. Do not describe natural idle as proven.

## Self-review and CI

The implementor (ChatGPT GPT-6) reviewed the exact diff and the source contracts, including initial P2 findings regarding arbitrary Rust first-proof failures and arbitrary exception handling for stale ACK denial. Both assertion oracles now inspect the real successful prerequisite and the precise failure reason. Physical re-runs also exposed and fixed native fixture flaws in session reuse after navigation and WebView2/CDP page target readiness. Generated native outputs were outside the source checkout, and `git diff --check` and .NET `-warnaserror` passed with zero warnings/errors.

At exact tested head `2590decb7c618ace066795c2aeb6b4e5e2612405`, **all 14/14 hosted PR checks succeeded**, including the Ubuntu/Windows Rust fmt, strict Clippy, unit/integration test, release build, signed event contract, native artifact build, and task DAG.

- [Rust Windows/Linux push CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39951179529)
- [Native integration artifact at exact source SHA](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39951179460)
- [Windows/Linux Task DAG](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39951179618)
- [Implementor self-review comment](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/51#issuecomment-6364726675)

## Scope, decision and next

**7L1 DONE / PASS** for the isolated physical Windows integration contract. The test uses fresh synthetic host/page fixtures and D:-only browser profiles; it does not exercise private user sessions, provider OAuth, real Send, a complete production desktop installer, or the future owner UX gates. Keep those separately scoped and pending.

`7D` (Direct browser tools / debugger / frame ownership) is now a **Cloud Ready candidate**, subject to checking current GitHub claims, CI and the `browser-tools` lock before work. `7E` remains blocked until `7D` is accepted.
