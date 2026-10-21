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
            if (args[i] == "--batch")
            {
                parsed.Add("--batch", "true");
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
                debugger == "roundtrip-event-detached" ? "PASS" : "UNVERIFIED",
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
        }
        catch (Exception ex)
        {
            Mark("fatal", "FAIL", ex.GetType().Name + ": " + ex.Message);
        }
        finally
        {
            // Deliberately never turn cloud checks into native PASS or invoke real browser/provider sign-in.
            Mark("mv3_lifecycle", "UNVERIFIED", "Worker suspension/restart requires 7P-L1 manual physical Windows reproduction.");
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
}
