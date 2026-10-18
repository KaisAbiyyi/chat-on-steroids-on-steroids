# Phase 1–6 parity closure — implementation summary v1.0

Started: 2026-10-17; updated: 2026-10-18, Asia/Jakarta. Status: **IN PROGRESS**.
Authorization: close all PARTIAL items, initialize Git and continue implementation.
Windows is active; macOS/other platforms are **HOLD at the user's request**. Linux is permitted if feasible.

## Current result

- Latest integrated checks: **237 Windows / 227 Linux tests PASS**, 0 failed/ignored, formatting and
  strict Clippy PASS. Linux executed exact content commit `209462d`; Windows uses the lean profiles.
- Windows service `209462d`: **12.108 MiB**, 63 executable/source checks PASS. Full desktop and installer
  acceptance remain open. [Release receipt](../../audit/2026-10-18/session-release-receipt.json).
- Storage checkpoint: Cargo removed the verified 25.394 GiB development cache; a complete lean rebuild
  including Clippy uses **1.464 GiB**. Assertions, overflow checks and fixture limits are unchanged.
  [Cold-build receipt](../../audit/2026-10-18/windows-lean-profile-receipt.json).
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

Latest integrated closure checkpoint: **237 Windows Rust tests PASS, 0 failed/ignored; formatting and strict Clippy PASS**.
[Windows receipt](../../audit/2026-10-18/session-content-windows-receipt.json). Latest exact-commit Linux
proof covers content commit `209462d`: **227 PASS**:
[Linux receipt](../../audit/2026-10-18/linux-session-content-receipt.json).
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

## Native-order qualification correction — 2026-10-18

The first search-engine Linux run at `c1ae547` exposed a fixture input mismatch: native enumeration
visited the nonmatching file first, while the captured provider supplied the hit first. The complete
responses therefore differed only in the correct scan count (2 versus 1). The source handler now
captures both directory inputs, producing 16 projections; each native run selects its actual input
and compares all response fields exactly. Product traversal and acceptance limits were unchanged.
The failed run is retained: [Linux receipt](../../audit/2026-10-18/linux-search-engine-lean-receipt.json).
It completed 63 tests successfully and failed one before Cargo stopped; it is not full qualification.
The full updated Windows suite passes **201 tests**, formatting and strict Clippy:
[receipt](../../audit/2026-10-18/search-engine-native-order-windows-receipt.json). A fresh Linux
exact-commit qualification follows this correction.

## Storage and executable qualification — 2026-10-18

The native-order correction at `4a40c93` passed **193 Linux tests**, formatting and strict Clippy,
with a **1.1 GiB** shared cache. The failed prior Linux run remains historical evidence. The Windows
release is 12,502,528 bytes, SHA-256 `f5a9a7aa80b5c6785fcbd38798d1557f3d6722be0158cf13dad567f051525877`.
It adds 202,752 bytes against `23c1f95`, covering toolchain and search work together. Actual release
qualification passed 17 runtime checks, 13 project-file comparisons and 11 search/MCP/code-worker checks.
The source handler corpus captures both directory inputs; each native run selects its actual input.

Content/handler fixtures use the original public filesystem fallback provider. They do not qualify
every local bundled-ripgrep branch. Partial-read hit retention and chunk-boundary behavior remain open,
alongside JavaScript server regex, unpaired JSON strings and SFTP pipelining/entry budgets. Installer,
full desktop startup and RAM measurements remain NOT RUN; the fresh whole-scope review is NOT RUN.

Development and test profiles now omit symbols and incremental state by default. A cold native Windows
rebuild passed all **201 tests**, formatting and strict Clippy. Debugger symbols remain available through
documented Cargo environment overrides; requesting another profile can allocate additional artifacts.
Before cleanup, the exact workspace development target, ancestry, reparse points and active processes
were verified. Cargo's scoped cleanup removed 39,243 files; the archived release and other Docker/user
data were retained. [Cleanup receipt](../../audit/2026-10-18/windows-debug-cache-cleanup.json).
The rebuilt cache measures 1,571,649,517 bytes. Free space at that receipt was **39.09 GiB on C / 67.01 GiB
on D**; external free-space changes are not attributed to this cleanup. Host fixture/archive temporary
files stay on D. Linux uses the same lean cache by default, with explicit opt-in for debugger profiles.
Exact commit `628b027` also passed the default Linux runner: **193 tests**, formatting and strict Clippy,
cache still **1.1 GiB**. [Default-profile receipt](../../audit/2026-10-18/linux-default-lean-profile-receipt.json).

