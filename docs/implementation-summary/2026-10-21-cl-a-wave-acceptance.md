# CL-05A / CL-08A / CL-09A — merged cloud acceptance ledger

Acceptance date: 2026-10-21. Canonical source: [CoSoS](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids).

- **CL-05A Done:** [PR #18](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/18), accepted code merge `b35ba3892345712ff4c481b6339e9b6bfd873790`, reconciled exact head `3adb8527a8d5d0a835a35c5374a528455aab0dba`, [Windows/Ubuntu CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39476380751) PASS; logical lock `process-supervisor` released.
- **CL-08A Done:** [PR #23](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/23), accepted code merge `9abe84d9265c994ad1213330a1efa080409c8cf3`, reconciled exact head `1744a96bc54b1a8c4d614d7c6175690d6bc7c098`, [Windows/Ubuntu CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39476137680) PASS; logical lock `runtime-secrets` released.
- **CL-09A Done:** [PR #21](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/21), accepted code merge `26f0ac407b5efab9c234ad78af410930d2f07c36`, reconciled exact head `768df800e2dd7683c4c3bb06f3ea0fc2ed191c8f`, [Windows/Ubuntu CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39476431442) PASS; logical lock `mcp-protocol` released.

- CL-06A accepted via PR #19/#22. CL-07A accepted via PR #20/#24.
- All five A-units now exist on merged main; this is scoped Rust backend parity only.
- Shared historical roadmap/claim edits from parallel PRs were intentionally reconciled against live main without overwriting distinct claims. No force-push, source data erase or Windows D: checkout modification.
- New six candidate units: `CL-05B`, `CL-06B`, `CL-07B`, `CL-08B`, `CL-09B`, `CL-09C`. CL-09C prerequisites are CL-09A and CL-05A, **not** CL-09B. L gates are still pending.
- A merge into main is not completion of the full 115-node DAG or entire desktop app.
