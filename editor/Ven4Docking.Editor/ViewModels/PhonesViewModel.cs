using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Ven4Docking.Core.Adb;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Models;
using Ven4Docking.Editor.Services;

namespace Ven4Docking.Editor.ViewModels;

/// <summary>Phone card: live data from the agent + night/brightness settings from config.json.</summary>
public sealed partial class PhoneCard : ObservableObject
{
    private readonly PhonesViewModel _owner;

    public PhoneCard(PhonesViewModel owner, string id, PhoneInfo? info, PhoneEntry? entry)
    {
        _owner = owner;
        Id = id;
        Info = info;
        Entry = entry;
    }

    public string Id { get; }
    public PhoneInfo? Info { get; }
    public PhoneEntry? Entry { get; private set; }

    public string Name => Info?.Name ?? Entry?.Name ?? Id;
    public string Model => Info?.Model ?? "—";
    public string Android => Info?.Android is { } a ? "Android " + a : "—";
    public string WebView => Info?.Webview ?? "—";
    public string Battery => Info?.Battery is { } b ? $"{b:0} %" : "—";
    public string Temp => Info?.Temp is { } t ? $"{t:0.0} °C" : "—";
    public bool Online => Info?.Online == true;

    public string LauncherVersion => Info?.LauncherVersion is { Length: > 0 } v ? v : "—";

    /// <summary>«Обновить лаунчер»: an APK is next to the editor and the phone's launcher is older or of unknown version.</summary>
    public bool LauncherUpdateOffered =>
        _owner.Apk is { } apk && LauncherApk.IsUpdateOffered(Info?.LauncherVersion, apk.Version);

    public string LauncherUpdateText =>
        _owner.Apk?.Version is { } v && LauncherApk.ParseVersion(Info?.LauncherVersion) is not null
            ? Loc.F("Launcher_UpdateTo", v)
            : Loc.T("Launcher_Update");

    public string LinkText => Info is null
        ? Loc.T("Phone_NotInAgent")
        : Online ? Loc.T("Phone_Online")
        : Info.LastSeen is { } ls ? Loc.F("Phone_LastSeen", DateTimeOffset.FromUnixTimeMilliseconds((long)(ls * 1000)).ToLocalTime().ToString("g"))
        : Loc.T("Phone_Offline");

    private PhoneEntry EnsureEntry()
    {
        if (Entry is not null) return Entry;
        var cfg = _owner.Session.Config;
        Entry = cfg.Phones.FirstOrDefault(p => p.Id == Id);
        if (Entry is null)
        {
            Entry = new PhoneEntry { Id = Id, Name = Name };
            cfg.Phones.Add(Entry);
        }
        return Entry;
    }

    private void Edit(Action<PhoneEntry> change, [System.Runtime.CompilerServices.CallerMemberName] string? prop = null)
    {
        _owner.Session.Checkpoint();
        change(EnsureEntry());
        OnPropertyChanged(prop);
    }

    public string NightFrom
    {
        get => Entry?.Night?.From ?? "23:00";
        set => Edit(e => (e.Night ??= new NightSettings()).From = value);
    }

    public string NightTo
    {
        get => Entry?.Night?.To ?? "07:00";
        set => Edit(e => (e.Night ??= new NightSettings()).To = value);
    }

    /// <summary>Night brightness in percent for the slider (stored as 0..1).</summary>
    public double NightBrightness
    {
        get => Math.Round((Entry?.Night?.Brightness ?? 0.02) * 100, 0);
        set => Edit(e => (e.Night ??= new NightSettings()).Brightness = Math.Round(Math.Clamp(value, 0, 100) / 100, 2));
    }

    public bool DimWhenPcOffline
    {
        get => Entry?.DimWhenPcOffline ?? true;
        set => Edit(e => e.DimWhenPcOffline = value);
    }

    public bool BurnInShift
    {
        get => Entry?.BurnInShift ?? true;
        set => Edit(e => e.BurnInShift = value);
    }
}

public sealed class PairItem
{
    public required PairRequest Request { get; init; }
    public string Text => Loc.F("Pair_Question", Request.Model ?? "?", Request.Code);
    public string Details => $"Android {Request.Android ?? "?"} · {Request.Ip ?? "?"}";
}

/// <summary>«Телефоны»: phone cards, pairing requests (accept/deny), revoke, the connection wizard.</summary>
public sealed partial class PhonesViewModel : SectionBase
{
    private readonly DispatcherTimer _pairTimer;
    private bool _active;

