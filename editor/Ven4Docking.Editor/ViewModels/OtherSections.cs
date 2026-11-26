using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Windows;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Models;
using Ven4Docking.Core.Net;
using Ven4Docking.Core.Security;
using Ven4Docking.Editor.Services;
using Ven4Docking.Editor.Themes;

namespace Ven4Docking.Editor.ViewModels;

// ======================= Серверы =======================

public sealed partial class ServerItem : ObservableObject
{
    private readonly ServersViewModel _owner;

    public ServerItem(ServersViewModel owner, ServerEntry model)
    {
        _owner = owner;
        Model = model;
    }

    public ServerEntry Model { get; }

    private void Edit(Action change, [System.Runtime.CompilerServices.CallerMemberName] string? prop = null)
    {
        _owner.Session.Checkpoint();
        change();
        OnPropertyChanged(prop);
        OnPropertyChanged(nameof(Summary));
    }

    public string Name { get => Model.Name; set => Edit(() => Model.Name = value); }
    public string Host { get => Model.Host; set => Edit(() => Model.Host = value.Trim()); }
    public int Port { get => Model.Port; set => Edit(() => Model.Port = Math.Clamp(value, 1, 65535)); }
    public string User { get => Model.User; set => Edit(() => Model.User = value.Trim()); }
    public string Key { get => Model.Key; set => Edit(() => Model.Key = string.IsNullOrWhiteSpace(value) ? "auto" : value.Trim()); }
    public string Summary => $"{Model.User}@{Model.Host}:{Model.Port}";

    [ObservableProperty] private string _testResult = "";
    [ObservableProperty] private bool? _testOk;
}

/// <summary>«Серверы»: list, add, «Проверить подключение» with plain-language errors.</summary>
public sealed partial class ServersViewModel : SectionBase
{
    public ServersViewModel(EditorSession session) : base(session) { }

    public ObservableCollection<ServerItem> Servers { get; } = new();
    [ObservableProperty] private ServerItem? _selected;
    [ObservableProperty] private bool _testing;

    public bool HasServers => Servers.Count > 0;

    public override void Rebuild()
    {
        string? keep = Selected?.Model.Id;
        Servers.Clear();
        foreach (var s in Session.Config.Servers) Servers.Add(new ServerItem(this, s));
        Selected = Servers.FirstOrDefault(s => s.Model.Id == keep) ?? Servers.FirstOrDefault();
        OnPropertyChanged(nameof(HasServers));
    }

    [RelayCommand]
    private void Add()
    {
        Session.Checkpoint();
        var used = Session.Config.Servers.Select(s => s.Id).ToHashSet();
        int n = 1;
        while (used.Contains("srv" + n)) n++;
        var entry = new ServerEntry { Id = "srv" + n, Name = Loc.F("Servers_NewName", n), Host = "", Port = 22, User = "root", Key = "auto" };
        Session.Config.Servers.Add(entry);
        var item = new ServerItem(this, entry);
        Servers.Add(item);
        Selected = item;
        OnPropertyChanged(nameof(HasServers));
    }

    [RelayCommand]
    private void Delete()
    {
        if (Selected is null) return;
        Session.Checkpoint();
        Session.Config.Servers.Remove(Selected.Model);
        Servers.Remove(Selected);
        Selected = Servers.FirstOrDefault();
        OnPropertyChanged(nameof(HasServers));
    }

    [RelayCommand]
    private async Task Test()
    {
        var s = Selected;
        if (s is null) return;
        if (!Session.AgentOnline)
        {
            s.TestOk = false;
            s.TestResult = Loc.T("Agent_NeededForThis");
            return;
        }
        Testing = true;
        s.TestResult = Loc.T("Servers_Testing");
        s.TestOk = null;
        try
        {
            var r = await Session.Admin.TestServerAsync(s.Model);
            s.TestOk = r.Ok;
            s.TestResult = r.Ok
                ? Loc.F("Servers_Ok", r.Ping is { } p ? $"{p:0} ms" : "—")
                : Loc.T("ServerErr_" + ServerErrors.Classify(r.Err)) + (string.IsNullOrEmpty(r.Err) ? "" : $" ({r.Err})");
        }
        catch (AdminException e)
        {
            s.TestOk = false;
            s.TestResult = e.AgentUnreachable ? Loc.T("Agent_NeededForThis") : e.Message;
        }
        finally { Testing = false; }
    }
}

