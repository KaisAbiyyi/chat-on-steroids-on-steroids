# CL-09A — MCP schema/error/permission conformance

- TASK_ID: CL-09A
- CLAIM_STATUS: CLAIMED / RUNNING; generation 1
- AGENT: ChatGPT (GitHub-connected cloud agent)
- SESSION: chatgpt-20261020-cl09a-01
- REPO: KaisAbiyyi/chat-on-steroids-on-steroids
- BRANCH: work/cl-09a-mcp-conformance-20261020
- BASE_SHA: 340cf55eddb2766c96e1e1f36eaeb118faecad04
- RESOURCE_LOCKS: mcp-protocol
- DEPENDENCIES: CL-L1 (accepted and merged)
- SCOPE: `crates/cos-mcp/src/protocol.rs`, `schema.rs`, `kernel.rs` only when necessary, focused MCP tests, and CL-09A-specific evidence and handoff. Avoid CL-09B transport, CL-09C code-engine, CL-05A process-supervisor and CL-06A project-files edits.
- PARALLEL: PR #18 owns process-supervisor; PR #19 owns project-files. Never rebase or modify either branch in this unit.
- CHECKPOINT: first branch claim file; subsequent durable code checkpoints tracked in PR. Do not infer native Windows PASS from GitHub CI.
- ACCEPTANCE: compare source contract/tool lists for core/desktop/plugins; reject malformed request/schema, missing/unauthorized operations and cross-surface access without side effects; preserve protocol error envelopes and negative regressions. Rust fmt, Clippy, workspace tests and Windows/Ubuntu CI on exact head. Independent security review remains a requirement.
- LOCAL_REQUIRED: CL-L5 only AFTER CL-09B and CL-09C; NOT RUN during CL-09A.
- HUMAN_REQUIRED: NO.
- NEXT: inspect source contract and implement bounded conformance regressions. Do not mark DONE until CI and independent review are accepted.

Other agents: this claim owns the MCP protocol/permission logical contract until explicitly released. The GitHub branch/PR, not the Notion status field, is the durable checkpoint.


## Accepted merge and claim release — 2026-10-21

- GitHub [PR #21](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/21) merged as `26f0ac407b5efab9c234ad78af410930d2f07c36` after independent scoped review.
- Exact reconciled code head: `768df800e2dd7683c4c3bb06f3ea0fc2ed191c8f`. [Windows and Ubuntu CI plus DAG](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39476431442) PASS.
- **Claim released:** `mcp-protocol` is no longer owned by this completed task. The local native gate is not implied.
- Acceptance registry and Notion status must track this scoped DONE state; no user private profile accessed.
