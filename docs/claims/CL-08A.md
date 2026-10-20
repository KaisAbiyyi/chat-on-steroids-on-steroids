# CL-08A — Windows startup/config/secret-store lifecycle

- Task: `CL-08A`, phase 1–6, cloud unit; dependency `CL-L1` accepted.
- Agent/session: ChatGPT GitHub cloud / `chatgpt-20261021-cl08a-01`; claim generation 1; status CLAIMED.
- Repository: `KaisAbiyyi/chat-on-steroids-on-steroids`; branch `work/cl-08a-startup-secrets-20261021`; base `7942906cc845a02df4629de3b7830f2f729573d4`.
- Exclusive logical resource lock: `runtime-secrets` (Windows crypto, config startup, explicit profile migration).
- Other active claims: CL-05A PR #18 (`process-supervisor`), CL-07A PR #20 (`search-patch`), CL-09A PR #21 (`mcp-protocol`). Do not edit their source or logical contracts.
- Planned bounded scope: `crates/cos-core/src/secrets.rs`, `crates/cos-core/src/persistence.rs`, focused cross-platform and Windows-only tests, task-specific docs; no local Windows D: or private profiles.
- Acceptance: secure legacy direct DPAPI migration into new keyless Local State; malformed/encrypted/unreadable credentials refuse without creating new encryption keys; config I/O errors are not mistaken for corrupt input; no secret leakage; Windows and Ubuntu Rust fmt/Clippy/workspace tests/release CI; independent review.
- L gate: `CL-L4` only after `CL-08B` accepted, not performed by this cloud task. U gate: NONE.
- Claim proof: GitHub branch and PR; update Notion task `CL-08A` at https://app.notion.com/3ffd8330eb468111945ffde33dab0e83.
- Recovery: subsequent agents must check PR/claim/CI SHA and receive release or fenced transfer before modifying runtime-secrets.


## Accepted merge and claim release — 2026-10-21

- GitHub [PR #23](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/23) merged as `9abe84d9265c994ad1213330a1efa080409c8cf3` after independent scoped review.
- Exact reconciled code head: `1744a96bc54b1a8c4d614d7c6175690d6bc7c098`. [Windows and Ubuntu CI plus DAG](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39476137680) PASS.
- **Claim released:** `runtime-secrets` is no longer owned by this completed task. The local native gate is not implied.
- Acceptance registry and Notion status must track this scoped DONE state; no user private profile accessed.
