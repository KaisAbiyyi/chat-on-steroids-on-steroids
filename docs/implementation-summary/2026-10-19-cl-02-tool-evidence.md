# CL-02 — Trusted tool execution evidence (2026-10-19)

**Status:** cloud implementation PR [#4](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/4), pending current-head Windows/Linux CI review and merge. Parent dependency: CL-01 [PR #3](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/3) merged as `1052c6a`.

## Runtime recording contract

The public MCP response remains unchanged. The native Kernel removes the private `_recordingEvidence` field before returning or serializing the tool result. The recorder appends a normalized `call.evidence` projection of trusted values:

- `exitCode`, `running` and `processSessionId`: from `exec_command`/`write_stdin` structured process return values, including actual nonzero exit and still-running sessions; no deduction from stdout or description strings.
- `durationMs`: positive, bounded native wall-clock seconds converted to milliseconds; unknown stays null.
- `count`: captured directly in the native find engine before response formatting, including the explicit-file branch, and not synthesized from "N matches" text.
- `changes`: bounded committed patch report paths/kinds; also captured when exec_command intercepts an explicit apply_patch statement. Errors produce no claimed changes.
- `failed` and `timedOut`: based on the native result flags; unknown process exit is not converted into success.

Evidence paths are sanitized against the original tool arguments *before persistence*, inheriting CL-01's recorder privacy boundary. The raw private evidence is neither exposed as a new MCP field nor included in serialized `resultText`.

## Validation and limits

- Added unit tests for observed process state, error/unknown semantics, direct patch and match reports.
- Added cross-platform kernel tests using real isolated processes and actual local temporary search/patch operations executed *inside GitHub CI*; no personal Windows local machine is touched.
- Tested result shape compatibility, failed patch with no phantom changes, post-restart event evidence and intercepted apply_patch without phantom process exit.
- Full human-facing caption text/activity taxonomy belongs to CL-03. CL-02 persists input facts for those captions, not invented completion/success labels.
- Does not infer a process timeout if no structured timed-out flag is available, or mine arbitrary CLI text for untrusted patch paths. Unknown fields remain null.

**LOCAL REQUIRED:** NONE now; CL-L1 is after CL-03.  
**YOU REQUIRED:** NO.  
**NEXT:** CL-03 only after this PR is CI-green, independently reviewed and merged.
