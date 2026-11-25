using System.IO.Compression;

namespace Ven4Docking.Core.Adb;

/// <summary>
/// Downloads Android platform-tools from dl.google.com (spec §9: Google's license does not allow shipping adb
/// in our installer). Called only after the user agreed in the consent dialog.
/// </summary>
public sealed class PlatformToolsDownloader
{
    public static readonly Uri Source = new("https://dl.google.com/android/repository/platform-tools-latest-windows.zip");

    private readonly HttpClient _http;

    public PlatformToolsDownloader(HttpClient http) => _http = http;

    /// <param name="targetDir">…\platform-tools (the zip's root folder is "platform-tools").</param>
    /// <returns>Path to adb.exe.</returns>
    public async Task<string> DownloadAsync(string targetDir, IProgress<double>? progress = null, CancellationToken ct = default)
    {
        string parent = Path.GetDirectoryName(Path.GetFullPath(targetDir))!;
        Directory.CreateDirectory(parent);
        string zip = Path.Combine(parent, "platform-tools-download.zip");

        using (var resp = await _http.GetAsync(Source, HttpCompletionOption.ResponseHeadersRead, ct).ConfigureAwait(false))
        {
            resp.EnsureSuccessStatusCode();
            long? total = resp.Content.Headers.ContentLength;
            await using var src = await resp.Content.ReadAsStreamAsync(ct).ConfigureAwait(false);
            await using var dst = File.Create(zip);
            var buf = new byte[81920];
            long done = 0;
            int n;
            while ((n = await src.ReadAsync(buf, ct).ConfigureAwait(false)) > 0)
            {
                await dst.WriteAsync(buf.AsMemory(0, n), ct).ConfigureAwait(false);
                done += n;
                if (total > 0) progress?.Report((double)done / total.Value);
            }
        }

        string staging = Path.Combine(parent, "platform-tools-staging");
        if (Directory.Exists(staging)) Directory.Delete(staging, recursive: true);
        ZipFile.ExtractToDirectory(zip, staging);
        File.Delete(zip);

        string extracted = Path.Combine(staging, "platform-tools");
        if (!File.Exists(Path.Combine(extracted, "adb.exe")))
        {
            Directory.Delete(staging, recursive: true);
            throw new InvalidDataException("adb.exe not found in the downloaded archive");
        }

        if (Directory.Exists(targetDir)) Directory.Delete(targetDir, recursive: true);
        Directory.Move(extracted, targetDir);
        Directory.Delete(staging, recursive: true);
        progress?.Report(1.0);
        return Path.Combine(targetDir, "adb.exe");
    }
}
