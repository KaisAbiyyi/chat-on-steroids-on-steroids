using System.Runtime.InteropServices;
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
    private readonly SortedDictionary<string, object> _checks = new();
    private readonly string _root;
    private readonly string _assets;
    private readonly string _extension;
    private readonly string _receipt;
    private readonly bool _batch;
    private readonly bool _nativeIdle;
    private readonly bool _nativeKeys;
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
            if (args[i] is "--batch" or "--native-idle" or "--native-keys")
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
            var own = _leases.Elect("A", "tab-A");
            var other = _leases.Elect("B", "tab-B");
            var hostCdp = await CdpTitleAsync(_a, own, "A", "tab-A");
            var hostCdpB = await CdpTitleAsync(_b, other, "B", "tab-B");
            var stale = _leases.Elect("A", "tab-A");
            var staleRefused = !_leases.Allows(own, "A", "tab-A");
            var staleCdpDenied = false;
            try { await CdpTitleAsync(_a, own, "A", "tab-A"); }
            catch (UnauthorizedAccessException) { staleCdpDenied = true; }
            Mark("debugger_target", hostCdp == "7P-B Synthetic" && hostCdpB == hostCdp && staleRefused && staleCdpDenied
                ? "PASS" : "FAIL", "Host CDP titles and actual stale CDP method denial. This is NOT extension chrome.debugger.");
            _leases.Close(stale);
            _leases.Close(other);

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
            Mark("sign_in_constraints", "UNVERIFIED", "No real provider or authenticated account used.");
            Mark("native_windows_gate", "NOT_RUN", "7P-L1 separate owner-operated physical Windows gate.");
            Mark("distribution_budgets", "NOT_RUN", "No installer size/RAM baselines.");
            var receipt = new
            {
                task = "7P-B", source = "synthetic WebView2 WinForms adapter spike (NOT production Rust host)",
                utc = DateTimeOffset.UtcNow, runtime = Environment.OSVersion.ToString(),
                webviewRuntime = _a.CoreWebView2?.Environment.BrowserVersionString ?? "unavailable",
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
        view.CoreWebView2.NavigationStarting += (_, e) =>
        {
            if (!Uri.TryCreate(e.Uri, UriKind.Absolute, out var u) ||
                u.Scheme != "https" || u.Host != "synthetic.cos.test")
            {
                e.Cancel = true;
                _trace.Add("blocked navigation to non-synthetic URL");
            }
        };
        view.CoreWebView2.SetVirtualHostNameToFolderMapping("synthetic.cos.test", _assets,
            CoreWebView2HostResourceAccessKind.DenyCors);
        if (install)
        {
            var extension = await view.CoreWebView2.Profile.AddBrowserExtensionAsync(_extension);
            if (!extension.IsEnabled || (await view.CoreWebView2.Profile.GetBrowserExtensionsAsync()).Count == 0)
                throw new InvalidOperationException("MV3 extension not installed and enabled");
            _trace.Add("synthetic extension installed for " + profile);
        }
    }

    // A real WebView2 controller must be destroyed and recreated in the same
    // isolated profile. LeaseBook-only tests cannot prove browser tab lifetime.
    private async Task VerifyTabCloseReopenAsync()
    {
        var oldProfile = Path.GetFullPath(_a.CoreWebView2.Profile.ProfilePath);
        var oldLease = _leases.Elect("A", "tab-close-reopen");
        var oldTargets = await ReadSyntheticTargetIdsAsync(_a);
        await _a.CoreWebView2.ExecuteScriptAsync(
            "localStorage.setItem('cos7pl1-tab-reopen','owned-A')");
        var panel = _a.Parent ?? throw new InvalidOperationException("Synthetic A panel not available");
        _a.Dispose();

        var replacement = new WebView2 { Dock = DockStyle.Fill };
        panel.Controls.Add(replacement);
        await SetupAsync(replacement, "A", enabled: true, install: false);
        var freshProfile = Path.GetFullPath(replacement.CoreWebView2.Profile.ProfilePath);
        var currentLease = _leases.Elect("A", "tab-close-reopen");
        bool revoked = !_leases.Allows(oldLease, "A", "tab-close-reopen");
        bool deniedByCdp = false;
        try { await CdpTitleAsync(replacement, oldLease, "A", "tab-close-reopen"); }
        catch (UnauthorizedAccessException) { deniedByCdp = true; }

        replacement.CoreWebView2.Navigate("https://synthetic.cos.test/index.html");
        var ready = await WaitForAsync(replacement, "document.documentElement.dataset.rendererReady");
        var debuggerAck = await WaitForAsync(replacement, "document.documentElement.dataset.cosDebugger");
        var stored = await EvaluateStringAsync(replacement,
            "localStorage.getItem('cos7pl1-tab-reopen') ?? 'missing'");
        var newTitle = await CdpTitleAsync(replacement, currentLease, "A", "tab-close-reopen");
        var newTargets = await ReadSyntheticTargetIdsAsync(replacement);
        var isolatedB = await EvaluateStringAsync(_b,
            "localStorage.getItem('cos7pl1-tab-reopen') ?? 'missing'");
        bool sameProfile = string.Equals(oldProfile, freshProfile, StringComparison.OrdinalIgnoreCase);
        bool corePassed = revoked && deniedByCdp && sameProfile && ready == "ready"
            && debuggerAck == "roundtrip-event-detached" && stored == "owned-A"
            && isolatedB == "missing" && newTitle == "7P-B Synthetic";
        Mark("native_tab_close_reopen", corePassed ? "PASS" : "FAIL",
            $"Disposed actual WebView2 controller and recreated same profile={sameProfile}; "
            + $"old CDP lease denied={deniedByCdp}, revoked={revoked}, new CDP={newTitle}, "
            + $"MV3 debugger={debuggerAck}, persisted storage={stored}, profile B isolated={isolatedB}.");

        // Target.getTargets is a browser-CDP capability and may be unavailable
        // through the page-scoped WebView2 CDP adapter. Never call that PASS
        // unless both source-bound native target IDs were actually observable.
        bool targetIdsObserved = oldTargets.Count > 0 && newTargets.Count > 0;
        bool changed = !oldTargets.Overlaps(newTargets);
        Mark("native_cdp_target_recreation",
            !targetIdsObserved ? "UNVERIFIED" : changed && corePassed ? "PASS" : "FAIL",
            $"Source-observed CDP page target IDs: before={oldTargets.Count}, after={newTargets.Count}, "
            + $"disjoint={changed}; no inferred target identity from lease epochs.");
        _leases.Close(currentLease);
    }

    private static async Task<HashSet<string>> ReadSyntheticTargetIdsAsync(WebView2 view)
    {
        var ids = new HashSet<string>(StringComparer.Ordinal);
        try
        {
            var raw = await view.CoreWebView2.CallDevToolsProtocolMethodAsync("Target.getTargets", "{}");
            using var response = JsonDocument.Parse(raw);
            if (!response.RootElement.TryGetProperty("targetInfos", out var targets)) return ids;
            foreach (var target in targets.EnumerateArray())
            {
                if (target.TryGetProperty("type", out var type)
                    && type.GetString() == "page"
                    && target.TryGetProperty("url", out var url)
                    && url.GetString()?.StartsWith("https://synthetic.cos.test/", StringComparison.Ordinal) == true
                    && target.TryGetProperty("targetId", out var id)
                    && id.GetString() is { Length: > 0 } value)
                    ids.Add(value);
            }
        }
        catch (COMException) { /* Unsupported browser CDP scope: remains UNVERIFIED. */ }
        return ids;
    }

    private async Task<string> CdpTitleAsync(WebView2 view, TabLease lease, string profile, string tab)
    {
        if (!_leases.Allows(lease, profile, tab))
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
