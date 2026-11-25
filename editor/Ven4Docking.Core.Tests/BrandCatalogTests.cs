using Ven4Docking.Core.Brands;

namespace Ven4Docking.Core.Tests;

public class BrandCatalogTests
{
    private static BrandCatalog Load() =>
        BrandCatalog.Load(Path.Combine(AppContext.BaseDirectory, "Assets", "brands.json"));

    [Fact]
    public void Brands_json_loads_and_every_brand_has_steps()
    {
        var c = Load();
        Assert.True(c.Brands.Count >= 10);
        Assert.Equal(c.Brands.Count, c.Brands.Select(b => b.Id).Distinct().Count());
        foreach (var b in c.Brands)
        {
            Assert.False(string.IsNullOrWhiteSpace(b.Name), b.Id);
            Assert.NotEmpty(b.Match);
            Assert.NotEmpty(b.Developer);
            Assert.NotEmpty(b.ChargeLimit);
            // Each section either has its own steps or falls back to generic ones.
            Assert.NotEmpty(c.StepsOrGeneric(b, x => x.KeepAlive));
            Assert.NotEmpty(c.StepsOrGeneric(b, x => x.Launcher));
            Assert.NotEmpty(c.StepsOrGeneric(b, x => x.Wifi));
            Assert.NotEmpty(c.StepsOrGeneric(b, x => x.PermissionDenied));
        }
    }

    [Fact]
    public void Every_step_has_russian_and_english_text()
    {
        var c = Load();
        var all = c.Brands.Append(c.Generic).SelectMany(b =>
            b.Developer.Concat(b.ExtraToggles).Concat(b.Wifi).Concat(b.KeepAlive).Concat(b.Launcher).Concat(b.ChargeLimit).Concat(b.PermissionDenied));
        foreach (var s in all)
        {
            Assert.False(string.IsNullOrWhiteSpace(s.Ru));
            Assert.False(string.IsNullOrWhiteSpace(s.En), s.Ru);
        }
    }

    [Fact]
    public void Generic_has_every_section()
    {
        var g = Load().Generic;
        Assert.NotEmpty(g.Developer);
        Assert.NotEmpty(g.Wifi);
        Assert.NotEmpty(g.KeepAlive);
        Assert.NotEmpty(g.Launcher);
        Assert.NotEmpty(g.ChargeLimit);
        Assert.NotEmpty(g.PermissionDenied);
    }

    [Theory]
    [InlineData("realme", "realme", "oppo")]
    [InlineData("OnePlus", "OnePlus", "oppo")]
    [InlineData("Xiaomi", "Redmi", "xiaomi")]
    [InlineData("samsung", "samsung", "samsung")]
    [InlineData("HUAWEI", "HONOR", "huawei")]
    [InlineData("TECNO MOBILE LIMITED", "TECNO", "transsion")]
    [InlineData("Google", "google", "google")]
    [InlineData("Fairphone", "Fairphone", "generic")]
    [InlineData(null, null, "generic")]
    public void Brand_is_matched_by_manufacturer_or_brand(string? manufacturer, string? brand, string expected) =>
        Assert.Equal(expected, Load().Find(manufacturer, brand).Id);

    [Fact]
    public void Unconfirmed_items_are_flagged_and_realme_has_permission_toggle()
    {
        var c = Load();
        Assert.Contains(c.Brands.SelectMany(b => b.Developer.Concat(b.Launcher)), s => s.Unconfirmed);
        var oppo = c.Find("realme");
        Assert.Contains(oppo.ExtraToggles, s => s.Ru.Contains("Отключить контроль разрешений"));
        Assert.Contains(c.Find("samsung").ExtraToggles, s => s.Ru.Contains("Автоблокировка"));
        Assert.Equal("Turn on «Install via USB» (needs a Mi account), otherwise install fails with INSTALL_FAILED_USER_RESTRICTED.",
            c.Find("xiaomi").ExtraToggles[0].Text("en"));
    }
}
