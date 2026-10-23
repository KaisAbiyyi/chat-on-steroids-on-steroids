using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text.Json;
using CoSoS.Probe7PB;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace CoSoS.Probe7PB.WindowsHost;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        Application.Run(new ProbeForm(args));
    }
}

/// <summary>
/// Disposable, synthetic-only Windows capability probe. Not the production Rust host.
/// GUI execution on physical Windows is the independent 7P-L1 acceptance gate.
/// </summary>
internal sealed class ProbeForm : Form
{
    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")]
    private static extern IntPtr GetForegroundWindow();
    private readonly WebView2 _a = new() { Dock = DockStyle.Fill };
    private readonly WebView2 _b = new() { Dock = DockStyle.Fill };
    private readonly WebView2 _negative = new() { Width = 2, Height = 2 };
    private readonly LeaseBook _leases = new();
    private sealed class NativeController
    {
        public required string Profile { get; init; }
        public required string ProfilePath { get; init; }
        public TabLease? Lease { get; set; }
    }
    private readonly Dictionary<WebView2, NativeController> _controllers = new(ReferenceEqualityComparer.Instance);
    private readonly SortedDictionary<string, object> _checks = new();
    private readonly string _root;
    private readonly string _assets;
    private readonly string _extension;
    private readonly string _receipt;
    private readonly bool _batch;
    private readonly bool _nativeIdle;
    private readonly bool _nativeKeys;
    private readonly bool _nativeAuth;
    private bool _failed;
    private readonly List<string> _trace = new();

    public ProbeForm(string[] args)
    {
        var options = Parse(args);
        _root = ProbeOutput.Resolve(options.GetValueOrDefault("--out"));
        _assets = Path.GetFullPath(options.GetValueOrDefault("--assets") ??
            Path.Combine(AppContext.BaseDirectory, "renderer"));
        _extension = Path.GetFullPath(options.GetValueOrDefault("--extension") ??
            Path.Combine(AppContext.BaseDirectory, "extension"));
        _receipt = Path.Combine(_root, "receipt.json");
        _batch = options.ContainsKey("--batch");
        _nativeIdle = options.ContainsKey("--native-idle");
        _nativeKeys = options.ContainsKey("--native-keys");
        _nativeAuth = options.ContainsKey("--native-auth");
        Text = "CoSoS 7P-B / isolated WebView2 browser capability probe — NOT production";
        Width = 1300;
        Height = 800;
        var panes = new SplitContainer { Dock = DockStyle.Fill, Orientation = Orientation.Vertical, SplitterDistance = 640 };
        panes.Panel1.Controls.Add(_a);
        panes.Panel2.Controls.Add(_b);
        Controls.Add(panes);
        Controls.Add(_negative);
        Shown += async (_, _) => await ProbeAndRecordAsync();
    }

    private static Dictionary<string, string> Parse(string[] args)
    {
        var parsed = new Dictionary<string, string>(StringComparer.Ordinal);
        for (var i = 0; i < args.Length; i++)
        {
            if (args[i] is "--batch" or "--native-idle" or "--native-keys" or "--native-auth")
            {
                parsed.Add(args[i], "true");
                continue;
            }
            if (i + 1 >= args.Length || args[i] is not ("--out" or "--assets" or "--extension"))
                throw new ArgumentException("Expected --out, --assets or --extension with value");
            parsed.Add(args[i], args[++i]);
        }
        return parsed;
    }

    private void Mark(string gate, string status, string evidence)
    {
        _checks[gate] = new { status, evidence };
        if (status == "FAIL") _failed = true;
    }

