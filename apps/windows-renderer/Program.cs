using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace CoSoS.WindowsRenderer;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        if (args.SequenceEqual(["--contract-selftest"]))
        {
            try
            {
                RendererContracts.SelfTest();
                RendererIpcClient.SelfTest();
                Console.WriteLine("15HOST typed contract and authenticated IPC selftests PASS");
                return 0;
            }
            catch (Exception)
            {
                Console.Error.WriteLine("15HOST contract selftest FAILED");
                return 1;
            }
        }

        if (args.SequenceEqual(["--authenticated-ipc"]))
        {
            try
            {
                var input = new BufferedStream(Console.OpenStandardInput());
                var output = new BufferedStream(Console.OpenStandardOutput());
                var bootstrap = RendererIpcClient.ReadBoundedLine(input, 4096)
                    ?? throw new InvalidDataException("Missing renderer bootstrap");
                using var ipc = RendererIpcClient.FromBootstrap(bootstrap, input, output);
                ApplicationConfiguration.Initialize();
                using var window = new RendererWindow(false, ipc, CaptureMode.None, "editor");
                Application.Run(window);
                return Environment.ExitCode;
            }
            catch (Exception)
            {
                Environment.ExitCode = 1;
                return 1;
            }
        }

        var mode = args.FirstOrDefault() switch
        {
            "--synthetic-native" when args.Length == 1 => CaptureMode.None,
            "--synthetic-native-capture" when args.Length == 1 => CaptureMode.Candidate,
            "--golden-native-capture" when args.Length == 1 => CaptureMode.Golden,
            _ => (CaptureMode?)null
        };
        if (mode is null)
        {
            Console.Error.WriteLine("Usage: WindowsRenderer --contract-selftest | --authenticated-ipc | --synthetic-native | --synthetic-native-capture | --golden-native-capture");
            return 2;
        }
        ApplicationConfiguration.Initialize();
        using var renderer = new RendererWindow(mode != CaptureMode.None, null, mode.Value, "editor");
        Application.Run(renderer);
        return Environment.ExitCode;
    }
}

internal static class RendererContracts
{
    internal const int HistoryTotal = 6000;
    internal const int PageLimit = 24;
    internal const int MaximumRequestBytes = 1024;
    internal const int MaximumResponseBytes = 65536;
    internal const ulong Epoch = 1;

    internal static bool IsValidRequest(string raw, out int start)
    {
        start = -1;
        if (Encoding.UTF8.GetByteCount(raw) > MaximumRequestBytes) return false;
        try
        {
            using var doc = JsonDocument.Parse(raw);
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object ||
                root.EnumerateObject().Count() != 3 ||
                !root.TryGetProperty("kind", out var kind) ||
                !root.TryGetProperty("start", out var startValue) ||
                !root.TryGetProperty("epoch", out var epochValue) ||
                kind.ValueKind != JsonValueKind.String ||
                kind.GetString() != "load-page" ||
                !epochValue.TryGetUInt64(out var epoch) || epoch != Epoch ||
                !startValue.TryGetInt32(out start) ||
                start < 0 || start >= HistoryTotal || start % PageLimit != 0)
                return false;
            return true;
        }
        catch (JsonException) { return false; }
        catch (InvalidOperationException) { return false; }
    }

    internal static string MakePage(int start)
    {
        if (start < 0 || start >= HistoryTotal || start % PageLimit != 0)
            throw new ArgumentOutOfRangeException(nameof(start));
        var records = Enumerable.Range(start, Math.Min(PageLimit, HistoryTotal - start))
            .Select(i => new {
                position = i,
                role = i % 2 == 0 ? "user" : "assistant",
                // Synthetic fixture ONLY; never derive from real archives.
                text = "Synthetic history message " + i.ToString("D4")
            }).ToArray();
        var raw = JsonSerializer.Serialize(new {
            kind = "history-page", epoch = Epoch, start,
            total = HistoryTotal, entries = records
        });
        if (Encoding.UTF8.GetByteCount(raw) > MaximumResponseBytes)
            throw new InvalidOperationException("15HOST renderer page exceeds wire bound");
        return raw;
    }

    internal static void SelfTest()
    {
        static void Expect(bool condition)
        {
            if (!condition) throw new InvalidOperationException("15HOST contract regression");
        }
        Expect(IsValidRequest("{\"kind\":\"load-page\",\"start\":0,\"epoch\":1}", out var zero)
            && zero == 0);
        Expect(IsValidRequest("{\"kind\":\"load-page\",\"start\":5976,\"epoch\":1}",
            out var last) && last == 5976);
        foreach (var raw in new[] {
            "{}", "[]", "{\"kind\":\"load-page\",\"start\":-1,\"epoch\":1}",
            "{\"kind\":\"load-page\",\"start\":1,\"epoch\":1}",
            "{\"kind\":\"load-page\",\"start\":6000,\"epoch\":1}",
            "{\"kind\":\"load-page\",\"start\":0,\"epoch\":2}",
            "{\"kind\":\"load-page\",\"start\":0,\"epoch\":1,\"other\":true}",
            "{\"kind\":\"send\",\"start\":0,\"epoch\":1}",
            "{\"kind\":\"load-page\",\"start\":0,\"epoch\":1,\"start\":24}",
            new string('X', MaximumRequestBytes + 1)
        })
            Expect(!IsValidRequest(raw, out _));
        var first = MakePage(0);
        using var doc = JsonDocument.Parse(first);
        Expect(doc.RootElement.GetProperty("entries").GetArrayLength() == PageLimit);
        Expect(doc.RootElement.GetProperty("total").GetInt32() == HistoryTotal);
        Expect(Encoding.UTF8.GetByteCount(first) <= MaximumResponseBytes);
        var end = MakePage(5976);
        using var endDoc = JsonDocument.Parse(end);
        Expect(endDoc.RootElement.GetProperty("entries")[23]
            .GetProperty("position").GetInt32() == 5999);
        Expect(MakePage(0) == first);
        Expect(SHA256.HashData(Encoding.UTF8.GetBytes(first)).Length == 32);
    }
    internal static string SyntheticStateJson(string captureSurface)
    {
        var translations = new {
            home = "Home", conversations = "Conversations", projects = "Projects",
            files = "Files", agents = "Agents", skills = "Skills", plugins = "Plugins",
            settings = "Settings", activity = "Activity", search = "Search",
            newChat = "New chat", save = "Save", cancel = "Cancel", close = "Close",
            navigation = "Navigation", mainContent = "Main content",
            messageInput = "Message input", confirm = "Confirm"
        };
        return JsonSerializer.Serialize(new {
            kind = "state", mode = "synthetic", syntheticOnly = true, epoch = Epoch,
            captureSurface,
            navigation = new { hostEpoch = Epoch, revision = 1, route = new { view = "home" }, sidebarOpen = true },
            conversation = new { hostEpoch = Epoch, sessionGeneration = 1, revision = 1, sessionId = "synthetic-session", page = (object?)null, pending = (object?)null },
            panels = new {
                hostEpoch = Epoch, revision = 1, panel = "projects",
                catalog = new {
                    projects = Array.Empty<object>(), files = Array.Empty<object>(), agents = Array.Empty<object>(), extensions = Array.Empty<object>(),
                    preferences = new { theme = "dark", locale = "en", fontScalePercent = 100, reducedMotion = false, confirmToolActions = true }
                }
            },
            rich = new {
                hostEpoch = Epoch, revision = 1, activeKey = "scratch-editor",
                panels = new object[] {
                    new { kind = "editor", key = "scratch-editor", title = "Untitled editor", language = "text", text = "Synthetic editor preview only.", readOnly = true },
                    new { kind = "terminal", key = "terminal-preview", title = "Terminal output", output = "Output-only preview. No process is attached." },
                    new { kind = "pdf", key = "pdf-preview", title = "PDF preview", assetId = "synthetic-document", page = 1, pageCount = 1 },
                    new { kind = "math", key = "math-preview", title = "Math preview", source = "x^2 + y^2 = z^2" }
                }
            },
            focus = new { hostEpoch = Epoch, revision = 1, target = "navigation", dialogOpen = false },
            localization = new { tag = "en", texts = translations },
            availableLocales = new[] { "en", "de", "es", "fr", "pt-BR", "pt-PT", "ru", "tr", "vi", "ja", "ko", "zh-CN", "zh-TW" },
            sessions = new[] { new { id = "synthetic-session", title = "Synthetic fixture", updatedAt = 0 } },
            historyWindow = new { total = HistoryTotal, pageLimit = PageLimit },
            captureContract = new { version = 1, logicalWidth = 800, logicalHeight = 600, dpi = 96, theme = "dark", locale = "en", surface = "shell" },
            lastError = (string?)null
        });
    }
}
internal enum CaptureMode
{
    None,
    Candidate,
    Golden
}

