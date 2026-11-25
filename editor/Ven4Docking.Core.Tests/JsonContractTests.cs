using System.Text.Json.Nodes;
using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Tests;

public class JsonContractTests
{
    private static void AssertSameJson(string expected, string actual)
    {
        var e = JsonNode.Parse(expected);
        var a = JsonNode.Parse(actual);
        Assert.True(JsonNode.DeepEquals(e, a), $"JSON differs.\nExpected: {e?.ToJsonString()}\nActual:   {a?.ToJsonString()}");
    }

    [Fact]
    public void Spec_layout_round_trips_unchanged() =>
        AssertSameJson(Samples.SpecLayout, JsonDefaults.Serialize(JsonDefaults.Deserialize<PanelLayout>(Samples.SpecLayout)));

    [Fact]
    public void Api_layout_round_trips_including_fields_the_editor_does_not_model() =>
        AssertSameJson(Samples.ApiLayout, JsonDefaults.Serialize(JsonDefaults.Deserialize<PanelLayout>(Samples.ApiLayout)));

    [Fact]
    public void Spec_config_round_trips_unchanged() =>
        AssertSameJson(Samples.SpecConfig, JsonDefaults.Serialize(JsonDefaults.Deserialize<DockConfig>(Samples.SpecConfig)));

    [Fact]
    public void Layout_is_parsed_into_typed_fields()
    {
        var l = JsonDefaults.Deserialize<PanelLayout>(Samples.SpecLayout)!;
        Assert.Equal(7, l.Version);
        Assert.Equal((8, 4), (l.Grid.Cols, l.Grid.Rows));
        var b2 = l.Screens[0].Blocks[1];
        Assert.Equal(("apps", 0, 3, 5, 1), (b2.Type, b2.X, b2.Y, b2.W, b2.H));
        Assert.Equal("chrome", b2.Opts["apps"]![1]!.GetValue<string>());
    }

    [Fact]
    public void New_objects_use_contract_names_and_omit_nulls()
    {
        var cfg = new DockConfig
        {
            Theme = "teal",
            Weather = new WeatherPlace { Name = "Москва", Lat = 55.75, Lon = 37.62 },
            Phones = { new PhoneEntry { Id = "p1", Name = "realme C11", DimWhenPcOffline = true } },
        };
        var node = JsonNode.Parse(JsonDefaults.Serialize(cfg))!.AsObject();
        Assert.Equal(1, node["version"]!.GetValue<int>());
        Assert.False(node.ContainsKey("port"));
        Assert.False(node.ContainsKey("language"));
        Assert.Equal("Москва", node["weather"]!["name"]!.GetValue<string>());
        var phone = node["phones"]![0]!.AsObject();
        Assert.True(phone["dimWhenPcOffline"]!.GetValue<bool>());
        Assert.False(phone.ContainsKey("tokenHash"));
        Assert.Contains("\"Москва\"", JsonDefaults.Serialize(cfg)); // Cyrillic is not \u-escaped

        var layout = LayoutTemplates.Create(LayoutTemplates.Classic);
        var ln = JsonNode.Parse(JsonDefaults.Serialize(layout))!.AsObject();
        Assert.Equal(new[] { "version", "orientation", "grid", "screens" }, ln.Select(p => p.Key));
        Assert.Equal(new[] { "id", "type", "x", "y", "w", "h", "opts" },
            ln["screens"]![0]!["blocks"]![0]!.AsObject().Select(p => p.Key));
    }

    [Fact]
    public void Effective_port_defaults_to_8765()
    {
        Assert.Equal(8765, new DockConfig().EffectivePort);
        Assert.Equal(18765, new DockConfig { Port = 18765 }.EffectivePort);
        Assert.Equal(8765, new DockConfig { Port = 70000 }.EffectivePort);
    }

    [Fact]
    public void Clone_is_deep()
    {
        var l = LayoutTemplates.Create(LayoutTemplates.Classic);
        var c = l.Clone();
        c.Screens[0].Blocks[0].X = 5;
        c.Screens[0].Blocks[0].Opts["weather"] = false;
        Assert.Equal(0, l.Screens[0].Blocks[0].X);
        Assert.True(l.Screens[0].Blocks[0].Opts["weather"]!.GetValue<bool>());
    }
}
