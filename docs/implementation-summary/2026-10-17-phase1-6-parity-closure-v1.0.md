# Phase 1–6 parity closure — implementation summary v1.0

Date: 2026-10-17, Asia/Jakarta. Status: **IN PROGRESS**.
Authorization: close all PARTIAL items, initialize Git and continue implementation.
Windows is active; macOS/other platforms are **HOLD at the user's request**. Linux is permitted if feasible.

## Current result

- Initialized the root repository and committed the verified prior source as `68f7954`.
- Continuing on `feat/close-phase1-6-partials`. The nested upstream remains pinned, ignored and untouched.
- Historical baseline: 116 Windows Rust tests, 17 executable checks, 8.476 MiB service binary.
  These counts are prior evidence, not proof that the new closure tasks have passed.
- Added contribution guidance and a pinned-action Windows/Linux CI workflow. GitHub CI is not yet run.
- Git Review now has snapshot/diff/statistics/branch comparison and metadata containment, with 35 upstream
  oracle vectors and functional regressions. CLI review/diff is executable. Full closure validation is running.
- Fixed a stored canonical Windows path being rejected by project tool/terminal workspace resolution;
  native project terminal and proven-session default patch cwd regressions passed RED→GREEN.
- Docker recovered by preserving its old two-entry socket directory and recreating the runtime directory.
  Images, volumes and settings were not reset. A real Linux container then passed **112 baseline Rust tests,
  0 failed/ignored**, including the previously compile-only Unix process cleanup test.
  [Linux baseline receipt](../../audit/2026-10-17/linux-baseline-receipt.json) applies to `68f7954`,
  not the subsequent Git implementation. [Docker recovery](../../audit/2026-10-17/docker-runtime-recovery.json).
- Remaining phase 1–6 gaps stay open and are being implemented; macOS is held by user instruction.

Latest integrated closure checkpoint: **166 Windows Rust tests PASS, 0 failed/ignored; strict Clippy PASS**.
The missing-project binding regression now refuses a default cwd instead of writing into another folder.
Git branches, committed comparisons, untracked/binary/large files, revision refusal and both new/legacy
metadata containment paths are covered. The initial concurrent link failed on Windows paging exhaustion;
the unchanged suite passed with two build jobs. Logs are retained separately; this is not a test removal.

The Git checkpoint `979f680` passed **122 Linux tests**, 0 failed/ignored:
[Linux receipt](../../audit/2026-10-17/linux-closure-receipt.json).
Real Windows-to-Linux SSH qualification passed six checks using generated fixture identities and
private config/known_hosts: [SSH receipt](../../audit/2026-10-17/ssh-qualification.json).
No personal SSH profile, credentials or public port was used. This proves an actual isolated SSH
transport, not an external production server or adversarial-server filesystem containment.

Project presentation and creation state are now ported: validated colors, soft removal, exclusive
claims, armed-attempt expiry, linked/gone/failed/uncertain states, explicit retry and bounded change
notifications. Restored armed attempts expire on the app clock without a browser poll. Seven
regressions passed RED→GREEN; five source-derived scenarios match 27 committed upstream steps.
CLI `projects color <id> <color|->` and `projects remove <id>` are executable. Browser creation
delivery remains a phase-7 dependency; no signed-in browser or remote creation was exercised.

Git watches now include nested worktree writes and linked-worktree common metadata. Four native
regressions passed RED→GREEN for invalidation, project switching and root revocation. The project
lifecycle commit `c3db4af` also passed **132 Linux tests**:
[receipt](../../audit/2026-10-17/linux-project-links-receipt.json).

An earlier integrated run hit the existing two-second tunnel-drain deadline once. Isolated and
concurrent retries passed before repair; that timing cause was not uniquely reproduced. Investigation
did reproduce a separate stop defect under pending output publication (RED→GREEN). Explicit sidecar
retirement now waits on actual child/pipe state, retains custody on failure and prevents observer
generation rollback. The repaired suite passed with the original deadline and 70 restart cycles.
The failed run remains in `audit/2026-10-17-closure-git-watch-windows.log` as separate local evidence.

## File-panel continuation — 2026-10-18

The human project file panel now lists and previews files, creates/renames entries and saves exact
UTF-8 text using source-compatible revision guards and atomic replacement. CLI commands are
`projects files`, `preview`, `create-entry`, `rename-entry` and `save-file`. These are host actions;
the model tool registry has not gained file-panel permissions.

Two source directory scenarios and 33 path/name/binary vectors are captured. Native source-versus-CLI
qualification matches three snapshot/save cases and ten complete preview projections. It reproduced
and repaired timestamp rounding branches and a large Windows file ID producing a different JS hash.
Selected-project junction refusal, stale edits and replaced-source custody have native regression proof.

Server project listing/text previews and private attachment snapshots use the existing explicit runtime
trust gate. Native SFTP exercises a streamed 600,000-byte copy, cleanup, 512 MiB admission, revocation
during copying and transport reuse after failure. Server edits are refused as in the original source.

The integrated checkpoint passed **160 Windows tests, 0 failed/ignored**, strict Clippy and formatting.
The same commit `10d4c81` passed **156 Linux tests, 0 failed/ignored**, including native Unix
parent/source replacement and SFTP regressions: [Linux receipt](../../audit/2026-10-18/linux-file-panel-receipt.json).
The Windows service at `10d4c81` builds to **10.269 MiB** (+1.793 MiB against the previous service),
passes 17 executable checks and 13 source comparisons: [release receipt](../../audit/2026-10-18/file-panel-release-receipt.json).
This is a headless service measurement; installer, full desktop startup and RAM comparisons are not run.
Full raster
parity (including AVIF), every locale/ICU version and later UI/attachment/trash integration remain open.
This is further implementation progress; the whole phase 1–6 exactness gate remains **IN PROGRESS**.

## Patch continuation — 2026-10-18

The model-facing patch adapter now selects the source's normal update mode and preserves line
endings for permitted moves when Edit is unavailable. Add-overwrite requires both Create and Edit.
Typed mismatch diagnostics retain bounded, reliable source anchors; the model receives the excerpt
only with current Read permission and unchanged caller/root authority. Five MCP regressions and a
differential corpus of 36 updates/14 searches pass. Native SFTP also verifies both update modes,
move-only byte preservation and typed diagnostics. Integrated Windows qualification: **166 PASS,
0 failed/ignored**; strict Clippy and formatting PASS. Linux qualification for this patch commit follows.

The existing Edit guard remains when a nominal move would rewrite bytes, including an unterminated
file gaining a newline. This conservative edge differs from the upstream applier; it remains explicit
in the plan's ruling. Streaming/invocation parsing, full search/toolchain behavior and the later tasks
are still open. The 10.269 MiB release receipt above applies to `10d4c81`, before these patch changes.

## Documents

- [Implementation plan and live ledger](../implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md)
- [Review record](../implementation-review/2026-10-17-phase1-6-parity-closure-v1.0.md)
- [Previous service result](2026-10-17-phase4-6-rust-services-v1.0.md)

The existing exactness gaps stay open until an implementation and executed proof close them. Platform
HOLD is separate from PASS, FAIL and a missing local implementation. Contribution readiness does not
imply that this service is already a full desktop replacement.
