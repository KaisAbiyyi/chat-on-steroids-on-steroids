# CL-06A — Git/project/file-panel/watcher claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-06A
- AGENT / SESSION: Codex cloud implementer / codex-20261020-cl06a-01
- CLAIM_VERSION: 1 (owner-directed, serialized project-files writer; not an atomic mutex)
- BASE_SHA: 340cf55eddb2766c96e1e1f36eaeb118faecad04; main Rust Windows/Linux and DAG CI PASS
- BRANCH: cl-06a/project-files-20261020
- RESOURCE_LOCKS: project-files (cos-tools Git, project panel, watcher contracts and tests)
- DEPENDENCIES: CL-L1 PASS; registry READY_CANDIDATE; CL-05A PR #18 holds disjoint process-supervisor
- STATUS: RELEASED / MERGED, 2026-10-20 Asia/Jakarta; project-files claim free
- CHECKPOINT_SHA: 9c42e87589c02b235880ef3a409c234a2870ff3f (qualified final head)
- MERGE_SHA: dd45a2585e8371bb51c1e00e84c5796a1b3dc163
- PR: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/19
- NOTION: https://app.notion.com/3ffd8330eb4681c3bf09e0c3511376d8 (existing task, cloud Done; CL-L3 separate)
- ACCEPTANCE: source-linked diff/preview/save/binary/symlink/watcher regressions; fmt, strict Clippy, workspace tests, Windows/Linux CI and independent review
- LOCAL_REQUIRED: CL-L3 after CL-06B and CL-07B; no native/device qualification now
- YOU_REQUIRED: NO
- TESTS: Windows 279/279 and Ubuntu 272/272 PASS, format/strict Clippy/release/DAG PASS on exact final head; independent review PASS
- EVIDENCE: docs/implementation-summary/2026-10-20-cl-06a-project-files.md; audit/2026-10-20/cl-06a/cloud-receipt.json
- NEXT: CL-06B new claim after verifying current main CI; CL-L3 after CL-06B/CL-07B
