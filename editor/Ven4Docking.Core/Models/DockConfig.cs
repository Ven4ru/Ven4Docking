using System.Text.Json;
using System.Text.Json.Serialization;

namespace Ven4Docking.Core.Models;

/// <summary>config.json (spec §4). Unknown fields are kept in <see cref="Extra"/> so the editor never drops what the agent wrote.</summary>
public sealed class DockConfig
{
    public const int DefaultPort = 8765;

    public int Version { get; set; } = 1;
    public int? Port { get; set; }
    public string? Theme { get; set; }
    public string? Language { get; set; }
    public WeatherPlace? Weather { get; set; }
    public List<AppEntry> Apps { get; set; } = new();
    public List<ServerEntry> Servers { get; set; } = new();
    public AiSettings? Ai { get; set; }
    public List<PhoneEntry> Phones { get; set; } = new();

    // 1.1 (spec 1.1 §3). Null = the field is absent and stays absent on save.
    public List<ActionEntry>? Actions { get; set; }
    public ObsSettings? Obs { get; set; }
    public NotifySettings? Notify { get; set; }
    public WolSettings? Wol { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    [JsonIgnore]
    public int EffectivePort => Port is > 0 and < 65536 ? Port.Value : DefaultPort;
}

public sealed class WeatherPlace
{
    public string Name { get; set; } = "";
    public double Lat { get; set; }
    public double Lon { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>An entry of the launch list. <see cref="Kind"/>: shortcut | exe | url | command.</summary>
public sealed class AppEntry
{
    public const string KindShortcut = "shortcut";
    public const string KindExe = "exe";
    public const string KindUrl = "url";
    public const string KindCommand = "command";

    public string Id { get; set; } = "";
    public string Title { get; set; } = "";
    public string Kind { get; set; } = KindExe;
    public string Target { get; set; } = "";
    public string? Args { get; set; }
    public string? Icon { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class ServerEntry
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Host { get; set; } = "";
    public int Port { get; set; } = 22;
    public string User { get; set; } = "root";
    public string Key { get; set; } = "auto";

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class AiSettings
{
    public static readonly IReadOnlyList<string> KnownSources =
        new[] { "claude-code", "claude-desktop", "codex", "ollama", "lmstudio" };

    public List<string> Enabled { get; set; } = new();

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class PhoneEntry
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string? TokenHash { get; set; }
    public NightSettings? Night { get; set; }
    public bool? DimWhenPcOffline { get; set; }
    public bool? BurnInShift { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class NightSettings
{
    public string From { get; set; } = "23:00";
    public string To { get; set; } = "07:00";
    public double Brightness { get; set; } = 0.02;

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}
