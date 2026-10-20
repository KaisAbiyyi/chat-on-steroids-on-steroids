# CL-L1-FIX-01 — completion metrics and command labels

Claimed 2026-10-20 (Asia/Jakarta), user-assigned cloud repair.

```text
TASK_ID: CoSoS/CL-L1-FIX-01
AGENT / ROLE: Codex cloud, single implementer; independent read-only reviewer /root/review_cl_l1
OWNER_SESSION: 01a15879-c39b-703a-aa3f-f2302052209e
REPO: KaisAbiyyi/chat-on-steroids-on-steroids
CLAIM_VERSION: 1 (user-serialized assignment, no atomic lock controller)
STATUS: REVIEW; product checkpoint pushed, independent review complete; hosted CI is a separate gate
BASE_SHA: 06f586e423e5abb4f95eba582996c4a5278c1311
BRANCH: fix/cl-l1-caption-parity
RESOURCE_LOCKS: cos-store caption projection and additive caption regression fixtures/tests
DEPENDENCIES: CL-L1 failure handoff in PR #10 at 12b2523f75799125ebfc714ba106e57048ef2226
CHECKPOINT_SHA: 9594e0e44bc6b44a1c3cd6943e54029274249813 (product fix pushed)
PR: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/11 (OPEN, draft)
LAST_ACTIVITY: 2026-10-20; Linux strict checks PASS, independent review complete; inspect live PR checks for hosted CI
PLANNED_TEST: additive pinned-source oracle cases reproduce CL-L1-F1/F2 RED, then GREEN; strict workspace checks and Windows/Linux CI
LOCAL_REQUIRED: CL-L1 retest on the reviewed merged repair SHA
YOU_REQUIRED: NO private-account or manual desktop acceptance
NEXT: repair caption projection, qualify cloud changes, then native CL-L1 retest; CL-04A remains BLOCKED
```

The original Windows failure receipt and frozen caption corpus remain historical
evidence. Cloud repair does not turn the failed native gate into PASS.

## RED checkpoint

The three existing caption tests PASS on the unchanged product. Executing the
pinned source generated 32 additive cases: all 14 archived native inputs, six
completion controls and 12 command-label boundaries. Four new regression tests
FAIL on the unchanged product for the expected metric/label differences,
including the original five native mismatches. No product code changed at this
checkpoint. The generator verifies all three source hashes against the old
corpus; no old fixture or acceptance exclusion changed.

## Product change and GREEN

- `write_stdin` emits `✓ finished` for a known zero exit or source-marked benign
  exit. Unknown/still-running processes have no completion metric; genuine
  nonzero exits retain their failure metric and error outcomes retain their
  failure/refusal projection.
- `exec_command` uses one source-equivalent command-label helper for single and
  batched commands: first non-comment line, ECMAScript whitespace normalization,
  first 70 UTF-16 code units, and ` …` for a shortened or multiline label.
  Empty filtered batches fall back to `cmd`, as upstream does. Truncation affects
  captions only; complete sanitized recorded arguments are retained.
- The recorder still redacts before projecting/persisting captions. No schema,
  grant, process-supervision or wire-contract changes; no new runtime dependency.

Both completed and running captions match all **44/44 additive source cases**,
including **14/14 archived Windows inputs** (formerly five mismatches). All three
existing caption tests remain green, including **338/338 representable original
source cases** and their unchanged five historical UTF-16 exclusions. Four new
tests cover these groups and actual recorder persistence/cold reopen.

Final command-label controls exercise all 12 inputs with both running and
completed facts (24 label cases total). Their RED run exposed a generic
`Running exec_command` fallback for successful completed batches, including
empty batches falling back to `cmd`. Removing the batch-only fallback preserves
the actual label, while timeout/non-benign failure fallbacks remain unchanged.
This is part of CL-L1-F2's running/completed label contract.

