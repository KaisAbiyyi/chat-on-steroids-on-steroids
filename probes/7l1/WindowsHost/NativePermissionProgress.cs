namespace CoSoS.Probe7L1.WindowsHost;

internal static class NativePermissionProgress
{
    // Entering PermissionRequested increments Events before a modal pumps
    // native callbacks. Only its finally block settles the owner's decision.
    internal static bool Settled(int before, int events, int completed, int busy) =>
        before >= 0 && events > before && completed == events && busy == 0;
}
