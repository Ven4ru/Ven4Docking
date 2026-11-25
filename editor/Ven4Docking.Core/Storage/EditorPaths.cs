namespace Ven4Docking.Core.Storage;

/// <summary>
/// Where user data lives (spec §2). <c>VEN4DOCKING_HOME</c> overrides the roaming folder; when it is set,
/// machine-local data (platform-tools, WebView2 cache) also goes inside it, so a dev/test run never touches
/// the real %APPDATA% / %LOCALAPPDATA% folders.
/// </summary>
public sealed class EditorPaths
{
    public const string HomeVariable = "VEN4DOCKING_HOME";

    public EditorPaths(string home, string localData)
    {
        Home = home;
        LocalData = localData;
    }

    /// <summary>%APPDATA%\Ven4Docking or VEN4DOCKING_HOME: config.json, layouts, icons, admin.key.</summary>
    public string Home { get; }

    /// <summary>%LOCALAPPDATA%\Ven4Docking or VEN4DOCKING_HOME\local: platform-tools, WebView2 data.</summary>
    public string LocalData { get; }

    public bool IsOverridden { get; init; }

    public string ConfigFile => Path.Combine(Home, "config.json");
    public string LayoutsDir => Path.Combine(Home, "layouts");
    public string IconsDir => Path.Combine(Home, "icons");
    public string AdminKeyFile => Path.Combine(Home, "admin.key");
    public string EditorSettingsFile => Path.Combine(Home, "editor.json");
    public string PlatformToolsDir => Path.Combine(LocalData, "platform-tools");
    public string AdbExe => Path.Combine(PlatformToolsDir, "adb.exe");
    public string WebViewDataDir => Path.Combine(LocalData, "WebView2");

    public string LayoutFile(string phoneId) => Path.Combine(LayoutsDir, SafeFileName(phoneId) + ".json");

    public static EditorPaths FromEnvironment(Func<string, string?>? getEnv = null)
    {
        getEnv ??= Environment.GetEnvironmentVariable;
        string? overrideHome = getEnv(HomeVariable);
        if (!string.IsNullOrWhiteSpace(overrideHome))
        {
            string home = Path.GetFullPath(overrideHome);
            return new EditorPaths(home, Path.Combine(home, "local")) { IsOverridden = true };
        }
        return new EditorPaths(
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Ven4Docking"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Ven4Docking"));
    }

    /// <summary>Phone ids come from the agent; never let them escape the layouts folder.</summary>
    public static string SafeFileName(string id)
    {
        var chars = id.Select(c => char.IsLetterOrDigit(c) || c is '-' or '_' ? c : '_').ToArray();
        string s = new(chars);
        return string.IsNullOrEmpty(s) ? "_" : s;
    }
}
