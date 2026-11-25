using System.Text.Json.Nodes;

namespace Ven4Docking.Core.Models;

/// <summary>First-run templates (spec §5): «Классика», «Монитор ПК», «Только приложения».</summary>
public static class LayoutTemplates
{
    public const string Classic = "classic";
    public const string PcMonitor = "pc-monitor";
    public const string AppsOnly = "apps-only";

    public static readonly IReadOnlyList<string> All = new[] { Classic, PcMonitor, AppsOnly };

    public static PanelLayout Create(string template, string language = "ru")
    {
        bool ru = language != "en";
        int n = 0;
        LayoutBlock B(string type, int x, int y, int w, int h)
        {
            var bt = BlockTypes.Find(type)!;
            return new LayoutBlock { Id = "b" + (++n), Type = type, X = x, Y = y, W = w, H = h, Opts = bt.CreateDefaultOpts() };
        }

        var layout = new PanelLayout { Version = 0 };
        switch (template)
        {
            case PcMonitor:
                layout.Screens.Add(new LayoutScreen
                {
                    Id = "s1", Name = ru ? "Монитор ПК" : "PC monitor",
                    Blocks = { B(BlockTypes.Cpu, 0, 0, 4, 2), B(BlockTypes.Gpu, 4, 0, 4, 2),
                               B(BlockTypes.Ram, 0, 2, 2, 2), B(BlockTypes.Disks, 2, 2, 2, 2), B(BlockTypes.Net, 4, 2, 4, 2) },
                });
                break;
            case AppsOnly:
                layout.Screens.Add(new LayoutScreen
                {
                    Id = "s1", Name = ru ? "Приложения" : "Apps",
                    Blocks = { B(BlockTypes.Apps, 0, 0, 8, 4) },
                });
                break;
            default:
                layout.Screens.Add(new LayoutScreen
                {
                    Id = "s1", Name = ru ? "Главный" : "Main",
                    Blocks = { B(BlockTypes.Clock, 0, 0, 3, 2), B(BlockTypes.Cpu, 3, 0, 2, 2), B(BlockTypes.Gpu, 5, 0, 3, 2),
                               B(BlockTypes.Ram, 0, 2, 2, 1), B(BlockTypes.Disks, 2, 2, 2, 1), B(BlockTypes.Net, 4, 2, 4, 1),
                               B(BlockTypes.Apps, 0, 3, 5, 1), B(BlockTypes.Media, 5, 3, 3, 1) },
                });
                layout.Screens.Add(new LayoutScreen
                {
                    Id = "s2", Name = ru ? "Серверы" : "Servers",
                    Blocks = { B(BlockTypes.Servers, 0, 0, 8, 3), B(BlockTypes.Ai, 0, 3, 6, 1), B(BlockTypes.Phone, 6, 3, 2, 1) },
                });
                break;
        }
        return layout;
    }

    internal static JsonObject Opts(string json) => (JsonObject)JsonNode.Parse(json)!;
}
