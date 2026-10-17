# Read-only whole-scope review package

Review the current phase 4–6 service implementation in D:/projects/CoS-Rust. This root is not Git.
Use `phase4-6-source-manifest.json` as the frozen source identity. Do not invent a Git base/head.
The nested upstream clone is pinned and must remain unchanged. No subagents or source edits in this review.

Scope: new crates cos-exec, cos-tools, cos-mcp; cos-app integration; ConfigStore directory accessor;
Recorder parent-call context; oracle/proof scripts and current dated docs. Older foundation behavior
remains within its recorded partial qualification. Existing broad tests pass, but verify important
claims and inspect failure paths yourself. A green suite does not prove the original full-app gates.

Authority: docs/implementation-plan/2026-10-16-rust-migration-phases-v1.0.md phases 4–6;
docs/implementation-plan/2026-10-17-phase4-6-implementation-v1.0.md (tasks, Review focus and Ruling lines).
Known coverage/qualification limits: docs/research/2026-10-17-phase4-6-coverage-v1.0.md.
Current summary and review: docs/implementation-summary/2026-10-17-phase4-6-rust-services-v1.0.md;
docs/implementation-review/2026-10-17-phase4-6-review-v1.0.md.

Review focus, verbatim:

Check authority after awaited calls; process IDs and model-provided conversation IDs are not custody.
Check link/reparse replacement, file-handle verification, stale/multi-file rollback and encoding edges.
Check body/chunk/request bounds, secret-path/Origin/Host behavior, disconnect vs accepted operations,
monotonic discovery, cross-surface leakage and JavaScript timeout after accepted side effects.
Check native PTY lifecycle, unread completion, foreign stdin, output/resource bounds, cleanup and SSH
quoting. Distinguish tested protocol adapters from external tunnel/SSH and cross-platform evidence.

Evidence: audit/2026-10-17-phase4-6-rust-tests.log (107 PASS), strict Clippy log, release build log,
audit/2026-10-17/phase4-6-runtime.json (fresh loopback proof), release receipt, upstream fixture manifest.
An additional unique upstream suite run may still be finishing; it is original-code evidence only.

Return concrete findings with severity, absolute file path/line and reproducible trigger. Grade by
effect on the person, not just spec coverage. Include strength, plan alignment and a verdict.
List every behavior considered but declined to judge and why; the implementer will rule on each.
Do not dispatch another reviewer. You are the one fresh whole-scope reviewer; the implementer gets
one bounded important-fix pass with failing regressions and a full suite, without re-review.
