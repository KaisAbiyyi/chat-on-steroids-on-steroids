// 16PKG-01: native Windows installer for the source-pinned Rust + WebView2 product.
// Non-test mutations refuse an unsigned publisher and every linked D: ancestor.
// No shell, profile import, credential lookup, or automatic updater download.
using System.Diagnostics;
using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

internal static class Program
{
    private const long MaxBytes = 1024L * 1024 * 1024;
    private const long MaxFile = 512L * 1024 * 1024;
    private const int MaxFiles = 251;
    private static readonly Regex Hex40 = new("^[a-f0-9]{40}$", RegexOptions.CultureInvariant);
    private static readonly Regex Thumb = new("^[0-9A-F]{40,64}$", RegexOptions.CultureInvariant);
    private static readonly StringComparer ICase = StringComparer.OrdinalIgnoreCase;

    private static int Main(string[] args)
    {
        try
        {
            if (args.Length == 1 && args[0] == "--verify")
            {
                var bytes = Resource("CoSoS.Payload");
                var source = Encoding.UTF8.GetString(Resource("CoSoS.Source")).Trim();
                VerifyBundle(bytes, source);
                Console.WriteLine("CANDIDATE_VERIFIED_UNSIGNED_NOT_RELEASED source=" + source
                    + " payload_sha256=" + Hash(bytes));
                return 0;
            }
            if (args.Length != 5 || args[0] is not ("--install" or "--update" or "--rollback" or "--uninstall")
                || args[3] != "--publisher")
                throw new InvalidDataException("Usage: CoSoSInstaller --verify OR --install|--update|--rollback|--uninstall D:\\path --publisher TRUSTED_THUMBPRINT");
            string operation = args[0], root = SafeRoot(args[1]), expectedSigner = args[4].ToUpperInvariant();
            if (!Thumb.IsMatch(expectedSigner))
                throw new InvalidDataException("Trusted publisher certificate thumbprint required");
            VerifyAuthenticode(Environment.ProcessPath!, expectedSigner);
            if (operation == "--install" || operation == "--update")
            {
                var source = Encoding.UTF8.GetString(Resource("CoSoS.Source")).Trim();
                var bytes = Resource("CoSoS.Payload");
                VerifyBundle(bytes, source);
                Apply(bytes, source, root, operation == "--install", expectedSigner);
            }
            else if (operation == "--rollback") Rollback(root, expectedSigner);
            else Uninstall(root, expectedSigner);
            Console.WriteLine("COSOS_OPERATION_SUCCESS operation=" + operation);
            return 0;
        }
        catch (Exception ex) when (ex is IOException or InvalidDataException or UnauthorizedAccessException
                                   or ArgumentException or System.ComponentModel.Win32Exception
                                   or CryptographicException)
        {
            Console.Error.WriteLine("COSOS_INSTALL_REFUSED: " + ex.GetType().Name + ": " + ex.Message);
            return 3;
        }
    }

    private static byte[] Resource(string name)
    {
        using var s = Assembly.GetExecutingAssembly().GetManifestResourceStream(name);
        if (s is null) throw new InvalidDataException("Required signed-build resource missing: " + name);
        using var memory = new MemoryStream();
        s.CopyTo(memory);
        if (memory.Length > MaxBytes) throw new InvalidDataException("Oversize embedded package");
        return memory.ToArray();
    }

    private static string Hash(byte[] bytes) => Convert.ToHexString(SHA256.HashData(bytes)).ToLowerInvariant();
    private static string HashFile(string path)
    {
        using var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        return Convert.ToHexString(SHA256.HashData(file)).ToLowerInvariant();
    }

