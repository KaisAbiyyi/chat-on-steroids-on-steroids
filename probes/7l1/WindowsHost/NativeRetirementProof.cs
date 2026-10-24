using System.Text.Json;

namespace CoSoS.Probe7L1.WindowsHost;

// Source-owned, testable proof: a successful close request is NOT evidence that
// the exact MV3 worker target disappeared. Only the browser's native CDP target
// inventory can attest absence of that specific target, with pinned ownership.
internal static class NativeRetirementProof
{
    internal static bool ExactTargetAbsent(JsonElement response, string expectedTargetId)
    {
        if (string.IsNullOrWhiteSpace(expectedTargetId) ||
            response.ValueKind != JsonValueKind.Object ||
            !response.TryGetProperty("targetInfos", out var targets) ||
            targets.ValueKind != JsonValueKind.Array ||
            targets.GetArrayLength() == 0)
            return false;
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var target in targets.EnumerateArray())
        {
            if (target.ValueKind != JsonValueKind.Object ||
                !target.TryGetProperty("targetId", out var targetId) ||
                targetId.ValueKind != JsonValueKind.String ||
                !target.TryGetProperty("type", out var type) ||
                type.ValueKind != JsonValueKind.String ||
                !target.TryGetProperty("url", out var url) ||
                url.ValueKind != JsonValueKind.String)
                return false;
            var id = targetId.GetString();
            if (string.IsNullOrWhiteSpace(id) || !seen.Add(id) ||
                string.IsNullOrWhiteSpace(type.GetString()) || url.GetString() is null)
                return false;
            if (string.Equals(id, expectedTargetId, StringComparison.Ordinal))
                return false;
        }
        return true;
    }

    internal static bool MayRetireAfterInventory(
        bool closeSucceeded, bool exactTargetAbsent,
        string expectedTargetId, string? currentTargetId,
        ulong expectedHostGeneration, ulong currentHostGeneration,
        ulong expectedWorkerGeneration, ulong currentWorkerGeneration,
        ulong expectedChannelId, ulong currentChannelId)
    {
        return closeSucceeded && exactTargetAbsent &&
            !string.IsNullOrWhiteSpace(expectedTargetId) &&
            string.Equals(expectedTargetId, currentTargetId, StringComparison.Ordinal) &&
            expectedHostGeneration != 0 && expectedHostGeneration == currentHostGeneration &&
            expectedWorkerGeneration != 0 && expectedWorkerGeneration == currentWorkerGeneration &&
            expectedChannelId != 0 && expectedChannelId == currentChannelId;
    }

    // Deterministic hosted Windows regression. No fake browser/physical PASS.
    internal static void RunSelfTests()
    {
        using var absent = JsonDocument.Parse(
            """{"targetInfos":[{"targetId":"page-1","type":"page","url":"https://synthetic.cos.test/index.html"},{"targetId":"new-worker","type":"service_worker","url":"chrome-extension://synthetic/sw.mjs"}]}""");
        using var present = JsonDocument.Parse(
            """{"targetInfos":[{"targetId":"old-worker","type":"service_worker","url":"chrome-extension://synthetic/sw.mjs"}]}""");
        using var malformed = JsonDocument.Parse("""{"targetInfos":null}""");
        using var empty = JsonDocument.Parse("""{"targetInfos":[]}""");
        using var duplicate = JsonDocument.Parse(
            """{"targetInfos":[{"targetId":"x","type":"page","url":"https://synthetic.cos.test/"},{"targetId":"x","type":"page","url":"https://synthetic.cos.test/"}]}""");

        static void Must(bool pass, string check)
        {
            if (!pass) throw new InvalidOperationException("MV3 proof self-test failed: " + check);
        }
        bool Gate(bool close, bool missing, string? current = "old-worker",
            ulong host = 12, ulong worker = 8, ulong channel = 99) =>
            MayRetireAfterInventory(close, missing, "old-worker", current,
                12, host, 8, worker, 99, channel);

        var verifiedMissing = ExactTargetAbsent(absent.RootElement, "old-worker");
        Must(verifiedMissing && Gate(true, verifiedMissing),
            "missing/late targetDestroyed event + confirmed native absence");
        Must(!ExactTargetAbsent(present.RootElement, "old-worker"),
            "live exact worker must not retire");
        Must(!Gate(true, ExactTargetAbsent(present.RootElement, "old-worker")),
            "Target.closeTarget.success alone is never proof");
        Must(!Gate(false, verifiedMissing), "injected closeTarget failure");
        Must(!Gate(true, false), "no inventory proof");
        Must(!Gate(true, verifiedMissing, "other-worker"), "changed target identity");
        Must(!Gate(true, verifiedMissing, host: 13), "changed host generation");
        Must(!Gate(true, verifiedMissing, worker: 9), "changed worker generation");
        Must(!Gate(true, verifiedMissing, channel: 100), "changed Rust channel");
        Must(!ExactTargetAbsent(malformed.RootElement, "old-worker"),
            "malformed native CDP response");
        Must(!ExactTargetAbsent(empty.RootElement, "old-worker"),
            "empty inventory cannot attest target disappearance");
        Must(!ExactTargetAbsent(duplicate.RootElement, "old-worker"),
            "duplicate target IDs");
        Must(!ExactTargetAbsent(absent.RootElement, ""), "missing exact target ID");
    }
}
