# CL-04A — provider/session ownership and rebind reconciliation

Date: 2026-10-20 (Asia/Jakarta). Cloud unit, PR #12, base `ae892add3126eb7bb65accc56dedd03f08a035b0`.

## Scope and baseline

Prerequisite CL-L1 PASS on merged repair `a00edd4a67e6c8acc9fc45bee659094719d6673a`; main Windows/Ubuntu CI PASS at `ae892add`. Existing pinned source fixtures `session-model-selection.json`, `session-title.json`, `session-recovery.json`, and related regression suites remain unchanged.

Two previously unguarded authority boundaries:

1. `SessionStore::create` accepted duplicate active or previously retired non-UUID conversation IDs; `rebind` checked collision only for UUID-like IDs. Multiple durable sessions could therefore claim the same provider conversation, making lookup/attribution ambiguous.
2. `title::rename` allowed an explicitly scoped manual title update carrying a stale conversation ID after rebind. A stale provider or model observation was already fenced, but the manual branch did not share that fence.

## Implemented on topic branch

- Reject opening another session for a conversation ID actively or historically owned by another local epoch; do not reassign retired aliases.
- Reject rebind into another session's active or retired provider ID, including plain non-UUID IDs.
- Fence manual renames with an explicitly mismatched conversation ID while preserving unscoped local manual renames, provider-ignore/auto title precedence and existing model timestamp semantics.
- Add `cl_04a_authority.rs` integration tests for active/retired collisions, stale manual/provider/model/correlation after rebind, durable restart, and a failed metadata-publication rollback.
- Do not alter request correlation semantics or native `observe_model` source rules already covered by pinned-source tests.

## Verification and limits

CI: running for PR #12; Rustfmt was normalized after the initial check failed. Report precise Windows/Ubuntu results before marking accepted. No local source checkout, personal profile or credentials accessed. No performance comparison run. Full Phase 1–6 parity remains PARTIAL; retention/replay budgets belong to CL-04B and lossless UTF-16 to CL-04C.

```text
UNIT: CoSoS/CL-04A
CLAIM: single GitHub-connected cloud implementer, session-identity lock
BRANCH: work/cl-04a-session-authority-20261020
PR: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/12
CLOUD VALIDATION: PENDING latest CI
PERFORMANCE: NOT MEASURED
LOCAL REQUIRED: NONE directly for CL-04A; later named L gates unchanged
YOU REQUIRED: NO direct account/clipboard/manual acceptance
KNOWN LIMITS: no historical ambiguous-ledger auto-repair; later CL-04B/CL-04C remain
NEXT: finish CI/review and merge only after acceptance, then CL-04B when ready
```
