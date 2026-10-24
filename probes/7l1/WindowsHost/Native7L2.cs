using System.Diagnostics;
using System.Text;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace CoSoS.Probe7L1.WindowsHost;

// 7D-FIX: the same privately attested 7L1 host owns the CDP session and the
// Rust tool subprocess. No WebMessageReceived, page IPC or arbitrary CDP method.
internal sealed partial class GateForm
{
    private sealed record PageDebuggerLease(string HostJson, string? SessionId);
    private bool _faultNextDebuggerDetach;
    private readonly Dictionary<Browser, PageDebuggerLease> _pageDebuggers =
        new(ReferenceEqualityComparer.Instance);

    private static bool SameHost(JsonElement host, Browser b)
    {
        try
        {
            var id = host.GetProperty("identity");
            return host.GetProperty("origin").GetString() == Origin &&
                host.GetProperty("hostGeneration").GetUInt64() == b.HostGeneration &&
                id.GetProperty("profileId").GetString() == b.Profile &&
                id.GetProperty("tabId").GetUInt32() == b.TabId &&
                id.GetProperty("frameId").GetUInt32() == 0 &&
                id.GetProperty("documentId").GetString() == b.DocumentId &&
                id.GetProperty("navigationEpoch").GetUInt64() == b.NavigationEpoch &&
                id.GetProperty("sessionId").GetString() == b.SessionId &&
                id.GetProperty("sessionEpoch").GetUInt64() == 1 &&
                id.GetProperty("workerGeneration").GetUInt64() == b.WorkerGeneration &&
                id.EnumerateObject().Count() == 8 &&
                host.EnumerateObject().Count() == 3;
        }
        catch (Exception ex) when (ex is KeyNotFoundException or InvalidOperationException or FormatException)
        {
            return false;
        }
    }
    private async Task<object> Run7L2CdpAsync(JsonElement callback)
    {
        if (!_l2) throw new UnauthorizedAccessException("not a 7L2 host");
        var op = callback.GetProperty("op").GetString();
        if (op is not ("attach" or "snapshot" or "detach") ||
            callback.EnumerateObject().Count() != 2)
            throw new UnauthorizedAccessException("invalid debugger callback opcode");
        var host = callback.GetProperty("host");
        // Native controller lookup is pinned to actual WebView2-owned profile +
        // tab. No identity object supplied by a renderer can create an entry.
        var id = host.GetProperty("identity");
        var b = _browsers.Values.SingleOrDefault(x =>
            x.Profile == id.GetProperty("profileId").GetString() &&
            x.TabId == id.GetProperty("tabId").GetUInt32());
        if (b is null) throw new UnauthorizedAccessException("unowned native tab");
        var proof = host.GetRawText();
        if (op == "detach")
        {
            // A lifecycle retire may arrive AFTER document/navigation mutation.
            // Permit only cleanup of the exact originally admitted session,
            // NEVER new capture or attach from the retired controller.
            if (!_pageDebuggers.TryGetValue(b, out var lease) ||
                !string.Equals(lease.HostJson, proof, StringComparison.Ordinal) ||
                string.IsNullOrWhiteSpace(lease.SessionId) || b.View.IsDisposed)
                throw new UnauthorizedAccessException("ambiguous debugger detach");
            if (_faultNextDebuggerDetach)
            {
                _faultNextDebuggerDetach = false;
                // Native-only deterministic fault injection: exercise Rust's
                // fail-closed ambiguous detach lease instead of lying about release.
                throw new UnauthorizedAccessException("injected native debugger detach refusal");
            }
            await CdpAsync(b, "Target.detachFromTarget", new { sessionId = lease.SessionId });
            _pageDebuggers.Remove(b);
            return new { detached = true };
        }
        if (!SameHost(host,b)) throw new UnauthorizedAccessException("stale host identity");
        await AssertOwnedAsync(b);
        if (op == "attach")
        {
            if (_pageDebuggers.ContainsKey(b))
                throw new UnauthorizedAccessException("debugger already attached or ambiguous");
            // Reserve the native lease BEFORE async attach. If CDP succeeds but
            // its response is lost the reservation persists and denies takeover.
            _pageDebuggers.Add(b,new PageDebuggerLease(proof,null));
            var page = await CdpAsync(b,"Target.getTargetInfo",new { });
            var info = page.GetProperty("targetInfo");
            if (info.GetProperty("type").GetString() != "page" ||
                info.GetProperty("url").GetString() != StartUrl)
                throw new UnauthorizedAccessException("untrusted CDP page target");
            var target = info.GetProperty("targetId").GetString();
            if (string.IsNullOrWhiteSpace(target))
                throw new UnauthorizedAccessException("missing CDP page target ID");
            var attached = await CdpAsync(b,"Target.attachToTarget",
                new { targetId = target, flatten = true });
            var session = attached.GetProperty("sessionId").GetString();
            if (string.IsNullOrWhiteSpace(session))
                throw new UnauthorizedAccessException("missing CDP debugger session");
            _pageDebuggers[b] = new PageDebuggerLease(proof,session);
            await AssertOwnedAsync(b);
            return new { attached = true };
        }
        if (!_pageDebuggers.TryGetValue(b,out var owned) ||
            owned.HostJson != proof || string.IsNullOrWhiteSpace(owned.SessionId))
            throw new UnauthorizedAccessException("unowned debugger snapshot");
        // Expression is source-owned and constant. No user-supplied JS/CDP,
        // mutation, click, Send, cookies, form values or network calls.
        const string expression = "(() => JSON.stringify({url:location.href,title:document.title,text:(document.documentElement?.innerText ?? '').slice(0,65536)}))()";
        var raw = await b.View.CoreWebView2.CallDevToolsProtocolMethodForSessionAsync(
            owned.SessionId,"Runtime.evaluate",
            JsonSerializer.Serialize(new { expression, returnByValue = true }));
        using var parsed = JsonDocument.Parse(raw);
        if (parsed.RootElement.TryGetProperty("exceptionDetails",out _))
            throw new UnauthorizedAccessException("snapshot CDP exception");
        var capture = parsed.RootElement.GetProperty("result").GetProperty("value").GetString();
        if (capture is null || Encoding.UTF8.GetByteCount(capture) > 270000)
            throw new UnauthorizedAccessException("oversized debugger payload");
        using var document = JsonDocument.Parse(capture);
        var snapshot = document.RootElement;
        var url = snapshot.GetProperty("url").GetString() ?? "";
        var title = snapshot.GetProperty("title").GetString() ?? "";
        var text = snapshot.GetProperty("text").GetString() ?? "";
        if (!url.StartsWith(Origin + "/",StringComparison.Ordinal) ||
            Encoding.UTF8.GetByteCount(url) > 2048 ||
            Encoding.UTF8.GetByteCount(title) > 2048 ||
            Encoding.UTF8.GetByteCount(text) > 256*1024)
            throw new UnauthorizedAccessException("cross-origin or oversized page snapshot");
        await AssertOwnedAsync(b); // fence after CDP async + data decode
        return new {
            identity = JsonSerializer.SerializeToElement(Host(b)).GetProperty("identity"),
            url, title, text
        };
    }

