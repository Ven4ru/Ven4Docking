using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;

namespace Ven4Docking.Core.Net;

public enum UpdateState
{
    UpToDate,
    Available,
    /// <summary>404: the repository is not public yet or has no releases.</summary>
    NoReleases,
    Failed,
}

/// <summary>A file of a release (<c>assets[]</c> of the GitHub API).</summary>
public sealed record ReleaseAsset(string Name, Uri Url, long? Size);

public sealed record UpdateResult(UpdateState State, string? LatestVersion = null, string? Url = null, string? Error = null,
    IReadOnlyList<ReleaseAsset>? Assets = null);

/// <summary>Checks GitHub Releases of Ven4ru/Ven4Docking (spec §9). Installing is only ever done by a button.</summary>
public sealed class UpdateChecker
{
    public const string Repo = "Ven4ru/Ven4Docking";
    public static readonly Uri LatestApi = new($"https://api.github.com/repos/{Repo}/releases/latest");
    public const string ReleasesPage = $"https://github.com/{Repo}/releases";

    private readonly HttpClient _http;

    public UpdateChecker(HttpClient http) => _http = http;

    public async Task<UpdateResult> CheckAsync(Version current, CancellationToken ct = default)
    {
        try
        {
            using var req = new HttpRequestMessage(HttpMethod.Get, LatestApi);
            req.Headers.UserAgent.Add(new ProductInfoHeaderValue("Ven4Docking-Editor", current.ToString(3)));
            req.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/vnd.github+json"));
            using var resp = await _http.SendAsync(req, ct).ConfigureAwait(false);
            if (resp.StatusCode == HttpStatusCode.NotFound) return new UpdateResult(UpdateState.NoReleases);
            if (!resp.IsSuccessStatusCode) return new UpdateResult(UpdateState.Failed, Error: ((int)resp.StatusCode).ToString());

            using var doc = JsonDocument.Parse(await resp.Content.ReadAsStringAsync(ct).ConfigureAwait(false));
            string tag = doc.RootElement.TryGetProperty("tag_name", out var t) ? t.GetString() ?? "" : "";
            string? url = doc.RootElement.TryGetProperty("html_url", out var u) ? u.GetString() : ReleasesPage;
            var latest = ParseTag(tag);
            if (latest is null) return new UpdateResult(UpdateState.Failed, tag, url, "bad tag");
            var assets = ParseAssets(doc.RootElement);
            return latest > Normalize(current)
                ? new UpdateResult(UpdateState.Available, latest.ToString(3), url, Assets: assets)
                : new UpdateResult(UpdateState.UpToDate, latest.ToString(3), url, Assets: assets);
        }
        catch (HttpRequestException e) { return new UpdateResult(UpdateState.Failed, Error: e.Message); }
        catch (TaskCanceledException) when (!ct.IsCancellationRequested) { return new UpdateResult(UpdateState.Failed, Error: "timeout"); }
        catch (JsonException e) { return new UpdateResult(UpdateState.Failed, Error: e.Message); }
    }

    private static IReadOnlyList<ReleaseAsset> ParseAssets(JsonElement release)
    {
        var list = new List<ReleaseAsset>();
        if (!release.TryGetProperty("assets", out var assets) || assets.ValueKind != JsonValueKind.Array) return list;
        foreach (var a in assets.EnumerateArray())
        {
            string? name = a.TryGetProperty("name", out var n) && n.ValueKind == JsonValueKind.String ? n.GetString() : null;
            string? href = a.TryGetProperty("browser_download_url", out var b) && b.ValueKind == JsonValueKind.String ? b.GetString() : null;
            long? size = a.TryGetProperty("size", out var s) && s.TryGetInt64(out long sz) ? sz : null;
            if (!string.IsNullOrEmpty(name) && Uri.TryCreate(href, UriKind.Absolute, out var uri))
                list.Add(new ReleaseAsset(name, uri, size));
        }
        return list;
    }

    public static Version? ParseTag(string tag)
    {
        string s = tag.Trim().TrimStart('v', 'V');
        int dash = s.IndexOfAny(new[] { '-', '+' });
        if (dash >= 0) s = s[..dash];
        return Version.TryParse(s, out var v) ? Normalize(v) : null;
    }

    private static Version Normalize(Version v) => new(v.Major, Math.Max(v.Minor, 0), Math.Max(v.Build, 0));
}
