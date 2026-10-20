# CL-06B — active cloud claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-06B
- AGENT / SESSION: ChatGPT GitHub-connected cloud / chatgpt-20261021-cl06b-01
- CLAIM_VERSION: 1; no automatic atomic lock; do not transfer without fencing
- STATUS: CLAIMED / IN PROGRESS (not accepted)
- BASE_SHA: ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e
- BRANCH: work/cl-06b-sftp-containment-20261021
- RESOURCE_LOCKS: sftp
- DEPENDENCIES: CL-06A merged and accepted
- EXCLUSIVE WRITE SCOPE: crates/cos-tools/src/remote.rs and focused SFTP tests
- CHECKPOINT_SHA: pending first implementation checkpoint
- ACCEPTANCE: Remote path/handle confinement, transport trust, bounded SFTP behavior and stable error mapping. Local CL-L3 remains separate.
- NOTION: https://app.notion.com/p/3ffd8330eb46818684e5f9a116e56336
- LOCAL REQUIRED: CL-L3; Windows native gate NOT RUN
- OTHER AGENTS: Do not edit this unit or the overlapping authority while this claim is active. Both requested B units have independent logical locks and worktrees.