## Session title, metadata and recovery continuation — 2026-10-18

Session previews now use the first canonical authored request, preserve its spacing and framing rules,
and follow manual/provider/desktop/worker title authority. `sessions rename` and `sessions clear-name`
are executable host operations; clearing restores the current automatic name. Planner helper repair
preserves every recorded message. The source capture contains 17 prompt inputs, 51 previews, 39 legacy
projections and 44 real-store operations across nine scenarios, including restart. Fifty previews are
representable; one lone-surrogate result stays a named JSON compatibility gap.

Metadata normalization now matches 45 source inputs plus four damaged JSON inputs, with actual store
outputs captured separately. It validates automatic titles, selected model, activity, worker/finish
metadata, lineage and legacy defaults. A Windows exclusive-lock test reproduced the previous false
absence result; unreadable metadata now refuses when no valid backup can answer. Three source crash
snapshots exposed lost unknown fields and changed startedAt. Recovery retains metadata-only facts,
the original clock and the context-token delta after rebind; raw events and first identities remain.

Activity now preserves fractional times, source UTF-16 preview lengths and typed finish-decision
authority. Four reducer regressions were observed RED then GREEN. The 38 captured source steps include
37 representable comparisons and one explicit unpaired-title refusal. All **219 Windows tests PASS**,
formatting and strict Clippy PASS. The complete store suite passes 41 tests, including a valid-backup
read under a real Windows exclusive lock, staged-write failure and raw activity preservation.
Provider recorder, content-reference/table/export branches
and the other plan tasks remain in progress. Fresh whole-scope review remains NOT RUN.

The initial integrated run completed 123 tests PASS and failed the historical restart oracle. It had
compared a cold Rust summary with a warm source snapshot. A fresh cold load of the unchanged historical
files through the actual source confirms that activity without `kind` becomes null, while all events
and recent rows remain identical. The warm assertion remains; the restart assertion now compares the
complete cold source snapshot with a hash of its exact input fixture. Product behavior was unchanged
for this correction. [Failed attempt](../../audit/2026-10-18/session-projection-first-run.json).
The Windows development cache now measures **1.684 GiB**, with **37.69 GiB free on C / 66.66 GiB on D**
at qualification. Exact commit `88c0867` also passes **209 Linux tests**, formatting and strict Clippy,
with a 1.1 GiB shared cache. The release artifact above still
applies to `4a40c93`, before the session changes.

## Metadata strings and native observations — 2026-10-18

Two further native regressions exposed a source-valid unpaired JSON string being answered from an
older backup and a different file UTF-8 decoding path. The metadata reader now preserves both files
and reports Unsupported for an unrepresentable primary. Paired/literal escapes remain usable; seven
actual Node byte-decoding inputs qualify decoding before JSON parsing. This preserves data while the
global lossless JSON representation remains OPEN. Both focused tests passed after observed RED.

The recorder now routes conversation titles after canonical messages, handles explicit model-selection
observations and accepts native request text with authored-time correction. Seventeen actual store
selection steps qualify model syntax, repeated/stale/non-finite time, conversation authority, rebind
and restart. Selection metadata is staged and durably written before publication; failed writes keep
the previous value. Retired/disabled observations cannot change the replacement session's selection.

A bounded native regression also reproduced a recursive binding-cache lock after rebind. The copied
binding now outlives only the lookup guard; the five-second acceptance bound remains and the lookup
settles. Four observation tests pass, including raw request and provider/manual title authority.
Broader recorder activity/proof/recovery branches, content references/tables/export and other tasks
remain open. Combined workspace qualification passes **225 Windows tests**, formatting and strict
Clippy: [receipt](../../audit/2026-10-18/session-observation-windows-receipt.json). The first attempt
passed every test but failed strict Clippy on a redundant struct update. That syntax was removed and
the entire suite rerun; the failed log/receipt remain. The shared Windows cache measures **1.728 GiB**,
with **37.49 GiB free on C / 66.62 GiB on D** at qualification. Exact new-commit Linux proof follows;
whole-scope review remains NOT RUN.