    public PhonesViewModel(EditorSession session) : base(session)
    {
        Wizard = new WizardViewModel(session, () => IsWizardOpen = false);
        _pairTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(3) };
        _pairTimer.Tick += async (_, _) => await RefreshPairsAsync();
        session.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(EditorSession.AgentOnline))
            {
                OnPropertyChanged(nameof(AgentOnline));
                if (_active) _ = RefreshAsync();
            }
        };
    }

    public WizardViewModel Wizard { get; }
    public ObservableCollection<PhoneCard> Phones { get; } = new();
    public ObservableCollection<PairItem> Pairs { get; } = new();

    [ObservableProperty] private PhoneCard? _selected;
    [ObservableProperty] private bool _isWizardOpen;
    [ObservableProperty] private string _message = "";

    public bool AgentOnline => Session.AgentOnline;
    public bool HasPhones => Phones.Count > 0;
    public bool HasPairs => Pairs.Count > 0;

    /// <summary>The launcher APK next to the editor (as the wizard finds it) with its version from apk\latest.json.</summary>
    public LauncherApk? Apk { get; private set; }

    public override void Rebuild()
    {
        Apk = LauncherApk.Locate(WizardViewModel.FindApk());
        string? keep = Selected?.Id;
        Phones.Clear();
        foreach (var choice in Session.Phones.Where(p => !p.IsDraft))
            Phones.Add(new PhoneCard(this, choice.Id, choice.Info, Session.Config.Phones.FirstOrDefault(p => p.Id == choice.Id)));
        Selected = Phones.FirstOrDefault(p => p.Id == keep) ?? Phones.FirstOrDefault();
        OnPropertyChanged(nameof(HasPhones));
        Wizard.RefreshTexts();
    }

    public override void Activated()
    {
        _active = true;
        _pairTimer.Start();
        _ = RefreshPairsAsync();
    }

    public override void Deactivated()
    {
        _active = false;
        _pairTimer.Stop();
    }

    [RelayCommand]
    private async Task RefreshAsync()
    {
        await Session.LoadAsync(Session.CurrentPhone?.Id);
        Rebuild();
        await RefreshPairsAsync();
    }

    private async Task RefreshPairsAsync()
    {
        if (!Session.AgentOnline)
        {
            if (Pairs.Count > 0) { Pairs.Clear(); OnPropertyChanged(nameof(HasPairs)); }
            return;
        }
        try
        {
            var list = await Session.Admin.GetPairsAsync();
            if (list.Select(p => p.PairId).SequenceEqual(Pairs.Select(p => p.Request.PairId))) return;
            Pairs.Clear();
            foreach (var p in list) Pairs.Add(new PairItem { Request = p });
            OnPropertyChanged(nameof(HasPairs));
        }
        catch (AdminException) { }
    }

    [RelayCommand]
    private async Task AcceptPair(PairItem item)
    {
        try
        {
            await Session.Admin.AcceptPairAsync(item.Request.PairId);
            Message = Loc.F("Pair_Accepted", item.Request.Model ?? "");
            await RefreshAsync();
        }
        catch (AdminException e) { Message = e.Message; }
    }

    [RelayCommand]
    private async Task DenyPair(PairItem item)
    {
        try
        {
            await Session.Admin.DenyPairAsync(item.Request.PairId);
            await RefreshPairsAsync();
        }
        catch (AdminException e) { Message = e.Message; }
    }

    [RelayCommand]
    private async Task Revoke(PhoneCard? card)
    {
        if (card is null) return;
        if (!Session.AgentOnline)
        {
            Message = Loc.T("Agent_NeededForThis");
            return;
        }
        var ok = MessageBox.Show(Loc.F("Phone_RevokeConfirm", card.Name), "Ven4Docking", MessageBoxButton.OKCancel, MessageBoxImage.Warning);
        if (ok != MessageBoxResult.OK) return;
        try
        {
            await Session.Admin.RevokePhoneAsync(card.Id);
            Message = Loc.F("Phone_Revoked", card.Name);
            await RefreshAsync();
        }
        catch (AdminException e) { Message = e.Message; }
    }

    [RelayCommand]
    private void OpenWizard()
    {
        Wizard.Restart();
        IsWizardOpen = true;
    }

    // ---------- launcher update over adb (the launcher does not install anything itself) ----------

    private PhoneCard? _launcherCard;

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(UpdateLauncherCommand), nameof(InstallToSelectedDeviceCommand))]
    private bool _launcherBusy;

    [ObservableProperty] private string _launcherMessage = "";
    [ObservableProperty] private bool _launcherMessageIsError;
    [ObservableProperty] private bool _launcherNeedsConnection;
    [ObservableProperty] private double _adbDownloadProgress;

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(InstallToSelectedDeviceCommand))]
    private AdbDevice? _selectedLauncherDevice;

    /// <summary>Several devices are connected: the user picks the phone.</summary>
    public ObservableCollection<AdbDevice> LauncherDevices { get; } = new();

    public bool HasLauncherDevices => LauncherDevices.Count > 0;

    partial void OnSelectedChanged(PhoneCard? value)
    {
        if (LauncherBusy) return;
        _launcherCard = null;
        LauncherDevices.Clear();
        OnPropertyChanged(nameof(HasLauncherDevices));
        LauncherNeedsConnection = false;
        SayLauncher("");
    }

    private void SayLauncher(string text, bool error = false)
    {
        LauncherMessage = text;
        LauncherMessageIsError = error;
    }

    private bool CanUpdateLauncher() => !LauncherBusy;

    [RelayCommand(CanExecute = nameof(CanUpdateLauncher))]
    private async Task UpdateLauncher(PhoneCard? card)
    {
        card ??= Selected;
        if (card is null) return;
        _launcherCard = card;
        LauncherDevices.Clear();
        OnPropertyChanged(nameof(HasLauncherDevices));
        LauncherNeedsConnection = false;
        Apk = LauncherApk.Locate(WizardViewModel.FindApk());
        if (Apk is null)
        {
            SayLauncher(Loc.T("Launcher_NoApk"), true);
            return;
        }

        LauncherBusy = true;
        try
        {
            var adb = await EnsureAdbAsync();
            if (adb is null) return;
            SayLauncher(Loc.T("Launcher_Searching"));
            var devices = (await adb.DevicesAsync()).Where(d => adb.IsAllowed(d.Serial)).ToList();
            var ready = devices.Where(d => d.IsReady).ToList();
            if (ready.Count == 0)
            {
                if (devices.Any(d => d.IsUnauthorized)) SayLauncher(Loc.T("Launcher_Unauthorized"), true);
                else
                {
                    SayLauncher(Loc.T("Launcher_NoDevice"), true);
                    LauncherNeedsConnection = true;
                }
                return;
            }
            var pick = LauncherInstaller.PickDevice(ready, card.Info?.Model);
            if (pick is null)
            {
                foreach (var d in ready) LauncherDevices.Add(d);
                OnPropertyChanged(nameof(HasLauncherDevices));
                SelectedLauncherDevice = ready.FirstOrDefault();
                SayLauncher(Loc.T("Launcher_ChooseDevice"));
                return;
            }
            await InstallAsync(adb, pick);
        }
        catch (InvalidOperationException e) { SayLauncher(Loc.F("Launcher_Failed", e.Message, ""), true); }
        catch (System.ComponentModel.Win32Exception e) { SayLauncher(Loc.F("Launcher_Failed", e.Message, ""), true); }
        finally { LauncherBusy = false; }
    }

    private bool CanInstallToSelectedDevice() => !LauncherBusy && SelectedLauncherDevice is not null;

    [RelayCommand(CanExecute = nameof(CanInstallToSelectedDevice))]
    private async Task InstallToSelectedDevice()
    {
        var device = SelectedLauncherDevice;
        if (device is null || Apk is null) return;
        LauncherBusy = true;
        try { await InstallAsync(AdbClient.FromEnvironment(Session.Paths.AdbExe), device); }
        catch (InvalidOperationException e) { SayLauncher(Loc.F("Launcher_Failed", e.Message, ""), true); }
        catch (System.ComponentModel.Win32Exception e) { SayLauncher(Loc.F("Launcher_Failed", e.Message, ""), true); }
        finally { LauncherBusy = false; }
    }

    private async Task InstallAsync(AdbClient adb, AdbDevice device)
    {
        SayLauncher(Loc.F("Launcher_Installing", device.DisplayName));
        var r = await LauncherInstaller.InstallAsync(adb, device.Serial, Apk!);
        if (!r.Ok)
        {
            SayLauncher(Loc.F("Launcher_Failed", Loc.T("AdbErr_" + r.ErrorKind), string.IsNullOrEmpty(r.Detail) ? "" : $"({r.Detail})"), true);
            return;
        }
        LauncherDevices.Clear();
        OnPropertyChanged(nameof(HasLauncherDevices));
        SayLauncher(r.InstalledVersion is { } v
            ? Loc.F("Launcher_Done", v, device.DisplayName)
            : Loc.F("Launcher_DoneNoVersion", device.DisplayName));
    }

    /// <summary>platform-tools as in the wizard: downloaded from dl.google.com only with consent.</summary>
    private async Task<AdbClient?> EnsureAdbAsync()
    {
        var adb = AdbClient.FromEnvironment(Session.Paths.AdbExe);
        if (adb.IsAvailable) return adb;
        var answer = MessageBox.Show(Loc.F("Adb_Consent_Text", Session.Paths.PlatformToolsDir), Loc.T("Adb_Consent_Title"),
            MessageBoxButton.YesNo, MessageBoxImage.Question, MessageBoxResult.No);
        if (answer != MessageBoxResult.Yes)
        {
            SayLauncher(Loc.T("Adb_Declined"), true);
            return null;
        }
        try
        {
            SayLauncher(Loc.T("Adb_Downloading"));
            var progress = new Progress<double>(p => AdbDownloadProgress = p * 100);
            using var http = new HttpClient { Timeout = TimeSpan.FromMinutes(5) };
            await new PlatformToolsDownloader(http).DownloadAsync(Session.Paths.PlatformToolsDir, progress);
        }
        catch (Exception e) when (e is HttpRequestException or IOException or InvalidDataException or TaskCanceledException)
        {
            SayLauncher(Loc.F("Adb_DownloadFailed", e.Message), true);
            return null;
        }
        finally { AdbDownloadProgress = 0; }
        adb = AdbClient.FromEnvironment(Session.Paths.AdbExe);
        return adb.IsAvailable ? adb : null;
    }
}