// ======================= Нейросети =======================

public sealed partial class AiItem : ObservableObject
{
    private readonly AiViewModel _owner;

    public AiItem(AiViewModel owner, string id, string name, bool? installed, double? lastData, bool enabled)
    {
        _owner = owner;
        Id = id;
        Name = name;
        Installed = installed;
        LastData = lastData;
        _enabled = enabled;
    }

    public string Id { get; }
    public string Name { get; }
    public bool? Installed { get; }
    public double? LastData { get; }

    public string InstalledText => Installed switch
    {
        true => Loc.T("Ai_Installed"),
        false => Loc.T("Ai_NotInstalled"),
        _ => Loc.T("Ai_Unknown"),
    };

    public string LastDataText => LastData is { } t
        ? Loc.F("Ai_LastData", DateTimeOffset.FromUnixTimeMilliseconds((long)(t * 1000)).ToLocalTime().ToString("g"))
        : Loc.T("Ai_NoData");

    [ObservableProperty] private bool _enabled;

    partial void OnEnabledChanged(bool value) => _owner.SetEnabled(Id, value);
}

/// <summary>«Нейросети»: sources found by the agent (files only, spec §6), last data time, on/off.</summary>
public sealed partial class AiViewModel : SectionBase
{
    private bool _building;

    public AiViewModel(EditorSession session) : base(session) { }

    public ObservableCollection<AiItem> Sources { get; } = new();
    [ObservableProperty] private string _message = "";

    public override void Rebuild() => _ = LoadAsync();

    public override void Activated() => _ = LoadAsync();

    [RelayCommand]
    private async Task LoadAsync()
    {
        IReadOnlyList<AiSourceInfo>? fromAgent = null;
        if (Session.AgentOnline)
        {
            try { fromAgent = await Session.Admin.GetAiSourcesAsync(); }
            catch (AdminException) { }
        }
        var enabled = Session.Config.Ai?.Enabled ?? AiSettings.KnownSources.ToList();
        _building = true;
        Sources.Clear();
        if (fromAgent is { Count: > 0 })
        {
            foreach (var s in fromAgent)
                Sources.Add(new AiItem(this, s.Id, s.Name, s.Installed, s.LastData, enabled.Contains(s.Id)));
        }
        else
        {
            foreach (var id in AiSettings.KnownSources)
                Sources.Add(new AiItem(this, id, Loc.T("Ai_" + id), null, null, enabled.Contains(id)));
        }
        _building = false;
        Message = fromAgent is null ? Loc.T("Ai_OfflineNote") : "";
    }

    public void SetEnabled(string id, bool on)
    {
        if (_building) return;
        Session.Checkpoint();
        var ai = Session.Config.Ai ??= new AiSettings { Enabled = AiSettings.KnownSources.ToList() };
        ai.Enabled.Remove(id);
        if (on) ai.Enabled.Add(id);
    }
}

// ======================= Настройки =======================

public sealed record Choice(string Id, string Title);

/// <summary>«Настройки»: theme, language, weather city (open-meteo geocoding), agent autostart, port, OBS, Wake on LAN.</summary>
public sealed partial class SettingsViewModel : SectionBase
{
    public const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    public const string RunValue = "Ven4Docking";
    private bool _building;

    public SettingsViewModel(EditorSession session) : base(session) { }

