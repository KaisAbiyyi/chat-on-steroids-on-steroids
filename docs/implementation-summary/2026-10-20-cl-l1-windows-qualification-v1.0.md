# CL-L1 Windows qualification — 2026-10-20 v1.0

**STATUS: FAIL.** Tested product SHA: `06f586e423e5abb4f95eba582996c4a5278c1311`, merged by [PR #9](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/9). Recorder privacy, trusted process facts and clean-restart persistence passed. Two source-caption differences prevent accepting CL-L1; **CL-04A remains BLOCKED**.

Implementation summary: completed local qualification and a reproducible cloud failure handoff. **No product implementation, fixture rewrite, threshold change or feature continuation.** The old checkout and its eight protected WIP paths retain the same HEAD, Git status and SHA-256 hashes.

[Machine-readable receipt](../../audit/2026-10-20/cl-l1-windows-06f586e-receipt.json) and [sanitized evidence directory](../../audit/2026-10-20/cl-l1-windows-06f586e/). The receipt identifies the product under test; a later documentation commit is not a new qualified product SHA.

## Native qualification and evidence

Windows 11 Home Single Language, build `10.0.26300`, AMD64; Rust/Cargo `1.91.1`. Started from a clean detached checkout at `D:\projects\CoS-Rust\.worktrees\cl-l1-06f586e`. After testing, this checkout moved to `feature/cl-l1-windows-qualification-20261020` for documentation/evidence only. The user's original checkout stayed on its protected branch.

| Gate | Status | Evidence / boundary |
| --- | --- | --- |
| Exact merged SHA and clean qualification source | PASS | Clean at native-probe start; pinned upstream `024731d8083d92c46e8248d64e7e0b36ab0a470f` remained unchanged |
| Eight original WIP paths | PASS | Before/after HEAD, porcelain status, byte lengths and SHA-256 match |
| Windows workspace tests | PASS | `python tools/check-workspace.py --strict`: 253 passed, 0 failed, 0 ignored; Rustfmt and strict all-target Clippy passed |
| Release build | PASS | `cargo build --release --locked -j2`; binary 12,825,600 bytes, SHA-256 `7d1ebdb4e34f9074f38fe0ea87e45160bca7de1cef3694945ca708f4eb4f5f45` |
| Exact-SHA cloud CI | PASS | [Run 39233631170](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39233631170), Windows and Ubuntu format/Clippy/tests/release jobs |
| Recorder inline and overflow privacy | PASS | Real release `record-call` API; complete argument JSON and result assets rehydrated; environment, secret/password, clipboard, image/binary and file-handle synthetic sentinels removed |
| Live poisoned process stdout privacy | PASS | Real MCP exec/write_stdin output contained the synthetic token positive control; stored inline and overflow args/results/captions did not |
| Journal/assets sentinel scan | PASS | 10 synthetic sentinel identities, UTF-8/UTF-16LE byte scan, four before/after stages, 28 file observations, 0 leaks |
| Native nonzero/running/completed facts | PASS | Actual exit 7/0, still-running session IDs and unread completion; Windows pipes and ConPTY; no success inferred from prose |
| History/caption/timeline persistence | PASS | Two distinct service PIDs, graceful drained shutdowns; 23 full history events, both native message shards, stable seq/call IDs/evidence/captions, complete turn identity, nonzero counter and last activity across cold restart |
| Existing source caption corpus | PASS, bounded | 338 representable cases; five historical UTF-16 exclusions unchanged |
| Additional live source differential | **FAIL** | 5 mismatches in 14 actual runtime projections, covering two root causes below; both completed and progressive captions compared with executed pinned upstream functions |
| Final native probe assertions | **FAIL** | 93 PASS, 3 FAIL for successful-completion caption metric; no unhandled error |
| Optional local Linux | BLOCKED | Actual Docker/WSL Windows-host backing storage was not verified on D:; no local Docker/WSL cache-producing run started |
| macOS | HOLD | Outside active target |
| Desktop/browser/installer, crash durability | NOT RUN | Clean service restart is the tested boundary |

The persistence PASS includes faithful persistence of the currently incorrect caption; it does not convert caption parity to PASS. No personal profile, account token or actual system clipboard was accessed. Synthetic clipboard tests exercise recorder inputs only.

All task-owned caches, temporary profiles, subprocess TEMP/TMP/TMPDIR, Cargo target/registry, oracle bundle and evidence remain under `D:\projects\CoS-Rust`. Initial online Cargo indexing stalled; that attempt was retained, then existing registry cache was copied read-only from C: to D: and the successful checks ran offline. No unknown C: data was cleaned or moved. Raw synthetic profiles, generated service credentials and build outputs remain ignored and are not publication material.

Performance comparison is **NOT MEASURED**. Observed check/build durations were 357.11 s / 385.67 s; final native probe 28.20 s. These are run-cost observations, not a baseline/candidate benchmark. Binary size alone does not establish an improvement, desktop footprint or installer size.

## Failures returned to cloud

**CL-L1-F1 (P2): successful process completion loses the source metric.** After a real process returns exit 0, `call.evidence.exitCode=0` and `running=false`, but `write_stdin` summary lacks `metric: "✓ finished"`. Reproduced with pipe background completion, a short successful command and ConPTY completion. Rust [`caption.rs`](../../crates/cos-store/src/caption.rs) lines 334–362 only assigns a metric for nonzero exit. Pinned upstream `src/main/session/summarize.ts` lines 367–369 also emits the successful-completion metric. The original corpus has **zero write_stdin exitCode=0 cases**, so its green result does not cover this path.

**CL-L1-F2 (P2): long shell-command captions omit source truncation.** Rust `caption.rs` lines 291–293 retains the complete first non-comment line. Pinned upstream `summarize.ts` lines 91–94 uses the first 70 UTF-16 code units and appends ` …` when shortened. Both actual pipe and ConPTY launch titles differ; progressive captions differ as well. These are ASCII inputs, independent of the deferred unpaired UTF-16 problem.

The [executed source-oracle result](../../audit/2026-10-20/cl-l1-windows-06f586e/completed-source-oracle.json) records expected/actual captions, trusted facts, call IDs and source SHA-256. Source hash matches the unchanged pinned corpus. The oracle bundles the actual pinned function on D:, not a handwritten expected-output substitute.

## Harness history and reproduction

All four preliminary attempt receipts/logs/harness versions are retained. Their harness-boundary failures were investigated rather than counted as product defects: history adds `turnOrigin`; native messages occupy separate shards; launch and continuation store different result records; `find` is not advertised when command capability is enabled. The final harness verifies every canonical journal field by seq, both rehydrated messages, projected turn origins and full API history across service restart. No acceptance threshold or frozen fixture was changed.

Published evidence uses canonical LF for reproducible Git checkout hashes. The manifest records both published-byte hashes and original-capture hashes; original raw captures remain unchanged under the D: run directory. The `runtime-history-before-restart.json` capture is the raw journal subset; the after file is full rehydrated API history. Their expected shard/projection differences are checked explicitly by the harness, which also compares full API history before/after the second service process.

Final harnesses are archived with the evidence. For a new run, set the expected new merged repair SHA and D: checkout/run/binary paths, then choose a **new** empty synthetic-profile directory in `qualify-cl-l1.py`; keep the assertions and original evidence. Run on a clean checkout of that exact SHA, using D: TEMP/TMP and Cargo paths. The archived source checker also needs the unchanged pinned upstream and its existing `esbuild` dependency; it writes its bundle only beneath the selected D: run directory. The final run intentionally exits nonzero while CL-L1-F1 remains reproducible.

## Cloud handoff

```text
UNIT: CL-L1 — Windows native recorder/privacy/process/restart qualification; FAIL
OWNER/ROLE: Codex local, user-assigned L session; qualification only
BASE / GITHUB SHA UNDER TEST: 06f586e423e5abb4f95eba582996c4a5278c1311
BRANCH: feature/cl-l1-windows-qualification-20261020 (documentation/evidence only)
GITHUB: product PR #9 merged; documentation publication tracked by its separate PR
CHANGES: implementation summary, immutable sanitized receipts/reproducers, CURRENT-PHASE pointer; no product changes
CLOUD VALIDATION: existing exact product-SHA CI run 39233631170 PASS, Windows + Ubuntu
NATIVE TESTS: strict checks PASS (253/0/0), privacy/process facts/restart PASS; caption parity FAIL
PERFORMANCE: comparison NOT MEASURED; binary 12,825,600 bytes; run-cost observations only
EVIDENCE: audit/2026-10-20/cl-l1-windows-06f586e-receipt.json and adjacent evidence directory
LOCAL REQUIRED: rerun CL-L1 against a new merged cloud repair SHA
YOU REQUIRED: NO manual/private-account/desktop action
KNOWN LIMITS: CL-L1-F1/F2; existing UTF-16 gap; clean restart only; no full-phase/desktop/installer acceptance
NEXT: proposed cloud repair CL-L1-FIX-01; source-equivalent completion metric and command-label truncation, additive source-oracle RED/GREEN regressions, strict Windows/Linux CI, reviewed merge, exact-SHA handoff; then CL-L1 retest
REMAINING: CL-04A BLOCKED until CL-L1 passes; no feature unit or cloud repair was started here
RECOVERY: original WIP untouched; raw local profiles/cache remain ignored on D:; resume from the reviewed product/repair SHA, never from protected old WIP
```
