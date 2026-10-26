# 8P-HOST — Native contract divergence, owner Windows evidence, security approval

**Status: P0 / CHANGES REQUIRED; PR #124 must stay Draft.** This is a
review-and-acceptance dossier, **not** proof of parity, physical qualification,
provider sign-in, or independent security approval. The exact PR HEAD and
source-bound Windows artifact must be recorded by the reviewer at execution.

## 1. Invariants: JS 8B/8C versus native C#/Rust 8P

| Contract | Accepted JS source | Production 8P native implementation | Acceptance |
| --- | --- | --- | --- |
| Distinct host-proven partitions | `extension/adapter/profile-session.mjs:73-87,135-153` requires `partitionId`, `isolationGeneration`, unique profile owner | `Native8P.cs:537-572,700+` compares distinct WebView2 `Profile.ProfilePath` and host-owned profile IDs; native `AssertOwnedAsync` additionally attests WebView2 source and installed extension target | **NOT EQUIVALENT** without independent confirmation of immutable partition generation and sibling/host lifecycle |
| Logical handoff only | `profile-session.mjs:11,135-175`: after commit `documentId`, navigation/host/worker generations, `channelId`, origin and native partition ID **must remain identical**; only app `sessionId/sessionEpoch` change | `Native8P.cs:572-650`: two CDP-signed first-proofs, retire BOTH Rust channels, Rust atomic `handoff_sessions`, update both native session epochs; then `Native8P.cs:875-915` navigates to real new WebView2 documents and requires fresh native/Rust/MV3 proofs | **DELIBERATE P0 DIVERGENCE** — do not claim literal JS 8B parity, mock JS passing is not acceptance |
| Redirect/rebind | `extension/adapter/browser-lifecycle.mjs:122-186`: one-use native event proof with monotonic sequence, preserve logical session for an ordinary rebind | Native WebView2 `NavigationStarting` synchronously revokes; **`NavigationId`+epoch admission fence** prevents foreign/cancelled `NavigationCompleted` rebinding Rust; new `BindAsync` uses current browser profile/target | Needs native owner evidence across redirected, cancelled, stale, worker-destroyed and crash/restart sequences; JS controller not executed in native host |
| OAuth | No consent substitutes for signed-in identity | `Native8P.cs`: exact host-owned origin+callback allowlists and five-minute, digest-only, one-use state correlation; code callback is always cancelled, no token exchange | **PARTIAL / FAIL-CLOSED**, not 7U1 |

**Do not silently weaken JS 8B/8C invariants.** A reviewer must choose
explicitly between:

1. Redesign the Rust/native handoff to commit a session-only attribution while
   preserving the live document/channel identity and prove this is safe against
   residual sibling leases; or
2. Define a separately versioned **two-phase native contract** (handoff +
   retire/rebind) with provable partition and event identity mapping, differential
   tests that exercise the **real compiled native entrypoint** rather than a
   JS mock, and independent reviewer approval of the deviation.

Until one path is implemented and accepted, **P0 unresolved**, even if all
hosted CI checks pass.

## 1A. Versioned cross-language contract vectors (cloud-only, not physical)

The production host now names its *different* physical protocol
`CoSoS/8P/native-two-phase/v1`, implemented by
`GateForm.Verified8PTwoPhaseResult` in `apps/windows-host/Native8P.cs`.
This function is called **both** by the actual product handoff
postcondition and by an isolated, deterministic test command:
`WindowsHost.dll --8p-native-differential-v1`. It does not construct an
untrusted alternate policy shim or silently change the accepted JS controller.

`probes/8p/native-contract-differential.test.mjs` starts that **compiled
product C# executable** and feeds its exact synthetic before/after values to
the original `createProfileSessionController` (JS 8B), not a rewritten JS
contract. Windows CI builds the product under `-warnaserror` and runs this
cross-language differential as a required check. Its positive controls must
prove *both* directions of the deliberate mismatch:

- `native-two-phase-fresh`: C# **accepts**, unchanged JS 8B **rejects**,
  because the new WebView2 document/channel/generation are not stable.
- `js-8b-stable-postcondition`: JS 8B **accepts**, native v1 **rejects**,
  because no retirement/new document/channel proof is established.

The same compiled-source harness covers foreign profile/partition, extension
substitution, stale document/epoch/channel/worker and host generations,
session theft, and epoch replay. The fake native callback in the JS test is
**synthetic** and is never interpreted as an independent security authority.
Native Rust channel handoff, retirement, replay denial and signed first-proof
are tested independently in `native_browser_bridge.rs` and
`7l2_bridge.rs`; this cross-language CI does **not** operate a live
C#/Rust bridge, MV3, or a physical WebView2 instance.

