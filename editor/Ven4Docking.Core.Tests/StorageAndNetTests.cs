using System.IO.Compression;
using System.Net;
using Ven4Docking.Core.Adb;
using Ven4Docking.Core.Apps;
using Ven4Docking.Core.Models;
using Ven4Docking.Core.Net;
using Ven4Docking.Core.Storage;

namespace Ven4Docking.Core.Tests;

public class LocalStoreTests
{
    [Fact]
    public void Home_comes_from_VEN4DOCKING_HOME_and_local_data_stays_inside_it()
    {
        using var t = new TempDir();
        var p = EditorPaths.FromEnvironment(n => n == EditorPaths.HomeVariable ? t.Path : null);
        Assert.Equal(t.Path, p.Home);
        Assert.True(p.IsOverridden);
        Assert.StartsWith(t.Path, p.PlatformToolsDir);
        Assert.Equal(Path.Combine(t.Path, "admin.key"), p.AdminKeyFile);
        Assert.Equal(Path.Combine(t.Path, "layouts", "p1.json"), p.LayoutFile("p1"));
        Assert.Equal(Path.Combine(t.Path, "layouts", "______x.json"), p.LayoutFile("../..\\x"));
    }

    [Fact]
    public void Default_home_is_appdata()
    {
        var p = EditorPaths.FromEnvironment(_ => null);
        Assert.EndsWith(Path.Combine("AppData", "Roaming", "Ven4Docking"), p.Home);
    }

    [Fact]
    public void Layout_save_bumps_version_and_refuses_overlap()
    {
        using var t = new TempDir();
        var store = new LocalStore(new EditorPaths(t.Path, Path.Combine(t.Path, "local")));
        Assert.Null(store.LoadLayout("p1"));

        var layout = LayoutTemplates.Create(LayoutTemplates.Classic);
        Assert.Equal(1, store.SaveLayout("p1", layout).NewVersion);
        Assert.Equal(2, store.SaveLayout("p1", store.LoadLayout("p1")!).NewVersion);

        var bad = store.LoadLayout("p1")!;
        bad.Screens[0].Blocks[1].X = 0; // onto the clock
        var r = store.SaveLayout("p1", bad);
        Assert.False(r.Ok);
        Assert.Equal("overlap", r.Error);
        Assert.Contains("b1", r.Blocks!);
        Assert.Equal(2, store.LoadLayout("p1")!.Version);
    }

    [Fact]
    public void Config_round_trips_through_file()
    {
        using var t = new TempDir();
        var store = new LocalStore(new EditorPaths(t.Path, t.Path));
        var cfg = store.LoadConfig();
        Assert.Equal(1, cfg.Version);
        cfg.Port = 18765;
        cfg.Apps.Add(new AppEntry { Id = "tg", Title = "Telegram", Kind = AppEntry.KindUrl, Target = "tg://" });
        store.SaveConfig(cfg);
        var back = store.LoadConfig();
        Assert.Equal(18765, back.EffectivePort);
        Assert.Equal("tg://", back.Apps.Single().Target);
        Assert.False(File.ReadAllBytes(store.Paths.ConfigFile).Take(3).SequenceEqual(new byte[] { 0xEF, 0xBB, 0xBF }));
    }

    [Fact]
    public void Admin_key_is_read_trimmed()
    {
        using var t = new TempDir();
        var store = new LocalStore(new EditorPaths(t.Path, t.Path));
        Assert.Null(store.ReadAdminKey());
        File.WriteAllText(store.Paths.AdminKeyFile, "abc123\r\n");
        Assert.Equal("abc123", store.ReadAdminKey());
    }
}

