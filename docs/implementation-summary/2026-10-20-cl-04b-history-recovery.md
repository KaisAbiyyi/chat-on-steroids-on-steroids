# CL-04B — History replay limits and durable fault recovery

Date: 2026-10-20 (Asia/Jakarta). Canonical cloud source: `KaisAbiyyi/chat-on-steroids-on-steroids`.

## Prerequisites, identity, scope

- CL-04A [PR #12](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/12) **MERGED** as `1232803d9dba3ba05cca11cfc8101bb54bd51cc2`.
- CL-04B [PR #13](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/13), branch `work/cl-04b-history-recovery-20261020`, lock `session-store`. One cloud implementer with an explicit claim.
- Pinned source fixtures and earlier phase/CI receipts are immutable. No local CoS MCP, private profile, actual credentials or D: workspace modifications.

## Behavior and limits implemented

1. Replay journals at most **1 GiB** each (metadata checked before scanning) and reject appends that would cross that same bound, including separator bytes; preserve oversized original file bytes.
2. Canonical in-memory message overlays at most **100,000 identities and 256 MiB serialized content**. Check the existing map count, bound shard ingestion, and enforce identical limits before each canonical write using a per-session byte ledger; never accept writes that would become unrecoverable at restart.
3. Journal/shard sequence IDs must remain within **JavaScript's safe integer range (2^53−1)**. Refuse corrupt unsafe IDs at replay and refuse sequence writes once exhausted, instead of overflowing or creating unsafe cursors. A valid last safe ID remains readable, but cannot be followed by an unsafe write.
4. Bound history read materialization at **100,000 records** and refuse oversized `recent()` page requests; consumers should paginate explicitly. Oversized legacy map/shard input refuses recovery rather than silently omitting records.
5. Leave history `prune()` deliberately **non-destructive**, preserving existing source contract; do not introduce silent age-based deletion or recycling.
6. Maintain previous tolerant behavior for malformed/torn JSON tail lines that are not trusted sequence IDs; append safely after a tear, verify stable history across service restart.
7. Preserve recovery of canonical events from journal when primary and backup metadata are invalid. The test checks a second restart and original journal events.
8. Keep unpaired UTF-16/JSON representability decisions explicitly scoped to CL-04C, not silently repaired here.

## Validation evidence

The new `cl_04b_replay.rs` includes seven synthetic negative/recovery tests: unsafe journal cursor, last-safe sequence exhaustion, corrupted shard, oversized shard, bounded pages, dual-corrupt metadata rebuild, torn journal and no-op retention. Existing pinned baseline/history/metadata/source tests must also remain green. Windows and Ubuntu CI run Rustfmt, strict Clippy, full workspace tests and release build. Record an exact final head SHA and CI URLs in PR before merge; earlier intermediate failures cannot be counted as final acceptance.

- Cloud CI: **PENDING final SHA checks** at document creation.
- Performance: **NOT MEASURED**; explicit resource ceilings are safety bounds, not speed claims.
- Local required: **NONE immediately** for this cloud unit; later Windows backend integrated L gates remain.
- Human required: **NO** real account/device acceptance.
- Known limits: no destructive archive/TTL policy, no multi-process journal lock proof, no installed desktop acceptance, no automatic salvage of unsafe sequence records. Full phase 1–6 **PARTIAL**.
- Next: qualify the head, merge reviewed PR, update merge SHA and release the claim, then `CL-04C` or safe independent lanes per dependency board.
