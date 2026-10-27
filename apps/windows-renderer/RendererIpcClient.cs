using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace CoSoS.WindowsRenderer;

/// <summary>
/// Authenticated child-process transport. The key never enters the WebView DOM;
/// the renderer only sees host-projected, bounded presentation state.
/// </summary>
internal sealed class RendererIpcClient : IDisposable
{
    internal const uint Protocol = 1;
    internal const int MaximumFrameBytes = 512 * 1024;
    internal const int MaximumPayloadBytes = 256 * 1024;
    internal const ulong MaximumSafeInteger = 9_007_199_254_740_991UL;
    private static readonly byte[] Domain = Encoding.UTF8.GetBytes("CoSoS/15HOST/native-ui/v1\0");
    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = false };

    private readonly Stream _input;
    private readonly Stream _output;
    private readonly object _writeLock = new();
    private byte[] _key;
    private ulong _epoch;
    private ulong _inboundSequence;
    private ulong _outboundSequence;
    private bool _disposed;

    private RendererIpcClient(ulong epoch, byte[] key, Stream input, Stream output)
    {
        if (epoch == 0 || epoch > MaximumSafeInteger || key.Length != 32)
            throw new InvalidDataException("Invalid renderer IPC bootstrap");
        _epoch = epoch;
        _key = key.ToArray();
        _input = input;
        _output = output;
    }

    internal static RendererIpcClient FromBootstrap(string raw, Stream input, Stream output)
    {
        using var document = JsonDocument.Parse(raw);
        var root = document.RootElement;
        if (root.ValueKind != JsonValueKind.Object || root.EnumerateObject().Count() != 4 ||
            !root.TryGetProperty("kind", out var kind) || kind.GetString() != "bootstrap" ||
            !root.TryGetProperty("protocol", out var protocol) ||
            !protocol.TryGetUInt32(out var version) || version != Protocol ||
            !root.TryGetProperty("epoch", out var epochValue) ||
            !epochValue.TryGetUInt64(out var epoch) ||
            !root.TryGetProperty("key", out var keyValue) ||
            keyValue.ValueKind != JsonValueKind.String)
            throw new InvalidDataException("Invalid renderer IPC bootstrap");

        byte[] key;
        try { key = Convert.FromHexString(keyValue.GetString() ?? string.Empty); }
        catch (FormatException) { throw new InvalidDataException("Invalid renderer IPC bootstrap"); }
        try
        {
            if (key.Length != 32) throw new InvalidDataException("Invalid renderer IPC bootstrap");
            return new RendererIpcClient(epoch, key, input, output);
        }
        finally { CryptographicOperations.ZeroMemory(key); }
    }

    internal static RendererIpcClient CreateForTest(ulong epoch, byte[] key) =>
        new(epoch, key, new MemoryStream(), new MemoryStream());

    internal string SignOutbound(string payloadJson)
    {
        ThrowIfDisposed();
        var payload = Encoding.UTF8.GetBytes(payloadJson);
        if (payload.Length > MaximumPayloadBytes) throw new InvalidDataException("Renderer payload too large");
        using (JsonDocument.Parse(payloadJson)) { }

        var sequence = Next(_outboundSequence);
        var mac = HMACSHA256.HashData(_key, MacInput(Protocol, _epoch, sequence, payload));
        var frame = new WireFrame
        {
            Protocol = Protocol,
            Epoch = _epoch,
            Sequence = sequence,
            PayloadJson = payloadJson,
            Mac = Convert.ToHexString(mac).ToLowerInvariant()
        };
        var serialized = JsonSerializer.Serialize(frame, JsonOptions);
        if (Encoding.UTF8.GetByteCount(serialized) > MaximumFrameBytes)
            throw new InvalidDataException("Renderer frame too large");
        _outboundSequence = sequence;
        CryptographicOperations.ZeroMemory(mac);
        return serialized;
    }

    internal void SendPayload(string payloadJson)
    {
        var frame = SignOutbound(payloadJson);
        var bytes = Encoding.UTF8.GetBytes(frame + "\n");
        lock (_writeLock)
        {
            ThrowIfDisposed();
            _output.Write(bytes, 0, bytes.Length);
            _output.Flush();
        }
    }

    internal bool TryVerifyInbound(string raw, out string payloadJson, out string code)
    {
        payloadJson = string.Empty;
        code = "invalid-renderer-frame";
        if (_disposed) { code = "renderer-transport-disposed"; return false; }
        if (Encoding.UTF8.GetByteCount(raw) > MaximumFrameBytes)
        {
            code = "renderer-frame-too-large";
            return false;
        }

        WireFrame frame;
        try
        {
            using var document = JsonDocument.Parse(raw);
            var root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object || root.EnumerateObject().Count() != 5 ||
                !root.TryGetProperty("protocol", out var protocol) ||
                !protocol.TryGetUInt32(out var version) ||
                !root.TryGetProperty("epoch", out var epochValue) ||
                !epochValue.TryGetUInt64(out var epoch) ||
                !root.TryGetProperty("sequence", out var sequenceValue) ||
                !sequenceValue.TryGetUInt64(out var sequence) ||
                !root.TryGetProperty("payloadJson", out var payloadValue) ||
                payloadValue.ValueKind != JsonValueKind.String ||
                !root.TryGetProperty("mac", out var macValue) ||
                macValue.ValueKind != JsonValueKind.String)
                return false;

            frame = new WireFrame
            {
                Protocol = version,
                Epoch = epoch,
                Sequence = sequence,
                PayloadJson = payloadValue.GetString() ?? string.Empty,
                Mac = macValue.GetString() ?? string.Empty
            };
        }
        catch (JsonException) { return false; }

        if (frame.Protocol != Protocol) { code = "unsupported-renderer-protocol"; return false; }
        if (frame.Epoch != _epoch) { code = "stale-renderer-epoch"; return false; }
        if (frame.Sequence == 0 || frame.Sequence > MaximumSafeInteger ||
            _inboundSequence >= MaximumSafeInteger ||
            frame.Sequence != _inboundSequence + 1)
        {
            code = "invalid-renderer-sequence";
            return false;
        }

        var payload = Encoding.UTF8.GetBytes(frame.PayloadJson);
        if (payload.Length > MaximumPayloadBytes)
        {
            code = "renderer-frame-too-large";
            return false;
        }

        byte[] supplied;
        try
        {
            if (frame.Mac.Length != 64) return false;
            supplied = Convert.FromHexString(frame.Mac);
        }
        catch (FormatException) { return false; }

        var expected = HMACSHA256.HashData(_key,
            MacInput(frame.Protocol, frame.Epoch, frame.Sequence, payload));
        var authenticated = CryptographicOperations.FixedTimeEquals(supplied, expected);
        CryptographicOperations.ZeroMemory(supplied);
        CryptographicOperations.ZeroMemory(expected);
        if (!authenticated)
        {
            code = "renderer-authentication-failed";
            return false;
        }

        // A valid MAC consumes the sequence even if the inner payload is bad.
        _inboundSequence = frame.Sequence;
        try
        {
            using var inner = JsonDocument.Parse(frame.PayloadJson);
            if (inner.RootElement.ValueKind is not (JsonValueKind.Object or JsonValueKind.Array))
                return false;
        }
        catch (JsonException) { return false; }

        payloadJson = frame.PayloadJson;
        code = string.Empty;
        return true;
    }

    internal async Task ReadLoopAsync(
        Action<string> onPayload,
        Action<string> onFailure,
        CancellationToken cancellationToken)
    {
        try
        {
            await Task.Run(() =>
            {
                while (!cancellationToken.IsCancellationRequested)
                {
                    var raw = ReadBoundedLine(_input, MaximumFrameBytes);
                    if (raw is null)
                    {
                        if (!cancellationToken.IsCancellationRequested)
                            onFailure("renderer-host-closed");
                        return;
                    }
                    if (!TryVerifyInbound(raw, out var payload, out var code))
                    {
                        onFailure(code);
                        return;
                    }
                    onPayload(payload);
                }
            }, CancellationToken.None).ConfigureAwait(false);
        }
        catch (Exception) when (cancellationToken.IsCancellationRequested) { }
        catch (Exception) { onFailure("renderer-transport-failed"); }
    }

    internal void Rebind(ulong epoch, byte[] key)
    {
        ThrowIfDisposed();
        if (epoch <= _epoch || epoch > MaximumSafeInteger || key.Length != 32)
            throw new InvalidDataException("Invalid renderer IPC rebind");
        CryptographicOperations.ZeroMemory(_key);
        _key = key.ToArray();
        CryptographicOperations.ZeroMemory(key);
        _epoch = epoch;
        _inboundSequence = 0;
        _outboundSequence = 0;
    }

    internal static string? ReadBoundedLine(Stream input, int maximumBytes)
    {
        var bytes = new List<byte>(Math.Min(maximumBytes, 4096));
        while (true)
        {
            var next = input.ReadByte();
            if (next < 0)
            {
                if (bytes.Count == 0) return null;
                throw new EndOfStreamException("Truncated renderer frame");
            }
            if (next == '\n') break;
            if (bytes.Count >= maximumBytes) throw new InvalidDataException("Renderer frame too large");
            bytes.Add((byte)next);
        }
        if (bytes.Count > 0 && bytes[^1] == '\r') bytes.RemoveAt(bytes.Count - 1);
        return new UTF8Encoding(false, true).GetString(bytes.ToArray());
    }

    internal static void SelfTest()
    {
        const string ready = "{\"kind\":\"ready\",\"rendererNonce\":\"00112233445566778899aabbccddeeff\"}";
        var key = Enumerable.Repeat((byte)0x0b, 32).ToArray();
        using var sender = CreateForTest(7, key);
        var frame = sender.SignOutbound(ready);
        using (var parsed = JsonDocument.Parse(frame))
        {
            if (parsed.RootElement.GetProperty("mac").GetString() !=
                "276dfc24a1e23b4c973d99dc1f9b76df690c5081bb4a66e3b09417d691df07d1")
                throw new InvalidOperationException("15HOST HMAC cross-language vector failed");
        }

        using var receiver = CreateForTest(7, key);
        if (!receiver.TryVerifyInbound(frame, out var payload, out _) || payload != ready)
            throw new InvalidOperationException("15HOST authenticated frame verification failed");
        if (receiver.TryVerifyInbound(frame, out _, out var replayCode) ||
            replayCode != "invalid-renderer-sequence")
            throw new InvalidOperationException("15HOST replay regression failed");

        using var stale = CreateForTest(8, key);
        if (stale.TryVerifyInbound(frame, out _, out var staleCode) || staleCode != "stale-renderer-epoch")
            throw new InvalidOperationException("15HOST stale epoch regression failed");

        using var otherProfile = CreateForTest(7, Enumerable.Repeat((byte)0x0c, 32).ToArray());
        if (otherProfile.TryVerifyInbound(frame, out _, out var profileCode) ||
            profileCode != "renderer-authentication-failed")
            throw new InvalidOperationException("15HOST cross-profile isolation regression failed");

        var downgrade = frame.Replace("\"protocol\":1", "\"protocol\":0", StringComparison.Ordinal);
        if (receiver.TryVerifyInbound(downgrade, out _, out var downgradeCode) ||
            downgradeCode != "unsupported-renderer-protocol")
            throw new InvalidOperationException("15HOST protocol downgrade regression failed");
        if (receiver.TryVerifyInbound("{", out _, out _))
            throw new InvalidOperationException("15HOST malformed frame regression failed");
        if (receiver.TryVerifyInbound(new string('x', MaximumFrameBytes + 1), out _, out var oversizeCode) ||
            oversizeCode != "renderer-frame-too-large")
            throw new InvalidOperationException("15HOST oversized frame regression failed");

        using var rebound = CreateForTest(7, key);
        if (!rebound.TryVerifyInbound(frame, out _, out _))
            throw new InvalidOperationException("15HOST rebind fixture setup failed");
        rebound.Rebind(8, Enumerable.Repeat((byte)0x0d, 32).ToArray());
        if (rebound.TryVerifyInbound(frame, out _, out var retiredCode) || retiredCode != "stale-renderer-epoch")
            throw new InvalidOperationException("15HOST rebind did not retire old frames");
        var fresh = rebound.SignOutbound(ready);
        using var freshFrame = JsonDocument.Parse(fresh);
        if (freshFrame.RootElement.GetProperty("sequence").GetUInt64() != 1 ||
            freshFrame.RootElement.GetProperty("epoch").GetUInt64() != 8)
            throw new InvalidOperationException("15HOST rebind sequence reset failed");

        try
        {
            sender.SignOutbound("{");
            throw new InvalidOperationException("15HOST malformed payload encoder regression failed");
        }
        catch (InvalidDataException) { }
    }

    private static byte[] MacInput(uint protocol, ulong epoch, ulong sequence, byte[] payload)
    {
        var result = new byte[Domain.Length + 20 + payload.Length];
        Domain.CopyTo(result, 0);
        var offset = Domain.Length;
        BinaryPrimitives.WriteUInt32BigEndian(result.AsSpan(offset, 4), protocol);
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(offset + 4, 8), epoch);
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(offset + 12, 8), sequence);
        payload.CopyTo(result, offset + 20);
        return result;
    }

    private static ulong Next(ulong current)
    {
        if (current >= MaximumSafeInteger) throw new InvalidDataException("Renderer sequence exhausted");
        return current + 1;
    }

    private void ThrowIfDisposed()
    {
        if (_disposed) throw new ObjectDisposedException(nameof(RendererIpcClient));
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        CryptographicOperations.ZeroMemory(_key);
    }

    private sealed class WireFrame
    {
        [JsonPropertyName("protocol"), JsonPropertyOrder(0)]
        public uint Protocol { get; set; }
        [JsonPropertyName("epoch"), JsonPropertyOrder(1)]
        public ulong Epoch { get; set; }
        [JsonPropertyName("sequence"), JsonPropertyOrder(2)]
        public ulong Sequence { get; set; }
        [JsonPropertyName("payloadJson"), JsonPropertyOrder(3)]
        public string PayloadJson { get; set; } = string.Empty;
        [JsonPropertyName("mac"), JsonPropertyOrder(4)]
        public string Mac { get; set; } = string.Empty;
    }
}
