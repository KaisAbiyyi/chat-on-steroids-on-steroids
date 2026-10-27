# 7U1-PREP — source-attested, fail-closed owner test preparation

## Result and exact blocker

The cloud deliverable here is a preflight security contract and Windows D:
entrypoint, **NOT** a signed-in 7U1 product browser or owner qualification.
Source-attested physical 7L2 PASS proves the synthetic WebView2/MV3/Rust
bridge, not a provider account. Existing accepted 8L1 WindowsHost binds to
https://synthetic.cos.test (issue #71 CLOSED/PASS on 2026-10-26);
cos-app remains a CLI. The separate real-provider 8P-HOST issue #123 /
draft PR #124 is not merged or independently accepted. Its navigation /
native two-phase handoff contract does NOT yet prove 7U1's read-only
signed-in snapshot/attach/detach contract.

Until a reviewed production owner-only observe-only integration ships, the
7U1 product gate reports BLOCKED. The missing
probes/7u1/product-integration.json is intentional, not broken CI.

## Cloud coverage

- SHA-pinned clean repository HEAD and SHA256 full-artifact manifest (no loose
  binaries/dependencies, case-colliding/reserved names, JSON duplicate keys, or
  symlink/junction entries).
- GitHub Actions builds/publishes a downloadable **synthetic-only** native Windows
  artifact. `package_artifact.py` writes its complete `source.sha` and
  `hashes.json`; its `artifact-kind.json` makes it impossible to mistake
  that staged bundle for provider observation readiness. A Windows cloud job
  intentionally checks the 7U1 preflight exit is BLOCKED, not a false PASS.
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

The separate production integration owner (#123, PR #124) must land a
reviewed provider-origin/host foundation. A subsequent 7U1-PREP integration
must implement a real product entrypoint
that privately connects the built Rust bridge and WebView2/MV3 companion,
with strict origin/profile/tab/frame/document/navigation/worker/host
generation and MAC/sequence authority. Need observe-only bounded DOM
snapshot, lease release on navigation/close/crash/restart/worker retirement,
cross-profile rejection, no arbitrary CDP, JS, Send, click, submit, cookie
or credential capture, and a reproducible exact-SHA binary manifest.

The synthetic 8L1 physical gate is already PASS, but is **not** real-provider
acceptance. Once 8P production foundation is accepted and a real restricted
observation harness passes security review and hosted cloud tests, only then
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

## Cloud artifact reproducibility (no real provider)

Run the exact-head Windows/Ubuntu `7U1-PREP fail-closed security` checks.
The Windows job publishes the `7u1-prep-synthetic-native-only` Actions
artifact (7-day retention) containing a .NET WindowsHost publish tree,
Rust `7l2_bridge.exe`, `source.sha`, `hashes.json`, and
`artifact-kind.json`. This is a fully hashed **synthetic** build, not the
product or an owner 7U1 installer. All checks and receipts MUST preserve:
`product_native_adapter=BLOCKED`, `owner7U1Status=NOT_RUN`,
`outgoingSendCount=null`. CI uses temp isolated synthetic directories.
The Windows D: PowerShell wrapper above is informational owner preflight
only and intentionally cannot run 7U1 until the separate real product
integration exists. Do not ask the owner to sign in on this artifact.

Cloud generation 2: isolated to `probes/7u1/**` and the 7U1 workflow;
do not modify source owned by active 8P-HOST PR #124.
