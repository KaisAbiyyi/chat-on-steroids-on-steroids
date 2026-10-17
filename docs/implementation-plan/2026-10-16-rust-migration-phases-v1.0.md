# CoS-Rust: full-parity migration phase assessment v1.0

Date: **2026-10-16, Asia/Jakarta**. Status: **proposed roadmap**, based on the completed static scan.
This is a phase and acceptance assessment, not an approved task-by-task implementation specification.
No Rust product code has been created. Phase 1 is PARTIAL from this investigation; phases 2–16 are NOT STARTED.

## Scope dan dasar keputusan

Tujuan pengguna: Chat On Steroids lebih kecil, dipindahkan ke Rust, dengan semua fitur dan perilaku
dipertahankan. Baseline source: `024731d8083d92c46e8248d64e7e0b36ab0a470f`; measured installer:
`v2.1.39` / `87ae9d631f82d8da2f462d910410031ad8f97ac5`. Main is 87 commits ahead of the release.

The recommended decomposition is **16 phases**, including baseline/feasibility and final qualification.
Some phases can overlap once their interfaces are frozen; merging phases would not remove their work.
All 539 product files and all 1,442 tracked files have a proposed primary owner in the
[migration file map](../../audit/2026-10-16/migration-file-map.csv).
One owner per file is a planning device, not a claim that a file's cross-feature dependencies are absent.

The requested whole-product scope includes Windows, macOS and Linux, x64 and arm64, preserving current
platform-specific availability. Windows is the only release payload measured in this investigation.
Linux's current lack of native desktop automation must remain explicit rather than being called a failed
port or silently replaced with a different feature set.

Keep current tool names/schemas/errors, bridge protocol 14, session/request/turn identities, durable data,
13 capability flags, 13 UI languages, native upload behavior, supported browser paths, plugins, Skills,
workers, Goal/Loop, recovery, editor/PDF/math/terminal features, pets, settings and updater behavior.
Any intended bug fix or accepted deviation needs its own decision and evidence.

## Rust boundary and early decisions

The proposed target is Rust-owned services and desktop UI. A native UI toolkit and browser host are **not
selected yet**: the strict visual/editor/browser requirements need early probes. Rust/Wasm is a candidate
for extension logic; JavaScript browser entry/API glue remains necessary under Chrome MV3. The `exec`
tool must still execute bounded JavaScript. Optional third-party plugin runtimes and tunnel sidecars keep
their original languages unless separately replaced with proved-compatible implementations.

Literal zero non-Rust runtime across this ecosystem is not a feasible promise with unchanged features.
A Rust backend with the existing HTML/TypeScript renderer is a valid size-focused alternative, but it is
a hybrid and must not be reported as a complete Rust UI conversion.

Proposed ownership layout, to validate in phase 1:

| Target component | Responsibility |
| --- | --- |
| `cos-core` | Typed contracts, configuration, permission checks, secure storage, cancellation and durable primitives |
| `cos-store` | Session identity, history, chronology, message/assets/indexes, usage and legacy reads |
| `cos-mcp` | MCP surfaces, ingress, code mode, tunnel lifecycle and local control projections |
| `cos-tools` / `cos-exec` | Files/patch/search/projects/SSH files; processes/PTY/shell policy/SSH execution |
| `cos-bridge` / `extension-wasm` | Browser wire, document custody, observations, actions and companion state |
| `cos-browser` | Embedded browser host, profile, Chrome API compatibility and sign-in |
| `cos-workflow` | Independently owned outbox, continuation, agents and Goal modules |
| `cos-plugins` | Plugin and Skill lifecycle, OAuth, package verification and optional runtime adapters |
| `cos-desktop` | OS capture/input/accessibility/clipboard, dictation, image pipeline and pet overlays |
| `cos-ui` / `cos-app` | Desktop views/commands plus bootstrap, shutdown, updater and release assembly |

