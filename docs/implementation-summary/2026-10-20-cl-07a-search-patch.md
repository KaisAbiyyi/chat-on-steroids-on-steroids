# CL-07A cloud implementation evidence (2026-10-20)

**Unit:** CL-07A — Search/patch traversal, exclusions, ordering, limits and transactional error exactness.  
**Repository:** [KaisAbiyyi/chat-on-steroids-on-steroids](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids)  
**Branch/PR:** `work/cl-07a-search-patch-20261020` / [PR #20](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/20)  
**Base SHA:** `340cf55eddb2766c96e1e1f36eaeb118faecad04`  
**Implementer:** ChatGPT GitHub cloud session `chatgpt-2026-10-20-cl07a-01`, claim generation 1, resource lock `search-patch`.  
**Implementation checkpoint:** `a2140ab18ab01e0bdb9cfa286e309fc9d058b2bc`. **Verified PR code head:** `52916c7764a2e425144d80f35a9b4811d74b0f99`.

## Proven scope

1. **Transactional patch authority:** Local `Files::apply_checked_mode` explicitly checks `deleteFile` before scheduling a removal and `move` before scheduling any move, including content-preserving moves. This closes an authorization gap for direct library callers that do not use MCP's separate preflight. Existing typed patch errors, content snapshots and transaction rollback behavior remain the authority.
2. **Search denied-read/folder behavior:** Source-compatible skipping of non-permission unreadable directories remains. Backend `PermissionDenied` from directory opening, content metadata, or byte reads now fails closed instead of silently publishing a misleading partial result.
3. **Tests:** New `crates/cos-tools/tests/cl_07a_parity.rs` validates denied delete, denied unchanged move, atomic failed later hunk, skipped missing directory vs denied directory, and refused content/metadata. Existing source-pinned traversal/exclusion/glob/search/patch oracle tests and complete workspace are required in CI.
4. **Boundaries:** The source-pinned BFS ordering, default exclusions, 40,000 files/10,000 directories, search time and 2 MiB content bounds remain unchanged. No regex stream/ripgrep changes; those are `CL-07B`. No `CL-06A` Git/file-panel/watchers code, `CL-05A` processes, user profile or D: local workspace changed.

## Validation gates

- CI [run 39447056961](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39447056961): **PASS on code head `52916c77`**, including Windows and Ubuntu rustfmt, strict Clippy, entire workspace tests and release builds.
- Task DAG [run 39447057301](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39447057301): **PASS on the same code head**.
- Current `main` moved to `7942906cc845a02df4629de3b7830f2f729573d4` after disjoint CL-06A acceptance; its Rust and DAG workflows both PASS ([main CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39449604269)). GitHub reports PR #20 mergeable with no reported conflict; reverify at merge time.
- This evidence precedes documentation closeout commits; require fresh exact-head CI on the final PR SHA.
- **Independent review / merge / post-merge CI:** NOT DONE. Do not mark the task accepted solely from CI or self-review.
- Native gate: `CL-L3` only after both `CL-06B` and `CL-07B`; user gate: none for CL-07A.
- Overall Phase 1–6 backend parity: PARTIAL, not full desktop equivalence.

## Follow-up

Mark PR #20 Ready for Review on completion of documentation and exact-head CI; request an actual independent reviewer rather than representing the author audit as independent. Reconcile with other current cloud PRs before merge. On accepted merge update `docs/task-registry.json` CL-07A to `done`, the parallel execution board, `docs/CURRENT-PHASE.md`, and [Notion CL-07A](https://app.notion.com/p/3ffd8330eb4681848b58c848a94f16c9) from Claimed/Review to Done with exact merged SHA, then rerun readiness and claim CL-07B separately. Do not edit concurrently claimed shared planning files inside PR #18/#19 to record this unmerged state; the durable [CL-07A claim](../claims/CL-07A.md) and PR are the active source.
