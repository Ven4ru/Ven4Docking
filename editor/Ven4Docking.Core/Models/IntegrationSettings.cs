using System.Net;
using System.Net.Sockets;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;

namespace Ven4Docking.Core.Models;

/// <summary>
/// <c>config.obs</c> (spec 1.1 §2, §3): obs-websocket address. The password is never stored in the clear:
/// <see cref="PasswordDpapi"/> is DPAPI (current user) ciphertext in base64, see <see cref="Security.Dpapi"/>.
/// </summary>
public sealed class ObsSettings
{
    public const string DefaultHost = "127.0.0.1";
    public const int DefaultPort = 4455;

    public string Host { get; set; } = DefaultHost;
    public int Port { get; set; } = DefaultPort;
    public string? PasswordDpapi { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    /// <summary>
    /// obs-websocket is plain ws://, so the agent connects only to this PC or the private network: «localhost» or an IP
    /// of 127/8, 10/8, 172.16/12, 192.168/16, 100.64/10, ::1 (the same rule the agent applies to config.json).
    /// </summary>
    public static bool IsAllowedHost(string? host)
    {
        string h = (host ?? "").Trim().TrimStart('[').TrimEnd(']');
        if (h.Equals("localhost", StringComparison.OrdinalIgnoreCase)) return true;
        if (!IPAddress.TryParse(h, out var ip) || (ip.AddressFamily == AddressFamily.InterNetwork && h.Count(c => c == '.') != 3))
            return false;
        if (IPAddress.IsLoopback(ip)) return true;
        if (ip.IsIPv4MappedToIPv6) ip = ip.MapToIPv4();
        if (ip.AddressFamily != AddressFamily.InterNetwork) return false;
        byte[] o = ip.GetAddressBytes();
        return o[0] == 10 || o[0] == 127
            || (o[0] == 172 && o[1] is >= 16 and <= 31)
            || (o[0] == 192 && o[1] == 168)
            || (o[0] == 100 && o[1] is >= 64 and <= 127);
    }
}

/// <summary>
/// <c>config.notify</c> (spec 1.1 §3). <see cref="Apps"/>: AppUserModelId → count | title | full;
/// an app that is not listed is not shown on the panel.
/// </summary>
public sealed class NotifySettings
{
    public const string ModeHidden = "hidden";
    public const string ModeCount = "count";
    public const string ModeTitle = "title";
    public const string ModeFull = "full";

    /// <summary>UI order; <see cref="ModeHidden"/> is not a stored value, it means «no entry».</summary>
    public static readonly IReadOnlyList<string> Modes = new[] { ModeHidden, ModeCount, ModeTitle, ModeFull };

    public bool Enabled { get; set; }
    public bool WallCountsOnly { get; set; }
    public bool NightFlash { get; set; } = true;
    public Dictionary<string, string> Apps { get; set; } = new(StringComparer.Ordinal);

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    public string GetMode(string aumid) =>
        Apps.TryGetValue(aumid, out var mode) && mode is ModeCount or ModeTitle or ModeFull ? mode : ModeHidden;

    public void SetMode(string aumid, string mode)
    {
        if (mode is ModeCount or ModeTitle or ModeFull) Apps[aumid] = mode;
        else Apps.Remove(aumid);
    }
}

/// <summary>
/// <c>config.wol</c> (spec 1.1 §3): MAC and broadcast address sent to the phone for «Включить ПК».
/// Absent → the agent takes them from the adapter with the default route.
/// </summary>
public sealed partial class WolSettings
{
    public string Mac { get; set; } = "";
    public string Broadcast { get; set; } = "";

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    [GeneratedRegex(@"^[0-9A-Fa-f]{2}([:-]?)[0-9A-Fa-f]{2}(\1[0-9A-Fa-f]{2}){4}$")]
    private static partial Regex MacPattern();

    /// <summary>«aa-bb-cc-dd-ee-ff», «AABBCCDDEEFF» → «AA:BB:CC:DD:EE:FF»; null when it is not a MAC address.</summary>
    public static string? NormalizeMac(string? text)
    {
        string t = (text ?? "").Trim();
        if (!MacPattern().IsMatch(t)) return null;
        string hex = t.Replace(":", "").Replace("-", "").ToUpperInvariant();
        if (hex is "000000000000" or "FFFFFFFFFFFF") return null;
        return string.Join(':', Enumerable.Range(0, 6).Select(i => hex.Substring(i * 2, 2)));
    }

    /// <summary>An IPv4 address in dotted form (the broadcast of the LAN, or 255.255.255.255).</summary>
    public static string? NormalizeBroadcast(string? text)
    {
        string t = (text ?? "").Trim();
        if (t.Count(c => c == '.') != 3 || !IPAddress.TryParse(t, out var ip) || ip.AddressFamily != AddressFamily.InterNetwork)
            return null;
        return ip.ToString();
    }

    /// <summary>Both fields valid → normalized copy for config.json; otherwise null.</summary>
    public static WolSettings? Create(string? mac, string? broadcast)
    {
        string? m = NormalizeMac(mac), b = NormalizeBroadcast(broadcast);
        return m is null || b is null ? null : new WolSettings { Mac = m, Broadcast = b };
    }
}