internal sealed class RendererWindow : Form
{
    private readonly WebView2 _browser = new() { Dock = DockStyle.Fill };
    private readonly string _isolationRoot = Path.Combine(Path.GetTempPath(),
        "cosos-15host-" + Guid.NewGuid().ToString("N"));
    private readonly bool _capture;
    private readonly RendererIpcClient? _ipc;
    private readonly CaptureMode _captureMode;
    private readonly string _captureSurface;
    private readonly CancellationTokenSource _readerCancellation = new();
    private readonly TaskCompletionSource<NavigationOutcome> _navigation =
        new(TaskCreationOptions.RunContinuationsAsynchronously);
    private readonly TaskCompletionSource<bool> _documentReady =
        new(TaskCreationOptions.RunContinuationsAsynchronously);
    private readonly TaskCompletionSource<bool> _captureReady =
        new(TaskCreationOptions.RunContinuationsAsynchronously);
    private readonly string _documentNonce = Guid.NewGuid().ToString("N");
    private readonly List<object> _navigationEvents = [];
    private bool _ready;
    private bool _awaitingOwnedDocument;
    private ulong _expectedNavigationId;
    private ulong _observedNavigationId;
    private string _webErrorStatus = "not-observed";
    private string _phase = "initialization";

    internal RendererWindow(bool capture, RendererIpcClient? ipc, CaptureMode captureMode, string captureSurface)
    {
        _capture = capture;
        _ipc = ipc;
        _captureMode = captureMode;
        _captureSurface = captureSurface;
        Text = "CoSoS 15HOST — Native Renderer";
        ClientSize = capture ? new Size(800, 600) : new Size(1280, 820);
        FormBorderStyle = capture ? FormBorderStyle.None : FormBorderStyle.Sizable;
        StartPosition = FormStartPosition.CenterScreen;
        Controls.Add(_browser);

        Shown += async (_, _) =>
        {
            try { await InitializeAsync(); }
            catch (Exception error) { Fail(error); }
        };
        FormClosing += (_, _) =>
        {
            if (_ipc is not null && _ready)
            {
                try { _ipc.SendPayload("{\"kind\":\"renderer-closing\"}"); }
                catch (Exception) { Environment.ExitCode = 1; }
            }
            _readerCancellation.Cancel();
        };
        FormClosed += (_, _) =>
        {
            _ready = false;
            _browser.Dispose();
            _ipc?.Dispose();
            _readerCancellation.Dispose();
            try { if (Directory.Exists(_isolationRoot)) Directory.Delete(_isolationRoot, true); }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
        };
    }

