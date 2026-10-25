using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;

namespace CoSoS.Probe7L1.WindowsHost;

// Dedicated product-owned 8L1 entrypoint. This code is linked into
// apps/windows-host, never loaded from the page or MV3 JS context.
internal sealed partial class GateForm
{
    private Native8L1Authority? _native8L1Authority;
    private int _native8L1ProcessFailures;

    // A real native ProcessFailed event invalidates every open 8C scope.
    // Retire on the Rust bridge as well, not only in the C# scope dictionary.
    private void On8L1ProcessFailure()
    {
        _native8L1ProcessFailures++;
        foreach (var browser in _browsers.Values.ToArray())
            RevokeAndRetire(browser);
    }

    // Native source authority; 8B/8C's JS controllers define the exact
    // profile/context/lifecycle contract. Only WebView2 host events and
    // the authenticated Rust/MV3 channel may create or change this state.
    private sealed class Native8L1Authority
    {
        private sealed record Binding(
            string ProfileId, string PartitionId, string ProfilePath,
            uint TabId, string DocumentId, ulong NavigationEpoch,
            ulong WorkerGeneration, ulong HostGeneration, ulong ChannelId,
            string SessionId, string Origin, ulong Sequence);

        private readonly Dictionary<Browser, Binding> _bindings = new(ReferenceEqualityComparer.Instance);
        private readonly Dictionary<string, (string Path, string Partition)> _profiles =
            new(StringComparer.Ordinal);
        private readonly HashSet<string> _consumedApprovals = new(StringComparer.Ordinal);
        // A modal owner consent dialog can pump native events. Serialize each
        // browser's permission request and reattest after the dialog closes.
        private readonly HashSet<Browser> _permissionBusy = new(ReferenceEqualityComparer.Instance);
        private readonly object _gate = new();

        private sealed record OneUseApproval(string Nonce, Binding Binding, string Capability);

        private static bool ExactOrigin(string requestUri, string trustedOrigin)
        {
            if (!Uri.TryCreate(requestUri, UriKind.Absolute, out var uri) ||
                uri.UserInfo.Length != 0 || uri.Fragment.Length != 0) return false;
            return string.Equals(uri.GetLeftPart(UriPartial.Authority),
                trustedOrigin, StringComparison.Ordinal);
        }

        // Shared by the actual WebView2 permission side effect and the cloud
        // contract self-test. Approval is tied to every 8B/8C scope field.
        private static bool Authorizes(OneUseApproval approval, Binding expected,
            Binding current, string capability, string requestUri) =>
            approval.Binding == expected && expected == current &&
            approval.Capability == capability && capability != "unsupported" &&
            ExactOrigin(requestUri, expected.Origin);

        // Equivalent to the accepted 8C suspended/navigated/rebound guard:
        // all authority generations must be fresh on renewal and partitions
        // cannot migrate under the same native profile/tab scope.
        private static bool FreshRebind(Binding prior, Binding next) =>
            prior.ProfileId == next.ProfileId &&
            prior.TabId == next.TabId &&
            prior.PartitionId == next.PartitionId &&
            string.Equals(prior.ProfilePath, next.ProfilePath, StringComparison.OrdinalIgnoreCase) &&
            prior.Origin == next.Origin &&
            next.HostGeneration > prior.HostGeneration &&
            next.NavigationEpoch > prior.NavigationEpoch &&
            next.DocumentId != prior.DocumentId;

        private static bool Consume(OneUseApproval approval, Binding expected,
            Binding current, string capability, string requestUri,
            HashSet<string> consumed) =>
            consumed.Add(approval.Nonce) &&
            Authorizes(approval, expected, current, capability, requestUri);

