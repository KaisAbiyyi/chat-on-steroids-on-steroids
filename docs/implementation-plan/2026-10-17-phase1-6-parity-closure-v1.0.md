# Phase 1–6 Windows parity closure — implementation plan v1.0

Date: 2026-10-17, Asia/Jakarta.
Authorization: “fix semua yang menjadi partial, git inisialisasi terus lanjutin”.
Platform steering: “yang selain windows hold dulu aja, kalau linux bisa gapapa”; prepare for contributions.

**Goal:** implement the remaining phase 1–6 source contracts and prove the Windows behavior; run Linux
qualification where the local environment permits. Preserve every historical receipt and open gate.
**Architecture:** retain the existing six Rust owners. Integrate missing source behavior into its owner,
using captured upstream functions and real disposable filesystem/process/HTTP tests. Later-phase
dependencies stay explicit; a missing backend cannot be replaced by fabricated success.
**Tech stack:** Rust 1.91+, Cargo locked workspace, Windows native APIs, host OpenSSH/Git, isolated code worker.
**Spec:** [16-phase roadmap](2026-10-16-rust-migration-phases-v1.0.md),
[phase 1–3 contracts](../research/2026-10-16-phase1-3-parity-spec-v1.0.md),
[phase 1–3 owner gaps](../research/2026-10-16-phase1-3-owner-coverage-v1.0.md),
[phase 4–6 owner gaps](../research/2026-10-17-phase4-6-coverage-v1.0.md).

## Global constraints

- Baseline upstream commit: `024731d8083d92c46e8248d64e7e0b36ab0a470f`; app 2.1.39, bridge 14.
- Preserve real tool names, errors, identity/epoch authority, existing data and permission boundaries.
- Public synthetic fixtures only. No personal CoS/browser/provider profile, credentials or automatic Send.
- All product edits require a functional regression demonstrated RED, then GREEN; run the integrated suite.
- macOS/other platforms are HOLD by user request. Linux is permitted where feasible; compile is not execution.
- Keep the three dated documentation categories and a persistent task/acceptance ledger.
- Local Git commits are authorized. Publication/remote repository creation is a separate action.

## Review focus

- Stale revisions and link/reparse replacement must never redirect file or Git operations.
- Process expiry, duplicate launches, UTF-8 chunking and shutdown must retain unread output and custody.
- Rich-history repair must preserve raw events/assets, first request proof and manual/provider title authority.
- Connection retries and control projections must follow real generation/permission/backend state.
- Platform/contribution claims must be reproducible without private profiles, source-oracle drift or hidden skips.

## Tasks and evidence

1. **Repository and contribution baseline.** Initialize root Git, checkpoint the verified 116-test source,
   continue on a feature branch, add CONTRIBUTING and Windows/Linux CI with explicit upstream bootstrap.
   Expected: clean tracked source, ignored upstream/binaries/profile data, reproducible locked checks.
2. **Execution qualification and lifecycle.** `cos-exec/{process,shell,tree,buffer}` and native tests:
   inspect exact upstream TTL/reuse/cancellation/exit/shell/chunk branches, capture vectors, add RED tests,
   implement, exercise disposable Windows processes and Linux if available. Expected: no wrong-owner signal,
   silent output loss or altered launch policy. Use a private loopback SSH daemon for real auth/known-host proof.
3. **Project Git and file panel.** Extend `cos-tools` with the original snapshot/diff/branch/tree projections,
   safe Git executable selection, project metadata/removal, revision-fenced file-panel operations and watchers.
   Expected: actual temporary repositories, renames/untracked/binary/stale diff vectors and filesystem races pass.
4. **Patch/search exactness.** Complete source diagnostic/normalization/invocation modes, default path/exclusion
   ordering, regex and toolchain branches. Expected: upstream-derived positive/negative vectors plus real files;
   aggregate resource bounds and transaction recovery continue to hold.
