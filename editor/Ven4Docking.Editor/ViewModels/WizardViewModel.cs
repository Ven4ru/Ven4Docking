using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Windows;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using Ven4Docking.Core.Adb;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Brands;
using Ven4Docking.Core.Net;
using Ven4Docking.Editor.Services;

namespace Ven4Docking.Editor.ViewModels;

public sealed class BrandLine
{
    public required string Text { get; init; }
    public bool Unconfirmed { get; init; }
}

public enum SetupState { Pending, Running, Ok, Failed, Denied, Manual, Warning }

public sealed partial class SetupRow : ObservableObject
{
    public required string Id { get; init; }
    public required string Title { get; init; }
    public WizardStep? Step { get; init; }

    [ObservableProperty] private SetupState _state = SetupState.Pending;
    [ObservableProperty] private string _detail = "";

    public string StateGlyph => State switch
    {
        SetupState.Ok => "",
        SetupState.Running => "",
        SetupState.Failed or SetupState.Denied => "",
        SetupState.Manual or SetupState.Warning => "",
        _ => "",
    };

    partial void OnStateChanged(SetupState value) => OnPropertyChanged(nameof(StateGlyph));
}

/// <summary>
/// Phone connection wizard (spec §9): 1 method (USB / Wi-Fi pair), 2 brand + version → instructions,
/// 3 install APK and settings with a check of each, 4 token + url to the launcher, 5 WebView version,
/// 6 charge-limit tip, 7 done + Ven4verse.
/// </summary>
public sealed partial class WizardViewModel : ObservableObject
{
    public const int StepCount = 7;
    public const string Ven4verseUrl = "https://ven4tools.ru";

    private readonly EditorSession _session;
    private readonly Action _close;
    private AdbClient _adb;
    private BrandInfo? _brand;

    public WizardViewModel(EditorSession session, Action close)
    {
        _session = session;
        _close = close;
        _adb = AdbClient.FromEnvironment(session.Paths.AdbExe);
        _apkPath = FindApk() ?? "";
    }

    [ObservableProperty] private int _step = 1;
    [ObservableProperty] private bool _isUsb = true;
    [ObservableProperty] private bool _busy;
    [ObservableProperty] private double _downloadProgress;
    [ObservableProperty] private string _message = "";
    [ObservableProperty] private bool _messageIsError;
    [ObservableProperty] private string _log = "";

    // Wi-Fi
    [ObservableProperty] private string _pairHostPort = "";
    [ObservableProperty] private string _pairCode = "";
    [ObservableProperty] private string _connectHostPort = "";

    // device
    public ObservableCollection<AdbDevice> Devices { get; } = new();
    [ObservableProperty] private AdbDevice? _selectedDevice;
    [ObservableProperty] private DeviceInfo? _device;
    public ObservableCollection<BrandLine> DeveloperLines { get; } = new();
    public ObservableCollection<BrandLine> ToggleLines { get; } = new();
    public ObservableCollection<BrandLine> WifiLines { get; } = new();
    public ObservableCollection<BrandLine> KeepAliveLines { get; } = new();
    public ObservableCollection<BrandLine> PermissionLines { get; } = new();
    public ObservableCollection<BrandLine> ChargeLines { get; } = new();

    // setup
    [ObservableProperty] private string _apkPath;
    public ObservableCollection<SetupRow> SetupRows { get; } = new();
    [ObservableProperty] private bool _permissionDenied;
    [ObservableProperty] private bool _setupDone;

    // token
    [ObservableProperty] private string _phoneName = "";
    [ObservableProperty] private string _agentUrl = "";
    [ObservableProperty] private bool _tokenSent;

    // webview
    [ObservableProperty] private string _webViewText = "";
    [ObservableProperty] private bool _webViewOld;

    public bool IsWifi
    {
        get => !IsUsb;
        set => IsUsb = !value;
    }

    public bool AdbReady => _adb.IsAvailable;
    public string AdbPath => _adb.AdbPath;
    public string StepTitle => Loc.T($"Wizard_Step{Step}_Title");
    public string StepCounter => Loc.F("Wizard_StepCounter", Step, StepCount);
    public string BrandName => _brand is null ? "" : _brand.Id == "generic" ? Loc.T("Wizard_BrandGeneric") : _brand.Name;
    public string DeviceText => Device is null ? "" : $"{Device.Manufacturer} {Device.Model} · Android {Device.AndroidLabel}";
    public bool WifiPairingSupported => Device is null || WizardCommandBuilder.SupportsWifiPairing(Device.Sdk);