    public IReadOnlyList<Choice> Themes => ThemeManager.All.Select(t => new Choice(t, Loc.T("Theme_" + t))).ToList();
    public IReadOnlyList<Choice> Languages { get; } = new[] { new Choice("ru", "Русский"), new Choice("en", "English") };

    [ObservableProperty] private Choice? _theme;
    [ObservableProperty] private Choice? _language;
    [ObservableProperty] private string _port = "";
    [ObservableProperty] private string _cityQuery = "";
    [ObservableProperty] private bool _searching;
    [ObservableProperty] private string _message = "";
    [ObservableProperty] private bool _autostart;
    public ObservableCollection<GeoPlace> CityResults { get; } = new();

    public string WeatherText => Session.Config.Weather is { } w ? $"{w.Name} ({w.Lat:0.##}, {w.Lon:0.##})" : Loc.T("Settings_NoCity");
    public string DataFolder => Session.Paths.Home;
    public string? AgentExe => MainViewModel.FindAgentExe();
    public bool CanAutostart => AgentExe is not null;

    public override void Rebuild()
    {
        _building = true;
        OnPropertyChanged(nameof(Themes));
        Theme = Themes.FirstOrDefault(t => t.Id == (Session.Config.Theme ?? ThemeManager.Teal)) ?? Themes[0];
        Language = Languages.FirstOrDefault(l => l.Id == (Session.Config.Language ?? "ru")) ?? Languages[0];
        Port = Session.Config.EffectivePort.ToString();
        Autostart = ReadAutostart();
        OnPropertyChanged(nameof(WeatherText));
        ObsHost = Session.Config.Obs?.Host ?? ObsSettings.DefaultHost;
        ObsPort = (Session.Config.Obs?.Port ?? ObsSettings.DefaultPort).ToString();
        OnPropertyChanged(nameof(ObsPasswordSet));
        ObsTestResult = "";
        ObsTestOk = null;
        FillWol();
        _building = false;
    }

    public override void Activated() => _ = LoadWolAsync();

    partial void OnThemeChanged(Choice? value)
    {
        if (_building || value is null || value.Id == Session.Config.Theme) return;
        Session.Checkpoint();
        Session.Config.Theme = value.Id;
        ThemeManager.Apply(value.Id);
    }

    partial void OnLanguageChanged(Choice? value)
    {
        if (_building || value is null || value.Id == (Session.Config.Language ?? "ru")) return;
        Session.Checkpoint();
        Session.Config.Language = value.Id;
        Loc.Instance.SetLanguage(value.Id);
    }

    partial void OnPortChanged(string value)
    {
        if (_building) return;
        if (!int.TryParse(value, out int p) || p is < 1024 or > 65535)
        {
            Message = Loc.T("Settings_BadPort");
            return;
        }
        if (p == Session.Config.EffectivePort) return;
        Session.Checkpoint();
        Session.Config.Port = p;
        Message = Loc.T("Settings_PortRestart");
    }

    [RelayCommand]
    private async Task SearchCity()
    {
        Searching = true;
        CityResults.Clear();
        try
        {
            var list = await new GeocodingClient(Session.Http).SearchAsync(CityQuery, Loc.Instance.Language);
            foreach (var p in list) CityResults.Add(p);
            Message = list.Count == 0 ? Loc.T("Settings_CityNotFound") : "";
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException or System.Text.Json.JsonException)
        {
            Message = Loc.F("Settings_CityError", e.Message);
        }
        finally { Searching = false; }
    }

    [RelayCommand]
    private void PickCity(GeoPlace? place)
    {
        if (place is null) return;
        Session.Checkpoint();
        Session.Config.Weather = place.ToWeather();
        CityResults.Clear();
        OnPropertyChanged(nameof(WeatherText));
    }

    private static bool ReadAutostart()
    {
        using var k = Registry.CurrentUser.OpenSubKey(RunKey);
        return k?.GetValue(RunValue) is string;
    }

