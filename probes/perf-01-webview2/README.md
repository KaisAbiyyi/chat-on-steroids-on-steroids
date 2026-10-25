# PERF-01: Physical Windows WebView2 6,000-message probe

This is a **synthetic disposable native WebView2 benchmark**, not a runnable CoSoS product UI. It demonstrates what a WinForms + WebView2 host does when a pre-generated 6,000-message JSON array is displayed as a lazy ~20-row DOM and scrolled between 60 positions. It is source code that can be built by hosted CI; real GUI/RAM/scroll results require a physical Windows interactive session.

On the approved D: Windows environment, from PowerShell in any clean D:-project checkout containing this directory:

```powershell
& '.\probes\perf-01-webview2\run-native.ps1'
```

The runner generates a disposable **public-only** 3,000-turn/6,000-message fixture, verifies its SHA, builds the host, then runs two new real WebView2 windows sequentially and stores receipts and profiles under the git-ignored D: `.test-tmp/perf01-webview2`. The native host admits **only the pinned generator fixture hash**, so unrelated personal chats cannot be passed to it by simply supplying a different JSON path. It refuses missing native output and never marks the physical result PASS merely because a .NET build succeeded. Actual native browser sessions open temporarily and close after each run. It does not sign in, read real conversations, use extensions, or request browser permissions. The host only accepts local virtual-host pages, denies other navigation and popups, and rejects non-D: fixture/output roots.

The source includes a fixture generator and a deterministic correctness test. Hosted tests: `node --test probes/perf-01-webview2/create-fixture.test.mjs`, `dotnet build probes/perf-01-webview2/NativeBrowserProbe.csproj --configuration Release` on Windows. CI compilation is **not native WebView2 acceptance**. Any output requires exact executable, managed assembly, benchmark source, page, fixture and source-ref hashes for usable reproducibility; `SourceSha` in the runtime receipt is a supplied checkout identity, not a signature.

Historical local physical Windows results (separate untracked harness, before this tracked adaptation) on 2026-10-25: two same-assembly PASS runs at 6,000 messages/20 visible rows/60 programmatic scroll positions, startup-to-content 1,166.81–1,285.14 ms, in-page double-requestAnimationFrame scroll p95 14.1 ms, sampled host+browser Working Set high 421.01–429.09 MiB. Host load, physical monitor refresh and runtime caches affect the figures. This is neither full desktop UX latency nor a matched RAM savings comparison with original CoS.

**Blockers remain:** 8L1 native privileged profile/lifecycle/permission authority and 15L3 physical large-history product UI. Do not claim PERF-01 Done, product performance savings, real account/browser parity or acceptance from this harness. See GitHub issues #71 and #96 and their dated native receipts.
