using System.Text.RegularExpressions;

namespace Ven4Docking.Core.Adb;

/// <summary>What the wizard learned from <c>getprop</c>.</summary>
public sealed record DeviceInfo(string Manufacturer, string Brand, string Model, string AndroidRelease, int Sdk)
{
    public string AndroidLabel => string.IsNullOrEmpty(AndroidRelease) ? $"API {Sdk}" : $"{AndroidRelease} (API {Sdk})";
}

public static class AndroidSdk
{
    public const int Android7 = 24;
    public const int Android8 = 26;
    public const int Android9 = 28;
    public const int Android10 = 29;
    public const int Android11 = 30;
    public const int Android13 = 33;
    public const int Android14 = 34;
}

public enum MatchKind
{
    /// <summary>Trimmed output equals <see cref="WizardCheck.Expected"/>.</summary>
    Equals,
    /// <summary>Output contains <see cref="WizardCheck.Expected"/>.</summary>
    Contains,
}

/// <summary>Arguments after <c>adb -s &lt;serial&gt;</c>.</summary>
public sealed record AdbCommand(IReadOnlyList<string> Args)
{
    public static AdbCommand Shell(params string[] args) => new(new[] { "shell" }.Concat(args).ToArray());
    public override string ToString() => string.Join(' ', Args);
}

public sealed record WizardCheck(AdbCommand Command, string Expected, MatchKind Kind = MatchKind.Equals)
{
    public bool IsOk(string output) => Kind switch
    {
        MatchKind.Equals => string.Equals(output.Trim(), Expected, StringComparison.OrdinalIgnoreCase),
        _ => output.Contains(Expected, StringComparison.OrdinalIgnoreCase),
    };
}

/// <summary>
/// One setup item (spec §9 step 3): commands to run and checks that prove the result.
/// <see cref="ManualOnly"/> = no adb command exists for this Android version; the UI shows <see cref="ManualHintKey"/>.
/// </summary>
public sealed record WizardStep(string Id, IReadOnlyList<AdbCommand> Commands, IReadOnlyList<WizardCheck> Checks,
    bool ManualOnly = false, string? ManualHintKey = null, bool Optional = false);

/// <summary>
/// Builds the adb commands of the phone wizard for a given Android version. Version rules come from
/// docs/research/phone-brands.md §0.3: RUN_ANY_IN_BACKGROUND and set-standby-bucket only on 9+,
/// locksettings only on 8+, HOME role on 10+, stay_on mask 15 (incl. dock) on 13+; REQUEST_INSTALL_PACKAGES app-op on 8+.
/// </summary>
public static class WizardCommandBuilder
{
    public const string Package = "ru.ven4tools.docking";
    public const string Component = Package + "/.MainActivity";
    public const int ScreenTimeoutMs = 30 * 60 * 1000;
    public const string AnimationScale = "0.5";
    public const string InstallPackagesOp = "REQUEST_INSTALL_PACKAGES";

    public static string BackgroundOp(int sdk) => sdk >= AndroidSdk.Android9 ? "RUN_ANY_IN_BACKGROUND" : "RUN_IN_BACKGROUND";

    public static int StayOnMask(int sdk) => sdk >= AndroidSdk.Android13 ? 15 : 7;

    public static bool SupportsWifiPairing(int sdk) => sdk >= AndroidSdk.Android11;

    public static AdbCommand Install(string apkPath) => new(new[] { "install", "-r", apkPath });

    public static IReadOnlyList<string> GetpropKeys { get; } = new[]
    {
        "ro.product.manufacturer", "ro.product.brand", "ro.product.model", "ro.build.version.release", "ro.build.version.sdk",
    };

