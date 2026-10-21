# CL-10 — integrated phase 1–6 Rust backend closure audit

**Date:** 2026-10-21 (Asia/Jakarta). **Unit:** CL-10. **Environment:** GitHub cloud only.
**Claim:** `docs/claims/CL-10.md`; branch `work/cl-10-integrated-backend-20261021`; draft [PR #37](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/37).
**Accepted integration baseline:** GitHub main `d46c5cc98a87223ddacda8957a23b6714cb58061` (merges CL-L5 #36).
**Audit status:** implementation and integrated cloud tests submitted; **independent reviewer sign-off, exact final-head CI and merge not yet accepted**. This is a review candidate, not a release certificate.

## Boundary and required verdict

CL-10 covers the *currently implemented* Rust backend/service portion of phases 1–6: durable sessions and recorder, process/PTY/SSH, project/files/search/SFTP, configuration/runtime and MCP/control/code workers. It does **not** close original phase-1 embedded-browser/extension feasibility, browser-authenticated provider actions, desktop UI/installer, user-approved private profile migration, actual external hosts or tunnel deployment.

Required prerequisite statuses at claim: `CL-04B`, `CL-04C`, `CL-L2`, `CL-L3`, `CL-L4`, `CL-L5` **Done**. `CL-L6` is subsequent native integrated Windows acceptance, **not substitutable** by cloud CI.

## Contract inventory and audit trail

| Unit(s) | Implementation boundary / risk | Existing explicit regression / evidence |
| --- | --- | --- |
| CL-01/02/03 | Recorder redaction before durable append/assets; structured outcomes; caption timeline | `cos-store/tests/recorder_privacy.rs`, `cos-mcp/tests/recording_evidence.rs`, `cos-store/tests/captions.rs`; per-unit summaries |
| CL-04A/B/C | Session authority, restart/retention replay, UTF-16/JSON edge decisions | `cos-store/tests/cl_04a_authority.rs`, `cl_04b_replay.rs`, `cl_04c_json_edges.rs`; phase gate summaries |
| CL-05A/B + CL-L2 | Process ownership, PTY/shell/SSH custody and native Windows behaviors | `cos-exec/tests/cl_05a_custody.rs`, `cmd_pty.rs`, `terminal.rs`, `env.rs`; `audit/2026-10-21/cl-l2-ea8ab7a-receipt.json` |
| CL-06A/B, CL-07A/B + CL-L3 | Files, roots and revocation, SFTP, search/regex streaming | `cos-tools/tests/project_files.rs`, `sftp_native.rs`, `cl_07b_deadline.rs`; `audit/2026-10-21/cl-l3-ce7d2ef-receipt.json` |
| CL-08A/B + CL-L4 | Explicit runtime/profile/config and DPAPI, diagnostics, shutdown | `cos-core/tests/cl_08a_startup.rs`, `cl_08b_diagnostics.rs`, `cos-app/tests/cli.rs`; `audit/2026-10-21/cl-l4-048e327-receipt.json` |
| CL-09A/B/C + CL-L5 | MCP schema/surface grants, loopback control, resource-limited code worker | `cos-mcp/tests/cl09a_conformance.rs`, `http.rs`, `code.rs`; `audit/2026-10-21/cl-l5-bc4ba86-receipt.json` |
| CL-10 additive | Cross-boundary privilege change, committed-vs-denied patch evidence and cold session replay; recorder secret redaction across service surfaces | `cos-mcp/tests/cl10_backend_closure.rs` — synthetic temp profile, single process; Windows/Ubuntu hosted CI required |

These are *coverage pointers*, not a claim that a source-oracle fixture proves every present consumer behavior. Reviewers should consult the implementation summaries and SHA-bound local receipts, including their excluded cases and corrected harness assumptions.

## Integration abuse-case matrix

1. **Privilege/ownership downgrade:** a tool exposed before capability/root revocation must refuse subsequent read/write; unused `Desktop` / `Plugins` surfaces must not inherit `Core` grants. New integrated test verifies refused mutations, unchanged physical test file and cold replay.
2. **Trusted effects vs claimed effects:** only the successfully committed patch should publish a nonempty `changes` receipt. A later refused patch must persist an empty list, not project a successful mutation. New integrated test checks positive and negative outcomes after reopening the session store.
3. **Storage confidentiality:** synthetic environment token-like arguments must be redacted prior to persistence. Test inspects event output and the reopened event stream; existing CL-01 asset tests separately inspect overflow assets and raw session journals.
4. **Service ingress:** Rust MCP and local control are separate listeners with host/origin/auth checks and bounded requests. Existing `cos-mcp/tests/http.rs` and native CL-L5 proof establish selected synthetic loopback scenarios; no cloud tunnel or private-account test is claimed.
5. **Process/worker admission and cancellation:** resource budget constraints are tested in `cos-mcp/tests/code.rs` and process custody tests; **already-admitted native side effects cannot be rolled back when code execution times out**. This remains a documented operational limit.
6. **Filesystem isolation:** approved roots and project/SSH identity are checked at use. Native CL-L3 covers Windows junction escape and real SFTP server over local process pipes. Host-trusted SSH server paths remain an explicit opt-in: do not promote this to hostile external-server containment proof.
7. **Restart/secret lifecycle:** separate local CL-L4 DPAPI/config/daemon samples and CL-L5 restarted control-token rotations exist. These receipts are scoped to their tested GitHub SHAs; CL-L6 still needs fresh integrated release proof at the final accepted merged SHA.

## Reproducible verification

GitHub Actions workflows `.github/workflows/ci.yml` and `.github/workflows/task-graph.yml`, using exact PR head:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
python tools/task_board.py check
python -m unittest tools/test_task_board.py
```

Use the **actual workflow step names** to reconcile the DAG command above; if a workflow does not execute a listed command, mark it NOT RUN rather than claiming success. Required CI: both Windows and Ubuntu Rust matrix jobs, plus Task DAG workflow. Report final head SHA and canonical run URLs after completion; intermediate green runs do not satisfy exact-head acceptance.

## Review checklist — independent sign-off required

- Inspect live PR patch for regressions and hidden test-only assumptions; confirm no bypass of `SessionStore` redaction/recorder or live permission checks.
- Confirm raw logs, local user profiles, binaries, credential and private sources are absent from public PR artifacts.
- Confirm cargo formatter, strict Clippy, complete tests, release build, DAG and final PR SHA match on both OS runners.
- Check known limitations (side-effects-after-timeout, SSH trust, external tunnel, browser/GUI/installer) remain explicit and no phase 1–6 *whole-product* parity claim slipped into the status.
- Record reviewer identity, independent findings, exact accepted PR head, merge SHA and CI run URLs. Only after review/owner-qualified merge mark `CL-10` done, release `backend-closure`, and unblock `CL-L6`.

## Claim-and-merge receipt (fill only with verified remote evidence)

```text
UNIT: CL-10
BASE: d46c5cc98a87223ddacda8957a23b6714cb58061
PR: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/37
HEAD: PENDING FINAL VERIFY
WINDOWS CI: PENDING
UBUNTU CI: PENDING
TASK DAG CI: PENDING
INDEPENDENT REVIEW: NOT YET EVIDENCED
MERGE: NOT DONE
STATUS: IN PROGRESS / REVIEW CANDIDATE
NEXT: exact-head CI + independent sign-off -> merged accepted CL-10 -> native CL-L6
```
