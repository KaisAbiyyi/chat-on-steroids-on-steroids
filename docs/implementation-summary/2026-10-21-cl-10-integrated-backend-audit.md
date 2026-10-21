# CL-10 — Independent integrated Rust backend/service closure audit

Date: 2026-10-21 (Asia/Jakarta)  
Claim: `chatgpt-20261021-cl10-cloud-01` / `backend-closure`  
Base: `d46c5cc98a87223ddacda8957a23b6714cb58061` (accepted CL-L5)  
Environment: GitHub cloud only — **no access to user-local workspace, D: artifacts or private profiles**.

## Decision and strict acceptance boundary

This unit independently inspects the phase 1–6 Rust backend already implemented by preceding agents. It adds a hermetic, **release-binary** full-process smoke contract to the GitHub Actions Windows/Ubuntu Rust matrix; its HTTP/control probes use only random ephemeral loopback ports and throwaway on-runner profiles. It is not a claim that the desktop product is finished, that every upstream integration is source-identical, or that CL-L6 native host qualification has happened.

**Disposition before final CI and review:** IMPLEMENTED / REVIEW; do not mark the task Done solely because code and this note exist. Require passing Windows and Ubuntu Rust CI at the exact PR head, task DAG CI, review and owner-qualified merge. If these pass and the PR is accepted, mark CL-10 Done and unlock **CL-L6** only; the native Windows integrated release receipt is independently outstanding.

## Cross-owner parity and security evidence matrix

| Backend boundary / mapped units | Inspected source contracts | Existing acceptance and important limitations |
| --- | --- | --- |
| Recorder privacy, tool evidence, captions/timeline (CL-01–03) | `cos-store/{redaction,evidence,caption,chronology,recorder}.rs`; runtime records through `cos-app` | Redaction before durable recorder storage, trusted tool outcomes and source-projected timeline are separate. CL-10 integration tests start a synthetic public conversation and verify durable restart. No authenticated browser-originated capture is claimed. |
| Session authority, replay, UTF-16 (CL-04A/B/C) | `cos-store/{access,store,correlation,model}.rs`; `cos-core` profile guard | Native store and earlier regression evidence accepted; CL-10 checks the public fixture through separate processes, export, list, history and a reopened service. Lossless UTF-16 decisions apply to their documented boundaries; whole browser/OS string fidelity remains separate. |
| Process/PTY/SSH custody (CL-05A/B) | `cos-exec/{process,terminal,ssh,policy}.rs`; `Kernel` | [CL-L2 receipt](../../audit/2026-10-21/cl-l2-ea8ab7a-receipt.json): 29 Rust + 2 isolated native SSH checks PASS on Windows source `ea8ab7a`. SSH server was a synthetic loopback fixture; no real user credentials or external SSH server. Command policy is a launch filter, **not an OS sandbox**. |
| Projects, Git, filesystem, SFTP, search and patch (CL-06A/B, CL-07A/B) | `cos-tools/{files,project,project_files,remote,search,patch}.rs`; MCP project tools | [CL-L3 receipt](../../audit/2026-10-21/cl-l3-ce7d2ef-receipt.json): 95 Windows Rust tests PASS, including real OpenSSH SFTP server via process pipes, NTFS junction refusal, search, watcher and file revision gates on source `ce7d2e`. No external SSH daemon or hostile remote FS; server-side path trust remains explicit opt-in. |
| Config, startup, secrets, diagnostics, shutdown (CL-08A/B) | `cos-core/{config,secrets,logging,runtime}.rs`, `cos-app` | [CL-L4 receipt](../../audit/2026-10-21/cl-l4-048e327-receipt.json): 32 core + 11 app + 7 native runtime checks PASS on source `048e327`. DPAPI is Windows-only. Memory/latency were limited synthetic samples, **not** peak/RAM improvements or an installer benchmark. |
| MCP, control, code worker, tunnel supervision (CL-09A/B/C) | `cos-mcp/{http,control,protocol,kernel,code,worker,tunnel}.rs` | [CL-L5 receipt](../../audit/2026-10-21/cl-l5-bc4ba86-receipt.json): 55 Rust tests + 15 external native Windows checks PASS on source `bc4ba86`. MCP bearer-path, HTTP Host/Origin refusal, control bearer and code-worker budget were exercised synthetically; real external proxy/tunnel/cloud-control/provider login NOT RUN. |