Crate boundaries are a proposal. No dependency on a whole large agent runtime is justified merely because
some current TypeScript tools were originally ported from Rust. Reuse small compatible leaf components
only after pinning source, comparing CoS-specific behavior, and accounting for licenses and size.

## The 16 phases

| Phase | Deliverable | Depends on | Relative risk |
| --- | --- | --- | --- |
| 1 | Frozen parity baseline and architecture probes | — | Critical decision gate |
| 2 | Rust runtime, config, permissions, secrets and durable primitives | 1 | High |
| 3 | Sessions, recording, identity, history and usage | 2 | Very high |
| 4 | MCP, tunnels, connection, code mode and local control API | 2, 3 | High |
| 5 | File tools, patches, search, projects, Git and SSH file access | 2–4 | High |
| 6 | Commands, PTYs, terminals, shell policy and SSH execution | 2–5 | High |
| 7 | Browser bridge, companion and direct browser tools | 2–4 | Very high |
| 8 | Built-in browser and sign-in | 3, 7 | Critical feasibility |
| 9 | Outbox, uploads, steering, finish and generated workflows | 3, 4, 7 | Very high |
| 10 | Compact & Resume and recovery | 3, 7–9 | Very high |
| 11 | Workers, inboxes, worktrees and worker archive | 3, 5–7, 9, 10 | Very high |
| 12 | Goal, Loop, planners and helper decisions | 9–11 | Very high |
| 13 | Plugins, Skills, OAuth and optional runtimes | 2, 4–6, 9 | High |
| 14 | Native desktop, media, dictation and pets | 2–5, 7, 11 | Very high |
| 15 | Complete desktop UI, rich panels and localization | 3, 5, 6, 8–14 | Very high |
| 16 | Data migration, cross-platform qualification, packaging and size | 1–15 | Release gate |

### Phase 1 — Freeze baseline and prove architecture

Inventory and current file hashes are done; release size and source/release divergence are measured.
Still required: capture the actual app's tool discovery, wire/error behavior, supported config states,
history fixtures, UI screenshots, process memory, startup/interaction measurements and representative
long workflows. Use isolated fixtures and test profiles, with permission for any live provider actions.

Run disposable feasibility probes for: native rich text/code editor/PDF/math/terminal fidelity; embedded
browser extension/debugger/sign-in on each target OS; existing encrypted-secret migration; bounded JS
interpreter behavior; Rust/Wasm access to MAIN/isolated browser worlds under actual CSP and MV3 suspension.

**Gate:** a concrete parity specification and non-Rust dependency/deviation register, supported by probe
evidence. A browser/GUI contradiction is resolved here before a long rewrite. Candidate budgets are
installer ≤80 MiB and app payload ≤200 MiB on Windows x64; freeze actual approved budgets after probes.
No reduction in history, features or platform scope is implicit permission to satisfy those budgets.

### Phase 2 — Runtime, config, permissions, secrets and durable primitives

Owners: `index.ts`, `config.ts`, `sandbox.ts`, `secrets.ts`, `durable.ts`, `shutdown.ts`, relevant shared types.
Port field-wise migrations, malformed-state behavior, effective grants, approved-root resolution, logging,
redaction, startup/shutdown and serialized durable writes. Introduce explicit state-owner interfaces rather
than new mirrored authorities. Validate existing encrypted data before attempting any replacement.

**Gate:** old config fixtures load with equivalent defaults/recovery; symlink/junction/case/virtual-path
negative tests preserve boundaries; failing writes never publish a grant/ACK; old secrets migrate safely
or a specifically accepted reauthentication deviation exists. Cancellation and shutdown remain bounded.

### Phase 3 — Sessions, recording, identity, history and usage

Owners: `session/store.ts`, `recorder.ts`, `correlation.ts`, `read-model.ts`, chronology, usage and provider-usage.
Keep JSONL events, canonical stable-id message shards, legacy message-map overlay, overflow assets, turn
traces, request correlations, read/search/export views and recorded model/effort attribution.

