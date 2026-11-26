using System.Text.Json;
using System.Text.Json.Serialization;

namespace Ven4Docking.Core.Models;

/// <summary>GET /admin/phones item.</summary>
public sealed class PhoneInfo
{
    public string Id { get; set; } = "";
    public string? Name { get; set; }
    public string? Model { get; set; }
    public string? Android { get; set; }
    public string? Webview { get; set; }

    /// <summary>versionName of the launcher on the phone; absent for launchers that did not report it.</summary>
    public string? LauncherVersion { get; set; }

    public double? Battery { get; set; }
    public double? Temp { get; set; }
    public double? LastSeen { get; set; }
    public bool Online { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>GET /admin/pairs item.</summary>
public sealed class PairRequest
{
    public string PairId { get; set; } = "";
    public string? Model { get; set; }
    public string? Android { get; set; }
    public string Code { get; set; } = "";
    public string? Ip { get; set; }
}

/// <summary>POST /admin/phones/token response.</summary>
public sealed class PhoneTokenResult
{
    public string Id { get; set; } = "";
    public string Token { get; set; } = "";
}

/// <summary>POST /admin/servers/test response.</summary>
public sealed class ServerTestResult
{
    public bool Ok { get; set; }
    public double? Ping { get; set; }
    public string? Err { get; set; }
}

/// <summary>GET /admin/ai/sources item.</summary>
public sealed class AiSourceInfo
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public bool Installed { get; set; }
    public double? LastData { get; set; }
}

/// <summary>GET /admin/status.</summary>
public sealed class AgentStatus
{
    public string Version { get; set; } = "";
    public int Port { get; set; }
    public double? CpuPct { get; set; }
    public long? RamBytes { get; set; }
    public int ActivePanels { get; set; }
    public Dictionary<string, JsonElement>? BlockErrors { get; set; }

    /// <summary>
    /// "ok" | "missing" | "blocked" | "unknown" — the inbound firewall rule of the agent. Absent in older agents.
    /// </summary>
    public string? Firewall { get; set; }

    [JsonIgnore]
    public FirewallState FirewallState => FirewallStates.Parse(Firewall);
}

/// <summary>Firewall rule state reported by the agent in <c>GET /admin/status</c>.</summary>
public enum FirewallState
{
    /// <summary>The agent did not say (older agent, unknown value, agent offline): show nothing.</summary>
    Unknown,
    Ok,
    /// <summary>No inbound rule: the phone cannot reach the PC over Wi-Fi.</summary>
    Missing,
    /// <summary>Windows created a Block rule for the agent program (the user pressed «Cancel» in its prompt).</summary>
    Blocked,
}

public static class FirewallStates
{
    public static FirewallState Parse(string? value) => value?.Trim().ToLowerInvariant() switch
    {
        "ok" => FirewallState.Ok,
        "missing" => FirewallState.Missing,
        "blocked" => FirewallState.Blocked,
        _ => FirewallState.Unknown,
    };

    /// <summary>The editor warns and offers «Исправить» only for these states.</summary>
    public static bool NeedsFix(FirewallState state) => state is FirewallState.Missing or FirewallState.Blocked;
}

// ---------- 1.1 (spec 1.1 §5, «Редактор») ----------

/// <summary>GET /admin/notify/apps: apps the agent saw notifications from since it started (no text).</summary>
public sealed class NotifyAppsResponse
{
    /// <summary>allowed | denied | unspecified | off — the Windows notification access of the agent.</summary>
    public string? Access { get; set; }
    public List<NotifyAppInfo> Apps { get; set; } = new();
}

public sealed class NotifyAppInfo
{
    public string Aumid { get; set; } = "";
    public string? Name { get; set; }
    public double? LastSeen { get; set; }
}

public static class NotifyAccess
{
    public const string Allowed = "allowed";
    public const string Denied = "denied";
    public const string Unspecified = "unspecified";
    public const string Off = "off";
}

/// <summary>GET /admin/audio/devices.</summary>
public sealed class AudioDevicesResponse
{
    public List<AudioDeviceInfo> Outputs { get; set; } = new();
    public List<AudioDeviceInfo> Inputs { get; set; } = new();
}

public sealed class AudioDeviceInfo
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public bool Default { get; set; }
}

/// <summary>POST /admin/obs/test response.</summary>
public sealed class ObsTestResult
{
    public bool Ok { get; set; }
    public string? Version { get; set; }
    public string? Error { get; set; }
}

/// <summary>GET /admin/processes item: a program with a window (for «Экран программы»).</summary>
public sealed class ProcessInfo
{
    public string Name { get; set; } = "";
    public string? Title { get; set; }
}

/// <summary>GET /admin/wol: what the agent found on the adapter with the default route.</summary>
public sealed class WolInfo
{
    public string? Mac { get; set; }
    public string? Broadcast { get; set; }
    public string? Adapter { get; set; }

    /// <summary>Wake on LAN allowed in Windows for the adapter; null = the agent could not tell.</summary>
    public bool? WakeEnabled { get; set; }
}

/// <summary>Result of PUT /admin/layouts or a local file save.</summary>
public sealed record SaveResult(bool Ok, string? Error = null, IReadOnlyList<string>? Blocks = null, int? NewVersion = null)
{
    public static SaveResult Success(int? version = null) => new(true, NewVersion: version);
}