    private async Task InitializeAsync()
    {
        _phase = "contract";
        RendererContracts.SelfTest();
        _phase = "isolated-user-data";
        if (Directory.Exists(_isolationRoot)) throw new InvalidOperationException("Renderer profile collision");
        Directory.CreateDirectory(_isolationRoot);

        _phase = "webview2-environment";
        var environment = await CoreWebView2Environment.CreateAsync(null, _isolationRoot);
        _phase = "webview2-controller";
        await _browser.EnsureCoreWebView2Async(environment);
        var core = _browser.CoreWebView2;
        core.Settings.AreDefaultContextMenusEnabled = false;
        core.Settings.AreDevToolsEnabled = false;
        core.Settings.IsStatusBarEnabled = false;
        core.Settings.IsWebMessageEnabled = true;
        core.PermissionRequested += (_, e) =>
        {
            e.State = CoreWebView2PermissionState.Deny;
            e.SavesInProfile = false;
            e.Handled = true;
        };
        core.NewWindowRequested += (_, e) => e.Handled = true;
        core.NavigationStarting += (_, e) =>
        {
            var owned = _awaitingOwnedDocument && e.Uri == "about:blank";
            RecordNavigation("starting", e.NavigationId, null, "pending", owned ? "about-blank" : "blocked");
            if (!owned) e.Cancel = true;
            else _expectedNavigationId = e.NavigationId;
            _ready = false;
        };
        core.NavigationCompleted += (_, e) =>
        {
            var status = e.WebErrorStatus.ToString();
            var expected = e.NavigationId == _expectedNavigationId && _expectedNavigationId != 0;
            RecordNavigation("completed", e.NavigationId, e.IsSuccess, status, expected ? "owned-document" : "unrelated");
            if (!expected) return;
            _observedNavigationId = e.NavigationId;
            _webErrorStatus = status;
            _navigation.TrySetResult(new NavigationOutcome(e.NavigationId, e.IsSuccess, status));
        };
        core.ProcessFailed += (_, _) =>
        {
            _ready = false;
            Environment.ExitCode = 1;
            try { BeginInvoke(new Action(Close)); } catch (InvalidOperationException) { }
        };
        core.WebMessageReceived += (_, e) => OnWebMessage(e);

        _phase = "navigate-owned-document";
        _awaitingOwnedDocument = true;
        var template = _captureMode == CaptureMode.Golden
            ? RendererDocument.LoadGoldenTemplate()
            : RendererDocument.Html;
        var html = RendererDocument.WithNonce(template, _documentNonce);
        _browser.NavigateToString(html);
        var navigation = await _navigation.Task.WaitAsync(TimeSpan.FromSeconds(30));
        _awaitingOwnedDocument = false;
        if (!navigation.IsSuccess)
            throw new InvalidOperationException("Owned WebView2 navigation failed: " + navigation.WebErrorStatus);
        if (!_documentReady.Task.IsCompleted)
            await _documentReady.Task.WaitAsync(TimeSpan.FromSeconds(15));
        _ready = true;

        if (_captureMode == CaptureMode.Golden)
        {
            // The independent reference document posts its own ready marker.
        }
        else if (_ipc is not null)
        {
            _phase = "authenticated-ipc-ready";
            _ipc.SendPayload("{\"kind\":\"ready\",\"rendererNonce\":\"" + _documentNonce + "\"}");
            _ = _ipc.ReadLoopAsync(OnHostPayload, OnIpcFailure, _readerCancellation.Token);
        }
        else
        {
            _phase = "synthetic-state";
            core.PostWebMessageAsJson(RendererContracts.SyntheticStateJson(_captureSurface));
        }

        if (_capture)
        {
            _phase = "native-capture-ready";
            await _captureReady.Task.WaitAsync(TimeSpan.FromSeconds(30));
            _phase = "native-webview2-capture";
            var coreView = _browser.CoreWebView2;
            var outputName = _captureMode == CaptureMode.Golden ? "15host-reference.png" : "15host-candidate.png";
            var output = Path.Combine(AppContext.BaseDirectory, outputName);
            await using (var stream = new FileStream(output, FileMode.Create, FileAccess.Write, FileShare.None))
            {
                await coreView.CapturePreviewAsync(CoreWebView2CapturePreviewImageFormat.Png, stream);
            }
            var png = await File.ReadAllBytesAsync(output);
            if (png.Length is <= 0 or > 16_777_216)
                throw new InvalidOperationException("Native preview is outside the image byte budget");
            var fingerprint = PixelFingerprint(output);
            if (fingerprint.Width != 800 || fingerprint.Height != 600)
                throw new InvalidOperationException("Native preview dimensions differ from 15F 800x600 contract");
            var referenceSha = RendererDocument.GoldenSourceSha256();
            var receipt = new {
                pass = true, phase = "native-capture-complete",
                pngSha256 = Convert.ToHexString(SHA256.HashData(png)).ToLowerInvariant(),
                pixelSha256 = fingerprint.Sha256, width = fingerprint.Width, height = fingerprint.Height,
                bytes = png.Length, surface = _captureSurface,
                syntheticOnly = true, rustAuthenticated = false, productAccepted = false,
                approvedGolden = false, referenceSourceSha256 = referenceSha,
                navigationId = _observedNavigationId, webErrorStatus = _webErrorStatus
            };
            File.WriteAllText(Path.Combine(AppContext.BaseDirectory, "15host-native-smoke.json"),
                JsonSerializer.Serialize(receipt));
            Close();
        }
    }

    private void OnWebMessage(CoreWebView2WebMessageReceivedEventArgs args)
    {
        if (args.Source != "about:blank") return;
        if (IsNonceMessage(args.WebMessageAsJson, "document-ready"))
        {
            _documentReady.TrySetResult(true);
            return;
        }
        if (IsNonceMessage(args.WebMessageAsJson, "capture-ready"))
        {
            _captureReady.TrySetResult(true);
            return;
        }
        if (!_ready) return;

        if (_ipc is not null)
        {
            if (!RendererDocument.TryValidateIntent(args.WebMessageAsJson, out var payload))
            {
                _browser.CoreWebView2.PostWebMessageAsJson("{\"kind\":\"error\",\"code\":\"invalid-renderer-intent\"}");
                return;
            }
            try { _ipc.SendPayload(payload); }
            catch (Exception error) { Fail(error); }
            return;
        }

        if (RendererContracts.IsValidRequest(args.WebMessageAsJson, out var start))
            _browser.CoreWebView2.PostWebMessageAsJson(RendererContracts.MakePage(start));
    }

