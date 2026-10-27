# CoSoS 15HOST — authenticated native renderer

Status: **IN PROGRESS / NOT PRODUCT ACCEPTED**. Tracked by [GitHub #115](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/115). The branch now contains a Rust-launched WebView2 child and a bounded, authenticated presentation bridge; this is still a draft acceptance candidate, not a merge-ready product claim.

## Implemented cloud-evaluable scope

- Rust requires an absolute renderer executable, separately supplied executable SHA-256 and independently trusted renderer-manifest SHA-256. It verifies every app-owned EXE, DLL, runtime sidecar and resource against the bounded manifest, rejects unexpected files, and retains non-write/non-delete-sharing Windows handles across the entire child lifetime. The host then launches the child with private redirected stdin/stdout and a fresh per-process 256-bit key and epoch. The caller-provided hash is **not by itself proof of a trusted release signer or source provenance**.
- Rust/C# share a versioned JSON-line HMAC-SHA256 protocol with a cross-language fixed vector, constant-time MAC checks, strict fields, 512 KiB frame / 256 KiB payload bounds, monotonic independent directions, stale epoch/replay/downgrade rejection, per-process key isolation and key rotation.
- Rust remains the authority for session selection and history. It validates IDs against the active explicitly opened profile, retains at most 6,000 sanitized entries, serves pages of at most 24, and sends only the current typed 15B/15C/15E/15F snapshots. The renderer has no authorized arbitrary file/process execution, provider, account, or Send capability. Its own static packaged HTML and bounded synthetic screenshot outputs are the only presentation fixture exceptions; non-owned navigation and resources are prohibited.
- Native UI includes navigation, session history virtualization, an in-memory editor, **host-authorized recorded terminal output only** (not an interactive shell), and **bounded PDF page display from host-tagged pre-rasterized PNG assets** (not PDF parsing/rasterization). Math uses a constrained token-validated MathML DOM renderer, not full LaTeX/TeX; the source editor remains separate. The 13 accepted locales and keyboard/focus/dialog paths are exercised by hosted native DOM probes.
- `NavigationStarting` and `NavigationCompleted` are correlated by `NavigationId`. Failures retain a nonzero process exit and write a sanitized receipt with `WebErrorStatus`, navigation IDs, phase, and recent URI classifications. No exit code is swallowed.
- `--synthetic-native-capture` captures actual WebView2 pixels; `--golden-native-capture` captures a separately authored static reference fixture. CI compares decoded RGBA pixel SHA-256 values and records the independent reference source hash. The reference is **not yet an accepted product golden** until independent review.

## Safety / boundedness

- Synthetic capture uses a fresh temporary WebView2 user-data folder; the authenticated product child does the same. No owner profile, real account, provider login, real credentials, tool execution, or message sending is used by cloud tests.
- Renderer intents are allowlisted and size-bounded in C# and then parsed again into strict Rust typed contracts. All user-visible content uses `textContent`; external navigation, new windows, permissions, network resources and arbitrary file paths are denied.
- Session switches retire session-bound rich panels, clear history and authorized raster caches before displaying the next session, and zeroize old sensitive buffers/IPC keys where supported. Hosted A/B tests use two distinct synthetic profile roots, deny foreign-session IDs and stale revisions, and inspect the actual editor, terminal, PDF and history WebView2 DOM for old-session sentinel leakage.

## Acceptance gates still open

1. Independent ChatGPT security review of the exact final HEAD, including Rust/C#/WebView2 integration and source-bound artifact evidence.
2. The native screenshot reference needs independent approval/provenance review; CI equality against the checked-in reference source is a regression signal, not proof of product visual parity.
3. Hosted terminal is recorded output-only; PDF consumes host-supplied pre-rasterized PNG pages, never untrusted PDF bytes; constrained MathML is rendered from a safe token subset, not full mathematical typesetting. These intentional scope limits must remain explicit in acceptance and cannot be described as native PDF parsing or a process-backed terminal.
4. Physical 15L1/15L2/15L3 and PERF-01 remain downstream **NOT_RUN**. No local/physical Windows or owner gate is part of this cloud-only task.

Keep PR #127 Draft and unmerged. Do not mark issue #115 Done until independent review and all applicable cloud checks pass.

## Trusted release issuer gate (15HOST final cloud follow-up)

The signed artifact subject is `renderer-integrity.json` (including `sourceSha`, SHA-256 for **every** app-owned file). In Actions, `actions/attest@v4` generates an OIDC/Sigstore SLSA provenance and `gh attestation verify` checks certificate-backed repository/workflow/ref/source identity with forged-issuer, substituted-artifact and wrong-source negative probes. A PR attestation is **candidate provenance only**, not an authorized release.

Non-test Rust launches refuse to start until the manifest also has an attestation verified by the OS-protected `C:\\Program Files\\GitHub CLI\\gh.exe` executable, with the pinned signer `KaisAbiyyi/chat-on-steroids-on-steroids/.github/workflows/15host-renderer.yml`, `refs/heads/main`, and the exact `--renderer-source-sha` 40-hex commit that must match the manifest sourceSHA. The prior `--renderer-sha256` and `--renderer-manifest-sha256` inputs remain required for file integrity but are **not sufficient** for authenticity. A missing trusted CLI, unavailable attestation service, source mismatch, issuer mismatch, or verification timeout rejects launch. The `gh.exe` install must be administrator-protected; writable/verifier replacement is outside the asserted threat boundary and must be explicitly prohibited by the distributor.

**Release requirement:** the authorized main-ref artifact must be built/attested from the exact approved source commit using the dedicated workflow (workflow_dispatch or main push after merge). The cloud PR run can test OIDC trust and forged-attestation denials but is NOT itself an authorized main-branch release. If GitHub attestation is unavailable, do not substitute a local checksum or skip provenance: release remains blocked until an authorized signing provider and pinned verifier are available.

## Product-state golden review pack

The workflow packages seven native WebView2 screenshot candidates: editor, terminal and PDF for independent profiles A/B, plus math. `15host-review-index.json` includes individual PNG SHA-256, receipt SHA-256, capture environment, exact source HEAD, 15F reference and explicit per-surface criteria. `python tools/host15_review_package.py --input DIR --source-sha SHA --output DIR/15host-review-index.json` verifies candidate bytes and rejects mutated receipt/source/PNG or spoofed approval flags. `--compare-dir INDEPENDENT_BASELINES` permits deterministic exact byte comparison but **never implies golden acceptance**.

The independent reviewer must supply separately approved PRODUCT-state baselines and an attributable approval/provenance record; no candidate may be promoted by the implementation author. The release/build attestation does not approve pixels. Until independent visual approval exists: `approvedGolden=false`, `productAccepted=false`; PR stays Draft/NOT MERGE SAFE. Physical 15L1/15L2/15L3/PERF-01 remain downstream NOT_RUN.
