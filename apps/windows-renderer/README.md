# CoSoS 15HOST — bounded cloud renderer foundation

Status: **IN PROGRESS / NOT PRODUCT ACCEPTED**. Implementation tracked by
[GitHub #115](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/115).
This stand-alone, native Windows WebView2 executable is an incremental **synthetic-only**
rendering foundation. It is **not** the authenticated Rust-backed CoSoS desktop application.

## Cloud-evaluable scope

- `--contract-selftest` executes the **compiled native C#** JSON protocol validator,
  including disallowed action, oversize, stale epoch, duplicate keys, out-of-bound
  pages, and a 6,000-message deterministic synthetic fixture with at most 24
  messages per delivered page.
- `--synthetic-native` loads host-owned inert HTML via WebView2 `NavigateToString`,
  with read-only virtualized synthetic history and preview-only rich panels.
- `--synthetic-native-capture` opt-in captures actual WebView2 pixels to an
  isolated temporary profile and outputs only sanitized SHA-256/byte counts.
  These pixels are **candidate captures, NOT independent goldens**.
- Every run creates an entirely fresh, random temporary user-data directory;
  does not load an owner's profile, sign in, use any provider, send messages,
  expose file paths, run tools, or grant WebView2 permission. New windows and
  external navigation are disabled; HTML has a no-network CSP and textContent-only
  rendering. All IPC responses contain synthetic fixture data.

## Explicit nonclaims / acceptance blockers

1. A real authenticated Rust host → 15B/15C/15E/15F typed state bridge and
   session/frame ownership checking with bounded IPC and hostile-renderer negatives
   are **NOT IMPLEMENTED**. C# synthetic page guard is NOT a Rust security boundary.
2. Real editor/terminal/PDF/math rendering and interaction, 13-locale/IME,
   focus/a11y interactions, 6,000-message *product-state* injection, source-approved
   independent screenshot goldens and screenshot comparison are **NOT_RUN**.
3. Hosted cloud Windows build/selftests and Ubuntu cross-build do not by themselves
   qualify actual Windows WebView2 execution/screenshots. Native
   `--synthetic-native-capture` is opt-in and must be run in a reproducible cloud
   Windows hosted interactive session or deferred with a concrete reason.
4. Owner physical 15L1/15L2/15L3, PERF-01 matching original CoS and
   credentials/sign-in are separate downstream **NOT_RUN**, not cloud CI proofs.

Never merge and mark 15HOST Done until its full acceptance contract has
source-bound artifact, all final-HEAD CI PASS, security review, and real
Rust↔renderer integration. No fake native PASS or fabricated golden pixels.