    private bool IsNonceMessage(string raw, string expectedKind)
    {
        try
        {
            using var document = JsonDocument.Parse(raw);
            var root = document.RootElement;
            return root.ValueKind == JsonValueKind.Object &&
                root.EnumerateObject().Count() == 2 &&
                root.TryGetProperty("kind", out var kind) && kind.GetString() == expectedKind &&
                root.TryGetProperty("nonce", out var nonce) && nonce.GetString() == _documentNonce;
        }
        catch (JsonException) { return false; }
    }

    private void OnHostPayload(string payloadJson)
    {
        try
        {
            using var document = JsonDocument.Parse(payloadJson);
            if (!document.RootElement.TryGetProperty("kind", out var kind) || kind.GetString() != "state")
            {
                OnIpcFailure("invalid-host-state");
                return;
            }
            if (IsDisposed || !IsHandleCreated) return;
            BeginInvoke(new Action(() =>
            {
                if (!_ready || IsDisposed) return;
                _browser.CoreWebView2.PostWebMessageAsJson(payloadJson);
            }));
        }
        catch (Exception) { OnIpcFailure("invalid-host-state"); }
    }

    private void OnIpcFailure(string code)
    {
        if (_readerCancellation.IsCancellationRequested) return;
        Environment.ExitCode = 1;
        try
        {
            if (IsHandleCreated && !IsDisposed)
                BeginInvoke(new Action(Close));
        }
        catch (InvalidOperationException) { }
    }

    private void RecordNavigation(string phase, ulong navigationId, bool? success, string webErrorStatus, string uriClass)
    {
        _navigationEvents.Add(new { phase, navigationId, isSuccess = success, webErrorStatus, uriClass });
        if (_navigationEvents.Count > 12) _navigationEvents.RemoveAt(0);
    }

    private void Fail(Exception error)
    {
        Environment.ExitCode = 1;
        _ready = false;
        try
        {
            File.WriteAllText(Path.Combine(AppContext.BaseDirectory, "15host-native-smoke.json"),
                JsonSerializer.Serialize(new {
                    pass = false, phase = _phase, failure = error.GetType().Name,
                    hresult = error.HResult, navigationId = _observedNavigationId,
                    expectedNavigationId = _expectedNavigationId, webErrorStatus = _webErrorStatus,
                    navigationEvents = _navigationEvents.ToArray(),
                    syntheticOnly = _ipc is null, exitCode = 1
                }));
        }
        catch (IOException) { }
        catch (UnauthorizedAccessException) { }
        try { if (IsHandleCreated) BeginInvoke(new Action(Close)); else Close(); }
        catch (InvalidOperationException) { }
    }

    private static (int Width, int Height, string Sha256) PixelFingerprint(string path)
    {
        using var bitmap = new Bitmap(path);
        var width = bitmap.Width;
        var height = bitmap.Height;
        if (width <= 0 || height <= 0 || (long)width * height * 4 > 16_777_216)
            throw new InvalidOperationException("Native screenshot pixel budget exceeded");
        var rect = new Rectangle(0, 0, width, height);
        var data = bitmap.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        try
        {
            var rgba = new byte[checked(width * height * 4)];
            var row = new byte[checked(width * 4)];
            for (var y = 0; y < height; y++)
            {
                Marshal.Copy(IntPtr.Add(data.Scan0, y * data.Stride), row, 0, row.Length);
                for (var x = 0; x < width; x++)
                {
                    var source = x * 4;
                    var target = (y * width + x) * 4;
                    rgba[target] = row[source + 2];
                    rgba[target + 1] = row[source + 1];
                    rgba[target + 2] = row[source];
                    rgba[target + 3] = row[source + 3];
                    if (rgba[target + 3] != 255)
                        throw new InvalidOperationException("Native screenshot contains non-opaque pixels");
                }
            }
            return (width, height, Convert.ToHexString(SHA256.HashData(rgba)).ToLowerInvariant());
        }
        finally { bitmap.UnlockBits(data); }
    }

    private readonly record struct NavigationOutcome(ulong NavigationId, bool IsSuccess, string WebErrorStatus);
}

internal static class RendererDocument
{
    private static readonly string[] AllowedLocales = ["en", "de", "es", "fr", "pt-BR", "pt-PT", "ru", "tr", "vi", "ja", "ko", "zh-CN", "zh-TW"];

