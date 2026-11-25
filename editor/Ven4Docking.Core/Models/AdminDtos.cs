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

/// <summary>Result of PUT /admin/layouts or a local file save.</summary>
public sealed record SaveResult(bool Ok, string? Error = null, IReadOnlyList<string>? Blocks = null, int? NewVersion = null)
{
    public static SaveResult Success(int? version = null) => new(true, NewVersion: version);
}
