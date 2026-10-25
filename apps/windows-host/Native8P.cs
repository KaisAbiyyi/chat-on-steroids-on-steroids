using System.Text.Json;
using Microsoft.Web.WebView2.Core;

namespace CoSoS.Probe7L1.WindowsHost;

// Production provider navigation is native owned. It intentionally DOES NOT
// use the synthetic WebView2/CDP/MV3 observation authority. Navigating an
// approved provider is not an authenticated or reattested signed-in channel.
internal sealed partial class GateForm
{
    private string? _host8PConfig;
    private string? _host8PProvider;

    private sealed record ProviderSpec(string Id, string Origin, HashSet<string> Origins);

    private sealed class ProviderPolicy
    {
        private readonly Dictionary<string, ProviderSpec> _providers;

        private ProviderPolicy(Dictionary<string, ProviderSpec> providers) => _providers = providers;

        private static void Deny(string reason) => throw new UnauthorizedAccessException("8P provider policy: " + reason);
        private static bool ExactKeys(JsonElement value, params string[] names) =>
            value.ValueKind == JsonValueKind.Object &&
            value.EnumerateObject().Count() == names.Length &&
            value.EnumerateObject().All(p => names.Contains(p.Name, StringComparer.Ordinal));

        // Reject noncanonical URL text rather than silently fixing it.
        private static bool ExactHttpsOrigin(string? raw)
        {
            if (string.IsNullOrEmpty(raw) || raw.Length > 253 || raw.EndsWith("/", StringComparison.Ordinal) ||
                raw.Contains('\\') || raw.Contains('@') || raw.Contains('%') ||
                !Uri.TryCreate(raw, UriKind.Absolute, out var uri) ||
                uri.Scheme != Uri.UriSchemeHttps || uri.UserInfo.Length != 0 ||
                uri.Query.Length != 0 || uri.Fragment.Length != 0 ||
                !uri.IsDefaultPort || !string.Equals(uri.AbsoluteUri, raw + "/", StringComparison.Ordinal) ||
                !string.Equals(uri.GetLeftPart(UriPartial.Authority), raw, StringComparison.Ordinal))
                return false;
            var host = uri.IdnHost;
            if (!string.Equals(uri.Host, host, StringComparison.Ordinal) ||
                host.Length > 253 || !host.Contains('.', StringComparison.Ordinal) ||
                host.StartsWith(".", StringComparison.Ordinal) || host.EndsWith(".", StringComparison.Ordinal))
                return false;
            return host.Split('.').All(label => label.Length is > 0 and <= 63 &&
                label[0] != '-' && label[^1] != '-' &&
                label.All(c => (c >= 'a' && c <= 'z') ||
                    (c >= '0' && c <= '9') || c == '-'));
        }

        public static ProviderPolicy Parse(string json)
        {
            using var document = JsonDocument.Parse(json);
            var root = document.RootElement;
            if (!ExactKeys(root, "providers")) { Deny("shape"); }
            var values = root.GetProperty("providers");
            if (values.ValueKind != JsonValueKind.Array || values.GetArrayLength() is < 1 or > 16)
                Deny("provider-count");
            var parsed = new Dictionary<string, ProviderSpec>(StringComparer.Ordinal);
            foreach (var entry in values.EnumerateArray())
            {
                if (!ExactKeys(entry, "id", "origin", "redirectOrigins")) Deny("entry-shape");
                var idElement = entry.GetProperty("id");
                var originElement = entry.GetProperty("origin");
                var redirects = entry.GetProperty("redirectOrigins");
                if (idElement.ValueKind != JsonValueKind.String ||
                    originElement.ValueKind != JsonValueKind.String ||
                    redirects.ValueKind != JsonValueKind.Array ||
                    redirects.GetArrayLength() > 16) Deny("types");
                var id = idElement.GetString() ?? "";
                var origin = originElement.GetString() ?? "";
                if (id.Length is < 1 or > 40 ||
                    !id.All(c => (c >= 'a' && c <= 'z') ||
                        (c >= '0' && c <= '9') || c == '-') ||
                    !ExactHttpsOrigin(origin)) Deny("provider-id-origin");
                var allowed = new HashSet<string>(StringComparer.Ordinal) { origin };
                foreach (var redirect in redirects.EnumerateArray())
                {
                    if (redirect.ValueKind != JsonValueKind.String ||
                        !ExactHttpsOrigin(redirect.GetString()) ||
                        !allowed.Add(redirect.GetString()!)) Deny("redirect-origin");
                }
                if (!parsed.TryAdd(id, new ProviderSpec(id, origin, allowed)))
                    Deny("duplicate-provider");
            }
            return new ProviderPolicy(parsed);
        }

