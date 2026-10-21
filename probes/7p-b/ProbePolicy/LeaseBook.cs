namespace CoSoS.Probe7PB;

/// <summary>
/// Synthetic target authorization only. Never derived from untrusted page messages.
/// An issued lease becomes invalid after close, re-election or epoch change.
/// </summary>
public sealed record TabLease(string Profile, string Tab, long Epoch);

public sealed class LeaseBook
{
    private readonly Dictionary<(string Profile, string Tab), TabLease> _active = new();
    private long _nextEpoch;

    public TabLease Elect(string profile, string tab)
    {
        RequireIdentifier(profile);
        RequireIdentifier(tab);
        var lease = new TabLease(profile, tab, checked(++_nextEpoch));
        _active[(profile, tab)] = lease;
        return lease;
    }

    public bool Allows(TabLease? lease, string profile, string tab)
        => lease is not null && lease.Profile == profile && lease.Tab == tab
           && _active.TryGetValue((profile, tab), out var current)
           && current == lease;

    public bool Close(TabLease? lease)
    {
        if (lease is null || !Allows(lease, lease.Profile, lease.Tab)) return false;
        return _active.Remove((lease.Profile, lease.Tab));
    }

    private static void RequireIdentifier(string value)
    {
        if (string.IsNullOrWhiteSpace(value) || value.Length > 100)
            throw new ArgumentException("Invalid synthetic profile or tab identifier");
    }
}

public static class ProbeOutput
{
    /// <summary>Never writes to C: if a D: volume is present.</summary>
    public static string Resolve(string? requested)
    {
        var driveD = OperatingSystem.IsWindows() && Directory.Exists(@"D:\");
        var folder = Path.GetFullPath(requested ??
            (driveD ? @"D:\CoSoSProbe7PB" : Path.Combine(Path.GetTempPath(), "cos-7p-b")));
        if (driveD && !folder.StartsWith(@"D:\", StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("D: is available; output and profiles must stay on D:");
        if (Path.GetPathRoot(folder) == Path.GetPathRoot(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile))
            && requested is null && driveD)
            throw new InvalidOperationException("Profile root must not be the user's profile.");
        return folder;
    }
}