    // The 7L1 privileged MV3 worker generates the 7B authorization MAC.
    // Each operation uses a unique, confirmed sequence; even denied leases
    // consume a verified authorization. Invalid MAC calls DO NOT advance.
    private async Task<JsonElement> ToolAsync(Browser b, string op, ulong sequence,
        ulong? handle = null, bool corruptMac = false)
    {
        if (op is not ("attach" or "snapshot" or "detach"))
            throw new ArgumentException("bounded 7D tools only");
        var signed = await WorkerAsync(b, Js("authorizeTool", sequence));
        var mac = signed.GetProperty("mac").GetString() ?? "";
        if (corruptMac) mac = (mac[0] == '0' ? "1" : "0") + mac[1..];
        var command = new Dictionary<string,object?>
        {
            ["op"] = "tool_" + op, ["host"] = Host(b),
            ["channelId"] = b.ChannelId, ["generation"] = b.HostGeneration,
            ["sequence"] = sequence,
            ["payload"] = signed.GetProperty("payload").GetString(),
            ["mac"] = mac
        };
        if (handle.HasValue) command["handle"] = handle.Value;
        var response = await RustAsync(command);
        if (!corruptMac)
            await WorkerAsync(b, Js("confirmToolAuthorization", sequence));
        return response;
    }
    private static bool Ok(JsonElement response) =>
        response.GetProperty("ok").GetBoolean();