    internal const string Html = """
<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-__DOCUMENT_NONCE__'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; connect-src 'none'; frame-src 'none'; img-src 'none'; object-src 'none'; worker-src 'none'">
<meta name="viewport" content="width=device-width,initial-scale=1">
<style>
:root{font:13px system-ui,sans-serif;color:#e6edf7;background:#10151c;color-scheme:dark}
*{box-sizing:border-box}body{margin:0;overflow:hidden}
header{height:54px;display:flex;align-items:center;gap:12px;padding:0 14px;border-bottom:1px solid #303947;background:#121923}
.brand{font-size:14px;font-weight:700;letter-spacing:.2px}.brand span{color:#94a3b8;font-weight:500}
.badge{font-size:10px;color:#a7f3d0;border:1px solid #245c4a;border-radius:4px;padding:4px 6px}
header .spacer{flex:1}select,button,textarea{font:inherit;color:inherit;background:#151e2a;border:1px solid #344256;border-radius:4px}
select{height:28px;padding:0 6px;max-width:84px}button{cursor:pointer;padding:6px 8px}button:hover{background:#202c3b}
button.active{background:#26364b;border-color:#526984}button:focus-visible,select:focus-visible,textarea:focus-visible{outline:2px solid #8ab4f8;outline-offset:1px}
.layout{display:grid;grid-template-columns:160px minmax(0,1fr) 280px;height:calc(100vh - 54px);min-height:0}
nav{padding:12px 9px;border-right:1px solid #303947;overflow:hidden;min-width:0}
.nav-title,.section-title{font-size:11px;text-transform:uppercase;letter-spacing:.7px;color:#8796aa;margin:5px 6px 9px}
#navItems{display:grid;gap:4px}#navItems button{display:block;width:100%;text-align:left;border-color:transparent;background:transparent;padding:7px 8px}
#navItems button.active{background:#202d3d;border-color:#344256}
#sessionList{display:grid;gap:4px;margin-top:6px}.session{width:100%;text-align:left;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.muted{color:#8796aa;font-size:11px}.section-title{margin-top:18px}
main{display:flex;flex-direction:column;min-width:0;min-height:0}
.toolbar{height:40px;flex:0 0 40px;display:flex;align-items:center;gap:8px;padding:0 10px;border-bottom:1px solid #263140}
.toolbar strong{font-size:12px}.toolbar .spacer{flex:1}.toolbar button{font-size:10px;padding:4px 6px}
#historyViewport{flex:1;min-height:0;overflow:auto;position:relative;contain:strict}
#historyCanvas{position:relative;height:0}
.message{position:absolute;left:8px;right:8px;height:60px;border-bottom:1px solid #243040;padding:9px 8px;overflow:hidden}
.role{display:block;font-size:9px;text-transform:uppercase;letter-spacing:.5px;color:#8292a7}
.message p{margin:4px 0 0;white-space:pre-wrap;overflow-wrap:anywhere;font-size:12px;line-height:16px}
section{min-height:0}.surface{padding:10px 0;border-bottom:1px solid #303947}
aside{padding:12px 10px;border-left:1px solid #303947;min-width:0;overflow:hidden;display:flex;flex-direction:column;gap:9px}
.panel-heading{display:flex;align-items:center;justify-content:space-between}
h2{font-size:12px;margin:0;color:#b0becf}.panel-tabs{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:3px}
.panel-tabs button{font-size:10px;padding:6px 2px;min-width:0}
#richSurface{flex:1;min-height:0;overflow:hidden;border:1px solid #303947;border-radius:5px;padding:8px;display:flex;flex-direction:column;gap:8px}
.panel-title{font-size:11px;color:#b0becf;margin:0}.panel-note{font-size:10px;line-height:15px;color:#8796aa}
.editor{width:100%;height:100%;min-height:150px;resize:none;padding:8px;border-color:#344256;background:#0c1118;font:11px ui-monospace,Consolas,monospace;line-height:16px}
.output{white-space:pre-wrap;overflow-wrap:anywhere;overflow:auto;font:11px ui-monospace,Consolas,monospace;color:#cbd5e1;margin:0}
.pdf-page{flex:1;display:grid;place-items:center;border:1px dashed #435064;background:#121923;color:#b0becf;text-align:center;padding:12px}
.math-source{font:11px ui-monospace,Consolas,monospace;padding:7px;min-height:54px;resize:vertical}
.math-preview{white-space:pre-wrap;overflow-wrap:anywhere;font-size:14px;padding:12px;border:1px solid #344256;border-radius:4px}
.panel-footer{font-size:10px;color:#7f8ea3;line-height:14px}
dialog{background:#151e2a;color:#e6edf7;border:1px solid #526984;border-radius:6px;max-width:300px}
dialog::backdrop{background:#0009}dialog button{margin-right:6px}
</style></head><body>
<header><div class="brand">CoSoS <span>/ 15HOST</span></div><div class="badge" id="modeBadge">Native UI</div><div class="spacer"></div><label class="muted" for="locale">Locale</label><select id="locale" aria-label="Locale"></select></header>
<div class="layout">
<nav id="navigation" tabindex="0" aria-label="Navigation"><div class="nav-title" id="navigationLabel">Navigation</div><div id="navItems"></div><div class="section-title" id="conversationsLabel">Conversations</div><div id="sessionList"></div></nav>
<main id="main" tabindex="0" aria-label="Main content"><div class="toolbar"><strong id="routeTitle">Home</strong><span class="muted" id="historyCount">6,000 messages</span><div class="spacer"></div><button id="openDialog">Confirm…</button></div><div id="historyViewport"><div id="historyCanvas"></div></div></main>
<aside id="panelRoot"><div class="panel-heading"><h2 id="panelsLabel">Panels</h2><span class="muted">host-owned</span></div><div id="richTabs" class="panel-tabs"></div><div id="richSurface"></div><div class="panel-footer">Terminal is output-only. Editor changes remain in memory; PDF assets are host-owned.</div></aside>
</div>
<dialog id="confirmDialog"><p id="dialogText">Confirm this presentation-only action?</p><button id="dialog-confirm">Confirm</button><button id="dialog-cancel">Cancel</button></dialog>
<script nonce="__DOCUMENT_NONCE__">
(()=>{
'use strict';
const NONCE='__DOCUMENT_NONCE__',ROW_HEIGHT=60,PAGE_SIZE=24;
const LOCALES=['en','de','es','fr','pt-BR','pt-PT','ru','tr','vi','ja','ko','zh-CN','zh-TW'];
const NAV=[['home','home'],['projects','projects'],['agents','agents'],['skills','skills'],['plugins','plugins'],['settings','settings'],['activity','activity']];
const $=id=>document.getElementById(id),viewport=$('historyViewport'),canvas=$('historyCanvas');
let state=null,total=0,generation=-1,pendingPage=false,captureReadySent=false;
let pages=new Map(),requested=new Set(),activePanelKey=null;
const navItems=$('navItems'),sessionList=$('sessionList'),tabs=$('richTabs'),surface=$('richSurface'),localeSelect=$('locale'),dialog=$('confirmDialog');
function send(intent){if(state&&state.mode==='authenticated')chrome.webview.postMessage(intent);}
function sendPage(start){
 if(!state||start<0||start>=total||start%PAGE_SIZE!==0)return;
 if(state.mode==='synthetic'){if(!requested.has(start)){requested.add(start);chrome.webview.postMessage({kind:'load-page',start,epoch:1});}}
 else if(!pendingPage&&!pages.has(start)){pendingPage=true;chrome.webview.postMessage({kind:'load-page',start,expectedRevision:state.conversation.revision});}
}
function addPage(page){
 if(!page||!Array.isArray(page.entries)||page.entries.length>PAGE_SIZE)return;
 if(!Number.isInteger(page.start)||page.start<0||page.start%PAGE_SIZE!==0||page.start>=Math.max(1,total))return;
 if(page.entries.some((item,i)=>item.position!==page.start+i||typeof item.text!=='string'||item.text.length>2048))return;
 pages.delete(page.start);pages.set(page.start,page.entries);
 while(pages.size>8)pages.delete(pages.keys().next().value);
}
function labels(){return state?.localization?.texts||{};}
function applyLocale(){
 const text=labels(),tag=state?.localization?.tag||'en';
 document.documentElement.lang=tag;
 $('navigationLabel').textContent=text.navigation||'Navigation';
 $('conversationsLabel').textContent=text.conversations||'Conversations';
 $('panelsLabel').textContent='Panels';
 $('openDialog').textContent=(text.confirm||'Confirm')+'…';
 $('modeBadge').textContent=state?.mode==='synthetic'?'SYNTHETIC FIXTURE':'AUTHENTICATED';
 localeSelect.replaceChildren();
 for(const item of LOCALES){const option=document.createElement('option');option.value=item;option.textContent=item;localeSelect.append(option);}
 localeSelect.value=LOCALES.includes(tag)?tag:'en';
}
function renderNav(){
 const text=labels(),route=state?.navigation?.route?.view||'home';
 navItems.replaceChildren();
 for(const [view,key] of NAV){
  const button=document.createElement('button');button.type='button';button.textContent=text[key]||key;
  if(route===view)button.classList.add('active');
  button.addEventListener('click',()=>send({kind:'navigate',route:{view},expectedRevision:state?.navigation?.revision||1}));
  navItems.append(button);
 }
}
function renderSessions(){
 sessionList.replaceChildren();
 const sessions=state?.sessions||[];
 if(state?.mode==='synthetic'){
  const button=document.createElement('button');button.type='button';button.className='session active';button.textContent='Synthetic fixture';button.disabled=true;sessionList.append(button);return;
 }
 if(!sessions.length){const empty=document.createElement('div');empty.className='muted';empty.textContent='No conversations in this profile';sessionList.append(empty);return;}
 for(const session of sessions){
  const button=document.createElement('button');button.type='button';button.className='session';
  button.textContent=String(session.title||'Conversation').slice(0,128);
  if(session.id===state?.conversation?.sessionId)button.classList.add('active');
  button.addEventListener('click',()=>send({kind:'select-session',sessionId:session.id,expectedRevision:state.navigation.revision}));
  sessionList.append(button);
 }
}
function renderHistory(){
 canvas.style.height=(total*ROW_HEIGHT)+'px';
 $('historyCount').textContent=state?.mode==='synthetic'?'6,000 synthetic messages':(total+' messages in bounded history window');
 const top=viewport.scrollTop,begin=Math.max(0,Math.floor(top/ROW_HEIGHT)-4);
 const end=Math.min(total,Math.ceil((top+viewport.clientHeight)/ROW_HEIGHT)+4);
 const fragment=document.createDocumentFragment();
 for(let index=begin;index<end;index++){
  const pageStart=Math.floor(index/PAGE_SIZE)*PAGE_SIZE,page=pages.get(pageStart);
  if(!page){sendPage(pageStart);continue;}
  const item=page[index-pageStart];if(!item)continue;
  const role=String(item.role||'activity').toLowerCase();
  const row=document.createElement('article');row.className='message '+role;row.style.top=(index*ROW_HEIGHT)+'px';
  const roleNode=document.createElement('span');roleNode.className='role';roleNode.textContent=role;
  const textNode=document.createElement('p');textNode.textContent=String(item.text||'').slice(0,2048);
  row.append(roleNode,textNode);fragment.append(row);
 }
 canvas.replaceChildren(fragment);
 if(state?.mode==='synthetic'&&pages.size>0&&!captureReadySent){
  captureReadySent=true;requestAnimationFrame(()=>requestAnimationFrame(()=>chrome.webview.postMessage({kind:'capture-ready',nonce:NONCE})));
 }
}
function renderRich(){
 const rich=state?.rich||{},panels=rich.panels||[];
 tabs.replaceChildren();surface.replaceChildren();
 const names={editor:'Editor',terminal:'Terminal',pdf:'PDF',math:'Math',code:'Code'};
 for(const panel of panels){
  const button=document.createElement('button');button.type='button';button.textContent=names[panel.kind]||'Panel';
  if((activePanelKey||rich.activeKey)===panel.key)button.classList.add('active');
  button.addEventListener('click',()=>{
   if(state.mode==='synthetic'){activePanelKey=panel.key;renderRich();}
   else send({kind:'rich-command',command:{action:'activate',key:panel.key,expectedRevision:rich.revision}});
  });
  tabs.append(button);
 }
 if(!panels.length){
  const note=document.createElement('p');note.className='panel-note';note.textContent='Open a host-owned presentation surface.';
  surface.append(note);
  for(const kind of ['editor','terminal','pdf','math']){
   const button=document.createElement('button');button.type='button';button.textContent='Open '+names[kind];
   button.addEventListener('click',()=>send({kind:'open-rich',surface:kind}));surface.append(button);
  }
  return;
 }
 let panel=panels.find(item=>item.key===(activePanelKey||rich.activeKey))||panels[0];
 if(state.mode==='synthetic'&&state.captureSurface&&state.captureSurface!=='shell'&&state.captureSurface!=='history'){
  panel=panels.find(item=>item.kind===state.captureSurface)||panel;
 }
 const title=document.createElement('p');title.className='panel-title';title.textContent=String(panel.title||names[panel.kind]||'Panel');surface.append(title);
 if(panel.kind==='editor'){
  const editor=document.createElement('textarea');editor.className='editor';editor.spellcheck=false;editor.setAttribute('aria-label','Editor');
  editor.value=String(panel.text||'');editor.readOnly=Boolean(panel.readOnly);
  editor.addEventListener('change',()=>send({kind:'rich-command',command:{action:'replace-editor',key:panel.key,expectedRevision:rich.revision,text:editor.value.slice(0,8192)}}));
  surface.append(editor);
 }else if(panel.kind==='terminal'||panel.kind==='code'){
  const output=document.createElement('pre');output.className='output';output.textContent=String(panel.output||panel.text||'');surface.append(output);
 }else if(panel.kind==='pdf'){
  const page=document.createElement('div');page.className='pdf-page';page.textContent='PDF preview · page '+panel.page+' of '+panel.pageCount+' · opaque host asset';surface.append(page);
  const controls=document.createElement('div');
  for(const [label,value] of [['Previous',Math.max(1,panel.page-1)],['Next',Math.min(panel.pageCount,panel.page+1)]]){
   const button=document.createElement('button');button.type='button';button.textContent=label;button.disabled=value===panel.page;
   button.addEventListener('click',()=>send({kind:'rich-command',command:{action:'set-pdf-page',key:panel.key,expectedRevision:rich.revision,page:value}}));controls.append(button);
  }
  surface.append(controls);
 }else if(panel.kind==='math'){
  const editor=document.createElement('textarea');editor.className='math-source';editor.value=String(panel.source||'');editor.setAttribute('aria-label','Math source');
  editor.addEventListener('change',()=>send({kind:'rich-command',command:{action:'replace-math',key:panel.key,expectedRevision:rich.revision,source:editor.value.slice(0,8192)}}));
  const preview=document.createElement('pre');preview.className='math-preview';preview.textContent=String(panel.source||'');surface.append(editor,preview);
 }
}
function applyFocus(){
 const focus=state?.focus;if(!focus)return;
 const targets={navigation:$('navigation'),main:$('main'),composer:surface.querySelector('textarea'),dialogConfirm:$('dialog-confirm'),dialogCancel:$('dialog-cancel')};
 if(focus.dialogOpen&&!dialog.open){try{dialog.showModal();}catch{}}
 if(!focus.dialogOpen&&dialog.open)dialog.close();
 const target=targets[focus.target];if(target&&document.activeElement!==target){try{target.focus({preventScroll:true});}catch{}}
}
function renderAll(){
 if(!state)return;
 applyLocale();renderNav();renderSessions();renderRich();renderHistory();applyFocus();
}
function receive(data){
 if(!data||typeof data!=='object')return;
 if(data.kind==='state'){
  const nextGeneration=data.conversation?.sessionGeneration??0;
  const changed=nextGeneration!==generation;
  state=data;total=Number.isSafeInteger(data.historyWindow?.total)?data.historyWindow.total:0;
  if(changed){pages.clear();requested.clear();pendingPage=false;generation=nextGeneration;}
  if(data.mode==='authenticated'&&data.conversation?.page){
   addPage(data.conversation.page);pendingPage=false;
   if(changed)viewport.scrollTop=Math.max(0,total*ROW_HEIGHT-viewport.clientHeight);
  }
  renderAll();return;
 }
 if(data.kind==='history-page'&&state?.mode==='synthetic'&&data.epoch===1&&data.total===total&&
  Number.isInteger(data.start)&&data.start>=0&&data.start<total&&data.start%PAGE_SIZE===0&&
  Array.isArray(data.entries)&&data.entries.length<=PAGE_SIZE){
  addPage(data);requested.delete(data.start);renderAll();return;
 }
 if(data.kind==='error'){$('historyCount').textContent=String(data.code||'UI request rejected');}
}
chrome.webview.addEventListener('message',event=>receive(event.data));
localeSelect.addEventListener('change',()=>send({kind:'set-locale',tag:localeSelect.value}));
$('openDialog').addEventListener('click',()=>send({kind:'dialog-action',action:'open'}));
$('dialog-confirm').addEventListener('click',()=>send({kind:'dialog-action',action:'confirm'}));
$('dialog-cancel').addEventListener('click',()=>send({kind:'dialog-action',action:'cancel'}));
document.addEventListener('keydown',event=>{
 if(event.isComposing)return;
 if(event.key==='Escape'){event.preventDefault();send({kind:'focus-event',event:{key:'escape',composing:false}});return;}
 if(event.key==='Tab'&&!event.ctrlKey&&!event.altKey&&!event.metaKey){
  event.preventDefault();send({kind:'focus-event',event:{key:'tab',shift:event.shiftKey,ctrl:false,alt:false,meta:false,composing:false}});
 }
});
viewport.addEventListener('scroll',renderHistory,{passive:true});window.addEventListener('resize',renderHistory);
chrome.webview.postMessage({kind:'document-ready',nonce:NONCE});
})();
</script></body></html>
""";

