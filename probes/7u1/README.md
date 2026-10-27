# 7U1-PREP — source-attested observe-only owner-runner preparation

## Current result and production audit

As of 2026-10-27, main includes the 8P-HOST merge at 16037935afcb45950f4a085c4da4a914ffbd5517. The production host source is now available in apps/windows-host/Native8P.cs and apps/windows-host/WindowsHost.csproj. The source audit confirms that this is a real-provider navigation and native Rust/MV3 handshake path, but it does not expose a safe read-only attach/snapshot/detach API.

The production 8P path remains handshake-only for this purpose. It does not invoke the synthetic host's ObserveAsync, ledger, or CDP tool capture; the production MV3 worker is transport-only. An allowed HTTPS origin and a successful Rust/MV3 handshake are not permission to read page content or evidence of a signed-in session.

Therefore this PR does not claim a real 7U1 runner. It must remain Draft and the 7U1-PREP issue must remain open until the product host provides a reviewed read-only API. No 8P runtime file is modified by this PR. The owner gate issue #91 remains BLOCKED / NOT_RUN.

## Exact missing contract

The production host, Rust bridge and MV3 worker need a shared, source-bound observe-only contract that provides only:

- Attach a one-use lease to the exact configured HTTPS provider origin and the host-attested profile/partition, tab, document, navigation, worker and host generations.
- Return a bounded snapshot of at most 65536 bytes with no persistence and no arbitrary script/CDP access.
- Detach and revoke the lease on navigation, tab close, renderer crash, host restart/rebind, worker retirement, or explicit detach.
- Reject stale handles, stale snapshots, replay, source/binary mismatch, provider/origin mismatch, partition reuse across profiles and generation rollback.
- Expose no outgoing Send, click, submit, cookie, credential or token capability.
- Bind the native host, Rust bridge, MV3 worker and every loadable binary to an immutable source SHA and complete SHA256 manifest.

The static source audit looks for the required host/Rust/MV3 API surface. At this HEAD the required methods are absent, so the production audit reports BLOCKED. A declaration file or product-looking filename is not evidence of integration.

## Cloud-only coverage

- The existing preflight and full SHA256 manifest checks remain in place; exact-head Windows and Ubuntu CI must run again after every change.
- The synthetic reference adapter in observe_only_contract.py exercises the contract's validation and revocation rules entirely in memory. It does not connect to WebView2, Rust or MV3, access a provider, read an account, simulate human consent, or persist snapshot data.
- Regression vectors cover source SHA and binary-manifest mismatch, exact provider origin, profile/partition isolation, stale document/worker generations, replay, bounded snapshots, navigation/close/crash/rebind/worker retirement, detach revocation, and the absence of Send capability.
- The Windows production-host audit job builds the actual apps/windows-host project and Rust bridge, copies the actual MV3 source, and publishes a SHA256-manifested production 8P handshake-only artifact. Its kind marker explicitly says it is not a 7U1 runner; preflight must keep product_native_adapter BLOCKED.
- A separate synthetic 7L2 artifact remains labelled synthetic-only. It must never be renamed or treated as the product runner.

Both artifacts are source-bound diagnostic evidence only. The production handshake-only artifact is not safe for owner sign-in and is not a substitute for the missing read-only API.

## Owner preflight and human gate

The PowerShell preflight is metadata-only and intentionally remains BLOCKED until the production observation adapter exists. CI never passes the owner-consent flag. No account login, provider navigation, credentials, cookies, tokens, or Send is performed by this cloud work.

Issue #91 remains BLOCKED / NOT_RUN until the owner independently runs the signed-in observe-only test with explicit consent, after the real read-only adapter is implemented and independently reviewed. CI success can never be labelled owner acceptance.
