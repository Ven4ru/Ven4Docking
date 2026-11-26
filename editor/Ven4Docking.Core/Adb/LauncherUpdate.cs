using System.Text.Json;
using System.Text.RegularExpressions;

namespace Ven4Docking.Core.Adb;

/// <summary>
/// The launcher APK shipped next to the editor (<c>apk\ven4docking.apk</c>) and its version from <c>apk\latest.json</c>
/// (written by build.ps1: <c>{"version", "versionCode", "url", "sha256"}</c>). The launcher does not install anything
/// itself: it is updated from the PC over adb, from «Телефоны».
/// </summary>
public sealed record LauncherApk(string Path, string? Version, int? VersionCode)
{
    public const string LatestJson = "latest.json";

    /// <summary>Null when there is no APK; Version stays null when latest.json is missing or unreadable.</summary>
    public static LauncherApk? Locate(string? apkPath)
    {
        if (string.IsNullOrEmpty(apkPath) || !File.Exists(apkPath)) return null;
        string json = System.IO.Path.Combine(System.IO.Path.GetDirectoryName(apkPath)!, LatestJson);
        try
        {
            if (!File.Exists(json)) return new LauncherApk(apkPath, null, null);
            using var doc = JsonDocument.Parse(File.ReadAllText(json));
            var root = doc.RootElement;
            string? version = root.TryGetProperty("version", out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
            int? code = root.TryGetProperty("versionCode", out var c) && c.TryGetInt32(out int n) ? n : null;
            return new LauncherApk(apkPath, string.IsNullOrWhiteSpace(version) ? null : version.Trim(), code);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException or InvalidOperationException)
        {
            return new LauncherApk(apkPath, null, null);
        }
    }

    /// <summary>«1.1.0», «v1.1.0», «1.1.0-debug» → 1.1.0; null for anything else.</summary>
    public static Version? ParseVersion(string? text)
    {
        if (string.IsNullOrWhiteSpace(text)) return null;
        var m = Regex.Match(text.Trim(), @"^[vV]?(\d+)\.(\d+)(?:\.(\d+))?");
        if (!m.Success) return null;
        return new Version(int.Parse(m.Groups[1].Value), int.Parse(m.Groups[2].Value),
            m.Groups[3].Success ? int.Parse(m.Groups[3].Value) : 0);
    }

    /// <summary>
    /// Offer «Обновить лаунчер»: when the phone's version is older than the APK, or when one of them is unknown
    /// (a launcher before 1.0 did not report its version; reinstalling the same APK is harmless).
    /// </summary>
    public static bool IsUpdateOffered(string? phoneVersion, string? apkVersion)
    {
        var phone = ParseVersion(phoneVersion);
        var apk = ParseVersion(apkVersion);
        return phone is null || apk is null || phone < apk;
    }
}

/// <summary>Outcome of <see cref="LauncherInstaller.InstallAsync"/>; <see cref="ErrorKind"/> is one of <see cref="AdbErrors"/>.</summary>
public sealed record LauncherInstallResult(bool Ok, string ErrorKind = AdbErrors.None, string? Detail = null, string? InstalledVersion = null);

/// <summary>Updates the launcher over adb, the same way the wizard installs it: install -r, start, read the version back.</summary>
public static class LauncherInstaller
{
    /// <summary>
    /// The device to update without asking: the only ready one, or the only one whose model matches the phone card
    /// (<c>adb devices -l</c> writes «RMX2185», «SM_A525F»). Null → let the user choose.
    /// </summary>
    public static AdbDevice? PickDevice(IReadOnlyList<AdbDevice> devices, string? phoneModel)
    {
        var ready = devices.Where(d => d.IsReady).ToList();
        if (ready.Count == 1) return ready[0];
        if (string.IsNullOrWhiteSpace(phoneModel)) return null;
        string want = Norm(phoneModel);
        var same = ready.Where(d => d.Model is not null && Norm(d.Model) == want).ToList();
        return same.Count == 1 ? same[0] : null;
    }

