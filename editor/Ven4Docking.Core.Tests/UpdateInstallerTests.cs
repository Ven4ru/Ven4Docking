using System.Net;
using System.Security.Cryptography;
using System.Text;
using Ven4Docking.Core.Net;

namespace Ven4Docking.Core.Tests;

public class UpdateInstallerTests
{
    private const string Version = "1.1.0";
    private static readonly byte[] SetupBytes = Encoding.ASCII.GetBytes(new string('x', 300_000));
    private static string Sha(byte[] b) => Convert.ToHexStringLower(SHA256.HashData(b));

    private static readonly Uri SetupUrl = new($"https://github.com/Ven4ru/Ven4Docking/releases/download/v{Version}/Ven4Docking-Setup-{Version}.exe");
    private static readonly Uri SumsUrl = new($"https://github.com/Ven4ru/Ven4Docking/releases/download/v{Version}/SHA256SUMS");

    private static IReadOnlyList<ReleaseAsset> Assets() => new[]
    {
        new ReleaseAsset($"Ven4Docking-{Version}.zip", new Uri("https://github.com/Ven4ru/Ven4Docking/releases/download/v1.1.0/Ven4Docking-1.1.0.zip"), 10),
        new ReleaseAsset($"Ven4Docking-Setup-{Version}.exe", SetupUrl, SetupBytes.Length),
        new ReleaseAsset("SHA256SUMS", SumsUrl, 200),
    };

    /// <summary>SHA256SUMS exactly as build.ps1 writes it: «hash  name», LF, trailing newline.</summary>
    private static string Sums(string setupHash) =>
        $"{setupHash}  Ven4Docking-Setup-{Version}.exe\n{new string('a', 64)}  Ven4Docking-{Version}.zip\n{new string('b', 64)}  ven4docking-launcher-1.1.0.apk\n";

