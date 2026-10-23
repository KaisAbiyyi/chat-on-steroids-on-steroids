# 7L1 — WebView2 / Rust 7B / MV3 7C integration

Issue #49: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/49

This is an executable, isolated Windows integration gate for the 7B Rust
authority and the 7C MV3 companion. It uses synthetic profiles and URLs, and
does not perform provider sign-in, Send, native product installation or UI input.

The WinForms host owns the actual WebView2 controllers and browser profiles,
current navigation and document epochs, installed extension IDs, page targets,
and exact extension service-worker targets. It attaches via
`Target.attachToTarget` with `flatten=true` and calls
`CallDevToolsProtocolMethodForSessionAsync` only after proving a nonempty
extension-worker CDP session. It refuses missing, foreign or stale sessions and
checks controller, profile, page target and worker target again after async
calls. Navigation/disposal invalidates the authority.

The native process is a private Rust child using JSON lines over stdin/stdout,
not a page message or HTTP server. It invokes the actual
`NativeBrowserBridge` register, handshake, observe and companion-event
authorization methods. A one-use per-channel key is passed directly to the
extension-ID-verified worker target and never placed in page or persistent
storage. The MV3 module imports 7C statically, exposes no content script
listener, and checks an exact-event HMAC and Rust authorization receipt before
claim/ACK/repair. Distinct Rust and 7C sequence counters are explicitly mapped.

## Cloud validation

Run Node security tests on the worker, existing 7C companion tests, Rust
native-bridge tests and strict Clippy, and build the WebView2 Windows host.
The GitHub workflow `7l1-native-bridge.yml` builds the real Windows Rust
helper and WebView2 host and uploads an artifact containing `source.sha`,
`hashes.json`, and both executables. Windows/Linux Rust CI and task-DAG
validation remain required.

## Exact-SHA D: Windows gate

Use the artifact from the *same source SHA* as the checked-out code. Verify
`source.sha` and the file SHA256 manifest against the extracted artifact.
Choose a fresh output directory on D: inside the CoS-Rust workspace.

```powershell
dotnet D:\path\to\gate\host\WindowsHost.dll --repo D:\projects\CoS-Rust\.test-tmp\issue49-integration --bridge D:\path\to\gate\7l1_bridge.exe --artifact-root D:\path\to\gate --out D:\projects\CoS-Rust\.test-tmp\issue49-native-run01 --native-idle
```

Inspect the process exit and the full `receipt.json` (including failures,
NOT_RUN checks, hashes and `native7L1Accepted`). The `--cdp-preflight`
mode proves only live WebView2 extension worker discovery; its receipt
explicitly reports the actual integrated native gate as NOT_RUN.
The WindowsHost executable also independently checks the exact HEAD SHA,
clean native/extension source files, pinned Rust/host binary locations and
every SHA256 manifest entry before any integrated 7L1 check may PASS.

`--native-idle` waits 65 seconds with the extension **enabled**, so actual
worker retirement/recreation may occur while `chrome.storage.session`
preserves a pending claim. The worker must refuse stale-generation ACK and
repair without duplicate Send. Disabling, reloading or updating the extension
clears session storage, so that is not an accepted positive recovery fixture.

This scoped gate only proves the synthetic native interaction. It does not
qualify provider login, full product packaging, signed distribution, installer
rollback, broader production origins or final acceptance of later phases.
