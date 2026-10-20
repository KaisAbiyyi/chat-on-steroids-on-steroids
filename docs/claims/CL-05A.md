# CL-05A — process supervisor custody claim

- **Task:** `CL-05A` (cloud, phase 1–6), prerequisite `CL-L1` accepted on `a00edd4`.
- **Agent/session:** ChatGPT GitHub-connected cloud coding, `chatgpt-2026-10-20-cl05a-01`.
- **Claim:** `CL-05A / KaisAbiyyi/chat-on-steroids-on-steroids / chatgpt-2026-10-20-cl05a-01 / work/cl-05a-process-custody-20261020 / base 340cf55eddb2766c96e1e1f36eaeb118faecad04 / claim generation 1`.
- **Exclusive logical lock:** `process-supervisor`; primary file scope `crates/cos-exec/src/process.rs`, process custody regression tests and task documentation. Other agents must avoid overlapping process ownership/cancellation/publication contracts until released.
- **Checkpoint:** this branch claim commit; implementation/tests to follow.
- **Acceptance:** cross-owner refusal, termination and timeout correctness, unread completion + rolled-back output publication without duplicating process execution, no orphaned child or released ownership while still running; Windows + Ubuntu CI format/clippy/test/release.
- **State:** CLAIMED. Registry `todo` remains unaccepted until merged evidence. `CL-05B`, `CL-L2` remain blocked.
- **No local changes:** Windows D: checkout untouched. `CL-06A/07A/08A/09A` are independent candidates only after own live claim checks.