    /// <summary>The autostart entry is a user setting outside config.json: applied immediately (HKCU\…\Run).</summary>
    partial void OnAutostartChanged(bool value)
    {
        if (_building) return;
        string? exe = AgentExe;
        if (value && exe is null)
        {
            Message = Loc.T("Agent_NotFound");
            _building = true;
            Autostart = false;
            _building = false;
            return;
        }
        using var k = Registry.CurrentUser.CreateSubKey(RunKey);
        if (value) k.SetValue(RunValue, $"\"{exe}\"");
        else if (k.GetValue(RunValue) is not null) k.DeleteValue(RunValue);
        Message = value ? Loc.T("Settings_AutostartOn") : Loc.T("Settings_AutostartOff");
    }

    [RelayCommand]
    private void OpenDataFolder()
    {
        Directory.CreateDirectory(Session.Paths.Home);
        Process.Start(new ProcessStartInfo("explorer.exe", $"\"{Session.Paths.Home}\"") { UseShellExecute = false });
    }

    // ---------- OBS (spec 1.1 §9): obs-websocket address, password encrypted with DPAPI ----------

    [ObservableProperty] private string _obsHost = ObsSettings.DefaultHost;
    [ObservableProperty] private string _obsPort = ObsSettings.DefaultPort.ToString();
    [ObservableProperty] private string _obsTestResult = "";
    [ObservableProperty] private bool? _obsTestOk;
    [ObservableProperty] private bool _obsTesting;

    public bool ObsPasswordSet => !string.IsNullOrEmpty(Session.Config.Obs?.PasswordDpapi);

    private ObsSettings EditObs()
    {
        Session.Checkpoint();
        return Session.Config.Obs ??= new ObsSettings();
    }

    partial void OnObsHostChanged(string value)
    {
        if (_building) return;
        string host = string.IsNullOrWhiteSpace(value) ? ObsSettings.DefaultHost : value.Trim();
        if (!ObsSettings.IsAllowedHost(host))
        {
            SayObs(Loc.T("Settings_ObsBadHost"));
            return;
        }
        SayObs("");
        if (host == Session.Config.Obs?.Host) return;
        EditObs().Host = host;
    }

    partial void OnObsPortChanged(string value)
    {
        if (_building) return;
        if (!int.TryParse(value, out int p) || p is < 1 or > 65535)
        {
            SayObs(Loc.T("Settings_ObsBadPort"));
            return;
        }
        SayObs("");
        if (p == Session.Config.Obs?.Port) return;
        EditObs().Port = p;
    }

    /// <summary>Input errors go next to «Проверить подключение», inside the OBS card.</summary>
    private void SayObs(string error)
    {
        ObsTestOk = error.Length > 0 ? false : null;
        ObsTestResult = error;
    }

    /// <summary>
    /// From the password box (on leaving it): only the DPAPI ciphertext is kept, the box is cleared by the view.
    /// Empty input keeps the stored password.
    /// </summary>
    public void SetObsPassword(string plain)
    {
        if (string.IsNullOrEmpty(plain)) return;
        EditObs().PasswordDpapi = Dpapi.Protect(plain);
        OnPropertyChanged(nameof(ObsPasswordSet));
        ObsTestResult = "";
        ObsTestOk = null;
    }