**Gate:** round-trip original fixtures without losing event order, originals, assets or audit history;
restart hydration cannot revive a completed generation; first proven request ownership cannot move;
frontend A→B replacement retains the local session epoch. Usage estimates remain distinct from provider
limits and actual billing. Large histories remain paged/bounded without destructive pruning.

### Phase 4 — MCP, tunnels, connections, code mode and local control API

Owners: `mcp/`, `connection.ts`, `tunnel/`, `control-*`, connector identity/proof and persistent paths.
Implement separate Core/Desktop/Plugins discovery, input validation and error formatting, live kernel
checks, HTTP origin/host/body/secret-path rules, streaming/drain semantics, tunnel supervision and opt-in
control API read/action separation. Keep JavaScript `exec` through a bounded interpreter and surface-local
tool bridges. Capability-dependent tools are advertised only when their real implementations are ready.

**Gate:** differential protocol fixtures for success/error/disconnect; cross-surface tool access refused;
permission revocation affects awaited nested calls; current code-mode limits and explicit emissions hold;
an accepted child side effect does not falsely roll back when the interpreter times out. No raw Node or
host authority leaks into the JS context. Tunnel status follows the actual process/generation.

### Phase 5 — Files, patches, search, projects, Git and SSH files

Owners: `rawfs.ts`, `fsops.ts`, `search.ts`, `ripgrep.ts`, `project-files.ts`, `projects.ts`, `workspace.ts`,
`project-git.ts`, `codex/apply-patch/`, `filesystem.ts`, `read-backend.ts`, `view-image.ts`, `remote/project-fs.ts`
and `remote/sftp.ts`. Preserve local and virtual paths, file previews, atomic edit preflight/rollback,
revision-checked saves, watcher lifetime, Git projection and OpenSSH-backed SFTP file behavior.

**Gate:** exact patch diagnostics and line/BOM/UTF-16/newline cases; a failed multi-file edit does not leave
unapproved partial changes; reads are bounded and reject nonregular handles; stale saves cannot overwrite
external revisions. Search ordering/exclusions and image previews match fixtures. SSH retains realpath,
error-code, alias/agent/known-host semantics, including remote feature limitations already documented.

### Phase 6 — Commands, PTYs, terminals, shell policy and SSH execution

Owners: `codex/unified-exec*`, `manager.ts`, `ownership.ts`, shell/buffering/truncation modules,
`workspace-terminal*`, `shared/command-allowlist.ts`, `remote/ssh.ts` and execution hints.
Port process-session ownership, pipes/PTYs, shell/env defaults, output paging/cursors, unread completion,
interactive input, exit codes, interruption and process cleanup. Preserve host OpenSSH and login-shell
behavior; replacing it with another SSH stack is a reviewed change, not a transparent substitution.

**Gate:** original execution vectors, including Windows and POSIX shells, tty vs pipe, output truncation,
background completion, timeout, Ctrl-C and nonzero exit; one session cannot write into another's process.
Command allow/deny rules retain current syntax and explicitly remain a launch policy, not an OS sandbox.

### Phase 7 — Browser bridge, companion and direct browser tools

Owners: `bridge.ts`, browser launch/wake/preferences/proof modules, `browser-control.ts`, shared browser
contracts and all extension files. Port app-side authenticated routes and command/repair ownership to Rust.
For extension logic, use the phase-1-approved Rust/Wasm route with minimal browser glue, or record the
preserved JS as an explicit exception; do not hide it under a “100% Rust” label.

**Gate:** protocol 14 compatibility, document/navigation epochs, first attribution proof, duplicate-tab
elections, actual command claim/receipt/ACK barriers, explicit Disconnect and restart behavior. Browser
restart and MV3 suspension must not duplicate input or reopen a user-closed tab without authority. Direct
browser tools preserve exact handles, debugger ownership, snapshots and bounded results.

### Phase 8 — Built-in browser and sign-in

