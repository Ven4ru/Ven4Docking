using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace Ven4Docking.Core.Net;

public enum UpdateDownloadError
{
    None,
    /// <summary>The release has no Ven4Docking-Setup-&lt;ver&gt;.exe or no SHA256SUMS.</summary>
    NoAsset,
    /// <summary>SHA256SUMS has no line for the installer.</summary>
    NoChecksum,
    /// <summary>A download (or a redirect) points outside GitHub.</summary>
    BadHost,
    Network,
    /// <summary>The installer does not match SHA256SUMS; the file was deleted.</summary>
    HashMismatch,
    Disk,
    Cancelled,
}

public sealed record UpdateDownload(UpdateDownloadError Error, string? SetupPath = null, string? Detail = null)
{
    public bool Ok => Error == UpdateDownloadError.None;
}

/// <summary>Bytes received and the expected total (null when the server did not say).</summary>
public readonly record struct DownloadProgress(long Received, long? Total);

/// <summary>
/// Downloads the installer of a newer release (spec §9: installing only by a button) into
/// <c>%TEMP%\Ven4Docking-update\&lt;ver&gt;\</c> together with SHA256SUMS and checks the installer's SHA-256 against it
/// (the file build.ps1 and the release workflow write: «&lt;sha256 lowercase&gt;  &lt;file name&gt;» per line).
/// Only GitHub addresses are followed, redirects included.
/// </summary>
public sealed class UpdateInstaller : IDisposable
{
    public const string SumsName = "SHA256SUMS";
    public const long MaxSumsBytes = 64 * 1024;
    private const int MaxRedirects = 5;
    private static readonly TimeSpan DownloadTimeout = TimeSpan.FromMinutes(30);

    /// <summary>
    /// github.com serves browser_download_url and redirects to the asset storage: objects.githubusercontent.com,
    /// or release-assets.githubusercontent.com on newer releases.
    /// </summary>
    public static readonly IReadOnlyList<string> AllowedHosts =
        new[] { "github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com" };

    private readonly HttpClient _http;
    private readonly string _root;

    /// <param name="handler">For tests; the real one does not follow redirects by itself (each hop is checked here).</param>
    /// <param name="root">Default: %TEMP%\Ven4Docking-update.</param>
    public UpdateInstaller(HttpMessageHandler? handler = null, string? root = null)
    {
        _http = handler is null
            ? new HttpClient(new SocketsHttpHandler { AllowAutoRedirect = false }) { Timeout = DownloadTimeout }
            : new HttpClient(handler, disposeHandler: false) { Timeout = DownloadTimeout };
        _root = root ?? Path.Combine(Path.GetTempPath(), "Ven4Docking-update");
    }

    public static string SetupName(string version) => $"Ven4Docking-Setup-{version}.exe";

    public static bool IsAllowedUrl(Uri? uri) =>
        uri is { IsAbsoluteUri: true } && uri.Scheme == Uri.UriSchemeHttps && uri.IsDefaultPort
        && AllowedHosts.Contains(uri.Host, StringComparer.OrdinalIgnoreCase);

    private static readonly Regex SumLine = new(@"^(?<h>[0-9a-fA-F]{64}) [ *](?<n>.+)$", RegexOptions.Compiled);