    [RelayCommand]
    private async Task TestObs()
    {
        if (!Session.AgentOnline)
        {
            ObsTestOk = false;
            ObsTestResult = Loc.T("Agent_NeededForThis");
            return;
        }
        string host = string.IsNullOrWhiteSpace(ObsHost) ? ObsSettings.DefaultHost : ObsHost.Trim();
        if (!ObsSettings.IsAllowedHost(host))
        {
            ObsTestOk = false;
            ObsTestResult = Loc.T("Settings_ObsBadHost");
            return;
        }
        if (!int.TryParse(ObsPort, out int port) || port is < 1 or > 65535) port = ObsSettings.DefaultPort;
        ObsTesting = true;
        ObsTestOk = null;
        ObsTestResult = Loc.T("Settings_ObsTesting");
        try
        {
            var r = await Session.Admin.TestObsAsync(host, port, Session.Config.Obs?.PasswordDpapi);
            ObsTestOk = r.Ok;
            ObsTestResult = r.Ok
                ? Loc.F("Settings_ObsOk", string.IsNullOrEmpty(r.Version) ? "—" : r.Version)
                : Loc.F("Settings_ObsFailed", string.IsNullOrEmpty(r.Error) ? "?" : r.Error);
        }
        catch (AdminException e)
        {
            ObsTestOk = false;
            ObsTestResult = e.AgentTooOld ? Loc.T("Agent_TooOld") : e.AgentUnreachable ? Loc.T("Agent_NeededForThis") : e.Message;
        }
        finally { ObsTesting = false; }
    }

    // ---------- Wake on LAN (spec 1.1 §9): MAC and broadcast from /admin/wol, can be corrected ----------

    private WolInfo? _wolInfo;

    [ObservableProperty] private string _wolMac = "";
    [ObservableProperty] private string _wolBroadcast = "";
    [ObservableProperty] private string _wolMessage = "";
    [ObservableProperty] private bool _wolMessageIsError;

    /// <summary>The address is set by hand in config.json (otherwise the agent takes it from the adapter).</summary>
    public bool WolManual => Session.Config.Wol is not null;

    public string WolAdapterText => _wolInfo?.Adapter is { Length: > 0 } a ? Loc.F("Settings_WolAdapter", a) : "";

    /// <summary>true → allowed in Windows, false → off, null → unknown (also when the agent is not running).</summary>
    public bool? WolWakeEnabled => _wolInfo?.WakeEnabled;

    public string WolWakeText => _wolInfo is null
        ? ""
        : _wolInfo.WakeEnabled switch
        {
            true => Loc.T("Settings_WolWakeOn"),
            false => Loc.T("Settings_WolWakeOff"),
            _ => Loc.T("Settings_WolWakeUnknown"),
        };

    private void FillWol()
    {
        bool was = _building;
        _building = true;
        WolMac = Session.Config.Wol?.Mac ?? _wolInfo?.Mac ?? "";
        WolBroadcast = Session.Config.Wol?.Broadcast ?? _wolInfo?.Broadcast ?? "";
        _building = was;
        foreach (var p in new[] { nameof(WolManual), nameof(WolAdapterText), nameof(WolWakeEnabled), nameof(WolWakeText) })
            OnPropertyChanged(p);
    }

    [RelayCommand]
    private async Task LoadWolAsync()
    {
        if (!Session.AgentOnline)
        {
            _wolInfo = null;
            SayWol(Loc.T("Settings_WolOffline"), false);
        }
        else
        {
            try
            {
                _wolInfo = await Session.Admin.GetWolAsync();
                SayWol(_wolInfo.Mac is null && Session.Config.Wol is null ? Loc.T("Settings_WolNotFound") : "", false);
            }
            catch (AdminException e)
            {
                _wolInfo = null;
                SayWol(e.AgentTooOld ? Loc.T("Agent_TooOld") : e.AgentUnreachable ? Loc.T("Settings_WolOffline") : e.Message, !e.AgentUnreachable);
            }
        }
        FillWol();
    }

    private void SayWol(string text, bool error)
    {
        WolMessage = text;
        WolMessageIsError = error;
    }

    partial void OnWolMacChanged(string value) => ApplyWol();

    partial void OnWolBroadcastChanged(string value) => ApplyWol();