    public bool IsStep1 => Step == 1;
    public bool IsStep2 => Step == 2;
    public bool IsStep3 => Step == 3;
    public bool IsStep4 => Step == 4;
    public bool IsStep5 => Step == 5;
    public bool IsStep6 => Step == 6;
    public bool IsStep7 => Step == 7;

    partial void OnStepChanged(int value)
    {
        foreach (var p in new[] { nameof(StepTitle), nameof(StepCounter), nameof(IsStep1), nameof(IsStep2), nameof(IsStep3),
                     nameof(IsStep4), nameof(IsStep5), nameof(IsStep6), nameof(IsStep7) })
            OnPropertyChanged(p);
        BackCommand.NotifyCanExecuteChanged();
        NextCommand.NotifyCanExecuteChanged();
        Message = "";
        if (value == 5 && Device is not null && string.IsNullOrEmpty(WebViewText)) _ = CheckWebView();
    }

    partial void OnIsUsbChanged(bool value) => OnPropertyChanged(nameof(IsWifi));
    partial void OnBusyChanged(bool value) => NextCommand.NotifyCanExecuteChanged();
    partial void OnDeviceChanged(DeviceInfo? value)
    {
        OnPropertyChanged(nameof(DeviceText));
        OnPropertyChanged(nameof(WifiPairingSupported));
        NextCommand.NotifyCanExecuteChanged();
    }
    partial void OnSetupDoneChanged(bool value) => NextCommand.NotifyCanExecuteChanged();
    partial void OnTokenSentChanged(bool value) => NextCommand.NotifyCanExecuteChanged();

    public void RefreshTexts()
    {
        OnPropertyChanged(nameof(StepTitle));
        OnPropertyChanged(nameof(StepCounter));
        OnPropertyChanged(nameof(BrandName));
        if (_brand is not null) FillBrand(_brand);
    }

    public void Restart()
    {
        _adb = AdbClient.FromEnvironment(_session.Paths.AdbExe);
        Step = 1;
        Devices.Clear();
        SelectedDevice = null;
        Device = null;
        _brand = null;
        ManualBrand = _session.Brands.Generic;
        SetupRows.Clear();
        SetupDone = false;
        TokenSent = false;
        // a second phone must not inherit the previous phone's name (it became the name of the new panel)
        PhoneName = "";
        AgentUrl = "";
        PermissionDenied = false;
        WebViewText = "";
        WebViewOld = false;
        Log = "";
        Message = "";
        OnPropertyChanged(nameof(AdbReady));
        OnPropertyChanged(nameof(AdbPath));
        OnPropertyChanged(nameof(StepTitle));
        OnPropertyChanged(nameof(StepCounter));
    }

    private void Say(string text, bool error = false)
    {
        Message = text;
        MessageIsError = error;
    }

    private void AppendLog(string line)
    {
        var sb = new StringBuilder(Log);
        if (sb.Length > 20000) sb.Remove(0, sb.Length - 15000);
        sb.AppendLine(line);
        Log = sb.ToString();
    }

    // ---------- navigation ----------

    private bool CanBack() => Step > 1 && !Busy;

    [RelayCommand(CanExecute = nameof(CanBack))]
    private void Back() => Step--;

    private bool CanNext() => !Busy && Step switch
    {
        1 => true,
        2 => Device is not null,
        3 => SetupDone,
        4 => TokenSent,
        _ => Step < StepCount,
    };

    [RelayCommand(CanExecute = nameof(CanNext))]
    private async Task Next()
    {
        if (Step == 1)
        {
            if (!AdbReady && !await EnsureAdbAsync()) return;
            Step = 2;
            await RefreshDevices();
            return;
        }
        if (Step == 3 && string.IsNullOrEmpty(PhoneName)) PhoneName = Device?.Model ?? "";
        if (Step == 3 && string.IsNullOrEmpty(AgentUrl))
        {
            var ip = LanAddress.GetPreferred();
            AgentUrl = ip is null ? "" : LanAddress.AgentUrl(ip, _session.Port);
        }
        Step++;
    }

    [RelayCommand]
    private void Close() => _close();

    [RelayCommand]
    private void OpenVen4verse() => Process.Start(new ProcessStartInfo(Ven4verseUrl) { UseShellExecute = true });

    // ---------- step 1: adb ----------