    private async Task ProbeAndRecordAsync()
    {
        Directory.CreateDirectory(_root);
        try
        {
            if (!File.Exists(Path.Combine(_extension, "manifest.json")) ||
                !File.Exists(Path.Combine(_assets, "index.html")))
                throw new FileNotFoundException("Missing compiled renderer or unpacked synthetic extension.");

            await SetupAsync(_a, "A", enabled: true);
            await SetupAsync(_b, "B", enabled: true);
            Mark("extension_install", "PASS", "Unpacked extension installed in both synthetic profiles.");

            // Fail-closed negative feature toggle: a new independent environment with extensions off.
            // The SDK explicitly permits enumerating previously installed extensions
            // while disabled: refusal and non-execution, NOT empty inventory, are the contract.
            await SetupAsync(_negative, "disabled", enabled: false, install: false);
            var beforeDisabled = await _negative.CoreWebView2.Profile.GetBrowserExtensionsAsync();
            var beforeIds = beforeDisabled.Select(e => e.Id).OrderBy(id => id).ToArray();
            var refused = false;
            var disabledError = "none";
            try
            {
                await _negative.CoreWebView2.Profile.AddBrowserExtensionAsync(_extension);
            }
            catch (COMException ex)
            {
                disabledError = ex.HResult.ToString("X8");
                refused = ex.HResult == unchecked((int)0x80070032); // ERROR_NOT_SUPPORTED
            }
            var afterDisabled = await _negative.CoreWebView2.Profile.GetBrowserExtensionsAsync();
            var afterIds = afterDisabled.Select(e => e.Id).OrderBy(id => id).ToArray();
            _negative.CoreWebView2.Navigate("https://synthetic.cos.test/index.html");
            var disabledTitle = await WaitForAsync(_negative, "document.title");
            var disabledMain = await EvaluateStringAsync(_negative, "typeof window.__cosMain");
            var disabledIsolated = await EvaluateStringAsync(_negative,
                "document.documentElement.dataset.cosIsolated ?? 'none'");
            Mark("extension_disabled_refusal",
                refused && beforeIds.SequenceEqual(afterIds)
                    && disabledTitle == "7P-B Synthetic" && disabledMain == "undefined"
                    && disabledIsolated == "none" ? "PASS" : "FAIL",
                "HRESULT " + disabledError + ", before=" + beforeIds.Length
                + ", after=" + afterIds.Length + ", MAIN=" + disabledMain
                + ", ISOLATED=" + disabledIsolated
                + "; installed inventory may persist but disabled extensions must not execute.");

            var pathA = Path.GetFullPath(_a.CoreWebView2.Profile.ProfilePath);
            var pathB = Path.GetFullPath(_b.CoreWebView2.Profile.ProfilePath);
            var pathDisabled = Path.GetFullPath(_negative.CoreWebView2.Profile.ProfilePath);
            Mark("profile_directories",
                !string.Equals(pathA, pathB, StringComparison.OrdinalIgnoreCase)
                && !string.Equals(pathA, pathDisabled, StringComparison.OrdinalIgnoreCase)
                && !string.Equals(pathB, pathDisabled, StringComparison.OrdinalIgnoreCase)
                  ? "PASS" : "FAIL",
                "Physical profile paths: A=" + pathA + ", B=" + pathB + ", disabled=" + pathDisabled);

            _a.CoreWebView2.Navigate("https://synthetic.cos.test/index.html");
            _b.CoreWebView2.Navigate("https://synthetic.cos.test/index.html");
            var a = await WaitForAsync(_a, "document.documentElement.dataset.rendererReady");
            var b = await WaitForAsync(_b, "document.documentElement.dataset.rendererReady");
            Mark("rich_editor", a == "ready" && b == "ready" ? "PASS" : "FAIL",
                "Runtime renderer DOM readiness: A=" + a + ", B=" + b + "; manual IME/keyboard inspection still required.");
            var worlds = await WaitForAsync(_a, "document.documentElement.dataset.cosMain");
            var isolated = await WaitForAsync(_a, "document.documentElement.dataset.cosIsolated");
            var leak = await EvaluateStringAsync(_a, "typeof window.__cosIsolatedSecret");
            var mainVisible = await EvaluateStringAsync(_a, "typeof window.__cosMain");
            Mark("extension_worlds", worlds == "yes" && isolated == "yes" && leak == "undefined" && mainVisible == "string"
                ? "PASS" : "FAIL", $"MAIN={worlds}, ISOLATED={isolated}, private={leak}, mainGlobal={mainVisible}");

            var worker = await WaitForAsync(_a, "document.documentElement.dataset.cosWorkerAck");
            var debugger = await WaitForAsync(_a, "document.documentElement.dataset.cosDebugger");
            Mark("worker_message", worker == "ok" ? "PASS" : "FAIL", "Content -> MV3 worker -> content ACK=" + worker);
            Mark("extension_debugger_api",
                debugger == "roundtrip-event-detached" ? "PASS" : "FAIL",
                "Synthetic chrome.debugger attach/evaluate/event/detach=" + debugger
                + "; physical WebView2/worker-target parity remains a separate 7P-L1 gate.");

            // Host CDP and target-ownership are separate from chrome.debugger and must never be conflated.
            var own = ElectController(_a, "A", "tab-A");
            var other = ElectController(_b, "B", "tab-B");
            var hostCdp = await CdpTitleAsync(_a, own, "A", "tab-A");
            var hostCdpB = await CdpTitleAsync(_b, other, "B", "tab-B");
            bool borrowedACdpDenied = false;
            try { await CdpTitleAsync(_b, own, "A", "tab-A"); }
            catch (UnauthorizedAccessException) { borrowedACdpDenied = true; }
            Mark("native_cross_profile_cdp_denial", borrowedACdpDenied ? "PASS" : "FAIL",
                $"Using A's valid lease and labels against the actual B WebView2 controller denied={borrowedACdpDenied}.");
            var stale = ElectController(_a, "A", "tab-A");
            var staleRefused = !_leases.Allows(own, "A", "tab-A");
            var staleCdpDenied = false;
            try { await CdpTitleAsync(_a, own, "A", "tab-A"); }
            catch (UnauthorizedAccessException) { staleCdpDenied = true; }
            Mark("debugger_target", hostCdp == "7P-B Synthetic" && hostCdpB == hostCdp && staleRefused && staleCdpDenied
                ? "PASS" : "FAIL", "Host CDP titles and actual stale CDP method denial. This is NOT extension chrome.debugger.");
            RetireControllerLease(_a);
            RetireControllerLease(_b);

            await _a.CoreWebView2.ExecuteScriptAsync("localStorage.setItem('cos7pb','A')");
            var cross = await EvaluateStringAsync(_b, "localStorage.getItem('cos7pb') ?? 'none'");
            Mark("tab_profile_isolation", cross == "none" ? "PASS" : "FAIL",
                "Synthetic profile B cannot read profile A storage. Physical device repetition pending.");

            // Refuse externally addressed navigation; no external request may be sent.
            var priorBlocked = _trace.Count;
            _a.CoreWebView2.Navigate("https://external.example.org/should-not-load");
            await Task.Delay(300);
            Mark("browser_navigation",
                _trace.Count > priorBlocked && _a.CoreWebView2.Source.StartsWith("https://synthetic.cos.test/", StringComparison.Ordinal)
                    ? "PASS" : "FAIL",
                "Attempted non-synthetic target canceled by NavigationStarting allowlist.");

            var components = await EvaluateStringAsync(_a, "document.documentElement.dataset.componentResults");
            Mark("pdf_math_terminal", components == "ready" ? "PASS" : "FAIL",
                "Real CodeMirror/KaTeX/pdf.js/xterm fixture initialized. PDF selection, IME and resize manual gate remains.");

            if (_nativeKeys)
            {
                // Chromium CDP dispatch exercises the embedded editor's native
                // input path without taking focus from another user's app.
                // It is NOT evidence of physical Windows keyboard or OS IME.
                await _a.CoreWebView2.ExecuteScriptAsync("document.querySelector('.cm-content').focus()");
                await _a.CoreWebView2.CallDevToolsProtocolMethodAsync("Input.insertText",
                    JsonSerializer.Serialize(new { text = "CDP_TYPED_PROBE" }));
                await Task.Delay(100);
                var cdpTyped = await EvaluateStringAsync(_a,
                    "document.querySelector('.cm-content').textContent");
                Mark("native_cdp_input",
                    cdpTyped.Contains("CDP_TYPED_PROBE") ? "PASS" : "UNVERIFIED",
                    $"Chromium CDP Input.insertText into actual embedded CodeMirror={cdpTyped.Contains("CDP_TYPED_PROBE")}; does not imply OS keyboard/IME.");
                var imeResult = "UNVERIFIED";
                var imeEvidence = "Chrome CDP composition not performed.";
                try
                {
                    await _a.CoreWebView2.CallDevToolsProtocolMethodAsync("Input.imeSetComposition",
                        JsonSerializer.Serialize(new { text = "にほん", selectionStart = 3, selectionEnd = 3 }));
                    await _a.CoreWebView2.CallDevToolsProtocolMethodAsync("Input.insertText",
                        JsonSerializer.Serialize(new { text = "日本" }));
                    await Task.Delay(100);
                    var afterComposition = await EvaluateStringAsync(_a,
                        "document.querySelector('.cm-content').textContent");
                    imeResult = afterComposition.Contains("日本") ? "PASS" : "UNVERIFIED";
                    imeEvidence = "Chromium CDP simulated IME composition/commit to embedded CodeMirror="
                        + afterComposition.Contains("日本") + "; not a real OS input method test.";
                }
                catch (Exception error)
                {
                    imeEvidence = "CDP composition did not complete: " + error.GetType().Name
                        + "; real OS IME remains unsupported on this layout.";
                }
                Mark("native_cdp_ime_simulation", imeResult, imeEvidence);
                // SendKeys routes through the Windows keyboard queue; unlike
                // ExecuteScriptAsync it does not set the editor DOM value directly.
                // Only target this task-owned synthetic window, never other apps.
                SetForegroundWindow(Handle);
                await Task.Delay(150);
                var foreground = GetForegroundWindow() == Handle;
                if (!foreground)
                {
                    Mark("native_keyboard_editor", "UNVERIFIED",
                        "Windows refused focus to the probe window; did not inject keyboard input.");
                }
                else
                {
                    _a.Focus();
                    await _a.CoreWebView2.ExecuteScriptAsync("document.querySelector('.cm-content').focus()");
                    await Task.Delay(150);
                    var before = await EvaluateStringAsync(_a,
                        "document.querySelector('.cm-content').textContent");
                    SendKeys.SendWait("^a");
                    await Task.Delay(80);
                    SendKeys.SendWait("NativeKeyboard42");
                    await Task.Delay(150);
                    var typed = await EvaluateStringAsync(_a,
                        "document.querySelector('.cm-content').textContent");
                    SendKeys.SendWait("^z");
                    await Task.Delay(200);
                    var restored = await EvaluateStringAsync(_a,
                        "document.querySelector('.cm-content').textContent");
                    SendKeys.SendWait("^a");
                    await Task.Delay(100);
                    var selected = await EvaluateStringAsync(_a,
                        "String(document.querySelector('.cm-content').getAttribute('aria-selected') ?? '') + '|' + String(window.getSelection()?.toString()?.length ?? 0)");
                    var selectionLength = int.TryParse(selected.Split('|').Last(), out var length) ? length : 0;
                    Mark("native_keyboard_editor",
                        before.Contains("const synthetic") && typed.Contains("NativeKeyboard42")
                        && restored.Contains("const synthetic") && selectionLength > 0 ? "PASS" : "FAIL",
                        $"Windows SendKeys to focused CodeMirror: injected text={typed.Contains("NativeKeyboard42")}, undo restored={restored.Contains("const synthetic")}, selected text chars={selectionLength}; no JS mutation used.");
                    // Unicode character input checks text dispatch, NOT real IME composition.
                    SendKeys.SendWait("é");
                    await Task.Delay(120);
                    var unicode = await EvaluateStringAsync(_a,
                        "document.querySelector('.cm-content').textContent");
                    Mark("native_unicode_keyboard",
                        unicode.Contains("é") ? "PASS" : "UNVERIFIED",
                        $"OS keyboard Unicode dispatch reached CodeMirror={unicode.Contains("é")}; not evidence of CJK IME preedit/candidate/commit.");
                }
            }

            if (_nativeAuth)
                await VerifySyntheticAuthAsync();

            if (_nativeIdle)
            {
            // 7P-L1 automated Windows interactions against the real embedded renderer.
            // DOM actions prove WebView2 integration; they do not prove OS IME/clipboard UX.
            var pdfSearch = await EvaluateStringAsync(_a,
                "(() => { document.querySelector('#search-pdf').click(); return document.querySelector('#result').textContent; })()");
            var terminalRows = await EvaluateStringAsync(_a,
                "(() => { document.querySelector('#resize-terminal').click(); return String(document.querySelectorAll('.xterm-rows > div').length); })()");
            var mathText = await EvaluateStringAsync(_a,
                "document.querySelector('.katex-mathml')?.textContent ?? 'missing'");
            var pdfText = await EvaluateStringAsync(_a,
                "document.querySelector('#pdf-text-layer')?.textContent ?? 'missing'");
            Mark("native_renderer_dom_interactions",
                pdfSearch == "PDF search: matched" && terminalRows == "12"
                && mathText.Contains("a2") && pdfText.Contains("searchable synthetic") ? "PASS" : "FAIL",
                $"Real WebView2 DOM search={pdfSearch}, xterm rows={terminalRows}, KaTeX accessible math={mathText.Length} chars, PDF text layer={pdfText.Length} chars; OS keyboard/IME/UIA pending.");

            var firstWorker = await EvaluateStringAsync(_a,
                "document.documentElement.dataset.cosWorkerInstance ?? 'missing'");
            var browserExtension = (await _a.CoreWebView2.Profile.GetBrowserExtensionsAsync())
                .Single(e => e.Name == "CoSoS 7P-B Synthetic Only");
            await browserExtension.EnableAsync(false);
            await ReloadObservedAsync(_a);
            var disabledMainAfterReload = await EvaluateStringAsync(_a, "typeof window.__cosMain");
            var disabledWorkerAfterReload = await EvaluateStringAsync(_a,
                "document.documentElement.dataset.cosWorkerInstance ?? 'missing'");
            await browserExtension.EnableAsync(true);
            await ReloadObservedAsync(_a);
            var secondWorker = await WaitForAsync(_a,
                "document.documentElement.dataset.cosWorkerInstance");
            var secondDebugger = await WaitForAsync(_a,
                "document.documentElement.dataset.cosDebugger");
            Mark("native_forced_mv3_retirement",
                firstWorker != "missing" && secondWorker != "missing"
                && firstWorker != secondWorker && disabledMainAfterReload == "undefined"
                && disabledWorkerAfterReload == "missing" ? "PASS" : "FAIL",
                $"Disable/re-enable extension caused new worker instance={firstWorker != secondWorker}; disabled MAIN={disabledMainAfterReload}, disabled worker={disabledWorkerAfterReload}. Natural idle suspension is a separate gate.");
            Mark("native_debugger_reattach",
                secondDebugger == "roundtrip-event-detached" ? "PASS" : "FAIL",
                $"After forced MV3 retirement/restart, synthetic chrome.debugger reattach/evaluate/event/detach={secondDebugger}.");
                // No extension messages, probes or debugger attachments during idle.
                // Verify real automatic MV3 eviction independently from forced disabling.
                await Task.Delay(TimeSpan.FromSeconds(65));
                await ReloadObservedAsync(_a);
                var afterIdleWorker = await WaitForAsync(_a,
                    "document.documentElement.dataset.cosWorkerInstance");
                var afterIdleDebugger = await WaitForAsync(_a,
                    "document.documentElement.dataset.cosDebugger");
                Mark("mv3_lifecycle",
                    afterIdleWorker != "missing" && afterIdleWorker != secondWorker
                        ? "PASS" : "UNVERIFIED",
                    $"65-second real idle: worker instance changed={afterIdleWorker != secondWorker}; reboot debugger={afterIdleDebugger}. Natural suspend is only demonstrated if worker generation changes.");
                Mark("native_debugger_after_idle",
                    afterIdleDebugger == "roundtrip-event-detached" ? "PASS" : "FAIL",
                    "Synthetic chrome.debugger reattach/evaluate/event/detach after 65-second idle and same-profile reload.");
                await VerifyTabCloseReopenAsync();
            }
        }
        catch (Exception ex)
        {
            Mark("fatal", "FAIL", ex.GetType().Name + ": " + ex.Message);
        }
        finally
        {
            // Deliberately never turn cloud checks into native PASS or invoke real browser/provider sign-in.
            if (!_checks.ContainsKey("mv3_lifecycle"))
                Mark("mv3_lifecycle", "UNVERIFIED", "Natural MV3 worker idle suspension not yet observed; pass --native-idle for physical Windows proof.");
            if (!_checks.ContainsKey("sign_in_constraints"))
                Mark("sign_in_constraints", "UNVERIFIED", "Synthetic consent flow was not run; no real provider/account used.");
            Mark("native_windows_gate", "NOT_RUN", "7P-L1 separate owner-operated physical Windows gate.");
            Mark("distribution_budgets", "NOT_RUN", "No installer size/RAM baselines.");
            var receipt = new
            {
                task = "7P-B", source = "synthetic WebView2 WinForms adapter spike (NOT production Rust host)",
                utc = DateTimeOffset.UtcNow, runtime = Environment.OSVersion.ToString(),
                executableVersion = typeof(ProbeForm).Assembly.GetCustomAttributes(
                    typeof(System.Reflection.AssemblyInformationalVersionAttribute), false)
                    .OfType<System.Reflection.AssemblyInformationalVersionAttribute>()
                    .FirstOrDefault()?.InformationalVersion ?? "unavailable",
                executableSha256 = HashCurrentExecutable(),
                // A is deliberately disposed during tab-recreation qualification.
                // B stays live and uses the same installed Evergreen WebView2 runtime.
                webviewRuntime = _b.CoreWebView2?.Environment.BrowserVersionString ?? "unavailable",
                profileRoot = _root, syntheticOnly = true, noRealProfilesOrAccounts = true,
                native7PL1Accepted = false, checks = _checks, trace = _trace
            };
            File.WriteAllText(_receipt, JsonSerializer.Serialize(receipt, new JsonSerializerOptions { WriteIndented = true }));
            Text += " | Receipt: " + _receipt;
            if (_batch)
            {
                Environment.ExitCode = _failed ? 1 : 0;
                BeginInvoke(new Action(Close));
            }
        }
    }

