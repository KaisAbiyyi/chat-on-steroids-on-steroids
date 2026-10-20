# CL-L1 cloud review and Windows candidate retest — 2026-10-20 v2.0

**Candidate native result: PASS. Official CL-L1 acceptance: BLOCKED.** [Repair PR #11](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/11) is still **OPEN / NOT MERGED**. Tested its exact head `6f4e6acc3594edc2759c8cb1dbefe551cb51e132`, not a merged repair SHA. **CL-04A remains BLOCKED.**

Implementation summary: completed independent source/fixture review and the full existing Windows qualification harness against the cloud candidate. No local feature implementation, cloud branch edits, merge, threshold changes or fixture regeneration. The original [FAIL summary](2026-10-20-cl-l1-windows-qualification-v1.0.md), receipts, raw attempts and previous executable remain unchanged.

[Retest receipt](../../audit/2026-10-20/cl-l1-retest-6f4e6ac-receipt.json) / [sanitized evidence](../../audit/2026-10-20/cl-l1-retest-6f4e6ac/).

## Review of cloud work

Bounded source review found **no actionable findings** in the repair. The final cloud head has the same product content as checkpoint `9594e0e44bc6b44a1c3cd6943e54029274249813`; later changes are documentation/evidence. This is an independent local inspection, not a submitted GitHub approval.

- `write_stdin` now emits `✓ finished` for known successful/benign exits, preserving unknown/running and genuine failure semantics.
- One source-equivalent helper normalizes ECMAScript whitespace and limits single/batched command labels to 70 UTF-16 units with the source ellipsis. Completed and progressive captions retain their label.
- Recorder redaction still precedes caption projection and inline/asset persistence. Full sanitized arguments remain intact. No schema, authorization, process supervision or runtime dependency change.
- Independently executed the actual pinned upstream functions: **44/44** additive completed/running cases match; **14/14** input records match the immutable original Windows evidence. All three upstream source hashes match. The historical corpus is byte-identical to the base; **338** representable cases and **five** existing UTF-16 exclusions are unchanged.
- [Exact-head hosted CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39397636815) PASS for Windows and Ubuntu. Live PR state, head and CI were re-read; PR #11 remained open at receipt capture.

## Windows retest

Clean separate checkout: `D:\projects\CoS-Rust\.worktrees\cl-l1-retest-6f4e6ac`, initially detached at the exact candidate SHA. Windows 11 build `10.0.26300`, AMD64, Rust/Cargo 1.91.1. The source checkout remained clean throughout qualification.

The original final harness was reused with **only** checkout path, binary path, expected SHA and fresh synthetic-profile name changed. Its assertions were preserved. A separate D: build cache was seeded from the old D: cache; the old executable's SHA-256 still matches its FAIL receipt. Registry, Cargo output, subprocess TEMP/TMP/TMPDIR, native profiles, esbuild bundles and evidence stayed on D: under `D:\projects\CoS-Rust`.

| Gate | Result | Evidence |
| --- | --- | --- |
| Windows workspace, Rustfmt, strict Clippy | PASS | `python tools/check-workspace.py --strict`: **257 passed, 0 failed, 0 ignored** |
| Release build | PASS | `cargo build --release --locked -j2`; 12,828,672 bytes; SHA-256 `db7602d642431dabaf85b686402e6f1c5067c045ef70222553158f09cdf81c37` |
| Existing native harness | PASS | **96 PASS / 0 FAIL**, no unhandled error |
| Successful completion captions | PASS | All three formerly failing exit-0 assertions now pass, including pipes and actual Windows ConPTY |
| Actual process states | PASS | Exit 7, still-running process session, completion exit 0, and unread output delivery |
| Runtime source differential | PASS | **14/14**, zero mismatches in both completed and progressive captions; original run had five mismatches |
| Inline/overflow privacy | PASS | Synthetic env/secret/clipboard/binary inputs and real poisoned process stdout sanitized before journal/assets/captions; full safe assets rehydrated |
| Sentinel scan | PASS | 10 sentinel identities, UTF-8/UTF-16LE, four before/after stages, 28 file observations, **0 leaks** |
| Clean restart and timeline | PASS | Two service PIDs, graceful drained shutdowns; 23 history events, both message shards, exact captions/evidence/call IDs/seq, turn identities, last activity and nonzero counter persist |
| Original eight WIP paths | PASS | Original HEAD/status, byte lengths and SHA-256 unchanged |
| Official merged-SHA CL-L1 gate | **BLOCKED** | PR #11 has not merged; no merged repair SHA has been locally qualified |

No real account/profile/token or actual system clipboard access. Optional local Linux remains **BLOCKED** on unverified Windows-host Docker/WSL storage; no Docker/WSL run was started. macOS **HOLD**. Crash durability, desktop/browser/installer acceptance **NOT RUN**. Original phases 1–6 remain **PARTIAL**; the deferred lossless UTF-16 decision remains `CL-04C`.

Performance comparison is **NOT MEASURED**. Observed check/build cost was 221.78 s / 242.43 s with a seeded cache; no speed, memory or package-size improvement is claimed. Published JSON/text uses canonical LF; the evidence manifest includes published and raw-capture hashes. Raw profiles, credentials and binaries stay ignored on D:.

## Handoff

```text
UNIT: CL-L1-RETEST-01 — source review and native candidate qualification
GITHUB / SHA UNDER TEST: PR #11 OPEN, head 6f4e6acc3594edc2759c8cb1dbefe551cb51e132; not merged
BRANCH: source fix/cl-l1-caption-parity (read-only); evidence feature/cl-l1-windows-qualification-20261020 / PR #10
CHANGES: additive implementation summary, receipt and sanitized evidence only; original FAIL evidence preserved
CLOUD VALIDATION: exact candidate-head Windows/Ubuntu CI PASS; 44 source cases and 14 archived inputs authenticated locally
MACHINE / NATIVE TESTS: Windows 11 AMD64 on D:, strict 257/0/0, native 96/0, runtime oracle 14/14, privacy and restart PASS
STATUS: candidate PASS; overall PARTIAL; official CL-L1 acceptance BLOCKED pending merged-SHA qualification
PERFORMANCE: comparison NOT MEASURED; binary 12,828,672 bytes; seeded-cache run observations only
LOCAL REQUIRED: repeat full CL-L1 on a reviewed merged repair SHA
YOU REQUIRED: NO private-account, actual clipboard or manual desktop acceptance
KNOWN LIMITS: PR unmerged; crash/desktop/browser/installer untested; UTF-16 lossless scope open; optional local Linux storage gate
NEXT: review/merge PR #11 via the repository workflow, hand off the exact merged SHA, run CL-L1 there, then consider CL-04A
RECOVERY: both qualification checkouts, raw synthetic profiles, old FAIL executable and original WIP remain on D:; do not derive cloud implementation from protected old WIP
```
