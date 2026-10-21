# CL-L5 — native Windows MCP/control/code-worker qualification

**Date:** 2026-10-21, Asia/Jakarta. **Status:** PASS for the scoped CL-L5 gate. **Exact merged GitHub `main` source SHA tested:** `bc4ba8657ed8f084894cc859ad6a4a3a7fbb48b4`.

## Scope and host

`CL-09B` connection/retry/tunnel health in [PR #30](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/30) and `CL-09C` code-worker boundaries in [PR #31](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/31) are merged ancestors of the tested SHA. Windows 11 x64 NT build `26300.9711`; physical `D:` volume is NTFS (approximately 80 GiB free at preflight). The only source checkout used was the new clean worktree `D:\projects\CoS-Rust\.test-tmp\cl-l5\worktree`, with `CARGO_HOME=D:\projects\CoS-Rust\.test-tmp\cl-l3\cargo-home` (reused prior D: cache), `CARGO_TARGET_DIR=D:\projects\CoS-Rust\target` and `TEMP`/`TMP` under `.test-tmp\cl-l5\tmp`. Historical local recorder/caption WIP files were untouched.

## Tests and live results

| Check | Result |
| --- | --- |
| `cargo test -p cos-mcp --tests --locked --offline -j 4 -- --test-threads=2` | **55 PASS**, 0 failed, 0 ignored; confirmed OS exit 0 |
| `cargo fmt --all --check` | PASS, exit 0 |
| `cargo clippy -p cos-mcp --all-targets --locked --offline -j 4 -- -D warnings` | PASS, exit 0 |
| `cargo build --release --locked --offline -j 4` | PASS, exit 0 |
| External Node harness driving a real spawned Windows release service | **15 PASS**, exit 0 |

The release executable is **12,853,760 bytes**, SHA-256 `76b03ea16a767171941044a638b51212d1bda159ad6c6c2338135cd736e18d11`. This is a source-bound size observation, not a before/after performance claim. No resource benchmark was requested in this gate; CL-L4 captured startup/RAM and CL-L6 will evaluate integrated release costs.

The native harness created disposable synthetic `D:` profile/session/files, then tested: MCP `server/discover` and tools schema using version `2026-07-28`; invalid host/origin/routes/header claims; an authorized file read; spawned isolated Windows code worker; nested read result; unawaited nested call rejection; bounded runaway CPU loop; cross-surface tool refusal; authenticated local control health and session/event reads; bearer/origin refusal and disabled mutation; an abruptly dropped TCP client followed by successful reconnect; clean timed shutdown and both listeners closed; restart with invalidated prior control token/MCP path and persistent session data; and fail-closed rejection without a companion identity or explicit unattributed-call opt-in. No private profile, account, or remote control service was accessed.

## Evidence and test-harness corrections

The [SHA-bound JSON receipt](../../audit/2026-10-21/cl-l5-bc4ba86-receipt.json) identifies exact test output, script, binary and earlier trial hashes. The complete harness, test logs and failed fixture attempts remain ignored on `D:\projects\CoS-Rust\.test-tmp\cl-l5`, not in Git. The first harness run passed 13 real checks, then asserted `404` for an empty JSON-RPC request sent to a **new valid** MCP URL (the server correctly returned `400`). The second run passed the same 13 checks and then incorrectly required a stale port to still accept TCP; the OS correctly returned `ECONNREFUSED`. The third run used a new synthetic profile and passed all 15 checks. These were harness expectation corrections; no Rust implementation was changed, no original failure receipt was relabeled or removed.

The initial Rust test capture used a PowerShell 5.1 merged stderr pipeline and returned a wrapper failure despite all test results reporting `ok`; the explicit stdout/stderr-file rerun returned Cargo exit **0** with 55 passes. Strict formatter, Clippy, release build and final native harness also returned exit 0. This distinction is preserved in the raw receipts.

## Acceptance and handoff

```text
UNIT: CL-L5 — physical Windows loopback MCP, local control/client sessions, code-worker negative cases and disconnect/retry
GITHUB SHA UNDER TEST: bc4ba8657ed8f084894cc859ad6a4a3a7fbb48b4
MACHINE/OS/DRIVE: Windows 11 x64, NT build 26300.9711, D: NTFS; all task artifacts/temp/target/cache on D:
NATIVE TESTS: 55 cos-mcp PASS; release/fmt/strict Clippy PASS; 15 external runtime checks PASS
EVIDENCE: audit/2026-10-21/cl-l5-bc4ba86-receipt.json; hashed ignored raw logs and synthetic fixture on D:
STATUS: PASS for CL-L5 scoped local gate
YOU REQUIRED: NO
KNOWN LIMITS: synthetic-only client/control, no live external tunnel/proxy, no installed desktop, no provider sign-in or full source parity; no comparable performance measurement
NEXT: CL-10 cloud integrated backend closure and independent review, followed by CL-L6 physical Windows release qualification
```
