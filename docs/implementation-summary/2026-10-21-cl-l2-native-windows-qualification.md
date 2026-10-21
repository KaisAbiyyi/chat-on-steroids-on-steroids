# CL-L2 — native Windows shell, ConPTY, process custody and SSH qualification

Date: **2026-10-21 (Asia/Jakarta)**. **Native gate: PASS** against exact existing GitHub `main` commit `ea8ab7a18c3f783eaaaf88310a9c2df5f29ea337` (not the SHA of this documentation receipt).

## Source and machine

- Prerequisite: `CL-05B` merged via [PR #26](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/26) as `e5ad3754c08c0c4edcd57ffa20ac875ce5b740fb`. The tested main commit descends from that merge.
- Exact main GitHub CI on the tested SHA: [Rust Windows/Linux run 39532713363](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39532713363) **SUCCESS** and [task DAG 39532713391](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39532713391) **SUCCESS**.
- Physical host: Windows 11 Home Single Language, x64, build **26300**; Cargo **1.91.1**, MSVC host toolchain; system `C:\Windows\System32\OpenSSH\ssh.exe`.
- Clean detached checkout at `D:\projects\CoS-Rust\.test-tmp\cl-l2\checkout`, `git rev-parse HEAD` = tested SHA. Existing root workspace with eight original WIP paths was not touched. Cargo cache, target, npm cache, test fixture, subprocess TEMP/TMP and evidence stayed under D:.

## Observed native execution

| Check | Evidence | Result |
| --- | --- | --- |
| Windows process/terminal tests | `cargo test -p cos-exec --locked --offline -j 4 -- --test-threads=2` | **29 PASS**, 0 failed, 0 ignored |
| CMD quoting and pipes | Verbatim native CMD PTY, exit 7, batch errors, environment control | **PASS** |
| ConPTY | Interactive PowerShell input, resize, interrupt, terminal pressure/ack, late and invalid dimensions, terminal ID reuse | **PASS** |
| Process custody | Cross-owner denial for poll/terminate/retire, rebind, revoked terminal permissions, pending publication and unread result replay | **PASS** |
| Windows OpenSSH + Rust Manager | Two one-off native integration tests using actual `ssh.exe` and in-memory owner-bound process manager | **2 PASS**, 0 failed |
| SSH trust/auth/exit | Correct ephemeral key login; remote `pwd`, output and exit 7; changed known_hosts key and unauthorized key both refused with exit 255; unauthorized session poll/terminate rejected; owned process retired | **PASS** |
| Formatting and lint | `cargo fmt --all --check`; `cargo clippy -p cos-exec --all-targets --locked --offline -j 4 -- -D warnings` | **PASS / PASS** |
| Disposable resources | Owned Node SSH server PID validated and stopped, `127.0.0.1:32222` confirmed closed; root source worktree remained clean before receipt edits | **PASS** |

The test server was a local `ssh2` 1.17.0 implementation on `127.0.0.1`, listening only on loopback with an ephemeral public-key identity, deliberately isolated from real users and their SSH configuration. **Windows OpenSSH.exe, ConPTY and Rust native process launch/custody were real.** This receipt does not claim external OpenSSH-server acceptance or production transport interoperability; native SFTP and filesystem containment belong to `CL-L3`. Docker/WSL backed-disk location was unverified and no Docker/WSL operation was attempted.

The one-off test source was copied to `D:\projects\CoS-Rust\.test-tmp\cl-l2\evidence\cl_l2_native_ssh.rs` and removed from the worktree before committing this docs-only evidence. Raw output, synthetic keys and fixture code remain ignored on D:. Only hashes and summaries are committed. No private profiles, SSH credentials or clipboard data were used.

## Evidence identity and limitations

The machine-readable [receipt](../../audit/2026-10-21/cl-l2-ea8ab7a-receipt.json) records the test source SHA and immutable raw log hashes. Logs are local and not published as containing shell environment details. Windows/Linux hosted CI does not itself qualify the physical host. No baseline/candidate startup, memory or throughput measurements were made, so **performance improvement: NOT MEASURED**. No browser/desktop/installer/UI gate was evaluated. macOS remains HOLD.

```text
UNIT: CL-L2 — native Windows process/PTY/SSH client qualification PASS
GITHUB SHA UNDER TEST: ea8ab7a18c3f783eaaaf88310a9c2df5f29ea337
MACHINE / DRIVE: Windows 11 Home Single Language x64 build 26300; all task outputs on D:
NATIVE TESTS: cos-exec 29/29 PASS, ephemeral native SSH 2/2 PASS, fmt and strict Clippy PASS
EVIDENCE: audit/2026-10-21/cl-l2-ea8ab7a-receipt.json and local D: test logs (hashes included)
STATUS: PASS for scoped CL-L2
YOU REQUIRED: NO
KNOWN LIMITS: synthetic local SSH server; external sshd/SFTP/desktop not qualified; performance NOT MEASURED
NEXT: CL-L5; CL-L3/CL-L4 before CL-10 integrated backend review
```