**Acceptance remains P0 OPEN:** the independent reviewer must approve the
semantic split and its phase-boundary/partition identity proof. The owner
must produce actual **C#/Rust/MV3/WebView2** source-bound traces and replay/
restart evidence before physical acceptance. A green differential *proves
and pins the divergence*, not that both contracts are interchangeable.

## 2. Source-verified physical Windows acceptance

The owner must use an isolated, fresh directory tree under
`D:\projects\CoS-Rust\`, **never** an existing browser user profile.
Prerequisites: Git checkout at same SHA as artifact `source.sha`, matching
`hashes.json` checked in `probes/8p/run-native.ps1`, configured provider
allowlist at an owner-controlled path, no personal secrets in artifacts.

Example with placeholders (select a real configured provider ID):
```powershell
pwsh -File .\probes\8p\run-native.ps1 `
  -Repo 'D:\projects\CoS-Rust\repo' `
  -ArtifactRoot 'D:\projects\CoS-Rust\artifacts\8p-final' `
  -Out 'D:\projects\CoS-Rust\evidence\8p-final-handoff' `
  -ProviderConfig 'D:\projects\CoS-Rust\private\provider-origins.json' `
  -ProviderId 'chatgpt' -HandoffProbe
```

The path examples are illustrative, not evidence that files exist. User consent
is a real default-No native dialog. **Do not enter provider credentials or run
7U1 signed-in checks within this 8P handoff probe.**

The generated `8p-navigation-receipt.json` includes `sourceSha`,
`nativeWorkerRebinds`, `mv3WorkerAttested`,
`crossProfileHandoffPassed`, `native8PAccepted=false` and
`owner7U1Accepted=false`. Even when handoff flags are true, this receipt is
**not** comprehensive physical acceptance: it does not attest a real cross-origin
OAuth redirect, native process restart, browser crash, or provider sign-in.
These remain individually **NOT_RUN** until a separate source-SHA-bound,
native event-authenticated receipt exists.

Required independent owner evidence:
- Actual native WebView2 and Rust child *process IDs and source identity* for
  each fresh host launch; same provider config and immutable extension manifest.
- Separate browser-owned profile paths and extension IDs for A and B, plus
  zero overlapping cookie/localStorage transfer; no raw profile data exported.
- Before/after Rust channel and worker generations; valid *new* HMAC-bound
  first-proof after handoff, and replay rejection of old channel/handoff.
- An **actual** allowed-to-allowed HTTPS redirect and a refused redirect,
  with WebView2 `NavigationId`, navigation epoch, final origin, denied default
  permission and stale-completion rejection, *without logging credential-bearing
  URLs, OAuth state, code, tokens, or HMAC keys*.
- Explicit worker termination/reactivation and browser crash/restart with
  old Rust channel denied, restored only under a new source-bound host and
  fresh worker proof; verify profile persistence/isolation separately.
- Negative paths: cancel default-No handoff, foreign profile/partition, session
  theft, sibling lease, navigation during consent, expired/replayed OAuth
  callback, ambiguous Rust commit, unexpected process close.

**No existing cloud-hosted Windows compilation substitutes for this owner-run
physical proof.** The synthetic 8L1 `-VerifyRestart` or `-CrashProbe` receipts
cannot be reused as 8P production provider qualification.

## 3. Independent review sign-off template

The reviewer must be a distinct authenticated security reviewer, **not the PR
implementor/owner-authored review**. The reviewer should inspect the final exact
commit, source hash, `Native8P.cs`, 7L1 `Program.cs`, Rust
`native_browser_bridge.rs` and `7l2_bridge.rs`, zero-permission MV3
`probes/8p/extension/sw.mjs`, `profile-session.mjs`,
`browser-lifecycle.mjs`, strict native runner and physical receipts.

Review decision must explicitly address: (a) which contract option from §1
was accepted and why, (b) partition identity and generation under two processes,
(c) Rust + C# atomicity and uncertain/replayed commits, (d) OAuth callback
canonicalization/state TTL/one-use boundaries, (e) redirect cancellation and
WebView2 event race fencing, (f) worker death/restart, and (g) source provenance.

An ordinary `16F` workflow success is not a human/independent reviewer
approval; a self-authored review is not independent. **The gate stays
`NOT_RUN` until an independent reviewer posts an explicit verdict on the
final HEAD.** Owner 7U1 signed-in acceptance stays **NOT_RUN, separate**.
