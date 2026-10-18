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

- Shared workspace regression: a proven session bound to a missing project previously fell back
  to another approved folder. `missing_bound_project_cannot_fallback_to_another_approved_folder`
  reproduced the unwanted write (RED), then refused the missing binding with no file created (GREEN).
  Integrated Windows suite: **127 PASS, 0 failed/ignored; strict Clippy PASS**.
- Task 2: real Windows OpenSSH client authenticated to an isolated Linux sshd using only generated
  public-fixture keys, private Host config and private known_hosts. Six checks PASS: authentication/
  alias, changed host key refusal, unauthorized identity refusal, SFTP read/patch, symlink escape
  refusal, command cwd/output/nonzero exit. The harness aggregates unread process chunks as
  required by the existing process contract. External/public SSH and adversarial SFTP remain separate.
- Linux checkpoint `979f680`: **122 PASS, 0 failed/ignored** on the pinned x64 Rust image, now
  including the Git implementation. The subsequent missing-project fix is not covered by this receipt.
- Task 3b: project colors and soft removal preserve workspace/request identity and survive restart.
  Native-project creation now has exclusive 60-second claims, armed/linked/failed/uncertain states,
  explicit retry, gone-link handling, bounded change subscriptions and an app-owned expiry clock.
  Seven functional regressions were demonstrated RED→GREEN, including a restored armed attempt
  expiring without browser polling and preservation of unrelated legacy rows. A further differential
  test matches **27 actual upstream lifecycle steps** (five scenarios), with source hashes retained.
  CLI color/remove integration reproduced the unknown-command failure and passed after integration.
  Integrated Windows suite **136 PASS, 0 failed/ignored**; strict Clippy and formatting PASS.
  Browser delivery/creation remains a phase-7 integration dependency; this backend never clicks ChatGPT.
- Linux checkpoint `c3db4af`: **132 PASS, 0 failed/ignored**, including the native-project lifecycle.
- Task 3c: native Git watchers cover nested worktree writes and linked-worktree common refs, coalesce
  invalidation signals, discard prior project generations and refuse publication after root revocation.
  All four new functional regressions were demonstrated RED→GREEN; strict Clippy PASS.
- Integrated watcher qualification exposed one intermittent pre-existing two-second tunnel-drain
  timeout. Both isolated and concurrent retries passed before repair; its specific scheduling cause
  was not reproduced. Keep the failed run as evidence rather than substituting the retry.
  Investigation separately reproduced a deterministic defect: a pending output publication prevented
  explicit sidecar stop. That regression passed RED→GREEN after host retirement began waiting on
  child/pipe state without consuming a response page. Failure paths retain custody/work directories
  and monotonically advance observer generations. The two-second deadline and 70-cycle test remain.
  A real-child delayed-reaper test also checks timeout custody and successful retry. The repaired
  integrated checkpoint is **142 Windows tests PASS, 0 failed/ignored**, strict Clippy/formatting PASS,
  including the unchanged 70-cycle stress test. Preserve the earlier failed run separately; the
  original intermittent scheduling cause is not claimed to be uniquely identified by this repair.
- Task 3d (continued on 2026-10-18): native human file-panel list/preview/create/rename/save now
  has selected-project link refusal, held-directory/file identity checks, bounded text/image/PDF
  reads, source-style optimistic revisions and atomic replacement. Source directory sorting and
  truncation match two actual upstream scenarios; 33 path/name/binary helper vectors are checked in.
  Native Windows relocation tests exercise the real OS sharing restriction and replaced-source
  refusal. CLI routing was demonstrated RED (unknown command) then GREEN, including stale save
  and root revocation. The local checkpoint passed 157 Windows tests before the subsequent SFTP work.
- Task 3d source differential repairs: exact native `textSnapshot` and `previewProjectFile` comparisons
  exposed rounded-versus-truncated timestamp branches, text disguised with an oversized image extension,
  and JS Number serialization of an inode above 2^53. Each functional failure was observed before repair.
  Three native text snapshots/save inputs and ten complete preview projections now match the pinned
  source, including BOM/CRLF, invalid UTF-8, controls, large text, image rejection and PDF headers.