    private static string Norm(string s) => s.Replace('_', ' ').Trim().ToUpperInvariant();

    public static async Task<LauncherInstallResult> InstallAsync(AdbClient adb, string serial, LauncherApk apk, CancellationToken ct = default)
    {
        var r = await adb.RunAsync(serial, WizardCommandBuilder.Install(apk.Path), AdbClient.InstallTimeout, ct).ConfigureAwait(false);
        string kind = AdbErrors.Classify(r.Output);
        if (kind != AdbErrors.None || !r.Output.Contains("Success", StringComparison.Ordinal))
            return new LauncherInstallResult(false, kind == AdbErrors.None ? AdbErrors.Other : kind, FirstLine(r.Output));

        // the old process was killed by the install; bring the panel back (not fatal if it fails)
        await adb.RunAsync(serial, WizardCommandBuilder.Start(), ct: ct).ConfigureAwait(false);
        var info = await adb.RunAsync(serial, WizardCommandBuilder.PackageInfoQuery(), ct: ct).ConfigureAwait(false);
        return new LauncherInstallResult(true, InstalledVersion: WizardCommandBuilder.ParseVersionName(info.StdOut));
    }

    private static string FirstLine(string s)
    {
        foreach (var line in s.Split('\n'))
        {
            string t = line.Trim();
            // «Performing Streamed Install» precedes the actual failure line
            if (t.Length > 0 && !t.StartsWith("Performing", StringComparison.Ordinal)) return t;
        }
        return s.Trim();
    }
}

/// <summary>Kinds of adb failures that the editor explains in plain words (<c>AdbErr_&lt;kind&gt;</c>).</summary>
public static class AdbErrors
{
    public const string None = "";
    public const string SignatureMismatch = "signature";
    public const string Downgrade = "downgrade";
    public const string Storage = "storage";
    public const string UserRestricted = "userRestricted";
    public const string Aborted = "aborted";
    public const string Unauthorized = "unauthorized";
    public const string Offline = "offline";
    public const string NotFound = "notFound";
    public const string Timeout = "timeout";
    public const string PermissionDenied = "permission";
    public const string Other = "other";

    /// <summary>Output of an adb command → one of the kinds above; <see cref="None"/> when it does not look like an error.</summary>
    public static string Classify(string? output)
    {
        string o = output ?? "";
        if (o.Contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE", StringComparison.Ordinal)
            || o.Contains("INSTALL_PARSE_FAILED_INCONSISTENT_CERTIFICATES", StringComparison.Ordinal)
            || o.Contains("signatures do not match", StringComparison.OrdinalIgnoreCase))
            return SignatureMismatch;
        if (o.Contains("INSTALL_FAILED_VERSION_DOWNGRADE", StringComparison.Ordinal)) return Downgrade;
        if (o.Contains("INSTALL_FAILED_INSUFFICIENT_STORAGE", StringComparison.Ordinal)) return Storage;
        if (o.Contains("INSTALL_FAILED_USER_RESTRICTED", StringComparison.Ordinal)) return UserRestricted;
        if (o.Contains("INSTALL_FAILED_ABORTED", StringComparison.Ordinal) || o.Contains("INSTALL_CANCELED_BY_USER", StringComparison.Ordinal))
            return Aborted;
        if (o.Contains("unauthorized", StringComparison.OrdinalIgnoreCase)) return Unauthorized;
        if (o.Contains("device offline", StringComparison.OrdinalIgnoreCase)) return Offline;
        if (Regex.IsMatch(o, @"device '[^']*' not found|no devices/emulators found", RegexOptions.IgnoreCase)) return NotFound;
        if (o.Contains("timed out", StringComparison.OrdinalIgnoreCase) || o.Contains("timeout", StringComparison.OrdinalIgnoreCase))
            return Timeout;
        if (WizardCommandBuilder.IsPermissionDenied(o)) return PermissionDenied;
        return WizardCommandBuilder.LooksLikeError(o) || o.StartsWith("adb: ", StringComparison.Ordinal) ? Other : None;
    }
}
