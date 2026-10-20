# CL-09A — MCP protocol, schema and permission conformance

**Date:** 2026-10-20 (Asia/Jakarta)  
**Unit:** CL-09A (Phases 1–6 backend closeout)  
**Cloud agent/session:** ChatGPT, `chatgpt-20261020-cl09a-01`, claim generation 1  
**Lock:** `mcp-protocol`; [durable claim](../claims/CL-09A.md)  
**Base:** `340cf55eddb2766c96e1e1f36eaeb118faecad04`  
**Review PR:** [#21](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/21) (branch `work/cl-09a-mcp-conformance-20261020`)  
**Task board (Notion):** [CL-09A](https://app.notion.com/p/3ffd8330eb4681ed926aebfbbbcbaa58), [CoSoS project](https://app.notion.com/p/3ffd8330eb46816c80caeaa41e6a1a28)

## Contract / independent scope

- Dependency `CL-L1`: accepted native Windows proof. `CL-09A` is a cloud unit; `CL-L5` remains a separate real Windows test **after** `CL-09B` and `CL-09C`.
- Logical protocol lock is separate from concurrent `CL-05A` PR #18 (`process-supervisor`) and `CL-06A` PR #19 (`project-files`). No edits to the Windows D: checkout, live user profiles, upstream pinned source or existing historical evidence.
- Source oracle: `fixtures/upstream/phase4-6/mcp-protocol.json` (36 SDK cases) and `tool-declarations.json`. Existing oracle is preserved unchanged.

## Changes

1. `crates/cos-mcp/src/protocol.rs`: fail with JSON-RPC `-32602` for non-object named `params`, before tool listing/call dispatch. Previously an array/scalar could be silently interpreted as a request with absent params.
2. `crates/cos-mcp/src/http.rs`: return JSON-RPC `-32700` parse-error envelope for malformed JSON, rather than an untyped transport error.
3. `crates/cos-mcp/src/http.rs`: reject repeated MCP protocol, method or name authority headers before collection into a map could discard contradictory claims.
4. New `crates/cos-mcp/tests/cl09a_conformance.rs` tests enforce malformed params, cross-surface tool isolation, argument validation, immediate permission revocation without content leaks, wire-level parse errors and conflicting authority headers.
5. Canonical `docs/task-registry.json`, executable DAG assertions, [parallel board](../PARALLEL-EXECUTION-BOARD.md), [current state](../CURRENT-PHASE.md) and task claim track `CL-09A` as `claimed`. Claim is not acceptance or a lock implemented by the DAG alone.

## Verification ledger

- Initial test checkpoint `21b16bb7`: expected RED; Rust CI format failed before tests because new source was not formatted.
- Repair commits `f06d6399` and `f42d9f4`: protocol/HTTP behavior. `126104c8`: Rustfmt corrections applied directly from CI diagnostics.
- Later documentation/claim ledger checkpoint: inspect [PR #21](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/21) for latest exact SHA.
- `cargo fmt --all --check`: PASS observed on in-progress Windows/Ubuntu CI head `bfc9438e...`; final exact HEAD CI must be independently checked.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS observed through the same in-progress CI steps; full run pending.
- `cargo test --workspace --locked`, `cargo build --release --locked`: **PENDING final exact-head completion**. Do not infer success before job conclusion.
- Task DAG checks: Windows and Ubuntu **PASS** for `bfc9438e1d03379cfcc77339cea64478276e030a` ([run](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39447646203)).
- [Windows and Ubuntu Rust matrix](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39447646253) on `bfc9438e...`: RUNNING at this snapshot.
- Independent security/compatibility reviewer: **PENDING**; separate from implementing agent.
- Performance: NOT MEASURED; no latency/memory/throughput improvement claimed.

## Acceptance and next

The PR remains unmerged pending all exact-head gates and independent review. After accepted review, merge, record **actual merge SHA**, change `CL-09A` from `claimed` to `done`, release `mcp-protocol` lock and reconcile Notion Task status `Done` with exact CI/merge links.

Then `CL-09B` becomes eligible; `CL-09C` requires `CL-09A` **and** `CL-05A`. `CL-L5` is still BLOCKED pending both `CL-09B` and `CL-09C`. No owner action is required for CL-09A alone; original 16-phase product remains PARTIAL.