    private async Task SetupAsync(WebView2 view, string profile, bool enabled, bool install = true)
    {
        var profileRoot = Path.Combine(_root, "isolated-" + profile);
        var env = await CoreWebView2Environment.CreateAsync(null, profileRoot,
            new CoreWebView2EnvironmentOptions
            {
                AreBrowserExtensionsEnabled = enabled,
                AllowSingleSignOnUsingOSPrimaryAccount = false
            });
        await view.EnsureCoreWebView2Async(env);
        var actualProfile = Path.GetFullPath(view.CoreWebView2.Profile.ProfilePath);
        var expectedProfile = Path.GetFullPath(profileRoot) + Path.DirectorySeparatorChar;
        if (!actualProfile.StartsWith(expectedProfile, StringComparison.OrdinalIgnoreCase))
            throw new UnauthorizedAccessException("WebView2 actual profile escaped the host-owned profile root");
        _controllers.Add(view, new NativeController { Profile = profile, ProfilePath = actualProfile });
        view.CoreWebView2.NavigationStarting += (_, e) =>
        {
            // A document navigation ends authorization for the previous document,
            // including allowed same-origin navigation and denied external attempts.
            RetireControllerLease(view);
            if (!Uri.TryCreate(e.Uri, UriKind.Absolute, out var u) ||
                u.Scheme != "https" || u.UserInfo.Length != 0 || u.Port != 443 ||
                !(u.Host == "synthetic.cos.test" ||
                  (u.Host == "login.synthetic.cos.test" && u.AbsolutePath == "/oauth-consent.html")))
            {
                e.Cancel = true;
                _trace.Add("blocked navigation to non-synthetic URL");
            }
        };
        view.CoreWebView2.SetVirtualHostNameToFolderMapping("synthetic.cos.test", _assets,
            CoreWebView2HostResourceAccessKind.DenyCors);
        view.CoreWebView2.SetVirtualHostNameToFolderMapping("login.synthetic.cos.test", _assets,
            CoreWebView2HostResourceAccessKind.DenyCors);
        view.CoreWebView2.NewWindowRequested += (_, e) =>
        {
            // OAuth popup windows must be assigned an owned profile explicitly
            // by a future product adapter. This probe refuses all unowned popups.
            e.Handled = true;
            _trace.Add("blocked unowned popup");
        };
        if (install)
        {
            var extension = await view.CoreWebView2.Profile.AddBrowserExtensionAsync(_extension);
            if (!extension.IsEnabled || (await view.CoreWebView2.Profile.GetBrowserExtensionsAsync()).Count == 0)
                throw new InvalidOperationException("MV3 extension not installed and enabled");
            _trace.Add("synthetic extension installed for " + profile);
        }
    }