    internal static string WithNonce(string template, string nonce) =>
        template.Replace("__DOCUMENT_NONCE__", nonce, StringComparison.Ordinal);

    internal static string LoadGoldenTemplate() =>
        File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "goldens", "shell-reference.html"));

    internal static string GoldenSourceSha256() =>
        Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(LoadGoldenTemplate()))).ToLowerInvariant();

    internal static bool TryValidateIntent(string raw, out string payload)
    {
        payload = string.Empty;
        if (Encoding.UTF8.GetByteCount(raw) > 16 * 1024) return false;
        try
        {
            using var document = JsonDocument.Parse(raw);
            var root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object ||
                !root.TryGetProperty("kind", out var kindValue) ||
                kindValue.ValueKind != JsonValueKind.String) return false;
            var kind = kindValue.GetString();
            var valid = kind switch
            {
                "navigate" => ValidateNavigate(root),
                "select-session" => root.EnumerateObject().Count() == 3 &&
                    HasExpectedRevision(root) && root.TryGetProperty("sessionId", out var session) &&
                    session.ValueKind == JsonValueKind.String && ValidSessionId(session.GetString() ?? ""),
                "load-page" => root.EnumerateObject().Count() == 3 && HasExpectedRevision(root) &&
                    root.TryGetProperty("start", out var startValue) && startValue.TryGetInt32(out var start) &&
                    start >= 0 && start < RendererContracts.HistoryTotal && start % RendererContracts.PageLimit == 0,
                "rich-command" => root.EnumerateObject().Count() == 2 && ValidateCommand(root, ["snapshot", "activate", "replace-editor", "replace-math", "set-pdf-page"]),
                "panel-command" => root.EnumerateObject().Count() == 2 && ValidateCommand(root, ["snapshot", "open-panel", "select-project", "select-file", "select-agent", "select-extension", "set-settings-tab", "set-preferences"]),
                "focus-event" => root.EnumerateObject().Count() == 2 && ValidateFocus(root),
                "set-locale" => root.EnumerateObject().Count() == 2 && root.TryGetProperty("tag", out var tag) &&
                    tag.ValueKind == JsonValueKind.String && AllowedLocales.Contains(tag.GetString() ?? ""),
                "open-rich" => root.EnumerateObject().Count() == 2 && root.TryGetProperty("surface", out var surface) &&
                    surface.ValueKind == JsonValueKind.String && new[] { "editor", "terminal", "pdf", "math" }.Contains(surface.GetString() ?? ""),
                "dialog-action" => root.EnumerateObject().Count() == 2 && root.TryGetProperty("action", out var action) &&
                    action.ValueKind == JsonValueKind.String && new[] { "open", "confirm", "cancel" }.Contains(action.GetString() ?? ""),
                _ => false
            };
            if (!valid) return false;
            payload = "{\"kind\":\"ui-intent\",\"intent\":" + raw + "}";
            return Encoding.UTF8.GetByteCount(payload) <= RendererIpcClient.MaximumPayloadBytes;
        }
        catch (JsonException) { return false; }
        catch (InvalidOperationException) { return false; }
    }

    private static bool ValidateNavigate(JsonElement root)
    {
        if (root.EnumerateObject().Count() != 3 || !HasExpectedRevision(root) ||
            !root.TryGetProperty("route", out var route) || route.ValueKind != JsonValueKind.Object ||
            !route.TryGetProperty("view", out var viewValue) || viewValue.ValueKind != JsonValueKind.String)
            return false;
        var view = viewValue.GetString();
        if (view is "project" or "files")
            return route.EnumerateObject().Count() == 2 && route.TryGetProperty("projectId", out var project) &&
                project.ValueKind == JsonValueKind.String && ValidOpaqueKey(project.GetString() ?? "");
        return route.EnumerateObject().Count() == 1 &&
            new[] { "home", "projects", "agents", "skills", "plugins", "settings", "activity" }.Contains(view ?? "");
    }

    private static bool ValidateCommand(JsonElement root, string[] actions)
    {
        if (!root.TryGetProperty("command", out var command) || command.ValueKind != JsonValueKind.Object ||
            command.EnumerateObject().Count() > 8 || !command.TryGetProperty("action", out var action) ||
            action.ValueKind != JsonValueKind.String || !actions.Contains(action.GetString() ?? "")) return false;
        foreach (var property in command.EnumerateObject())
        {
            if (property.Value.ValueKind == JsonValueKind.String && (property.Value.GetString()?.Length ?? 0) > 8192) return false;
        }
        return true;
    }

    private static bool ValidateFocus(JsonElement root)
    {
        if (!root.TryGetProperty("event", out var frame) || frame.ValueKind != JsonValueKind.Object ||
            !frame.TryGetProperty("key", out var key) || key.ValueKind != JsonValueKind.String) return false;
        var name = key.GetString();
        if (name == "escape")
            return frame.EnumerateObject().Count() == 2 && frame.TryGetProperty("composing", out var composing) &&
                composing.ValueKind is JsonValueKind.True or JsonValueKind.False;
        return name == "tab" && frame.EnumerateObject().Count() == 6 &&
            new[] { "shift", "ctrl", "alt", "meta", "composing" }.All(field =>
                frame.TryGetProperty(field, out var value) && value.ValueKind is JsonValueKind.True or JsonValueKind.False);
    }

    private static bool HasExpectedRevision(JsonElement root) =>
        root.TryGetProperty("expectedRevision", out var value) && value.TryGetUInt64(out var revision) &&
        revision > 0 && revision <= RendererIpcClient.MaximumSafeInteger;

    private static bool ValidSessionId(string value) =>
        value.Length is >= 8 and <= 64 && value.All(character =>
            char.IsAsciiLetterOrDigit(character) || character == '-');

    private static bool ValidOpaqueKey(string value) =>
        value.Length is > 0 and <= 128 && !value.StartsWith('.') && !value.EndsWith('.') &&
        !value.Contains("..", StringComparison.Ordinal) && value.All(character =>
            char.IsAsciiLetterOrDigit(character) || character is '-' or '_' or '.' or ':');
}