- Task 3d server paths: host file-panel reads route through the existing explicit lifetime trust gate.
  Native SFTP proves listing/text preview, unavailable image/PDF projection, refused edits, a streamed
  600,000-byte attachment, automatic snapshot cleanup, 512 MiB admission and mid-stream root revocation.
  A failed copy closes its SFTP handle and leaves the transport usable. No new model tool is advertised.
  The server trust restriction remains; adversarial opened-handle containment is still unqualified.
- File-panel acceptance still open: AVIF decode and full Sharp raster/pixel/encoder comparisons,
  all-locale/ICU-version ordering, remaining OS mutation races and later UI/attach/trash wiring.
  Local directory scan and existing SFTP directory budgets remain explicit resource differences.
  Integrated Windows checkpoint: **160 PASS, 0 failed/ignored; strict Clippy and formatting PASS**.
  Exact commit `10d4c81` passed **156 Linux tests, 0 failed/ignored**, including native Unix
  parent/source relocation, Git watches, sidecar retirement and SFTP. The release is 10.269 MiB
  (+1.793 MiB versus the previous headless service), with 17 runtime and 13 source checks PASS.
- Task 4a: actual patch update/seek functions now provide 36 update and 14 seek vectors across both
  update modes, EOF behavior, JS whitespace, out-of-order diagnostics and bounded anchored excerpts.
  Functional RED reproduced the generic out-of-order error, wrong model-facing newline mode, missing
  move-only support, missing readable excerpt and Add overwrite bypassing its Create permission.
  Focused tests are GREEN after both modes, typed diagnostics, current Read checks and separate
  syntax/effect authorization were integrated. Native SFTP verifies both modes, move-only preservation
  and typed diagnostics. Full Windows suite: **166 PASS, 0 failed/ignored**; strict Clippy and formatting
  PASS. Current patch-commit Linux qualification follows this checkpoint.
- Ruling: retain the existing Edit requirement whenever a context-only move would change actual
  bytes, including appending a newline to an unterminated file — upstream's preserve mode still
  appends that newline, while the frozen contract keeps Move separate from content editing.
  Cost: that upstream edge case remains a conservative compatibility deviation; the caller needs
  Edit or a byte-preserving rename. Exact CRLF/context-only moves now work with Move alone.

- Linux patch checkpoint `0e5457e`: **162 PASS, 0 failed/ignored**, recorded separately from the
  following streaming work. The first native all-target strict Clippy run exposed two Unix-only
  collapsible conditions and Windows-only test imports. These are corrected without changing behavior;
  the focused dirty-checkout Clippy run passes. A new exact-commit strict receipt follows the checkpoint.
- Task 4b: the native incremental parser now produces source-compatible hunk snapshots and environment
  IDs at each chunk, with strict/lenient entry points integrated into the normal parser. Three functional
  regressions were observed RED, then GREEN. The corpus covers 36 source streams and 10 full parse cases;
  aggregate admission runs before an unterminated buffer can exceed its limit. Integrated Windows suite:
  **169 PASS, 0 failed/ignored; strict Clippy PASS**. Shell interception/search/toolchain remain open.
- Docker recurrence: no Docker processes/engine were present, and the same two stale socket endpoints
  remained. Their verified runtime directory was preserved again, then the engine and owned container
  restarted. No Docker data disks, volumes or settings were reset. See the dated recovery receipt.

- Exact streaming checkpoint `67e905a`: **165 native Linux tests PASS, 0 failed/ignored**, formatting
  and strict all-target Clippy PASS. The receipt retains the exact source checkout and command exits.
- Task 4c: direct/heredoc/implicit patch invocation classification matches **504 actual upstream
  vectors**, including PowerShell/cmd wrappers, exact shell/path conventions, comments, extra commands,
  malformed patches and `cd` captures. Input admission precedes tree allocation. Three MCP regressions
  were observed RED (shell ParserError/no file), then GREEN: native interception without a child,
  approved existing cwd selection and correctness errors. Command revocation and the allowlist also
  refuse mutation. Shared patch execution rechecks current Command authority without inventing file-tool
  grants, as the source's command route already grants shell-equivalent access. Ordinary apply_patch
  retains its separate file permissions and conservative move ruling. Full Windows suite: **176 PASS,
  0 failed/ignored**, strict all-target Clippy and formatting PASS.
