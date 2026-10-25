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
            if (_consumed.Contains(digest) || _pending.ContainsKey(digest) ||
                _pending.Count >= 16) return false;
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
        a.Profile != b.Profile && a.ProfilePath != b.ProfilePath &&
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
        await BindAsync(a, a.SessionId);
        await BindAsync(b, b.SessionId);
        await Prove8PSessionAsync(a);
        await Prove8PSessionAsync(b);
        if (a.SessionId != empty || b.SessionId != first.SessionId ||
            a.SessionEpoch != newSourceEpoch ||
            b.SessionEpoch != newTargetEpoch ||
            a.ChannelId == first.ChannelId || b.ChannelId == second.ChannelId ||
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
        RejectReparseAncestors(_host8PConfig);
        if (!IsUnderProject(_host8PConfig) || !File.Exists(_host8PConfig))
            throw new UnauthorizedAccessException("8P configuration must be an existing D: host file");
        if (Directory.Exists(_root) && Directory.EnumerateFileSystemEntries(_root).Any())
            throw new IOException("8P needs a fresh isolated D: output directory");
        Directory.CreateDirectory(_root);
        VerifySourceArtifact();
        var policy = ProviderPolicy.Parse(File.ReadAllText(_host8PConfig));
        var oauth = new OAuthNavigationGate();
        var landing = policy.Landing(_host8PProvider); // rejects unlisted provider before navigation
        var decision = MessageBox.Show(this,
            "Navigate the isolated CoSoS profile to the configured provider " +
            _host8PProvider + "?\nNo sign-in, observation, or permission is authorized by this action.",
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
        var profile = Path.Combine(_root, "8p-isolated-profile");
        RejectReparseAncestors(profile);
        var env = await CoreWebView2Environment.CreateAsync(null, profile,
            new CoreWebView2EnvironmentOptions { AreBrowserExtensionsEnabled = true });
        await _a.EnsureCoreWebView2Async(env);
        var effectiveProfile = Path.GetFullPath(_a.CoreWebView2.Profile.ProfilePath);
        RejectReparseAncestors(effectiveProfile);
        if (!IsUnderProject(effectiveProfile))
            throw new UnauthorizedAccessException("foreign 8P profile");
        if ((await _a.CoreWebView2.Profile.GetBrowserExtensionsAsync()).Count != 0)
            throw new UnauthorizedAccessException("8P profile must have no prior extensions");
        var extension = await _a.CoreWebView2.Profile.AddBrowserExtensionAsync(
            Path.Combine(_root, "extension"));
        if (!extension.IsEnabled)
            throw new UnauthorizedAccessException("8P native MV3 extension disabled");
        var browser = new Browser
        {
            View = _a, Profile = "8p-" + _host8PProvider,
            ProfilePath = effectiveProfile, ExtensionId = extension.Id,
            TabId = 1, SessionId = "8p-unattributed", Production8P = true
        };
        _browsers.Add(_a, browser);
        var rebindLock = new SemaphoreSlim(1, 1);
        var refused = 0;
        var oauthCallbacksConsumed = 0;
        var bound = 0;
        var completed = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        _a.CoreWebView2.PermissionRequested += (_, e) =>
        {
            e.State = CoreWebView2PermissionState.Deny;
            e.SavesInProfile = false;
            e.Handled = true;
        };
        _a.CoreWebView2.NewWindowRequested += (_, e) => e.Handled = true;
        _a.CoreWebView2.ProcessFailed += (_, _) =>
        {
            RevokeAndRetire(browser); // failure cannot inherit live Rust proof
            oauth.InvalidateProfile(browser.Profile);
            browser.ProductionUrl = null;
            browser.ProductionOrigin = null;
            completed.TrySetResult(false);
        };
        _a.CoreWebView2.NavigationStarting += (_, e) =>
        {
            // Synchronous local revocation precedes any document navigation.
            // Rust retirement must finish before BindAsync can allocate a new
            // channel; no async callback can revive the previous page.
            RevokeAndRetire(browser);
            browser.ProductionUrl = null;
            browser.ProductionOrigin = null;
            browser.NavigationEpoch++;
            browser.DocumentId = "doc-" + Guid.NewGuid().ToString("N");
            // The callback may carry a real authorization code. Consume the
            // host-bound state at most once, but ALWAYS cancel browser
            // navigation: code must never be loaded into a page or logged.
            if (policy.IsCallbackCandidate(_host8PProvider, e.Uri))
            {
                var callbackPath = new Uri(e.Uri).GetLeftPart(UriPartial.Path);
                if (policy.TryCallback(_host8PProvider, e.Uri,
                        out var callbackState) &&
                    oauth.Consume(_host8PProvider, browser.Profile,
                        browser.SessionId, callbackPath, callbackState,
                        browser.NavigationEpoch, DateTimeOffset.UtcNow))
                    oauthCallbacksConsumed++;
                else
                    refused++;
                e.Cancel = true;
                return;
            }
            if (!policy.Allows(_host8PProvider, e.Uri))
            {
                refused++;
                oauth.InvalidateProfile(browser.Profile);
                e.Cancel = true;
                return;
            }
            if (policy.TryAuthorizationRequest(_host8PProvider, e.Uri,
                    out var state, out var callback))
            {
                // Native event source must currently be the selected provider;
                // an arbitrary untrusted window may not seed callback state.
                if (!string.Equals(
                        new Uri(_a.CoreWebView2.Source).GetLeftPart(UriPartial.Authority),
                        policy.PrimaryOrigin(_host8PProvider), StringComparison.Ordinal) ||
                    !oauth.Begin(_host8PProvider, browser.Profile,
                        browser.SessionId, state, callback,
                        browser.NavigationEpoch, DateTimeOffset.UtcNow))
                {
                    refused++;
                    e.Cancel = true;
                }
            }
        };
        _a.CoreWebView2.NavigationCompleted += async (_, e) =>
        {
            if (!e.IsSuccess || _a.IsDisposed)
            {
                completed.TrySetResult(false);
                return;
            }
            var epoch = browser.NavigationEpoch;
            var url = _a.CoreWebView2.Source;
            if (!policy.Allows(_host8PProvider, url))
            {
                RevokeAndRetire(browser);
                completed.TrySetResult(false);
                return;
            }
            await rebindLock.WaitAsync();
            try
            {
                if (_a.IsDisposed || browser.NavigationEpoch != epoch ||
                    !string.Equals(_a.CoreWebView2.Source, url,
                        StringComparison.Ordinal)) return;
                browser.ProductionUrl = url; // webview-owned committed document
                browser.ProductionOrigin = new Uri(url).GetLeftPart(UriPartial.Authority);
                var response = await BindAsync(browser, browser.SessionId);
                var authenticated = response.GetProperty("authenticated").GetBoolean();
                if (!authenticated || browser.NavigationEpoch != epoch ||
                    browser.Revoked || browser.ChannelId == 0)
                    throw new UnauthorizedAccessException("8P stale or unbound worker");
                bound++;
                completed.TrySetResult(true);
            }
            catch
            {
                RevokeAndRetire(browser);
                completed.TrySetResult(false);
            }
            finally
            {
                rebindLock.Release();
            }
        };
        _a.Disposed += (_, _) => RevokeAndRetire(browser);
        FormClosed += (_, _) =>
        {
            RevokeAndRetire(browser);
            try
            {
                if (_bridge is { HasExited: false })
                    _bridge.Kill(entireProcessTree: true);
            }
            catch (InvalidOperationException) { }
            finally { _bridge?.Dispose(); _bridge = null; }
        };
        _a.CoreWebView2.Navigate(landing);
        var settled = await Task.WhenAny(
            completed.Task, Task.Delay(TimeSpan.FromSeconds(60)));
        var success = settled == completed.Task && await completed.Task;
        // Never include page URLs, account metadata, OAuth parameters, secrets
        // or worker key/challenge in a source-attested receipt.
        var receipt = new
        {
            sourceSha = _verifiedSourceSha, providerId = _host8PProvider,
            navigationAllowed = success, navigationEpoch = browser.NavigationEpoch,
            deniedNavigationCount = refused, oauthCallbacksConsumed,
            nativeWorkerRebinds = bound,
            rustChannelActive = success && !browser.Revoked && browser.ChannelId != 0,
            mv3WorkerAttested = success && !browser.Revoked &&
                browser.WorkerTargetId is not null && browser.WorkerSession is not null,
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
