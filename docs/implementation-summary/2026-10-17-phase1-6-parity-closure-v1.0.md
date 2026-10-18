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

Latest integrated closure checkpoint: **201 Windows Rust tests PASS, 0 failed/ignored; formatting and strict Clippy PASS**.
[Windows receipt](../../audit/2026-10-18/search-engine-windows-receipt.json). Latest exact-commit Linux
proof currently covers matcher commit `531fcac`: **177 PASS**; search-engine Linux qualification follows.
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
0 failed/ignored**; strict Clippy and formatting PASS. The exact patch commit `0e5457e` also passes **162 Linux tests**, 0 failed/ignored:
[receipt](../../audit/2026-10-18/linux-patch-modes-receipt.json).

The existing Edit guard remains when a nominal move would rewrite bytes, including an unterminated
file gaining a newline. This conservative edge differs from the upstream applier; it remains explicit
in the plan's ruling. Shell invocation parsing, full search/toolchain behavior and the later tasks
are still open. The 10.269 MiB release receipt above applies to `10d4c81`, before these patch changes.

## Streaming continuation — 2026-10-18

Native streaming patch parsing is integrated with the main parser and strict/lenient modes. Three
regressions were observed RED then GREEN; 36 source stream scenarios verify intermediate snapshots,
errors and environment IDs, and 10 source cases verify full parsing. Input admission precedes buffering.
The integrated Windows suite passes **169 tests, 0 failed/ignored**, with strict Clippy. Linux's first
strict all-target Clippy run exposed Unix-only style/import warnings; the focused repaired checkout
passes. Exact streaming commit `67e905a` passes **165 Linux tests**, strict Clippy and formatting:
[receipt](../../audit/2026-10-18/linux-streaming-strict-receipt.json).

Docker's engine was absent at continuation. The exact two stale runtime sockets were preserved and
Docker plus the owned test container restarted: [recovery](../../audit/2026-10-18/docker-runtime-recovery.json).
No Docker data disks, volumes or settings were reset. The earlier service release measurements remain
applicable only to their named commit. Whole phase 1–6 exactness and final review remain IN PROGRESS.

## Invocation continuation — 2026-10-18

The native invocation adapter matches 504 actual upstream cases and now intercepts explicit patch
scripts before shell launch. Implicit/malformed patches return correctness errors; `cd` must name an
existing approved directory. Intercepted execution retains Command authority, including revocation
and allowlist checks, with no child process or invented process metadata. Three new MCP behavior
regressions were observed RED then GREEN; five MCP tests and two invocation tests now pass. Full
Windows qualification is **176 PASS, 0 failed/ignored**, strict Clippy and formatting PASS.

The pinned Tree-sitter/Bash 0.25.1 bindings retain the source grammar/query. Grammar/scanner/type
hashes match; generated parser tables differ only in package version metadata:
[provenance](../../audit/2026-10-18/bash-grammar-provenance.json). This adds an MIT C parser dependency,
so the runtime is not claimed to be pure Rust. Exact invocation commit `23c1f95` also passes **172 Linux tests**, strict Clippy and formatting.
The service release is **11.731 MiB** (12,299,776 bytes), +1.461 MiB against `10d4c81` for
the patch-mode/streaming/invocation changes together. It passes 17 executable runtime checks and
13 project-file source comparisons: [release receipt](../../audit/2026-10-18/invocation-release-receipt.json).
Installer, full desktop startup and RAM remain NOT RUN.
Search/toolchain, session/runtime/control branches and final whole-scope review remain in progress.

## Toolchain continuation — 2026-10-18

Windows child preparation now discovers otherwise unreachable JDK/Go installations without changing
existing toolchain choices or the parent environment. It requires a compiler, retains source version
ranking and cache behavior, and matches 34 source steps plus five version lists. Native public files
and a real cmd child verify selection/delivery. Four functional tests were demonstrated RED then GREEN,
including actual Windows environment blocks containing unpaired UTF-16. Such inherited values are
preserved; automatic discovery is suppressed if its inputs cannot be represented losslessly. Global
JSON/unpaired-string exactness remains open.

Current integrated Windows result: **180 PASS, 0 failed/ignored**, strict Clippy PASS. Exact Linux
qualification of `6839269` passes **172 tests**, strict Clippy and formatting:
[receipt](../../audit/2026-10-18/linux-toolchain-checkpoint-receipt.json). The 11.731 MiB/17-runtime-check
release above apply to `23c1f95`, before these toolchain changes. Search source corpora are captured
(160 globs, 60 excludes, 12 scans, six trees); native search changes have not started.

