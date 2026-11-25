using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Ven4Docking.Core.Models;

/// <summary>layouts/&lt;phone&gt;.json (spec §4, plan «Контракты»).</summary>
public sealed class PanelLayout
{
    public const int MinScreens = 1;
    public const int MaxScreens = 10;

    public int Version { get; set; }
    public string Orientation { get; set; } = "landscape";
    public GridSize Grid { get; set; } = new();
    public List<LayoutScreen> Screens { get; set; } = new();

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    public PanelLayout Clone() =>
        JsonDefaults.Deserialize<PanelLayout>(JsonDefaults.Serialize(this, indented: false))!;

    public IEnumerable<LayoutBlock> AllBlocks() => Screens.SelectMany(s => s.Blocks);
}

public sealed class GridSize
{
    public int Cols { get; set; } = 8;
    public int Rows { get; set; } = 4;

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class LayoutScreen
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public List<LayoutBlock> Blocks { get; set; } = new();

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class LayoutBlock
{
    public string Id { get; set; } = "";
    public string Type { get; set; } = "";
    public int X { get; set; }
    public int Y { get; set; }
    public int W { get; set; } = 1;
    public int H { get; set; } = 1;
    public JsonObject Opts { get; set; } = new();

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    /// <summary>Right edge (exclusive), in cells.</summary>
    [JsonIgnore] public int Right => X + W;

    /// <summary>Bottom edge (exclusive), in cells.</summary>
    [JsonIgnore] public int Bottom => Y + H;
}
