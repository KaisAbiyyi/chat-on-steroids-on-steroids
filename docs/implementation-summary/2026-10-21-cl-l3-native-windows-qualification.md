# CL-L3 — native Windows filesystem, search, watcher and SFTP qualification

Date: **2026-10-21 (Asia/Jakarta)**. Status: **PASS for the scoped CL-L3 gate**, tested against exact existing merged GitHub `main` SHA `ce7d2ef0df3e1b62a98664fade70a825d6f9d72e`.

## Prerequisite and host evidence

- `CL-06B` SFTP containment merged in [PR #27](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/27), `6856491d`; `CL-07B` streamed native search/regex merged in [PR #28](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/28), `7b876894`. Exact tested main SHA includes both merges.
- [Exact-SHA Rust Windows/Linux CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39545344458) **SUCCESS**; [Task DAG CI](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39545344361) **SUCCESS**. Hosted CI is separate from native acceptance.
- Physical Windows 11 x64 host, build 26300; D: is NTFS. Clean detached qualifying worktree `D:\projects\CoS-Rust\.test-tmp\cl-l3\worktree`. Cargo dependencies/cache, build outputs, temporary files and local evidence use D:; 8 older local recorder/caption WIP paths preserved.

## Tests observed

| Area | Native result |
| --- | --- |
| `cargo test -p cos-tools --tests --locked --offline -j 4 -- --test-threads=2` | **95 PASS**, 0 failed, 0 ignored across 24 result groups; exit 0 |
| NTFS/junction and transactions | Escape through `mklink /J` refused, no outside-root exposure; commit failure rollback, byte/identity preservation **PASS** |
| File panel | Directory ordering/limits, preview/atomic save, stale revision denial, junction refusal and project-scope revocation **PASS** |
| Search | Traversal never follows junctions, native search ordering/limits, stream/regex and deadline regressions **PASS** |
| Git watcher races | Rapid ref namespace replacement/rearm, late events after project swap/close and revoked-root invalidation **PASS** |
| Real SFTP v3 protocol | Actual installed `C:\Program Files\Git\usr\lib\ssh\sftp-server.exe` launched in an isolated fixture; realpath/stat/read/write/rename, preview, patch, denied moves/deletes, commit-time revoke and cleanup **PASS** |
| Strict Clippy | `cargo clippy -p cos-tools --all-targets --locked --offline -j 4 -- -D warnings`: **PASS**, exit 0 |
| Rustfmt | `cargo fmt --all --check`: **PASS**, exit 0 |

This used the real OpenSSH SFTP subsystem over process pipes, not a remote network SSH daemon. It proves SFTP v3 integration and the native server's filesystem effects; a separate remote-host/network SFTP interoperability test may be added if needed for eventual product acceptance. No private SSH identities or accounts, Docker/WSL, browser GUI or installed desktop were accessed. Performance, external-host behavior and desktop completeness **NOT MEASURED / NOT QUALIFIED**.

## Receipts and next gate

The [committed SHA-bound JSON receipt](../../audit/2026-10-21/cl-l3-ce7d2ef-receipt.json) records raw log hashes, tested commands, native machine/storage context, CI URLs, result counts and fixture limitations. Actual logs and script remain ignored under `D:\projects\CoS-Rust\.test-tmp\cl-l3\evidence` and `qualify.ps1`, respectively. The disposable server has no remaining process; no deployed service or external host was changed.

```text
UNIT: CL-L3 — native Windows NTFS, watcher, file panel/search, real SFTP subsystem PASS
GITHUB SHA UNDER TEST: ce7d2ef0df3e1b62a98664fade70a825d6f9d72e
MACHINE/OS/DRIVE: Windows 11 x64 build 26300; D: NTFS; test artifacts on D:
NATIVE TESTS: cos-tools 95/95 PASS; fmt PASS; strict Clippy PASS
EVIDENCE: audit/2026-10-21/cl-l3-ce7d2ef-receipt.json and hashed local logs
STATUS: PASS for scoped CL-L3
YOU REQUIRED: NO
KNOWN LIMITS: SFTP v3 real server via pipes, external SSH network NOT RUN; no desktop/performance acceptance
NEXT: CL-L4 and CL-L5 independently; CL-10 only after both native gates pass
```