- Ruling: use the pinned Tree-sitter/Bash 0.25.1 Rust bindings for the source's exact Bash query.
  Grammar, external scanner and node types match the npm source; generated parser tables are identical,
  with only package patch-version metadata differing. Cost: this retains an MIT-licensed C parser
  dependency alongside the existing QuickJS exception; it is not a pure-Rust parser claim. A release
  size comparison follows this implementation checkpoint. Replacing it with another grammar would
  require fresh invocation conformance proof; 504 vectors are bounded evidence, not every Bash script.

- Invocation commit `23c1f95`: **172 Linux tests PASS, 0 failed/ignored**, strict Clippy and formatting
  PASS. Windows service release **12,299,776 bytes (11.731 MiB)**, +1,531,904 bytes against `10d4c81`
  for the patch-mode/streaming/invocation work together; 17 executable and 13 project-file source checks
  PASS. This measures the headless service, not a full desktop/installer or isolated compiler cost.

- Task 4d: Windows JDK/Go discovery is integrated into both pipe and PTY child preparation. It fills
  only missing unreachable toolchains, requires `javac.exe` for a JDK, ranks versions with source
  JS Number/UTF-16 semantics, memoizes discovery and bounds PATH reachability to 16 keys. A 34-step
  source corpus (12 scenarios) and five version lists pass; all three initial functional tests were RED.
  A native public filesystem/compiler-selection and real cmd child prove child-only environment changes.
- A further real Windows subprocess regression reproduced native unpaired UTF-16 `JAVA_HOME` being
  treated as absent. The repair preserves presence; unrepresentable PATH/discovery inputs suppress
  automatic discovery, preserving inheritance and explicit overrides. Both native UTF-16 cases pass
  after observed RED. This does not close global JSON/unpaired-string parity. Current Windows suite:
  **180 PASS, 0 failed/ignored**, strict Clippy PASS. The last Linux receipt still applies to `23c1f95`.
- Ruling: if native PATH/discovery inputs cannot be represented losslessly by this UTF-8 owner,
  preserve them and explicit overrides. Cost: automatic JDK/Go discovery in that uncommon native
  UTF-16 case remains a conservative compatibility difference until the global representation gate
  is resolved. A pre-existing JAVA_HOME/GOROOT choice is never replaced as a consequence.
- Search investigation captured 160 include-glob, 60 exclusion, 12 scan and six ordered-tree source
  scenarios. These are investigation artifacts; native search integration and its source exactness
  proof have not started. The UTF-16 clipping boundary includes the existing global unpaired-string gate.

- Toolchain commit `6839269` passes **172 native Linux tests**, strict Clippy and formatting; its four
  Windows-only toolchain tests are platform-specific, not ignored Linux tests.
- Task 4e: a real native patch and native SFTP both reproduced acceptance of a move whose source and
  destination resolve to the same path. Both now refuse during preflight; earlier hunks remain unapplied.
  Twelve actual upstream dry-run verifier scenarios capture local Windows case folding and remote/Unix
  case sensitivity. Native local results consume the applicable three scenarios; Linux SFTP additionally
  verifies a distinct case-only destination. Windows integrated result: **181 PASS, 0 failed/ignored**,
  strict Clippy and formatting PASS. Error/permission/resource boundaries from earlier checkpoints remain.

- Same-path move checkpoint `3a446b0`: **173 Linux tests PASS**, 0 failed/ignored, formatting
  and strict Clippy PASS. The receipt applies to that exact commit.
- Task 4e: source include globs now match basename fallback, case rules and UTF-16 units;
  name queries match filenames rather than parent paths, and Unicode folder exclusions use
  source lowercase semantics. Four functional regressions were demonstrated RED then GREEN.
  Checked-in corpora retain 1,064 platform-specific glob rows and 117 exclusions; native
  integrated Windows suite **185 PASS, 0 failed/ignored**, strict Clippy and formatting PASS.
  BFS traversal, all-root defaults, content/remote search and model response parity remain open.
