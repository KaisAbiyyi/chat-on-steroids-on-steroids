using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace NativeBrowserProbe;
internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        Application.Run(new BrowserForm(args));
    }
}

internal sealed class BrowserForm : Form
{
    private const string Origin = "https://synthetic.cos.test";
    private const string Url = Origin + "/index.html";
    // Only this exact public generator output is admitted; arbitrary real chat JSON is refused.
    private const string FixtureSha = "bbe3f0dadd3369fd2574d7212b646bd725633697eec4386a5c4293b8ed71cb6a";
    private readonly WebView2 _view = new() { Dock = DockStyle.Fill };
    private readonly string _output;
    private readonly string _fixture;
    private readonly string _expectedFixtureSha;
    private readonly string _sourceSha;
    private readonly string _harnessSourcePath;
    private readonly Stopwatch _clock = Stopwatch.StartNew();
    private readonly List<Sample> _samples = [];
    private CoreWebView2Environment? _environment;

    private sealed record Sample(double ElapsedMs, long WorkingSetBytes, long PrivateBytes, int ProcessCount);
    private sealed class Outcome
    {
        public string Task { get; init; } = "PERF-01";
        public string Scope { get; init; } = "synthetic_webview2_native_probe_only_not_product_ui";
        public required string SourceSha { get; init; }
        public required string FixtureSha256 { get; init; }
        public string BinarySha256 { get; init; } = "";
        public string ManagedAssemblySha256 { get; init; } = "";
        public string PageSha256 { get; init; } = "";
        public string HarnessSourceSha256 { get; init; } = "";
        public bool ProbeHarnessIncludedInGitCommit { get; init; } = false;
        public bool RealWebView2Used { get; init; }
        public string Status { get; init; } = "NOT_RUN";
        public string Error { get; init; } = "";
        public double StartupToContentMs { get; init; }
        public double P50ScrollMs { get; init; }
        public double P95ScrollMs { get; init; }
        public double MaxScrollMs { get; init; }
        public int RenderedMessages { get; init; }
        public int VisibleDomRows { get; init; }
        public int SampleCount { get; init; }
        public double MaxWholeTreeWorkingSetMiB { get; init; }
        public double MaxWholeTreePrivateMiB { get; init; }
        public int MaxProcessCount { get; init; }
        public string OriginalCosMatchedUi { get; init; } = "NOT_RUN";
        public string CososProductionUi { get; init; } = "NOT_RUN";
    }