    /// <summary>Asks for consent, then downloads platform-tools from dl.google.com (spec §9, §10).</summary>
    [RelayCommand]
    private async Task<bool> EnsureAdbAsync()
    {
        if (AdbReady) return true;
        var answer = MessageBox.Show(Loc.F("Adb_Consent_Text", _session.Paths.PlatformToolsDir), Loc.T("Adb_Consent_Title"),
            MessageBoxButton.YesNo, MessageBoxImage.Question, MessageBoxResult.No);
        if (answer != MessageBoxResult.Yes)
        {
            Say(Loc.T("Adb_Declined"), true);
            return false;
        }
        Busy = true;
        try
        {
            Say(Loc.T("Adb_Downloading"));
            var progress = new Progress<double>(p => DownloadProgress = p * 100);
            using var http = new HttpClient { Timeout = TimeSpan.FromMinutes(5) };
            await new PlatformToolsDownloader(http).DownloadAsync(_session.Paths.PlatformToolsDir, progress);
            _adb = AdbClient.FromEnvironment(_session.Paths.AdbExe);
            Say(Loc.T("Adb_Ready"));
            return true;
        }
        catch (Exception e) when (e is HttpRequestException or IOException or InvalidDataException or TaskCanceledException)
        {
            Say(Loc.F("Adb_DownloadFailed", e.Message), true);
            return false;
        }
        finally
        {
            Busy = false;
            OnPropertyChanged(nameof(AdbReady));
            OnPropertyChanged(nameof(AdbPath));
        }
    }

    [RelayCommand]
    private async Task Pair()
    {
        if (!await EnsureAdbAsync()) return;
        if (string.IsNullOrWhiteSpace(PairHostPort) || string.IsNullOrWhiteSpace(PairCode))
        {
            Say(Loc.T("Wifi_FillPair"), true);
            return;
        }
        Busy = true;
        try
        {
            var r = await _adb.PairAsync(PairHostPort.Trim(), PairCode.Trim());
            AppendLog("adb pair → " + r.Output.Trim());
            bool ok = r.Output.Contains("Successfully paired", StringComparison.OrdinalIgnoreCase);
            Say(ok ? Loc.T("Wifi_Paired") : Loc.F("Wifi_PairFailed", r.Output.Trim()), !ok);
            if (ok && string.IsNullOrWhiteSpace(ConnectHostPort))
                ConnectHostPort = PairHostPort.Split(':')[0] + ":";
        }
        catch (InvalidOperationException e) { Say(e.Message, true); }
        finally { Busy = false; }
    }

    [RelayCommand]
    private async Task Connect()
    {
        if (!await EnsureAdbAsync()) return;
        if (string.IsNullOrWhiteSpace(ConnectHostPort)) { Say(Loc.T("Wifi_FillConnect"), true); return; }
        Busy = true;
        try
        {
            var r = await _adb.ConnectAsync(ConnectHostPort.Trim());
            AppendLog("adb connect → " + r.Output.Trim());
            bool ok = r.Output.Contains("connected to", StringComparison.OrdinalIgnoreCase);
            Say(ok ? Loc.T("Wifi_Connected") : Loc.F("Wifi_ConnectFailed", r.Output.Trim()), !ok);
        }
        catch (InvalidOperationException e) { Say(e.Message, true); }
        finally { Busy = false; }
    }

    // ---------- step 2: device + brand ----------

    [RelayCommand]
    private async Task RefreshDevices()
    {
        if (!AdbReady) return;
        Busy = true;
        try
        {
            var list = await _adb.DevicesAsync();
            Devices.Clear();
            foreach (var d in list.Where(d => _adb.IsAllowed(d.Serial))) Devices.Add(d);
            if (Devices.Count == 0) Say(Loc.T("Wizard_NoDevices"), true);
            else if (Devices.All(d => d.IsUnauthorized)) Say(Loc.T("Wizard_Unauthorized"), true);
            else Say("");
            SelectedDevice ??= Devices.FirstOrDefault(d => d.IsReady);
        }
        finally { Busy = false; }
    }

    partial void OnSelectedDeviceChanged(AdbDevice? value)
    {
        Device = null;
        if (value is { IsReady: true }) _ = DetectAsync(value);
        else if (value is { IsUnauthorized: true }) Say(Loc.T("Wizard_Unauthorized"), true);
    }

    private async Task DetectAsync(AdbDevice device)
    {
        Busy = true;
        try
        {
            var info = await _adb.GetDeviceInfoAsync(device.Serial);
            if (info is null) { Say(Loc.T("Wizard_DetectFailed"), true); return; }
            if (info.Sdk < AndroidSdk.Android7) { Say(Loc.F("Wizard_TooOld", info.AndroidRelease), true); return; }
            Device = info;
            ManualBrand = _session.Brands.Find(info.Manufacturer, info.Brand);
            Say("");
        }
        catch (InvalidOperationException e) { Say(e.Message, true); }
        finally { Busy = false; }
    }

