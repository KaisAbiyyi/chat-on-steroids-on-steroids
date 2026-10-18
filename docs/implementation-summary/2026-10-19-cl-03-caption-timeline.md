# CL-03 — Activity captions and durable timeline (2026-10-19)

**Cloud unit:** [PR #6](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/6), based on `main` at `7d5bc0340cf6f6119524c548a9c2a49a3f08dd1d`. Acceptance requires exact PR head Windows/Ubuntu fmt, strict Clippy, full workspace tests and release build; unmerged work is not marked DONE.

## Producer and persistence

- `caption::completed` / `caption::running` are pure source-oriented caption projections for actual tool names, arguments and CL-02 **structured** execution evidence. Taxonomy includes file read/search/edit/create/delete/move, shell process, desktop screen/input/clipboard, session, planning, agents and unknown tools.
- `Recorder::record_call_details` takes CL-01 **sanitized** args and result text, constructs `call.summary` and `call.runningSummary`, then stores both alongside the stable `tool_call` event *before* the append. Actual `call.evidence` remains bounded and sanitized by CL-02.
- `SessionStore` continues to derive `lastToolActivity`, timestamps, outcome counts and chronology from persisted events. No synthetic turn or change event is inserted; tool provenance, seq and nested call identity are unaltered.
- Real running process outcomes are not invented: `started`, nonzero exit and timeout captions come only from native evidence. When unknown, no success verdict is fabricated. Error and rejected captions are distinguished.
- Raw clipboard, environment values, keys and binary payloads must never be composed into captions. `resultHead` comes exclusively from CL-01-sanitized output; the full result remains in its original persisted stored-text boundary.

## Differential acceptance

Pinned [source corpus](../../fixtures/upstream/phase1-6-closure/session-summaries.json) contains **343 source-generated captions and running states**. The gate compares **338 representable** source cases exactly, including outcome precedence, process timing, search metrics, patch mismatch and multi-action computer summaries.

**Five source-only JS UTF-16 titles** involving unpaired surrogates are explicitly out of this Rust `String` comparison: they are neither rewritten to U+FFFD nor presented as exact parity. Preserve this gap for `CL-04C` (lossless/rejected UTF-16 decisions). Do not silently lower accepted-case count or alter source expected vectors.

End-to-end regression additionally verifies sanitized arguments/results, evidence, stable events, `lastToolActivity` and process nonzero counters survive a true cold `SessionStore` reopen.

## Gate boundary and handoff

- **LOCAL REQUIRED for CL-03:** NO. The **next** gate is `CL-L1` [L] only after the cloud PR is merged. User initiates a distinct local Windows agent qualification session against the exact merged SHA. The cloud agent must **never** use MCP CoS to inspect the device workspace.
- **YOU REQUIRED for CL-03:** NO. `CL-L1` requires a user-initiated local handoff, not manual end-user acceptance. Do not open a private browser/account profile without separately scoped authorization.
- `CL-L1` smoke-check: isolated Windows recorder tool calls for safe inline/overflow and poisoned secret args; live PTY/shell result nonzero/running/completed; activity caption and timeline before/after clean restart; verify disk journal/assets contain no sentinels; record exact SHA and evidence. Do not use actual personal tokens or private profile.
- **Next C:** `CL-04A` after `CL-L1` qualification passes, as defined by the cloud-first phase matrix.
