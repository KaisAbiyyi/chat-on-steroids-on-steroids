# CL-08B — Diagnostics/logging/awake/shutdown bounded behavior

- **STATUS:** CLAIMED / IN PROGRESS (not accepted).
- **AGENT:** ChatGPT connected GitHub cloud; session `chatgpt-20261021-cl-08b-01`; claim generation 1.
- **BASE SHA:** `ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e`; **BRANCH:** `work/cl-08b-diagnostics-20261021`.
- **RESOURCE LOCK:** `runtime-diagnostics`, independent of other CL-B agents. No automatic shared-file writes before review.
- **SCOPED PATHS:** crates/cos-core/src/{logging,runtime}.rs; crates/cos-core/tests/cl_08b_diagnostics.rs; no secrets/config/mcp-transport files.
- **DEPENDENCY:** CL-08A accepted/merged and registry done; Notion task: https://app.notion.com/p/3ffd8330eb46814287f3c46484504aa2.
- **CLOUD ACCEPTANCE:** focused RED→GREEN regressions; full Windows+Ubuntu Rust formatter, Clippy, tests, release + DAG CI on final exact SHA; independent security/correctness review before merge. No native proof asserted.
- **LOCAL REQUIRED:** CL-L4 after cloud acceptance; owner U not requested.
- **NEXT:** implementation on this branch; draft PR and synchronize Notion status, tests, summary. Other agents must not modify scoped contract without explicit fenced handoff.

## Implementation checkpoint — 2026-10-21
- Diagnostics: bounded redacted inputs, severity normalization, poisoned-lock recovery.
- Serve: cancellation before bounded services/sessions/durable shutdown, Windows thread-affine awake guard honoring ui.keepAwake / ui.keepScreenAwake.
- Exact-head CI and independent review NOT RUN; CL-L4 native NOT RUN; state remains In Progress.