5. **Session presentation and recording.** Port title/summary/activity, content references/tables/export,
   recorder observation/recovery branches, provider cache and large-history/fault behavior into `cos-store`.
   Expected: actual upstream pure-function oracles and restart/epoch/retention regressions preserve raw data.
6. **Runtime/platform source gaps.** Complete applicable Windows startup/awake/log/report/config/crypto
   integration and historical fixture matrix. Linux secure storage only if a disposable provider is available;
   macOS remains HOLD. Expected: no insecure fallback, discarded credentials or unbounded shutdown.
7. **Connection/control/code conformance.** Complete missing connection/retry/proxy/health classifications,
   control response/rate/stream branches, MCP errors/instructions and code-engine conformance. Expected:
   actual protocol fixtures, real loopback transports and failure/revocation pressure tests. External services
   needing unavailable credentials remain a named evidence dependency, not a local fake pass.
8. **Architecture and final qualification.** Run bounded Windows UI/browser/editor feasibility probes needed
   by phase 1, record source dependencies on phases 7–15 without pretending they are implemented, execute full
   Windows suite/release/smoke/size and available Linux checks, obtain one fresh whole-scope review and repair
   Important/Critical findings in one RED→GREEN pass. Expected: versioned gap-by-gap receipt, commits and summary.

## Execution ledger

- Task 1: root Git initialized, baseline `68f7954`, feature branch `feat/close-phase1-6-partials`.
  Upstream remains ignored and unmodified; source/oracles/docs/audit metadata are tracked; binaries stay local.
- Pre-flight: Tasks 2/3/4 share file/process custody and authorization; retain current APIs and recheck live
  grants at publication. Tasks 5/7 consume existing immutable request/epoch owners; never fork identity authority.
- Platform ruling: Windows is active; macOS/other platforms HOLD under the user's explicit steering. Linux
  proof is optional where practical. No platform is silently removed from the eventual upstream roadmap.
- Environment: Docker Desktop startup failed at a pre-existing inaccessible AF_UNIX runtime endpoint.
  Its stopped, verified two-entry run directory was renamed for recovery, preserving every old endpoint.
  No image/volume disk, settings or factory reset was changed. Engine recovered; a pinned Rust Linux
  container executed the committed baseline: 112 PASS, 0 failed/ignored, including native Unix custody.
- Task 1: CONTRIBUTING and a pinned-action Windows/Linux CI workflow added. GitHub execution is NOT RUN;
  local native Windows/Linux checks remain the actual evidence. The upstream clone is unnecessary for
  normal Rust tests; checked-in source-derived fixtures retain provenance.
- Task 2: actual Linux baseline closes the previous compile-only Unix process cleanup qualification
  for this Linux x64/WSL2 environment. It does not qualify macOS or external SSH authentication.
- Task 3a: 35 actual upstream Git parser/projection vectors captured; Git snapshots/diffs/branch
  comparison and metadata containment implemented. Functional RED→GREEN covers unborn/untracked,
  rename/binary, committed-vs-dirty comparison, stale revision and legacy metadata escape refusal.
  CLI review/diff integration also passed RED→GREEN after exposing a stored Windows canonical-path bug.
- Task 2/3 shared repair: re-project trusted catalog paths through currently approved virtual roots,
  retaining strict untrusted path syntax. Native project terminal creation and proven-session default
  patch cwd each reproduced the Windows failure, then passed with root revocation still refused.
- Ruling: qualify the existing baseline on Linux while porting independent Git services; they share
  no new producer dependency. Cost if wrong: Linux evidence applies only to `68f7954`, until the new
  source is separately synchronized and tested. Final integrated qualification must name its commit.
- Task 3a / shared workspace repair: integrated Windows suite **126 PASS, 0 failed/ignored**;
  strict Clippy PASS. The first unrestricted parallel link exhausted Windows paging (1455); its
  failure log is retained and the same suite passed with `-j 2`, without changing Windows settings.
  Git watcher/file-panel/project-lifecycle work and the remaining tasks are still open.

Task details, commits, RED/GREEN evidence and outstanding acceptance are appended during implementation.