Owners: `cos-browser/`, its preload/toolbar/sign-in views and `shared/cos-browser-sites.ts`.
Replace Electron BaseWindow/WebContentsView hosting and the custom Chrome API/extension-worker adapter
with the approved browser host. Preserve tabs/windows/groups, selected document, persistent profile,
navigation, safe page schemes, debugger routing, browser preferences, sign-in transfer and exact errors.

**Gate:** end-to-end CoS extension workflows, multi-tab ownership and sign-in on all supported native
targets, not just a webview that displays chatgpt.com. Do not silently substitute external-browser-only
operation or bundle a full fixed browser while claiming it has no distribution cost. If parity is not
feasible, return to the explicit phase-1 architecture decision before progressing on that assumption.

### Phase 9 — Outbox, uploads, steering, finish and generated workflows

Owners: `session/input*`, `start-input.ts`, `prompt.ts`, `finish.ts`, `request-plans.ts`, approval wait,
turn rotation and shared input/plan contracts. Model each frozen input UUID, exclusive claim, native files,
steering recipient, finish hold and workflow checkpoint under one authority.

**Gate:** queued, inserted and actually accepted are distinct; a retry cannot duplicate an ambiguous Send;
attachment bytes are immutable and reach the native upload path; delayed events cannot affect a different
draft/turn. Test queue edit/cancel/reorder races, worker wait, direct-turn rotation and user corrections.

### Phase 10 — Compact & Resume and recovery

Owners: `session/continuation.ts`, `resume-gate.ts`, handoff modules, shared recovery and bridge repair hooks.
Port source/destination checkpoints, same-session rebind, continuation WAL, project lineage, pending input,
trust/blocked rules, saved summary recovery and operation-specific repair clocks/budgets.

**Gate:** crash/restart at every send/lease/commit boundary; no phantom replacement session, lost queue,
resurrected retired owner or repeat destination send. Simulate uncertain receipts without converting them
to safe-retry claims. Pro/ordinary/helper models retain their distinct current recovery eligibility.

### Phase 11 — Workers, inboxes, worktrees and worker archive

Owners: `agents.ts`, `agent-admission.ts`, `worktrees/`, `worker-archive.ts`, worker brief/health contracts.
Port prime families/run incarnations, admission/slots, addressed messages and receipts, sleeping/reused
histories, retire fences, permitted worktree creation/disposal and optional provider archive scheduling.

**Gate:** two independent prime families cannot steal each other's worker or message; failed durable
mutations are not published/opened; worker wake/reuse preserves context and custody; cleanup accepts only
marker-proven owned worktrees. Current opt-in defaults and archive retry/deferred semantics remain exact.

### Phase 12 — Goal, Loop, planning and helper decisions

Owners: `goal.ts`, helper sizing, `task-request.ts` and shared Goal templates/reasoning/history/errors.
Separate durable objective, switch, obligation/reply debt and replaceable decision attempts. Preserve
ChatGPT/OpenRouter/custom providers, observed model choices, generated workflows, wait-for-workers,
Goal's ability to finish and Loop's continuing behavior within the user's brief.

**Gate:** restart, Off→On, provider/secret changes and stale helper replies cannot resurrect canceled debt
or overwrite newer intent. A completed turn is evidence-driven; interim prose or failure is not success.
Reproduce documented atomicity gaps before deciding how they should be fixed in the parity specification.

### Phase 13 — Plugins, Skills, OAuth and optional runtimes

Owners: `plugins/`, plugin IPC/refresh, Skill import/library/GitHub/routing/package modules and user skills.
Port catalog, MCP stdio/HTTP clients, OAuth, installed-version/license provenance, package path checks,
runtime/process cleanup, profile isolation, per-tool exposure and revision-pinned Skill selection.

