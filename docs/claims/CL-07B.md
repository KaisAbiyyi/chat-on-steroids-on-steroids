# CL-07B — Ripgrep/JS regex streamed search exactness

- **STATUS:** CLAIMED / IN PROGRESS (not accepted).
- **AGENT:** ChatGPT connected GitHub cloud; session `chatgpt-20261021-cl-07b-01`; claim generation 1.
- **BASE SHA:** `ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e`; **BRANCH:** `work/cl-07b-search-stream-20261021`.
- **RESOURCE LOCK:** `search-stream`, independent of other CL-B agents. No automatic shared-file writes before review.
- **SCOPED PATHS:** crates/cos-tools/src/search/{engine,scheduling}.rs; crates/cos-tools/tests/cl_07b_stream.rs; search library boundary only; no project Git/SFTP files.
- **DEPENDENCY:** CL-07A accepted/merged and registry done; Notion task: https://app.notion.com/p/3ffd8330eb4681c9ac0bdf5ec1c51f2b.
- **CLOUD ACCEPTANCE:** focused RED→GREEN regressions; full Windows+Ubuntu Rust formatter, Clippy, tests, release + DAG CI on final exact SHA; independent security/correctness review before merge. No native proof asserted.
- **LOCAL REQUIRED:** CL-L3 (also depends on CL-06B) after cloud acceptance; owner U not requested.
- **NEXT:** implementation on this branch; draft PR and synchronize Notion status, tests, summary. Other agents must not modify scoped contract without explicit fenced handoff.
