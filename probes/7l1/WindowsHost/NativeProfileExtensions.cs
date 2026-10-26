using System.Runtime.InteropServices;
using System.Security.Cryptography.X509Certificates;

namespace CoSoS.Probe7L1.WindowsHost;

// Native-only bootstrap inventory. These two IDs are built into Microsoft's
// WebView2 runtime. Display names and page/worker input NEVER confer trust.
// The caller proves an empty new profile BEFORE creating the environment,
// verifies the actual browser process executable, and rechecks the inventory.
internal static class NativeProfileExtensions
{
    private static readonly HashSet<string> Components = new(StringComparer.Ordinal)
    {
        "dgiklkfkllikcanfonkcabmbdfmgleag", // Microsoft Clipboard component
        "mhjfbmdgcfjbbpaeojofohoefgiehjai"  // Microsoft PDF component
    };

    internal static bool Accepts(IEnumerable<string> ids, bool verifiedRuntime, string? ownedFixture)
    {
        if (!verifiedRuntime || (ownedFixture is not null &&
            (!CanonicalId(ownedFixture) || Components.Contains(ownedFixture)))) return false;
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var id in ids)
        {
            if (!CanonicalId(id) || !seen.Add(id) || seen.Count > 3 ||
                (!Components.Contains(id) && id != ownedFixture)) return false;
        }
        return ownedFixture is null || seen.Contains(ownedFixture);
    }

    private static bool CanonicalId(string id) =>
        id.Length == 32 && id.All(c => c is >= 'a' and <= 'p');

    internal static bool VerifiedMicrosoftRuntime(string path)
    {
        if (!OperatingSystem.IsWindows() ||
            !string.Equals(Path.GetFileName(path), "msedgewebview2.exe", StringComparison.OrdinalIgnoreCase))
            return false;
        try
        {
            var full = Path.GetFullPath(path);
            var versionFolder = Path.GetDirectoryName(full);
            if (versionFolder is null || !Version.TryParse(Path.GetFileName(versionFolder), out _)) return false;
            var application = Path.GetDirectoryName(versionFolder);
            var installed = new[] { Environment.SpecialFolder.ProgramFiles, Environment.SpecialFolder.ProgramFilesX86 }
                .Select(folder => Environment.GetFolderPath(folder))
                .Where(root => !string.IsNullOrWhiteSpace(root))
                .Any(root => string.Equals(application,
                    Path.Combine(root, "Microsoft", "EdgeWebView", "Application"), StringComparison.OrdinalIgnoreCase));
            if (!installed) return false;
            for (string? cursor = full; cursor is not null; cursor = Path.GetDirectoryName(cursor))
                if ((File.GetAttributes(cursor) & FileAttributes.ReparsePoint) != 0) return false;
            // Native Authenticode policy validates the signed file digest and
            // trust chain. Cache-only retrieval keeps qualification offline.
            // Extracting a certificate alone would NOT verify the file digest.
            if (!ValidAuthenticode(full)) return false;
            using var certificate = new X509Certificate2(X509Certificate.CreateFromSignedFile(full));
            return certificate.GetNameInfo(X509NameType.SimpleName, false) == "Microsoft Corporation";
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or
            System.Security.Cryptography.CryptographicException or ArgumentException)
        {
            return false;
        }
    }

    private static bool ValidAuthenticode(string path)
    {
        var action = new Guid("00AAC56B-CD44-11d0-8CC2-00C04FC295EE");
        var name = Marshal.StringToCoTaskMemUni(path);
        var file = new TrustFile { Size = (uint)Marshal.SizeOf<TrustFile>(), Path = name };
        var pointer = Marshal.AllocHGlobal(Marshal.SizeOf<TrustFile>());
        Marshal.StructureToPtr(file, pointer, false);
        var trust = new TrustData
        {
            Size = (uint)Marshal.SizeOf<TrustData>(), UiChoice = 2,
            UnionChoice = 1, File = pointer, StateAction = 1,
            ProviderFlags = 0x1000
        };
        try { return WinVerifyTrust(new IntPtr(-1), ref action, ref trust) == 0; }
        finally
        {
            trust.StateAction = 2;
            _ = WinVerifyTrust(new IntPtr(-1), ref action, ref trust);
            Marshal.FreeHGlobal(pointer);
            Marshal.FreeCoTaskMem(name);
        }
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct TrustFile
    {
        public uint Size;
        public IntPtr Path, File, KnownSubject;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct TrustData
    {
        public uint Size;
        public IntPtr PolicyCallback, SipClient;
        public uint UiChoice, RevocationChecks, UnionChoice;
        public IntPtr File;
        public uint StateAction;
        public IntPtr StateData, UrlReference;
        public uint ProviderFlags, UiContext;
        public IntPtr SignatureSettings;
    }

    [DllImport("wintrust.dll", ExactSpelling = true)]
    private static extern int WinVerifyTrust(IntPtr window, ref Guid action, ref TrustData data);
}