    /// <summary>The setup items after the APK is installed, in UI order.</summary>
    public static IReadOnlyList<WizardStep> BuildSetup(int sdk, string? pcTimeZoneIana)
    {
        var steps = new List<WizardStep>();

        // Home screen (launcher): set-home-activity exists since 7.0; HOME role since 10.
        var home = new List<AdbCommand> { AdbCommand.Shell("cmd", "package", "set-home-activity", Component) };
        if (sdk >= AndroidSdk.Android10)
            home.Add(AdbCommand.Shell("cmd", "role", "add-role-holder", "android.app.role.HOME", Package));
        steps.Add(new WizardStep("home", home, new[]
        {
            new WizardCheck(AdbCommand.Shell("cmd", "package", "resolve-activity", "--brief",
                "-a", "android.intent.action.MAIN", "-c", "android.intent.category.HOME"), Package, MatchKind.Contains),
        }));

        // Screen stays on while charging.
        string mask = StayOnMask(sdk).ToString();
        steps.Add(new WizardStep("stayOn",
            new[] { AdbCommand.Shell("settings", "put", "global", "stay_on_while_plugged_in", mask) },
            new[] { new WizardCheck(AdbCommand.Shell("settings", "get", "global", "stay_on_while_plugged_in"), mask) }));

        // Screen timeout 30 min.
        string timeout = ScreenTimeoutMs.ToString();
        steps.Add(new WizardStep("timeout",
            new[] { AdbCommand.Shell("settings", "put", "system", "screen_off_timeout", timeout) },
            new[] { new WizardCheck(AdbCommand.Shell("settings", "get", "system", "screen_off_timeout"), timeout) }));

        // No lock screen: locksettings appeared in 8.0; on 7 it must be done by hand.
        if (sdk >= AndroidSdk.Android8)
            steps.Add(new WizardStep("noLock",
                new[] { AdbCommand.Shell("locksettings", "set-disabled", "true") },
                new[] { new WizardCheck(AdbCommand.Shell("locksettings", "get-disabled"), "true") }));
        else
            steps.Add(new WizardStep("noLock", Array.Empty<AdbCommand>(), Array.Empty<WizardCheck>(),
                ManualOnly: true, ManualHintKey: "Wizard_Manual_NoLock"));

        // Background work: battery optimisation whitelist + app-op + standby bucket.
        string op = BackgroundOp(sdk);
        var bg = new List<AdbCommand>
        {
            AdbCommand.Shell("dumpsys", "deviceidle", "whitelist", "+" + Package),
            AdbCommand.Shell("cmd", "appops", "set", Package, op, "allow"),
        };
        var bgChecks = new List<WizardCheck>
        {
            new(AdbCommand.Shell("dumpsys", "deviceidle", "whitelist"), Package, MatchKind.Contains),
            new(AdbCommand.Shell("cmd", "appops", "get", Package, op), "allow", MatchKind.Contains),
        };
        if (sdk >= AndroidSdk.Android9)
        {
            bg.Add(AdbCommand.Shell("am", "set-standby-bucket", Package, "active"));
            bgChecks.Add(new WizardCheck(AdbCommand.Shell("am", "get-standby-bucket", Package), "10"));
        }
        steps.Add(new WizardStep("background", bg, bgChecks));

        // Launcher updates through the PC (spec 1.1 §1.2): PackageInstaller needs «install unknown apps» for
        // Ven4Docking, an app-op since 8.0. Without it the update still works after the user allows it on the phone.
        if (sdk >= AndroidSdk.Android8)
            steps.Add(new WizardStep("installPackages",
                new[] { AdbCommand.Shell("cmd", "appops", "set", Package, InstallPackagesOp, "allow") },
                new[] { new WizardCheck(AdbCommand.Shell("cmd", "appops", "get", Package, InstallPackagesOp), "allow", MatchKind.Contains) },
                ManualHintKey: "Wizard_Manual_InstallPackages", Optional: true));

        // Time zone like the PC. IAlarmManager.setTimeZone is transaction 3; if the shell may not call it the
        // check fails and the UI asks to set the zone by hand.
        if (!string.IsNullOrEmpty(pcTimeZoneIana))
            steps.Add(new WizardStep("timeZone",
                new[] { AdbCommand.Shell("service", "call", "alarm", "3", "s16", pcTimeZoneIana) },
                new[] { new WizardCheck(AdbCommand.Shell("getprop", "persist.sys.timezone"), pcTimeZoneIana) },
                ManualHintKey: "Wizard_Manual_TimeZone", Optional: true));

        // Animations 0.5.
        string[] scales = { "window_animation_scale", "transition_animation_scale", "animator_duration_scale" };
        steps.Add(new WizardStep("animations",
            scales.Select(s => AdbCommand.Shell("settings", "put", "global", s, AnimationScale)).ToList(),
            scales.Select(s => new WizardCheck(AdbCommand.Shell("settings", "get", "global", s), AnimationScale)).ToList(),
            Optional: true));

        return steps;
    }

