# CoSoS 15HOST — authenticated native renderer

Status: **IN PROGRESS / NOT PRODUCT ACCEPTED**. Tracked by [GitHub #115](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/115). The branch now contains a Rust-launched WebView2 child and a bounded, authenticated presentation bridge; this is still a draft acceptance candidate, not a merge-ready product claim.

## Implemented cloud-evaluable scope

- Rust requires an absolute renderer executable plus its expected SHA-256, verifies the source-bound binary hash, then launches the child with private redirected stdin/stdout and a fresh per-process 256-bit key and epoch.
- Rust/C# share a versioned JSON-line HMAC-SHA256 protocol with a cross-language fixed vector, constant-time MAC checks, strict fields, 512 KiB frame / 256 KiB payload bounds, monotonic independent directions, stale epoch/replay/downgrade rejection, per-process key isolation and key rotation.
- Rust remains the authority for session selection and history. It validates IDs against the active explicitly opened profile, retains at most 6,000 sanitized entries, serves pages of at most 24, and sends only the current typed 15B/15C/15E/15F snapshots. The renderer has no filesystem, process, provider, account, or Send capability.
- Native UI includes navigation, session history virtualization, an in-memory editor, output-only terminal surface, host-owned opaque PDF preview controls, inert math-source preview, locale selector for the 13 accepted tags, and keyboard/focus/dialog wiring. PDF bytes and a real math typesetter are not attached by this UI-only task; those remain explicit residual scope.
- `NavigationStarting` and `NavigationCompleted` are correlated by `NavigationId`. Failures retain a nonzero process exit and write a sanitized receipt with `WebErrorStatus`, navigation IDs, phase, and recent URI classifications. No exit code is swallowed.
- `--synthetic-native-capture` captures actual WebView2 pixels; `--golden-native-capture` captures a separately authored static reference fixture. CI compares decoded RGBA pixel SHA-256 values and records the independent reference source hash. The reference is **not yet an accepted product golden** until independent review.

## Safety / boundedness

- Synthetic capture uses a fresh temporary WebView2 user-data folder; the authenticated product child does the same. No owner profile, real account, provider login, real credentials, tool execution, or message sending is used by cloud tests.
- Renderer intents are allowlisted and size-bounded in C# and then parsed again into strict Rust typed contracts. All user-visible content uses `textContent`; external navigation, new windows, permissions, network resources and arbitrary file paths are denied.
- Session-switch code clears the old cached text before reading a new session; history cache entries and per-process IPC keys are zeroized on drop where supported.

## Acceptance gates still open

1. Independent ChatGPT security review of the exact final HEAD, including Rust/C#/WebView2 integration and source-bound artifact evidence.
2. The native screenshot reference needs independent approval/provenance review; CI equality against the checked-in reference source is a regression signal, not proof of product visual parity.
3. Full PDF asset resolution/rendering and actual math typesetting are not implemented here; the UI exposes safe typed preview surfaces only.
4. Physical 15L1/15L2/15L3 and PERF-01 remain downstream **NOT_RUN**. No local/physical Windows or owner gate is part of this cloud-only task.

Keep PR #127 Draft and unmerged. Do not mark issue #115 Done until independent review and all applicable cloud checks pass.
