# CL-05B — active cloud claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-05B
- AGENT / SESSION: ChatGPT GitHub-connected cloud / chatgpt-20261021-cl05b-01
- CLAIM_VERSION: 1; no automatic atomic lock; do not transfer without fencing
- STATUS: CLAIMED / IN PROGRESS (not accepted)
- BASE_SHA: ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e
- BRANCH: work/cl-05b-pty-shell-ssh-20261021
- RESOURCE_LOCKS: process-pty
- DEPENDENCIES: CL-05A merged and accepted
- EXCLUSIVE WRITE SCOPE: crates/cos-exec/src/{process,shell,ssh,terminal,winpty}.rs and focused cos-exec tests
- CHECKPOINT_SHA: 3af7d344181783fbbbd4c50c0bc3c3267eefdf4e (pre-summary source checkpoint; final-head CI pending)
- ACCEPTANCE: PTY, shell, SSH lifecycle and native error semantics with cross-platform regression coverage. Local CL-L2 remains separate.
- NOTION: https://app.notion.com/p/3ffd8330eb468186801fef1b5136e786
- LOCAL REQUIRED: CL-L2; Windows native gate NOT RUN
- OTHER AGENTS: Do not edit this unit or the overlapping authority while this claim is active. Both requested B units have independent logical locks and worktrees.

## Implementation checkpoint — 2026-10-21

Source changes and regression cases are on this claimed branch. Detailed limitations and gates: [implementation summary](../implementation-summary/2026-10-21-cl-05b-pty-shell-ssh.md). Exact final-head Windows/Ubuntu CI, independent review, merge, task-registry acceptance and native gate all remain pending; no claim release.
