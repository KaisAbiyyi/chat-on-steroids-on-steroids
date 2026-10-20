# CL-08B — diagnostics and bounded runtime lifecycle (2026-10-21)

## Implementation
- DiagnosticLog normalizes severity to a known allowlist, omits inputs over 1 MiB instead of exposing a clipped credential, redacts accepted messages before clipping to 16,000 characters, and recovers poisoned mutexes.
- Runtime close cancels admission before three separately bounded service/session/durable teardown phases; records each status and refuses false success on timeout/error/panic.
- Windows serve acquires the thread-affine awake guard only when ui.keepAwake is enabled, and sets display keep-awake only when ui.keepScreenAwake is explicitly enabled.

## Tests, limits and handoff
- New synthetic tests: secret and oversized diagnostic suppression, severity/ring bounds, panic phase continuation, timeout continuation.
- Exact-head Windows+Ubuntu CI and DAG: NOT RUN until Actions execute. Independent reviewer: NOT RUN. Native CL-L4: NOT RUN.
- A timed-out thread cannot be forcibly unwound; reporting a timeout does not certify clean OS resource teardown. No user private data or local D: workstation touched.
- UNIT CL-08B; BRANCH work/cl-08b-diagnostics-20261021; PR #29; PERFORMANCE no measured claim; LOCAL REQUIRED CL-L4 only after code acceptance; YOU REQUIRED no; NEXT CI, review, merge then registry/board/Notion Done.
