# Phase 4–6 Rust services — implementation summary v1.0

Date: 2026-10-17, Asia/Jakarta. Status: **Windows service implementation and bounded review repair
verified; whole-phase exact acceptance remains PARTIAL**.
Authorization: implement phases 4–6 and keep plan/review/summary in separate `docs/` directories.
Baseline: `024731d8083d92c46e8248d64e7e0b36ab0a470f`, Chat on Steroids 2.1.39 / bridge 14.

## Implemented services

- `cos-exec`: literal shell policy against 400 actual upstream cases; shell and OpenSSH derivation;
  bounded head/tail output, session ownership, unread polling and retained completion, environment
  separation, native pipes and PTYs. Windows ConPTY now uses a Rust adapter with verbatim cmd.exe
  arguments and suspended creation followed by Job Object custody before resume.
- `cos-tools`: approved-root capability handles, bounded text/image reads, UTF-8/BOM/UTF-16 previews
  and revision-checked saves; strict UTF-8 patches with mixed line endings, in-place file identity,
  preflight/guarded rollback, create/edit authority and stale revision refusal; search; persistent
  local/remote projects, Git status/branch/revision and revoked-root watchers; host OpenSSH/SFTP v3,
  OPEN/FSTAT regular handles, large streaming previews, remote saves and patch rollback.
- `cos-mcp`: separate Core/Desktop/Plugins paths and discovery, live permission checks, schema
  validation, phase-3 identity/recording owners, MCP legacy + 2026 protocol adapter, bounded HTTP,
  opt-in control reads/actions, process/generation tunnel supervision and isolated JavaScript workers.
- `cos-app`: persistent `serve` mode, hidden code worker, direct tool/project/file commands and the
  existing explicit-profile/lease/shutdown contract.
- Human terminals have separate renderer-owned IDs, pending-open cancellation, project/permission
  revalidation and native ACK backpressure. Process output has publication leases; an aborted HTTP
  body restores unread results. Later calls identify outstanding sessions without rerunning commands.

## Evidence available

The release executable/loopback proof passed **17 checks** in
[final runtime receipt](../../audit/2026-10-17/phase4-6-runtime-final.json), including accepted file mutation surviving
JS timeout, parent-call recording, read-only CLI refusal and graceful shutdown. Native Windows pipe,
PTY input/resize/interrupt, cmd.exe quoting and native OpenSSH subsystem tests pass. Upstream
function/protocol fixtures live in [fixtures](../../fixtures/upstream/phase4-6/manifest.json).
The final Rust workspace run passed **116 tests**, with 0 failed or ignored. Format checks, strict
Clippy and the release build passed. `cos-exec`/`cos-tools`, including their test sources, compile-check
for x86_64 Linux; native Linux tests were not executed. Original-code verification is separate:
a selected 20-suite run had 268 PASS,
1 renderer-test timeout and 12 SKIPPED; the unchanged focused retry passed 3/3 at the original timeout.
Both receipts are retained. Earlier runs of 64 and 220 overlap and are not an aggregate total.

The Windows x64 release service binary is **8,887,808 bytes (8.476 MiB)**, SHA-256
`b46a07a840daa8c8249687fc64f0aa1ecb7e17f09f938c90e047ff4a347de3ce`.
[Release receipt](../../audit/2026-10-17/phase4-6-release-receipt.json).
This is a CLI/service measurement. Installer and full desktop size reductions are not established.

## Review and repair

One fresh whole-scope source review found **0 Critical, 7 Important, 0 Minor** issues and initially
returned **not ready**. One bounded repair pass added process-group custody, tunnel completion
draining, partial-write recovery, live search authorization, profile-anchored control paths and
aggregate patch budgets. Six findings have source repairs; native Unix behavior still needs its
own execution evidence. The seventh is mitigated by default SSH path refusal, with an explicit
trusted-server exception rather than a claim of adversarial SFTP containment. No second reviewer
was dispatched, and no minor findings were deferred.

Patch transactions admit at most **128 paths** and a conservative **256 MiB estimated accounting
budget**, including input, snapshots and line-vector workspace. Reads and growth reservations are
checked before retaining buffers. Larger transactions must be split; this differs from original
per-file-only admission and is not a measured process RSS ceiling.

Kernel SSH filesystem/path operations, including remote command/terminal cwd checks, are off by
default. Human host flag `--trust-ssh-server-paths` opts in for one runtime on a trusted server
filesystem. Trust is unavailable through MCP arguments or persisted config and resets on restart.
SFTP v3 OPEN/FSTAT does not attest canonical opened-handle containment; adversarial remote containment
remains **UNQUALIFIED**. See the [review and all executor rulings](../implementation-review/2026-10-17-phase4-6-review-v1.0.md).

The [pre-repair manifest](../../audit/2026-10-17/phase4-6-source-manifest.json) remains immutable.
The [repaired source manifest](../../audit/2026-10-17/phase4-6-repaired-source-manifest.json) and
[repair receipt](../../audit/2026-10-17/phase4-6-repair-receipt.json) identify the final source/evidence.
The pre-review executable and original receipt are retained under
`audit/2026-10-17/phase4-6-pre-review-release/`; phase 1–3's historical binary/receipt also remain intact.

## Remaining acceptance

Whole-phase exact acceptance for **4, 5 and 6 remains PARTIAL**. The
[coverage register](../research/2026-10-17-phase4-6-coverage-v1.0.md) names each implemented source family
and its unported/unqualified branches, including full connection/retry/proxy setup, control projections,
patch diagnostics/invocation modes, complete Git/file-panel/search branches and execution edge cases.

External SSH server, actual public/OpenAI tunnels, native macOS/Linux and full desktop/browser acceptance
are **NOT RUN**. Desktop/Plugins and later-phase workflow tools stay unadvertised while their owners are
unavailable. Phases 1–3 retain their previous partial acceptance. A Rust service binary is not an
installer or a complete GUI application; no full-product size reduction is claimed here.

The review's separate rulings preserve first-proven historical recording during rebind and keep
already accepted native effects owned after a JS timeout. They retain existing phase-3 authority
contracts and accepted-effect regressions; no new in-flight rebind integration proof is claimed.

## Reproduce the checks

```powershell
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --release --locked
cargo check -p cos-tools --target x86_64-unknown-linux-gnu --tests
python tools/verify-phase4-6.py --binary target/release/cos-rust.exe
python tools/verify-foundation.py
```

The Linux command requires that Rust target to be installed and checks only the named crates/dependencies.
It does not compile the QuickJS worker or run native Linux code.
Final logs: [Rust tests](../../audit/2026-10-17-phase4-6-rust-tests-final.log),
[Clippy](../../audit/2026-10-17-phase4-6-clippy-final.log),
[release build](../../audit/2026-10-17-phase4-6-release-build-final.log),
[Linux compile check](../../audit/2026-10-17-phase4-6-linux-check.log).

## Documents

- [Executable plan and ledger](../implementation-plan/2026-10-17-phase4-6-implementation-v1.0.md)
- [Review record](../implementation-review/2026-10-17-phase4-6-review-v1.0.md)
- [Original phase roadmap](../implementation-plan/2026-10-16-rust-migration-phases-v1.0.md)

## Dependency boundary

Application services are Rust. Bounded JavaScript uses statically linked QuickJS through `rquickjs`
0.14; the interpreter includes C, matching the original feature's need to run JavaScript. There is no
Node/Electron process in this service build. POSIX PTYs use portable-pty; Windows uses native Win32
ConPTY/Job Objects. SSH and tunnel adapters retain host OpenSSH and explicit sidecar executables.