    /// <summary>
    /// Brand picker: a phone without USB debugging is invisible to adb, so the instructions must be readable before
    /// detection. Detection then selects the brand automatically.
    /// </summary>
    public IReadOnlyList<BrandInfo> BrandList => new[] { _session.Brands.Generic }.Concat(_session.Brands.Brands).ToList();

    public BrandInfo? ManualBrand
    {
        get => _brand;
        set
        {
            if (value is null || ReferenceEquals(value, _brand)) return;
            _brand = value;
            FillBrand(value);
            OnPropertyChanged();
            OnPropertyChanged(nameof(BrandName));
        }
    }

    private void FillBrand(BrandInfo brand)
    {
        string lang = Loc.Instance.Language;
        var cat = _session.Brands;
        void Fill(ObservableCollection<BrandLine> target, IEnumerable<BrandStep> steps)
        {
            target.Clear();
            foreach (var s in steps) target.Add(new BrandLine { Text = s.Text(lang), Unconfirmed = s.Unconfirmed });
        }
        Fill(DeveloperLines, cat.StepsOrGeneric(brand, b => b.Developer));
        Fill(ToggleLines, brand.ExtraToggles);
        Fill(WifiLines, cat.StepsOrGeneric(brand, b => b.Wifi));
        Fill(KeepAliveLines, cat.StepsOrGeneric(brand, b => b.KeepAlive).Concat(cat.StepsOrGeneric(brand, b => b.Launcher)));
        Fill(PermissionLines, cat.StepsOrGeneric(brand, b => b.PermissionDenied));
        Fill(ChargeLines, cat.StepsOrGeneric(brand, b => b.ChargeLimit));
    }

    // ---------- step 3: install + settings ----------

