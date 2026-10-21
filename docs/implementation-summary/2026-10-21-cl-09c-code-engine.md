# CL-09C — code engine resource admission and nested tool custody

## Ownership and scope

- UNIT: CL-09C; OWNER_SESSION: `chatgpt-20261021-cl09c-01`; CLAIM_GENERATION: 1
- BASE_SHA: `ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e`
- BRANCH: `work/cl-09c-code-engine-20261021`; PR: [#31](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/31) (draft pending CI/review)
- Exclusive logical resource: `code-engine`; no local D: checkout, real user data, or pinned upstream modification.

## Implemented / checkpoint

- `crates/cos-mcp/src/code.rs`: reject invalid CPU, wall clock and JavaScript heap budgets before an in-process execution starts; malformed nested arguments now produce an explicit refusal reason.
- `crates/cos-mcp/src/worker.rs`: apply the same admission check before spawning an isolated code worker; reject duplicate/zero/non-integer tool IDs and non-object arguments before dispatch.
- Enforce cumulative 32 MiB nested tool results in addition to the per-result 12 MiB limit; unresolved nested operations cannot be reported as successful on an incoming worker done frame. Existing OS process tree/memory bounds and invocation provenance are retained.
- `crates/cos-mcp/tests/code.rs`: reject invalid resource budgets with zero tool side effects, reject unawaited and over-limit nested operations. Removed a long-running fixture sleep after it caused unrelated tests to contend for the global capacity permit.

## Verification status (update with exact final head)

- GitHub Task DAG validation on earlier implementation head: PASS.
- First Rust Windows/Ubuntu run: FAIL at formatting; formatting corrected.
- Second run: Ubuntu Rust test global BUSY contention; corrected on isolated branch.
- Latest exact-head Windows and Ubuntu CI: PENDING at this checkpoint. No PASS inferred from an older run; check the PR Actions tab before marking REVIEW or DONE.
- Git diff, security review and merge: PENDING.

## Limitations and next gate

- Side effects of already-admitted native tool calls cannot be rolled back on JavaScript worker timeout. Session/grant cancellation still must be checked at each nested host invocation; a global code-worker kill is not equivalent to undoing a dispatched native mutation.
- Native Windows CL-L5, child process/device proof and independent security review: NOT RUN. Only merged exact-head CI and independent review can finalize this cloud task.

NEXT: obtain exact-head cargo fmt, strict Clippy, workspace tests, build, DAG validation and independent review; merge only after acceptance. CL-L5 remains BLOCKED until CL-09B AND CL-09C are accepted; then run explicit native Windows proof on D:.
