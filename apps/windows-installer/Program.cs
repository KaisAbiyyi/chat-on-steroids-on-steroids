// 16PKG: signed release-authority installer. Production operations require OS administrator,
// a pinned trusted Authenticode issuer and administrator-only NTFS installation slots.
// CI uses disposable, self-signed TEST certificates. Neither passes release acceptance.
using System.Diagnostics;
using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;
using System.Security.Principal;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

internal static class Program
{
    const int MaxFiles = 254;
    const long MaxFile = 512L * 1024 * 1024;
    const long MaxTotal = 1024L * 1024 * 1024;
    static readonly StringComparer IC = StringComparer.OrdinalIgnoreCase;
    static readonly Regex Hex40 = new("^[a-f0-9]{40}$", RegexOptions.CultureInvariant);
    static readonly Regex Fingerprint = new("^[0-9A-F]{40,64}$", RegexOptions.CultureInvariant);
    static readonly Regex SlotPattern = new("^slot-[a-f0-9]{32}$", RegexOptions.CultureInvariant);
    record Identity(string Source, string Version, string ManifestSha, string Publisher);
    record Selector(string Active, string? Previous, string Publisher, string HighWater, long Generation);

    static int Main(string[] args)
    {
        try
        {
            if (args.Length == 1 && (args[0] == "--verify" || args[0] == "--identity" || args[0] == "--signed-manifest"))
            {
                var source = Source();
                var payload = Archive();
                var data = VerifyBundle(payload, source);
                if (args[0] == "--signed-manifest")
                {
                    Console.WriteLine(Convert.ToBase64String(data["release.json"]));
                    return 0;
                }
                var version = VersionOf(data["release.json"]);
                var proof = new Identity(source, version, Hash(data["release.json"]), 
                    ResourceText("CoSoS.TrustedPublisher", optional: true));
                if (args[0] == "--identity")
                    Console.WriteLine(JsonSerializer.Serialize(proof));
                else Console.WriteLine(JsonSerializer.Serialize(new {
                    status = "UNSIGNED_CANDIDATE_VERIFIED_ONLY", source, version,
                    bundleSha256 = Hash(payload), accepted = false
                }));
                return 0;
            }
            if (args.Length != 4 || args[2] != "--publisher" ||
                args[0] is not ("--install" or "--update" or "--rollback" or "--uninstall" or "--recover" or "--finalize"))
                throw new InvalidDataException("Usage: --verify|--identity OR --install|--update|--rollback|--uninstall|--recover|--finalize D:\\owned-path --publisher PIN");
            var op = args[0];
            var path = SafeRoot(args[1]);
            var caller = args[3].ToUpperInvariant();
            var pin = ResourceText("CoSoS.TrustedPublisher", optional: false).ToUpperInvariant();
            if (!Fingerprint.IsMatch(caller) || !IC.Equals(caller, pin))
                throw new InvalidDataException("Approved code signing publisher not configured or mismatched");
            RequireAdmin();
            CheckSignature(Environment.ProcessPath!, pin);
            if (op == "--install" || op == "--update")
                InstallOrUpdate(path, op == "--install", pin);
            else if (op == "--rollback") Rollback(path, pin);
            else if (op == "--finalize") FinalizePrevious(path, pin);
            else if (op == "--recover") Recover(path, pin);
            else Uninstall(path, pin);
            Console.WriteLine("COSOS_NATIVE_TRANSACTION_OK:" + op);
            return 0;
        }
        catch (Exception ex) when (ex is IOException or InvalidDataException or UnauthorizedAccessException
             or ArgumentException or System.ComponentModel.Win32Exception or CryptographicException
             or JsonException or InvalidOperationException)
        {
            Console.Error.WriteLine("COSOS_NATIVE_REFUSED:" + ex.GetType().Name + ":" + ex.Message);
            return 3;
        }
    }