    private void ApplyWol()
    {
        if (_building) return;
        var wol = WolSettings.Create(WolMac, WolBroadcast);
        if (wol is null)
        {
            SayWol(WolSettings.NormalizeMac(WolMac) is null ? Loc.T("Settings_WolBadMac") : Loc.T("Settings_WolBadBroadcast"), true);
            return;
        }
        var cur = Session.Config.Wol;
        bool sameAsAgent = cur is null && wol.Mac == WolSettings.NormalizeMac(_wolInfo?.Mac) && wol.Broadcast == WolSettings.NormalizeBroadcast(_wolInfo?.Broadcast);
        if (sameAsAgent || (cur is not null && cur.Mac == wol.Mac && cur.Broadcast == wol.Broadcast))
        {
            SayWol("", false);
            FillWol();
            return;
        }
        Session.Checkpoint();
        if (cur is null) Session.Config.Wol = wol;
        else (cur.Mac, cur.Broadcast) = (wol.Mac, wol.Broadcast);
        SayWol(Loc.T("Settings_WolSaved"), false);
        FillWol();
    }

    /// <summary>«Определять автоматически»: drop config.wol, the agent takes the adapter's address again.</summary>
    [RelayCommand]
    private void ResetWol()
    {
        if (Session.Config.Wol is null) return;
        Session.Checkpoint();
        Session.Config.Wol = null;
        SayWol("", false);
        FillWol();
    }
}

// ======================= О программе =======================

public sealed partial class AboutViewModel : SectionBase
{
    public AboutViewModel(EditorSession session) : base(session) { }

    public static Version AppVersion =>
        Assembly.GetExecutingAssembly().GetName().Version is { } v ? new Version(v.Major, v.Minor, Math.Max(v.Build, 0)) : new Version(1, 0, 0);

    public string VersionText => Loc.F("About_Version", AppVersion.ToString(3));

    [ObservableProperty] private string _updateText = "";
    [ObservableProperty] private bool _checking;
    [ObservableProperty] private string? _releaseUrl;

    /// <summary>The newer release found by the last check (null when there is none).</summary>
    [ObservableProperty] private UpdateResult? _available;
    [ObservableProperty] private bool _downloading;
    [ObservableProperty] private double _downloadPercent;
    [ObservableProperty] private bool _downloadSizeKnown;
    [ObservableProperty] private string _downloadText = "";
    [ObservableProperty] private string _installError = "";
    [ObservableProperty] private bool _bannerHidden;

    private CancellationTokenSource? _downloadCts;

    public string InstallText => Available?.LatestVersion is { } v ? Loc.F("Update_Install", v) : "";
    public bool CanInstall => Available is not null && !Downloading;

    /// <summary>The main window shows «Доступна версия X.Y.Z · Обновить» above the sections.</summary>
    public bool ShowBanner => Available is not null && !BannerHidden;
    public string BannerText => Available?.LatestVersion is { } v ? Loc.F("Update_Banner", v) : "";

    partial void OnAvailableChanged(UpdateResult? value) => NotifyInstall();
    partial void OnDownloadingChanged(bool value) => NotifyInstall();
    partial void OnBannerHiddenChanged(bool value) => NotifyInstall();

    private void NotifyInstall()
    {
        foreach (var p in new[] { nameof(InstallText), nameof(CanInstall), nameof(ShowBanner), nameof(BannerText) }) OnPropertyChanged(p);
        InstallUpdateCommand.NotifyCanExecuteChanged();
        CancelDownloadCommand.NotifyCanExecuteChanged();
    }

    public override void Rebuild()
    {
        OnPropertyChanged(nameof(VersionText));
        NotifyInstall();
    }

    [RelayCommand]
    private void HideBanner() => BannerHidden = true;

    [RelayCommand]
    private async Task CheckUpdates()
    {
        Checking = true;
        try
        {
            var r = await new UpdateChecker(Session.Http).CheckAsync(AppVersion);
            ReleaseUrl = r.State == UpdateState.Available ? r.Url : null;
            Available = r.State == UpdateState.Available ? r : null;
            InstallError = "";
            UpdateText = r.State switch
            {
                UpdateState.Available => Loc.F("Update_Available", r.LatestVersion),
                UpdateState.UpToDate => Loc.T("Update_UpToDate"),
                UpdateState.NoReleases => Loc.T("Update_NoReleases"),
                _ => Loc.F("Update_Failed", r.Error),
            };
        }
        finally { Checking = false; }
    }

