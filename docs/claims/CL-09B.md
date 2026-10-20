# CL-09B exclusive cloud claim

- STATUS: CLAIMED / implementation in progress; not DONE or MERGED.
- OWNER_SESSION: chatgpt-20261021-cl09b-01
- CLAIM_GENERATION: 1
- BASE_SHA: ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e
- BRANCH: work/cl-09b-mcp-transport-20261021
- RESOURCE_LOCK: mcp-transport
- OWNED_SCOPE: crates/cos-mcp/src/{http,tunnel}.rs and focused transport tests; no code/worker/process-pty edits
- PLANNED_TESTS: loopback health/auth/reconnect, proxy handling, tunnel generation teardown, Windows+Ubuntu CI; cargo fmt --all --check, cargo clippy --workspace --all-targets --locked -- -D warnings, cargo test --workspace --locked, task registry validator
- PR: pending; update after creation
- LAST_PUSHED_CHECKPOINT_SHA: first claim commit (record from GitHub result)
- DEPENDENCIES: CL-09A accepted and merged
- EXCLUSIONS: no CL-05B/06B/07B/08B resource edits, no local D: checkout, no live user profile, no local CL-L5 proof.
- NEXT: implement/test on isolated PR; record exact-head CI and independent review. CL-L5 remains blocked until both accepted.