    private async Task Execute7L2Async()
    {
        var exit = 2;
        var rustExit = -1;
        try
        {
            if (Directory.Exists(_root) && Directory.EnumerateFileSystemEntries(_root).Any())
                throw new IOException("7L2 requires fresh empty D: output folder");
            Directory.CreateDirectory(_root);
            VerifySourceArtifact(); // source+all loadable binaries SHA256
            BuildExtension(); // real installed MV3, not a fake Rust debugger
            var psi = new ProcessStartInfo(_bridgePath)
            {
                UseShellExecute = false, RedirectStandardInput = true,
                RedirectStandardOutput = true, RedirectStandardError = true,
                CreateNoWindow = true, WorkingDirectory = _root
            };
            psi.Environment["TEMP"] = _root;
            psi.Environment["TMP"] = _root;
            _bridge = Process.Start(psi) ?? throw new InvalidOperationException("Rust 7D helper missing");
            var a = await SetupAsync(_a,"A",1);
            var b = await SetupAsync(_b,"B",2);
            Mark("actual_independent_webview2",a.ProfilePath != b.ProfilePath,
                "Separate browser-owned profile controllers and MV3 extension installations");
            await BindAsync(a,"7l2-session-a");
            Mark("real_native_7b_authentication",
                RequireSuccess(await ObserveAsync(a)).GetProperty("observation").GetString() == "FirstProof",
                "MV3 CDP service_worker signed 7B handshake and frame observation");
            var first = await ToolAsync(a,"attach",2);
            var handle = Ok(first) ? first.GetProperty("value").GetProperty("handle").GetUInt64() : 0;
            Mark("actual_debugger_attach",Ok(first) && handle > 0,
                "Real WebView2 Target.attachToTarget flat page session via Rust DirectBrowserTools");
            var duplicate = await ToolAsync(a,"attach",3);
            Mark("duplicate_attach_denied",!Ok(duplicate),
                "Same verified tab cannot acquire a second debugger lease");
            var captured = await ToolAsync(a,"snapshot",4,handle);
            Mark("native_bounded_snapshot",Ok(captured) &&
                captured.GetProperty("value").GetProperty("url").GetString() == StartUrl &&
                captured.GetProperty("value").GetProperty("title").GetString() ==
                    "7L1 Isolated Native Authority",
                "Actual CDP Runtime.evaluate in pinned page session, bounded snapshot");
            var guessed = await ToolAsync(a,"snapshot",5,handle+42);
            Mark("guessed_handle_denied",!Ok(guessed),
                "A valid signed MAC alone cannot bypass exact native lease handle");
            var badMac = await ToolAsync(a,"snapshot",6,handle,true);
            var afterBadMac = await ToolAsync(a,"snapshot",6,handle);
            Mark("invalid_mac_replay_denied",!Ok(badMac) && Ok(afterBadMac),
                "Bad MAC did not advance the 7B authorization sequence");
            // A distinct browser/tab and worker cannot steal the original lease.
            await BindAsync(b,"7l2-session-b");
            Mark("second_profile_7b_handshake",
                Ok(await ObserveAsync(b)),"Independent profile signed native authority");
            var attachLedger = await LedgerEventAsync(b,"attach",1);
            var claimLedger = await LedgerEventAsync(b,"claim",2,"7l2-ambiguous",
                new string('a',64));
            var repairLedger = await LedgerEventAsync(b,"repair",3,"7l2-ambiguous",
                new string('a',64));
            Mark("persisted_ambiguous_ack_no_send",
                attachLedger.GetProperty("kind").GetString() == "attached" &&
                claimLedger.GetProperty("status").GetString() == "claimed" &&
                !repairLedger.GetProperty("resendAllowed").GetBoolean(),
                "Real MV3 chrome.storage.session repair never authorizes a duplicate Send");
            var stolen = await ToolAsync(b,"snapshot",5,handle);
            Mark("cross_profile_tab_denied",!Ok(stolen),
                "Another WebView2 profile and tab cannot read owner A lease");
            var second = await ToolAsync(b,"attach",6);
            var bHandle = Ok(second) ? second.GetProperty("value").GetProperty("handle").GetUInt64() : 0;
            var ownSnapshot = await ToolAsync(b,"snapshot",7,bHandle);
            Mark("independent_profile_snapshot",Ok(second) && Ok(ownSnapshot) &&
                ownSnapshot.GetProperty("value").GetProperty("url").GetString() == StartUrl,
                "Second actual controller can independently attach/read owned page");
            var bDetach = await ToolAsync(b,"detach",8,bHandle);
            Mark("second_profile_detach",Ok(bDetach),
                "WebView2 Target.detachFromTarget released second native session");
            var signedBeforeClose = await WorkerAsync(b,Js("authorizeTool",9));
            var closedHost = Host(b);
            var closedChannel = b.ChannelId;
            var closedGeneration = b.HostGeneration;
            _b.Dispose();
            var closedRetired = b.RetirementTask is not null && await b.RetirementTask;
            var closedStale = await RustDeniedAsync(new {
                op = "tool_snapshot", host = closedHost, channelId = closedChannel,
                generation = closedGeneration, sequence = 9,
                payload = signedBeforeClose.GetProperty("payload").GetString(),
                mac = signedBeforeClose.GetProperty("mac").GetString(), handle = bHandle
            });
            Mark("native_tab_close_revocation",closedRetired && closedStale && b.View.IsDisposed,
                "Disposed real WebView2 controller retires 7B channel and denies stale signed tools");
            // Cross-frame spoof: a valid signed frame for frame 0 must not authorize
            // a host object claiming frame 1 (no real frame 1 controller exists).
            var signed = await WorkerAsync(a,Js("authorizeTool",7));
            var fakeHost = new
            {
                identity = new {
                    profileId = a.Profile, tabId = a.TabId, frameId = 1,
                    documentId = a.DocumentId, navigationEpoch = a.NavigationEpoch,
                    sessionId = a.SessionId, sessionEpoch = 1,
                    workerGeneration = a.WorkerGeneration
                },
                origin = Origin, hostGeneration = a.HostGeneration
            };
            var crossFrame = await RustDeniedAsync(new {
                op = "tool_snapshot", host = fakeHost, channelId = a.ChannelId,
                generation = a.HostGeneration, sequence = 7,
                payload = signed.GetProperty("payload").GetString(),
                mac = signed.GetProperty("mac").GetString(), handle
            });
            var realAfterCrossFrame = await ToolAsync(a,"snapshot",7,handle);
            Mark("cross_frame_spoof_denied",crossFrame && Ok(realAfterCrossFrame),
                "Authenticated frame 0 cannot be lifted into a forged frame 1");
            var detached = await ToolAsync(a,"detach",8,handle);
            Mark("actual_debugger_detach",Ok(detached),
                "Native Target.detachFromTarget completed before Rust lease release");
            var reattach = await ToolAsync(a,"attach",9);
            var newHandle = Ok(reattach) ?
                reattach.GetProperty("value").GetProperty("handle").GetUInt64() : 0;
            Mark("verified_reattach",Ok(reattach) && newHandle != handle,
                "Detached WebView2 page can be attached with fresh Rust handle");
            _faultNextDebuggerDetach = true;
            var failedDetach = await ToolAsync(a,"detach",10,newHandle);
            var competingAfterFailure = await ToolAsync(a,"attach",11);
            var successfulCleanup = await ToolAsync(a,"detach",12,newHandle);
            Mark("detach_failure_retains_lease",!Ok(failedDetach) &&
                !Ok(competingAfterFailure) && Ok(successfulCleanup),
                "Injected actual host CDP refusal retains Rust lease until confirmed native detach");
            var afterFailure = await ToolAsync(a,"attach",13);
            newHandle = Ok(afterFailure) ?
                afterFailure.GetProperty("value").GetProperty("handle").GetUInt64() : 0;
            Mark("reattach_after_verified_cleanup",Ok(afterFailure) && newHandle > 0,
                "New native debugger lease admitted only after genuine detach succeeds");
            var staleHost = Host(a);
            var staleChannel = a.ChannelId;
            var staleGeneration = a.HostGeneration;
            var oldSigned = await WorkerAsync(a,Js("authorizeTool",14));
            _a.CoreWebView2.Navigate(StartUrl + "?native-retire=1");
            await Task.Delay(350);
            var retired = a.RetirementTask is not null && await a.RetirementTask;
            var stale = await RustDeniedAsync(new {
                op = "tool_snapshot", host = staleHost, channelId = staleChannel,
                generation = staleGeneration, sequence = 14,
                payload = oldSigned.GetProperty("payload").GetString(),
                mac = oldSigned.GetProperty("mac").GetString(), handle = newHandle
            });
            Mark("navigation_worker_channel_retirement",retired && stale,
                "Real navigation retires the signed Rust channel and WebView2 session");
            _a.CoreWebView2.Navigate(StartUrl);
            await WaitPageAsync(a);
            await BindAsync(a,"7l2-session-a");
            Mark("rebind_after_navigation",
                Ok(await ObserveAsync(a)),"Rebound actual MV3 worker and new authenticated generation");
            var workerAttach = await ToolAsync(a,"attach",2);
            var workerHandle = Ok(workerAttach) ?
                workerAttach.GetProperty("value").GetProperty("handle").GetUInt64() : 0;
            var signedBeforeWorkerStop = await WorkerAsync(a,Js("authorizeTool",3));
            var workerStopHost = Host(a);
            var workerStopChannel = a.ChannelId;
            var workerStopGeneration = a.HostGeneration;
            var workerTarget = a.WorkerTargetId ??
                throw new InvalidOperationException("missing source-owned MV3 worker");
            var stopWorkerGeneration = a.WorkerGeneration;
            // A hash identifies the exact synthetic CDP target without exposing
            // a live target/session identifier in the persisted physical receipt.
            var targetFingerprint = Convert.ToHexString(
                System.Security.Cryptography.SHA256.HashData(Encoding.UTF8.GetBytes(workerTarget)))
                .ToLowerInvariant()[..12];
            var proofId = "synthetic-worker-target-sha256=" + targetFingerprint +
                ", hostGeneration=" + workerStopGeneration +
                ", workerGeneration=" + stopWorkerGeneration +
                ", channel=" + workerStopChannel;
            bool closeTargetSuccess = false;
            string closeError = "none";
            try
            {
                var close = await CdpAsync(a, "Target.closeTarget",
                    new { targetId = workerTarget });
                closeTargetSuccess = close.TryGetProperty("success", out var success) &&
                    success.ValueKind == JsonValueKind.True;
            }
            catch (Exception ex) when (ex is JsonException or InvalidOperationException or
                System.Runtime.InteropServices.COMException)
            {
                closeError = ex.GetType().Name;
                // An error or unacknowledged close NEVER constitutes retirement.
            }
            Mark("mv3_close_target_success", closeTargetSuccess,
                proofId + ", browserCloseError=" + closeError);

            // WebView2 can deliver Target.targetDestroyed late or not at all.
            // Poll the real controller's native CDP inventory; a closeTarget ACK
            // by itself, a timeout or a guessed worker target cannot revoke.
            bool verifiedExactAbsence = false;
            string inventoryError = "none";
            for (var attempts = 0; attempts < 30; attempts++)
            {
                if (a.View.IsDisposed) break;
                try
                {
                    var inventory = await CdpAsync(a, "Target.getTargets", new { });
                    var absent = NativeRetirementProof.ExactTargetAbsent(inventory, workerTarget);
                    if (NativeRetirementProof.MayRetireAfterInventory(
                        closeTargetSuccess, absent, workerTarget, a.WorkerTargetId,
                        workerStopGeneration, a.HostGeneration,
                        stopWorkerGeneration, a.WorkerGeneration,
                        workerStopChannel, a.ChannelId))
                    {
                        verifiedExactAbsence = true;
                        // Source-attested absence, NOT closeTarget success, can
                        // trigger the same synchronous fence as a native event.
                        if (!a.Revoked) RevokeAndRetire(a);
                        break;
                    }
                }
                catch (Exception ex) when (ex is JsonException or InvalidOperationException or
                    System.Runtime.InteropServices.COMException)
                {
                    inventoryError = ex.GetType().Name;
                    // Retry only within the bounded observation window.
                }
                await Task.Delay(100);
            }
            _checks["mv3_destroyed_event_observation"] = new
            {
                status = a.WorkerDestroyedEventObserved ? "PASS" : "NOT_OBSERVED",
                observed = a.WorkerDestroyedEventObserved,
                evidence = proofId + ", native Target.targetDestroyed event (not required when exact native inventory proves absence)"
            };
            Mark("mv3_exact_target_absent", verifiedExactAbsence,
                proofId + ", native Target.getTargets verified exact target absent; inventoryError=" + inventoryError);
            Mark("mv3_generation_revoked", a.Revoked,
                proofId + ", fail-closed native controller generation fence");
            var retirementTask = a.RetirementTask;
            Mark("mv3_retirement_task_observed", retirementTask is not null,
                proofId + ", native initiated exact Rust channel retirement");
            var workerRetired = retirementTask is not null && await retirementTask;
            Mark("mv3_rust_retired", workerRetired,
                proofId + ", Rust confirmed retirement of original channel");
            var staleWorker = await RustDeniedAsync(new {
                op = "tool_snapshot",host = workerStopHost,channelId = workerStopChannel,
                generation = workerStopGeneration,sequence = 3,
                payload = signedBeforeWorkerStop.GetProperty("payload").GetString(),
                mac = signedBeforeWorkerStop.GetProperty("mac").GetString(), handle = workerHandle
            });
            Mark("mv3_stale_snapshot_denied", staleWorker,
                proofId + ", pre-signed stale generation denied by Rust after native lifecycle proof");
            Mark("mv3_worker_retirement", Ok(workerAttach) &&
                closeTargetSuccess && verifiedExactAbsence &&
                a.Revoked && retirementTask is not null && workerRetired && staleWorker,
                proofId + ", real exact-target native CDP absence, fence, Rust retire and stale denial");
            // This test must fail closed if native debugger detach was ambiguous.
            var required = new[] {
                "source_artifact_provenance","actual_independent_webview2",
                "real_native_7b_authentication","actual_debugger_attach",
                "duplicate_attach_denied","native_bounded_snapshot",
                "guessed_handle_denied","invalid_mac_replay_denied",
                "second_profile_7b_handshake","persisted_ambiguous_ack_no_send",
                "cross_profile_tab_denied","independent_profile_snapshot",
                "second_profile_detach","native_tab_close_revocation",
                "cross_frame_spoof_denied",
                "actual_debugger_detach","verified_reattach",
                "detach_failure_retains_lease","reattach_after_verified_cleanup",
                "navigation_worker_channel_retirement",
                "rebind_after_navigation","mv3_close_target_success",
                "mv3_exact_target_absent","mv3_generation_revoked",
                "mv3_retirement_task_observed","mv3_rust_retired",
                "mv3_stale_snapshot_denied","mv3_worker_retirement"
            };
            var passed = required.All(k=>_checks.TryGetValue(k,out var value) &&
                JsonSerializer.Serialize(value).Contains("\"status\":\"PASS\"",StringComparison.Ordinal));
            Mark("native_7l2_debugger_gate",passed && !_failed,
                "No synthetic substitute; all physical WebView2+Rust tests required");
            exit = passed && !_failed ? 0 : 2;
        }
        catch (Exception ex)
        {
            Mark("fatal",false,ex.GetType().Name + ": " + ex.Message);
            exit=1;
        }
        finally
        {
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
                    Mark("rust_process_exit",false,ex.GetType().Name);
                    exit=1;
                }
                _bridge.Dispose();
            }
            if (rustExit != 0)
            {
                Mark("rust_process_exit",false,"Process exit=" + rustExit);
                exit=1;
            }
            else Mark("rust_process_exit",true,"Native Rust child exit=0");
            Directory.CreateDirectory(_root);
            File.WriteAllText(Path.Combine(_root,"receipt.json"),
                JsonSerializer.Serialize(new {
                    scope="7D-FIX 7L2 real native WebView2+MV3+Rust browser tools (synthetic profiles)",
                    dateUtc=DateTimeOffset.UtcNow,sourceSha=_verifiedSourceSha ?? "NOT_RUN",
                    hostBinarySha256=HashFile(typeof(GateForm).Assembly.Location),
                    rustBinarySha256=File.Exists(_bridgePath) ? HashFile(_bridgePath) : "missing",
                    native7L2Accepted=exit==0,rustProcessExit=rustExit,checks=_checks
                },new JsonSerializerOptions{WriteIndented=true}));
            Environment.ExitCode=exit;
            BeginInvoke(new Action(Close));
        }
    }
}