    public static string? FindApk()
    {
        var candidates = new List<string>
        {
            Path.Combine(AppContext.BaseDirectory, "apk", "ven4docking.apk"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", "Ven4Docking", "apk", "ven4docking.apk"),
        };
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        for (int i = 0; i < 8 && dir is not null; i++, dir = dir.Parent)
        {
            var outDir = Path.Combine(dir.FullName, "android", "app", "build", "outputs", "apk", "debug");
            if (Directory.Exists(outDir)) candidates.AddRange(Directory.GetFiles(outDir, "*.apk"));
        }
        return candidates.FirstOrDefault(File.Exists);
    }

    [RelayCommand]
    private void BrowseApk()
    {
        var dlg = new OpenFileDialog { Filter = "APK|*.apk", Title = Loc.T("Wizard_ApkBrowse") };
        if (dlg.ShowDialog() == true) ApkPath = dlg.FileName;
    }

    [RelayCommand]
    private async Task RunSetup()
    {
        if (SelectedDevice is null || Device is null) return;
        if (!File.Exists(ApkPath)) { Say(Loc.T("Wizard_ApkMissing"), true); return; }

        string? tz = TimeZoneInfo.TryConvertWindowsIdToIanaId(TimeZoneInfo.Local.Id, out var iana) ? iana : null;
        SetupRows.Clear();
        SetupRows.Add(new SetupRow { Id = "install", Title = Loc.T("Wizard_Item_install") });
        foreach (var s in WizardCommandBuilder.BuildSetup(Device.Sdk, tz))
            SetupRows.Add(new SetupRow { Id = s.Id, Title = Loc.T("Wizard_Item_" + s.Id), Step = s });
        await RunRowsAsync(SetupRows);
    }

    [RelayCommand]
    private async Task Retry() =>
        await RunRowsAsync(SetupRows.Where(r => r.State is SetupState.Failed or SetupState.Denied or SetupState.Warning).ToList());

    private async Task RunRowsAsync(IReadOnlyList<SetupRow> rows)
    {
        if (SelectedDevice is null) return;
        string serial = SelectedDevice.Serial;
        Busy = true;
        PermissionDenied = false;
        try
        {
            foreach (var row in rows)
            {
                row.State = SetupState.Running;
                row.Detail = "";
                if (row.Id == "install")
                {
                    var r = await _adb.RunAsync(serial, WizardCommandBuilder.Install(ApkPath), AdbClient.InstallTimeout);
                    AppendLog("install → " + r.Output.Trim());
                    var check = await _adb.RunAsync(serial, WizardCommandBuilder.PackageInstalledCheck());
                    bool ok = check.StdOut.Contains("package:", StringComparison.Ordinal) && !WizardCommandBuilder.LooksLikeError(r.Output);
                    row.State = ok ? SetupState.Ok : SetupState.Failed;
                    row.Detail = ok ? "" : FirstLine(r.Output);
                    if (!ok) break; // nothing else makes sense without the app
                    continue;
                }
                await RunStepAsync(serial, row);
                if (row.State == SetupState.Denied) PermissionDenied = true;
            }
        }
        catch (InvalidOperationException e) { Say(e.Message, true); }
        finally { Busy = false; }

        SetupDone = SetupRows.Count > 0 && SetupRows.All(r => r.State is SetupState.Ok or SetupState.Manual or SetupState.Warning);
        Say(SetupDone ? Loc.T("Wizard_SetupOk") : PermissionDenied ? Loc.T("Wizard_SetupDenied") : Loc.T("Wizard_SetupFailed"), !SetupDone);
    }

    private async Task RunStepAsync(string serial, SetupRow row)
    {
        var step = row.Step!;
        if (step.ManualOnly)
        {
            row.State = SetupState.Manual;
            row.Detail = Loc.T(step.ManualHintKey ?? "Wizard_Manual_Generic");
            return;
        }
        foreach (var cmd in step.Commands)
        {
            var r = await _adb.RunAsync(serial, cmd);
            AppendLog($"{cmd} → {r.Output.Trim()}");
            if (WizardCommandBuilder.IsPermissionDenied(r.Output))
            {
                row.State = SetupState.Denied;
                row.Detail = Loc.T("Wizard_PermissionDenied");
                return;
            }
        }
        foreach (var check in step.Checks)
        {
            var r = await _adb.RunAsync(serial, check.Command);
            AppendLog($"{check.Command} → {r.Output.Trim()}");
            if (!check.IsOk(r.StdOut))
            {
                row.State = step.Optional ? SetupState.Warning : SetupState.Failed;
                row.Detail = step.ManualHintKey is not null
                    ? Loc.T(step.ManualHintKey)
                    : Loc.F("Wizard_CheckFailed", check.Expected, FirstLine(r.Output));
                return;
            }
        }
        row.State = SetupState.Ok;
    }

    private static string FirstLine(string s)
    {
        s = s.Trim();
        int nl = s.IndexOf('\n');
        return nl < 0 ? s : s[..nl].Trim();
    }

    // ---------- step 4: token ----------

    [RelayCommand]
    private async Task SendToken()
    {
        if (SelectedDevice is null || Device is null) return;
        if (!_session.AgentOnline) { Say(Loc.T("Agent_NeededForThis"), true); return; }
        if (!Uri.TryCreate(AgentUrl, UriKind.Absolute, out _)) { Say(Loc.T("Wizard_BadUrl"), true); return; }
        Busy = true;
        try
        {
            var t = await _session.Admin.CreatePhoneTokenAsync(string.IsNullOrWhiteSpace(PhoneName) ? Device.Model : PhoneName, Device.Model);
            var r = await _adb.RunAsync(SelectedDevice.Serial, WizardCommandBuilder.Launch(AgentUrl, t.Token));
            AppendLog("am start → " + r.Output.Trim().Replace(t.Token, "***"));
            TokenSent = !WizardCommandBuilder.LooksLikeError(r.Output);
            Say(TokenSent ? Loc.T("Wizard_TokenSent") : Loc.F("Wizard_TokenFailed", FirstLine(r.Output)), !TokenSent);
        }
        catch (AdminException e) { Say(Loc.F("Wizard_TokenFailed", e.Message), true); }
        catch (InvalidOperationException e) { Say(e.Message, true); }
        finally { Busy = false; }
    }

    // ---------- step 5: WebView ----------

    [RelayCommand]
    private async Task CheckWebView()
    {
        if (SelectedDevice is null) return;
        Busy = true;
        try
        {
            var r = await _adb.RunAsync(SelectedDevice.Serial, WizardCommandBuilder.WebViewQuery());
            var wv = WizardCommandBuilder.ParseWebView(r.StdOut);
            if (wv is null)
            {
                // Android 7.x: dumpsys webviewupdate is empty — read the provider packages' versions instead
                var p = await _adb.RunAsync(SelectedDevice.Serial, WizardCommandBuilder.WebViewPackagesQuery());
                wv = WizardCommandBuilder.ParseWebViewPackages(p.StdOut);
            }
            int? major = WizardCommandBuilder.MajorVersion(wv?.Version);
            WebViewOld = major is < WizardCommandBuilder.FullPanelMinWebView;
            WebViewText = wv is null ? Loc.T("WebView_Unknown") : Loc.F("WebView_Version", wv.Value.Package, wv.Value.Version);
        }
        catch (InvalidOperationException e) { Say(e.Message, true); }
        finally { Busy = false; }
    }
}