    /// <summary>SHA256SUMS → file name → lowercase hash. Accepts «hash  name» and «hash *name» (sha256sum -b).</summary>
    public static IReadOnlyDictionary<string, string> ParseSums(string text)
    {
        var map = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var raw in text.Split('\n'))
        {
            var m = SumLine.Match(raw.TrimEnd('\r').Trim('﻿'));
            if (m.Success) map[m.Groups["n"].Value.Trim()] = m.Groups["h"].Value.ToLowerInvariant();
        }
        return map;
    }

    /// <summary>Downloads and checks; the returned path is a verified installer.</summary>
    public async Task<UpdateDownload> DownloadAsync(string version, IReadOnlyList<ReleaseAsset> assets,
        IProgress<DownloadProgress>? progress = null, CancellationToken ct = default)
    {
        if (!Regex.IsMatch(version, @"^\d{1,5}\.\d{1,5}\.\d{1,5}$"))
            return new UpdateDownload(UpdateDownloadError.NoAsset, Detail: version);
        string setupName = SetupName(version);
        var setup = assets.FirstOrDefault(a => a.Name == setupName);
        var sums = assets.FirstOrDefault(a => a.Name == SumsName);
        if (setup is null || sums is null)
            return new UpdateDownload(UpdateDownloadError.NoAsset, Detail: setup is null ? setupName : SumsName);
        if (!IsAllowedUrl(setup.Url) || !IsAllowedUrl(sums.Url))
            return new UpdateDownload(UpdateDownloadError.BadHost, Detail: (IsAllowedUrl(setup.Url) ? sums.Url : setup.Url).Host);

        string dir = Path.Combine(_root, version);
        string target = Path.Combine(dir, setupName);
        string part = target + ".part";
        try
        {
            Directory.CreateDirectory(dir);

            // checksums first: a small text file
            byte[] sumsBytes;
            using (var resp = await GetAsync(sums.Url, ct).ConfigureAwait(false))
            {
                if (resp.Content.Headers.ContentLength > MaxSumsBytes)
                    return new UpdateDownload(UpdateDownloadError.NoChecksum, Detail: SumsName);
                sumsBytes = await resp.Content.ReadAsByteArrayAsync(ct).ConfigureAwait(false);
            }
            if (sumsBytes.Length > MaxSumsBytes) return new UpdateDownload(UpdateDownloadError.NoChecksum, Detail: SumsName);
            await File.WriteAllBytesAsync(Path.Combine(dir, SumsName), sumsBytes, ct).ConfigureAwait(false);
            if (!ParseSums(Encoding.UTF8.GetString(sumsBytes)).TryGetValue(setupName, out var expected))
                return new UpdateDownload(UpdateDownloadError.NoChecksum, Detail: setupName);

            // already downloaded earlier and intact; a damaged leftover is removed
            if (File.Exists(target))
            {
                if (await HashFileAsync(target, ct).ConfigureAwait(false) == expected)
                {
                    long len = new FileInfo(target).Length;
                    progress?.Report(new DownloadProgress(len, len));
                    return new UpdateDownload(UpdateDownloadError.None, target);
                }
                File.Delete(target);
            }

            string actual;
            using (var resp = await GetAsync(setup.Url, ct).ConfigureAwait(false))
            {
                long? total = resp.Content.Headers.ContentLength ?? setup.Size;
                using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
                await using (var src = await resp.Content.ReadAsStreamAsync(ct).ConfigureAwait(false))
                await using (var dst = new FileStream(part, FileMode.Create, FileAccess.Write, FileShare.None, 81920, useAsync: true))
                {
                    var buf = new byte[81920];
                    long done = 0;
                    int n;
                    progress?.Report(new DownloadProgress(0, total));
                    while ((n = await src.ReadAsync(buf, ct).ConfigureAwait(false)) > 0)
                    {
                        hash.AppendData(buf, 0, n);
                        await dst.WriteAsync(buf.AsMemory(0, n), ct).ConfigureAwait(false);
                        done += n;
                        progress?.Report(new DownloadProgress(done, total));
                    }
                }
                actual = Convert.ToHexStringLower(hash.GetHashAndReset());
            }
            if (actual != expected)
            {
                TryDelete(part);
                return new UpdateDownload(UpdateDownloadError.HashMismatch, Detail: setupName);
            }
            File.Move(part, target, overwrite: true);
            return new UpdateDownload(UpdateDownloadError.None, target);
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
            TryDelete(part);
            return new UpdateDownload(UpdateDownloadError.Cancelled);
        }
        catch (BadHostException e)
        {
            TryDelete(part);
            return new UpdateDownload(UpdateDownloadError.BadHost, Detail: e.Message);
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException)
        {
            TryDelete(part);
            return new UpdateDownload(UpdateDownloadError.Network, Detail: e.Message);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            TryDelete(part);
            return new UpdateDownload(UpdateDownloadError.Disk, Detail: e.Message);
        }
    }

    private sealed class BadHostException(string host) : Exception(host);

    /// <summary>GET with redirects followed by hand, every hop must be a GitHub address. The caller disposes the response.</summary>
    private async Task<HttpResponseMessage> GetAsync(Uri url, CancellationToken ct)
    {
        for (int hop = 0; ; hop++)
        {
            if (!IsAllowedUrl(url)) throw new BadHostException(url.Host);
            var req = new HttpRequestMessage(HttpMethod.Get, url);
            req.Headers.UserAgent.Add(new ProductInfoHeaderValue("Ven4Docking-Editor", "1"));
            req.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/octet-stream"));
            var resp = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, ct).ConfigureAwait(false);
            int code = (int)resp.StatusCode;
            if (code is >= 300 and < 400 && resp.Headers.Location is { } loc)
            {
                resp.Dispose();
                if (hop >= MaxRedirects) throw new HttpRequestException("too many redirects");
                url = loc.IsAbsoluteUri ? loc : new Uri(url, loc);
                continue;
            }
            if (!resp.IsSuccessStatusCode)
            {
                var status = resp.StatusCode;
                string reason = resp.ReasonPhrase ?? "";
                resp.Dispose();
                throw new HttpRequestException($"{code} {reason}".Trim(), null, status);
            }
            return resp;
        }
    }

    private static async Task<string> HashFileAsync(string path, CancellationToken ct)
    {
        await using var fs = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read, 81920, useAsync: true);
        return Convert.ToHexStringLower(await SHA256.HashDataAsync(fs, ct).ConfigureAwait(false));
    }

    private static void TryDelete(string path)
    {
        try { File.Delete(path); } catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
    }

    public void Dispose() => _http.Dispose();
}