**Gate:** npm/Python/remote integrations work through their real runtime contracts; optional runtime
downloads remain verified and on demand; revoked exposure cannot be called through code mode. Skill
archives cannot escape import roots; updates preserve user choices, content identity and secrets. Core
can start without installing every plugin runtime or bundling Playwright browser downloads.

### Phase 14 — Native desktop, media, dictation and pets

Owners: `computer/`, native macOS helper/addon, dictation, image export and pet library/overlay/focus/rendering.
Replace Windows PowerShell/C# helpers and macOS Swift/N-API host logic with Rust OS adapters where feasible;
retain necessary system ABI/library calls. Preserve Windows.Graphics.Capture, UI Automation, foreground
rules, stale accessibility refs, frame/DPI/capture generation, clipboard order and macOS permission identity.

**Gate:** actual background screenshots/input on Windows; actual ScreenCaptureKit/accessibility/consent
on macOS; stale frame/ref and partial-batch negative cases. Image fixtures cover orientation/color/alpha,
format limits and previews. Dictation targets its original draft and never sends automatically. Pet assets,
interaction, focus forwarding and lifecycle cleanup remain available. Platform nonavailability is truthful.

### Phase 15 — Complete desktop UI, rich panels and localization

Owners: fixed IPC/preload API, renderer chat/main/settings/sidebar/composer/agent/Skill/plugin views,
file code/PDF viewers, terminal, math/markdown, appearance, resize/docks, setup help, notices and shortcuts.
Recreate the full workspace in the approved Rust UI stack. Preserve all 13 languages, fonts/assets,
themes, keyboard and input behavior, drag/drop, drafts, selection generations, paged history, tool rows,
code edits/diffs, PDFs, math, ANSI terminals, desktop accessibility and tray/window behavior.

**Gate:** screenshot and interaction comparisons at fixed font/DPI/theme/language, plus real long-history
and mixed-media fixtures. A simpler editor, plain-text math, image-only PDF, terminal log textarea or
missing settings is a failed parity gate. Pixel equivalence cannot be assumed from native Rust rendering;
phase 1 must establish what is accepted and how dynamic regions are controlled.

### Phase 16 — Migration, qualification, packaging and size gates

Owners: updater/update-all/progress/readiness, diagnostics, packaging scripts, notice/source checks, CI and
whole-product acceptance. Integrate the full scope, migrate copied data transactionally, verify rollback,
separate application state ownership, finalize shutdown/update handoff and build six native OS/arch targets.

**Gate:** functional/wire/data/visual/security parity ledger with no unexplained regressions; representative
actual browser/provider/desktop workflows; clean install, upgrade, downgrade/read-only rollback behavior,
no lost session/audit data and no imported-test-profile contamination. Report installer, unpacked payload,
shared/optional runtime costs, all owned process RSS, startup and interaction latency. Hash and inspect
actual artifacts, preserve licenses/source obligations and target-specific availability.

## Verification strategy

Port or retain the current 382 test files as behavioral oracles where valid; not every test must become
Rust, especially browser/DOM acceptance tests. Test names and counts alone are not proof. Keep an explicit
status for every gate: PASS, FAIL, PARTIAL, BLOCKED or NOT RUN, tied to the exact source/build identity.

The most likely failure classes needing dedicated evidence are:

1. Restart/cancellation after a side effect but before its receipt, including duplicate tabs and MV3 suspension.
2. Two concurrent prime families or old frontend epochs attempting to claim the same worker/input/process.
3. Unicode/BOM/UTF-16 paths and files, shell quoting, symlinks/junctions and remote POSIX paths on Windows.
4. Large histories, image/PDF/math/code/terminal combinations and stale renderer draft/save generations.
5. Existing encrypted secrets/config/session data migrating across real OS permission and update boundaries.

The [deep scan](../research/2026-10-16-chat-on-steroids-deep-scan-v1.0.md) distinguishes all completed
investigation from work still needed. No implementation, visual acceptance, runtime speedup or Rust package
size is claimed by this roadmap. The current source clone remains unchanged and reviewable.