## Content, DIL and Markdown continuation — 2026-10-18

Content/directive, HTML, provider-entity, DIL table and transcript/answer projections are now ported.
Markdown fills recorded table rows, removes recorded dead-button labels, converts link cards and
hydrates only truncated text. `export <id> --format md --scope answer --turn <turn-id>` requires a
recorded completed turn and exports its final answer. Native source regressions were observed RED
for links, tables, hydration and the CLI answer route, then GREEN. The source function's text is
captured with a memory-only sink; real clipboard/dialog and full desktop acceptance remain NOT RUN.

The corpus contains 122 content rows (104 exact values, 15 exact source errors, three unpaired output
refusals), 24 DIL rows, seven transcript cases, eight filenames and eight full export text outcomes.
JS line boundaries, escaped continuation's 200-unit window, source whitespace and NFC filename
clipping are preserved. An intermediate unpaired HTML value can still qualify a boolean predicate;
it is never substituted into output. Numeric entity errors use the existing pinned ryu-js dependency
for JS Number formatting and correctly rounded hex parsing. These fixes have focused source/native
proof; combined qualification passes **237 Windows tests**, formatting and strict Clippy:
[receipt](../../audit/2026-10-18/session-content-windows-receipt.json). The Windows cache is **2.241 GiB**,
with **37.41 GiB free on C / 66.07 GiB on D** at qualification. Assertions, acceptance limits and the
shared lean build profile are unchanged; current-commit Linux and release measurements follow.

Model-facing content text remains a qualified library contract for the later final-response owner.
The Rust JSON archive is a separate extension. Global lossless JSON, bounded-asset differences,
broader recorder/provider/history branches and remaining plan tasks stay open. Exact observation
commit `caa234a` passed **215 Linux tests**, formatting/strict Clippy, using the same **1.1 GiB** cache.
Whole-scope independent review remains NOT RUN.

## Session release and Linux checkpoint — 2026-10-18

Exact commit `209462d` passed **227 native Linux tests**, formatting and strict Clippy, with a **1.4 GiB**
shared cache. The Windows service is **12,696,576 bytes (12.108 MiB)**, SHA-256
`66fde14aa697af8a2f15f0bc934c236898859f5458de9866a0d0015e2b020d3d`, +194,048 bytes versus
`4a40c93` for all session work together. It was built with one job after observing 2.30 GiB free RAM;
no heavy Linux build ran in parallel. The original archive and every older receipt remain.

Actual executable qualification passed **63 checks**: 17 runtime, 13 project-file source comparisons,
11 search/MCP/code-worker checks and 22 session CLI projections. The latter compare eight complete
source export payloads/errors and fourteen named selection-state projections. Three other selection
cases remain native API tests, explicitly listed in the receipt because non-finite JSON values and
an explicit store-owner lookup differ from recorder routing. This is headless service proof;
clipboard/dialog, installer, full desktop startup and RAM comparisons remain NOT RUN. macOS stays HOLD.

Next Task 5 investigation captured **343 actual timeline/running-caption source cases** and eight
short-path cases, with four unpaired-string cases recorded as code units. Product caption/evidence
integration has **not started**; current recorder captions and broader provider/history branches remain
open. See `fixtures/upstream/phase1-6-closure/session-summaries.json`. Whole-scope review stays NOT RUN.

## Documents

- [Implementation plan and live ledger](../implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md)
- [Review record](../implementation-review/2026-10-17-phase1-6-parity-closure-v1.0.md)
- [Previous service result](2026-10-17-phase4-6-rust-services-v1.0.md)

The existing exactness gaps stay open until an implementation and executed proof close them. Platform
HOLD is separate from PASS, FAIL and a missing local implementation. Contribution readiness does not
imply that this service is already a full desktop replacement.
