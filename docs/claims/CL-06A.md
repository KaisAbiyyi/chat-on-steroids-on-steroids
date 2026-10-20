# CL-06A — Git/project/file-panel/watcher claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-06A
- AGENT / SESSION: Codex cloud implementer / codex-20261020-cl06a-01
- CLAIM_VERSION: 1 (owner-directed, serialized project-files writer; not an atomic mutex)
- BASE_SHA: 340cf55eddb2766c96e1e1f36eaeb118faecad04; main Rust Windows/Linux and DAG CI PASS
- BRANCH: cl-06a/project-files-20261020
- RESOURCE_LOCKS: project-files (cos-tools Git, project panel, watcher contracts and tests)
- DEPENDENCIES: CL-L1 PASS; registry READY_CANDIDATE; CL-05A PR #18 holds disjoint process-supervisor
- STATUS: REVIEW, 2026-10-20 Asia/Jakarta; final exact-head CI/merge pending, project-files claim retained
- CHECKPOINT_SHA: d8b82faf4b23fa55c0ea0a333e7a5929605f512a (code/review repair durable)
- PR: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/19
- NOTION: https://app.notion.com/3ffd8330eb4681c3bf09e0c3511376d8 (existing task, Review)
- ACCEPTANCE: source-linked diff/preview/save/binary/symlink/watcher regressions; fmt, strict Clippy, workspace tests, Windows/Linux CI and independent review
- LOCAL_REQUIRED: CL-L3 after CL-06B and CL-07B; no native/device qualification now
- YOU_REQUIRED: NO
- TESTS: 272/272 workspace PASS, format/strict Clippy/DAG PASS; independent review PASS after watcher repair
- EVIDENCE: docs/implementation-summary/2026-10-20-cl-06a-project-files.md; audit/2026-10-20/cl-06a/cloud-receipt.json
- NEXT: exact-head Windows/Linux CI, merge and exact-SHA release handoff; then CL-06B
