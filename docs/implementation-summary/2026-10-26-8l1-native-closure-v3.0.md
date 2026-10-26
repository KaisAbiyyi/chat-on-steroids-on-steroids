# 8L1 exact merged-SHA native closure v3.0 — PASS

**UNIT 8L1: PASS.** [PR125](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/125) merged after **37/37 exact-head CI SUCCESS** and direct implementor self-review. Accepted product source/merge SHA: `934cc293f0d59c6eea74d3c2171bd93f35a7ac2d`; reviewed PR head `f3820612a8a680f574be8753845aabe126553304`. Their complete Git trees are identical. The native host/Rust bridge were rebuilt at the exact merge SHA, all loadable-file hashes recorded, and the **full four-switch physical gate rerun successfully** with actual owner-operated dialogs.

## Final evidence

- Native run09 at exact merge SHA: **54 in-process assertions PASS** (27 per OS process round), **4 real crash/profile/restart comparisons PASS**, runner exit0, both Rust children exit0. Same profile paths, source-owned extension identities and persisted synthetic markers; new host/Rust PIDs and fresh authenticated channels.
- Each round: owner **Yes / No / Yes** for permissions and **Yes / Yes** for synthetic navigation and logical handoff. Actual counters: 3 prompts,1 grant,1 denial,4 native permission events,1 replay refusal,1 stale-allow refusal,4 settled,0 busy. Multi-tab/profile refusal, navigation/rebind, source attestation, atomic session-only handoff/replay refusal, actual Browser.crash and all Rust retirements PASS.
- Exact merge SHA strict Windows workspace: **405 PASS,0 FAIL,0 ignored**, fmt and strict all-target Clippy `-D warnings`, exit0. Explicit Rust bridge3, native linked-source policy25 (including actual signed runtime), JavaScript65, DAG16 and browser guardrails16 PASS in this repair. Both native host builds and compiled authority/retirement self-tests PASS.
- Host DLL SHA256 `4950b79b8fe29eb75dc3af956fae10d6684093f7c81dcda3eb1579c955e83e0d`; Rust bridge SHA256 `1ebe4e34efed6e16c8609575aa793f23f3cef553a09a8b8b58ef0165a3f0b3ed`.
- [Final aggregate](../../audit/2026-10-26/8l1-native-merged/qualification-pass.json), [before restart](../../audit/2026-10-26/8l1-native-merged/run09-before-pass.json), [after restart](../../audit/2026-10-26/8l1-native-merged/run09-after-pass.json), [real restart comparison](../../audit/2026-10-26/8l1-native-merged/run09-restart-pass.json). Raw native self-acceptance flags remain false by design; this separate L aggregate checks the full required set and actual cross-process evidence, without altering old receipts or thresholds.

## Changes and review

See [repair implementation and complete RED/GREEN ledger](2026-10-26-8l1-native-repair-v2.0.md). The runtime component bootstrap, crash CDP reply hang, crash-lost fixture metadata and permission callback reentrancy/decision-settlement defects are resolved. Source-owned fixture recovery requires exact identity/source; unknown extensions, stale native/profile/permission proofs and unconfirmed Rust retirements remain denied.

**Self-review — Codex local / COS-8L1-20261026 generation3:** native/profile/source authority, exact artifact provenance, callback deferral lifetime, permission modal/revocation rechecks, per-profile transaction exclusion and bounded crash handling reviewed directly against base-to-head. No open material finding in this bounded unit. Run08 did not exercise required No (2 grants/0 denials), so the harness correctly refused PASS; [that raw receipt](../../audit/2026-10-26/8l1-native-merged/run08-required-denial-not-exercised.json) remains unchanged. Run09 supplies the successful actual owner Yes/No/Yes path. Earlier failures/interruption/denied handoff are all preserved in prior receipts.

This closure publication changes **documentation and synthetic receipts only**. It does not produce a new runtime implementation or invalidate the explicitly pinned product source934cc29. Preserve the exact qualified SHA as the native acceptance target; subsequent runtime changes require their own gate.

## Handoff

- **UNIT / merge:** 8L1, local Windows; PR125 merged934cc29, final source-native acceptance PASS. Closure documentation PR records the completed merged-SHA proof.
- **CLOUD VALIDATION:** PR125 headf38206137/37 SUCCESS, complete merge tree equality verified. Closure PR has its own CI/self-review before eligible merge.
- **PERFORMANCE:** no product RAM/CPU/size improvement claim.
- **LOCAL REQUIRED:** none for this accepted bounded gate. Requalify changed runtime/source/artifact, not unchanged historical source.
- **YOU REQUIRED:** no remaining action for8L1. Actual owner interactions completed in both exact merged-SHA rounds.
- **KNOWN LIMITS:** production8P-HOST124, generalized JS/native contract acceptance and actual provider sign-in remain separate; 8U1/7U1 unaccepted, IME HOLD, no product renderer/performance/installer claim. D:-resident synthetic output/cache/temp; original eight-path WIP/status and upstream024731d pin preserved.
- **NEXT:** release8L1 claim, synchronize/read back Notion Done/Last Done and GitHub acceptance. 8U1 remains Blocked behind production8P-HOST124; no account access or dependent feature claimed here.
