using System.Collections.ObjectModel;
using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Models;
using Ven4Docking.Editor.Services;

namespace Ven4Docking.Editor.ViewModels;

/// <summary>An app that sends notifications, with its mode on the panel.</summary>
public sealed partial class NotifyAppItem : ObservableObject
{
    private readonly NotifyViewModel _owner;

    public NotifyAppItem(NotifyViewModel owner, string aumid, string? name, double? lastSeen)
    {
        _owner = owner;
        Aumid = aumid;
        Name = string.IsNullOrWhiteSpace(name) ? aumid : name;
        LastSeen = lastSeen;
    }

    public string Aumid { get; }
    public string Name { get; }
    public double? LastSeen { get; }
    public bool HasOwnName => Name != Aumid;

    public string LastSeenText => LastSeen is { } t
        ? Loc.F("Notify_LastSeen", DateTimeOffset.FromUnixTimeMilliseconds((long)(t * 1000)).ToLocalTime().ToString("g"))
        : Loc.T("Notify_NotSeen");

    /// <summary>Second line: when it was seen, plus the AppUserModelId if the name differs from it.</summary>
    public string Details => HasOwnName ? $"{LastSeenText} · {Aumid}" : LastSeenText;

    public IReadOnlyList<Choice> Modes => _owner.ModeChoices;

    // ListBoxItem automation name (screen readers, UI tests) — otherwise it is the type name.
    public override string ToString() => Name;

    public string Mode
    {
        get => _owner.GetMode(Aumid);
        set
        {
            if (value == Mode) return;
            _owner.SetMode(Aumid, value);
            OnPropertyChanged();
        }
    }
}

/// <summary>
/// «Уведомления» (spec 1.1 §9): on/off, Windows access with «Открыть параметры», per-app mode
/// (скрыть | счётчик | заголовок | текст целиком), «На стене только счётчики», night flash.
/// </summary>
public sealed partial class NotifyViewModel : SectionBase
{
    public const string WindowsSettingsUri = "ms-settings:privacy-notifications";

    private NotifyAppsResponse? _fromAgent;

    public NotifyViewModel(EditorSession session) : base(session) { }

    public ObservableCollection<NotifyAppItem> Apps { get; } = new();

    [ObservableProperty] private string _message = "";
    [ObservableProperty] private bool _loading;
    [ObservableProperty] private string? _access;

    public IReadOnlyList<Choice> ModeChoices { get; private set; } = Array.Empty<Choice>();

    public bool HasApps => Apps.Count > 0;

    private NotifySettings Settings => Session.Config.Notify ?? new NotifySettings();

    private void Edit(Action<NotifySettings> change, [System.Runtime.CompilerServices.CallerMemberName] string? prop = null)
    {
        Session.Checkpoint();
        change(Session.Config.Notify ??= new NotifySettings());
        OnPropertyChanged(prop);
    }

    public bool Enabled { get => Settings.Enabled; set => Edit(n => n.Enabled = value); }
    public bool WallCountsOnly { get => Settings.WallCountsOnly; set => Edit(n => n.WallCountsOnly = value); }
    public bool NightFlash { get => Settings.NightFlash; set => Edit(n => n.NightFlash = value); }

    public string GetMode(string aumid) => Settings.GetMode(aumid);

    public void SetMode(string aumid, string mode)
    {
        Session.Checkpoint();
        (Session.Config.Notify ??= new NotifySettings()).SetMode(aumid, mode);
    }

    /// <summary>true → allowed, false → denied, null → unknown or not asked yet (for the state colour).</summary>
    public bool? AccessOk => Access switch
    {
        NotifyAccess.Allowed => true,
        NotifyAccess.Denied => false,
        _ => null,
    };

    public string AccessText => Access switch
    {
        null => Loc.T("Notify_AccessUnknown"),
        NotifyAccess.Allowed => Loc.T("Notify_AccessAllowed"),
        NotifyAccess.Denied => Loc.T("Notify_AccessDenied"),
        NotifyAccess.Unspecified => Loc.T("Notify_AccessUnspecified"),
        NotifyAccess.Off => Loc.T("Notify_AccessOff"),
        _ => Access,
    };

    partial void OnAccessChanged(string? value)
    {
        OnPropertyChanged(nameof(AccessOk));
        OnPropertyChanged(nameof(AccessText));
    }

    public override void Rebuild()
    {
        ModeChoices = NotifySettings.Modes.Select(m => new Choice(m, Loc.T("NotifyMode_" + m))).ToList();
        OnPropertyChanged(nameof(ModeChoices));
        foreach (var p in new[] { nameof(Enabled), nameof(WallCountsOnly), nameof(NightFlash), nameof(AccessText) }) OnPropertyChanged(p);
        Fill();
    }

    public override void Activated() => _ = LoadAsync();

    /// <summary>GET /admin/notify/apps; the list also keeps the apps already in config.json.</summary>
    [RelayCommand]
    private async Task LoadAsync()
    {
        if (!Session.AgentOnline)
        {
            _fromAgent = null;
            Access = null;
            Message = Loc.T("Notify_Offline");
            Fill();
            return;
        }
        Loading = true;
        try
        {
            _fromAgent = await Session.Admin.GetNotifyAppsAsync();
            Access = _fromAgent.Access;
            Message = "";
        }
        catch (AdminException e)
        {
            _fromAgent = null;
            Access = null;
            Message = e.AgentTooOld ? Loc.T("Agent_TooOld") : e.AgentUnreachable ? Loc.T("Notify_Offline") : e.Message;
        }
        finally { Loading = false; }
        Fill();
    }

    private void Fill()
    {
        Apps.Clear();
        foreach (var a in (_fromAgent?.Apps ?? new List<NotifyAppInfo>()).Where(a => !string.IsNullOrEmpty(a.Aumid)).OrderByDescending(a => a.LastSeen ?? 0))
            Apps.Add(new NotifyAppItem(this, a.Aumid, a.Name, a.LastSeen));
        foreach (var aumid in Settings.Apps.Keys.Where(k => Apps.All(a => a.Aumid != k)))
            Apps.Add(new NotifyAppItem(this, aumid, null, null));
        OnPropertyChanged(nameof(HasApps));
    }

    [RelayCommand]
    private void OpenWindowsSettings() =>
        Process.Start(new ProcessStartInfo(WindowsSettingsUri) { UseShellExecute = true });
}