## Move verification continuation — 2026-10-18

Native and SFTP patches now refuse a move to the same resolved path before applying earlier hunks.
Both defects were demonstrated RED then GREEN. Twelve actual upstream verifier scenarios capture
Windows local case folding and Unix/remote case sensitivity. Native local tests consume the applicable
source scenarios; native Linux SFTP also exercises a distinct case-only destination. Current Windows
qualification: **181 PASS, 0 failed/ignored**, strict Clippy and formatting PASS. The fresh Linux
receipt for this move checkpoint follows. Search integration and the remaining plan tasks stay open.

## Search matching and storage continuation — 2026-10-18

Search include globs now retain source basename fallback, case handling and UTF-16 matching.
Name queries inspect the filename, and explicit Unicode folder exclusions match source casing.
Four functional regressions passed after observed RED; 1,064 source glob rows and 117 exclusions
are retained. Integrated Windows checks: **185 PASS, 0 failed/ignored**, strict Clippy and
formatting PASS. Search traversal/default roots/content/remote responses still need implementation.
The earlier same-path move commit `3a446b0` passed **173 Linux tests**, strict Clippy and formatting:
[receipt](../../audit/2026-10-18/linux-patch-paths-receipt.json).

The first combined Windows run exhausted C Temp during the unchanged 512 MiB fixture test.
Its failed log is retained; the complete rerun passed with temporary files on D. The new
`tools/check-workspace.py` keeps those files in the workspace with a free-space preflight.
Linux qualification supports one lean build cache, disables debug symbols/incremental state and
places host archives on D. It preserves test assertions and records storage settings in receipts.
The system drive had already recovered to about 42 GiB free at continuation, and the old Linux
cache was absent; no claim is made that this work recovered that space. Native Windows build
cache remains on D. Exact search-commit Linux qualification follows this checkpoint.

## Search traversal/content continuation — 2026-10-18

Model search now uses breadth-first native ordering, counts files before include filtering and
stops filename traversal immediately at its result limit. Omitting path searches every approved
root with a shared deadline. Explicit files, directory metadata, size errors and content results
match eight executions of the original registered `find` handler; six source trees and two
large synthetic source filesystem bounds qualify traversal without allocating large disk fixtures.
Native content uses twelve overlapping readers while checking grants on the owner thread; an
actual twelve-reader gate verifies success and revoked publication. Native SFTP covers name/content
search, lifetime-only trust and multi-chunk revocation with transport reuse after handle closure.

Full Windows qualification: **201 PASS, 0 failed/ignored**, formatting and strict Clippy PASS:
[receipt](../../audit/2026-10-18/search-engine-windows-receipt.json). Six initial behavior regressions,
a missing server route, two differential projection defects and admission-before-resolution were
observed RED then GREEN. Windows sharing violation 32 was measured before correcting that test's
OS error classification; its containment requirement was retained.

Eleven content projections match source decoding/sniffing/line behavior. The twelfth contains an
unpaired surrogate produced by source clipping: its exact code units remain in the fixture and
the Rust adapter explicitly refuses that result. This is an **OPEN exactness gap**, despite the
refusal test passing. Server JS regex, full local ripgrep conformance, SFTP pipelining/entry budgets
and the other plan tasks remain open. The whole phase 1–6 gate and fresh final review stay in progress.

Matcher commit `531fcac` passed **177 Linux tests**, formatting and strict Clippy:
[receipt](../../audit/2026-10-18/linux-search-matcher-lean-receipt.json). Its fresh cache measured
**998 MiB**. Archives/temp files stay on the workspace drive; tests retain all original assertions
and file-size limits. The final search-engine commit needs its own Linux receipt.

## Documents

- [Implementation plan and live ledger](../implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md)
- [Review record](../implementation-review/2026-10-17-phase1-6-parity-closure-v1.0.md)
- [Previous service result](2026-10-17-phase4-6-rust-services-v1.0.md)

The existing exactness gaps stay open until an implementation and executed proof close them. Platform
HOLD is separate from PASS, FAIL and a missing local implementation. Contribution readiness does not
imply that this service is already a full desktop replacement.
