# CL-09B exclusive cloud claim

- STATUS: CLAIMED / implementation in progress; not DONE or MERGED.
- OWNER_SESSION: chatgpt-20261021-cl09b-01
- CLAIM_GENERATION: 1
- BASE_SHA: ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e
- BRANCH: work/cl-09b-mcp-transport-20261021
- RESOURCE_LOCK: mcp-transport
- OWNED_SCOPE: crates/cos-mcp/src/{http,tunnel}.rs and focused transport tests; no code/worker/process-pty edits
- PLANNED_TESTS: loopback health/auth/reconnect, proxy handling, tunnel generation teardown, Windows+Ubuntu CI; cargo fmt --all --check, cargo clippy --workspace --all-targets --locked -- -D warnings, cargo test --workspace --locked, task registry validator
- PR: #30 — https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/30 (draft / review pending)
- LAST_PUSHED_CHECKPOINT_SHA: see live draft PR #30; CI exact head supersedes initial claim commit
- DEPENDENCIES: CL-09A accepted and merged
- EXCLUSIONS: no CL-05B/06B/07B/08B resource edits, no local D: checkout, no live user profile, no local CL-L5 proof.
- IMPLEMENTED: source code and regression tests pushed; see docs/implementation-summary/2026-10-21-cl-09b-mcp-transport.md.
- NEXT: record exact-head CI and independent review. CL-L5 remains blocked until both accepted.
