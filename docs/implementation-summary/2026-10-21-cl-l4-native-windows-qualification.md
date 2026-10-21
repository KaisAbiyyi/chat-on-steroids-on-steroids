# CL-L4 — native Windows configuration, credentials, shutdown and resources

**Date:** 2026-10-21 (Asia/Jakarta). **Status:** PASS for the scoped native CL-L4 gate. Exact tested GitHub `main` SHA: `048e327587edf738d1bbd0e7ecc73b0f758596d7`.

## Prerequisites and environment

- `CL-08A` Windows startup/secret lifecycle merged in [PR #23](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/23); `CL-08B` diagnostics/awake/shutdown merged in [PR #29](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/29), merge `ea8ab7a`.
- On the exact tested `main` SHA, [Windows/Linux Rust CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39584693596) and [Task DAG CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39584693801) both succeeded.
- Physical host: Windows 11 x64, build 26300, NTFS D:. Detached qualification checkout `D:\projects\CoS-Rust\.test-tmp\cl-l4\worktree`. Test logs, synthetic profiles and temporary files on D:. Shared Cargo cache reused from the completed CL-L3 gate on D:; Cargo target on D:.
- The eight historical recorder/caption WIP paths in the original checkout were untouched.

## Native results

| Check | Outcome |
| --- | --- |
| `cargo test -p cos-core --tests --locked --offline -j 4 -- --test-threads=2` | 32 PASS, 0 failed/ignored |
| `cargo test -p cos-app --tests --locked --offline -j 4 -- --test-threads=2` | 11 PASS, 0 failed/ignored |
| Strict Clippy for `cos-core` and `cos-app`, all targets | PASS |
| `cargo fmt --all --check` | PASS |
| `cargo build --release --locked --offline -j 4` | PASS |
| Windows CLI/runtime checks | 7 PASS, 0 failed |

The native CLI checked fresh profile settings persistence across process restarts; Windows DPAPI-backed `Local State` initialization; refusal to replace a keyless encrypted vault; hard failure on unreadable `config.json` filesystem paths; recovery from readable malformed JSON; awake-enabled `serve` lifecycle; clean process shutdown and listener closure; and five fresh cold starts plus five restarts using those profiles. Windows-only Rust regressions separately exercised real DPAPI direct-vault migration/cold restart, retained `Local State` fields and corrupt/keyless refusal.

The first CLI harness attempt reported one failure because its test fixture assumed `info` persisted a fresh `config.json`; the application intentionally does not write that file until a settings update. The revised harness explicitly persists configuration before testing its unreadable-path failure, and all seven checks PASS with distinct `profiles-v2` fixtures. Both local receipts are preserved. No source code was changed during CL-L4.

## Resource observations

Measured using the new release executable `53d98c10cf2f131b2d88b7479f784f7e8efef1a38b837508596716929d943a1a` (12,853,760 bytes). Each class has five samples:

| Metric | Cold start median | Cold p95 | Restart median | Restart p95 |
| --- | ---: | ---: | ---: | ---: |
| Process launch to `ready` (ms) | 47.45 | 62.64 | 33.17 | 42.08 |
| Working set (MiB), single sample/run | 6.73 | 6.74 | 6.67 | 6.71 |
| Private memory (MiB), single sample/run | 1.17 | 1.18 | 1.16 | 1.18 |

Total synthetic profile disk footprint across the run: **17,839 bytes**. p95 of five points is the largest sample; memory is a point-in-time measurement, **not peak RSS**. No matched previous-SHA benchmark was run and no optimization or release-product claim is made.

## Receipt and handoff

The committed [exact-SHA receipt](../../audit/2026-10-21/cl-l4-048e327-receipt.json) includes the binary hash, logs and script hashes, CI URLs, test counts, environment and measurement methodology. Full raw local logs and the scripts are ignored under `D:\projects\CoS-Rust\.test-tmp\cl-l4`. No private provider profile, tunnel credentials, Electron browser, Docker/WSL or external SSH server was accessed.

```text
UNIT: CL-L4 native Windows startup, DPAPI, config, shutdown, resource qualification
GITHUB SHA UNDER TEST: 048e327587edf738d1bbd0e7ecc73b0f758596d7
MACHINE/OS/DRIVE: Windows 11 x64 build 26300; D: NTFS
NATIVE TESTS: cos-core 32/32; cos-app 11/11; seven CLI checks; release/fmt/Clippy PASS
EVIDENCE: audit/2026-10-21/cl-l4-048e327-receipt.json and hashed local logs
STATUS: PASS for scoped CL-L4
YOU REQUIRED: NO
KNOWN LIMITS: only headless runtime; resource sample size 5, no peak/long-run or historic matched benchmark
NEXT: CL-L5 local MCP gate; CL-10 after CL-L5; CL-L6 after accepted CL-10
```