    private async Task VerifySyntheticAuthAsync()
    {
        // Every URL below is a local virtual host mapped to the checked fixture.
        // State belongs to this host-only probe, never to page JavaScript.
        var accepted = new SyntheticConsentGrant("A", RandomNumberGenerator.GetHexString(16));
        _a.CoreWebView2.Navigate(accepted.AuthorizationUrl);
        var consentReady = await WaitForAsync(_a,
            "document.title === '7P-L1 Mock Consent' ? (document.documentElement.dataset.mockConsentReady ?? '') : ''");
        Mark("mock_oauth_consent_page", consentReady == "ready" ? "PASS" : "FAIL",
            "Mapped login.synthetic.cos.test consent fixture loaded in actual WebView2 profile A.");
        if (consentReady != "ready") return;

        await _a.CoreWebView2.ExecuteScriptAsync("document.querySelector('#approve').click()");
        var callbackReady = await WaitForAsync(_a,
            "document.title === '7P-L1 Mock Callback' ? 'ready' : ''");
        var callbackUrl = _a.CoreWebView2.Source;
        var modifiedStateUrl = callbackUrl.Replace("state=", "state=forged", StringComparison.Ordinal);
        var forgedDenied = !accepted.Complete("A", modifiedStateUrl, approved: true)
            && !accepted.Complete("A", "https://external.example.org/oauth-callback.html?state=forged&code=synthetic-code-only", approved: true)
            && !accepted.Complete("A", "https://synthetic.cos.test/oauth-callback.html?state=forged&error=access_denied", approved: true);
        // The same callback URL is navigable in profile B as a public synthetic
        // document; it must never authorize B using A's host-owned grant.
        _b.CoreWebView2.Navigate(callbackUrl);
        var bCallbackReady = await WaitForAsync(_b,
            "document.title === '7P-L1 Mock Callback' ? 'ready' : ''");
        // Derive both identities from the actual live WebView2 controllers, not
        // an attacker-controlled profile string supplied by the caller.
        var wrongProfileDenied = bCallbackReady == "ready"
            && !CompleteOwnedConsent(_b, accepted, callbackUrl, approved: true);
        var firstAccepted = callbackReady == "ready"
            && CompleteOwnedConsent(_a, accepted, callbackUrl, approved: true);
        var replayDenied = !CompleteOwnedConsent(_a, accepted, callbackUrl, approved: true);
        Mark("mock_oauth_callback", firstAccepted && replayDenied ? "PASS" : "FAIL",
            $"Native profile A consent redirect verified={firstAccepted}, one-shot replay refused={replayDenied}.");
        Mark("mock_oauth_forgery", forgedDenied ? "PASS" : "FAIL",
            $"Forged state/origin/error callback rejected={forgedDenied} before valid completion.");
        Mark("mock_oauth_profile_isolation",
            bCallbackReady == "ready" && wrongProfileDenied ? "PASS" : "FAIL",
            $"Profile B native controller loaded callback={bCallbackReady == "ready"}; pending A grant rejected when checked against B's actual profile={wrongProfileDenied}.");

        var denied = new SyntheticConsentGrant("A", RandomNumberGenerator.GetHexString(16));
        _a.CoreWebView2.Navigate(denied.AuthorizationUrl);
        var denyReady = await WaitForAsync(_a,
            "document.title === '7P-L1 Mock Consent' ? (document.documentElement.dataset.mockConsentReady ?? '') : ''");
        if (denyReady == "ready")
            await _a.CoreWebView2.ExecuteScriptAsync("document.querySelector('#deny').click()");
        var denyCallbackReady = denyReady == "ready" && await WaitForAsync(_a,
            "document.title === '7P-L1 Mock Callback' ? 'ready' : ''") == "ready";
        var deniedUrl = _a.CoreWebView2.Source;
        var denialRecognized = denyCallbackReady
            && CompleteOwnedConsent(_a, denied, deniedUrl, approved: false);
        var denialNoGrant = !CompleteOwnedConsent(_a, denied, deniedUrl, approved: true);
        Mark("mock_oauth_consent_denial", denialRecognized && denialNoGrant ? "PASS" : "FAIL",
            $"Consent denial callback={denyCallbackReady}, access_denied verified={denialRecognized}, no grant issued={denialNoGrant}.");

        // Keep the test's primary renderer visible for manual follow-up.
        _a.CoreWebView2.Navigate("https://synthetic.cos.test/index.html");
        _b.CoreWebView2.Navigate("https://synthetic.cos.test/index.html");
        var restored = await WaitForAsync(_a,
            "document.documentElement.dataset.rendererReady ?? ''");
        var all = firstAccepted && replayDenied && wrongProfileDenied && forgedDenied
            && bCallbackReady == "ready" && denialRecognized && denialNoGrant
            && consentReady == "ready" && restored == "ready";
        Mark("sign_in_constraints", all ? "PASS" : "FAIL",
            "Synthetic-only WebView2 consent/callback/state/replay/denial/profile checks; "
            + "real provider OAuth, account sign-in, OS SSO and token exchange remain future user gates.");
    }

