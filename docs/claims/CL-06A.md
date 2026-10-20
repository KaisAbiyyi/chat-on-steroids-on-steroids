# CL-06A — Git/project/file-panel/watcher claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-06A
- AGENT / SESSION: Codex cloud implementer / codex-20261020-cl06a-01
- CLAIM_VERSION: 1 (owner-directed, serialized project-files writer; not an atomic mutex)
- BASE_SHA: 340cf55eddb2766c96e1e1f36eaeb118faecad04; main Rust Windows/Linux and DAG CI PASS
- BRANCH: cl-06a/project-files-20261020
- RESOURCE_LOCKS: project-files (cos-tools Git, project panel, watcher contracts and tests)
- DEPENDENCIES: CL-L1 PASS; registry READY_CANDIDATE; CL-05A PR #18 holds disjoint process-supervisor
- STATUS: CLAIMED / RUNNING, 2026-10-20 Asia/Jakarta
- CHECKPOINT_SHA: 476da26e181915152d1b2db50bdef2e0731d140d (durable claim); implementation checkpoint follows
- PR: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/19
- NOTION: https://app.notion.com/3ffd8330eb4681c3bf09e0c3511376d8 (existing task, In Progress)
- ACCEPTANCE: source-linked diff/preview/save/binary/symlink/watcher regressions; fmt, strict Clippy, workspace tests, Windows/Linux CI and independent review
- LOCAL_REQUIRED: CL-L3 after CL-06B and CL-07B; no native/device qualification now
- YOU_REQUIRED: NO
- NEXT: inspect pinned contracts and callers, RED/GREEN regressions, safe pushed checkpoint and exact-SHA handoff
