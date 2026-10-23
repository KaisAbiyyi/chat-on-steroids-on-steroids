// 7P-L1 synthetic-only, offline/unsigned independent setup EXE. NOT product CoSoS.
// Compiles with stock Windows .NET Framework C# compiler (v4), no third-party MSI tools.
using System;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;
using System.Collections.Generic;

internal static class SyntheticProbeInstaller
{
    private const string PayloadResource = "CoSoS7PL1.Payload";
    private const string HashResource = "CoSoS7PL1.PayloadHash";

    private static string Hex(byte[] bytes)
    {
        return BitConverter.ToString(bytes).Replace("-", "");
    }

    // Reject existing junction/symlink ancestors before writing to an alleged
    // D: destination. Pure lexical GetFullPath checks do not enforce volume
    // containment when a pre-existing reparse point points outside the drive.
    private static void RequireSafeDDriveAncestors(string path)
    {
        string full = Path.GetFullPath(path);
        if (!string.Equals(Path.GetPathRoot(full), @"D:\", StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("Install destination MUST be on D: drive.");
        string cursor = @"D:\";
        foreach (string component in full.Substring(cursor.Length).Split(
            new char[] { Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar },
            StringSplitOptions.RemoveEmptyEntries))
        {
            cursor = Path.Combine(cursor, component);
            if (Directory.Exists(cursor) || File.Exists(cursor))
            {
                if ((File.GetAttributes(cursor) & FileAttributes.ReparsePoint) != 0)
                    throw new InvalidOperationException("Existing reparse-point ancestor forbidden: " + cursor);
            }
        }
    }

    private static int Main(string[] args)
    {
        try
        {
            string destination = Environment.GetEnvironmentVariable("COS_7PL1_INSTALL_ROOT");
            if (string.IsNullOrWhiteSpace(destination)) destination = @"D:\Apps\CoSoS-7PL1-Synthetic-Probe";
            if (args.Length == 2 && args[0] == "--install") destination = args[1];
            else if (args.Length > 0 && !(args.Length == 1 && args[0] == "--verify"))
                throw new ArgumentException("Usage: setup.exe [--verify | --install D:\\path]");
            destination = Path.GetFullPath(destination).TrimEnd(Path.DirectorySeparatorChar);
            if (!string.Equals(Path.GetPathRoot(destination), @"D:\", StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("Install destination MUST be on D: drive.");
            if (!Directory.Exists(@"D:\")) throw new InvalidOperationException("D: volume not mounted.");
            RequireSafeDDriveAncestors(destination);

            Assembly self = Assembly.GetExecutingAssembly();
            using (Stream payload = self.GetManifestResourceStream(PayloadResource))
            using (Stream expectedHashStream = self.GetManifestResourceStream(HashResource))
            {
                if (payload == null || expectedHashStream == null) throw new InvalidDataException("Missing installer resources.");
                string expected;
                using (StreamReader reader = new StreamReader(expectedHashStream))
                    expected = reader.ReadToEnd().Trim().Split(' ')[0];
                string actual;
                using (SHA256 sha = SHA256.Create()) actual = Hex(sha.ComputeHash(payload));
                if (!string.Equals(expected, actual, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("Embedded archive SHA256 differs from signed-offline build checksum.");
                Console.WriteLine("PAYLOAD_SHA256=" + actual);
                if (args.Length == 1 && args[0] == "--verify") return 0;
                if (Directory.Exists(destination) || File.Exists(destination))
                    throw new IOException("Destination exists: " + destination + ". Refuse overwrite.");

                string parent = Path.GetDirectoryName(destination);
                Directory.CreateDirectory(parent);
                RequireSafeDDriveAncestors(parent);
                string staging = Path.Combine(parent, "7PL1-Stage-" + Guid.NewGuid().ToString("N"));
                Directory.CreateDirectory(staging);
                bool installed = false;
                try
                {
                    payload.Position = 0;
                    using (ZipArchive zip = new ZipArchive(payload, ZipArchiveMode.Read, true))
                    {
                        foreach (ZipArchiveEntry entry in zip.Entries)
                        {
                            string target = Path.GetFullPath(Path.Combine(staging,
                                entry.FullName.Replace('/', Path.DirectorySeparatorChar)));
                            if (!target.StartsWith(staging + Path.DirectorySeparatorChar,
                                    StringComparison.OrdinalIgnoreCase))
                                throw new InvalidDataException("Blocked ZIP path traversal: " + entry.FullName);
                            // PowerShell Compress-Archive on Windows may represent
                            // directory entries using backslashes, not ZIP slashes.
                            if (entry.FullName.EndsWith("/") || entry.FullName.EndsWith("\\")
                                || string.IsNullOrEmpty(entry.Name))
                            {
                                Directory.CreateDirectory(target);
                                continue;
                            }
                            Directory.CreateDirectory(Path.GetDirectoryName(target));
                            using (Stream input = entry.Open())
                            using (FileStream output = File.Create(target)) input.CopyTo(output);
                        }
                    }

                    string manifest = Path.Combine(staging, "probe-files.sha256");
                    if (!File.Exists(manifest)) throw new InvalidDataException("Missing ownership manifest.");
                    string[] lines = File.ReadAllLines(manifest);
                    if (lines.Length < 4) throw new InvalidDataException("Incomplete ownership manifest.");
                    int count = 0;
                    foreach (string line in lines)
                    {
                        if (string.IsNullOrWhiteSpace(line)) continue;
                        int separator = line.IndexOf(' ');
                        if (separator != 64) throw new InvalidDataException("Malformed SHA256 manifest entry.");
                        string relative = line.Substring(separator + 1).Trim().Replace('/', Path.DirectorySeparatorChar);
                        string full = Path.GetFullPath(Path.Combine(staging, relative));
                        RequireSafeDDriveAncestors(full);
                        if (!full.StartsWith(staging + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)
                            || !File.Exists(full)) throw new InvalidDataException("Missing or escaping manifest file: " + relative);
                        using (SHA256 sha = SHA256.Create())
                        using (FileStream input = File.OpenRead(full))
                            if (!string.Equals(line.Substring(0, 64), Hex(sha.ComputeHash(input)), StringComparison.OrdinalIgnoreCase))
                                throw new InvalidDataException("Payload file SHA256 mismatch: " + relative);
                        count++;
                    }
                    RequireSafeDDriveAncestors(destination);
                    Directory.Move(staging, destination);
                    installed = true;
                    Console.WriteLine("INSTALLED=" + destination + " VERIFIED_FILES=" + count);
                    Console.WriteLine("This is a synthetic-only Windows probe; NOT the CoSoS production app.");
                    return 0;
                }
                finally
                {
                    if (!installed && Directory.Exists(staging)) Directory.Delete(staging, true);
                }
            }
        }
        catch (Exception error)
        {
            Console.Error.WriteLine("SETUP_FAILED: " + error.GetType().Name + ": " + error.Message);
            return 2;
        }
    }
}