    private bool CompleteOwnedConsent(WebView2 view, SyntheticConsentGrant grant,
        string callbackUrl, bool approved)
    {
        if (view.IsDisposed || !_controllers.TryGetValue(view, out var owner)
            || !IsNativeControllerCurrent(view, owner)
            || !string.Equals(view.CoreWebView2.Source, callbackUrl, StringComparison.Ordinal))
            return false;
        return grant.Complete(owner.Profile, callbackUrl, approved);
    }

    private TabLease ElectController(WebView2 view, string profile, string tab)
    {
        if (view.IsDisposed || !_controllers.TryGetValue(view, out var owner)
            || owner.Profile != profile || !IsNativeControllerCurrent(view, owner))
            throw new UnauthorizedAccessException("Cannot elect CDP for an unowned native controller");
        RetireControllerLease(view);
        var lease = _leases.Elect(profile, tab);
        owner.Lease = lease;
        return lease;
    }

    private static bool IsNativeControllerCurrent(WebView2 view, NativeController owner)
        => !view.IsDisposed && view.CoreWebView2 is not null
           && string.Equals(Path.GetFullPath(view.CoreWebView2.Profile.ProfilePath),
               owner.ProfilePath, StringComparison.OrdinalIgnoreCase);

    private static string HashCurrentExecutable()
    {
        try
        {
            if (Environment.ProcessPath is not string path) return "unavailable";
            using var stream = File.OpenRead(path);
            return Convert.ToHexString(SHA256.HashData(stream));
        }
        catch (IOException) { return "unavailable"; }
        catch (UnauthorizedAccessException) { return "unavailable"; }
    }

