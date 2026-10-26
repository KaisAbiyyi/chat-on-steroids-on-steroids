using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace CoSoS.WindowsRenderer;

// 15HOST cloud foundation: native WebView2 running a bounded, READ-ONLY,
// synthetic 6k-history view. Not a Rust-authorized production UI yet;
// no real user profile, credential, Send, tool or file access is exposed.
internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        if (args.SequenceEqual(["--contract-selftest"]))
        {
            RendererContracts.SelfTest();
            Console.WriteLine("15HOST synthetic contract selftest PASS");
            return 0;
        }
        if (!args.SequenceEqual(["--synthetic-native"]))
        {
            Console.Error.WriteLine("Usage: WindowsRenderer --contract-selftest | --synthetic-native");
            return 2;
        }
        ApplicationConfiguration.Initialize();
        Application.Run(new RendererWindow());
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
}

internal sealed class RendererWindow : Form
{
    private readonly WebView2 _browser = new() { Dock = DockStyle.Fill };
    private readonly string _isolationRoot = Path.Combine(Path.GetTempPath(),
        "cosos-15host-synthetic-" + Guid.NewGuid().ToString("N"));
    private bool _ready;

    internal RendererWindow()
    {
        Text = "CoSoS 15HOST — Synthetic Native Renderer (NOT PRODUCT ACCEPTED)";
        Width = 1280;
        Height = 820;
        StartPosition = FormStartPosition.CenterScreen;
        Controls.Add(_browser);
        Shown += async (_, _) =>
        {
            try { await InitializeAsync(); }
            catch (Exception ex)
            {
                Environment.ExitCode = 1;
                Console.Error.WriteLine("15HOST native synthetic initialization failed: " +
                    ex.GetType().Name);
                Close();
            }
        };
        FormClosed += (_, _) => _browser.Dispose();
    }

    private async Task InitializeAsync()
    {
        RendererContracts.SelfTest();
        // Always a freshly generated synthetic WebView2 user-data directory.
        if (Directory.Exists(_isolationRoot)) throw new IOException("Profile collision");
        Directory.CreateDirectory(_isolationRoot);
        var env = await CoreWebView2Environment.CreateAsync(null, _isolationRoot);
        await _browser.EnsureCoreWebView2Async(env);
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
            if (e.Uri != "about:blank") e.Cancel = true;
            _ready = false;
        };
        core.ProcessFailed += (_, _) => { _ready = false; Environment.ExitCode = 1; };
        core.WebMessageReceived += (_, e) =>
        {
            // Only this isolated HTML document may ask for a synthetic page;
            // requests are never interpreted as host commands or permissions.
            if (!_ready || e.Source != "about:blank" ||
                !RendererContracts.IsValidRequest(e.WebMessageAsJson, out var start))
                return;
            core.PostWebMessageAsJson(RendererContracts.MakePage(start));
        };
        var navigation = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        core.NavigationCompleted += (_, e) => navigation.TrySetResult(e.IsSuccess);
        _browser.NavigateToString(RendererDocument.Html);
        if (!await navigation.Task.WaitAsync(TimeSpan.FromSeconds(30)))
            throw new IOException("15HOST local renderer navigation rejected");
        _ready = true;
        core.PostWebMessageAsJson(RendererContracts.MakePage(0));
    }
}

internal static class RendererDocument
{
    // Content has no network, iframe, external code, links or arbitrary HTML.
    // User-supplied text must always use textContent, NEVER innerHTML/eval.
    internal const string Html = """
<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy"
 content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; connect-src 'none'; frame-src 'none'; img-src 'none'">
<meta name="viewport" content="width=device-width,initial-scale=1">
<style>
:root{font:14px system-ui,sans-serif;color:#e6edf7;background:#10151c}
*{box-sizing:border-box} body{margin:0}
header{height:54px;border-bottom:1px solid #303947;padding:15px 20px;font-weight:650}
main{display:grid;grid-template-columns:180px minmax(0,1fr) 320px;height:calc(100vh - 54px)}
nav,aside{padding:18px;border-right:1px solid #303947}
aside{border-left:1px solid #303947;border-right:0}
h2{font-size:13px;color:#b0becf} small{color:#8796aa}
#scroll{height:100%;overflow-y:auto;position:relative;contain:strict}
#canvas{position:relative;height:360000px}
.message{position:absolute;left:12px;right:12px;height:55px;border-bottom:1px solid #243040;padding:10px;overflow:hidden}
.role{font-size:10px;text-transform:uppercase;color:#8292a7}
.message p{margin:4px 0;white-space:pre-wrap;overflow-wrap:anywhere}
section{min-height:0} .surface{padding:10px 0;border-bottom:1px solid #303947}
</style></head><body><header>CoSoS / 15HOST — source-bound synthetic UI prototype</header>
<main><nav><h2>History</h2><small>6,000 synthetic messages</small><p>Read-only fixture</p></nav>
<section><div id="scroll"><div id="canvas"></div></div></section>
<aside><h2>Panels</h2><div class="surface">Editor · preview only</div>
<div class="surface">Terminal · preview only</div><div class="surface">PDF / Math · preview only</div>
<small>Native Rust authenticated IPC and independent goldens are NOT_RUN.</small>
</aside></main>
<script>
'use strict';
const ROW_HEIGHT=60, PAGE_SIZE=24, TOTAL=6000;
const viewport=document.getElementById('scroll'),canvas=document.getElementById('canvas');
canvas.style.height=(TOTAL*ROW_HEIGHT)+'px';
const pages=new Map();let requested=new Set();
function request(start){if(requested.has(start))return;requested.add(start);
  chrome.webview.postMessage({kind:'load-page',start,epoch:1});}
function render(){const top=viewport.scrollTop;
 const begin=Math.max(0,Math.floor(top/ROW_HEIGHT)-4);
 const end=Math.min(TOTAL,Math.ceil((top+viewport.clientHeight)/ROW_HEIGHT)+4);
 const fragment=document.createDocumentFragment();
 for(let i=begin;i<end;i++){
  const start=Math.floor(i/PAGE_SIZE)*PAGE_SIZE;let page=pages.get(start);
  if(!page){request(start);continue;}const item=page[i-start];
  if(!item)continue;
  const el=document.createElement('div');el.className='message';
  el.style.top=(i*ROW_HEIGHT)+'px';
  const role=document.createElement('span');role.className='role';role.textContent=item.role;
  const text=document.createElement('p');text.textContent=item.text;
  el.append(role,text);fragment.append(el);
 }
 canvas.replaceChildren(fragment);
}
chrome.webview.addEventListener('message',event=>{
 const data=event.data;if(!data||data.kind!=='history-page'||data.epoch!==1||
  data.total!==TOTAL||!Number.isInteger(data.start)||
  data.start<0||data.start>=TOTAL||data.start%PAGE_SIZE!==0||
  !Array.isArray(data.entries)||data.entries.length>PAGE_SIZE)return;
 if(data.entries.some((item,i)=>item.position!==data.start+i||
  !['assistant','user'].includes(item.role)||typeof item.text!=='string'||
  item.text.length>2048))return;
 pages.set(data.start,data.entries);if(pages.size>8)pages.delete(pages.keys().next().value);
 requested.delete(data.start);render();
});
viewport.addEventListener('scroll',render,{passive:true});
window.addEventListener('resize',render);render();
</script></body></html>
""";
}
