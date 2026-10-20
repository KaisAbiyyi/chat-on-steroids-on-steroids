# CL-06A — Git/project/file-panel/watcher closure

Base: `340cf55eddb2766c96e1e1f36eaeb118faecad04`; lock `project-files`.
PR: [#19](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/19).
Session: `codex-20261020-cl06a-01`, claim generation 1.

## Contract and design

Pinned upstream `024731d8083d92c46e8248d64e7e0b36ab0a470f`:

- `src/main/project-git.ts`, SHA-256 `65bdac3c7e8de27f96f7c0c94ad3468783e3155617be1e4aed8b784135b79ed1`: snapshot/diff, selected-project `readWorkingBytes`, debounced watcher invalidation.
- `src/main/project-files.ts`, SHA-256 `53d96cca9fa68788f667c3d4352bdd86d862dbe43fbd2e7c313fb254f8fdf075`: regular-file/no-link panel, byte/mtime/revision optimistic saves, exact BOM/terminators, preview limits.
- Both source hashes were fetched and checked against the existing immutable fixture manifest. Existing source fixtures are unchanged.

Reuse the existing `cos-tools` owners and CLI callers. The panel already implements bounded previews, atomic staged saves, stale-revision refusal, binary handling and link refusal. Extend regression coverage for a same-size external edit whose mtime is restored, preserving external bytes and leaving no staging files.

Git working reads must apply selected-project link refusal before opening, verify the opened handle's actual path and revalidate the path after opening. Check native change time as well as size/mtime/identity around bounded reads. Reuse the panel's native stamp implementation.

Working-tree revision tokens incorporate current branch/HEAD and per-changed-file native stamps; diff publication rechecks the snapshot. **Deliberate conservative deviation:** working-tree revision digest bytes differ from upstream's statistics-only digest. Tokens are opaque, not persisted identifiers; old tokens require refresh. Committed branch comparison digest/output is retained. This prevents equal-statistics content edits or a different base commit from reusing the old token. No filesystem-wide atomicity is claimed against arbitrary external writes after the final check.

Watcher access events must not enqueue invalidation or overflow the queue. Only essential repository directories remain custody identities; optional refs/log directories may disappear normally. Reconcile at most 96 metadata watch candidates after invalidation, preserving generation discard, root revocation, debounce and bounded channel behavior. Events remain invalidation signals, never Git status data.

## Execution and acceptance

1. Verify main CI, CL-L1 and disjoint claim; publish claim/PR and update existing Notion task.
2. Demonstrate RED for stale statistics/base commit, approved-sibling symlink reads, optional-ref pruning and read-only access invalidation. Preserve existing oracles.
3. Implement the narrow owner changes; exercise linked-worktree refs and panel stale-save/preview semantics.
4. Run workspace tests, format, strict Clippy, task-DAG checks, independent boundary review and exact-head Windows/Linux CI.
5. Publish exact checkpoint/PR handoff in markdown and existing CoSoS Notion task. Keep CL-06B blocked until CL-06A acceptance/merge; CL-L3 waits for CL-06B and CL-07B.

Native Windows NTFS/device qualification is **NOT RUN** here. Full Phase 1–6 parity remains **PARTIAL**. `YOU REQUIRED: NO` now; no real accounts or private profiles.