- Storage steering: the first combined search run failed with OS 112 (StorageFull) while a
  512 MiB fixture was allocated in C Temp. Its log is retained. The same tests passed with
  TEMP/TMP on D; neither assertions nor fixture sizes were reduced. A reusable local runner
  now puts child temp files in the workspace and reserves free space. Linux qualification
  gains a lean profile (debug symbols/incremental disabled) and records its storage choices.
  At continuation C already had about 42 GiB free and the old Linux cache was absent;
  that intervening free-space change is not attributed to this implementation.

- Matcher checkpoint `531fcac`: **177 Linux tests PASS**, formatting and strict Clippy PASS.
  Its fresh lean build cache is **998 MiB**, with no incremental/debug-symbol state; source archives
  are staged on D. This is a new cache measurement, not a claim that this task deleted the old cache.
- Task 4f: model search now walks breadth first in native entry order, counts regular files before
  include filtering, stops name traversal at the result limit, and retains source file/directory
  budgets. A bounded queue keeps the same first 10,000 directory prefix. Omitted paths search every
  approved root with one whole-tool deadline; explicit files use the source response and size refusal.
  Six native/MCP regressions were observed RED then GREEN. Six actual source trees, two source bounds
  and eight executions of the actual registered find handler qualify ordering/counts/text projections.
- Task 4f content: lossy TextDecoder behavior, BOM-aware binary sniffing, LF/CR splitting and JS trim
  match 11 representable source cases. Twelve initial content jobs are admitted before completion;
  native reads overlap and results publish in completion order. All grant checks run on the calling
  thread, including during worker reads and publication. A native twelve-reader gate qualifies both
  success and mid-flight revocation. Source-handler differential testing exposed wrong size formatting
  and an incorrect truncation flag after one exact hit plus an already-admitted oversized file; both
  passed RED to GREEN after the source formatting and scheduling rules were integrated.
- Task 4f server search: native OpenSSH SFTP proves BFS name results, content reads, all-root routing,
  runtime-only path trust, multi-chunk root revocation, handle closure and usable transport afterward.
  The twelve jobs share the existing mutable SFTP request owner, so wire requests remain serialized.
  JS-regex behavior, local bundled-ripgrep conformance and the existing 5,000-entry server directory
  cap remain open; these are not claimed closed by the new source response comparisons.
- Qualification repairs: Windows refused a held-directory relocation with native sharing violation
  32 (Rust categorizes it as Uncategorized). The test now asserts that observed native code and
  preserved source/destination state; Linux exercises the successful relocation/refused publication
  branch. A further regression found the legacy search adapter resolving a path before its grant
  check; it now refuses before resolution (observed RED to GREEN). Integrated Windows result:
  **201 PASS, 0 failed/ignored**, formatting and strict all-target Clippy PASS.
- Ruling: keep the original unpaired UTF-16 clipping result as code units in the source corpus and
  explicitly refuse an unrepresentable output instead of substituting or dropping a code unit.
  Cost: that content result remains a compatibility gap until the whole-workspace lossless JSON
  string owner is implemented. Eleven content projections pass; the twelfth is a refusal test,
  not an exactness pass. Counted reads also retain the existing hard 2 MiB growth ceiling.

- Linux checkpoint `c1ae547` failed one native handler oracle (63 tests completed PASS, 1 FAIL;
  remaining test binaries NOT RUN because Cargo stopped). Formatting/strict Clippy passed. The
  exact mismatch was input ordering: Linux enumerated the nonmatching file before the hit, so
  source semantics correctly require files_scanned 2; the fixture supplied the reverse order
  and expected 1. Both two-entry permutations are now executed through the actual source handler
  (16 projections). The native test selects the captured provider input using the filesystem's
  actual order, and still compares the entire response exactly. Product traversal is unchanged;
  no counts are normalized, cases removed or thresholds relaxed. The failed Linux receipt is
  preserved. The updated fixture passed the full Windows suite again: **201 PASS**, strict checks.

- Search checkpoint `4a40c93`: **193 native Linux tests PASS**, 0 failed/ignored, formatting and
  strict Clippy PASS; the shared lean cache measures 1.1 GiB. The Windows release is 12,502,528
  bytes (11.923 MiB), +202,752 bytes against `23c1f95` for toolchain/search work together. It passes
  17 runtime, 13 project-file source comparisons and 11 actual search/MCP/code-worker checks.