    private void RetireControllerLease(WebView2 view)
    {
        if (_controllers.TryGetValue(view, out var owner) && owner.Lease is { } lease)
        {
            // Retire the authority before disposing or replacing the controller.
            owner.Lease = null;
            _leases.Close(lease);
        }
    }

    private void RetireController(WebView2 view)
    {
        RetireControllerLease(view);
        _controllers.Remove(view);
    }

    // A real WebView2 controller must be destroyed and recreated in the same
    // isolated profile. LeaseBook-only tests cannot prove browser tab lifetime.
    private async Task VerifyTabCloseReopenAsync()
    {
        var oldProfile = Path.GetFullPath(_a.CoreWebView2.Profile.ProfilePath);
        var oldLease = ElectController(_a, "A", "tab-close-reopen");
        var oldTarget = await ReadOwnedTargetIdAsync(_a);
        await _a.CoreWebView2.ExecuteScriptAsync(
            "localStorage.setItem('cos7pl1-tab-reopen','owned-A')");
        var panel = _a.Parent ?? throw new InvalidOperationException("Synthetic A panel not available");
        RetireController(_a);
        _a.Dispose();
        bool revokedOnClose = !_leases.Allows(oldLease, "A", "tab-close-reopen");

        var replacement = new WebView2 { Dock = DockStyle.Fill };
        try
        {
            panel.Controls.Add(replacement);
            await SetupAsync(replacement, "A", enabled: true, install: false);
            var freshProfile = Path.GetFullPath(replacement.CoreWebView2.Profile.ProfilePath);
            bool deniedByCdp = false;
            try { await CdpTitleAsync(replacement, oldLease, "A", "tab-close-reopen"); }
            catch (UnauthorizedAccessException) { deniedByCdp = true; }

            replacement.CoreWebView2.Navigate("https://synthetic.cos.test/index.html");
            var ready = await WaitForAsync(replacement, "document.documentElement.dataset.rendererReady");
            var debuggerAck = await WaitForAsync(replacement, "document.documentElement.dataset.cosDebugger");
            var stored = await EvaluateStringAsync(replacement,
                "localStorage.getItem('cos7pl1-tab-reopen') ?? 'missing'");
            var currentLease = ElectController(replacement, "A", "tab-close-reopen");
            var newTitle = await CdpTitleAsync(replacement, currentLease, "A", "tab-close-reopen");
            var newTarget = await ReadOwnedTargetIdAsync(replacement);
            var isolatedB = await EvaluateStringAsync(_b,
                "localStorage.getItem('cos7pl1-tab-reopen') ?? 'missing'");
            bool sameProfile = string.Equals(oldProfile, freshProfile, StringComparison.OrdinalIgnoreCase);
            bool corePassed = revokedOnClose && deniedByCdp && sameProfile && ready == "ready"
                && debuggerAck == "roundtrip-event-detached" && stored == "owned-A"
                && isolatedB == "missing" && newTitle == "7P-B Synthetic";
            Mark("native_tab_close_reopen", corePassed ? "PASS" : "FAIL",
                $"Disposed real controller, revoked before replacement={revokedOnClose}, "
                + $"same profile={sameProfile}, old CDP denied={deniedByCdp}, new CDP={newTitle}, "
                + $"MV3 debugger={debuggerAck}, persisted storage={stored}, B isolated={isolatedB}.");

            // Target.getTargetInfo with no targetId refers to the invoking CDP
            // session's target. Enumerating all same-origin targets is NOT proof
            // that a target belongs to this specific WebView2 controller.
            bool targetIdsObserved = oldTarget is not null && newTarget is not null;
            bool changed = oldTarget != newTarget;
            Mark("native_cdp_target_recreation",
                !targetIdsObserved ? "UNVERIFIED" : changed && corePassed ? "PASS" : "FAIL",
                $"Per-controller CDP target identity observed before={oldTarget is not null}, "
                + $"after={newTarget is not null}, changed={changed}.");
            await ReloadObservedAsync(replacement);
            bool oldDocumentLeaseDenied = false;
            try { await CdpTitleAsync(replacement, currentLease, "A", "tab-close-reopen"); }
            catch (UnauthorizedAccessException) { oldDocumentLeaseDenied = true; }
            var afterNavigationLease = ElectController(replacement, "A", "tab-close-reopen");
            var afterNavigationTitle = await CdpTitleAsync(
                replacement, afterNavigationLease, "A", "tab-close-reopen");
            Mark("native_navigation_cdp_revocation",
                oldDocumentLeaseDenied && afterNavigationTitle == "7P-B Synthetic" ? "PASS" : "FAIL",
                $"Same-origin navigation revoked old document CDP lease={oldDocumentLeaseDenied}; "
                + $"new lease title={afterNavigationTitle}.");
        }
        finally
        {
            RetireController(replacement);
            replacement.Dispose();
        }
    }