    internal BrowserForm(string[] args)
    {
        if (args.Length != 5) throw new ArgumentException("Usage: NativeBrowserProbe.exe <D:-output-root> <D:-fixture.json> <fixture-sha256> <source-commit-sha> <D:-Program.cs>");
        _output = Path.GetFullPath(args[0]);
        _fixture = Path.GetFullPath(args[1]);
        _expectedFixtureSha = args[2];
        _sourceSha = args[3];
        _harnessSourcePath = Path.GetFullPath(args[4]);
        if (!string.Equals(_expectedFixtureSha, FixtureSha, StringComparison.OrdinalIgnoreCase))
            throw new UnauthorizedAccessException("Only the pinned public synthetic fixture is allowed");
        if (!_output.StartsWith(@"D:\projects\CoS-Rust\", StringComparison.OrdinalIgnoreCase) ||
            !_fixture.StartsWith(@"D:\projects\CoS-Rust\", StringComparison.OrdinalIgnoreCase) ||
            !_harnessSourcePath.StartsWith(@"D:\projects\CoS-Rust\", StringComparison.OrdinalIgnoreCase) ||
            !File.Exists(_harnessSourcePath))
            throw new UnauthorizedAccessException("D: project-only paths");
        RejectReparseAncestors(_output);
        RejectReparseAncestors(_fixture);
        RejectReparseAncestors(_harnessSourcePath);
        if (Directory.Exists(_output)) throw new IOException("Output directory already exists");
        Directory.CreateDirectory(_output);
        RejectReparseAncestors(_output);
        Text = "CoSoS PERF-01 WebView2 synthetic native feasibility (NOT production)";
        Width = 960;
        Height = 720;
        Controls.Add(_view);
        Shown += async (_, _) => await RunAsync();
    }

    private static string FileSha(string path)
    {
        using var f = File.OpenRead(path);
        return Convert.ToHexString(SHA256.HashData(f)).ToLowerInvariant();
    }
    private static void RejectReparseAncestors(string candidate)
    {
        var current = Path.GetFullPath(candidate);
        while (true)
        {
            if ((Directory.Exists(current) || File.Exists(current)) &&
                (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                throw new UnauthorizedAccessException("Reparse/junction path refused for benchmark evidence");
            var parent = Path.GetDirectoryName(current);
            if (parent is null || string.Equals(parent, current, StringComparison.OrdinalIgnoreCase)) break;
            current = parent;
        }
    }
    private (long WorkingSet, long Private, int Count) ReadTree()
    {
        var pids = new HashSet<int> { Environment.ProcessId };
        try
        {
            if (_environment != null)
                foreach (var proc in _environment.GetProcessInfos()) pids.Add((int)proc.ProcessId);
        }
        catch (Exception ex) when (ex is InvalidOperationException or COMException) { }
        long ws = 0, privateBytes = 0;
        int count = 0;
        foreach (var pid in pids)
        {
            try
            {
                using var p = Process.GetProcessById(pid);
                ws += p.WorkingSet64;
                privateBytes += p.PrivateMemorySize64;
                count++;
            }
            catch (Exception ex) when (ex is ArgumentException or InvalidOperationException) { }
        }
        return (ws, privateBytes, count);
    }
    private void CaptureSample()
    {
        var mem = ReadTree();
        _samples.Add(new Sample(_clock.Elapsed.TotalMilliseconds, mem.WorkingSet, mem.Private, mem.Count));
    }
    private static async Task<string> Eval(WebView2 view, string script)
    {
        var result = await view.CoreWebView2.ExecuteScriptAsync(script);
        if (result == "null") throw new InvalidOperationException("WebView2 script returned null");
        return result;
    }
    private static double Number(JsonElement result, string key) => result.GetProperty(key).GetDouble();

    private async Task RunAsync()
    {
        var status = "FAIL";
        var error = "";
        double startup = 0, p50 = 0, p95 = 0, max = 0;
        var messages = 0;
        var dom = 0;
        try
        {
            if (FileSha(_fixture) != _expectedFixtureSha.ToLowerInvariant())
                throw new InvalidOperationException("Synthetic source fixture SHA mismatch");
            var profile = Path.Combine(_output, "webview2-profile");
            _environment = await CoreWebView2Environment.CreateAsync(null, profile,
                new CoreWebView2EnvironmentOptions { AreBrowserExtensionsEnabled = false });
            await _view.EnsureCoreWebView2Async(_environment);
            _view.CoreWebView2.PermissionRequested += (_, eventArgs) =>
            {
                eventArgs.State = CoreWebView2PermissionState.Deny;
                eventArgs.Handled = true;
            };
            _view.CoreWebView2.NewWindowRequested += (_, eventArgs) => eventArgs.Handled = true;
            _view.CoreWebView2.NavigationStarting += (_, eventArgs) =>
            {
                if (!eventArgs.Uri.StartsWith(Origin + "/", StringComparison.Ordinal))
                    eventArgs.Cancel = true;
            };
            var assets = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "page"));
            _view.CoreWebView2.SetVirtualHostNameToFolderMapping("synthetic.cos.test", assets,
                CoreWebView2HostResourceAccessKind.DenyCors);
            var ready = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
            _view.CoreWebView2.NavigationCompleted += (_, ev) => ready.TrySetResult(ev.IsSuccess);
            _view.CoreWebView2.Navigate(Url);
            if (!await ready.Task.WaitAsync(TimeSpan.FromSeconds(15)))
                throw new InvalidOperationException("Native WebView2 virtual host failed");
            var fixtureJson = await File.ReadAllTextAsync(_fixture);
            CaptureSample();
            using var loaded = JsonDocument.Parse(await Eval(_view, "window.perf01Load(" + fixtureJson + ")"));
            startup = _clock.Elapsed.TotalMilliseconds;
            messages = loaded.RootElement.GetProperty("count").GetInt32();
            dom = loaded.RootElement.GetProperty("domRows").GetInt32();
            CaptureSample();
            await Eval(_view, "window.perf01ScrollResult = null; window.perf01Scroll().then(x => { window.perf01ScrollResult = x; }).catch(e => { window.perf01ScrollResult = {failed:String(e)}; }); true");
            string? finishedScroll = null;
            for (var n = 0; n < 150; n++)
            {
                CaptureSample();
                var value = await _view.CoreWebView2.ExecuteScriptAsync("window.perf01ScrollResult");
                if (value != "null")
                {
                    finishedScroll = value;
                    break;
                }
                await Task.Delay(150);
            }
            if (finishedScroll is null) throw new TimeoutException("Native scroll test did not settle");
            using var scroll = JsonDocument.Parse(finishedScroll);
            if (scroll.RootElement.TryGetProperty("failed", out var failure))
                throw new InvalidOperationException("Native WebView2 scroll failed: " + failure.GetString());
            p50 = Number(scroll.RootElement, "p50Ms");
            p95 = Number(scroll.RootElement, "p95Ms");
            max = Number(scroll.RootElement, "maxMs");
            dom = Math.Max(dom, scroll.RootElement.GetProperty("maxVisibleDomRows").GetInt32());
            for (var i = 0; i < 20; i++)
            {
                CaptureSample();
                await Task.Delay(250);
            }
            status = messages == 6000 && dom < 50 && scroll.RootElement.GetProperty("n").GetInt32() == 60
                ? "PASS_SYNTHETIC_WEBVIEW2_ONLY" : "FAIL";
        }
        catch (Exception e)
        {
            error = e.GetType().Name + ": " + e.Message;
        }
        finally
        {
            CaptureSample();
            var program = Environment.ProcessPath;
            var pagePath = Path.Combine(AppContext.BaseDirectory, "page", "index.html");
            var report = new Outcome
            {
                SourceSha = _sourceSha,
                FixtureSha256 = _expectedFixtureSha,
                BinarySha256 = program != null ? FileSha(program) : "UNKNOWN",
                ManagedAssemblySha256 = FileSha(typeof(BrowserForm).Assembly.Location),
                PageSha256 = FileSha(pagePath),
                HarnessSourceSha256 = FileSha(_harnessSourcePath),
                RealWebView2Used = _environment != null,
                Status = status, Error = error,
                StartupToContentMs = startup, P50ScrollMs = p50, P95ScrollMs = p95,
                MaxScrollMs = max, RenderedMessages = messages, VisibleDomRows = dom,
                SampleCount = _samples.Count,
                MaxWholeTreeWorkingSetMiB = Math.Round(_samples.Count == 0 ? 0 : _samples.Max(x => x.WorkingSetBytes) / 1048576.0, 2),
                MaxWholeTreePrivateMiB = Math.Round(_samples.Count == 0 ? 0 : _samples.Max(x => x.PrivateBytes) / 1048576.0, 2),
                MaxProcessCount = _samples.Count == 0 ? 0 : _samples.Max(x => x.ProcessCount)
            };
            await File.WriteAllTextAsync(Path.Combine(_output, "native-probe-receipt.json"),
                JsonSerializer.Serialize(report, new JsonSerializerOptions { WriteIndented = true }));
            await File.WriteAllTextAsync(Path.Combine(_output, "native-memory-samples.json"),
                JsonSerializer.Serialize(_samples, new JsonSerializerOptions { WriteIndented = true }));
            Close();
        }
    }
}
