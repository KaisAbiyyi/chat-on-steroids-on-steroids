using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace CoSoS.Probe7L1.WindowsHost;

// Production provider navigation is native owned. It intentionally DOES NOT
// use the synthetic WebView2/CDP/MV3 observation authority. Navigating an
// approved provider is not an authenticated or reattested signed-in channel.
internal sealed partial class GateForm
{
    private string? _host8PConfig;
    private string? _host8PProvider;

    private sealed record ProviderSpec(string Id, string Origin, HashSet<string> Origins,
        HashSet<string> CallbackUrls);

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
                if (!ExactKeys(entry, "id", "origin", "redirectOrigins") &&
                    !ExactKeys(entry, "id", "origin", "redirectOrigins", "callbackUrls"))
                    Deny("entry-shape");
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
                var callbacks = new HashSet<string>(StringComparer.Ordinal);
                if (entry.TryGetProperty("callbackUrls", out var callbackList))
                {
                    if (callbackList.ValueKind != JsonValueKind.Array ||
                        callbackList.GetArrayLength() > 8) Deny("callback-shape");
                    foreach (var item in callbackList.EnumerateArray())
                    {
                        if (item.ValueKind != JsonValueKind.String) Deny("callback-type");
                        var callbackRaw = item.GetString() ?? "";
                        if (!Uri.TryCreate(callbackRaw, UriKind.Absolute, out var callback) ||
                            callback.Scheme != Uri.UriSchemeHttps ||
                            callback.UserInfo.Length != 0 || callback.Query.Length != 0 ||
                            callback.Fragment.Length != 0 || callbackRaw.Contains('\\') ||
                            callbackRaw.Contains('@') || callbackRaw.Contains('%') ||
                            !string.Equals(callback.AbsoluteUri, callbackRaw,
                                StringComparison.Ordinal) ||
                            callback.AbsolutePath == "/" ||
                            !allowed.Contains(callback.GetLeftPart(UriPartial.Authority)) ||
                            !callbacks.Add(callbackRaw))
                            Deny("callback-url");
                    }
                }
                if (!parsed.TryAdd(id, new ProviderSpec(id, origin, allowed, callbacks)))
                    Deny("duplicate-provider");
            }
            return new ProviderPolicy(parsed);
        }

        public string Landing(string providerId)
        {
            if (!_providers.TryGetValue(providerId, out var provider)) { Deny("unlisted-provider"); }
            return provider!.Origin + "/";
        }

        // A URL from WebView2 is untrusted: a permitted HTTPS origin does not
        // authorize user/session identity or any native/MV3 capability.
        // OAuth authorization *requests* may contain a bounded PKCE query, but
        // credential-bearing callbacks and fragment/implicit-token flows are
        // denied until a separately native-attested one-use callback is built.
        private static readonly HashSet<string> OAuthRequestKeys = new(StringComparer.Ordinal)
        {
            "response_type", "client_id", "redirect_uri", "scope", "state",
            "nonce", "code_challenge", "code_challenge_method", "prompt",
            "audience"
        };

        private static bool SafeOAuthRequest(ProviderSpec provider, string query)
        {
            if (query.Length is < 2 or > 2049 || query[0] != '?') return false;
            var args = new Dictionary<string, string>(StringComparer.Ordinal);
            foreach (var pair in query[1..].Split('&'))
            {
                var split = pair.IndexOf('=');
                if (split <= 0 || split == pair.Length - 1) return false;
                var key = pair[..split];
                var raw = pair[(split + 1)..];
                // Only the value may use percent encoding; keys are exact and
                // unambiguous. No encoded delimiters, controls or double
                // decoding are ever treated as authority.
                if (!OAuthRequestKeys.Contains(key) || !args.TryAdd(key, raw) ||
                    raw.Length > 1024 || raw.Contains('+') ||
                    raw.Any(c => c <= 0x20 || c >= 0x7f || c == '#' || c == '\\' ||
                        c == '@')) return false;
                for (var i = 0; i < raw.Length; i++)
                {
                    if (raw[i] != '%') continue;
                    if (i + 2 >= raw.Length ||
                        !Uri.IsHexDigit(raw[i + 1]) || !Uri.IsHexDigit(raw[i + 2]))
                        return false;
                    i += 2;
                }
                var decoded = Uri.UnescapeDataString(raw);
                if (decoded.Length > 1024 ||
                    decoded.Any(c => char.IsControl(c) || c == '\\'))
                    return false;
            }
            // The request is just a browser navigation, not an OAuth result.
            // Native code never receives/logs the verifier, state or code.
            if (!args.TryGetValue("response_type", out var responseType) ||
                responseType != "code" ||
                !args.TryGetValue("code_challenge_method", out var method) ||
                method != "S256" ||
                !args.TryGetValue("client_id", out var client) ||
                client.Length is < 1 or > 128 ||
                !args.TryGetValue("state", out var state) ||
                state.Length is < 16 or > 256 ||
                !args.TryGetValue("code_challenge", out var challenge) ||
                challenge.Length is < 43 or > 128 ||
                !args.TryGetValue("redirect_uri", out var encodedRedirect))
                return false;
            var redirectText = Uri.UnescapeDataString(encodedRedirect);
            if (!Uri.TryCreate(redirectText, UriKind.Absolute, out var redirect) ||
                redirect.Scheme != Uri.UriSchemeHttps ||
                redirect.UserInfo.Length != 0 || redirect.Query.Length != 0 ||
                redirect.Fragment.Length != 0 || redirectText.Contains('\\') ||
                redirectText.Contains('@') ||
                !provider.Origins.Contains(redirect.GetLeftPart(UriPartial.Authority)))
                return false;
            return provider.CallbackUrls.Contains(redirectText);
        }

        public bool Allows(string providerId, string url)
        {
            if (!_providers.TryGetValue(providerId, out var provider) ||
                string.IsNullOrEmpty(url) || url.Length > 4096 ||
                url.Contains('\\') || url.Contains('@') ||
                !Uri.TryCreate(url, UriKind.Absolute, out var parsed) ||
                parsed.Scheme != Uri.UriSchemeHttps || parsed.UserInfo.Length != 0 ||
                parsed.Fragment.Length != 0 ||
                !string.Equals(parsed.AbsoluteUri, url, StringComparison.Ordinal) ||
                !provider.Origins.Contains(parsed.GetLeftPart(UriPartial.Authority)))
                return false;
            if (parsed.AbsolutePath.Contains('%') || parsed.AbsolutePath.Contains(';'))
                return false;
            if (parsed.Query.Length == 0) return true;
            return SafeOAuthRequest(provider, parsed.Query);
        }

        // These methods only parse host-configured flow transitions. The
        // provider page owns its OAuth request; none of these values are
        // credentials or proof of authentication.
        public bool TryAuthorizationRequest(string providerId, string url,
            out string state, out string callback)
        {
            state = callback = "";
            if (!Allows(providerId, url) ||
                !Uri.TryCreate(url, UriKind.Absolute, out var parsed) ||
                parsed.Query.Length == 0) return false;
            var query = parsed.Query[1..].Split('&')
                .Select(value => value.Split('=', 2))
                .ToDictionary(parts => parts[0], parts => parts[1],
                    StringComparer.Ordinal);
            state = Uri.UnescapeDataString(query["state"]);
            callback = Uri.UnescapeDataString(query["redirect_uri"]);
            return true;
        }

        public bool IsCallbackCandidate(string providerId, string url)
        {
            if (!_providers.TryGetValue(providerId, out var provider) ||
                !Uri.TryCreate(url, UriKind.Absolute, out var uri) ||
                uri.Scheme != Uri.UriSchemeHttps || uri.UserInfo.Length != 0)
                return false;
            return provider.CallbackUrls.Contains(uri.GetLeftPart(UriPartial.Path));
        }

        public bool TryCallback(string providerId, string url,
            out string state)
        {
            state = "";
            if (!IsCallbackCandidate(providerId, url) ||
                !Uri.TryCreate(url, UriKind.Absolute, out var parsed) ||
                parsed.Fragment.Length != 0 || parsed.Query.Length is < 2 or > 2049 ||
                url.Contains('\\') || url.Contains('@')) return false;
            var pieces = parsed.Query[1..].Split('&');
            if (pieces.Length != 2) return false;
            var values = new Dictionary<string, string>(StringComparer.Ordinal);
            foreach (var pair in pieces)
            {
                var split = pair.IndexOf('=');
                if (split <= 0 || split == pair.Length - 1) return false;
                var key = pair[..split];
                var value = pair[(split + 1)..];
                if (key is not ("state" or "code") ||
                    !values.TryAdd(key, value) || value.Length > 1024 ||
                    value.Contains('+') || value.Any(ch => ch <= 0x20 ||
                        ch >= 0x7f || ch == '#' || ch == '@' || ch == '\\'))
                    return false;
                for (var i = 0; i < value.Length; i++)
                {
                    if (value[i] != '%') continue;
                    if (i + 2 >= value.Length ||
                        !Uri.IsHexDigit(value[i + 1]) ||
                        !Uri.IsHexDigit(value[i + 2])) return false;
                    i += 2;
                }
            }
            if (!values.TryGetValue("state", out var encodedState) ||
                !values.TryGetValue("code", out var code) ||
                code.Length is < 1 or > 1024) return false;
            state = Uri.UnescapeDataString(encodedState);
            return state.Length is >= 16 and <= 256 &&
                state.All(ch => (ch >= 'a' && ch <= 'z') ||
                    (ch >= 'A' && ch <= 'Z') ||
                    (ch >= '0' && ch <= '9') || ch is '-' or '_');
        }

        public string PrimaryOrigin(string providerId)
        {
            if (!_providers.TryGetValue(providerId, out var provider))
                Deny("unknown-provider");
            return provider!.Origin;
        }

        public static void SelfTest()
        {
            const string config = """{"providers":[{"id":"chatgpt","origin":"https://chatgpt.com","redirectOrigins":["https://auth.openai.com"],"callbackUrls":["https://chatgpt.com/auth/callback"]},{"id":"gemini","origin":"https://gemini.google.com","redirectOrigins":[],"callbackUrls":[]}]}""";
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
            // Safe authorization-code + PKCE request is navigation-only.
            // Callback codes, bearer tokens, unsolicited state and open
            // redirect_uri hosts never pass the production origin gate.
            var query = "?response_type=code&client_id=public-web" +
                "&state=abcdefghijklmnop0123456789" +
                "&code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ" +
                "&code_challenge_method=S256" +
                "&redirect_uri=https%3A%2F%2Fchatgpt.com%2Fauth%2Fcallback";
            Check(p.Allows("chatgpt", "https://auth.openai.com/authorize" + query));
            Check(p.TryAuthorizationRequest("chatgpt",
                "https://auth.openai.com/authorize" + query,
                out var requestState, out var requestCallback) &&
                requestState == "abcdefghijklmnop0123456789" &&
                requestCallback == "https://chatgpt.com/auth/callback");
            Check(p.TryCallback("chatgpt",
                "https://chatgpt.com/auth/callback?code=one-use&state=abcdefghijklmnop0123456789",
                out var callbackState) &&
                callbackState == requestState);
            Check(!p.TryCallback("chatgpt",
                "https://chatgpt.com/auth/callback?access_token=evil&state=abcdefghijklmnop0123456789",
                out _));
            Check(!p.TryCallback("chatgpt",
                "https://chatgpt.com/auth/callback?code=one&state=abcdefghijklmnop0123456789&code=two",
                out _));

            foreach (var suffix in new[] {
                "?code=credential", "?access_token=secret", "?state=guess",
                "?response_type=token", query + "&code=credential",
                query.Replace("chatgpt.com%2Fauth", "attacker.test%2Fauth"),
                query.Replace("chatgpt.com%2Fauth%2Fcallback",
                    "chatgpt.com%2Fnot-an-allowed-callback"),
                query.Replace("state=abcdefghijklmnop0123456789", "state=x"),
                query.Replace("code_challenge_method=S256", "code_challenge_method=plain"),
                query + "&state=duplicate"
            }) Check(!p.Allows("chatgpt", "https://auth.openai.com/authorize" + suffix));
            Check(!p.Allows("unlisted", "https://chatgpt.com/"));
            foreach (var invalid in new[] {
                """{"providers":[]}""",
                """{"providers":[{"id":"bad","origin":"http://example.com","redirectOrigins":[]}]}""",
                """{"providers":[{"id":"bad","origin":"https://user@example.com","redirectOrigins":[]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com/path","redirectOrigins":[]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com/","redirectOrigins":[]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com.evil/","redirectOrigins":["http://example.com"]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[],"callbackUrls":["https://evil.example/callback"]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[],"callbackUrls":["http://example.com/callback"]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[],"callbackUrls":["https://example.com/"]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[],"callbackUrls":["https://example.com/callback?code=raw"]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[],"callbackUrls":["https://example.com/callback","https://example.com/callback"]}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[],"unsafe":true}]}""",
                """{"providers":[{"id":"bad","origin":"https://example.com","redirectOrigins":[]},{"id":"bad","origin":"https://example.com","redirectOrigins":[]}]}"""
            })
            {
                try { Parse(invalid); throw new InvalidOperationException("8P invalid config admitted"); }
                catch (UnauthorizedAccessException) { }
            }
        }
    }

    // Native, memory-only correlation for a provider-managed OAuth redirect.
    // A matching code callback is consumed ONCE and navigation is canceled:
    // no token exchange, browser-history persistence, account observation or
    // provider authentication is implied. All values remain private to host.
    private sealed class OAuthNavigationGate
    {
        private sealed record Pending(string Provider, string Profile,
            string Session, string Callback, ulong Epoch, DateTimeOffset Started);
        private readonly Dictionary<string, Pending> _pending =
            new(StringComparer.Ordinal);
        private readonly HashSet<string> _consumed = new(StringComparer.Ordinal);
        private static string Digest(string value) =>
            Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(
                System.Text.Encoding.UTF8.GetBytes(value)));

        public bool Begin(string provider, string profile, string session,
            string state, string callback, ulong epoch, DateTimeOffset now)
        {
            if (state.Length is < 16 or > 256 || epoch == 0 ||
                state.Any(ch => !((ch >= 'a' && ch <= 'z') ||
                    (ch >= 'A' && ch <= 'Z') ||
                    (ch >= '0' && ch <= '9') || ch is '-' or '_')))
                return false;
            Prune(now);
            var digest = Digest(state);
            // Never evict replay tombstones during this host session; when
            // exhausted, fail closed rather than permitting an old state again.
            if (_consumed.Contains(digest) || _pending.ContainsKey(digest) ||
                _pending.Count >= 16 || _consumed.Count >= 1024) return false;
            _pending.Add(digest, new Pending(provider, profile, session,
                callback, epoch, now));
            return true;
        }

        public bool Consume(string provider, string profile, string session,
            string callback, string state, ulong epoch, DateTimeOffset now)
        {
            Prune(now);
            var digest = Digest(state);
            if (!_pending.TryGetValue(digest, out var pending) ||
                _consumed.Contains(digest) ||
                !string.Equals(pending.Provider, provider, StringComparison.Ordinal) ||
                !string.Equals(pending.Profile, profile, StringComparison.Ordinal) ||
                !string.Equals(pending.Session, session, StringComparison.Ordinal) ||
                !string.Equals(pending.Callback, callback, StringComparison.Ordinal) ||
                epoch <= pending.Epoch) return false;
            _pending.Remove(digest);
            _consumed.Add(digest);
            return true;
        }

        public void InvalidateProfile(string profile)
        {
            foreach (var pair in _pending.ToArray())
                if (string.Equals(pair.Value.Profile, profile,
                    StringComparison.Ordinal))
                {
                    _pending.Remove(pair.Key);
                    _consumed.Add(pair.Key);
                }
        }

        private void Prune(DateTimeOffset now)
        {
            foreach (var pair in _pending.ToArray())
                if (now < pair.Value.Started ||
                    now - pair.Value.Started > TimeSpan.FromMinutes(5))
                {
                    _pending.Remove(pair.Key);
                    _consumed.Add(pair.Key);
                }
        }

        public static void SelfTest()
        {
            static void Check(bool value)
            {
                if (!value) throw new InvalidOperationException(
                    "8P OAuth one-use state regression");
            }
            var now = DateTimeOffset.UtcNow;
            var gate = new OAuthNavigationGate();
            const string state = "abcdefghijklmnop0123456789";
            const string callback = "https://chatgpt.com/auth/callback";
            Check(gate.Begin("chatgpt", "profile-A", "session-A", state,
                callback, 2, now));
            Check(!gate.Begin("chatgpt", "profile-A", "session-A", state,
                callback, 2, now));
            Check(!gate.Consume("chatgpt", "profile-B", "session-A", callback,
                state, 3, now));
            Check(!gate.Consume("chatgpt", "profile-A", "session-B", callback,
                state, 3, now));
            Check(!gate.Consume("gemini", "profile-A", "session-A", callback,
                state, 3, now));
            Check(!gate.Consume("chatgpt", "profile-A", "session-A",
                "https://chatgpt.com/evil", state, 3, now));
            Check(!gate.Consume("chatgpt", "profile-A", "session-A", callback,
                state, 2, now));
            Check(gate.Consume("chatgpt", "profile-A", "session-A", callback,
                state, 3, now));
            Check(!gate.Consume("chatgpt", "profile-A", "session-A", callback,
                state, 4, now));
            Check(!gate.Begin("chatgpt", "profile-A", "session-A", state,
                callback, 4, now));
            var other = new OAuthNavigationGate();
            Check(other.Begin("chatgpt", "profile-A", "session-A",
                "different_state_1234567890", callback, 1, now));
            Check(!other.Consume("chatgpt", "profile-A", "session-A", callback,
                "different_state_1234567890", 2, now.AddMinutes(6)));
        }
    }

    internal static void Run8PPolicyTests()
    {
        ProviderPolicy.SelfTest();
        OAuthNavigationGate.SelfTest();
        static void Verify(bool ok)
        {
            if (!ok) throw new InvalidOperationException(
                "8P native 8B profile/session handoff policy regression");
        }
        var first = new Native8PSessionSnapshot("8p-A",
            @"D:\\projects\\CoS-Rust\\a", "https://chatgpt.com",
            "session-A", 1, "doc-A", 1, 1, 1);
        var second = first with {
            Profile = "8p-B",
            ProfilePath = @"D:\\projects\\CoS-Rust\\b",
            SessionId = "session-B",
            DocumentId = "doc-B",
            ChannelId = 2
        };
        Verify(Eligible8PHandoff(first, second, "https://chatgpt.com"));
        Verify(!Eligible8PHandoff(first, first, "https://chatgpt.com"));
        Verify(!Eligible8PHandoff(first, second with {
            ProfilePath = first.ProfilePath
        }, "https://chatgpt.com"));
        Verify(!Eligible8PHandoff(first, second with {
            SessionId = first.SessionId
        }, "https://chatgpt.com"));
        Verify(!Eligible8PHandoff(first, second with {
            Origin = "https://auth.openai.com"
        }, "https://chatgpt.com"));
        Verify(!Eligible8PHandoff(first, second,
            "https://wrong.example"));
    }

    private void BuildProductionExtension()
    {
        var source = Path.Combine(_repo, "probes", "8p", "extension");
        var destination = Path.Combine(_root, "extension");
        Directory.CreateDirectory(destination);
        foreach (var file in new[] { "manifest.json", "sw.mjs" })
        {
            var input = Path.Combine(source, file);
            var output = Path.Combine(destination, file);
            RejectReparseAncestors(input);
            RejectReparseAncestors(output);
            if (File.Exists(output))
                throw new UnauthorizedAccessException("8P extension output is not fresh");
            File.Copy(input, output);
        }
    }

    private async Task Prove8PSessionAsync(Browser browser)
    {
        // Signed identity only: the 8P MV3 worker has no provider host grants
        // and exposes no snapshot, observation or browser tool API.
        var proof = await WorkerAsync(browser, Js("attestSession", new { }));
        await RustAllowAsync(new
        {
            op = "observe", host = Host(browser),
            channelId = browser.ChannelId,
            generation = browser.HostGeneration, sequence = 1,
            payload = proof.GetProperty("payload").GetString(),
            mac = proof.GetProperty("mac").GetString()
        });
    }

    private sealed record Native8PSessionSnapshot(
        string Profile, string ProfilePath, string Origin,
        string SessionId, ulong SessionEpoch, string DocumentId,
        ulong NavigationEpoch, ulong HostGeneration, ulong ChannelId);

    private static Native8PSessionSnapshot Snapshot8P(Browser browser)
    {
        if (!browser.Production8P || browser.Revoked ||
            browser.ChannelId == 0 || browser.ProductionOrigin is null ||
            browser.ProductionUrl is null || browser.WorkerSession is null ||
            browser.View.IsDisposed ||
            !string.Equals(browser.View.CoreWebView2.Source,
                browser.ProductionUrl, StringComparison.Ordinal) ||
            !string.Equals(browser.ProfilePath,
                Path.GetFullPath(browser.View.CoreWebView2.Profile.ProfilePath),
                StringComparison.OrdinalIgnoreCase))
            throw new UnauthorizedAccessException("8P handoff scope unbound");
        return new(browser.Profile, browser.ProfilePath,
            browser.ProductionOrigin, browser.SessionId, browser.SessionEpoch,
            browser.DocumentId, browser.NavigationEpoch, browser.HostGeneration,
            browser.ChannelId);
    }

    private static bool Same8P(Browser b, Native8PSessionSnapshot s)
    {
        try { return Snapshot8P(b) == s; }
        catch (UnauthorizedAccessException) { return false; }
    }

    private static bool Eligible8PHandoff(
        Native8PSessionSnapshot a, Native8PSessionSnapshot b,
        string primaryOrigin) =>
        a.Profile != b.Profile &&
        !string.Equals(a.ProfilePath,b.ProfilePath,StringComparison.OrdinalIgnoreCase) &&
        a.Origin == b.Origin && a.Origin == primaryOrigin &&
        a.SessionId != b.SessionId && a.SessionEpoch > 0 &&
        b.SessionEpoch > 0 &&
        a.SessionEpoch < 9_007_199_254_740_989UL &&
        b.SessionEpoch < 9_007_199_254_740_989UL;

    private async Task<bool> Execute8PHandoffProbeAsync(
        Browser a, Browser b, string primaryOrigin)
    {
        // Both profiles are host-created and MV3/Rust-bound. No page can
        // request this operation. Consent is a native default-No modal.
        var first = Snapshot8P(a);
        var second = Snapshot8P(b);
        if (!Eligible8PHandoff(first, second, primaryOrigin))
            throw new UnauthorizedAccessException("8P foreign/shared handoff");
        var decision = MessageBox.Show(this,
            "Transfer ONLY the CoSoS logical application session from profile A " +
            "to profile B? Browser logins, cookies, storage and files stay separated. " +
            "Both Rust channels will be retired. This action cannot be retried.",
            "8P-HOST one-use cross-profile handoff",
            MessageBoxButtons.YesNo, MessageBoxIcon.Warning,
            MessageBoxDefaultButton.Button2);
        if (decision != DialogResult.Yes) return false;
        if (!Same8P(a, first) || !Same8P(b, second))
            throw new UnauthorizedAccessException("8P handoff scope changed during consent");
        // Exactly one signed first-proof per native scope before the transaction.
        await Prove8PSessionAsync(a);
        await Prove8PSessionAsync(b);
        if (!Same8P(a, first) || !Same8P(b, second))
            throw new UnauthorizedAccessException("8P handoff proof raced navigation");
        var oldA = Host(a);
        var oldB = Host(b);
        var empty = "8p-empty-" + Guid.NewGuid().ToString("N");
        var newSourceEpoch = first.SessionEpoch + 1;
        var newTargetEpoch = Math.Max(first.SessionEpoch, second.SessionEpoch) + 1;
        RevokeAndRetire(a);
        RevokeAndRetire(b);
        await RequireRetirementAsync(a);
        await RequireRetirementAsync(b);
        // After retirement, C# must still own exactly the old document and
        // profile; failures keep both channels revoked (never retry/rollback
        // ambiguously committed Rust transactions).
        bool Stable(Browser v, Native8PSessionSnapshot s) =>
            v.Revoked && v.ChannelId == 0 &&
            v.SessionId == s.SessionId && v.SessionEpoch == s.SessionEpoch &&
            v.DocumentId == s.DocumentId &&
            v.NavigationEpoch == s.NavigationEpoch &&
            !v.View.IsDisposed &&
            string.Equals(v.ProductionOrigin,s.Origin,StringComparison.Ordinal) &&
            string.Equals(Path.GetFullPath(v.View.CoreWebView2.Profile.ProfilePath),
                s.ProfilePath, StringComparison.OrdinalIgnoreCase);
        if (!Stable(a, first) || !Stable(b, second))
            throw new UnauthorizedAccessException("8P handoff changed after retirement");
        var committed = await RustAllowAsync(new
        {
            op = "handoff", source = oldA, target = oldB,
            newSourceSessionId = empty,
            sourceNextSessionEpoch = newSourceEpoch,
            targetNextSessionEpoch = newTargetEpoch
        });
        if (!committed.GetProperty("committed").GetBoolean() ||
            !Stable(a, first) || !Stable(b, second))
            throw new UnauthorizedAccessException("8P native/Rust commit uncertain");
        // No await between second native precommit recheck and paired updates.
        a.SessionId = empty;
        a.SessionEpoch = newSourceEpoch;
        b.SessionId = first.SessionId;
        b.SessionEpoch = newTargetEpoch;
        var replayDenied = await RustDeniedAsync(new
        {
            op = "handoff", source = oldA, target = oldB,
            newSourceSessionId = empty,
            sourceNextSessionEpoch = newSourceEpoch,
            targetNextSessionEpoch = newTargetEpoch
        });
        if (!replayDenied)
            throw new UnauthorizedAccessException("8P handoff replay unexpectedly accepted");
        // The next proof MUST originate from a real new WebView2 document.
        // The caller navigates both profiles after dropping its native
        // transaction lock. Never fabricate a documentId/navigationEpoch.
        if (a.SessionId != empty || b.SessionId != first.SessionId ||
            a.SessionEpoch != newSourceEpoch ||
            b.SessionEpoch != newTargetEpoch ||
            a.ProfilePath == b.ProfilePath)
            throw new UnauthorizedAccessException("8P native handoff postcondition");
        return true;
    }

    private async Task Execute8PAsync()
    {
        // This opt-in source-owned navigation entrypoint is NOT the 7U1 sign-in
        // gate, an MV3/host rebind, or permission approval. No account data,
        // cookies, form input, history or tokens are read or exported here.
        if (_host8PConfig is null || _host8PProvider is null)
            throw new ArgumentException("8P requires --8p-config and --8p-provider");
        var providerId = _host8PProvider ??
            throw new ArgumentException("8P provider required");
        RejectReparseAncestors(_host8PConfig);
        if (!IsUnderProject(_host8PConfig) || !File.Exists(_host8PConfig))
            throw new UnauthorizedAccessException("8P configuration must be an existing D: host file");
        if (Directory.Exists(_root) && Directory.EnumerateFileSystemEntries(_root).Any())
            throw new IOException("8P needs a fresh isolated D: output directory");
        Directory.CreateDirectory(_root);
        VerifySourceArtifact();
        var policy = ProviderPolicy.Parse(File.ReadAllText(_host8PConfig));
        var oauth = new OAuthNavigationGate();
        var landing = policy.Landing(providerId); // rejects unlisted provider before navigation
        var decision = MessageBox.Show(this,
            "Navigate the isolated CoSoS profile to the configured provider " +
            providerId + "?\nNo sign-in, observation, or permission is authorized by this action.",
            "8P-HOST native provider consent", MessageBoxButtons.YesNo,
            MessageBoxIcon.Question, MessageBoxDefaultButton.Button2);
        if (decision != DialogResult.Yes) { Close(); return; }

        // This is a private host-to-installed-extension bootstrap. The probe
        // worker has NO provider host permission, page listener, content script,
        // or access to provider cookies/content. It only authenticates the
        // native Rust channel; actual signed-in 7U1 observation stays disabled.
        BuildProductionExtension();
        if (!File.Exists(_bridgePath))
            throw new FileNotFoundException("source-bound Rust bridge not installed");
        var psi = new System.Diagnostics.ProcessStartInfo(_bridgePath)
        {
            UseShellExecute = false, RedirectStandardInput = true,
            RedirectStandardOutput = true, RedirectStandardError = true,
            CreateNoWindow = true, WorkingDirectory = _root
        };
        psi.Environment["TEMP"] = _root;
        psi.Environment["TMP"] = _root;
        _bridge = System.Diagnostics.Process.Start(psi) ??
            throw new InvalidOperationException("8P Rust child process missing");
        var refused = 0;
        var oauthCallbacksConsumed = 0;
        var bound = 0;
        var handoffPassed = false;
        var handoffLocked = false;
        async Task<(Browser Browser, TaskCompletionSource<bool> Ready)> SetupProductionAsync(
            WebView2 view, string label, uint tab)
        {
            var profile = Path.Combine(_root, "8p-isolated-profile-" + label);
            RejectReparseAncestors(profile);
            var env = await CoreWebView2Environment.CreateAsync(null, profile,
                new CoreWebView2EnvironmentOptions {
                    AreBrowserExtensionsEnabled = true
                });
            await view.EnsureCoreWebView2Async(env);
            var effectiveProfile = Path.GetFullPath(view.CoreWebView2.Profile.ProfilePath);
            RejectReparseAncestors(effectiveProfile);
            if (!IsUnderProject(effectiveProfile))
                throw new UnauthorizedAccessException("foreign 8P profile");
            if ((await view.CoreWebView2.Profile.GetBrowserExtensionsAsync()).Count != 0)
                throw new UnauthorizedAccessException("8P profile must have no prior extensions");
            var extension = await view.CoreWebView2.Profile.AddBrowserExtensionAsync(
                Path.Combine(_root, "extension"));
            if (!extension.IsEnabled)
                throw new UnauthorizedAccessException("8P native MV3 extension disabled");
            var browser = new Browser
            {
                View = view, Profile = "8p-" + label,
                ProfilePath = effectiveProfile, ExtensionId = extension.Id,
                TabId = tab, SessionId = "8p-session-" + label,
                Production8P = true
            };
            _browsers.Add(view, browser);
            var rebindLock = new SemaphoreSlim(1, 1);
            var ready = new TaskCompletionSource<bool>(
                TaskCreationOptions.RunContinuationsAsynchronously);
            view.CoreWebView2.PermissionRequested += (_, e) =>
            {
                e.State = CoreWebView2PermissionState.Deny;
                e.SavesInProfile = false;
                e.Handled = true;
            };
            view.CoreWebView2.NewWindowRequested += (_, e) => e.Handled = true;
            view.CoreWebView2.ProcessFailed += (_, _) =>
            {
                RevokeAndRetire(browser);
                oauth.InvalidateProfile(browser.Profile);
                browser.ProductionUrl = null;
                browser.ProductionOrigin = null;
                ready.TrySetResult(false);
            };
            view.CoreWebView2.NavigationStarting += (_, e) =>
            {
                // Native modal consent must never admit reentrant navigation.
                if (handoffLocked) { e.Cancel = true; refused++; return; }
                RevokeAndRetire(browser);
                browser.ProductionUrl = null;
                browser.ProductionOrigin = null;
                browser.NavigationEpoch++;
                browser.DocumentId = "doc-" + Guid.NewGuid().ToString("N");
                if (policy.IsCallbackCandidate(providerId, e.Uri))
                {
                    var callbackPath = new Uri(e.Uri).GetLeftPart(UriPartial.Path);
                    if (policy.TryCallback(providerId, e.Uri,
                            out var callbackState) &&
                        oauth.Consume(providerId, browser.Profile,
                            browser.SessionId, callbackPath, callbackState,
                            browser.NavigationEpoch, DateTimeOffset.UtcNow))
                        oauthCallbacksConsumed++;
                    else
                        refused++;
                    e.Cancel = true; // never load the code-bearing page
                    return;
                }
                if (!policy.Allows(providerId, e.Uri))
                {
                    refused++;
                    oauth.InvalidateProfile(browser.Profile);
                    e.Cancel = true;
                    return;
                }
                if (policy.TryAuthorizationRequest(providerId, e.Uri,
                        out var state, out var callback))
                {
                    var currentUrl = view.CoreWebView2.Source;
                    if (!Uri.TryCreate(currentUrl, UriKind.Absolute,
                            out var current) ||
                        !string.Equals(current.GetLeftPart(UriPartial.Authority),
                            policy.PrimaryOrigin(providerId),
                            StringComparison.Ordinal) ||
                        !oauth.Begin(providerId, browser.Profile,
                            browser.SessionId, state, callback,
                            browser.NavigationEpoch, DateTimeOffset.UtcNow))
                    {
                        refused++;
                        e.Cancel = true;
                    }
                }
            };
            view.CoreWebView2.NavigationCompleted += async (_, e) =>
            {
                // Never allow a deferred event to rebind while the owner
                // modal/Rust two-profile transaction holds exclusive control.
                if (handoffLocked)
                {
                    RevokeAndRetire(browser);
                    ready.TrySetResult(false);
                    return;
                }
                if (!e.IsSuccess || view.IsDisposed)
                {
                    ready.TrySetResult(false);
                    return;
                }
                var epoch = browser.NavigationEpoch;
                var url = view.CoreWebView2.Source;
                if (!policy.Allows(providerId, url))
                {
                    RevokeAndRetire(browser);
                    ready.TrySetResult(false);
                    return;
                }
                await rebindLock.WaitAsync();
                try
                {
                    if (view.IsDisposed || browser.NavigationEpoch != epoch ||
                        !string.Equals(view.CoreWebView2.Source, url,
                            StringComparison.Ordinal)) return;
                    browser.ProductionUrl = url;
                    browser.ProductionOrigin =
                        new Uri(url).GetLeftPart(UriPartial.Authority);
                    var response = await BindAsync(browser, browser.SessionId);
                    if (!response.GetProperty("authenticated").GetBoolean() ||
                        browser.NavigationEpoch != epoch || browser.Revoked ||
                        browser.ChannelId == 0)
                        throw new UnauthorizedAccessException("8P stale worker");
                    bound++;
                    ready.TrySetResult(true);
                }
                catch
                {
                    RevokeAndRetire(browser);
                    ready.TrySetResult(false);
                }
                finally { rebindLock.Release(); }
            };
            view.Disposed += (_, _) => RevokeAndRetire(browser);
            return (browser, ready);
        }

        FormClosed += (_, _) =>
        {
            foreach (var item in _browsers.Values) RevokeAndRetire(item);
            try
            {
                if (_bridge is { HasExited: false })
                    _bridge.Kill(entireProcessTree: true);
            }
            catch (InvalidOperationException) { }
            finally { _bridge?.Dispose(); _bridge = null; }
        };
        var first = await SetupProductionAsync(_a, "A", 1);
        var second = _host8PHandoffProbe ?
            await SetupProductionAsync(_b, "B", 2) : default;
        _a.CoreWebView2.Navigate(landing);
        if (_host8PHandoffProbe) _b.CoreWebView2.Navigate(landing);
        var firstSettled = await Task.WhenAny(first.Ready.Task,
            Task.Delay(TimeSpan.FromSeconds(60)));
        var success = firstSettled == first.Ready.Task &&
            await first.Ready.Task;
        if (_host8PHandoffProbe)
        {
            var secondSettled = await Task.WhenAny(second.Ready!.Task,
                Task.Delay(TimeSpan.FromSeconds(60)));
            success = success && secondSettled == second.Ready.Task &&
                await second.Ready.Task;
            if (success)
            {
                handoffLocked = true; // blocks reentrant WebView2 nav during modal
                var oldA = first.Browser.ChannelId;
                var oldB = second.Browser!.ChannelId;
                var oldEpochA = first.Browser.NavigationEpoch;
                var oldEpochB = second.Browser.NavigationEpoch;
                try
                {
                    handoffPassed = await Execute8PHandoffProbeAsync(
                        first.Browser, second.Browser,
                        policy.PrimaryOrigin(providerId));
                }
                finally { handoffLocked = false; }
                if (handoffPassed)
                {
                    // Rebind on ACTUAL navigation events, not a synthetic
                    // document counter change. Both OAuth/profile partitions
                    // stay on their original OS WebView2 environments.
                    var startBound = bound;
                    _a.CoreWebView2.Navigate(landing);
                    _b.CoreWebView2.Navigate(landing);
                    for (var attempt = 0; attempt < 150 &&
                        bound < startBound + 2; attempt++)
                        await Task.Delay(100);
                    handoffPassed =
                        bound >= startBound + 2 &&
                        first.Browser.NavigationEpoch > oldEpochA &&
                        second.Browser.NavigationEpoch > oldEpochB &&
                        first.Browser.ChannelId != oldA &&
                        second.Browser.ChannelId != oldB &&
                        first.Browser.ChannelId != 0 &&
                        second.Browser.ChannelId != 0 &&
                        !first.Browser.Revoked && !second.Browser.Revoked &&
                        first.Browser.ProfilePath != second.Browser.ProfilePath &&
                        first.Browser.SessionId != second.Browser.SessionId;
                    if (handoffPassed)
                    {
                        await Prove8PSessionAsync(first.Browser);
                        await Prove8PSessionAsync(second.Browser);
                    }
                }
                success = handoffPassed;
            }
        }
        // Never include page URLs, account metadata, OAuth parameters, secrets
        // or worker key/challenge in a source-attested receipt.
        var receipt = new
        {
            sourceSha = _verifiedSourceSha, providerId = providerId,
            navigationAllowed = success, navigationEpoch = first.Browser.NavigationEpoch,
            deniedNavigationCount = refused, oauthCallbacksConsumed,
            nativeWorkerRebinds = bound, crossProfileHandoffPassed = handoffPassed,
            rustChannelActive = success && !first.Browser.Revoked &&
                first.Browser.ChannelId != 0,
            mv3WorkerAttested = success && !first.Browser.Revoked &&
                first.Browser.WorkerTargetId is not null &&
                first.Browser.WorkerSession is not null,
            permissionPolicy = "Deny (nonpersistent)", authenticated = false,
            native8PAccepted = false, owner7U1Accepted = false
        };
        File.WriteAllText(Path.Combine(_root, "8p-navigation-receipt.json"),
            JsonSerializer.Serialize(receipt, new JsonSerializerOptions { WriteIndented = true }));
        Environment.ExitCode = success ? 0 : 1;
        // Keep owner-controlled WebView2 open for HTTPS redirect navigation.
        // Each completed document must obtain a *different* Rust/MV3 channel.
        // No content observation, tool request or permission is enabled here.

    }
}