    /// <summary>Hands the agent address and the token to the launcher (contract with ПР-2).</summary>
    public static AdbCommand Launch(string url, string token) =>
        AdbCommand.Shell("am", "start", "-n", Component, "--es", "url", url, "--es", "token", token);

    public static AdbCommand WebViewQuery() => AdbCommand.Shell("dumpsys", "webviewupdate");

    /// <summary>
    /// Fallback for Android 7.x, where <c>dumpsys webviewupdate</c> prints nothing (checked on API 24): versionName of
    /// the WebView providers in the order Android 7–9 prefers them (Chrome, then the WebView packages).
    /// Output lines: <c>&lt;package&gt; versionName=&lt;version&gt;</c>.
    /// </summary>
    public static AdbCommand WebViewPackagesQuery() => AdbCommand.Shell(
        "for p in com.android.chrome com.google.android.webview com.android.webview; do " +
        "v=$(dumpsys package $p | grep -m 1 versionName=); [ -n \"$v\" ] && echo \"$p $v\"; done");

    private static readonly Regex WebViewPackageLine =
        new(@"^\s*(?<p>[\w.]+)\s+versionName=(?<v>\S+)", RegexOptions.Compiled | RegexOptions.Multiline);

    /// <summary>Parses <see cref="WebViewPackagesQuery"/> output → the first (preferred) provider found.</summary>
    public static (string Package, string Version)? ParseWebViewPackages(string output)
    {
        var m = WebViewPackageLine.Match(output);
        return m.Success ? (m.Groups["p"].Value, m.Groups["v"].Value) : null;
    }

    public static AdbCommand PackageInstalledCheck() => AdbCommand.Shell("pm", "path", Package);

    /// <summary>The settings/appops write was refused by the OEM shell (realme/OPPO, Xiaomi).</summary>
    public static bool IsPermissionDenied(string output) =>
        output.Contains("SecurityException", StringComparison.OrdinalIgnoreCase)
        || output.Contains("Permission denial", StringComparison.OrdinalIgnoreCase)
        || output.Contains("WRITE_SECURE_SETTINGS", StringComparison.OrdinalIgnoreCase);

    public static bool LooksLikeError(string output) =>
        IsPermissionDenied(output)
        || output.Contains("Exception", StringComparison.Ordinal)
        || output.StartsWith("Error", StringComparison.OrdinalIgnoreCase)
        || output.Contains("Failure [", StringComparison.Ordinal)
        || output.Contains("Unknown command", StringComparison.OrdinalIgnoreCase);

    private static readonly Regex WebViewCurrent =
        new(@"Current WebView package \(name, version\):\s*\(([^,]+),\s*([^)]+)\)", RegexOptions.Compiled);

    /// <summary>Parses <c>dumpsys webviewupdate</c> → (package, version) of the current WebView.</summary>
    public static (string Package, string Version)? ParseWebView(string dumpsys)
    {
        var m = WebViewCurrent.Match(dumpsys);
        return m.Success ? (m.Groups[1].Value.Trim(), m.Groups[2].Value.Trim()) : null;
    }

    public static int? MajorVersion(string? version)
    {
        if (string.IsNullOrEmpty(version)) return null;
        int dot = version.IndexOf('.');
        return int.TryParse(dot < 0 ? version : version[..dot], out int major) ? major : null;
    }

    /// <summary>Below Chrome 57 the panel uses the absolute-position fallback (spec §8).</summary>
    public const int FullPanelMinWebView = 57;

    /// <summary>Parses the output of <c>getprop</c> (all props, "[key]: [value]" lines).</summary>
    public static IReadOnlyDictionary<string, string> ParseGetprop(string output)
    {
        var map = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (Match m in Regex.Matches(output, @"^\[(?<k>[^\]]+)\]:\s*\[(?<v>[^\]]*)\]", RegexOptions.Multiline))
            map[m.Groups["k"].Value] = m.Groups["v"].Value;
        return map;
    }

    public static DeviceInfo? ToDeviceInfo(IReadOnlyDictionary<string, string> props)
    {
        if (!props.TryGetValue("ro.build.version.sdk", out var sdkText) || !int.TryParse(sdkText, out int sdk)) return null;
        string Get(string k) => props.TryGetValue(k, out var v) ? v : "";
        return new DeviceInfo(Get("ro.product.manufacturer"), Get("ro.product.brand"), Get("ro.product.model"),
            Get("ro.build.version.release"), sdk);
    }
}
