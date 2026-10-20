# CL-04B — Cloud claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-04B
- AGENT / SESSION: ChatGPT GPT-6, current cloud GitHub implementation conversation, 2026-10-20
- CLAIM_VERSION: 1 (coordination only; no atomic lock controller)
- BASE_SHA: 1232803d9dba3ba05cca11cfc8101bb54bd51cc2 (CL-04A PR #12 merged)
- BRANCH: work/cl-04b-history-recovery-20261020
- RESOURCE_LOCKS: session-store, history replay/checkpoint and bounded history reads
- DEPENDENCIES: CL-04A merged; post-merge main CI awaiting final result at claim
- STATUS: CLAIMED / RUNNING
- EXPECTED: prevent crash/metadata replay from corrupting sequence authority, enforce safe large-session read budgets, preserve non-destructive retention; add restart/fault/limits tests and source-compatible evidence
- LOCAL_REQUIRED: NONE directly for this C unit
- YOU_REQUIRED: NO
- NEXT: implement on this branch, PR + Windows/Ubuntu CI, do not merge without final validation