    private static async Task<string?> ReadOwnedTargetIdAsync(WebView2 view)
    {
        try
        {
            var raw = await view.CoreWebView2.CallDevToolsProtocolMethodAsync("Target.getTargetInfo", "{}");
            using var response = JsonDocument.Parse(raw);
            if (!response.RootElement.TryGetProperty("targetInfo", out var target)
                || target.GetProperty("type").GetString() != "page"
                || target.GetProperty("url").GetString()?.StartsWith(
                    "https://synthetic.cos.test/", StringComparison.Ordinal) != true)
                return null;
            return target.GetProperty("targetId").GetString();
        }
        catch (COMException) { return null; /* Unsupported scope stays UNVERIFIED. */ }
    }

    private async Task<string> CdpTitleAsync(WebView2 view, TabLease lease, string profile, string tab)
    {
        if (!_leases.Allows(lease, profile, tab)
            || !_controllers.TryGetValue(view, out var owner) || owner.Profile != profile
            || owner.Lease != lease || !IsNativeControllerCurrent(view, owner))
            throw new UnauthorizedAccessException("Stale or cross-profile CDP lease");
        var result = await view.CoreWebView2.CallDevToolsProtocolMethodAsync(
            "Runtime.evaluate", "{\"expression\":\"document.title\",\"returnByValue\":true}");
        using var response = JsonDocument.Parse(result);
        return response.RootElement.GetProperty("result").GetProperty("value").GetString() ?? "";
    }

