using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
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

    public override void Rebuild()
    {
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
}