    /// <summary>github.com answers 302 to the asset storage, like the real download URLs.</summary>
    private static StubHandler GitHub(string sums, byte[] setup, string storageHost = "objects.githubusercontent.com") => new((req, _) =>
    {
        var u = req.RequestUri!;
        if (u.Host == "github.com")
        {
            var r = new HttpResponseMessage(HttpStatusCode.Found);
            r.Headers.Location = new Uri($"https://{storageHost}/store/{Path.GetFileName(u.AbsolutePath)}");
            return r;
        }
        if (u.AbsolutePath.EndsWith("/SHA256SUMS")) return new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(sums) };
        if (u.AbsolutePath.EndsWith(".exe")) return new HttpResponseMessage(HttpStatusCode.OK) { Content = new ByteArrayContent(setup) };
        return new HttpResponseMessage(HttpStatusCode.NotFound);
    });

    private sealed class Collect : IProgress<DownloadProgress>
    {
        public List<DownloadProgress> Items { get; } = new();
        public void Report(DownloadProgress value) => Items.Add(value);
    }

    [Fact]
    public void Sums_in_build_ps1_and_sha256sum_formats_are_parsed()
    {
        string h1 = new string('A', 64), h2 = new string('c', 64);
        var map = UpdateInstaller.ParseSums($"﻿{h1}  Ven4Docking-Setup-1.1.0.exe\r\n{h2} *Ven4Docking-1.1.0.zip\nnot a line\n{h2}  \n\n{new string('d', 63)}  short.exe\n");
        Assert.Equal(2, map.Count);
        Assert.Equal(new string('a', 64), map["Ven4Docking-Setup-1.1.0.exe"]);
        Assert.Equal(h2, map["Ven4Docking-1.1.0.zip"]);
        Assert.Empty(UpdateInstaller.ParseSums(""));
    }

    [Fact]
    public void Only_github_https_addresses_are_allowed()
    {
        Assert.True(UpdateInstaller.IsAllowedUrl(SetupUrl));
        Assert.True(UpdateInstaller.IsAllowedUrl(new Uri("https://objects.githubusercontent.com/x")));
        Assert.True(UpdateInstaller.IsAllowedUrl(new Uri("https://release-assets.githubusercontent.com/x")));
        Assert.False(UpdateInstaller.IsAllowedUrl(new Uri("http://github.com/x")));
        Assert.False(UpdateInstaller.IsAllowedUrl(new Uri("https://github.com.evil.example/x")));
        Assert.False(UpdateInstaller.IsAllowedUrl(new Uri("https://github.com:8443/x")));
        Assert.False(UpdateInstaller.IsAllowedUrl(new Uri("https://example.com/x")));
        Assert.False(UpdateInstaller.IsAllowedUrl(null));
    }

    [Fact]
    public async Task Downloads_checks_hash_and_reports_progress()
    {
        using var tmp = new TempDir();
        var h = GitHub(Sums(Sha(SetupBytes)), SetupBytes);
        using var inst = new UpdateInstaller(h, tmp.Path);
        var progress = new Collect();
        var r = await inst.DownloadAsync(Version, Assets(), progress);

        Assert.True(r.Ok, r.Error + " " + r.Detail);
        Assert.Equal(Path.Combine(tmp.Path, Version, $"Ven4Docking-Setup-{Version}.exe"), r.SetupPath);
        Assert.Equal(SetupBytes, File.ReadAllBytes(r.SetupPath!));
        Assert.True(File.Exists(Path.Combine(tmp.Path, Version, "SHA256SUMS")));
        Assert.False(File.Exists(r.SetupPath + ".part"));
        // progress follows the bytes up to the total
        var last = progress.Items[^1];
        Assert.Equal((SetupBytes.LongLength, (long?)SetupBytes.LongLength), (last.Received, last.Total));
        Assert.True(progress.Items.Count > 2);
        // every request went to GitHub; the zip was not downloaded
        Assert.All(h.Calls, c => Assert.True(UpdateInstaller.IsAllowedUrl(c.Request.RequestUri)));
        Assert.DoesNotContain(h.Calls, c => c.Request.RequestUri!.AbsolutePath.EndsWith(".zip"));

        // a second run reuses the checked file without downloading it again
        int before = h.Calls.Count;
        var again = await inst.DownloadAsync(Version, Assets());
        Assert.True(again.Ok);
        Assert.DoesNotContain(h.Calls.Skip(before), c => c.Request.RequestUri!.AbsolutePath.EndsWith(".exe"));
    }

    [Fact]
    public async Task Hash_mismatch_deletes_the_file()
    {
        using var tmp = new TempDir();
        using var inst = new UpdateInstaller(GitHub(Sums(new string('0', 64)), SetupBytes, "release-assets.githubusercontent.com"), tmp.Path);
        var r = await inst.DownloadAsync(Version, Assets());
        Assert.Equal(UpdateDownloadError.HashMismatch, r.Error);
        Assert.Null(r.SetupPath);
        Assert.Empty(Directory.GetFiles(Path.Combine(tmp.Path, Version), "*.exe*"));
    }

    [Fact]
    public async Task Damaged_earlier_download_is_replaced()
    {
        using var tmp = new TempDir();
        Directory.CreateDirectory(Path.Combine(tmp.Path, Version));
        string target = Path.Combine(tmp.Path, Version, $"Ven4Docking-Setup-{Version}.exe");
        File.WriteAllText(target, "broken");
        using var inst = new UpdateInstaller(GitHub(Sums(Sha(SetupBytes)), SetupBytes), tmp.Path);
        var r = await inst.DownloadAsync(Version, Assets());
        Assert.True(r.Ok);
        Assert.Equal(SetupBytes, File.ReadAllBytes(target));
    }

    [Fact]
    public async Task No_checksum_line_means_no_installer()
    {
        using var tmp = new TempDir();
        var h = GitHub($"{new string('a', 64)}  Ven4Docking-{Version}.zip\n", SetupBytes);
        using var inst = new UpdateInstaller(h, tmp.Path);
        var r = await inst.DownloadAsync(Version, Assets());
        Assert.Equal(UpdateDownloadError.NoChecksum, r.Error);
        Assert.DoesNotContain(h.Calls, c => c.Request.RequestUri!.AbsolutePath.EndsWith(".exe"));
    }

    [Fact]
    public async Task Missing_assets_and_odd_versions_are_refused_without_requests()
    {
        using var tmp = new TempDir();
        var h = GitHub(Sums(Sha(SetupBytes)), SetupBytes);
        using var inst = new UpdateInstaller(h, tmp.Path);
        Assert.Equal(UpdateDownloadError.NoAsset, (await inst.DownloadAsync(Version, Assets().Where(a => a.Name != "SHA256SUMS").ToList())).Error);
        Assert.Equal(UpdateDownloadError.NoAsset, (await inst.DownloadAsync("1.2.0", Assets())).Error);
        Assert.Equal(UpdateDownloadError.NoAsset, (await inst.DownloadAsync(@"..\1.1.0", Assets())).Error);
        Assert.Empty(h.Calls);
    }

    [Fact]
    public async Task Non_github_addresses_and_redirects_are_refused()
    {
        using var tmp = new TempDir();
        var foreign = Assets().Select(a => a.Name.EndsWith(".exe") ? a with { Url = new Uri("https://example.com/setup.exe") } : a).ToList();
        var h = GitHub(Sums(Sha(SetupBytes)), SetupBytes);
        using (var inst = new UpdateInstaller(h, tmp.Path))
            Assert.Equal(UpdateDownloadError.BadHost, (await inst.DownloadAsync(Version, foreign)).Error);
        Assert.Empty(h.Calls);

        var redirectOut = GitHub(Sums(Sha(SetupBytes)), SetupBytes, "evil.example");
        using (var inst = new UpdateInstaller(redirectOut, tmp.Path))
        {
            var r = await inst.DownloadAsync(Version, Assets());
            Assert.Equal(UpdateDownloadError.BadHost, r.Error);
            Assert.Equal("evil.example", r.Detail);
        }
        Assert.DoesNotContain(redirectOut.Calls, c => c.Request.RequestUri!.Host == "evil.example");
    }

    [Fact]
    public async Task Network_errors_and_cancel_are_reported()
    {
        using var tmp = new TempDir();
        using (var inst = new UpdateInstaller(new StubHandler((_, _) => new HttpResponseMessage(HttpStatusCode.InternalServerError)), tmp.Path))
            Assert.Equal(UpdateDownloadError.Network, (await inst.DownloadAsync(Version, Assets())).Error);

        using var cts = new CancellationTokenSource();
        cts.Cancel();
        using (var inst = new UpdateInstaller(GitHub(Sums(Sha(SetupBytes)), SetupBytes), tmp.Path))
            Assert.Equal(UpdateDownloadError.Cancelled, (await inst.DownloadAsync(Version, Assets(), null, cts.Token)).Error);
        Assert.Empty(Directory.GetFiles(tmp.Path, "*.exe*", SearchOption.AllDirectories));
    }

    [Fact]
    public async Task Checker_returns_release_assets()
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.OK, $$"""
            {"tag_name":"v1.1.0","html_url":"https://github.com/Ven4ru/Ven4Docking/releases/tag/v1.1.0",
             "assets":[{"name":"Ven4Docking-Setup-1.1.0.exe","size":300000,"browser_download_url":"{{SetupUrl}}"},
                       {"name":"SHA256SUMS","size":200,"browser_download_url":"{{SumsUrl}}"},
                       {"name":"broken"}]}
            """));
        var r = await new UpdateChecker(new HttpClient(h)).CheckAsync(new Version(1, 0, 0));
        Assert.Equal(UpdateState.Available, r.State);
        Assert.Equal("1.1.0", r.LatestVersion);
        Assert.Equal(new[] { "Ven4Docking-Setup-1.1.0.exe", "SHA256SUMS" }, r.Assets!.Select(a => a.Name));
        Assert.Equal(SetupUrl, r.Assets![0].Url);
        Assert.Equal(300000, r.Assets[0].Size);
    }
}