The initial cold-reopen test incorrectly expected a metric in `lastToolActivity`;
the existing `model::apply_summary` contract projects only title/kind there.
The test was corrected to check that projection and retain full summary/metric
checks in every history event. No production timeline schema or acceptance
threshold changed to accommodate the test.

## Validation and boundaries

`python tools/check-workspace.py --strict` PASS in this isolated Linux cloud
checkout: **247 passed, 0 failed, 0 ignored**, including actual process, PTY,
recorder/privacy/restart and native OpenSSH tests; Rustfmt and all-target Clippy
with `-D warnings` PASS. Targeted caption tests: **7 passed**. Fixture regeneration
was rerun and produced byte-identical output. Hosted Windows/Linux exact-head CI
is tracked by [PR #11's live checks](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/11/checks);
require both operating systems to pass on the final head before merge. The
test-only RED checkpoint has intentionally failing CI and is not the candidate.

[Independent review](../implementation-review/2026-10-20-cl-l1-fix-01-caption-parity.md)
found no remaining actionable code/test issues at the final product checkpoint.
The reviewer separately authenticated source hashes, all 44 oracle outputs and
all 14 archived native inputs. The review does not replace CI or the native gate.

Historical fixture SHA-256 remains
`351b6c5ddd762e127a012a6d946c8664b0abb2a8bc9496ef8ccec09235c28884`.
The generator executes pinned `summarizeToolCall`, `summarizeRunningCall` and
`emptyEvidence`; it verifies all source hashes against the unchanged old corpus.
For reproduction, fetch evidence commit
`12b2523f75799125ebfc714ba106e57048ef2226`, check out the pinned upstream under
`upstream/chat-on-steroids`, install `esbuild@0.25.12` into the ignored
`.cache/cl-l1-oracle` directory and run `node tools/capture-cl-l1-captions.mjs`.
Only pure source functions run; no Windows commands or tool calls are executed
by fixture regeneration or source-case replay in this Linux VM.

**PERFORMANCE:** baseline/candidate comparison NOT MEASURED; no speed, memory or
binary-size improvement claimed. Caption output is bounded; resource admission
and process behavior are unchanged. Linux cloud release build PASS; ELF binary
12,249,336 bytes. This is an observation for this environment, not a comparison
with the historical Windows binary or an installer-size claim.

[Machine-readable receipt and log hashes](../../audit/2026-10-20/cl-l1-fix-01/receipt.json)
identify the exact product checkpoint, source blob, fixture and five command logs:
initial RED, completed-batch RED, final GREEN, strict workspace checks and release
build. Hosted CI was still running when that receipt was captured; live final-head
PR checks supersede only that CI observation, not the immutable native failure.
Published logs use one final LF for a clean Git whitespace check; the manifest
also retains the original-capture byte counts/hashes. Original command output
remains unchanged in the ignored cloud cache. No assertion or result changed.

**KNOWN LIMITS:** cutting a surrogate uses the existing printable-escape policy;
lossless JS UTF-16 remains `CL-04C`. No historical exclusions were broadened and
the new fixture excludes no rows. Native Windows CL-L1, crash durability,
desktop/browser/installer acceptance are NOT RUN by this cloud repair. macOS HOLD.

**LOCAL REQUIRED:** rerun full CL-L1 using a clean isolated D: checkout of the
reviewed **merged repair SHA**, a fresh synthetic profile, D: cache/temp/evidence,
the archived harness/assertions from PR #10 and the unchanged pinned upstream.
Keep the original failed receipt and eight old WIP paths intact. Replay in cloud
does not establish a native PASS.

**YOU REQUIRED:** NO private-account, clipboard or manual desktop acceptance.
**NEXT:** finish review and Windows/Linux CI, merge the repair through review,
then native CL-L1 retest. `CL-04A` remains BLOCKED until that gate is PASS.

**RECOVERY:** the product fix is pushed to PR #11 at the checkpoint above;
documentation/evidence follow as a separate commit. Inspect actual branch/head
and PR checks before resuming. No local D: workspace was modified.