        public static void RunSelfTests()
        {
            static void Require(bool condition)
            {
                if (!condition) throw new InvalidOperationException("8L1 native contract regression");
            }
            var first = new Binding("profile-A", "partition-A", @"D:\projects\CoS-Rust\synthetic\A",
                1, "doc-1", 1, 1, 1, 1, "session-A", Origin, 1);
            var stale = first with { DocumentId = "doc-2", NavigationEpoch = 2 };
            var foreign = first with { ProfileId = "profile-B", PartitionId = "partition-B" };
            var consumed = new HashSet<string>(StringComparer.Ordinal);
            var grant = new OneUseApproval("native-owner-approval", first, "camera");
            Require(Consume(grant, first, first, "camera", Origin + "/index.html", consumed));
            Require(!Consume(grant, first, first, "camera", Origin, consumed));
            Require(!Consume(new OneUseApproval("stale", first, "camera"), first,
                stale, "camera", Origin, consumed));
            Require(!Consume(new OneUseApproval("foreign", first, "camera"), first,
                foreign, "camera", Origin, consumed));
            Require(!Consume(new OneUseApproval("wrong-kind", first, "camera"), first,
                first, "microphone", Origin, consumed));
            Require(!Consume(new OneUseApproval("origin", first, "camera"), first,
                first, "camera", "https://attacker.invalid", consumed));
            Require(!Consume(new OneUseApproval("unsupported", first, "unsupported"), first,
                first, "unsupported", Origin, consumed));
            Require(!ExactOrigin("https://synthetic.cos.test.evil.test/", Origin));
            Require(!ExactOrigin("http://synthetic.cos.test/", Origin));
            Require(!ExactOrigin("https://user@synthetic.cos.test/", Origin));
            Require(first.PartitionId != foreign.PartitionId);
            Require(!FreshRebind(first, first));
            Require(!FreshRebind(first, stale));
            Require(!FreshRebind(first, first with {
                HostGeneration = 2, NavigationEpoch = 2, DocumentId = "doc-2",
                PartitionId = "partition-replaced" }));
            Require(!FreshRebind(first, first with {
                HostGeneration = 2, NavigationEpoch = 2, DocumentId = "doc-2",
                ProfileId = "profile-B" }));
            Require(FreshRebind(first, first with {
                HostGeneration = 2, NavigationEpoch = 2, DocumentId = "doc-2" }));
            Console.WriteLine("8L1 native authority consent, replay, stale-scope and origin policy PASS");
        }

        private static string Capability(CoreWebView2PermissionKind kind) => kind switch
        {
            CoreWebView2PermissionKind.Camera => "camera",
            CoreWebView2PermissionKind.Microphone => "microphone",
            CoreWebView2PermissionKind.Geolocation => "geolocation",
            CoreWebView2PermissionKind.Notifications => "notifications",
            CoreWebView2PermissionKind.ClipboardRead => "clipboard-read",
            CoreWebView2PermissionKind.MultipleAutomaticDownloads => "downloads",
            _ => "unsupported"
        };

        public void Attach(Browser b)
        {
            // Actual WebView2 browser-side effect handlers: deny before
            // any renderer or extension can authorize a permission or popup.
            b.View.CoreWebView2.PermissionRequested += (_, e) =>
            {
                // Explicit state prevents a WebView2 default grant. Never
                // persist it in the profile: every request needs new consent.
                e.State = CoreWebView2PermissionState.Deny;
                e.SavesInProfile = false;
                e.Handled = true;
                var cap = Capability(e.PermissionKind);
                Binding? snapshot;
                lock (_gate)
                {
                    if (cap == "unsupported" || _permissionBusy.Contains(b) ||
                        !_bindings.TryGetValue(b, out snapshot) ||
                        !Matches(b, snapshot) || !ExactOrigin(e.Uri, snapshot.Origin))
                        return;
                    _permissionBusy.Add(b);
                }
                try
                {
                    // The actual native owner dialog is the sole grant source;
                    // no page, MV3 script or webmessage can supply approval.
                    var result = MessageBox.Show(
                        b.View.FindForm(),
                        "Allow ONE " + cap + " request?\nOrigin: " + snapshot.Origin +
                        "\nProfile: " + snapshot.ProfileId + "\nTab: " + snapshot.TabId +
                        "\nDefault is Deny. Permission will not be saved.",
                        "CoSoS native browser permission",
                        MessageBoxButtons.YesNo, MessageBoxIcon.Question,
                        MessageBoxDefaultButton.Button2);
                    if (result != DialogResult.Yes) return;
                    lock (_gate)
                    {
                        if (!_bindings.TryGetValue(b, out var current) ||
                            !Matches(b, current)) return;
                        var approval = new OneUseApproval(
                            Guid.NewGuid().ToString("N"), snapshot, cap);
                        if (_consumedApprovals.Count < 4096 &&
                            Consume(approval, snapshot, current, cap,
                                e.Uri, _consumedApprovals))
                            e.State = CoreWebView2PermissionState.Allow;
                    }
                }
                finally { lock (_gate) _permissionBusy.Remove(b); }
            };
            b.View.CoreWebView2.NewWindowRequested += (_, e) => e.Handled = true;
            b.View.CoreWebView2.WebMessageReceived += (_, _) =>
            {
                // No WebMessageReceived handler may mint authority.
            };
        }

