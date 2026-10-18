# Phase 1–6 parity closure — implementation review v1.0

Date: 2026-10-17, Asia/Jakarta. Status: **NOT RUN — implementation in progress**.
The one fresh whole-scope review is performed after the integrated closure work and qualification.
Existing review findings and receipts remain historical evidence; this file does not imply approval.

- [Plan and ledger](../implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md)
- [Current implementation summary](../implementation-summary/2026-10-17-phase1-6-parity-closure-v1.0.md)

## Qualification notes for the pending whole-scope review — 2026-10-18

These are implementation receipts, not an independent review or approval. Search currently passes
201 native Windows tests plus formatting/strict Clippy. Inspect the caller-thread authorization
coordinator, worker settlement after revocation, native directory relocation, all-root selection
and SFTP handle cleanup against the dated source/hash receipts. Retain the unpaired-string refusal
as OPEN; it must not be counted among the eleven exact content projections. Server regex, local
ripgrep details, SFTP pipelining/entry bounds and remaining tasks still require closure. The matcher
commit's 177-test Linux receipt does not qualify the subsequent search engine. Storage is separately
recorded: workspace-drive temp/archive paths and a 998 MiB fresh lean Linux build cache.

The first search-engine Linux receipt is FAIL because the test compared different directory-order
inputs. Both inputs are now captured from the original handler and the exact native input selects
the expected response; counts are never rewritten. Review that selection separately from product
traversal. Updated Windows qualification remains 201 PASS; subsequent Linux proof is still required.

Exact correction commit `4a40c93` subsequently passed **193 Linux tests**, formatting and strict
Clippy. Windows cold qualification under the default lean profiles passed **201 tests** plus strict
checks; review symbol/incremental settings separately from unchanged assertions and acceptance limits.
The release passes 41 actual runtime/source checks and measures 11.923 MiB. Storage cleanup and the
1.464 GiB rebuilt Windows / 1.1 GiB Linux caches have separate receipts. Content fixtures qualify the
source filesystem fallback; full local ripgrep, partial-read hit retention and stream/chunk semantics
stay open. These notes remain implementation evidence; independent whole-scope review is **NOT RUN**.
The default Linux runner at `628b027` also passed all 193 tests and strict checks, retaining the
1.1 GiB cache. This proves the committed default lean settings, separately from earlier explicit runs.

Task 5 evidence for the eventual whole-scope reviewer: inspect source title precedence and automatic
title restore, existing mutation-fence reuse, write-before-publish on rename and recovery, explicit
Windows unreadable-file handling and unknown metadata retention. The source corpus distinguishes
the private normalizer from a full durable store load. Three crash snapshots check canonical revision,
original timestamps, current epoch and rebound context deltas. Activity reducer vectors cover typed
decision state and safe integers; retain two derived lone-surrogate gaps separately from exact passes.
Combined qualification is in progress. This is implementation evidence, not an independent review.
The combined session checkpoint passes **219 Windows tests**, formatting and strict Clippy. Its first
run (123 PASS / 1 FAIL) is retained: a historical restart assertion used a warm source summary. Fresh
execution of the unchanged source files confirms the cold difference, with identical events/recent.
The test now compares both complete source states and hashes the exact historical input. Review this
oracle correction separately from implementation. Whole-scope independent review remains **NOT RUN**.
Exact commit `88c0867` also passes 209 native Linux tests and strict checks. Subsequent metadata-string
work must be qualified separately: verify escaped backslashes/paired surrogates are accepted, and an
unrepresentable primary is refused before fallback or file replacement. Two focused native regressions
pass after observed RED; the source has four string and seven byte-decoding inputs. Global lossless JSON
remains OPEN. These notes do not substitute for the final whole-scope review.
Model/recorder subset: inspect same-conversation/newest-time checks, duplicate/no-write behavior,
metadata failure staging and the binding guard's lifetime after rebind. Seventeen actual source store
steps and four native tests cover the implemented routing subset; the five-second deadlock regression
must remain bounded. Broader recorder activity, proof and superseded-prose semantics remain open.
The combined metadata/observation checkpoint passes **225 native Windows tests**, formatting and
strict Clippy. Retain the prior Clippy failure (all tests passed) and complete rerun separately.
Linux current-commit qualification follows; existing 209-test proof applies only to `88c0867`.
Exact commit `caa234a` subsequently passes 215 Linux tests and strict checks, keeping a 1.1 GiB cache.
For Task 5c, inspect stored-text hydration only when truncated, recorded HTML admission, transcript
deduplication/first placement, answer completion gates and source JS whitespace/line boundaries.
The export source function uses a memory sink; clipboard/dialog proof stays NOT RUN. Three unpaired
content projections remain explicit refusals. Number formatting reuses pinned ryu-js, already present
in the workspace. Cached regex references were hoisted after strict Clippy feedback. Integrated checks
are running; the fresh whole-scope review remains NOT RUN.
The content checkpoint passes 237 native Windows tests, formatting and strict Clippy. Its corpus
includes 199/200/201-unit escaped continuation boundaries and all eight session/answer outcomes.
Keep three unpaired output refusals distinct from 119 exact content values/errors. No threshold or
resource limit changed. Current-commit Linux/release evidence follows; this remains implementation
evidence ahead of the single whole-scope review.
