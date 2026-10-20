# CL-04B — Cloud claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-04B
- AGENT / SESSION: ChatGPT GPT-6, current cloud GitHub implementation conversation, 2026-10-20
- CLAIM_VERSION: 1 (coordination only; no atomic lock controller)
- BASE_SHA: 1232803d9dba3ba05cca11cfc8101bb54bd51cc2 (CL-04A PR #12 merged)
- BRANCH: work/cl-04b-history-recovery-20261020
- RESOURCE_LOCKS: session-store, history replay/checkpoint and bounded history reads
- DEPENDENCIES: CL-04A merged; post-merge main CI awaiting final result at claim
- STATUS: RELEASED / MERGED (2026-10-20); cloud code accepted at PR head Windows/Ubuntu CI
- EXPECTED: prevent crash/metadata replay from corrupting sequence authority, enforce safe large-session read budgets, preserve non-destructive retention; add restart/fault/limits tests and source-compatible evidence
- LOCAL_REQUIRED: NONE directly for this C unit
- YOU_REQUIRED: NO
- CHECKPOINT_SHA: 39b80f3342f3885bb523136974395d9f7069e224 (PR-head Windows/Ubuntu PASS)
- PR: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/13
- MERGE_SHA: ce065f8b449ff4a215f1169ca8efd9158760dbca
- NEXT: CL-04C; independent resource locks required for parallel lanes