        private static bool Matches(Browser b, Binding v) =>
            !b.View.IsDisposed && !b.Revoked && b.WorkerSession is not null &&
            b.Profile == v.ProfileId && b.TabId == v.TabId &&
            b.DocumentId == v.DocumentId &&
            b.NavigationEpoch == v.NavigationEpoch &&
            b.WorkerGeneration == v.WorkerGeneration &&
            b.HostGeneration == v.HostGeneration &&
            b.ChannelId == v.ChannelId && b.ChannelId > 0 &&
            b.SessionId == v.SessionId &&
            string.Equals(b.View.CoreWebView2.Source, v.Origin + "/index.html",
                StringComparison.Ordinal) &&
            string.Equals(Path.GetFullPath(b.View.CoreWebView2.Profile.ProfilePath),
                v.ProfilePath, StringComparison.OrdinalIgnoreCase);

        public void Track(Browser b)
        {
            lock (_gate)
            {
                if (b.Revoked || b.ChannelId == 0 || b.HostGeneration == 0 ||
                    b.WorkerGeneration == 0 || b.WorkerSession is null ||
                    !string.Equals(b.View.CoreWebView2.Source, Origin + "/index.html",
                        StringComparison.Ordinal))
                    throw new UnauthorizedAccessException("8L1 native channel not authenticated");
                var path = Path.GetFullPath(b.View.CoreWebView2.Profile.ProfilePath);
                if (!IsUnderProject(path)) throw new UnauthorizedAccessException("foreign profile root");
                RejectReparseAncestors(path);
                var conflicting = _profiles.Any(p => p.Key != b.Profile &&
                    string.Equals(p.Value.Path, path, StringComparison.OrdinalIgnoreCase));
                if (conflicting) throw new UnauthorizedAccessException("8B partition reused by other profile");
                if (!_profiles.TryGetValue(b.Profile, out var owner))
                {
                    if (_profiles.Count >= 64) throw new UnauthorizedAccessException("profile cap");
                    owner = (path, "partition-" + Guid.NewGuid().ToString("N"));
                    _profiles.Add(b.Profile, owner);
                }
                else if (!string.Equals(owner.Path, path, StringComparison.OrdinalIgnoreCase))
                    throw new UnauthorizedAccessException("8B partition rebound");
                if (_bindings.Count >= 256 && !_bindings.ContainsKey(b))
                    throw new UnauthorizedAccessException("8C frame capacity");
                var prior = _bindings.GetValueOrDefault(b);
                var next = new Binding(b.Profile, owner.Partition, path,
                    b.TabId, b.DocumentId, b.NavigationEpoch,
                    b.WorkerGeneration, b.HostGeneration, b.ChannelId,
                    b.SessionId, Origin, (prior?.Sequence ?? 0) + 1);
                if (prior is not null && !FreshRebind(prior, next))
                    throw new UnauthorizedAccessException("8C stale or cross-partition rebind");
                _bindings[b] = next;
            }
        }

        public bool Current(Browser b)
        {
            lock (_gate)
                return _bindings.TryGetValue(b, out var binding) && Matches(b, binding);
        }

        public bool SamePartition(Browser a, Browser b)
        {
            lock (_gate)
                return _bindings.TryGetValue(a, out var left) &&
                    _bindings.TryGetValue(b, out var right) &&
                    left.PartitionId == right.PartitionId;
        }

        public bool RejectForeign(Browser source, Browser foreign)
        {
            lock (_gate)
                return _bindings.TryGetValue(source, out var identity) &&
                    _bindings.TryGetValue(foreign, out var other) &&
                    identity.ProfileId != other.ProfileId &&
                    identity.PartitionId != other.PartitionId &&
                    !Matches(foreign, identity);
        }

