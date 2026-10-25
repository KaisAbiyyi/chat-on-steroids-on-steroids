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
    private string _native8L1MarkerA = "";
    private string _native8L1MarkerB = "";
    private string _native8L1ProfilePathA = "";
    private string _native8L1ProfilePathB = "";
    private string _native8L1ExtensionA = "";
    private string _native8L1ExtensionB = "";

    // Synthetic-only storage sentinel. This is NOT authority: only real
    // WebView2 profile isolation and new Rust/MV3 attestation confer it.
    private async Task<string> ReadRecoveryMarkerAsync(Browser browser)
    {
        var expression = _reuse8L1Profile
            ? "localStorage.getItem('cos8l1-synthetic-recovery')"
            : "(() => { const key='cos8l1-synthetic-recovery'; " +
              "if (localStorage.getItem(key)) throw Error('not a clean profile'); " +
              "const value=crypto.randomUUID(); localStorage.setItem(key,value); return value; })()";
        var raw = await browser.View.CoreWebView2.ExecuteScriptAsync(expression);
        return JsonSerializer.Deserialize<string>(raw) ?? "";
    }

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
        public sealed record Binding(
            string ProfileId, string PartitionId, string ProfilePath,
            uint TabId, string DocumentId, ulong NavigationEpoch,
            ulong WorkerGeneration, ulong HostGeneration, ulong ChannelId,
            string SessionId, ulong SessionEpoch, string Origin, ulong Sequence);

        private readonly Dictionary<Browser, Binding> _bindings = new(ReferenceEqualityComparer.Instance);
        private readonly Dictionary<string, (string Path, string Partition)> _profiles =
            new(StringComparer.Ordinal);
        private readonly HashSet<string> _consumedApprovals = new(StringComparer.Ordinal);
        private readonly HashSet<string> _consumedHandoffs = new(StringComparer.Ordinal);
        // A modal owner consent dialog can pump native events. Serialize each
        // browser's permission request and reattest after the dialog closes.
        private readonly HashSet<Browser> _permissionBusy = new(ReferenceEqualityComparer.Instance);
        // Hold exclusive 8B profile ownership across modal consent, Rust
        // retirement/commit and fresh WebView2 rebind. Consent alone is not a
        // transaction lock because MessageBox pumps native events.
        private readonly HashSet<string> _profileActionBusy = new(StringComparer.Ordinal);
        private readonly HashSet<Browser> _armStaleOnNextEvent = new(ReferenceEqualityComparer.Instance);
        private readonly object _gate = new();
        private bool _armReplayOnNextEvent;
        private OneUseApproval? _priorGrant;
        private int _ownerPrompts, _ownerGrants, _ownerDenials;
        private int _permissionEvents, _replayDenied, _staleAllowDenied;

        public (int Prompts, int Grants, int Denials, int Events, int ReplayDenied,
            int StaleAllowDenied) OwnerConsentCounts()
        {
            lock (_gate) return (_ownerPrompts, _ownerGrants, _ownerDenials,
                _permissionEvents, _replayDenied, _staleAllowDenied);
        }

        // Test-only arms: these cannot grant a permission; an actual
        // WebView2 PermissionRequested event is still necessary to test them.
        public void ArmReplayProbe()
        {
            lock (_gate)
            {
                if (_priorGrant is null) throw new InvalidOperationException("No owner grant to replay");
                _armReplayOnNextEvent = true;
            }
        }
        public void ArmStaleProbe(Browser browser)
        {
            lock (_gate) _armStaleOnNextEvent.Add(browser);
        }

        private sealed record OneUseApproval(string Nonce, Binding Binding, string Capability);

        private static bool ReserveProfiles(HashSet<string> busy, string[] ids)
        {
            if (ids.Length == 0 || ids.Distinct(StringComparer.Ordinal).Count() != ids.Length ||
                ids.Any(id => string.IsNullOrWhiteSpace(id) || busy.Contains(id)))
                return false;
            foreach (var id in ids) busy.Add(id);
            return true;
        }

        private sealed class ProfileActionLease : IDisposable
        {
            private readonly Native8L1Authority _owner;
            private readonly string[] _profiles;
            private bool _disposed;
            public ProfileActionLease(Native8L1Authority owner, string[] profiles)
            {
                _owner = owner;
                _profiles = profiles;
            }
            public void Dispose()
            {
                lock (_owner._gate)
                {
                    if (_disposed) return;
                    foreach (var id in _profiles) _owner._profileActionBusy.Remove(id);
                    _disposed = true;
                }
            }
        }

        public IDisposable ClaimProfileActions(params Browser[] browsers)
        {
            lock (_gate)
            {
                if (browsers.Length == 0 || browsers.Any(browser =>
                    !_bindings.TryGetValue(browser, out var binding) ||
                    !Matches(browser, binding) || _permissionBusy.Contains(browser)))
                    throw new UnauthorizedAccessException("8B profile action requires current native proof");
                var ids = browsers.Select(browser => _bindings[browser].ProfileId).ToArray();
                if (!ReserveProfiles(_profileActionBusy, ids))
                    throw new UnauthorizedAccessException("8B profile action already in progress");
                return new ProfileActionLease(this, ids);
            }
        }

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
            next.WorkerGeneration >= prior.WorkerGeneration &&
            next.SessionId == prior.SessionId &&
            next.SessionEpoch == prior.SessionEpoch &&
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
                1, "doc-1", 1, 1, 1, 1, "session-A", 1, Origin, 1);
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
            var busy = new HashSet<string>(StringComparer.Ordinal);
            Require(ReserveProfiles(busy, new[] { "profile-A", "profile-B" }));
            Require(!ReserveProfiles(busy, new[] { "profile-B", "profile-C" }));
            Require(!busy.Contains("profile-C")); // failed reservation is atomic
            Require(!ReserveProfiles(busy, new[] { "profile-C", "profile-C" }));
            busy.Remove("profile-A");
            busy.Remove("profile-B");
            Require(ReserveProfiles(busy, new[] { "profile-B" }));
            var second = first with {
                ProfileId = "profile-B", PartitionId = "partition-B",
                ProfilePath = @"D:\projects\CoS-Rust\synthetic\B",
                TabId = 2, SessionId = "session-B"
            };
            Require(CanHandoff(first, second));
            Require(!CanHandoff(first, first));
            Require(!CanHandoff(first, second with { PartitionId = first.PartitionId }));
            Require(!CanHandoff(first, second with { Origin = "https://foreign.invalid" }));
            Require(!CanHandoff(first, second with { SessionId = first.SessionId }));
            Require(!CanHandoff(first, second with { ProfilePath = first.ProfilePath }));
            Require(!FreshRebind(first, first));
            Require(!FreshRebind(first, stale));
            Require(!FreshRebind(first, first with {
                HostGeneration = 2, NavigationEpoch = 2, DocumentId = "doc-2",
                PartitionId = "partition-replaced" }));
            Require(!FreshRebind(first, first with {
                HostGeneration = 2, NavigationEpoch = 2, DocumentId = "doc-2",
                ProfileId = "profile-B" }));
            Require(!FreshRebind(first, first with {
                HostGeneration = 2, NavigationEpoch = 2, DocumentId = "doc-2",
                SessionId = "stolen-session" }));
            Require(!FreshRebind(first, first with {
                HostGeneration = 2, NavigationEpoch = 2, DocumentId = "doc-2",
                WorkerGeneration = 0 }));
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
                bool forceStale;
                lock (_gate)
                {
                    _permissionEvents++;
                    if (cap == "unsupported" || _permissionBusy.Contains(b) ||
                        !_bindings.TryGetValue(b, out snapshot) ||
                        _profileActionBusy.Contains(snapshot.ProfileId) ||
                        !Matches(b, snapshot) || !ExactOrigin(e.Uri, snapshot.Origin))
                        return;
                    if (_armReplayOnNextEvent)
                    {
                        _armReplayOnNextEvent = false;
                        var previous = _priorGrant;
                        if (previous is not null && !Consume(previous, previous.Binding,
                            snapshot, cap, e.Uri, _consumedApprovals))
                            _replayDenied++;
                        // This real WebView2 request stays Deny. No new dialog.
                        return;
                    }
                    forceStale = _armStaleOnNextEvent.Remove(b);
                    _permissionBusy.Add(b);
                    _ownerPrompts++;
                }
                try
                {
                    if (forceStale)
                    {
                        // WinForms modal MessageBox pumps this posted native
                        // navigation, revoking the exact old document scope.
                        b.View.BeginInvoke(new Action(() =>
                        {
                            if (!b.View.IsDisposed)
                                b.View.CoreWebView2.Navigate(StartUrl);
                        }));
                    }
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
                    if (result != DialogResult.Yes)
                    {
                        lock (_gate) _ownerDenials++;
                        return;
                    }
                    lock (_gate)
                    {
                        if (!_bindings.TryGetValue(b, out var current) ||
                            _profileActionBusy.Contains(current.ProfileId) ||
                            !Matches(b, current))
                        {
                            if (forceStale) _staleAllowDenied++;
                            return;
                        }
                        if (forceStale) return; // probe never grants without proven revocation
                        var approval = new OneUseApproval(
                            Guid.NewGuid().ToString("N"), snapshot, cap);
                        if (_consumedApprovals.Count < 4096 &&
                            Consume(approval, snapshot, current, cap,
                                e.Uri, _consumedApprovals))
                        {
                            e.State = CoreWebView2PermissionState.Allow;
                            _priorGrant = approval;
                            _ownerGrants++;
                        }
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
            b.SessionId == v.SessionId && b.SessionEpoch == v.SessionEpoch &&
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
                    b.SessionId, b.SessionEpoch, Origin, (prior?.Sequence ?? 0) + 1);
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
                // Keep an in-flight modal request serialized until its
                // handler unwinds. Removing busy here would permit a second
                // dialog after navigation/revocation while the first remains
                // open; the stale handler itself always denies on recheck.
                // Retain the old binding as a tombstone so the next Track
                // requires a strictly newer authenticated host generation.
            }
        }

        // 8B synthetic provider navigation: this confirms consent to START
        // navigation, never an authenticated account or an imported cookie.
        public bool ApproveSyntheticSignIn(Browser browser, string providerId)
        {
            Binding? snapshot;
            lock (_gate)
            {
                if (providerId != "synthetic" ||
                    !_bindings.TryGetValue(browser, out snapshot) ||
                    !_profileActionBusy.Contains(snapshot.ProfileId) ||
                    !Matches(browser, snapshot) || _permissionBusy.Contains(browser))
                    return false;
            }
            var result = MessageBox.Show(browser.View.FindForm(),
                "Start opt-in synthetic provider sign-in navigation for profile " +
                snapshot.ProfileId + "? No account or credential is accessed.",
                "CoSoS 8B native sign-in", MessageBoxButtons.YesNo,
                MessageBoxIcon.Question, MessageBoxDefaultButton.Button2);
            lock (_gate)
                return result == DialogResult.Yes &&
                    _bindings.TryGetValue(browser, out var current) &&
                    current == snapshot && Matches(browser, current);
        }

        public sealed record HandoffTicket(
            string Nonce, Binding Source, Binding Target,
            string NewSourceSessionId, ulong SourceNextEpoch, ulong TargetNextEpoch);

        private static bool CanHandoff(Binding source, Binding target) =>
            source.ProfileId != target.ProfileId &&
            source.PartitionId != target.PartitionId &&
            !string.Equals(source.ProfilePath, target.ProfilePath,
                StringComparison.OrdinalIgnoreCase) &&
            source.Origin == target.Origin &&
            source.SessionId != target.SessionId &&
            source.SessionEpoch > 0 && target.SessionEpoch > 0 &&
            source.SessionEpoch < 9_007_199_254_740_989UL &&
            target.SessionEpoch < 9_007_199_254_740_989UL;

        // Cross-profile transfer is a whole-profile operation: deny while an
        // authenticated sibling tab still owns a native/Rust lease.
        private bool HasLiveSibling(Browser source, Browser target) =>
            _bindings.Any(pair =>
                !ReferenceEquals(pair.Key, source) &&
                !ReferenceEquals(pair.Key, target) &&
                !pair.Key.Revoked && pair.Key.ChannelId != 0 &&
                (pair.Value.ProfileId == source.Profile ||
                 pair.Value.ProfileId == target.Profile));

        public HandoffTicket? ApproveHandoff(Browser source, Browser target)
        {
            Binding? first, second;
            lock (_gate)
            {
                if (!_bindings.TryGetValue(source, out first) ||
                    !_bindings.TryGetValue(target, out second) ||
                    !Matches(source, first) || !Matches(target, second) ||
                    !CanHandoff(first, second) || HasLiveSibling(source, target) ||
                    !_profileActionBusy.Contains(first.ProfileId) ||
                    !_profileActionBusy.Contains(second.ProfileId) ||
                    _permissionBusy.Contains(source) || _permissionBusy.Contains(target))
                    return null;
            }
            var choice = MessageBox.Show(source.View.FindForm(),
                "Transfer ONLY application session attribution from " + first.ProfileId +
                " to " + second.ProfileId + "? Browser cookies, login and " +
                "profile data will NOT be moved. Both native Rust channels will retire.",
                "CoSoS 8B explicit logical handoff",
                MessageBoxButtons.YesNo, MessageBoxIcon.Warning,
                MessageBoxDefaultButton.Button2);
            if (choice != DialogResult.Yes) return null;
            lock (_gate)
            {
                if (!_bindings.TryGetValue(source, out var nowSource) ||
                    !_bindings.TryGetValue(target, out var nowTarget) ||
                    first != nowSource || second != nowTarget ||
                    !Matches(source, nowSource) || !Matches(target, nowTarget) ||
                    HasLiveSibling(source, target) ||
                    _permissionBusy.Contains(source) || _permissionBusy.Contains(target))
                    return null;
                return new HandoffTicket(
                    Guid.NewGuid().ToString("N"), first, second,
                    "handoff-empty-" + Guid.NewGuid().ToString("N"),
                    first.SessionEpoch + 1,
                    Math.Max(first.SessionEpoch, second.SessionEpoch) + 1);
            }
        }

        // Called ONLY after a successful Rust-side atomic handoff, and after
        // BOTH old channels have confirmed retirement. The host changes both
        // local session bindings together under the native gate lock.
        public bool CommitHandoffAfterRust(
            Browser source, Browser target, HandoffTicket ticket)
        {
            lock (_gate)
            {
                if (_consumedHandoffs.Contains(ticket.Nonce) ||
                    !_bindings.TryGetValue(source, out var nowSource) ||
                    !_bindings.TryGetValue(target, out var nowTarget) ||
                    nowSource != ticket.Source || nowTarget != ticket.Target ||
                    !CanHandoff(nowSource, nowTarget) ||
                    HasLiveSibling(source, target) ||
                    !_profileActionBusy.Contains(nowSource.ProfileId) ||
                    !_profileActionBusy.Contains(nowTarget.ProfileId) ||
                    !source.Revoked || !target.Revoked ||
                    source.ChannelId != 0 || target.ChannelId != 0 ||
                    source.RetirementTask is not null || target.RetirementTask is not null ||
                    source.DocumentId != nowSource.DocumentId ||
                    target.DocumentId != nowTarget.DocumentId ||
                    source.NavigationEpoch != nowSource.NavigationEpoch ||
                    target.NavigationEpoch != nowTarget.NavigationEpoch ||
                    source.SessionId != nowSource.SessionId ||
                    target.SessionId != nowTarget.SessionId ||
                    !string.Equals(Path.GetFullPath(source.View.CoreWebView2.Profile.ProfilePath),
                        nowSource.ProfilePath, StringComparison.OrdinalIgnoreCase) ||
                    !string.Equals(Path.GetFullPath(target.View.CoreWebView2.Profile.ProfilePath),
                        nowTarget.ProfilePath, StringComparison.OrdinalIgnoreCase))
                    return false;
                _consumedHandoffs.Add(ticket.Nonce);
                source.SessionId = ticket.NewSourceSessionId;
                source.SessionEpoch = ticket.SourceNextEpoch;
                target.SessionId = nowSource.SessionId;
                target.SessionEpoch = ticket.TargetNextEpoch;
                // Tombstones preserve strict FreshRebind generation fencing.
                _bindings[source] = nowSource with {
                    SessionId = source.SessionId, SessionEpoch = source.SessionEpoch
                };
                _bindings[target] = nowTarget with {
                    SessionId = target.SessionId, SessionEpoch = target.SessionEpoch
                };
                return true;
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
            _native8L1ProfilePathA = a.ProfilePath;
            _native8L1ProfilePathB = b.ProfilePath;
            _native8L1ExtensionA = a.ExtensionId;
            _native8L1ExtensionB = b.ExtensionId;
            _native8L1MarkerA = await ReadRecoveryMarkerAsync(a);
            _native8L1MarkerB = await ReadRecoveryMarkerAsync(b);
            Mark("native_profile_storage_proof",
                _native8L1MarkerA.Length > 0 && _native8L1MarkerB.Length > 0 &&
                _native8L1MarkerA != _native8L1MarkerB &&
                _native8L1Authority.Current(a) && _native8L1Authority.Current(b),
                _reuse8L1Profile
                    ? "Retained browser-local storage loaded before verified fresh Rust/MV3 channel registration"
                    : "Independent real profiles create distinct synthetic-only browser-local markers");
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
            if (_host8L1ConsentProbe)
            {
                // Physical permission events only. Owner selects Yes to first
                // request, No to second, then Yes to the stale third dialog.
                // The replay request is automatically refused without a dialog.
                const string permissionScript =
                    "navigator.geolocation.getCurrentPosition(() => {}, () => {}, " +
                    "{timeout:1000,maximumAge:0});";
                for (var request = 0; request < 2; request++)
                {
                    var before = _native8L1Authority.OwnerConsentCounts().Events;
                    await _b.CoreWebView2.ExecuteScriptAsync(permissionScript);
                    for (var attempts = 0; attempts < 100 &&
                        _native8L1Authority.OwnerConsentCounts().Events == before; attempts++)
                        await Task.Delay(100);
                }
                var normal = _native8L1Authority.OwnerConsentCounts();
                Mark("native_permission_positive_consent",
                    normal.Prompts >= 2 && normal.Grants >= 1 && normal.Denials >= 1 &&
                    _native8L1Authority.Current(b),
                    "Two real WebView2 permission events: explicit owner Yes and No, no persisted grant");

                if (normal.Grants == 0)
                    throw new UnauthorizedAccessException("Owner Allow missing for replay probe");
                _native8L1Authority.ArmReplayProbe();
                var beforeReplay = normal.Events;
                await _b.CoreWebView2.ExecuteScriptAsync(permissionScript);
                for (var attempts = 0; attempts < 100 &&
                    _native8L1Authority.OwnerConsentCounts().Events == beforeReplay; attempts++)
                    await Task.Delay(100);
                var replay = _native8L1Authority.OwnerConsentCounts();
                Mark("native_permission_replay_event_denied",
                    replay.Events > beforeReplay && replay.ReplayDenied == 1 &&
                    replay.Grants == normal.Grants && replay.Prompts == normal.Prompts,
                    "Actual third WebView2 permission event refuses already-consumed owner approval");

                _native8L1Authority.ArmStaleProbe(b);
                var beforeStale = replay.Events;
                try { await _b.CoreWebView2.ExecuteScriptAsync(permissionScript); }
                catch (Exception ex) when (ex is InvalidOperationException or
                    System.Runtime.InteropServices.COMException)
                {
                    // Native navigation may abort the in-flight page evaluation.
                    // Only native event counts decide whether the probe passed.
                }
                for (var attempts = 0; attempts < 100 &&
                    _native8L1Authority.OwnerConsentCounts().Events == beforeStale; attempts++)
                    await Task.Delay(100);
                var stale = _native8L1Authority.OwnerConsentCounts();
                Mark("native_permission_stale_event_denied",
                    stale.Events > beforeStale && stale.StaleAllowDenied == 1 &&
                    stale.Grants == normal.Grants && !_native8L1Authority.Current(b),
                    "Actual modal Yes after native navigation cannot grant old document permission");
                await WaitPageAsync(b);
                await BindAsync(b, "8l1-synthetic-b");
                _native8L1Authority.Track(b);
                Mark("native_permission_recovered_new_channel",
                    _native8L1Authority.Current(b) && Ok(await ObserveAsync(b)),
                    "After stale consent is refused, new real document and Rust channel reauthenticate");
            }
            else
            {
                Pending("native_permission_positive_consent",
                    "Requires owner-driven live WebView2 Yes/No via -ConsentProbe");
                Pending("native_permission_replay_event_denied",
                    "Requires real WebView2 event against consumed grant via -ConsentProbe");
                Pending("native_permission_stale_event_denied",
                    "Requires owner Yes after real navigation during modal prompt via -ConsentProbe");
                Pending("native_permission_recovered_new_channel",
                    "Requires native channel rebind after stale consent via -ConsentProbe");
            }
            // 8L1 physical full gate is still NOT_RUN until restart/crash,
            // one-use real consent and multi-tab are independently exercised.
            Pending("native_host_process_restart",
                "OS process restart and browser crash/recovery require separate physical Windows proof");
            if (_host8L1ProfileActionsProbe)
            {
                // The first 8B action is an actual WebView2 navigation,
                // consented to at the NATIVE side rather than by page JS.
                // Synthetic provider only: this never claims a real login.
                using var signInLease = _native8L1Authority.ClaimProfileActions(b);
                if (!_native8L1Authority.ApproveSyntheticSignIn(b, "synthetic"))
                    throw new UnauthorizedAccessException("Native sign-in consent denied");
                var originalSession = b.SessionId;
                var originalEpoch = b.SessionEpoch;
                var originalDocument = b.DocumentId;
                _b.CoreWebView2.Navigate(StartUrl + "?native-sign-in=1");
                await Task.Delay(300);
                var signInNavigated = b.Revoked &&
                    b.DocumentId != originalDocument &&
                    b.RetirementTask is not null &&
                    await b.RetirementTask;
                Mark("native_8b_signin_navigation",
                    signInNavigated && b.SessionId == originalSession &&
                    b.SessionEpoch == originalEpoch,
                    "Native Yes starts real synthetic provider WebView2 navigation; " +
                    "old Rust channel retired, no authentication or cookie transfer is claimed");
                _b.CoreWebView2.Navigate(StartUrl);
                await WaitPageAsync(b);
                await BindAsync(b, originalSession);
                _native8L1Authority.Track(b);
                Mark("native_8b_signin_rebind",
                    _native8L1Authority.Current(b) &&
                    b.SessionEpoch == originalEpoch &&
                    Ok(await ObserveAsync(b)),
                    "Fresh Rust/MV3 channel after sign-in navigation, same profile/session/partition");
                signInLease.Dispose();

                // Every signed observation must exist before the Rust first-
                // proof fence can transfer its two currently owned sessions.
                Mark("native_8b_source_reattested",
                    _native8L1Authority.Current(a) && Ok(await ObserveAsync(a)),
                    "Source reauthenticated at fresh document and Rust first-proof boundary");
                using var handoffLease = _native8L1Authority.ClaimProfileActions(a, b);
                var ticket = _native8L1Authority.ApproveHandoff(a, b) ??
                    throw new UnauthorizedAccessException("Native 8B handoff owner consent denied");
                var oldA = Host(a);
                var oldB = Host(b);
                var oldSessionA = a.SessionId;
                var oldSessionB = b.SessionId;
                RevokeAndRetire(a);
                RevokeAndRetire(b);
                await RequireRetirementAsync(a);
                await RequireRetirementAsync(b);
                var transferred = await RustAsync(new
                {
                    op = "handoff", source = oldA, target = oldB,
                    newSourceSessionId = ticket.NewSourceSessionId,
                    sourceNextSessionEpoch = ticket.SourceNextEpoch,
                    targetNextSessionEpoch = ticket.TargetNextEpoch
                });
                var rustCommitted = Ok(transferred) &&
                    transferred.GetProperty("value").GetProperty("committed").GetBoolean();
                if (!rustCommitted ||
                    !_native8L1Authority.CommitHandoffAfterRust(a, b, ticket))
                    throw new UnauthorizedAccessException("Native 8B atomic handoff was not confirmed");
                Mark("native_8b_session_handoff_atomic",
                    a.SessionId == ticket.NewSourceSessionId &&
                    b.SessionId == oldSessionA && b.SessionId != oldSessionB &&
                    a.SessionEpoch == ticket.SourceNextEpoch &&
                    b.SessionEpoch == ticket.TargetNextEpoch,
                    "Native one-use owner grant; both Rust leases retired before atomic " +
                    "first-proof and C# session attribution transaction, no profile data copied");
                Mark("native_8b_handoff_replay_denied",
                    await RustDeniedAsync(new
                    {
                        op = "handoff", source = oldA, target = oldB,
                        newSourceSessionId = ticket.NewSourceSessionId,
                        sourceNextSessionEpoch = ticket.SourceNextEpoch,
                        targetNextSessionEpoch = ticket.TargetNextEpoch
                    }) &&
                    !_native8L1Authority.CommitHandoffAfterRust(a, b, ticket),
                    "Both Rust first-proof and native one-use consent refuse handoff replay");
                _a.CoreWebView2.Navigate(StartUrl);
                _b.CoreWebView2.Navigate(StartUrl);
                await WaitPageAsync(a);
                await WaitPageAsync(b);
                await BindAsync(a, a.SessionId);
                await BindAsync(b, b.SessionId);
                _native8L1Authority.Track(a);
                _native8L1Authority.Track(b);
                Mark("native_8b_handoff_fresh_observation",
                    _native8L1Authority.Current(a) &&
                    _native8L1Authority.Current(b) &&
                    !_native8L1Authority.SamePartition(a, b) &&
                    Ok(await ObserveAsync(a)) && Ok(await ObserveAsync(b)),
                    "New Rust/MV3 first proofs bind reattributed sessions to " +
                    "separate original WebView2 storage partitions");
            }
            else
            {
                Pending("native_8b_signin_navigation",
                    "Actual synthetic provider navigation needs owner opt-in ProfileActionsProbe");
                Pending("native_8b_signin_rebind",
                    "Actual synthetic navigation must rebind with fresh Rust channel");
                Pending("native_8b_source_reattested",
                    "8B source Rust signed proof needed before live handoff");
                Pending("native_8b_session_handoff_atomic",
                    "Native owner one-use consent and retired source/target channels required");
                Pending("native_8b_handoff_replay_denied",
                    "Requires live native handoff and replay denial");
                Pending("native_8b_handoff_fresh_observation",
                    "Requires both fresh native Rust/MV3 identities after handoff");
            }
            if (_host8L1CrashProbe)
            {
                // Optional physically executed failure gate. A real browser
                // process crash must asynchronously emit WebView2 ProcessFailed.
                // No production tab/window profile is used by this runner.
                var channels = new[] { a, b }.Select(v => v.ChannelId).ToArray();
                try
                {
                    await CdpAsync(a, "Browser.crash", new { });
                }
                catch (Exception) { /* an actual crash can interrupt its own CDP reply */ }
                for (var tries = 0; tries < 50 && _native8L1ProcessFailures == 0; tries++)
                    await Task.Delay(100);
                var revokedAll = _native8L1ProcessFailures > 0 &&
                    !_native8L1Authority.Current(a) && !_native8L1Authority.Current(b);
                var retiredAll = revokedAll;
                foreach (var browser in new[] { a, b })
                    retiredAll &= browser.RetirementTask is not null &&
                        await browser.RetirementTask;
                Mark("native_browser_process_failed",
                    revokedAll && retiredAll && channels.All(v => v != 0),
                    "Real Browser.crash must trigger WebView2 ProcessFailed, revoke all native scopes and confirm Rust retire");
            }
            else
                Pending("native_browser_process_failed",
                    "Opt in to a real synthetic WebView2 browser crash with run-native.ps1 -CrashProbe");
            Mark("native_browser_failure_listener_installed",
                _host8L1CrashProbe ? _native8L1ProcessFailures > 0 :
                    _native8L1ProcessFailures == 0 && _native8L1Authority.Current(b),
                "Actual WebView2 ProcessFailed event handler retires every tracked native Rust channel");
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
                    rustProcessExit = rustExit,
                    retainedProfileMode = _retained8L1ProfileRoot is not null,
                    reusedRetainedProfiles = _reuse8L1Profile,
                    profileAPath = _native8L1ProfilePathA,
                    profileBPath = _native8L1ProfilePathB,
                    extensionAId = _native8L1ExtensionA,
                    extensionBId = _native8L1ExtensionB,
                    syntheticMarkerA = _native8L1MarkerA,
                    syntheticMarkerB = _native8L1MarkerB,
                    checks = _checks
                }, new JsonSerializerOptions { WriteIndented = true }));
            Environment.ExitCode = exit;
            BeginInvoke(new Action(Close));
        }
    }
}