    [RelayCommand]
    private void OpenRelease()
    {
        if (ReleaseUrl is not null) Process.Start(new ProcessStartInfo(ReleaseUrl) { UseShellExecute = true });
    }

    /// <summary>
    /// «Обновить до X.Y.Z»: unsaved edits first, then download + SHA-256 check, then the installer runs silently
    /// (<c>/S /RELAUNCH=1</c>: it closes the agent and the editor, installs and starts both again).
    /// </summary>
    [RelayCommand(CanExecute = nameof(CanInstall))]
    private async Task InstallUpdate()
    {
        var release = Available;
        if (release?.LatestVersion is not { } version) return;
        InstallError = "";

        if (Session.IsDirty)
        {
            var answer = MessageBox.Show(Loc.T("Update_AskSave"), "Ven4Docking", MessageBoxButton.YesNoCancel, MessageBoxImage.Question);
            if (answer == MessageBoxResult.Cancel) return;
            if (answer == MessageBoxResult.Yes)
            {
                SaveResult saved;
                try { saved = await Session.SaveAsync(); }
                catch (Exception e) when (e is IOException or UnauthorizedAccessException or AdminException)
                {
                    saved = new SaveResult(false, e.Message);
                }
                if (!saved.Ok)
                {
                    InstallError = Loc.F("Update_SaveFailed", saved.Error);
                    return;
                }
            }
        }

        Downloading = true;
        DownloadPercent = 0;
        DownloadSizeKnown = false;
        DownloadText = Loc.T("Update_Connecting");
        _downloadCts = new CancellationTokenSource();
        UpdateDownload result;
        try
        {
            var progress = new Progress<DownloadProgress>(p =>
            {
                DownloadSizeKnown = p.Total is > 0;
                DownloadPercent = p.Total is > 0 ? 100.0 * p.Received / p.Total.Value : 0;
                DownloadText = p.Total is > 0
                    ? Loc.F("Update_Downloading", Mb(p.Received), Mb(p.Total.Value))
                    : Loc.F("Update_DownloadingNoTotal", Mb(p.Received));
            });
            using var installer = new UpdateInstaller();
            result = await installer.DownloadAsync(version, release.Assets ?? Array.Empty<ReleaseAsset>(), progress, _downloadCts.Token);
        }
        finally
        {
            _downloadCts.Dispose();
            _downloadCts = null;
            Downloading = false;
        }

        if (!result.Ok)
        {
            DownloadText = "";
            InstallError = result.Error == UpdateDownloadError.Cancelled ? Loc.T("Update_Err_Cancelled") : Loc.F("Update_Err_" + result.Error, result.Detail);
            return;
        }

        DownloadText = Loc.T("Update_Starting");
        try
        {
            Process.Start(new ProcessStartInfo(result.SetupPath!, "/S /RELAUNCH=1") { UseShellExecute = true });
        }
        catch (Exception e) when (e is System.ComponentModel.Win32Exception or IOException)
        {
            DownloadText = "";
            InstallError = Loc.F("Update_Err_Start", e.Message);
            return;
        }
        // the installer restarts the editor; unsaved edits were saved or explicitly dropped above
        Session.ClosingForUpdate = true;
        Application.Current.Shutdown();
    }

    private static string Mb(long bytes) => (bytes / 1024.0 / 1024.0).ToString("0.0");

    private bool CanCancelDownload() => Downloading;

    [RelayCommand(CanExecute = nameof(CanCancelDownload))]
    private void CancelDownload() => _downloadCts?.Cancel();

    [RelayCommand]
    private void OpenVen4verse() => Process.Start(new ProcessStartInfo(WizardViewModel.Ven4verseUrl) { UseShellExecute = true });
}
