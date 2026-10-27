# 15HOST Authenticated Native Renderer Implementation Plan

> **For agentic workers:** Use the existing CoSoS typed UI contracts; keep every step within 15HOST and preserve the `native-ui-renderer` and `ui-integration` locks.

**Goal:** Replace the synthetic-only renderer foundation with an authenticated Rust-to-WebView2 UI bridge, bounded real session history, rich panels, localization/focus, and truthful native screenshot evidence.

**Architecture:** Rust remains the authority and launches the Windows renderer as a child with private redirected stdio. A per-launch 256-bit secret authenticates bounded, ordered, epoch-bound frames; the renderer receives only host-projected 15A/15B/15C/15D/15E state and returns narrow UI intents. The renderer never receives filesystem, process, provider, or account authority.

**Tech Stack:** Rust 1.91, `cos-core` typed UI models, `cos-store::SessionStore`, C# .NET 8 WinForms/WebView2, GitHub Actions on Windows and Ubuntu.

**Spec:** GitHub issue #115 and accepted 15B/15C/15E/15F contracts on current `main`.

## Global Constraints

- Cloud-only; no local/physical Windows, owner profile, real account, provider credentials, or Send.
- Preserve locks `native-ui-renderer` and `ui-integration`; do not alter other task runtime code without a documented technical dependency.
- Each history page contains at most 24 entries; cap payload/frame bytes and retained renderer page cache.
- Epoch and sequence are replay fences, never authentication; authenticate every frame with HMAC-SHA256 and isolate each renderer/profile with a fresh key.
- Strict JSON, unknown-field rejection, fixed non-sensitive error codes, source-bound artifact/hash, and exact-final-HEAD CI.
- Do not merge or mark Done. PR stays Draft. 15L1/15L2/15L3/PERF-01 remain downstream NOT_RUN.

## Review Focus

- Malformed/duplicate/oversized outer or inner frames are rejected before dispatch.
- Stale epochs, replay, downgrade, cross-profile keys, renderer restarts and asynchronous stale history results fail closed.
- Session/page IDs are host-validated; renderer input never becomes a path, URL, command, or capability.
- WebView2 navigation failures retain a nonzero exit and expose sanitized event correlation/WebErrorStatus.
- Native screenshot output is compared only against independently sourced, provenance-documented baselines; candidate output is never self-promoted to a golden.

---

### Task 1: Sync and establish the test contract

**Files:** `docs/superpowers/plans/2026-10-27-15host-integration.md`, `crates/cos-core/tests/ui_renderer_ipc.rs`.

- [x] Sync the branch with the verified current `main` commit `16037935afcb45950f4a085c4da4a914ffbd5517` without replacing the four isolated 15HOST files.
- [x] Add failing tests for a stable Rust/C# HMAC vector, malformed/oversized frames, stale epoch, replay/downgrade, profile-key isolation, and rebind lifecycle.

### Task 2: Implement authenticated frame protocol

**Files:** `crates/cos-core/src/ui_renderer_ipc.rs`, `crates/cos-core/src/lib.rs`, `crates/cos-core/tests/ui_renderer_ipc.rs`.

- [ ] Implement strict bounded versioned frames with a domain-separated HMAC-SHA256, constant-time MAC verification, independent inbound/outbound sequence counters, epoch checks, and key rotation.
- [ ] Run the targeted test red/green cycle, then Windows/Ubuntu workspace tests.

### Task 3: Connect Rust host and native renderer

**Files:** `crates/cos-app/src/main.rs`, `crates/cos-app/src/renderer.rs`, `crates/cos-app/Cargo.toml`, `apps/windows-renderer/Program.cs`.

- [ ] Launch only an explicitly configured/source-verified renderer child with private stdio and fresh per-process key/epoch.
- [ ] Render host-published navigation, 24-item conversation pages, rich-panel snapshots, focus/localization and safe preferences.
- [ ] Convert renderer gestures into strict typed 15A/15B/15C/15E intents; host validates actual session ownership before loading history, and every asynchronous completion must match its ticket/epoch.
- [ ] Never expose arbitrary commands, filesystem paths, URLs, provider access, owner profile, or Send.

### Task 4: Diagnose navigation and complete visual surfaces

**Files:** `apps/windows-renderer/Program.cs`, `apps/windows-renderer/README.md`, screenshot fixtures and regression tests.

- [ ] Correlate `NavigationStarting` and `NavigationCompleted` by NavigationId; record sanitized `WebErrorStatus`, source category and phase, and fail on unsuccessful navigation.
- [ ] Implement real editor, terminal-output-only, PDF page display and safe math rendering; integrate the 13 exact 15E locale tags and focus/accessibility behavior.
- [ ] Inspect the accepted 15F tree for independent native goldens. If no accepted baseline exists, document the missing provenance and do not claim visual PASS or create a golden from candidate output.

### Task 5: Exact-head validation and handoff

**Files:** `.github/workflows/15host-renderer.yml`, issue #115, Notion 15HOST page.

- [ ] Add hostile IPC, profile isolation, lifecycle/race and source-bound artifact/hash checks.
- [ ] Re-run all relevant Windows/Ubuntu CI on the exact final SHA; no skipped/failed/cancelled checks may be counted as PASS.
- [ ] Update GitHub and Notion with final SHA, workflow/run IDs, artifact SHA-256, implemented scope, independent-golden provenance or blocker, and residual risks.
- [ ] Keep PR Draft/unmerged and hand off for independent ChatGPT security review.
