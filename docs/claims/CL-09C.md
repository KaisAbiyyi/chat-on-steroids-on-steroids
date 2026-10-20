# CL-09C exclusive cloud claim

- STATUS: CLAIMED / implementation in progress; not DONE or MERGED.
- OWNER_SESSION: chatgpt-20261021-cl09c-01
- CLAIM_GENERATION: 1
- BASE_SHA: ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e
- BRANCH: work/cl-09c-code-engine-20261021
- RESOURCE_LOCK: code-engine
- OWNED_SCOPE: crates/cos-mcp/src/{code,worker}.rs and focused code-mode tests; no http/tunnel/process-pty edits
- PLANNED_TESTS: nested tool limits, admission, cancellation/revocation/output custody, Windows+Ubuntu CI; cargo fmt --all --check, cargo clippy --workspace --all-targets --locked -- -D warnings, cargo test --workspace --locked, task registry validator
- PR: pending; update after creation
- LAST_PUSHED_CHECKPOINT_SHA: first claim commit (record from GitHub result)
- DEPENDENCIES: CL-09A and CL-05A accepted and merged
- EXCLUSIONS: no CL-05B/06B/07B/08B resource edits, no local D: checkout, no live user profile, no local CL-L5 proof.
- NEXT: implement/test on isolated PR; record exact-head CI and independent review. CL-L5 remains blocked until both accepted.
