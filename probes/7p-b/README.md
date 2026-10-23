# 7P-B — Cloud synthetic embedded browser compatibility probe

This is a real executable Windows WebView2 WinForms adapter spike: a **disposable C# API capability adapter** to evaluate WebView2 before committing to a native Rust Windows host. It is NOT a CoSoS production GUI, is NOT signed into a provider, and does NOT complete native 7P-L1.

Prerequisite: accepted 7P-A PR #40, provisional WebView2 + Rust host + JS/TS renderer/MV3 proposal. All new work is isolated in probes/7p-b, with no production bridge changes.

## Cloud CI (Windows and Ubuntu)

1. Run dotnet run --project probes/7p-b/PolicyTests -c Release (synthetic tab/profile/epoch authorizations).
2. Run node probes/7p-b/tools/check_contract.mjs (manifest/origin/extensions, renderer packages, fail-closed architecture ledger).
3. Run npm install --prefix probes/7p-b/renderer ; npm run build --prefix probes/7p-b/renderer (bundled actual CodeMirror, KaTeX, pdf.js, xterm.js).
4. Windows: dotnet build probes/7p-b/WindowsHost/WindowsHost.csproj -c Release (WebView2 API binding compile).

**Do not report a native browser PASS from these commands.** A Windows Actions runner may have no interactive desktop/WebView2 runtime even though it can compile the host.

## Separate isolated physical Windows gate 7P-L1

The optional `--batch --native-auth` flag runs a **synthetic consent and callback**
flow in actual WebView2 on Windows. Build the renderer first: Vite copies the
`renderer/public/oauth-*` fixture files to `renderer/dist`. The probe maps
`login.synthetic.cos.test` and `synthetic.cos.test` to that local directory,
disables OS account single sign-on, and denies unowned popup windows. It tests
an approve/deny choice, host-validated state and callback URL, one-time use,
forged callback/origin rejection and authorization separation across A/B profiles.
The receipt's `sign_in_constraints=PASS` proves **only those synthetic checks**.
No authentic provider login, third-party cookies, PKCE/token exchange, actual
provider OAuth restrictions, consent to a real account or product implementation
is exercised. Keep those at 7U1/8U1 and related feature gates.

For an already built host, run `WindowsHost.exe --batch --native-auth --out
D:\\projects\\CoS-Rust\\.test-tmp\\7p-l1\\<new-test-folder> --assets
<absolute-renderer-dist> --extension <absolute-synthetic-extension>` with
`TEMP`, `TMP`, `.NET` cache/home and NuGet output pointing to D:.

On an authorized isolated Windows desktop, after building renderer and host, run with explicit paths to renderer/dist and the unpacked synthetic extension. Example:

    dotnet run --project probes/7p-b/WindowsHost -- --out D:\CoSoSProbe7PB --assets D:\...\probes\7p-b\renderer\dist --extension D:\...\probes\7p-b\extension

Inspect both synthetic panes, all receipt entries and actual WebView2 runtime version. Verify IME/selection/undo/accessibility, PDF text selection/search, KaTeX copy, xterm ANSI/resize, CDP target identity, worker sleep/restart and chromium extension debugger attach/detach on owned synthetic targets.

The native Windows host also exposes an explicit `--batch --native-idle` mode for
the gate. It checks that disabling/re-enabling the unpacked extension retires its
ephemeral worker instance, verifies debugger reattachment, runs actual WebView2
DOM interactions for PDF search / terminal resize / KaTeX text, and then leaves
the extension idle for 65 seconds before reloading the same synthetic page.
An independently generated worker instance must be observed to mark natural MV3
idle suspension PASS; unchanged instances are UNVERIFIED. This test exercises
the *actual* embedded WebView2 runtime and must be run on the physical Windows
host. Use a new D: `--out` directory for each run. The debugger methods remain
distinct from host CDP and are restricted to `synthetic.cos.test`.

The extra checks are synthetic security/compatibility evidence, not OS keyboard,
IME composition, native accessibility/clipboard, provider sign-in or production
installer acceptance. They cannot change `native7PL1Accepted` to true; those
additional gate decisions must be recorded in a separate, source-bound native
qualification receipt.

**Host CDP is not equivalent to chrome.debugger extension semantics.** Extension debugger API presence gives OBSERVED_ONLY evidence. Attach/detach/reattach and worker event ordering must be tested separately at 7P-L1. Any FAIL, NOT_RUN or UNVERIFIED hard blocker keeps 7P-L1 blocked or failed. Never assume an extension compatibility API by name.

Record size, cold/warm startup and RAM on D:, tied to exact pinned SHA. No signed-in real accounts, browser profile copies, auto Send or personal provider access without a separate U gate. If any hard browser invariant fails, stop Phase 7A and reassess an embedded CEF or another proven adapter, not an unapproved external-browser shortcut.

Receipts are confined to the isolated output root; D: is required when present. The JSON always has native7PL1Accepted false. Synthetic navigation is allowlisted to synthetic.cos.test virtual host; OS single sign-on is disabled. Windows cloud compilation does not certify native Windows, macOS/Linux parity, provider OAuth or installer budgets.

Known non-proofs from cloud alone: real Evergreen WebView2 installation, MV3 worker lifecycle, chrome.debugger attach parity, extension under provider CSP, IME/screen-reader UX, real sign-in and installer distribution size. The separate native source-bound receipt decides which of these have actually been observed; do not infer a PASS from compilation.