        public string Landing(string providerId)
        {
            if (!_providers.TryGetValue(providerId, out var provider)) { Deny("unlisted-provider"); }
            return provider!.Origin + "/";
        }

        // A redirect must remain at an exact host-configured HTTPS origin.
        // Query, fragment, credential-bearing, noncanonical and popup URLs are
        // denied (some real IdP OAuth flows thus remain intentionally unsupported).
        public bool Allows(string providerId, string url)
        {
            if (!_providers.TryGetValue(providerId, out var provider) ||
                string.IsNullOrEmpty(url) || url.Length > 2048 ||
                url.Contains('\\') || url.Contains('@') || url.Contains('%') ||
                !Uri.TryCreate(url, UriKind.Absolute, out var parsed) ||
                parsed.Scheme != Uri.UriSchemeHttps || parsed.UserInfo.Length != 0 ||
                parsed.Query.Length != 0 || parsed.Fragment.Length != 0 ||
                !string.Equals(parsed.AbsoluteUri, url, StringComparison.Ordinal))
                return false;
            return provider.Origins.Contains(parsed.GetLeftPart(UriPartial.Authority));
        }

        public static void SelfTest()
        {
            const string config = """{"providers":[{"id":"chatgpt","origin":"https://chatgpt.com","redirectOrigins":["https://auth.openai.com"]},{"id":"gemini","origin":"https://gemini.google.com","redirectOrigins":[]}]}""";
            var p = Parse(config);
            static void Check(bool value) { if (!value) throw new InvalidOperationException("8P origin policy regression"); }
            Check(p.Landing("chatgpt") == "https://chatgpt.com/");
            Check(p.Allows("chatgpt", "https://chatgpt.com/") &&
                  p.Allows("chatgpt", "https://auth.openai.com/login") &&
                  p.Allows("gemini", "https://gemini.google.com/"));
            foreach (var rejected in new[] {
                "http://chatgpt.com/", "https://chatgpt.com.evil.test/",
                "https://user@chatgpt.com/", "https://chatgpt.com@evil.test/",
                "https://chatgpt.com/?token=secret", "https://chatgpt.com/#access_token",
                "javascript:alert(1)", "file:///C:/secret", "https://auth.openai.com.evil.test/",
                "https://chatgpt.com/%2f", "https://gemini.google.com/"
            }) Check(!p.Allows("chatgpt", rejected));
            Check(!p.Allows("unlisted", "https://chatgpt.com/"));
            foreach (var invalid in new[] {
                """{"providers":[]}""",
                """{"providers":[{"id":"bad","origin":"http://example.com","redirectOrigins":[]}]}""",
                """{"providers":[{"id":"bad","origin":"https://user@example.com","redirectOrigins":[]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com/path","redirectOrigins":[]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com/","redirectOrigins":[]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com.evil/","redirectOrigins":["http://example.com"]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[],"unsafe":true}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[]},{"id":"bad","origin":"https://example.com","redirectOrigins":[]}]}"""
            })
            {
                try { Parse(invalid); throw new InvalidOperationException("8P invalid config admitted"); }
                catch (UnauthorizedAccessException) { }
            }
        }
    }