        public void Revoke(Browser b)
        {
            lock (_gate)
            {
                _permissionBusy.Remove(b);
                // Retain the old binding as a tombstone so the next Track
                // requires a strictly newer authenticated host generation.
            }
        }

        public bool PermissionDefaultDeny()
        {
            lock (_gate) return _permissionBusy.Count == 0;
        }
    }

    private async Task Execute8L1Async()
    {
        var exit = 2;
        var rustExit = -1;
        var rustPid = 0;
        try
        {
            if (Directory.Exists(_root) && Directory.EnumerateFileSystemEntries(_root).Any())
                throw new IOException("8L1 requires an empty isolated D: run directory");
            Directory.CreateDirectory(_root);
            VerifySourceArtifact();
            BuildExtension();
            _native8L1Authority = new Native8L1Authority();
            if (!File.Exists(_bridgePath))
                throw new FileNotFoundException("source-bound Rust/MV3 bridge not installed");
            var psi = new ProcessStartInfo(_bridgePath)
            {
                UseShellExecute = false, RedirectStandardInput = true,
                RedirectStandardOutput = true, RedirectStandardError = true,
                CreateNoWindow = true, WorkingDirectory = _root
            };
            psi.Environment["TEMP"] = _root;
            psi.Environment["TMP"] = _root;
            _bridge = Process.Start(psi) ?? throw new InvalidOperationException("Rust 7L2 bridge unavailable");
            rustPid = _bridge.Id;
            var a = await SetupAsync(_a, "A", 1);
            var b = await SetupAsync(_b, "B", 2);
            await BindAsync(a, "8l1-synthetic-a");
            await BindAsync(b, "8l1-synthetic-b");
            Mark("native_rust_mv3_authenticated",
                Ok(await ObserveAsync(a)) && Ok(await ObserveAsync(b)),
                "Two independent real WebView2 MV3 workers authenticated by existing 7L2 Rust bridge");
            _native8L1Authority.Track(a);
            _native8L1Authority.Track(b);
            Mark("native_8b_independent_partitions",
                _native8L1Authority.Current(a) && _native8L1Authority.Current(b) &&
                !_native8L1Authority.SamePartition(a, b),
                "Distinct browser-created OS profile paths and host-minted opaque partition handles");
            Mark("native_8c_foreign_profile_denial",
                _native8L1Authority.RejectForeign(a,b) &&
                _native8L1Authority.RejectForeign(b,a),
                "Privileged native target identity and opaque storage partition cannot cross profiles");
            Mark("native_permission_default_deny",
                _native8L1Authority.PermissionDefaultDeny(),
                "Real native permission side effect default-deny; each owner approval is one-use and not persisted");
            // Two distinct real WebView2 controllers share the OS profile,
            // extension storage and opaque 8B partition, with separate tabs.
            var rival = await SetupAsync(_sameProfileTab, "A", 3,
                _a.CoreWebView2.Environment, a.ExtensionId);
            var actualSharedProfile = rival.ProfilePath == a.ProfilePath &&
                rival.ExtensionId == a.ExtensionId && rival.TabId != a.TabId &&
                !ReferenceEquals(rival.View.CoreWebView2, a.View.CoreWebView2);
            rival.HostGeneration = a.HostGeneration;
            var rivalHandshake = await BindAsync(rival, "8l1-synthetic-a");
            _native8L1Authority.Track(rival);
            Mark("native_multitab_same_partition",
                actualSharedProfile && rivalHandshake.GetProperty("authenticated").GetBoolean() &&
                _native8L1Authority.Current(rival) &&
                _native8L1Authority.SamePartition(a, rival) &&
                !_native8L1Authority.SamePartition(b, rival),
                "Two live WebView2 tabs share profile A only; distinct profile B cannot share partition");
            var rivalObservation = await ObserveAsync(rival);
            Mark("native_multitab_conflicting_claim_denied",
                !rivalObservation.GetProperty("ok").GetBoolean() &&
                rivalObservation.GetProperty("error").GetString() ==
                    "a session is already attributed to another browser scope",
                "Rust 7A refuses two tabs claiming the same application session");
            _sameProfileTab.Dispose();
            var rivalRetired = rival.RetirementTask is not null && await rival.RetirementTask;
            Mark("native_multitab_close_retires_channel",
                rivalRetired && !_native8L1Authority.Current(rival) &&
                _native8L1Authority.Current(b),
                "Closing one physical tab retires its own Rust channel without revoking profile B");
            // Rebind an existing native scope only after a real navigation,
            // fresh WebView2 document and confirmed retirement of its old Rust channel.
            _a.CoreWebView2.Navigate(StartUrl + "?after-rival-close=1");
            await Task.Delay(300);
            _a.CoreWebView2.Navigate(StartUrl);
            await WaitPageAsync(a);
            a.HostGeneration = Math.Max(a.HostGeneration, rival.HostGeneration);
            await BindAsync(a, "8l1-synthetic-a");
            _native8L1Authority.Track(a);
            var oldChannel = a.ChannelId;
            _a.CoreWebView2.Navigate(StartUrl + "?new-document=1");
            await Task.Delay(300);
            var retired = a.RetirementTask is not null && await a.RetirementTask;
            Mark("native_navigation_revokes",
                retired && !_native8L1Authority.Current(a) && oldChannel != 0,
                "Actual WebView2 navigation revokes prior native identity and Rust channel");
            _a.CoreWebView2.Navigate(StartUrl);
            await WaitPageAsync(a);
            await BindAsync(a, "8l1-synthetic-a");
            _native8L1Authority.Track(a);
            Mark("native_rebind_fresh_generation", _native8L1Authority.Current(a) &&
                a.ChannelId != oldChannel,
                "Rebind requires a fresh Rust/MV3 channel, native document and generation");
            Mark("native_prior_profile_preserved",
                _native8L1Authority.Current(b) && !_native8L1Authority.SamePartition(a,b),
                "Navigating one real WebView2 did not steal a separate browser-owned partition");
            // 8L1 physical full gate is still NOT_RUN until restart/crash,
            // one-use real consent and multi-tab are independently exercised.
            Pending("native_host_process_restart",
                "OS process restart and browser crash/recovery require separate physical Windows proof");
            Pending("native_browser_process_failed",
                "Native ProcessFailed revokes ALL active Rust channels; physical crash event not injected");
            Mark("native_browser_failure_listener_installed",
                _native8L1ProcessFailures == 0 && _native8L1Authority.Current(b),
                "Real WebView2 ProcessFailed callback is wired to host-wide Rust channel retirement");
            Pending("native_permission_positive_consent",
                "Native owner Yes/No prompt implemented; physical one-use WebView2 permission grant/deny interaction not yet exercised");
            exit = _failed ? 1 : 0;
        }
        catch (Exception ex)
        {
            Mark("fatal", false, ex.GetType().Name + ": " + ex.Message);
            exit = 1;
        }
        finally
        {
            _native8L1Authority = null;
            if (_bridge is not null)
            {
                try
                {
                    _bridge.StandardInput.Close();
                    if (!_bridge.WaitForExit(5000)) _bridge.Kill(entireProcessTree:true);
                    _bridge.WaitForExit(5000);
                    rustExit = _bridge.ExitCode;
                }
                catch (Exception ex)
                {
                    Mark("rust_process_exit", false, ex.GetType().Name);
                    exit = 1;
                }
                _bridge.Dispose();
            }
            if (rustExit != 0) { Mark("rust_process_exit", false, "Exit=" + rustExit); exit = 1; }
            else Mark("rust_process_exit", true, "Native Rust child exit=0");
            Directory.CreateDirectory(_root);
            File.WriteAllText(Path.Combine(_root, "receipt.json"),
                JsonSerializer.Serialize(new {
                    scope = "8L1-HOST limited real native integration; synthetic profiles only",
                    dateUtc = DateTimeOffset.UtcNow, sourceSha = _verifiedSourceSha ?? "NOT_RUN",
                    hostBinarySha256 = HashFile(typeof(GateForm).Assembly.Location),
                    rustBinarySha256 = File.Exists(_bridgePath) ? HashFile(_bridgePath) : "missing",
                    native8L1Accepted = false, cloudIntegrationPartial = exit == 0,
                    hostProcessId = Environment.ProcessId, rustProcessId = rustPid,
                    rustProcessExit = rustExit, checks = _checks
                }, new JsonSerializerOptions { WriteIndented = true }));
            Environment.ExitCode = exit;
            BeginInvoke(new Action(Close));
        }
    }
}
