namespace Ven4Docking.Core.Net;

/// <summary>
/// Turns the agent's server-test error text into a category the editor can explain in plain words
/// (spec §9: timeout, port closed, key not accepted, host key mismatch).
/// </summary>
public static class ServerErrors
{
    public const string Timeout = "timeout";
    public const string PortClosed = "portClosed";
    public const string KeyRejected = "keyRejected";
    public const string HostKeyMismatch = "hostKeyMismatch";
    public const string HostKeyUnknown = "hostKeyUnknown";
    public const string NoKey = "noKey";
    public const string Dns = "dns";
    public const string Other = "other";

    public static string Classify(string? err)
    {
        string e = (err ?? "").ToLowerInvariant();
        if (e.Length == 0) return Other;
        if (e.Contains("mismatch") || e.Contains("changed") || e.Contains("не совпал")) return HostKeyMismatch;
        if (e.Contains("unknown host") || e.Contains("known_hosts") || e.Contains("hostkey") || e.Contains("host key")) return HostKeyUnknown;
        if (e.Contains("timeout") || e.Contains("timed out") || e.Contains("таймаут")) return Timeout;
        if (e.Contains("refused") || e.Contains("closed") || e.Contains("unreachable")) return PortClosed;
        if (e.Contains("no key") || e.Contains("id_ed25519") || e.Contains("id_rsa")) return NoKey;
        if (e.Contains("auth") || e.Contains("permission denied") || e.Contains("publickey")) return KeyRejected;
        if (e.Contains("resolve") || e.Contains("dns") || e.Contains("no such host")) return Dns;
        return Other;
    }
}