    private static async Task<string> EvaluateStringAsync(WebView2 view, string js)
    {
        var response = await view.CoreWebView2.ExecuteScriptAsync(js);
        return JsonSerializer.Deserialize<string>(response) ?? "";
    }

    private static async Task<string> WaitForAsync(WebView2 view, string expression)
    {
        for (var attempt = 0; attempt < 40; attempt++)
        {
            try
            {
                var value = await EvaluateStringAsync(view, expression + " ?? ''");
                if (!string.IsNullOrEmpty(value)) return value;
            }
            catch (COMException) { } // navigation context not yet initialized
            await Task.Delay(250);
        }
        return "timeout";
    }

    private static async Task ReloadObservedAsync(WebView2 view)
    {
        var completion = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        EventHandler<CoreWebView2NavigationCompletedEventArgs>? handler = null;
        handler = (_, args) =>
        {
            view.CoreWebView2.NavigationCompleted -= handler;
            completion.TrySetResult(args.IsSuccess);
        };
        view.CoreWebView2.NavigationCompleted += handler;
        try
        {
            view.CoreWebView2.Reload();
            var loaded = await completion.Task.WaitAsync(TimeSpan.FromSeconds(15));
            if (!loaded) throw new InvalidOperationException("Synthetic reload failed");
        }
        finally
        {
            view.CoreWebView2.NavigationCompleted -= handler;
        }
    }
}