- Scope ruling: captured content and handler projections exercise the original public filesystem
  fallback provider. They do not close all local bundled-ripgrep behavior, partial-read hit retention
  or chunk-boundary exactness. These remain open with server JavaScript regex, global unpaired JSON
  strings and SFTP pipelining/entry caps; do not extrapolate the bounded source comparisons.
- Storage ruling: dev/test profiles default to symbols 0 and incremental false, preserving default
  assertions, overflow checks and all acceptance/file-size limits. Debugger overrides are documented.
  Cost: less incremental reuse and explicit symbol opt-in when debugging. After exact scope/reparse/
  process verification, Cargo's development-profile cleanup removed 39,243 files (25.394 GiB), keeping
  release archives. The cold rebuild passed **201 Windows tests**, formatting and strict Clippy; its
  cache measures **1.464 GiB**. No global Docker prune, user-data cleanup or factory reset occurred.
  Linux defaults to the same lean settings; an exact committed default-run receipt follows.
- Next: Task 5 source title/prompt/rename/summary/history contracts. Tasks 2–8 and final review remain
  in progress; this checkpoint is not whole-application or whole-phase exactness acceptance.
- Exact default-profile checkpoint `628b027` passed **193 native Linux tests**, formatting and strict
  Clippy with a 1.1 GiB shared cache. No explicit lean switch was supplied; recorded Cargo environment
  confirms symbols 0/incremental 0. Task 5 implementation starts after this storage checkpoint.
- Task 5a: actual prompt/title/store captures now cover 17 framing inputs, 51 title previews (50
  representable, one unpaired-unit case), 39 legacy/authority projections and 44 persistent-store
  steps. Framed first-request titles, revisions, manual/provider precedence, worker/desktop rules,
  planner-helper classification and clear-name restore are ported. Four native defects and the
  missing CLI route were observed RED then GREEN. Rename/clear stage and durably write metadata
  before publication under the existing session fence; the CLI exposes both host operations.
- Task 5a metadata/recovery: 45 actual normalizer cases plus four damaged inputs and actual store
  reads qualify title/automatic title, model, activity, worker, finish, lineage and legacy defaults.
  A real Windows exclusive file lock reproduced an unreadable primary being treated as absence;
  read failures now propagate unless a validated backup answers. Three actual source crash snapshots
  qualify stale canonical revisions, metadata-only future fields, original timestamps, missing outcome
  counters and the context-token delta after rebind. Native RED exposed the prior whitelist dropping
  metadata and changing startedAt; the source merge now retains these fields and persists recovery.
- Task 5b activity: 38 actual reducer steps qualify recent activity, fractional timestamps, typed
  finish decisions/safe integer sequences, optional identity and taxonomy. Four independent regressions
  were observed RED then GREEN. One unpaired activity-title slice remains a refusal gap, separately
  from the 37 representable source projections. Combined workspace qualification is running.
- Ruling: reuse the global unpaired-JSON limitation for derived title/activity previews. Canonical raw
  events/messages survive; a split surrogate does not become U+FFFD or a scalar-truncated exactness
  claim. Rename refuses before committing an unrepresentable title; automatic message previews retain
  their prior name, and unrepresentable activity previews are unavailable. Cost: these remain explicit
  compatibility gaps until the lossless JSON owner is implemented across the workspace.
- Source qualification distinction: pure normalizeSummary and a complete durable store read have
  different outputs when normalization removes finishTurn. Both are captured; the full-store check
  compares the actual full-store output for every case, rather than skipping invalid legacy inputs.
- Task 5 checkpoint: **219 Windows tests PASS**, 0 failed/ignored, formatting and strict Clippy PASS.
  The complete store suite has 41 tests. Native backup-under-lock and exact raw activity retention
  survive restart; failed metadata writes expose no uncommitted label. Cache is 1.684 GiB on D.
- Historical oracle correction: the first combined run completed 123 PASS / 1 FAIL before stopping.
  It compared the warm historical summary to a cold load. Actual pinned source execution of the
  unchanged baseline files confirms the cold activity without kind is null; events/recent are exact.
  Keep warm and cold assertions separately and the original failed receipt; no product/threshold
  change was needed. Current-commit Linux qualification follows. Recorder and remaining tasks stay open.
