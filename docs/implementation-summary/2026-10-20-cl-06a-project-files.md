# CL-06A — Git/project/file-panel/watcher cloud evidence

- UNIT / SESSION: `CL-06A`, `codex-20261020-cl06a-01`, claim generation 1.
- BASE: `340cf55eddb2766c96e1e1f36eaeb118faecad04` (CI-qualified main).
- CODE CHECKPOINT: `d8b82faf4b23fa55c0ea0a333e7a5929605f512a`.
- BRANCH / PR: `cl-06a/project-files-20261020`, [#19](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/19).
- STATUS: **REVIEW**; implementation/review complete, final exact-head CI and merge pending.

## Changes and source compatibility

Working-tree Git revision tokens include current branch/HEAD and changed-file native identity/change stamps. Equal-statistics edits, replaced files and new base commits invalidate old Review requests. Diff construction checks the snapshot again before publishing text. Git working reads enforce the selected-project no-symlink/junction boundary, verify opened handle paths, and compare change time alongside identity/size/mtime.

Watcher reads/opens no longer trigger refresh loops or overflow the queue. Optional-ref pruning no longer revokes repository custody. At most 96 metadata watch candidates are reconciled after debounce; optional watches are rearmed even if a namespace was deleted and recreated at the same path. Essential Git/common anchors stay registered. Events remain invalidation signals, never Git status.

Existing panel semantics remain: bounded text/image/PDF previews, binary refusal, exact BOM/CRLF bytes, revision/size/mtime save preconditions, staged atomic save, selected-project link refusal and native create/rename guards. A new test proves same-size edits with restored mtime cannot overwrite external bytes or leave staging files.

Pinned source hashes match the immutable manifest; original fixtures are unchanged. The [plan](../implementation-plan/2026-10-20-cl-06a-project-files.md) records the deliberate working-tree revision digest deviation: opaque tokens now fence more than source statistics. Committed branch comparison output/digests retain the source contract. No filesystem-wide atomicity against arbitrary external writes after the final check is claimed.

## Validation and review

- **PASS:** 272 workspace tests, 0 failed/ignored, 73 test/doc-test suites on isolated cloud Linux, Rust 1.91.0; format, strict workspace/all-target Clippy, task-DAG validation and whitespace checks.
- Seven observed **RED → GREEN** cases are in the [machine receipt](../../audit/2026-10-20/cl-06a/cloud-receipt.json). First five failed against baseline. Ref-namespace replacement failed against `5389a06`. The publication-race test failed with only the final snapshot fence removed, then passed with it restored.
- **PASS independent review:** `/root/review_cl06a` found a P2 optional-watch replacement issue in `5389a06`; reproduced, repaired and re-reviewed in `d8b82fa`. Independent reproduction, all eight watcher tests and the deterministic diff-publication test pass. No remaining actionable blocker.
- **PENDING:** final exact-head GitHub Windows/Ubuntu format, strict Clippy, workspace tests, release build and DAG checks. An older green SHA does not qualify a later head.
- **Windows CI observation:** push run [39444869755](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39444869755) on `d8b82fa` failed the namespace test's immediate no-further-event assertion. Windows may deliver trailing native notifications after the first debounce. The regression now lives in the private watcher test module and requires bounded quiescence of both raw pending debounce state and published invalidations before creating the next branch. Restoring stale-watch retention still produces RED; rearming produces GREEN. Independent review accepted the stronger isolation; no runtime change was needed for this test correction.

## Performance and handoff

The same [public synthetic driver](../../audit/2026-10-20/cl-06a/perf-driver.rs) used 64 modified UTF-8 files of 4000 bytes, two warmups and ten samples per baseline/candidate on the same cloud Linux VM, workspace debug profile. Nearest-rank p50/p95, milliseconds:

| Operation | Baseline p50 / p95 | Candidate p50 / p95 |
| --- | --- | --- |
| Snapshot | 63.08 / 64.13 | 64.82 / 75.02 |
| Diff | 94.39 / 96.12 | 161.25 / 175.95 |

Final snapshot validation adds subprocess/metadata cost. This small debug sample does not establish release/native latency, RSS, binary/installer size or an improvement. No new dependency or persisted schema was introduced.

LOCAL REQUIRED: **CL-L3** after CL-06B and CL-07B; none immediately. Native NTFS/device acceptance **NOT RUN**. YOU REQUIRED: **NO** now. Full Phase 1–6 parity remains **PARTIAL**; desktop/browser/installer acceptance is outside this cloud unit.

NEXT: qualify the final PR head, merge after green CI/review, publish exact merge SHA and release claim. Then CL-06B is eligible; retain CL-L3 dependencies.

TRACKING: [existing CoSoS Notion CL-06A task](https://app.notion.com/3ffd8330eb4681c3bf09e0c3511376d8) is Review with checkpoints, tests, review repair, performance and native gates. GitHub remains source/claim/CI authority.

ATTRIBUTION: initial claim used the pre-existing repository author identity; subsequent implementation commits use the connected owner's verified GitHub noreply identity. Actual Codex implementer/reviewer involvement is recorded above.