public class NetClientTests
{
    [Fact]
    public async Task Geocoding_returns_places()
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.OK,
            """{"results":[{"name":"Москва","latitude":55.75222,"longitude":37.61556,"country":"Россия","admin1":"Москва"}]}"""));
        var places = await new GeocodingClient(new HttpClient(h)).SearchAsync("Моск", "ru");
        var p = Assert.Single(places);
        Assert.Equal("Москва, Москва, Россия", p.Display);
        Assert.Equal(55.7522, p.ToWeather().Lat);
        var uri = h.Calls[0].Request.RequestUri!;
        Assert.Equal("geocoding-api.open-meteo.com", uri.Host);
        Assert.Contains("language=ru", uri.Query);
    }

    [Fact]
    public async Task Geocoding_without_results_is_empty()
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.OK, """{"generationtime_ms":0.5}"""));
        Assert.Empty(await new GeocodingClient(new HttpClient(h)).SearchAsync("qqqqq", "en"));
        Assert.Empty(await new GeocodingClient(new HttpClient(h)).SearchAsync("q", "en"));
    }

    [Fact]
    public async Task Update_check_handles_404_as_no_releases()
    {
        var h = new StubHandler((_, _) => new HttpResponseMessage(HttpStatusCode.NotFound));
        var r = await new UpdateChecker(new HttpClient(h)).CheckAsync(new Version(1, 0, 0));
        Assert.Equal(UpdateState.NoReleases, r.State);
        Assert.Equal("api.github.com", h.Calls[0].Request.RequestUri!.Host);
        Assert.Contains("/repos/Ven4ru/Ven4Docking/releases/latest", h.Calls[0].Request.RequestUri!.AbsolutePath);
        Assert.NotEmpty(h.Calls[0].Request.Headers.UserAgent);
    }

    [Theory]
    [InlineData("v1.0.1", UpdateState.Available)]
    [InlineData("v1.0.0", UpdateState.UpToDate)]
    [InlineData("0.9.0", UpdateState.UpToDate)]
    [InlineData("v2.0.0-beta.1", UpdateState.Available)]
    public async Task Update_check_compares_versions(string tag, UpdateState expected)
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.OK,
            $$"""{"tag_name":"{{tag}}","html_url":"https://github.com/Ven4ru/Ven4Docking/releases/tag/{{tag}}"}"""));
        var r = await new UpdateChecker(new HttpClient(h)).CheckAsync(new Version(1, 0, 0, 0));
        Assert.Equal(expected, r.State);
    }

    [Fact]
    public async Task Update_check_network_error_is_failed_not_thrown()
    {
        var h = new StubHandler((_, _) => throw new HttpRequestException("offline"));
        Assert.Equal(UpdateState.Failed, (await new UpdateChecker(new HttpClient(h)).CheckAsync(new Version(1, 0, 0))).State);
    }

    [Fact]
    public async Task Platform_tools_are_downloaded_from_google_and_extracted()
    {
        using var t = new TempDir();
        var zip = new MemoryStream();
        using (var a = new ZipArchive(zip, ZipArchiveMode.Create, leaveOpen: true))
        {
            using (var w = new StreamWriter(a.CreateEntry("platform-tools/adb.exe").Open())) w.Write("fake adb");
            using (var w = new StreamWriter(a.CreateEntry("platform-tools/AdbWinApi.dll").Open())) w.Write("dll");
        }
        var h = new StubHandler((_, _) => new HttpResponseMessage(HttpStatusCode.OK) { Content = new ByteArrayContent(zip.ToArray()) });
        var target = Path.Combine(t.Path, "local", "platform-tools");
        var progress = new List<double>();

        string adb = await new PlatformToolsDownloader(new HttpClient(h)).DownloadAsync(target, new SyncProgress(progress.Add));

        Assert.Equal("dl.google.com", h.Calls.Single().Request.RequestUri!.Host);
        Assert.Equal(Path.Combine(target, "adb.exe"), adb);
        Assert.Equal("fake adb", File.ReadAllText(adb));
        Assert.True(File.Exists(Path.Combine(target, "AdbWinApi.dll")));
        Assert.Equal(1.0, progress.Last());
        Assert.Equal(new[] { "platform-tools" }, Directory.GetFileSystemEntries(Path.Combine(t.Path, "local")).Select(Path.GetFileName));
    }

    [Fact]
    public async Task Platform_tools_archive_without_adb_is_rejected()
    {
        using var t = new TempDir();
        var zip = new MemoryStream();
        using (var a = new ZipArchive(zip, ZipArchiveMode.Create, leaveOpen: true)) a.CreateEntry("other/readme.txt");
        var h = new StubHandler((_, _) => new HttpResponseMessage(HttpStatusCode.OK) { Content = new ByteArrayContent(zip.ToArray()) });
        await Assert.ThrowsAsync<InvalidDataException>(() =>
            new PlatformToolsDownloader(new HttpClient(h)).DownloadAsync(Path.Combine(t.Path, "platform-tools")));
        Assert.False(Directory.Exists(Path.Combine(t.Path, "platform-tools")));
    }

    [Theory]
    [InlineData("Connection timed out", ServerErrors.Timeout)]
    [InlineData("connection refused", ServerErrors.PortClosed)]
    [InlineData("authentication failed (publickey)", ServerErrors.KeyRejected)]
    [InlineData("host key mismatch", ServerErrors.HostKeyMismatch)]
    [InlineData("unknown host key, not in known_hosts", ServerErrors.HostKeyUnknown)]
    [InlineData("failed to resolve host", ServerErrors.Dns)]
    [InlineData(null, ServerErrors.Other)]
    public void Server_errors_are_classified(string? err, string expected) => Assert.Equal(expected, ServerErrors.Classify(err));

    [Fact]
    public void Private_addresses()
    {
        Assert.True(LanAddress.IsPrivate(IPAddress.Parse("192.168.1.5")));
        Assert.True(LanAddress.IsPrivate(IPAddress.Parse("172.20.0.1")));
        Assert.True(LanAddress.IsPrivate(IPAddress.Parse("100.100.1.1")));
        Assert.False(LanAddress.IsPrivate(IPAddress.Parse("203.0.113.10")));
        Assert.False(LanAddress.IsPrivate(IPAddress.Parse("172.32.0.1")));
    }

    private sealed class SyncProgress(Action<double> report) : IProgress<double>
    {
        public void Report(double value) => report(value);
    }
}