    private static void SafeAncestors(string full)
    {
        if (Path.GetPathRoot(full) is not string drive || !ICase.Equals(drive, @"D:\"))
            throw new InvalidDataException("Only an actual isolated D: destination allowed");
        string relative = full.Substring(drive.Length);
        string walk = drive;
        foreach (var segment in relative.Split(Path.DirectorySeparatorChar, StringSplitOptions.RemoveEmptyEntries))
        {
            if (segment is "." or "..") throw new InvalidDataException("Path traversal refused");
            walk = Path.Combine(walk, segment);
            if (File.Exists(walk) || Directory.Exists(walk))
            {
                if ((File.GetAttributes(walk) & FileAttributes.ReparsePoint) != 0)
                    throw new InvalidDataException("Reparse point in path refused");
            }
        }
    }

    private static string SafeRoot(string root)
    {
        if (!Path.IsPathFullyQualified(root)) throw new InvalidDataException("Absolute install root required");
        var full = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar);
        SafeAncestors(full);
        if (ICase.Equals(full, "D:") || ICase.Equals(full, @"D:\"))
            throw new InvalidDataException("Drive root install forbidden");
        return full;
    }

    private static bool SafeMember(string name)
    {
        if (name.Length == 0 || name.Length > 240 || name.StartsWith('/') || name.Contains('\\')
            || name.Contains(':') || name.Contains('\0')) return false;
        return name.Split('/').All(part => part.Length != 0 && part != "." && part != "..");
    }

    private static Dictionary<string, (string Sha, long Size)> ReadManifest(byte[] raw, string source)
    {
        if (raw.Length > 128 * 1024) throw new InvalidDataException("Oversize release manifest");
        using var doc = JsonDocument.Parse(raw);
        var head = doc.RootElement;
        if (head.ValueKind != JsonValueKind.Object || head.EnumerateObject().Count() != 6
            || head.GetProperty("schema").GetString() != "cosos.release.v1"
            || head.GetProperty("product").GetString() != "CoSoS"
            || head.GetProperty("platform").GetString() != "windows-x64"
            || head.GetProperty("source_sha").GetString() != source || !Hex40.IsMatch(source))
            throw new InvalidDataException("Release source and schema mismatch");
        string? version = head.GetProperty("version").GetString();
        if (version is null || !Regex.IsMatch(version, @"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$"))
            throw new InvalidDataException("Release version invalid");
        var files = head.GetProperty("files");
        if (files.ValueKind != JsonValueKind.Array || files.GetArrayLength() > MaxFiles
            || files.GetArrayLength() == 0) throw new InvalidDataException("Invalid release file count");
        var indexed = new Dictionary<string, (string, long)>(ICase);
        foreach (var e in files.EnumerateArray())
        {
            if (e.ValueKind != JsonValueKind.Object || e.EnumerateObject().Count() != 3)
                throw new InvalidDataException("Unexpected release manifest member");
            string name = e.GetProperty("path").GetString() ?? "";
            string sha = e.GetProperty("sha256").GetString() ?? "";
            long size = e.GetProperty("bytes").GetInt64();
            if (!SafeMember(name) || name == "release.json" || sha.Length != 64
                || !sha.All(Uri.IsHexDigit) || size <= 0 || size > MaxFile
                || !indexed.TryAdd(name, (sha, size)))
                throw new InvalidDataException("Unsafe release index");
        }
        foreach (string name in new[] { "bin/cos-rust.exe", "renderer/WindowsRenderer.exe",
                     "renderer/WindowsRenderer.dll", "renderer/renderer-integrity.json", "LICENSE.txt" })
            if (!indexed.ContainsKey(name)) throw new InvalidDataException("Missing real product " + name);
        return indexed;
    }

    private static Dictionary<string, byte[]> VerifyBundle(byte[] bundle, string source)
    {
        var payload = new Dictionary<string, byte[]>(ICase);
        long total = 0;
        using var zip = new ZipArchive(new MemoryStream(bundle, false), ZipArchiveMode.Read);
        if (zip.Entries.Count > MaxFiles + 1) throw new InvalidDataException("Package member cap");
        foreach (var entry in zip.Entries)
        {
            if (!SafeMember(entry.FullName) || entry.FullName.EndsWith('/')
                || (entry.ExternalAttributes >> 16 & 0xF000) == 0xA000
                || entry.Length <= 0 || entry.Length > MaxFile)
                throw new InvalidDataException("Unsafe ZIP member");
            total += entry.Length;
            if (total > MaxBytes) throw new InvalidDataException("Expanded ZIP cap");
            using var input = entry.Open();
            using var memory = new MemoryStream();
            input.CopyTo(memory);
            if (memory.Length != entry.Length || !payload.TryAdd(entry.FullName, memory.ToArray()))
                throw new InvalidDataException("Duplicate, case-aliased or truncated ZIP member");
        }
        if (!payload.TryGetValue("release.json", out var raw))
            throw new InvalidDataException("Missing release manifest");
        var index = ReadManifest(raw, source);
        if (payload.Count != index.Count + 1)
            throw new InvalidDataException("Unmanifested package member");
        foreach (var (name, expected) in index)
        {
            if (!payload.TryGetValue(name, out var data) || data.LongLength != expected.Size
                || !ICase.Equals(Hash(data), expected.Sha))
                throw new InvalidDataException("Payload digest mismatch");
        }
        // The 15HOST rust renderer will enforce its independent complete
        // renderer-integrity.json manifest on actual launch.
        return payload;
    }

    private static void VerifyAuthenticode(string exe, string expected)
    {
        // Windows-owned authenticode trust engine, fixed System32 PowerShell.
        string ps = Environment.GetFolderPath(Environment.SpecialFolder.System);
        string shell = Path.GetFullPath(Path.Combine(ps, @"WindowsPowerShell\v1.0\powershell.exe"));
        string script = "$ErrorActionPreference='Stop';$s=Get-AuthenticodeSignature -LiteralPath '"
            + exe.Replace("'", "''") + "';"
            + "if($s.Status -ne 'Valid' -or $null -eq $s.SignerCertificate){exit 7};"
            + "[Console]::Out.Write($s.SignerCertificate.Thumbprint)";
        string encoded = Convert.ToBase64String(Encoding.Unicode.GetBytes(script));
        using var child = Process.Start(new ProcessStartInfo(shell) {
            UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true,
            CreateNoWindow = true, Arguments = "-NoProfile -NonInteractive -EncodedCommand " + encoded
        }) ?? throw new InvalidDataException("No Authenticode verifier");
        if (!child.WaitForExit(30000))
        {
            child.Kill(true);
            throw new InvalidDataException("Authenticode verifier timed out");
        }
        var actual = child.StandardOutput.ReadToEnd().Trim();
        if (child.ExitCode != 0 || !ICase.Equals(actual, expected))
            throw new InvalidDataException("Untrusted/unsigned native setup issuer");
    }

    private static string StateFile(string root) => Path.Combine(root, "active.json");
    private static (string Active, string? Previous, string Publisher) State(string root)
    {
        var path = StateFile(root);
        SafeAncestors(path);
        if (!File.Exists(path) || new FileInfo(path).Length > 4096)
            throw new InvalidDataException("Missing or oversized installation state");
        using var json = JsonDocument.Parse(File.ReadAllBytes(path));
        var obj = json.RootElement;
        if (obj.EnumerateObject().Count() != 3)
            throw new InvalidDataException("Unexpected installation state");
        string active = obj.GetProperty("active").GetString() ?? "";
        string? previous = obj.GetProperty("previous").GetString();
        string publisher = obj.GetProperty("publisher").GetString() ?? "";
        if (!SlotName(active) || (previous is not null && (!SlotName(previous) || previous == active))
            || !Thumb.IsMatch(publisher))
            throw new InvalidDataException("Unsafe install selector");
        return (active, previous, publisher);
    }
    private static bool SlotName(string name) => Regex.IsMatch(name, @"^slot-[0-9a-f]{32}$");

    private static void WriteState(string root, string active, string? previous, string publisher)
    {
        var temp = Path.Combine(root, "state-" + Guid.NewGuid().ToString("N") + ".tmp");
        File.WriteAllText(temp, JsonSerializer.Serialize(new { active, previous, publisher }),
            new UTF8Encoding(false));
        File.Move(temp, StateFile(root), true); // same-volume atomic replacement
    }

    private static void ExtractTo(string directory, Dictionary<string, byte[]> payload)
    {
        Directory.CreateDirectory(directory);
        foreach (var (name, bytes) in payload)
        {
            var file = Path.GetFullPath(Path.Combine(directory, name.Replace('/', Path.DirectorySeparatorChar)));
            if (!file.StartsWith(directory + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("ZIP escaped staging directory");
            SafeAncestors(file);
            Directory.CreateDirectory(Path.GetDirectoryName(file)!);
            using var stream = new FileStream(file, FileMode.CreateNew, FileAccess.Write, FileShare.None);
            stream.Write(bytes);
        }
    }

    private static void VerifySlot(string root, string name)
    {
        if (!SlotName(name)) throw new InvalidDataException("Invalid version slot");
        var slot = Path.Combine(root, "slots", name);
        SafeAncestors(slot);
        var release = Path.Combine(slot, "release.json");
        if (!File.Exists(release)) throw new InvalidDataException("Missing slot manifest");
        using var doc = JsonDocument.Parse(File.ReadAllBytes(release));
        string source = doc.RootElement.GetProperty("source_sha").GetString() ?? "";
        var indexed = ReadManifest(File.ReadAllBytes(release), source);
        var files = Directory.GetFiles(slot, "*", SearchOption.AllDirectories);
        if (files.Length != indexed.Count + 1) throw new InvalidDataException("Slot has extra/unowned files");
        foreach (var (relative, expected) in indexed)
        {
            var file = Path.Combine(slot, relative.Replace('/', Path.DirectorySeparatorChar));
            SafeAncestors(file);
            if (!File.Exists(file) || new FileInfo(file).Length != expected.Size
                || !ICase.Equals(HashFile(file), expected.Sha))
                throw new InvalidDataException("Retained slot tampered");
        }
    }

    private static void Apply(byte[] package, string source, string root, bool install, string publisher)
    {
        var payload = VerifyBundle(package, source);
        if (install && (File.Exists(root) || Directory.Exists(root)))
            throw new InvalidDataException("Refuse overwrite of pre-existing destination");
        if (!install && !Directory.Exists(root))
            throw new InvalidDataException("Update requires an installed root");
        (string Active, string? Previous, string Publisher) before = default;
        if (!install)
        {
            before = State(root);
            if (!ICase.Equals(before.Publisher, publisher))
                throw new InvalidDataException("Release publisher changed");
            VerifySlot(root, before.Active);
            using var old = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(root, "slots", before.Active, "release.json")));
            using var next = JsonDocument.Parse(payload["release.json"]);
            Version current = Version.Parse(old.RootElement.GetProperty("version").GetString()!);
            Version incoming = Version.Parse(next.RootElement.GetProperty("version").GetString()!);
            if (incoming <= current) throw new InvalidDataException("Downgrade/reinstall refused");
        }
        string parent = Path.GetDirectoryName(root)!;
        SafeAncestors(parent);
        Directory.CreateDirectory(parent);
        var stageRoot = install ? Path.Combine(parent, "cosos-stage-" + Guid.NewGuid().ToString("N")) : root;
        bool createdStage = false, published = false;
        var slot = "slot-" + Guid.NewGuid().ToString("N");
        var dir = Path.Combine(stageRoot, "slots", slot);
        try
        {
            if (install) { Directory.CreateDirectory(stageRoot); createdStage = true; }
            ExtractTo(dir, payload);
            VerifySlot(stageRoot, slot);
            if (install)
            {
                WriteState(stageRoot, slot, null, publisher);
                SafeAncestors(root);
                Directory.Move(stageRoot, root);
            }
            else WriteState(root, slot, before.Active, publisher);
            published = true;
        }
        finally
        {
            if (!published)
            {
                if (createdStage && Directory.Exists(stageRoot)) Directory.Delete(stageRoot, true);
                else if (Directory.Exists(dir)) Directory.Delete(dir, true);
            }
        }
    }

    private static void Rollback(string root, string publisher)
    {
        var before = State(root);
        if (!ICase.Equals(before.Publisher, publisher) || before.Previous is null)
            throw new InvalidDataException("Rollback target/publisher unavailable");
        VerifySlot(root, before.Active);
        VerifySlot(root, before.Previous);
        WriteState(root, before.Previous, before.Active, publisher);
    }

    private static void Uninstall(string root, string publisher)
    {
        var before = State(root);
        if (!ICase.Equals(before.Publisher, publisher))
            throw new InvalidDataException("Unexpected uninstall publisher");
        VerifySlot(root, before.Active);
        if (before.Previous is not null) VerifySlot(root, before.Previous);
        // Refuse any unowned file. No partial uninstall on a modified install.
        var expected = new HashSet<string>(ICase) { StateFile(root) };
        foreach (var name in new[] { before.Active, before.Previous })
        {
            if (name is null) continue;
            var folder = Path.Combine(root, "slots", name);
            foreach (var path in Directory.GetFiles(folder, "*", SearchOption.AllDirectories))
                expected.Add(path);
        }
        var actual = Directory.GetFiles(root, "*", SearchOption.AllDirectories);
        if (!expected.SetEquals(actual))
            throw new InvalidDataException("Unowned/modified installation bytes remain; uninstall refused");
        foreach (var file in actual) { SafeAncestors(file); File.Delete(file); }
        foreach (var dir in Directory.GetDirectories(root, "*", SearchOption.AllDirectories)
                      .OrderByDescending(s => s.Length))
        {
            SafeAncestors(dir);
            Directory.Delete(dir);
        }
        Directory.Delete(root);
    }
}
