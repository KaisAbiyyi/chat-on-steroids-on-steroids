using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace CoSoS.Probe7L1.WindowsHost;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        // Windows CI can exercise fail-closed proof logic without creating
        // WebView2 profiles, attaching debuggers or claiming physical acceptance.
        if (args.Length == 1 && args[0] == "--mv3-retirement-self-test")
        {
            NativeRetirementProof.RunSelfTests();
            return;
        }
#if COS_8L1_PRODUCT
        if (args.Length == 1 && args[0] == "--8l1-authority-self-test")
        {
            GateForm.Run8L1PolicyTests();
            return;
        }
#endif
        ApplicationConfiguration.Initialize();
        Application.Run(new GateForm(args));
    }
}

internal sealed partial class GateForm : Form
{
    private const string Origin = "https://synthetic.cos.test";
    private const string StartUrl = Origin + "/index.html";
    private readonly WebView2 _a = new() { Dock = DockStyle.Fill };
    private readonly WebView2 _sameProfileTab = new() { Dock = DockStyle.Fill };
    private readonly WebView2 _b = new() { Dock = DockStyle.Fill };
    private readonly SortedDictionary<string, object> _checks = new();
    private readonly string _root, _repo, _bridgePath;
    private readonly string? _artifactRoot;
    private readonly string? _retained8L1ProfileRoot;
    private readonly bool _reuse8L1Profile;
    private readonly bool _idle, _cdpPreflight, _l2, _host8L1, _host8L1CrashProbe, _host8L1ConsentProbe;
    private readonly Dictionary<WebView2, Browser> _browsers = new(ReferenceEqualityComparer.Instance);
    private readonly SemaphoreSlim _rustIo = new(1, 1);
    private Process? _bridge;
    private bool _failed;
    private string? _verifiedSourceSha;

#if COS_8L1_PRODUCT
    internal static void Run8L1PolicyTests() => Native8L1Authority.RunSelfTests();
#endif

    private sealed class Browser
    {
        public required WebView2 View { get; init; }
        public required string Profile { get; init; }
        public required string ProfilePath { get; init; }
        public required string ExtensionId { get; init; }
        public required uint TabId { get; init; }
        public string DocumentId { get; set; } = "";
        public ulong NavigationEpoch { get; set; }
        public ulong WorkerGeneration { get; set; }
        public ulong HostGeneration { get; set; }
        public string? WorkerTargetId { get; set; }
        public string? WorkerSession { get; set; }
        public string? WorkerInstance { get; set; }
        public ulong ChannelId { get; set; }
        public string SessionId { get; set; } = "";
        public ulong BoundNavigation { get; set; }
        public bool Revoked { get; set; }
        public bool WorkerDestroyedEventObserved { get; set; }
        public int InFlightLedgerWrites { get; set; }
        public int RefusedOrigins { get; set; }
        public bool RetireScheduled { get; set; }
        public Task<bool>? RetirementTask { get; set; }
    }

    public GateForm(string[] args)
    {
        var options = new Dictionary<string, string>(StringComparer.Ordinal);
        for (var i = 0; i < args.Length; i++)
        {
            if (args[i] is "--native-idle" or "--cdp-preflight" or "--7l2" or "--8l1" or "--8l1-crash" or "--8l1-consent" or "--reuse-8l1-profile") { options.Add(args[i], "true"); continue; }
            if (args[i] is not ("--out" or "--repo" or "--bridge" or "--artifact-root" or "--profile-root") ||
                i + 1 == args.Length)
                throw new ArgumentException("Use --out D:\\... --repo D:\\... --bridge D:\\... --artifact-root D:\\... [--native-idle]");
            options.Add(args[i], args[++i]);
        }
        _root = Path.GetFullPath(options["--out"]);
        _repo = Path.GetFullPath(options["--repo"]);
        _bridgePath = Path.GetFullPath(options.GetValueOrDefault("--bridge") ??
            Path.Combine(_root, "not-run-rust-bridge.exe"));
        _artifactRoot = options.TryGetValue("--artifact-root", out var artifact)
            ? Path.GetFullPath(artifact) : null;
        if (!IsUnderProject(_root) || !IsUnderProject(_repo) || !IsUnderProject(_bridgePath))
            throw new UnauthorizedAccessException("7L1 requires D: under projects/CoS-Rust");
        _idle = options.ContainsKey("--native-idle");
        _cdpPreflight = options.ContainsKey("--cdp-preflight");
        _host8L1 = options.ContainsKey("--8l1");
        _host8L1CrashProbe = options.ContainsKey("--8l1-crash");
        _host8L1ConsentProbe = options.ContainsKey("--8l1-consent");
        _retained8L1ProfileRoot = options.TryGetValue("--profile-root", out var retained)
            ? Path.GetFullPath(retained) : null;
        _reuse8L1Profile = options.ContainsKey("--reuse-8l1-profile");
        if ((_retained8L1ProfileRoot is not null || _reuse8L1Profile) && !_host8L1)
            throw new ArgumentException("Retained profiles exclusive to 8L1");
        if (_reuse8L1Profile && _retained8L1ProfileRoot is null)
            throw new ArgumentException("Reuse requires exact profile root");
        if (_retained8L1ProfileRoot is not null)
        {
            var prefix = Path.DirectorySeparatorChar;
            if (!IsUnderProject(_retained8L1ProfileRoot) ||
                _root.Equals(_retained8L1ProfileRoot, StringComparison.OrdinalIgnoreCase) ||
                _root.StartsWith(_retained8L1ProfileRoot.TrimEnd(prefix) + prefix, StringComparison.OrdinalIgnoreCase) ||
                _retained8L1ProfileRoot.StartsWith(_root.TrimEnd(prefix) + prefix, StringComparison.OrdinalIgnoreCase))
                throw new UnauthorizedAccessException("Profile root and output must not overlap");
            RejectReparseAncestors(_retained8L1ProfileRoot);
            if (_reuse8L1Profile && !Directory.Exists(_retained8L1ProfileRoot))
                throw new UnauthorizedAccessException("Missing retained profiles");
            if (!_reuse8L1Profile && Directory.Exists(_retained8L1ProfileRoot) &&
                Directory.EnumerateFileSystemEntries(_retained8L1ProfileRoot).Any())
                throw new UnauthorizedAccessException("First launch needs empty profile root");
        }
        if ((_host8L1CrashProbe || _host8L1ConsentProbe) && !_host8L1)
            throw new ArgumentException("Native crash probe requires the 8L1 host");
        _l2 = options.ContainsKey("--7l2") || _host8L1;
        if (_l2 && (_cdpPreflight || _idle || (options.ContainsKey("--7l2") && _host8L1)))
            throw new ArgumentException("7L2 is a distinct source-pinned native gate");
        if (!_cdpPreflight &&
            (_artifactRoot is null || !IsUnderProject(_artifactRoot)))
            throw new UnauthorizedAccessException("Integrated 7L1 requires verified D: artifact root");
        foreach (var path in new[] { _root, _repo, _bridgePath })
            RejectReparseAncestors(path);
        if (_artifactRoot is not null)
            RejectReparseAncestors(_artifactRoot);
        Text = _host8L1 ? "CoSoS 8L1 source-owned privileged native host" :
            _l2 ? "CoSoS 7L2 real WebView2 debugger native gate" :
            "CoSoS 7L1 native Rust-MV3 bridge, isolated synthetic acceptance";
        Width = 1200;
        Height = 720;
        var split = new SplitContainer
        {
            Dock = DockStyle.Fill, Orientation = Orientation.Vertical, SplitterDistance = 590
        };
        split.Panel1.Controls.Add(_sameProfileTab);
        split.Panel1.Controls.Add(_a);
        split.Panel2.Controls.Add(_b);
        Controls.Add(split);
        FormClosing += (_, e) =>
        {
            if (_browsers.Values.Any(b => b.InFlightLedgerWrites > 0))
                e.Cancel = true;
        };
        Shown += async (_, _) => await ExecuteAsync();
    }