The existing CL-04B, CL-04C, CL-L2, CL-L3, CL-L4, and CL-L5 DAG prerequisites are all accepted at the reviewed base commit. Each earlier receipt is tied to **its own tested SHA**, not automatically to CL-10's latest code. Cloud Windows/Ubuntu CI at CL-10's exact final SHA is the integration requalification; CL-L6 is the separate physical Windows acceptance.

## New cloud integration proof

- `tools/cl10_backend_smoke.py` runs `target/release/cos-rust(.exe)` with isolated tempfile profiles on each cloud OS. **Not a mocked Rust service.**
- Positive: explicit data-dir/profile, public self-test, four-plus retained event rows, export, sessions list, search/usage, tool schema, secure-store platform, live loopback service start, authorized control health, graceful shutdown and durable restart.
- Negative: missing profile selection, non-Rust unmarked directory preservation, unattributed model invocation, bearer-path mismatch, hostile MCP Host/Origin, unauthenticated control health, and denied action POST.
- `.github/workflows/ci.yml` runs this after `cargo fmt --all --check`, strict all-target Clippy, all workspace tests, and release build, on **windows-latest and ubuntu-latest**. Tests emit no bearer tokens or personal data.
- If the smoke fails, the PR **must not** be accepted. No silent skipping for platform flakes; repair or document a real scoped blocker.

## Security review and residual hazards

1. **Bounded ingress / authority:** reviewed source validates explicit runtime profiles, request attribution for CLI model tool calls, cryptographically random bearer paths, loopback-only listeners, host/origin header conditions, control bearer and action-disable gating. The new live negative tests cover these together, beyond module unit tests. A command allowlist is not a sandbox or Windows AppContainer.
2. **Resource and integrity limits:** content admission and bounded bodies exist in `cos-mcp::http`; session flush precedes release; the backend's earlier NTFS/SFTP/worker capacity checks retain their individual native receipts. This audit does **not** make claims about adversarial external SSH hosts, untrusted third-party binaries, indefinite operation or all timeout races.
3. **Privacy:** a public synthetic recorder fixture is used; neither provider tokens nor user files are read; no actual clipboard/screen, browser extension, GUI, tunnel or real account consent is exercised.
4. **Scope / provenance:** source contracts `BRIDGE_PROTOCOL=14` and `UPSTREAM_COMMIT=024731d...` are explicit. This is a scoped, independent *backend* closure review, not comprehensive browser/Desktop parity. A separate third-party security sign-off should not be inferred unless someone actually supplies one.
5. **Release gates:** cloud CI can run on a hosted Windows VM, **but cannot substitute** for the user's D:-based native CL-L6, real browser feasibility 7P-A/B/L1, native desktop/installer, or the later 7–16 integration gates. No size, speed or RAM **improvement** is claimed here.

## Evidence and reviewer/merge handoff

- Source branch: [CL-10 cloud branch](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/tree/work/cl-10-integrated-backend-audit-20261021)
- Claim: [docs/claims/CL-10.md](../claims/CL-10.md)
- Final PR/CI: capture exact final PR head SHA, GitHub Actions Rust Windows/Ubuntu and Task DAG links, failures/retries and any reviewer/owner decision in the PR and Notion record **after they have actually completed**.
- After accepted merge: make an atomic DAG/board/status change to `done`, release the claim, move Notion current focus to CL-L6. **Do not claim CL-L6 completed without separate native Windows evidence.**
