# 7P-B — Cloud synthetic embedded browser compatibility probe

This is a real executable Windows WebView2 WinForms adapter spike: a **disposable C# API capability adapter** to evaluate WebView2 before committing to a native Rust Windows host. It is NOT a CoSoS production GUI, is NOT signed into a provider, and does NOT complete native 7P-L1.

Prerequisite: accepted 7P-A PR #40, provisional WebView2 + Rust host + JS/TS renderer/MV3 proposal. All new work is isolated in probes/7p-b, with no production bridge changes.

## Cloud CI (Windows and Ubuntu)

1. Run dotnet run --project probes/7p-b/PolicyTests -c Release (synthetic tab/profile/epoch authorizations).
2. Run node probes/7p-b/tools/check_contract.mjs (manifest/origin/extensions, renderer packages, fail-closed architecture ledger).
3. Run npm install --prefix probes/7p-b/renderer ; npm run build --prefix probes/7p-b/renderer (bundled actual CodeMirror, KaTeX, pdf.js, xterm.js).
4. Windows: dotnet build probes/7p-b/WindowsHost/WindowsHost.csproj -c Release (WebView2 API binding compile).

**Do not report a native browser PASS from these commands.** A Windows Actions runner may have no interactive desktop/WebView2 runtime even though it can compile the host.

## Later isolated physical Windows gate 7P-L1 (not run in this cloud task)

On an authorized isolated Windows desktop, after building renderer and host, run with explicit paths to renderer/dist and the unpacked synthetic extension. Example:

    dotnet run --project probes/7p-b/WindowsHost -- --out D:\CoSoSProbe7PB --assets D:\...\probes\7p-b\renderer\dist --extension D:\...\probes\7p-b\extension

Inspect both synthetic panes, all receipt entries and actual WebView2 runtime version. Verify IME/selection/undo/accessibility, PDF text selection/search, KaTeX copy, xterm ANSI/resize, CDP target identity, worker sleep/restart and chromium extension debugger attach/detach on owned synthetic targets.

**Host CDP is not equivalent to chrome.debugger extension semantics.** Extension debugger API presence gives OBSERVED_ONLY evidence. Attach/detach/reattach and worker event ordering must be tested separately at 7P-L1. Any FAIL, NOT_RUN or UNVERIFIED hard blocker keeps 7P-L1 blocked or failed. Never assume an extension compatibility API by name.

Record size, cold/warm startup and RAM on D:, tied to exact pinned SHA. No signed-in real accounts, browser profile copies, auto Send or personal provider access without a separate U gate. If any hard browser invariant fails, stop Phase 7A and reassess an embedded CEF or another proven adapter, not an unapproved external-browser shortcut.

Receipts are confined to the isolated output root; D: is required when present. The JSON always has native7PL1Accepted false. Synthetic navigation is allowlisted to synthetic.cos.test virtual host; OS single sign-on is disabled. Windows cloud compilation does not certify native Windows, macOS/Linux parity, provider OAuth or installer budgets.

Known non-proofs: cloud runner GUI, real Evergreen WebView2 installation, MV3 worker restart, chrome.debugger attach parity, extension under real provider CSP, IME/screen-reader UX, real sign-in, installer distribution size. These remain explicit future gates.