    private static bool IsUnderProject(string path) =>
        path.StartsWith(@"D:\projects\CoS-Rust\", StringComparison.OrdinalIgnoreCase);

    private static void RejectReparseAncestors(string path)
    {
        var current = Path.GetFullPath(path);
        while (true)
        {
            if ((File.Exists(current) || Directory.Exists(current)) &&
                (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                throw new UnauthorizedAccessException("reparse/junction ancestor refused");
            var parent = Path.GetDirectoryName(current);
            if (parent is null || string.Equals(current, parent, StringComparison.OrdinalIgnoreCase))
                break;
            current = parent;
        }
    }

    private static string HashFile(string file)
    {
        using var input = File.OpenRead(file);
        return Convert.ToHexString(SHA256.HashData(input)).ToLowerInvariant();
    }

    private static string GitOutput(string repo, params string[] args)
    {
        var info = new ProcessStartInfo("git")
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            WorkingDirectory = repo
        };
        info.ArgumentList.Add("-C");
        info.ArgumentList.Add(repo);
        foreach (var arg in args) info.ArgumentList.Add(arg);
        using var process = Process.Start(info) ??
            throw new InvalidOperationException("Git source attestor missing");
        var response = process.StandardOutput.ReadToEnd();
        if (!process.WaitForExit(5000) || process.ExitCode != 0)
        {
            if (!process.HasExited) process.Kill();
            throw new InvalidOperationException("Git source attestor rejected checkout");
        }
        return response.Trim();
    }

    private void VerifySourceArtifact()
    {
        if (_artifactRoot is null) throw new UnauthorizedAccessException("missing native artifact");
        var manifestPath = Path.Combine(_artifactRoot, "hashes.json");
        var sourcePath = Path.Combine(_artifactRoot, "source.sha");
        var expectedRust = Path.GetFullPath(Path.Combine(_artifactRoot, _l2 ? "7l2_bridge.exe" : "7l1_bridge.exe"));
        var expectedHost = Path.GetFullPath(Path.Combine(_artifactRoot, "host", "WindowsHost.dll"));
        if (!string.Equals(_bridgePath, expectedRust, StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(Path.GetFullPath(typeof(GateForm).Assembly.Location), expectedHost,
                StringComparison.OrdinalIgnoreCase))
            throw new UnauthorizedAccessException("unowned native executables");
        var sha = File.ReadAllText(sourcePath).Trim();
        if (sha.Length != 40 || !sha.All(Uri.IsHexDigit) ||
            !string.Equals(sha, GitOutput(_repo, "rev-parse", "HEAD"),
                StringComparison.OrdinalIgnoreCase))
            throw new UnauthorizedAccessException("native source SHA mismatch");
        var dirty = GitOutput(_repo, "status", "--porcelain", "--",
            "probes/7l1/extension", "probes/7l1/assets", "probes/7l1/WindowsHost",
            "probes/7l2", "crates/cos-core/src/browser_tools.rs",
            "extension/companion/worker.mjs",
            "extension/companion/ledger.mjs", "crates/cos-core/src/native_browser_bridge.rs",
            "crates/cos-core/examples/7l1_bridge.rs",
            "crates/cos-core/examples/7l2_bridge.rs",
            "apps/windows-host", "probes/8l1",
            "extension/adapter/browser-lifecycle.mjs",
            "extension/adapter/profile-session.mjs");
        if (dirty.Length != 0)
            throw new UnauthorizedAccessException("dirty native/extension source");
        using var json = JsonDocument.Parse(File.ReadAllBytes(manifestPath));
        if (json.RootElement.ValueKind != JsonValueKind.Array)
            throw new UnauthorizedAccessException("invalid artifact manifest");
        var seen = new HashSet<string>(StringComparer.Ordinal);
        var rootPrefix = _artifactRoot.TrimEnd(Path.DirectorySeparatorChar) +
            Path.DirectorySeparatorChar;
        foreach (var entry in json.RootElement.EnumerateArray())
        {
            var relative = entry.GetProperty("path").GetString() ??
                throw new UnauthorizedAccessException("missing artifact path");
            if (relative.Length == 0 || Path.IsPathRooted(relative) ||
                relative.Contains("..", StringComparison.Ordinal) || !seen.Add(relative))
                throw new UnauthorizedAccessException("unsafe or duplicate artifact path");
            var owned = Path.GetFullPath(Path.Combine(_artifactRoot,
                relative.Replace('/', Path.DirectorySeparatorChar)));
            if (!owned.StartsWith(rootPrefix, StringComparison.OrdinalIgnoreCase) ||
                !File.Exists(owned) ||
                !string.Equals(HashFile(owned), entry.GetProperty("sha256").GetString(),
                    StringComparison.OrdinalIgnoreCase))
                throw new UnauthorizedAccessException("artifact hash or path mismatch");
        }
        foreach (var required in new[] {_l2 ? "7l2_bridge.exe" : "7l1_bridge.exe", "host/WindowsHost.dll", "source.sha"})
            if (!seen.Contains(required))
                throw new UnauthorizedAccessException("incomplete artifact manifest");
        // The listed hashes are insufficient unless every loadable file is
        // listed. Traverse explicitly so a junction cannot redirect a DLL
        // to an unverified dependency outside the source-pinned artifact.
        var remaining = new HashSet<string>(seen, StringComparer.Ordinal);
        var directories = new Stack<string>();
        directories.Push(_artifactRoot);
        while (directories.Count != 0)
        {
            var directory = directories.Pop();
            foreach (var child in Directory.EnumerateFileSystemEntries(directory))
            {
                var attributes = File.GetAttributes(child);
                if ((attributes & FileAttributes.ReparsePoint) != 0)
                    throw new UnauthorizedAccessException("reparse path inside native artifact");
                if ((attributes & FileAttributes.Directory) != 0)
                {
                    directories.Push(child);
                    continue;
                }
                var relative = Path.GetRelativePath(_artifactRoot, child)
                    .Replace(Path.DirectorySeparatorChar, '/');
                if (relative == "hashes.json") continue;
                if (!remaining.Remove(relative))
                    throw new UnauthorizedAccessException("unmanifested native executable/dependency");
            }
        }
        if (remaining.Count != 0)
            throw new UnauthorizedAccessException("manifest lists missing native files");
        _verifiedSourceSha = sha;
        Mark("source_artifact_provenance", true,
            "In-host checked Git HEAD, clean authority files and all " +
            seen.Count + " SHA256 manifests: " + sha);
    }

    private void Mark(string name, bool passed, string evidence)
    {
        _checks[name] = new { status = passed ? "PASS" : "FAIL", evidence };
        if (!passed) _failed = true;
    }

    private void Pending(string name, string evidence) =>
        _checks[name] = new { status = "NOT_RUN", evidence };

    private void BuildExtension()
    {
        var source = Path.Combine(_repo, "probes", "7l1", "extension");
        var companion = Path.Combine(_repo, "extension", "companion");
        var destination = Path.Combine(_retained8L1ProfileRoot ?? _root, "extension");
        Directory.CreateDirectory(destination);
        foreach (var (from, file) in new[]
        {
            (source, "manifest.json"), (source, "sw.mjs"),
            (companion, "worker.mjs"), (companion, "ledger.mjs")
        })
        {
            var sourceFile = Path.Combine(from, file);
            var destinationFile = Path.Combine(destination, file);
            if (File.Exists(destinationFile))
            {
                if (!HashFile(sourceFile).Equals(HashFile(destinationFile),
                    StringComparison.OrdinalIgnoreCase))
                    throw new UnauthorizedAccessException("Retained extension source changed");
            }
            else if (_reuse8L1Profile)
                throw new UnauthorizedAccessException("Retained extension incomplete");
            else
                File.Copy(sourceFile, destinationFile);
        }
    }

    private async Task<Browser> SetupAsync(
        WebView2 view, string label, uint tabId,
        CoreWebView2Environment? sharedEnvironment = null, string? installedExtensionId = null)
    {
        var profileRoot = Path.Combine(_retained8L1ProfileRoot ?? _root, "profile-" + label);
        var env = sharedEnvironment ??
            await CoreWebView2Environment.CreateAsync(null, profileRoot,
                new CoreWebView2EnvironmentOptions { AreBrowserExtensionsEnabled = true });
        await view.EnsureCoreWebView2Async(env);
        var profilePath = Path.GetFullPath(view.CoreWebView2.Profile.ProfilePath);
        if (!IsUnderProject(profilePath)) throw new UnauthorizedAccessException("unsafe WebView2 profile");
        var extensionRoot = Path.Combine(_retained8L1ProfileRoot ?? _root, "extension");
        var idFile = Path.Combine(_retained8L1ProfileRoot ?? _root, "extension-id-" + label + ".txt");
        CoreWebView2BrowserExtension extension;
        if (installedExtensionId is not null)
            extension = (await view.CoreWebView2.Profile.GetBrowserExtensionsAsync())
                .Single(x => x.Id == installedExtensionId);
        else if (_reuse8L1Profile)
        {
            if (!File.Exists(idFile))
                throw new UnauthorizedAccessException("Missing retained extension ID");
            var id = File.ReadAllText(idFile).Trim();
            extension = (await view.CoreWebView2.Profile.GetBrowserExtensionsAsync())
                .Single(x => x.Id == id);
        }
        else
        {
            if (_retained8L1ProfileRoot is not null &&
                (await view.CoreWebView2.Profile.GetBrowserExtensionsAsync()).Count != 0)
                throw new UnauthorizedAccessException("Unexpected extension in clean profile");
            extension = await view.CoreWebView2.Profile.AddBrowserExtensionAsync(extensionRoot);
            if (_retained8L1ProfileRoot is not null)
                File.WriteAllText(idFile, extension.Id);
        }
        if (!extension.IsEnabled) throw new InvalidOperationException("extension disabled");
        var browser = new Browser
        {
            View = view, Profile = "7l1-" + label, ProfilePath = profilePath,
            ExtensionId = extension.Id, TabId = tabId
        };
        _browsers.Add(view, browser);
        _native8L1Authority?.Attach(browser);
        view.CoreWebView2.SetVirtualHostNameToFolderMapping("synthetic.cos.test",
            Path.Combine(_repo, "probes", "7l1", "assets"),
            CoreWebView2HostResourceAccessKind.DenyCors);
        view.CoreWebView2.NavigationStarting += (_, e) =>
        {
            if (!Uri.TryCreate(e.Uri, UriKind.Absolute, out var url) ||
                !string.Equals(url.GetLeftPart(UriPartial.Authority), Origin, StringComparison.Ordinal))
            {
                browser.RefusedOrigins++;
                e.Cancel = true;
                return;
            }
            // Ledger mutation is a bounded authenticated transaction. Refuse
            // navigation until its storage write settles, so the document
            // cannot be revoked half-way through a valid native commit.
            if (browser.InFlightLedgerWrites != 0)
            {
                e.Cancel = true;
                return;
            }
            RevokeAndRetire(browser); // synchronous fence before CDP/ledger awaits
            browser.NavigationEpoch++;
            browser.DocumentId = "doc-" + Guid.NewGuid().ToString("N");
        };
        view.CoreWebView2.ProcessFailed += (_, _) =>
        {
#if COS_8L1_PRODUCT
            if (_host8L1)
            {
                On8L1ProcessFailure();
                return;
            }
#endif
            RevokeAndRetire(browser);
        };
        if (_l2)
        {
            // Real browser-owned MV3 worker target retirement. A pre-signed
            // stale frame cannot retain authority after this CDP event.
            var destroyed = view.CoreWebView2.GetDevToolsProtocolEventReceiver(
                "Target.targetDestroyed");
            destroyed.DevToolsProtocolEventReceived += (_, evt) =>
            {
                try
                {
                    using var report = JsonDocument.Parse(evt.ParameterObjectAsJson);
                    if (!report.RootElement.TryGetProperty("targetId", out var target) ||
                        target.ValueKind != JsonValueKind.String ||
                        string.IsNullOrWhiteSpace(target.GetString()))
                    {
                        RevokeAndRetire(browser); // malformed native event fails closed
                        return;
                    }
                    if (!string.IsNullOrWhiteSpace(browser.WorkerTargetId) &&
                        string.Equals(target.GetString(), browser.WorkerTargetId,
                            StringComparison.Ordinal))
                    {
                        // Native CDP event for THIS tracked worker, not a
                        // similarly named worker from a subsequent generation.
                        browser.WorkerDestroyedEventObserved = true;
                        RevokeAndRetire(browser);
                    }
                }
                catch (Exception ex) when (ex is JsonException or InvalidOperationException)
                {
                    RevokeAndRetire(browser); // invalid native lifecycle report fails closed
                }
            };
        }
        view.Disposed += (_, _) => RevokeAndRetire(browser);
        view.CoreWebView2.Navigate(StartUrl);
        await WaitPageAsync(browser);
        return browser;
    }

    private static async Task WaitPageAsync(Browser browser)
    {
        for (var i = 0; i < 100; i++)
        {
            if (browser.View.CoreWebView2.Source == StartUrl)
            {
                var value = await browser.View.CoreWebView2.ExecuteScriptAsync("document.title");
                if (value == "\"7L1 Isolated Native Authority\"")
                {
                    // WebView2 Source/title can settle before the CDP page
                    // target catches up, particularly after cancelled
                    // navigation. Native ownership requires all three.
                    var page = await CdpAsync(browser, "Target.getTargetInfo", new { });
                    var info = Field(page, "targetInfo");
                    if (info.GetProperty("type").GetString() == "page" &&
                        info.GetProperty("url").GetString() == StartUrl)
                        return;
                }
            }
            await Task.Delay(100);
        }
        throw new InvalidOperationException("WebView2 source/title/CDP page target did not stabilize");
    }

    private static JsonElement Field(JsonElement root, params string[] names)
    {
        foreach (var name in names) root = root.GetProperty(name);
        return root;
    }

    private static async Task<JsonElement> CdpAsync(Browser browser, string method, object args)
    {
        var raw = await browser.View.CoreWebView2.CallDevToolsProtocolMethodAsync(
            method, JsonSerializer.Serialize(args));
        using var parsed = JsonDocument.Parse(raw);
        return parsed.RootElement.Clone();
    }

    private async Task<JsonElement> WorkerAsync(Browser browser, string expression)
    {
        await AssertOwnedAsync(browser);
        var session = browser.WorkerSession;
        // The WebView2 API silently falls back to the PAGE if session is empty.
        if (string.IsNullOrWhiteSpace(session)) throw new UnauthorizedAccessException("page CDP fallback");
        var raw = await browser.View.CoreWebView2.CallDevToolsProtocolMethodForSessionAsync(
            session, "Runtime.evaluate",
            JsonSerializer.Serialize(new
            {
                expression, awaitPromise = true, returnByValue = true
            }));
        await AssertOwnedAsync(browser); // fencing AFTER the async worker operation
        using var parsed = JsonDocument.Parse(raw);
        var result = parsed.RootElement.GetProperty("result");
        if (parsed.RootElement.TryGetProperty("exceptionDetails", out _))
            throw new InvalidOperationException("worker Runtime.evaluate rejected");
        return result.GetProperty("value").Clone();
    }

    private static string Js(string call, object data) =>
        "globalThis.__cos7l1Native." + call + "(" + JsonSerializer.Serialize(data) + ")";

    private async Task AssertOwnedAsync(Browser b)
    {
        if (b.View.IsDisposed || b.Revoked || b.NavigationEpoch != b.BoundNavigation ||
            string.IsNullOrWhiteSpace(b.WorkerSession) ||
            string.IsNullOrWhiteSpace(b.WorkerTargetId) ||
            !string.Equals(Path.GetFullPath(b.View.CoreWebView2.Profile.ProfilePath),
                b.ProfilePath, StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(b.View.CoreWebView2.Source, StartUrl, StringComparison.Ordinal))
            throw new UnauthorizedAccessException("stale controller/profile/page");
        var page = await CdpAsync(b, "Target.getTargetInfo", new { });
        var pageInfo = Field(page, "targetInfo");
        if (pageInfo.GetProperty("type").GetString() != "page" ||
            pageInfo.GetProperty("url").GetString() != StartUrl)
            throw new UnauthorizedAccessException("current real page target changed");
        var worker = await CdpAsync(b, "Target.getTargetInfo",
            new { targetId = b.WorkerTargetId });
        var workerInfo = Field(worker, "targetInfo");
        if (workerInfo.GetProperty("targetId").GetString() != b.WorkerTargetId ||
            workerInfo.GetProperty("type").GetString() != "service_worker" ||
            workerInfo.GetProperty("url").GetString() !=
                $"chrome-extension://{b.ExtensionId}/sw.mjs")
            throw new UnauthorizedAccessException("foreign/replaced MV3 worker target");
        if (b.View.IsDisposed || b.Revoked || b.NavigationEpoch != b.BoundNavigation ||
            !string.Equals(b.View.CoreWebView2.Source, StartUrl, StringComparison.Ordinal))
            throw new UnauthorizedAccessException("controller revoked during asynchronous target verification");
    }

    private async Task AttachWorkerAsync(Browser b)
    {
        await CdpAsync(b, "Target.setDiscoverTargets", new { discover = true });
        var expected = $"chrome-extension://{b.ExtensionId}/sw.mjs";
        for (var i = 0; i < 100; i++)
        {
            var targets = await CdpAsync(b, "Target.getTargets", new { });
            foreach (var target in Field(targets, "targetInfos").EnumerateArray())
            {
                if (target.GetProperty("type").GetString() != "service_worker" ||
                    target.GetProperty("url").GetString() != expected) continue;
                var id = target.GetProperty("targetId").GetString();
                if (string.IsNullOrWhiteSpace(id)) continue;
                var session = await CdpAsync(b, "Target.attachToTarget",
                    new { targetId = id, flatten = true });
                var sessionId = Field(session, "sessionId").GetString();
                if (string.IsNullOrWhiteSpace(sessionId))
                    throw new UnauthorizedAccessException("missing worker CDP session");
                b.WorkerDestroyedEventObserved = false;
                b.WorkerTargetId = id;
                b.WorkerSession = sessionId;
                b.BoundNavigation = b.NavigationEpoch;
                b.Revoked = false;
                await AssertOwnedAsync(b);
                await b.View.CoreWebView2.CallDevToolsProtocolMethodForSessionAsync(
                    sessionId, "Runtime.enable", "{}");
                return;
            }
            await Task.Delay(100);
        }
        throw new InvalidOperationException("No installed extension MV3 service_worker target");
    }

    private static object Context(Browser b) => new
    {
        profileId = b.Profile, tabId = b.TabId, frameId = 0,
        documentId = b.DocumentId, navigationEpoch = b.NavigationEpoch,
        sessionId = b.SessionId, sessionEpoch = 1, workerGeneration = b.WorkerGeneration,
        hostGeneration = b.HostGeneration, channelId = b.ChannelId
    };

    private static object Host(Browser b) => new
    {
        identity = new
        {
            profileId = b.Profile, tabId = b.TabId, frameId = 0,
            documentId = b.DocumentId, navigationEpoch = b.NavigationEpoch,
            sessionId = b.SessionId, sessionEpoch = 1, workerGeneration = b.WorkerGeneration
        },
        origin = Origin,
        hostGeneration = b.HostGeneration
    };

    private void RevokeAndRetire(Browser browser)
    {
        // Runs synchronously from WebView2 lifetime events, so the stale
        // channel cannot start another privileged evaluation.
        browser.Revoked = true;
        _native8L1Authority?.Revoke(browser);
        if (browser.ChannelId != 0 && !browser.RetireScheduled)
        {
            browser.RetireScheduled = true;
            browser.RetirementTask = RetireAsync(browser.ChannelId);
        }
    }

    private async Task<bool> RetireAsync(ulong channelId)
    {
        try
        {
            var response = await RustAsync(new { op = "retire", channelId });
            return response.GetProperty("ok").GetBoolean() &&
                response.GetProperty("value").GetProperty("retired").GetBoolean();
        }
        catch
        {
            return false; // Never silently revive after uncertain retirement.
        }
    }

    private static async Task RequireRetirementAsync(Browser browser)
    {
        if (browser.RetirementTask is null || !await browser.RetirementTask)
            throw new UnauthorizedAccessException("Rust channel retirement not confirmed");
        browser.ChannelId = 0;
        browser.RetirementTask = null;
        browser.RetireScheduled = false;
    }

    private async Task<JsonElement> RustAsync(object command)
    {
        await _rustIo.WaitAsync();
        try
        {
            if (_bridge is null || _bridge.HasExited)
                throw new InvalidOperationException("Rust authority process unavailable");
            await _bridge.StandardInput.WriteLineAsync(JsonSerializer.Serialize(command));
            await _bridge.StandardInput.FlushAsync();
            for (var callbackCount = 0; callbackCount < 4; callbackCount++)
            {
                var response = await _bridge.StandardOutput.ReadLineAsync()
                    .WaitAsync(TimeSpan.FromSeconds(15));
                if (response is null) throw new IOException("Rust authority exited without reply");
                using var data = JsonDocument.Parse(response);
                if (!data.RootElement.TryGetProperty("cdpCallback", out var callback))
                    return data.RootElement.Clone();
                if (!_l2) throw new UnauthorizedAccessException("CDP callback outside 7L2");
                object answer;
                try
                {
                    var value = await Run7L2CdpAsync(callback);
                    answer = new { cdpResponse = true, ok = true, value };
                }
                catch
                {
                    answer = new { cdpResponse = true, ok = false, value = (object?)null };
                }
                await _bridge.StandardInput.WriteLineAsync(JsonSerializer.Serialize(answer));
                await _bridge.StandardInput.FlushAsync();
            }
            throw new UnauthorizedAccessException("native debugger callback limit");
        }
        finally
        {
            _rustIo.Release();
        }
    }

    private static JsonElement RequireSuccess(JsonElement reply)
    {
        if (!reply.GetProperty("ok").GetBoolean())
            throw new InvalidOperationException("Rust authority denied: " +
                reply.GetProperty("error").GetString());
        return reply.GetProperty("value");
    }

    private async Task<JsonElement> RustAllowAsync(object command) =>
        RequireSuccess(await RustAsync(command));

    private async Task<JsonElement> BindAsync(Browser b, string sessionId)
    {
        if (b.ChannelId != 0 && !b.RetireScheduled)
            RevokeAndRetire(b);
        if (b.RetireScheduled)
            await RequireRetirementAsync(b);
        b.SessionId = sessionId;
        b.HostGeneration++;
        b.WorkerGeneration++;
        await AttachWorkerAsync(b);
        var registration = await RustAllowAsync(new { op = "register", host = Host(b) });
        b.ChannelId = registration.GetProperty("channelId").GetUInt64();
        var spec = new
        {
            channelId = b.ChannelId, hostGeneration = b.HostGeneration,
            challenge = registration.GetProperty("challenge").GetString(),
            key = registration.GetProperty("key").GetString(),
            context = Context(b)
        };
        // A one-use key is delivered ONLY to the pinned chrome-extension
        // service_worker CDP session, never to ExecuteScript/page/WebMessage.
        var worker = await WorkerAsync(b, Js("bootstrap", spec));
        b.WorkerInstance = worker.GetProperty("workerInstance").GetString();
        return await RustAllowAsync(new
        {
            op = "handshake", host = Host(b), channelId = b.ChannelId,
            generation = b.HostGeneration, proof = worker.GetProperty("proof").GetString()
        });
    }

    private async Task<JsonElement> ObserveAsync(Browser b)
    {
        var signed = await WorkerAsync(b, Js("observe", 1));
        var result = await RustAsync(new
        {
            op = "observe", host = Host(b), channelId = b.ChannelId,
            generation = b.HostGeneration, sequence = 1,
            payload = signed.GetProperty("payload").GetString(),
            mac = signed.GetProperty("mac").GetString()
        });
        if (result.GetProperty("ok").GetBoolean())
            await WorkerAsync(b, Js("confirmObservation", 1));
        return result;
    }

    private async Task<JsonElement> LedgerEventAsync(Browser b, string kind,
        ulong ledgerSequence, string? intentId = null, string? digest = null,
        string? receiptId = null, bool captureLedgerRejection = false)
    {
        var ev = new Dictionary<string, object>
        {
            ["kind"] = kind, ["protocol"] = 14, ["sequence"] = ledgerSequence
        };
        if (intentId is not null) ev["intentId"] = intentId;
        if (digest is not null) ev["digest"] = digest;
        if (receiptId is not null) ev["receiptId"] = receiptId;
        var sequence = ledgerSequence + 1; // 7B observe uses sequence=1
        var signed = await WorkerAsync(b, "globalThis.__cos7l1Native.prepare(" +
            JsonSerializer.Serialize(ev) + "," + sequence + ")");
        var authorized = await RustAllowAsync(new
        {
            op = "authorize", host = Host(b), channelId = b.ChannelId,
            generation = b.HostGeneration, sequence,
            payload = signed.GetProperty("payload").GetString(),
            mac = signed.GetProperty("mac").GetString()
        });
        b.InFlightLedgerWrites++;
        try
        {
            await AssertOwnedAsync(b);
            var expression = "globalThis.__cos7l1Native.accept(" +
                JsonSerializer.Serialize(ev) + "," + sequence + "," +
                JsonSerializer.Serialize(authorized.GetProperty("receipt").GetString()) + ")";
            if (captureLedgerRejection)
                expression += ".then(value => ({accepted:true,value}), " +
                    "error => ({accepted:false,name:error.name,code:error.code}))";
            return await WorkerAsync(b, expression);
        }
        finally
        {
            b.InFlightLedgerWrites--;
        }
    }

    private async Task<bool> RustDeniedAsync(object command)
    {
        var reply = await RustAsync(command);
        return !reply.GetProperty("ok").GetBoolean();
    }

    private static async Task<bool> ThrowsAsync(Func<Task> action)
    {
        try { await action(); return false; }
        catch (UnauthorizedAccessException) { return true; }
        catch (InvalidOperationException) { return true; }
    }

    private async Task ExecuteAsync()
    {
        if (_host8L1) { await Execute8L1Async(); return; }
        if (_l2) { await Execute7L2Async(); return; }
        var exit = 2;
        var rustExit = -1;
        try
        {
            if (Directory.Exists(_root) && Directory.EnumerateFileSystemEntries(_root).Any())
                throw new IOException("7L1 requires a fresh, empty D: output directory");
            Directory.CreateDirectory(_root);
            if (!_cdpPreflight) VerifySourceArtifact();
            BuildExtension();
            if (_cdpPreflight)
            {
                var cdpA = await SetupAsync(_a, "A", 1);
                var cdpB = await SetupAsync(_b, "B", 2);
                await AttachWorkerAsync(cdpA);
                await AttachWorkerAsync(cdpB);
                var workerA = await WorkerAsync(cdpA,
                    "globalThis.__cos7l1Native.workerInstance");
                var workerB = await WorkerAsync(cdpB,
                    "globalThis.__cos7l1Native.workerInstance");
                Mark("actual_mv3_cdp_discovery",
                    !string.IsNullOrEmpty(workerA.GetString()) &&
                    !string.IsNullOrEmpty(workerB.GetString()),
                    "Installed independent WebView2 MV3 service_worker target sessions; no Rust gate run");
                Pending("native_7b_7c_integration", "CDP discovery preflight only");
                exit = _failed ? 1 : 0;
                return;
            }
            if (!File.Exists(_bridgePath)) throw new FileNotFoundException("Rust 7L1 helper missing");
            var psi = new ProcessStartInfo(_bridgePath)
            {
                UseShellExecute = false, RedirectStandardInput = true,
                RedirectStandardOutput = true, RedirectStandardError = true,
                CreateNoWindow = true, WorkingDirectory = _root
            };
            psi.Environment["TEMP"] = _root;
            psi.Environment["TMP"] = _root;
            _bridge = Process.Start(psi) ?? throw new InvalidOperationException("Rust bridge start failed");
            var a = await SetupAsync(_a, "A", 1);
            var b = await SetupAsync(_b, "B", 2);
            Mark("distinct_native_profiles",
                a.ProfilePath != b.ProfilePath && a.ExtensionId.Length > 0 &&
                b.ExtensionId.Length > 0,
                "Two actual independently installed WebView2 profiles with browser extension IDs");
            Mark("page_message_spoof_denied",
                await _a.CoreWebView2.ExecuteScriptAsync("typeof globalThis.__cos7l1Native") ==
                    "\"undefined\"" &&
                await _b.CoreWebView2.ExecuteScriptAsync(
                    "(() => { window.postMessage({op:'register',channelId:1,kind:'ack'}, '*'); return typeof globalThis.__cos7l1Native; })()") ==
                    "\"undefined\"",
                "MAIN world and posted spoof event cannot reach the worker's CDP-only native adapter");
            var originDenialBefore = a.RefusedOrigins;
            _a.CoreWebView2.Navigate("https://other.synthetic.invalid/foreign-document");
            await Task.Delay(250);
            Mark("false_origin_denial",
                a.RefusedOrigins == originDenialBefore + 1 &&
                _a.CoreWebView2.Source == StartUrl,
                "Attempted foreign-origin navigation canceled synchronously at live WebView2 controller");
            await WaitPageAsync(a);
            await BindAsync(a, "7l1-shared-session");
            var aFirst = await ObserveAsync(a);
            Mark("native_7b_mv3_7c_handshake",
                aFirst.GetProperty("ok").GetBoolean() &&
                aFirst.GetProperty("value").GetProperty("observation").GetString() == "FirstProof",
                "Actual service_worker target signed Rust handshake + 7B first-proof observation");

            // Two physical controllers in the SAME profile share one worker.
            // Give the challenger a NEWER worker generation so it can complete
            // actual handshake; only Rust 7A first-proof arbitration may deny.
            var rival = await SetupAsync(_sameProfileTab, "A", 3,
                _a.CoreWebView2.Environment, a.ExtensionId);
            var realSameProfile = rival.ProfilePath == a.ProfilePath &&
                rival.ExtensionId == a.ExtensionId &&
                rival.TabId != a.TabId &&
                rival.View.CoreWebView2 != a.View.CoreWebView2;
            rival.HostGeneration = a.HostGeneration;
            var rivalHandshake = await BindAsync(rival, "7l1-shared-session");
            var contender = await ObserveAsync(rival);
            var rivalDeniedByFirstProof = !contender.GetProperty("ok").GetBoolean() &&
                contender.GetProperty("error").GetString() ==
                    "a session is already attributed to another browser scope";
            _sameProfileTab.Dispose();
            var rivalRetired = rival.RetirementTask is not null &&
                await rival.RetirementTask;
            Mark("same_profile_competing_tab",
                realSameProfile && rivalHandshake.GetProperty("authenticated").GetBoolean() &&
                rivalDeniedByFirstProof && rivalRetired,
                "Second physical tab authenticated with Rust and signed a valid observation; Rust returned exact 7A ConflictingAttribution and retired rival channel");
            a.HostGeneration = Math.Max(a.HostGeneration, rival.HostGeneration);
            await BindAsync(a, "7l1-shared-session");
            var originalRestored = await ObserveAsync(a);
            Mark("same_profile_original_restored",
                originalRestored.GetProperty("ok").GetBoolean() &&
                originalRestored.GetProperty("value").GetProperty("observation").GetString() ==
                    "Advanced",
                "Original verified owner reauthenticated at higher generation after Rust rejected same-profile contender");

            var crossProfileHandshake = await BindAsync(b, "7l1-shared-session");
            var competing = await ObserveAsync(b);
            Mark("cross_profile_session_conflict",
                crossProfileHandshake.GetProperty("authenticated").GetBoolean() &&
                !competing.GetProperty("ok").GetBoolean() &&
                competing.GetProperty("error").GetString() ==
                    "a session is already attributed to another browser scope",
                "Separate physical WebView2 profile authenticated with Rust; exact 7A ConflictingAttribution refused first-proof takeover");
            await BindAsync(b, "7l1-independent-session");
            var bFirst = await ObserveAsync(b);
            Mark("cross_profile_recovery",
                bFirst.GetProperty("ok").GetBoolean(),
                "Second profile can elect its independently bound session after refused competition");

            Mark("page_cdp_fallback_refused", await ThrowsAsync(async () =>
            {
                var old = a.WorkerSession;
                a.WorkerSession = "";
                try { await WorkerAsync(a, "1+1"); }
                finally { a.WorkerSession = old; }
            }), "An empty CDP session is refused before WebView2 can route to the top-level page");

            var digest = new string('a', 64);
            var attached = await LedgerEventAsync(a, "attach", 1);
            var claimed = await LedgerEventAsync(a, "claim", 2, "intent-7l1", digest);
            var repair = await LedgerEventAsync(a, "repair", 3, "intent-7l1", digest);
            Mark("real_mv3_persisted_claim",
                attached.GetProperty("kind").GetString() == "attached" &&
                claimed.GetProperty("status").GetString() == "claimed" &&
                !claimed.GetProperty("duplicate").GetBoolean() &&
                repair.GetProperty("status").GetString() == "claimed" &&
                !repair.GetProperty("resendAllowed").GetBoolean(),
                "7C chrome.storage.session claim and repair returned after actual worker mutation");

            var completedClaim = await LedgerEventAsync(a, "claim", 4, "intent-acked", digest);
            var acked = await LedgerEventAsync(a, "ack", 5, "intent-acked", digest, "receipt-verified");
            var ackRecovered = await LedgerEventAsync(a, "repair", 6, "intent-acked", digest);
            Mark("real_mv3_ack_persisted",
                completedClaim.GetProperty("status").GetString() == "claimed" &&
                acked.GetProperty("status").GetString() == "acked" &&
                !acked.GetProperty("duplicate").GetBoolean() &&
                ackRecovered.GetProperty("status").GetString() == "acked" &&
                ackRecovered.GetProperty("receiptId").GetString() == "receipt-verified" &&
                !ackRecovered.GetProperty("resendAllowed").GetBoolean(),
                "Positive real Rust-approved MV3 claim/ACK committed and recovered from storage.session");

            var bAttached = await LedgerEventAsync(b, "attach", 1);
            Mark("separate_profile_ledger", bAttached.GetProperty("kind").GetString() == "attached",
                "Different WebView2 profile has independent MV3 storage.session");

            var before = a.WorkerInstance;
            if (_idle)
            {
                // An attached debugger can keep a worker awake. Detach first,
                // observe whether it ACTUALLY disappears before triggering a
                // new browser event, then force-stop only if idle cannot be shown.
                var sessionToRelease = a.WorkerSession;
                if (string.IsNullOrWhiteSpace(sessionToRelease))
                    throw new InvalidOperationException("missing worker session to detach");
                await CdpAsync(a, "Target.detachFromTarget", new { sessionId = sessionToRelease });
                a.WorkerSession = null;
                a.WorkerTargetId = null;
                await Task.Delay(TimeSpan.FromSeconds(65));
                var targetsAfterIdle = await CdpAsync(a, "Target.getTargets", new { });
                var expectedWorkerUrl = $"chrome-extension://{a.ExtensionId}/sw.mjs";
                var activeWorkers = Field(targetsAfterIdle, "targetInfos").EnumerateArray()
                    .Where(t => t.GetProperty("type").GetString() == "service_worker" &&
                        t.GetProperty("url").GetString() == expectedWorkerUrl)
                    .ToArray();
                var naturalIdle = activeWorkers.Length == 0;
                if (naturalIdle)
                    Mark("native_mv3_natural_idle", true,
                        "Service worker target absent BEFORE page reload or forced stop");
                else
                {
                    Pending("native_mv3_natural_idle",
                        "Worker target remained live after 65s of detached idle; exercise forced stop");
                    foreach (var target in activeWorkers)
                    {
                        var targetId = target.GetProperty("targetId").GetString();
                        var closed = await CdpAsync(a, "Target.closeTarget", new { targetId });
                        if (!closed.GetProperty("success").GetBoolean())
                            throw new InvalidOperationException("real worker stop refused");
                    }
                }
                _a.CoreWebView2.Reload();
                await WaitPageAsync(a);
                await AttachWorkerAsync(a);
                var after = await WorkerAsync(a, "globalThis.__cos7l1Native.workerInstance");
                var changed = after.GetString() != before;
                Mark("real_mv3_idle_or_forced_restart", changed,
                    "65s detached idle, actual new MV3 instance=" + changed +
                    ", naturalIdleObserved=" + naturalIdle +
                    ", forcedWorkerTargetStop=" + !naturalIdle);
                if (changed)
                {
                    // New channel and worker generation; the 7C ledger remains
                    // stored across idle retirement and never resends an intent.
                    await BindAsync(a, a.SessionId);
                    await ObserveAsync(a);
                    await LedgerEventAsync(a, "attach", 1);
                    var pending = await LedgerEventAsync(a, "repair", 2, "intent-7l1", digest);
                    Mark("restart_repair_no_duplicate",
                        pending.GetProperty("status").GetString() == "claimed" &&
                        !pending.GetProperty("resendAllowed").GetBoolean(),
                        "Persisted 7C claim repaired after real worker restart, resend forbidden");
                    var priorAck = await LedgerEventAsync(a, "repair", 3, "intent-acked", digest);
                    Mark("restart_positive_ack_recovery",
                        priorAck.GetProperty("status").GetString() == "acked" &&
                        priorAck.GetProperty("receiptId").GetString() == "receipt-verified" &&
                        !priorAck.GetProperty("resendAllowed").GetBoolean(),
                        "Committed positive ACK remains queryable across real MV3 incarnation restart");
                    var storageBefore = await WorkerAsync(a,
                        "chrome.storage.session.get('cosos7cLedgerV1').then(x => x.cosos7cLedgerV1)");
                    var claimedBefore = storageBefore.GetProperty("intents")
                        .GetProperty("intent-7l1");
                    // The helper performs a fresh signed worker event and
                    // successful Rust authorization before returning a caught
                    // CompanionError from the actual 7C ledger dispatch.
                    var ackOutcome = await LedgerEventAsync(a, "ack", 4,
                        "intent-7l1", digest, "receipt-stale", captureLedgerRejection: true);
                    var storageAfter = await WorkerAsync(a,
                        "chrome.storage.session.get('cosos7cLedgerV1').then(x => x.cosos7cLedgerV1)");
                    Mark("cross_generation_ack_refused",
                        !ackOutcome.GetProperty("accepted").GetBoolean() &&
                        ackOutcome.GetProperty("name").GetString() == "CompanionError" &&
                        ackOutcome.GetProperty("code").GetString() == "stale-claim-generation" &&
                        claimedBefore.GetProperty("status").GetString() == "claimed" &&
                        claimedBefore.GetProperty("receiptId").ValueKind == JsonValueKind.Null &&
                        storageBefore.GetRawText() == storageAfter.GetRawText(),
                        "Rust authorized exact signed ACK; 7C rejected stale-claim-generation and persisted storage.session remained byte-for-byte unchanged");
                }
            }
            else
            {
                Pending("native_mv3_natural_idle", "Run with --native-idle for 65-second observation");
                Pending("real_mv3_idle_or_forced_restart", "Run with --native-idle for actual worker retirement");
                Pending("restart_repair_no_duplicate", "Requires worker restart while extension remains enabled");
                Pending("restart_positive_ack_recovery", "Requires worker restart while extension remains enabled");
                Pending("cross_generation_ack_refused", "Requires new authenticated worker generation");
            }

            // Navigation revokes native controller identity immediately.
            _b.CoreWebView2.Navigate(StartUrl + "?other-document=1");
            await Task.Delay(300);
            var revoked = await ThrowsAsync(async () => await WorkerAsync(b, "1+1"));
            var retired = b.RetirementTask is not null && await b.RetirementTask;
            Mark("navigation_revocation",
                revoked && retired,
                "Native navigation event automatically retired Rust channel and revoked privileged CDP access");

            // Close the ACTUAL second WebView2 controller while a correctly
            // signed 7C event is prepared but has not been admitted. Reusing
            // its old frame/worker signature after physical disposal must
            // fail at both the native session and Rust authority boundaries.
            _b.CoreWebView2.Navigate(StartUrl);
            await WaitPageAsync(b);
            // Navigation advances the document identity, but 7A never
            // implicitly rebinds this physical tab to a different session.
            // Preserve the already proven owner when testing real tab close.
            await BindAsync(b, b.SessionId);
            var closeFirst = RequireSuccess(await ObserveAsync(b));
            if (closeFirst.GetProperty("observation").GetString() != "Advanced")
                throw new InvalidOperationException("tab-close prerequisite did not advance the verified owner");
            var signedBeforeClose = await WorkerAsync(b,
                "globalThis.__cos7l1Native.prepare(" +
                JsonSerializer.Serialize(new { protocol = 14, kind = "attach", sequence = 1 }) +
                ",2)");
            var closeHost = Host(b);
            var closeChannel = b.ChannelId;
            var closeGeneration = b.HostGeneration;
            _b.Dispose();
            var disposedRefused = await ThrowsAsync(async () =>
                await WorkerAsync(b, "globalThis.__cos7l1Native.workerInstance"));
            var retiredAfterClose = b.RetirementTask is not null &&
                await b.RetirementTask;
            var lateEventDenied = await RustDeniedAsync(new
            {
                op = "authorize", host = closeHost, channelId = closeChannel,
                generation = closeGeneration, sequence = 2,
                payload = signedBeforeClose.GetProperty("payload").GetString(),
                mac = signedBeforeClose.GetProperty("mac").GetString()
            });
            Mark("native_tab_close_revocation",
                closeFirst.GetProperty("observation").GetString() == "Advanced" &&
                b.Revoked && b.View.IsDisposed && disposedRefused &&
                retiredAfterClose && lateEventDenied,
                "Physical WebView2 controller disposal automatically retires Rust channel; stale CDP and pre-signed late ledger event rejected");

            var required = new[]
            {
                "source_artifact_provenance",
                "distinct_native_profiles", "page_message_spoof_denied",
                "false_origin_denial",
                "native_7b_mv3_7c_handshake", "same_profile_competing_tab",
                "same_profile_original_restored",
                "cross_profile_session_conflict",
                "cross_profile_recovery", "page_cdp_fallback_refused",
                "real_mv3_persisted_claim", "real_mv3_ack_persisted",
                "separate_profile_ledger", "navigation_revocation",
                "native_tab_close_revocation",
                "real_mv3_idle_or_forced_restart", "restart_repair_no_duplicate",
                "restart_positive_ack_recovery", "cross_generation_ack_refused"
            };
            // A missing test cannot turn into a synthetic or reported PASS.
            var everyPassed = required.All(k =>
                _checks.TryGetValue(k, out var value) &&
                JsonSerializer.Serialize(value).Contains("\"status\":\"PASS\"", StringComparison.Ordinal));
            Mark("integrated_native_7l1_gate", everyPassed && !_failed,
                "Physical 7B Rust process + actual WebView2 7C MV3 worker; all required gates must execute");
            exit = everyPassed && !_failed ? 0 : 2;
        }
        catch (Exception ex)
        {
            Mark("fatal", false, ex.GetType().Name + ": " + ex.Message);
            exit = 1;
        }
        finally
        {
            if (_bridge is not null)
            {
                try
                {
                    _bridge.StandardInput.Close();
                    if (!_bridge.WaitForExit(5000)) _bridge.Kill(entireProcessTree: true);
                    _bridge.WaitForExit(5000);
                    rustExit = _bridge.ExitCode;
                }
                catch (Exception ex)
                {
                    Mark("rust_process_exit", false, ex.GetType().Name);
                }
                _bridge.Dispose();
            }
            if (_cdpPreflight)
                Pending("rust_process_exit", "CDP preflight did not start Rust");
            else if (rustExit != 0)
            {
                Mark("rust_process_exit", false, "Process exit=" + rustExit);
                exit = 1;
            }
            else Mark("rust_process_exit", true, "Real child process exit=0");
            Directory.CreateDirectory(_root);
            var receipt = new
            {
                scope = "7L1 Rust-to-WebView2 CDP extension service_worker; synthetic D: only",
                preflightOnly = _cdpPreflight,
                dateUtc = DateTimeOffset.UtcNow,
                sourceSha = _verifiedSourceSha ?? "NOT_RUN",
                hostBinarySha256 = HashFile(typeof(GateForm).Assembly.Location),
                rustBinarySha256 = File.Exists(_bridgePath) ? HashFile(_bridgePath) : "missing",
                extensionWorkerSha256 = File.Exists(Path.Combine(_repo,"probes","7l1","extension","sw.mjs"))
                    ? HashFile(Path.Combine(_repo,"probes","7l1","extension","sw.mjs")) : "missing",
                native7L1Accepted = !_cdpPreflight && exit == 0,
                rustProcessExit = rustExit, checks = _checks
            };
            File.WriteAllText(Path.Combine(_root, "receipt.json"),
                JsonSerializer.Serialize(receipt, new JsonSerializerOptions { WriteIndented = true }));
            Environment.ExitCode = exit;
            BeginInvoke(new Action(Close));
        }
    }
}