- Exact session checkpoint `88c0867`: **209 native Linux tests PASS**, formatting and strict Clippy,
  shared cache 1.1 GiB. The two Windows file-lock cases account for part of the platform-specific count.
- Task 5 follow-up: source metadata read corpus adds four escaped-string inputs and seven native
  byte-decoding inputs. Two functional RED failures reproduced a valid source unpaired string being
  mistaken for damage/answered from an older backup, and Node-compatible UTF-8 decoding being skipped.
  Both focused regressions are now GREEN. The unpaired primary is explicitly Unsupported before any
  fallback or replacement, preserving both files; paired/literal escapes and byte decoding match source.
  This protects data while keeping global unpaired JSON exactness OPEN. Integrated proof follows.
- Task 5 recorder/model subset: 17 actual observeSessionModel operations qualify current-conversation
  and observed-time authority, format checks, non-finite/repeated values, rebind and restart. Native
  recorder tests reproduced unsupported presentation kinds and rejected raw request text, then passed
  after typed metadata routing and native text/authored-time handling. Provider title promotion runs
  after canonical writes, preserving desktop/manual/project-page rules. Publication uses the existing
  fence and durable summary owner; a failed write leaves the committed selection unchanged.
- A further functional rebind test exposed recursive locking of the recorder binding cache. Its
  five-second bound demonstrated RED; copying the binding before session lookup/removal passed GREEN.
  Retired or disabled model observations leave the replacement's selection unchanged. The failed
  unbounded attempt and bounded failure are retained. Broad source recorder/proof branches are still
  open; this routing subset does not qualify the later browser producer or inferred ownership.
- Combined metadata-string/recorder checkpoint: **225 native Windows tests PASS**, 0 failed/ignored,
  formatting and strict Clippy PASS. Initial full tests passed but strict Clippy rejected a redundant
  default struct update; removing that syntax and rerunning the entire suite passed. Both receipts
  are retained. Windows cache 1.728 GiB; no assertions, timeouts or file-size limits were relaxed.
  Linux exact-commit proof follows; content/DIL/Markdown projection work is next within Task 5.
- Exact metadata/observation checkpoint `caa234a`: **215 native Linux tests PASS**, formatting and
  strict Clippy, shared cache still 1.1 GiB. Windows checkpoint is 225 PASS; later content changes
  require their own integrated proof.
- Task 5c: capture actual content/directive/HTML/provider-entity functions, DIL tables, transcript/
  answer selection and main-process export projection. The corpus has 122 content rows, 24 DIL
  rows, seven transcripts, eight filenames and eight session/answer export outcomes. Source reads
  use public synthetic data and a memory-only text sink; real clipboard/dialog acceptance is NOT RUN.
  Native RED exposed raw link markers, missing table rows/dead-button removal and unconditional
  overflow hydration. CLI RED reproduced exporting a full session for answer scope. All are now GREEN.
- Additional source regressions exposed an unpaired intermediate HTML string wrongly refusing a
  boolean predicate and numeric entity errors using Rust number formatting. Exact UTF-16 units stay
  internal to HTML processing; predicate-only foreign-unit mapping cannot become output text. Three
  source outputs still need unpaired JSON strings and are explicitly refused. Numeric errors use the
  existing pinned pure-Rust ryu-js 1.0.3 dependency and correctly rounded hexadecimal Numbers.
  Strict Clippy passes after hoisting cached regex references out of loops; integrated proof is running.
- Source compatibility scope: Markdown applies completedDilText then providerEntitiesAsMarkdown;
  modelFacingText remains a separately qualified library contract for its later final-response owner.
  The original Rust JSON archive stays a separate extension. Asset read bounds and explicit read
  failures remain; native UI copy/save and every-locale/Unicode-version proof are not implied here.
- Content/Markdown checkpoint: **237 Windows tests PASS**, 0 failed/ignored, formatting and strict
  Clippy PASS. Source errors include correctly rounded hex and JS Number formatting across 2^53,
  scientific notation and overflow. The escaped-marker corpus tests 199/200/201-unit boundaries.
  All eight actual source session/answer outcomes are compared, including incomplete-turn refusal.
  Cache 2.241 GiB; Windows assertions/resource limits unchanged. Exact Linux/release proof follows.