    static byte[] Resource(string name)
    {
        using var input = Assembly.GetExecutingAssembly().GetManifestResourceStream(name);
        if (input is null) throw new InvalidDataException("Required signed-build resource absent: " + name);
        using var output = new MemoryStream();
        input.CopyTo(output);
        if (output.Length == 0 || output.Length > MaxTotal) throw new InvalidDataException("Resource size refused");
        return output.ToArray();
    }
    static string ResourceText(string name, bool optional) =>
        optional && Assembly.GetExecutingAssembly().GetManifestResourceStream(name) is null
        ? "" : Encoding.UTF8.GetString(Resource(name)).Trim();
    static string Source()
    {
        var sha = ResourceText("CoSoS.Source", false);
        if (!Hex40.IsMatch(sha)) throw new InvalidDataException("Signed source id invalid");
        return sha;
    }
    static byte[] Archive() => Resource("CoSoS.Payload");
    static string Hash(byte[] bytes) => Convert.ToHexString(SHA256.HashData(bytes)).ToLowerInvariant();
    static string FileHash(string file)
    {
        SafeAncestors(file);
        using var input = new FileStream(file, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (input.Length == 0 || input.Length > MaxFile) throw new InvalidDataException("File size refused");
        return Convert.ToHexString(SHA256.HashData(input)).ToLowerInvariant();
    }
    static Version Semver(string text)
    {
        if (!Regex.IsMatch(text, @"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$"))
            throw new InvalidDataException("Only canonical x.y.z version allowed");
        return Version.Parse(text);
    }
    static bool SafeMember(string name) =>
        name.Length is > 0 and <= 240 && !name.StartsWith('/') && 
        name.IndexOfAny(new[] { '\\', ':', '\0' }) < 0 &&
        name.Split('/').All(part => part.Length != 0 && part is not "." and not "..");

    static void SafeAncestors(string full)
    {
        full = Path.GetFullPath(full);
        if (!IC.Equals(Path.GetPathRoot(full), @"D:\"))
            throw new InvalidDataException("Only physical or disposable D: task-owned paths are allowed");
        string walk = @"D:\";
        foreach (var component in full.Substring(3).Split('\\', StringSplitOptions.RemoveEmptyEntries))
        {
            if (component is "." or "..") throw new InvalidDataException("Unsafe ancestor");
            walk = Path.Combine(walk, component);
            if ((File.Exists(walk) || Directory.Exists(walk)) &&
                (File.GetAttributes(walk) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("Linked ancestor refused");
        }
    }
    static string SafeRoot(string value)
    {
        if (!Path.IsPathFullyQualified(value)) throw new InvalidDataException("Absolute path required");
        var full = Path.GetFullPath(value).TrimEnd('\\');
        SafeAncestors(full);
        if (full.Length < 9) throw new InvalidDataException("Refuse drive/top-level installation");
        return full;
    }

    static (Dictionary<string, (string Hash, long Size)> Files, string Version) Index(byte[] manifest, string source)
    {
        if (manifest.Length > 128 * 1024) throw new InvalidDataException("Manifest oversized");
        using var parsed = JsonDocument.Parse(manifest);
        var root = parsed.RootElement;
        if (root.ValueKind != JsonValueKind.Object || root.EnumerateObject().Count() != 6
            || root.GetProperty("schema").GetString() != "cosos.release.v1"
            || root.GetProperty("product").GetString() != "CoSoS"
            || root.GetProperty("platform").GetString() != "windows-x64"
            || root.GetProperty("source_sha").GetString() != source || !Hex40.IsMatch(source))
            throw new InvalidDataException("Release index identity invalid");
        var ver = root.GetProperty("version").GetString() ?? "";
        Semver(ver);
        var entries = root.GetProperty("files");
        if (entries.ValueKind != JsonValueKind.Array || entries.GetArrayLength() is < 5 or > MaxFiles)
            throw new InvalidDataException("Invalid release index count");
        var files = new Dictionary<string, (string, long)>(IC);
        foreach (var element in entries.EnumerateArray())
        {
            if (element.EnumerateObject().Count() != 3) throw new InvalidDataException("Unexpected entry schema");
            string path = element.GetProperty("path").GetString() ?? "";
            string digest = element.GetProperty("sha256").GetString() ?? "";
            long bytes = element.GetProperty("bytes").GetInt64();
            if (!SafeMember(path) || digest.Length != 64 || !digest.All(Uri.IsHexDigit)
                || bytes <= 0 || bytes > MaxFile || !files.TryAdd(path, (digest, bytes)))
                throw new InvalidDataException("Bad index path/digest");
        }
        foreach (var required in new[] { "bin/cos-rust.exe","renderer/WindowsRenderer.exe",
                 "renderer/WindowsRenderer.dll","renderer/renderer-integrity.json","LICENSE.txt" })
            if (!files.ContainsKey(required)) throw new InvalidDataException("Incomplete desktop app payload");
        return (files, ver);
    }
    static string VersionOf(byte[] manifest)
    {
        using var j = JsonDocument.Parse(manifest);
        var v = j.RootElement.GetProperty("version").GetString() ?? "";
        Semver(v);
        return v;
    }

    static Dictionary<string, byte[]> VerifyBundle(byte[] archive, string source)
    {
        var payload = new Dictionary<string, byte[]>(IC);
        using var zip = new ZipArchive(new MemoryStream(archive, false), ZipArchiveMode.Read);
        long total = 0;
        if (zip.Entries.Count > MaxFiles + 1) throw new InvalidDataException("ZIP count refused");
        foreach (var e in zip.Entries)
        {
            if (!SafeMember(e.FullName) || e.FullName.EndsWith('/') || e.Length is <= 0 or > MaxFile
                || (e.ExternalAttributes >> 16 & 0xF000) == 0xA000)
                throw new InvalidDataException("Unsafe archive member");
            total += e.Length;
            if (total > MaxTotal) throw new InvalidDataException("Expanded ZIP exceeded bound");
            using var stream = e.Open();
            using var result = new MemoryStream();
            stream.CopyTo(result);
            if (result.Length != e.Length || !payload.TryAdd(e.FullName, result.ToArray()))
                throw new InvalidDataException("Duplicated or truncated ZIP member");
        }
        if (!payload.TryGetValue("release.json", out var manifest))
            throw new InvalidDataException("Missing release manifest");
        var index = Index(manifest, source).Files;
        if (index.Count != payload.Count - 1) throw new InvalidDataException("Unexpected ZIP member");
        foreach (var (name, pair) in index)
        {
            if (!payload.TryGetValue(name, out var bytes) || bytes.LongLength != pair.Size
                || !IC.Equals(Hash(bytes), pair.Hash))
                throw new InvalidDataException("Payload hash mismatch");
        }
        using var inner = JsonDocument.Parse(payload["renderer/renderer-integrity.json"]);
        if (inner.RootElement.GetProperty("sourceSha").GetString() != source)
            throw new InvalidDataException("15HOST renderer source mismatch");
        return payload;
    }

    static string Powershell() => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.System), @"WindowsPowerShell\v1.0\powershell.exe");
    static string RunChecked(string binary, params string[] args)
    {
        var psi = new ProcessStartInfo(binary) { UseShellExecute = false, CreateNoWindow = true,
            RedirectStandardOutput = true, RedirectStandardError = true };
        foreach (var a in args) psi.ArgumentList.Add(a);
        using var process = Process.Start(psi) ?? throw new InvalidDataException("Required Windows verifier unavailable");
        if (!process.WaitForExit(30000))
        {
            process.Kill(true);
            throw new InvalidDataException("Verifier timeout");
        }
        var output = process.StandardOutput.ReadToEnd();
        if (process.ExitCode != 0 || output.Length > 256 * 1024)
            throw new InvalidDataException("Trusted Windows operation refused");
        return output.Trim();
    }
    static string PS(string script) => RunChecked(Powershell(), "-NoProfile","-NonInteractive",
        "-EncodedCommand",Convert.ToBase64String(Encoding.Unicode.GetBytes(script)));
    static string Quote(string value) => "'" + value.Replace("'","''") + "'";
    static void CheckSignature(string path, string pinned)
    {
        if (path.StartsWith(@"D:\",StringComparison.OrdinalIgnoreCase)) SafeAncestors(path);
        var result = PS("$ErrorActionPreference='Stop';$s=Get-AuthenticodeSignature -LiteralPath "
              + Quote(path) + ";if($s.Status -ne 'Valid' -or !$s.SignerCertificate){exit 7};"
              + "[Console]::Out.Write($s.SignerCertificate.Thumbprint)");
        if (!IC.Equals(result,pinned)) throw new InvalidDataException("Untrusted code signing publisher");
    }
    static void RequireAdmin()
    {
        using var identity = WindowsIdentity.GetCurrent();
        if (!new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator))
            throw new InvalidDataException("Administrator token required: same-user slot security boundary");
    }
    static void Protect(string directory)
    {
        SafeAncestors(directory);
        if (!Directory.Exists(directory)) throw new InvalidDataException("Missing protected root");
        // icacls ACL establishes NTFS admin/system ownership boundary. Never grants
        // Users, Authenticated Users, Creator Owner, Everyone, or inherited ACEs.
        string icacls = Path.Combine(Environment.GetFolderPath(
            Environment.SpecialFolder.System), "icacls.exe");
        RunChecked(icacls, directory, "/inheritance:r", "/grant:r",
            "*S-1-5-32-544:(OI)(CI)F", "*S-1-5-18:(OI)(CI)F");
        RunChecked(icacls, directory, "/setowner", "*S-1-5-32-544");
        VerifyProtection(directory);
    }
    static void VerifyProtection(string directory)
    {
        SafeAncestors(directory);
        // Inspects *explicit effective* ACL, refusing writable non-admin principals
        // and inherited permissions. OS admin is the documented trust boundary.
        var script = "$a=Get-Acl -LiteralPath " + Quote(directory) + ";"
            + "$owner=try{$a.Owner.Translate([Security.Principal.SecurityIdentifier]).Value}catch{$a.Owner};"
            + "if(!$a.AreAccessRulesProtected -or $owner -ne 'S-1-5-32-544'){exit 5};"
            + "$allowed=@('S-1-5-32-544','S-1-5-18');"
            + "foreach($rule in $a.Access){"
            + "$sid=try{$rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value}catch{exit 5};"
            + "if($sid -notin $allowed){exit 5};}"
            + "[Console]::Write('ACL_OK')";
        if (PS(script) != "ACL_OK")
            throw new InvalidDataException("Install directory ACL is not admin/SYSTEM-only");
    }

    static void SyntheticCrash(string point)
    {
#if COS_16PKG_TEST
        if (Environment.GetEnvironmentVariable("COSOS_16PKG_FAIL_AT") == point)
            Environment.FailFast("16PKG_SYNTHETIC_TEST_CRASH:" + point);
#endif
    }

    static string SelectorPath(string root) => Path.Combine(root,"active.json");
    static bool SlotName(string name) => SlotPattern.IsMatch(name);
    static Selector ReadState(string root, string publisher)
    {
        VerifyProtection(root);
        var path = SelectorPath(root);
        SafeAncestors(path);
        if (!File.Exists(path) || new FileInfo(path).Length > 4096)
            throw new InvalidDataException("Missing or oversized protected state");
        var state = JsonSerializer.Deserialize<Selector>(File.ReadAllBytes(path))
            ?? throw new InvalidDataException("Invalid transaction state");
        if (!SlotName(state.Active) || state.Previous is not null &&
             (!SlotName(state.Previous) || state.Previous == state.Active)
             || state.Publisher != publisher || state.Generation < 1)
            throw new InvalidDataException("Invalid state/publisher");
        Semver(state.HighWater);
        return state;
    }
    static void WriteState(string root, Selector state)
    {
        VerifyProtection(root);
        var tmp = Path.Combine(root,"state-" + Guid.NewGuid().ToString("N") + ".tmp");
        File.WriteAllBytes(tmp,JsonSerializer.SerializeToUtf8Bytes(state));
        File.Move(tmp,SelectorPath(root),true);
    }

    static (List<string> Files, List<string> Dirs) Inventory(string root)
    {
        var files = new List<string>();
        var dirs = new List<string>();
        void Walk(string parent,int depth)
        {
            if (depth > 12) throw new InvalidDataException("Directory depth exceeded");
            SafeAncestors(parent);
            foreach (var item in Directory.GetFileSystemEntries(parent))
            {
                SafeAncestors(item);
                var attr = File.GetAttributes(item);
                if ((attr & FileAttributes.ReparsePoint) != 0)
                    throw new InvalidDataException("Junction or symlink refused");
                if ((attr & FileAttributes.Directory) != 0) { dirs.Add(item); Walk(item,depth+1); }
                else files.Add(item);
                if (files.Count > MaxFiles*4 || dirs.Count > MaxFiles*4)
                    throw new InvalidDataException("Inventory bound exceeded");
            }
        }
        Walk(root,0);
        return (files,dirs);
    }
    static void CopyFile(string src,string dst)
    {
        SafeAncestors(src); SafeAncestors(dst);
        Directory.CreateDirectory(Path.GetDirectoryName(dst)!);
        using var source = new FileStream(src,FileMode.Open,FileAccess.Read,FileShare.Read);
        using var dest = new FileStream(dst,FileMode.CreateNew,FileAccess.Write,FileShare.None);
        source.CopyTo(dest);
    }
    static void Extract(string directory,Dictionary<string,byte[]> payload)
    {
        Directory.CreateDirectory(directory);
        foreach (var (name,bytes) in payload)
        {
            var file = Path.GetFullPath(Path.Combine(directory,name.Replace('/','\\')));
            if (!file.StartsWith(directory + "\\",StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("Extraction escaped release slot");
            SafeAncestors(file);
            Directory.CreateDirectory(Path.GetDirectoryName(file)!);
            using var outFile = new FileStream(file,FileMode.CreateNew,FileAccess.Write,FileShare.None);
            outFile.Write(bytes);
        }
    }
    static string SignedProof(string root,string slot) => Path.Combine(root,"slots",slot,"authority","issuer.exe");
    static Identity VerifyProof(string path,string publisher)
    {
        CheckSignature(path,publisher);
        // Signed *proof executable* carries immutable embedded release manifest,
        // source commit, and publisher binding. Never trust a writable D: JSON.
        var raw = RunChecked(path,"--identity");
        var proof = JsonSerializer.Deserialize<Identity>(raw)
            ?? throw new InvalidDataException("Signed installer identity absent");
        if (!Hex40.IsMatch(proof.Source) || !Fingerprint.IsMatch(proof.Publisher)
            || proof.Publisher != publisher || proof.ManifestSha.Length != 64
            || !proof.ManifestSha.All(Uri.IsHexDigit))
            throw new InvalidDataException("Issuer proof identity/source invalid");
        Semver(proof.Version);
        CheckSignature(path,publisher);
        return proof;
    }
    static Identity VerifySlot(string root,string name,string publisher)
    {
        if (!SlotName(name)) throw new InvalidDataException("Bad slot id");
        VerifyProtection(root);
        var slot = Path.Combine(root,"slots",name);
        SafeAncestors(slot);
        var proofPath = SignedProof(root,name);
        var proof = VerifyProof(proofPath,publisher);
        var manifest = Path.Combine(slot,"release.json");
        if (FileHash(manifest) != proof.ManifestSha)
            throw new InvalidDataException("Installed manifest not authorized by signed issuer");
        var indexed = Index(File.ReadAllBytes(manifest),proof.Source);
        if (indexed.Version != proof.Version)
            throw new InvalidDataException("Manifest version differs from issuer proof");
        var files = Inventory(slot).Files;
        if (files.Count != indexed.Files.Count + 2)
            throw new InvalidDataException("Unexpected installed slot contents");
        foreach (var (relative,info) in indexed.Files)
        {
            var file = Path.Combine(slot,relative.Replace('/','\\'));
            SafeAncestors(file);
            if (!File.Exists(file) || new FileInfo(file).Length != info.Size
                || !IC.Equals(FileHash(file),info.Hash))
                throw new InvalidDataException("Installed binary/manifest mismatch");
        }
        return proof;
    }

    static void CleanOwnedSlot(string root,string slot,string publisher)
    {
        // Reject unowned files *before* any deletion. Refuse mutation. The
        // signed authority is deleted last so interruption can be resumed.
        VerifySlot(root,slot,publisher);
        var folder = Path.Combine(root,"slots",slot);
        var manifest = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(folder,"release.json")));
        var index = Index(File.ReadAllBytes(Path.Combine(folder,"release.json")),
            manifest.RootElement.GetProperty("source_sha").GetString()!).Files;
        foreach (var file in index.Keys)
            File.Delete(Path.Combine(folder,file.Replace('/','\\')));
        File.Delete(Path.Combine(folder,"release.json"));
        File.Delete(SignedProof(root,slot));
        foreach (var directory in Inventory(folder).Dirs.OrderByDescending(s => s.Length))
            if (!Directory.EnumerateFileSystemEntries(directory).Any()) Directory.Delete(directory);
        if (Directory.EnumerateFileSystemEntries(folder).Any())
            throw new InvalidDataException("Unowned directories/files survive");
        Directory.Delete(folder);
    }
    static void Recover(string root,string publisher)
    {
        var state = ReadState(root,publisher);
        VerifySlot(root,state.Active,publisher);
        if (state.Previous is not null) VerifySlot(root,state.Previous,publisher);
        var slots = Path.Combine(root,"slots");
        foreach (var directory in Directory.GetDirectories(slots))
        {
            var name = Path.GetFileName(directory);
            if (!SlotName(name)) throw new InvalidDataException("Unexpected slot directory");
            if (name == state.Active || name == state.Previous) continue;
            // Orphan from before atomic publish; only delete if completely
            // verified by its own signed issuer proof. Never delete unowned.
            CleanOwnedSlot(root,name,publisher);
        }
    }
    static void InstallOrUpdate(string root,bool install,string publisher)
    {
        var payload = VerifyBundle(Archive(),Source());
        var manifest = payload["release.json"];
        var version = VersionOf(manifest);
        var signedCurrent = VerifyProof(Environment.ProcessPath!,publisher);
        if (signedCurrent.Source != Source() || signedCurrent.Version != version ||
            signedCurrent.ManifestSha != Hash(manifest))
            throw new InvalidDataException("Self signed release manifest/source mismatch");
        if (install && (Directory.Exists(root) || File.Exists(root)))
            throw new InvalidDataException("Target exists; never overwrite");
        if (!install && !Directory.Exists(root))
            throw new InvalidDataException("Installed target missing");
        // Updates are prepared in an isolated sibling directory. A crash during
        // copy never leaves a partial, untrusted slot under the active root.
        var stage = Path.Combine(Path.GetDirectoryName(root)!,
            "cosos-16pkg-stage-" + Guid.NewGuid().ToString("N"));
        var slot = "slot-" + Guid.NewGuid().ToString("N");
        var folder = Path.Combine(stage,"slots",slot);
        var publishedSlot = false;
        Selector? previous = null;
        if (!install)
        {
            Recover(root,publisher);
            previous = ReadState(root,publisher);
            var active = VerifySlot(root,previous.Active,publisher);
            if (Semver(version) <= Semver(previous.HighWater) || Semver(version) <= Semver(active.Version))
                throw new InvalidDataException("Downgrade/replay refused by high-water version");
            if (previous.Previous is not null)
                FinalizePrevious(root,publisher);
            previous = ReadState(root,publisher);
        }
        try
        {
            Directory.CreateDirectory(stage);
            Protect(stage);
            Extract(folder,payload);
            var proof = Path.Combine(folder,"authority","issuer.exe");
            CopyFile(Environment.ProcessPath!,proof);
            VerifySlot(stage,slot,publisher);
            SyntheticCrash("before_publish");
            var next = install ? new Selector(slot,null,publisher,version,1) :
                new Selector(slot,previous!.Active,publisher,version,previous.Generation + 1);
            if (install)
            {
                WriteState(stage,next);
                SafeAncestors(root);
                Directory.Move(stage,root);
                VerifyProtection(root);
            }
            else
            {
                VerifyProtection(root);
                Directory.Move(folder,Path.Combine(root,"slots",slot));
                publishedSlot = true;
                SyntheticCrash("after_slot_publish");
                VerifySlot(root,slot,publisher);
                WriteState(root,next);
                SyntheticCrash("after_selector_publish");
                if (Directory.Exists(Path.Combine(stage,"slots")))
                    Directory.Delete(Path.Combine(stage,"slots"));
                Directory.Delete(stage);
            }
        }
        catch
        {
            // In-flight stage remains disconnected from active slot selector.
            // Only delete fully signer-verified owned files; do not recursively
            // remove untrusted data. A crash may leave a sibling quarantine.
            if (Directory.Exists(stage))
            {
                if (Directory.Exists(folder))
                    try { CleanOwnedSlot(stage,slot,publisher); } catch { }
                var nested = Path.Combine(stage,"slots");
                if (Directory.Exists(nested) && !Directory.EnumerateFileSystemEntries(nested).Any())
                    Directory.Delete(nested);
                if (!Directory.EnumerateFileSystemEntries(stage).Any())
                    Directory.Delete(stage);
            }
            if (publishedSlot && Directory.Exists(Path.Combine(root,"slots",slot)))
                try { CleanOwnedSlot(root,slot,publisher); } catch { }
            throw;
        }
    }
    static void FinalizePrevious(string root,string publisher)
    {
        Recover(root,publisher);
        var state=ReadState(root,publisher);
        if (state.Previous is null) return;
        // Publish selector first, then clean known old slot. Orphan cleanup is
        // idempotent after an interruption.
        var other=state.Previous;
        VerifySlot(root,other,publisher);
        WriteState(root,state with { Previous = null, Generation = state.Generation + 1 });
        CleanOwnedSlot(root,other,publisher);
    }
    static void Rollback(string root,string publisher)
    {
        Recover(root,publisher);
        var state = ReadState(root,publisher);
        if (state.Previous is null) throw new InvalidDataException("No previous signed release");
        VerifySlot(root,state.Active,publisher);
        VerifySlot(root,state.Previous,publisher);
        WriteState(root,state with { Active = state.Previous, Previous = state.Active,
                                     Generation = state.Generation + 1 });
        SyntheticCrash("after_rollback_publish");
        // High-water remains the highest version even after rollback.
    }
    static void Uninstall(string root,string publisher)
    {
        Recover(root,publisher);
        var state=ReadState(root,publisher);
        var inv=Inventory(root);
        var expected=new HashSet<string>(IC) { SelectorPath(root) };
        foreach (var slot in new[] {state.Active,state.Previous})
        {
            if (slot is null) continue;
            var folder=Path.Combine(root,"slots",slot);
            foreach (var file in Inventory(folder).Files) expected.Add(file);
        }
        if (!expected.SetEquals(inv.Files))
            throw new InvalidDataException("Unowned installation files must survive");
        foreach (var slot in new[] {state.Previous,state.Active})
            if (slot is not null) CleanOwnedSlot(root,slot,publisher);
        File.Delete(SelectorPath(root));
        foreach(var dir in Inventory(root).Dirs.OrderByDescending(d=>d.Length))
            if (!Directory.EnumerateFileSystemEntries(dir).Any()) Directory.Delete(dir);
        if (Directory.EnumerateFileSystemEntries(root).Any())
            throw new InvalidDataException("Unowned data survived and was preserved");
        Directory.Delete(root);
    }
}
