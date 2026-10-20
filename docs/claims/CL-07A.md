# CL-07A search/patch cloud claim

- **TASK:** CL-07A (C); prerequisite CL-L1 accepted, CL-07B blocked until completion.
- **REPOSITORY:** KaisAbiyyi/chat-on-steroids-on-steroids
- **AGENT / SESSION:** ChatGPT GitHub-connected cloud, chatgpt-2026-10-20-cl07a-01.
- **CLAIM GENERATION:** 1; resource lock: `search-patch` (traversal/excludes/order/limits + patch transaction/diagnostic behavior).
- **BASE_SHA:** `340cf55eddb2766c96e1e1f36eaeb118faecad04`; **BRANCH:** `work/cl-07a-search-patch-20261020`.
- **LIVE CONFLICT CHECK:** main CI Windows/Linux PASS; #18 CL-05A process-supervisor and #19 CL-06A project-files have disjoint owned source files; no other active PR was observed.
- **LOCKED CODE SCOPE:** `crates/cos-tools/src/search/engine.rs`, search-traversal parity tests, `crates/cos-tools/src/patch/updates.rs`, patch/transaction parity tests. Avoid `project_files.rs`, `git.rs`, process supervisor and other agents' contracts.
- **ACCEPTANCE:** source-aligned traversal ordering, exclusion and bound semantics; transactional refusals and unchanged bytes on failure; RED/GREEN regressions, rustfmt/strict Clippy/full workspace tests/release for Ubuntu+Windows, task-DAG CI, independent review and exact SHA.
- **CODE CHECKPOINT:** `a2140ab18ab01e0bdb9cfa286e309fc9d058b2bc`; implementation head before documentation closeout `52916c7764a2e425144d80f35a9b4811d74b0f99`. Five CL-07A regression tests; search denied-path and patch move/delete authority. **EXACT-HEAD CI PASS:** Rust Windows/Linux (fmt, strict Clippy, complete workspace tests, release) [39447056961](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39447056961) and Task DAG [39447057301](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39447057301). Documentation-only closeout commits require new exact-head checks. **Independent reviewer sign-off pending; no accepted merge.**
- **LOCAL REQUIRED:** none for this cloud unit; `CL-L3` occurs only after CL-06B and CL-07B; D: native checkout untouched.
- **NEXT:** PR #20 ready for independent review after exact CI; reviewer must verify permission semantics, traversal exclusions/ordering/bounds, transaction rollback and source parity; then merge against current green main and commit acceptance ledger (registry/board/current phase) without overwriting CL-05A or CL-09A claims. CL-07B stays blocked until accepted merge. Notion task is Review, not Done.
