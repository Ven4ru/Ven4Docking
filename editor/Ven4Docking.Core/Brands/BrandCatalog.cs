using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Brands;

/// <summary>
/// One instruction line. <see cref="Unconfirmed"/> marks items that docs/research/phone-brands.md lists as
/// «не подтверждено»: the UI adds «может называться иначе».
/// </summary>
public sealed class BrandStep
{
    public string Ru { get; set; } = "";
    public string? En { get; set; }
    public bool Unconfirmed { get; set; }

    public string Text(string language) => language == "en" && !string.IsNullOrEmpty(En) ? En! : Ru;
}

public sealed class BrandInfo
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";

    /// <summary>Lower-case substrings matched against ro.product.manufacturer / ro.product.brand.</summary>
    public List<string> Match { get; set; } = new();

    /// <summary>How to turn on developer options and USB debugging.</summary>
    public List<BrandStep> Developer { get; set; } = new();

    /// <summary>Extra switches that adb needs on this brand (besides USB debugging).</summary>
    public List<BrandStep> ExtraToggles { get; set; } = new();

    public List<BrandStep> Wifi { get; set; } = new();
    public List<BrandStep> KeepAlive { get; set; } = new();
    public List<BrandStep> Launcher { get; set; } = new();
    public List<BrandStep> ChargeLimit { get; set; } = new();

    /// <summary>Shown when settings/appops fail with SecurityException.</summary>
    public List<BrandStep> PermissionDenied { get; set; } = new();
}

public sealed class BrandCatalog
{
    public int Version { get; set; } = 1;
    public BrandInfo Generic { get; set; } = new();
    public List<BrandInfo> Brands { get; set; } = new();

    public static BrandCatalog Load(string path) =>
        JsonDefaults.Deserialize<BrandCatalog>(File.ReadAllText(path))
        ?? throw new InvalidDataException("brands.json is empty");

    /// <summary>The brand for this phone or <see cref="Generic"/> (stock Android rules).</summary>
    public BrandInfo Find(string? manufacturer, string? brand = null)
    {
        string m = (manufacturer ?? "").ToLowerInvariant();
        string b = (brand ?? "").ToLowerInvariant();
        foreach (var info in Brands)
        {
            if (info.Match.Any(k => (m.Length > 0 && m.Contains(k)) || (b.Length > 0 && b.Contains(k))))
                return info;
        }
        return Generic;
    }

    /// <summary>A brand's own steps for a section, or the generic ones when the brand has none.</summary>
    public IReadOnlyList<BrandStep> StepsOrGeneric(BrandInfo brand, Func<BrandInfo, List<BrandStep>> section)
    {
        var own = section(brand);
        return own.Count > 0 ? own : section(Generic);
    }
}
