# 7U1-PREP — source-attested, fail-closed owner test preparation

## Result and exact blocker

The cloud deliverable here is a preflight security contract and Windows D:
entrypoint, **NOT** a signed-in 7U1 product browser or owner qualification.
Source-attested physical 7L2 PASS proves the synthetic WebView2/MV3/Rust
bridge, not a provider account. Existing WindowsHost binds to
https://synthetic.cos.test, and cos-app is still a CLI. Native 8L1 issue #71
lacks a product-owned privileged WebView2 profile/controller adapter.

Until 8L1 is fixed, the tool must report BLOCKED. The missing
probes/7u1/product-integration.json is intentional, not a broken CI fixture.

## Cloud coverage

- SHA-pinned repository HEAD and SHA256 full-artifact manifest (no loose
  binaries/dependencies and no symlink/junction entries).
- Exact provider HTTPS origin allowlist: chatgpt.com, claude.ai or gemini.google.com.
- Human opt-in and distinct 7u1-prefixed dedicated test profile.
- Reviewed integration contract for a real product-owned WebView2 host
  at host/CoSoSProductBrowser.dll plus 7u1_product_bridge.exe, not the
  existing synthetic WindowsHost.dll/7l2_bridge.exe.
- Restricted operations: attach, snapshot, detach only, max 65536 capture
  bytes, no persistence, no Send capability, no automatic navigation.
- Sanitized JSON receipt with distinct BLOCKED/PASS/NOT_RUN per check.
  Even successful metadata preflight never sets owner7U1Accepted=true or
  outgoingSendCount=0. Native behavior still needs real evidence.

The self-tests use synthetic binary bytes only to test **metadata rejection**.
They do not verify a Windows executable or its truthfully claimed abilities.

## Future native integration prerequisite (8L1 #71)

The separate native integration owner must implement a real product entrypoint
that privately connects the built Rust bridge and WebView2/MV3 companion,
with strict origin/profile/tab/frame/document/navigation/worker/host
generation and MAC/sequence authority. Need observe-only bounded DOM
snapshot, lease release on navigation/close/crash/restart/worker retirement,
cross-profile rejection, no arbitrary CDP, JS, Send, click, submit, cookie
or credential capture, and a reproducible exact-SHA binary manifest.

Once actual native implementation passes source-diff security review and
hosted tests, the native 8L1 Windows gate needs physical PASS. Only then
create/review a distinct **interactive** 7U1 signed-in owner test runner
with the above provenance. A JSON declaration by itself is NOT security
proof and is never permission to sign in automatically.

## Owner preflight example — only after native adapter exists

PowerShell:

    .\probes\7u1\run-preflight.ps1 -Repo 'D:\projects\CoS-Rust' -ArtifactRoot 'D:\projects\CoS-Rust\artifacts\7u1-product' -Out 'D:\projects\CoS-Rust\.test-tmp\7u1\preflight01' -Provider chatgpt -ProfileId '7u1-freshprofile01' -OwnerConsent

Requires an empty/new Out folder on D:. Currently this intentionally fails
BLOCKED. No account login, provider navigation, browser launch or Send occurs.

## Human 7U1 gate (issue #91) — NOT_RUN

Owner separately opts in, manually signs in with a distinct test profile,
opens only a deliberately safe conversation, verifies bounded read-only
observation and signed authority, navigates/closes/restarts, confirms stale
handles refused and zero outgoing Send. Evidence is sanitized per-check
PASS/FAIL/NOT_RUN without private chat, credentials, cookies, 2FA or tokens.
Synthetic and hosted tests cannot upgrade this to PASS.