public class StartMenuScannerTests
{
    [Fact]
    public void Finds_shortcuts_and_skips_uninstallers()
    {
        using var t = new TempDir();
        Directory.CreateDirectory(Path.Combine(t.Path, "Tools"));
        File.WriteAllText(Path.Combine(t.Path, "Telegram.lnk"), "");
        File.WriteAllText(Path.Combine(t.Path, "Uninstall Telegram.lnk"), "");
        File.WriteAllText(Path.Combine(t.Path, "Tools", "Сайт.url"), "");
        File.WriteAllText(Path.Combine(t.Path, "Tools", "notes.txt"), "");
        File.WriteAllText(Path.Combine(t.Path, "Tools", "Telegram.lnk"), ""); // duplicate name

        // Sorted by the current UI culture, so compare as a set.
        var apps = StartMenuScanner.Scan(new[] { t.Path, Path.Combine(t.Path, "missing") })
            .OrderBy(a => a.Name, StringComparer.Ordinal).ToList();
        Assert.Equal(new[] { "Telegram", "Сайт" }, apps.Select(a => a.Name));
        Assert.True(apps[1].IsUrlShortcut);
        Assert.Equal("Tools", apps[1].Folder);
        Assert.Equal(Path.Combine(t.Path, "Telegram.lnk"), apps[0].Path);

        var entry = StartMenuScanner.ToEntry(apps[0], new[] { "telegram" });
        Assert.Equal(("telegram-2", AppEntry.KindShortcut), (entry.Id, entry.Kind));
    }

    [Theory]
    [InlineData("Telegram Desktop", "telegram-desktop")]
    [InlineData("Яндекс Музыка", "yandeks-muzyka")]
    [InlineData("!!!", "app")]
    public void Ids_are_ascii(string title, string id) => Assert.Equal(id, StartMenuScanner.MakeId(title, Array.Empty<string>()));
}
