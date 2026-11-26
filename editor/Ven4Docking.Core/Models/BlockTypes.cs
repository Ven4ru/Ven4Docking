using System.Text.Json.Nodes;

namespace Ven4Docking.Core.Models;

/// <summary>A block type (spec §5, spec 1.1 §4) with its default size and options.</summary>
public sealed record BlockType(string Id, string Glyph, int DefaultW, int DefaultH, string DefaultOptsJson)
{
    public JsonObject CreateDefaultOpts() => (JsonObject)JsonNode.Parse(DefaultOptsJson)!;
}

public static class BlockTypes
{
    public const string Clock = "clock";
    public const string Weather = "weather";
    public const string Cpu = "cpu";
    public const string Gpu = "gpu";
    public const string Ram = "ram";
    public const string Disks = "disks";
    public const string Net = "net";
    public const string Ai = "ai";
    public const string Apps = "apps";
    public const string Media = "media";
    public const string Servers = "servers";
    public const string Phone = "phone";

    // 1.1
    public const string Power = "power";
    public const string Keys = "keys";
    public const string Mixer = "mixer";
    public const string Obs = "obs";
    public const string Notify = "notify";

    public static readonly IReadOnlyList<string> PowerButtons = new[] { "lock", "sleep", "restart", "shutdown" };
    public static readonly IReadOnlyList<string> MixerParts = new[] { "output", "mic", "apps" };
    public static readonly IReadOnlyList<string> ObsParts = new[] { "stream", "record", "scenes", "audio", "stats" };
    public const int NotifyDefaultMax = 5;
    public const int NotifyMaxLimit = 20;

    // Glyphs: Segoe Fluent Icons / Segoe MDL2 Assets code points.
    public static readonly IReadOnlyList<BlockType> All = new[]
    {
        new BlockType(Clock,   "", 3, 2, """{"weather":true,"h24":true}"""),
        new BlockType(Weather, "", 2, 2, "{}"),
        new BlockType(Cpu,     "", 2, 2, """{"cores":true}"""),
        new BlockType(Gpu,     "", 2, 2, """{"gpu":0}"""),
        new BlockType(Ram,     "", 2, 1, "{}"),
        new BlockType(Disks,   "", 2, 1, """{"disks":[]}"""),
        new BlockType(Net,     "", 2, 2, "{}"),
        new BlockType(Ai,      "", 3, 2, """{"sources":[]}"""),
        new BlockType(Apps,    "", 4, 1, """{"apps":[]}"""),
        new BlockType(Media,   "", 3, 1, """{"buttons":["prev","playpause","next"]}"""),
        new BlockType(Servers, "", 4, 2, """{"servers":[]}"""),
        new BlockType(Phone,   "", 2, 1, """{"hotTemp":45}"""),
        new BlockType(Power,   "", 4, 1, """{"buttons":["lock","sleep","restart","shutdown"]}"""),
        new BlockType(Keys,    "", 4, 1, """{"actions":[]}"""),
        new BlockType(Mixer,   "", 3, 2, """{"output":true,"mic":true,"apps":true}"""),
        new BlockType(Obs,     "", 4, 2, """{"stream":true,"record":true,"scenes":true,"audio":true,"stats":true}"""),
        new BlockType(Notify,  "", 3, 2, """{"max":5}"""),
    };

    public static BlockType? Find(string id) => All.FirstOrDefault(t => t.Id == id);
}