    internal static void Run8PPolicyTests() => ProviderPolicy.SelfTest();

    private async Task Execute8PAsync()
    {
        // This opt-in source-owned navigation entrypoint is NOT the 7U1 sign-in
        // gate, an MV3/host rebind, or permission approval. No account data,
        // cookies, form input, history or tokens are read or exported here.
        if (_host8PConfig is null || _host8PProvider is null)
            throw new ArgumentException("8P requires --8p-config and --8p-provider");
        RejectReparseAncestors(_host8PConfig);
        if (!IsUnderProject(_host8PConfig) || !File.Exists(_host8PConfig))
            throw new UnauthorizedAccessException("8P configuration must be an existing D: host file");
        if (Directory.Exists(_root) && Directory.EnumerateFileSystemEntries(_root).Any())
            throw new IOException("8P needs a fresh isolated D: output directory");
        Directory.CreateDirectory(_root);
        VerifySourceArtifact();
        var policy = ProviderPolicy.Parse(File.ReadAllText(_host8PConfig));
        var landing = policy.Landing(_host8PProvider); // rejects unlisted provider before navigation
        var decision = MessageBox.Show(this,
            "Navigate the isolated CoSoS profile to the configured provider " +
            _host8PProvider + "?\nNo sign-in, observation, or permission is authorized by this action.",
            "8P-HOST native provider consent", MessageBoxButtons.YesNo,
            MessageBoxIcon.Question, MessageBoxDefaultButton.Button2);
        if (decision != DialogResult.Yes) { Close(); return; }

        var profile = Path.Combine(_root, "8p-isolated-profile");
        RejectReparseAncestors(profile);
        var env = await CoreWebView2Environment.CreateAsync(null, profile);
        await _a.EnsureCoreWebView2Async(env);
        var effectiveProfile = Path.GetFullPath(_a.CoreWebView2.Profile.ProfilePath);
        if (!IsUnderProject(effectiveProfile)) throw new UnauthorizedAccessException("foreign 8P profile");
        var navigationEpoch = 0UL;
        var refused = 0;
        var completed = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        _a.CoreWebView2.PermissionRequested += (_, e) =>
        {
            e.State = CoreWebView2PermissionState.Deny;
            e.SavesInProfile = false;
            e.Handled = true;
        };
        _a.CoreWebView2.NewWindowRequested += (_, e) => e.Handled = true;
        _a.CoreWebView2.NavigationStarting += (_, e) =>
        {
            // Every document transition invalidates any preceding channel.
            // Production flow cannot invoke synthetic BindAsync/ObserveAsync.
            navigationEpoch++;
            if (!policy.Allows(_host8PProvider, e.Uri))
            {
                refused++;
                e.Cancel = true;
            }
        };
        _a.CoreWebView2.NavigationCompleted += (_, e) =>
        {
            if (!e.IsSuccess || !policy.Allows(_host8PProvider,
                _a.CoreWebView2.Source)) completed.TrySetResult(false);
            else completed.TrySetResult(true);
        };
        _a.CoreWebView2.Navigate(landing);
        var settled = await Task.WhenAny(completed.Task, Task.Delay(TimeSpan.FromSeconds(30)));
        var success = settled == completed.Task && await completed.Task;
        var receipt = new
        {
            sourceSha = _verifiedSourceSha, providerId = _host8PProvider,
            navigationAllowed = success, navigationEpoch, deniedNavigationCount = refused,
            rustChannelActive = false, mv3WorkerAttested = false,
            permissionPolicy = "Deny (nonpersistent)", authenticated = false,
            native8PAccepted = false, owner7U1Accepted = false
        };
        File.WriteAllText(Path.Combine(_root, "8p-navigation-receipt.json"),
            JsonSerializer.Serialize(receipt, new JsonSerializerOptions { WriteIndented = true }));
        // Do not automatically continue to a provider sign-in workflow.
    }
}
