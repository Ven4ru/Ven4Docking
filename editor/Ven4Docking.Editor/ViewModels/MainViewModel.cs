using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Models;
using Ven4Docking.Core.Validation;
using Ven4Docking.Editor.Services;
using Ven4Docking.Editor.Themes;

namespace Ven4Docking.Editor.ViewModels;

public abstract partial class SectionBase : ObservableObject
{
    protected SectionBase(EditorSession session) => Session = session;

    public EditorSession Session { get; }

    /// <summary>Rebuild item view models from the session model (after undo, reload or a language switch).</summary>
    public virtual void Rebuild() { }

    /// <summary>The section became visible.</summary>
    public virtual void Activated() { }

    /// <summary>Another section was selected.</summary>
    public virtual void Deactivated() { }
}

public sealed class SectionItem : ObservableObject
{
    public SectionItem() => Loc.Instance.LanguageChanged += () => OnPropertyChanged(nameof(Title));

    public required string Key { get; init; }
    public required string Glyph { get; init; }
    public required SectionBase ViewModel { get; init; }
    public string Title => Loc.T("Section_" + Key);

    // ListBoxItem automation name (screen readers, UI tests) — otherwise it is the type name.
    public override string ToString() => Title;
}

public sealed partial class MainViewModel : ObservableObject
{
    private readonly DispatcherTimer _statusTimer;
    private bool _switchingPhone;

    public MainViewModel(EditorSession session)
    {
        Session = session;
        Phones = new PhonesViewModel(session);
        Screens = new ScreensViewModel(session);
        Sections = new ObservableCollection<SectionItem>
        {
            new() { Key = "Phones", Glyph = "", ViewModel = Phones },
            new() { Key = "Screens", Glyph = "", ViewModel = Screens },
            new() { Key = "Apps", Glyph = "", ViewModel = new AppsViewModel(session) },
            new() { Key = "Buttons", Glyph = "", ViewModel = new ButtonsViewModel(session) },
            new() { Key = "Servers", Glyph = "", ViewModel = new ServersViewModel(session) },
            new() { Key = "Ai", Glyph = "", ViewModel = new AiViewModel(session) },
            new() { Key = "Notify", Glyph = "", ViewModel = new NotifyViewModel(session) },
            new() { Key = "Settings", Glyph = "", ViewModel = new SettingsViewModel(session) },
            new() { Key = "About", Glyph = "", ViewModel = new AboutViewModel(session) },
        };
        _selectedSection = Sections[1];

        session.Restored += () =>
        {
            foreach (var s in Sections) s.ViewModel.Rebuild();
            ThemeManager.Apply(Session.Config.Theme);
            Loc.Instance.SetLanguage(Session.Config.Language);
        };
        session.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(EditorSession.AgentOnline) or nameof(EditorSession.AgentStatus))
            {
                Firewall.Update(Session.AgentOnline ? Session.AgentStatus : null);
                OnPropertyChanged(nameof(AgentText));
                OnPropertyChanged(nameof(AgentOnline));
                OnPropertyChanged(nameof(StorageText));
                StartAgentCommand.NotifyCanExecuteChanged();
            }
            if (e.PropertyName is nameof(EditorSession.IsDirty) or nameof(EditorSession.Issues))
                SaveCommand.NotifyCanExecuteChanged();
            if (e.PropertyName == nameof(EditorSession.CanUndo))
                UndoCommand.NotifyCanExecuteChanged();
        };
        Loc.Instance.LanguageChanged += () =>
        {
            OnPropertyChanged(nameof(AgentText));
            OnPropertyChanged(nameof(StorageText));
            NotifyFirewall();
            foreach (var s in Sections) s.ViewModel.Rebuild();
        };
        Firewall.Changed += NotifyFirewall;

        _statusTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(5) };
        _statusTimer.Tick += async (_, _) => await Session.RefreshStatusAsync();
    }

    public EditorSession Session { get; }
    public PhonesViewModel Phones { get; }
    public ScreensViewModel Screens { get; }
    public ObservableCollection<SectionItem> Sections { get; }

    [ObservableProperty] private SectionItem _selectedSection;
    [ObservableProperty] private PhoneChoice? _selectedPhone;
    [ObservableProperty] private string _statusMessage = "";
    [ObservableProperty] private bool _statusIsError;

    public bool AgentOnline => Session.AgentOnline;

    public string AgentText => Session.AgentOnline
        ? Loc.F("Agent_Online", Session.AgentStatus?.Version ?? "?", Session.Port)
        : Loc.T("Agent_Offline");

    public string StorageText => Session.StorageDescription;

    /// <param name="forceUpdateCheck">«--update» from the agent's tray: check even when VEN4DOCKING_NO_UPDATE_CHECK=1.</param>
    public async Task InitializeAsync(bool forceUpdateCheck = false)
    {
        await Session.LoadAsync();
        _switchingPhone = true;
        SelectedPhone = Session.CurrentPhone;
        _switchingPhone = false;
        ThemeManager.Apply(Session.Config.Theme);
        Loc.Instance.SetLanguage(Session.Config.Language);
        foreach (var s in Sections) s.ViewModel.Rebuild();
        SelectedSection.ViewModel.Activated();
        _statusTimer.Start();
        OnPropertyChanged(nameof(StorageText));
        if (forceUpdateCheck || Environment.GetEnvironmentVariable("VEN4DOCKING_NO_UPDATE_CHECK") != "1")
            _ = About.CheckUpdatesCommand.ExecuteAsync(null);
    }

    /// <summary>«Обновить» on the banner: go to «О программе», where the progress is shown, and start the download.</summary>
    [RelayCommand]
    private async Task UpdateFromBanner()
    {
        Select("About");
        if (About.InstallUpdateCommand.CanExecute(null)) await About.InstallUpdateCommand.ExecuteAsync(null);
    }

    public AboutViewModel About => (AboutViewModel)Sections.Single(s => s.Key == "About").ViewModel;

    /// <summary>For screenshots and deep links: «--section Phones».</summary>
    public void Select(string key)
    {
        var s = Sections.FirstOrDefault(x => string.Equals(x.Key, key, StringComparison.OrdinalIgnoreCase));
        if (s is not null) SelectedSection = s;
    }

    partial void OnSelectedSectionChanged(SectionItem? oldValue, SectionItem newValue)
    {
        oldValue?.ViewModel.Deactivated();
        newValue.ViewModel.Activated();
    }

    partial void OnSelectedPhoneChanged(PhoneChoice? value)
    {
        if (_switchingPhone || value is null || value.Id == Session.CurrentPhone?.Id) return;
        _ = SwitchPhoneAsync(value);
    }

    private async Task SwitchPhoneAsync(PhoneChoice phone)
    {
        if (Session.IsDirty)
        {
            var answer = MessageBox.Show(Loc.T("Ask_SaveBeforeSwitch"), "Ven4Docking", MessageBoxButton.YesNoCancel, MessageBoxImage.Question);
            if (answer == MessageBoxResult.Cancel)
            {
                _switchingPhone = true;
                SelectedPhone = Session.CurrentPhone;
                _switchingPhone = false;
                return;
            }
            if (answer == MessageBoxResult.Yes && !(await SaveCoreAsync())) return;
        }
        Session.CurrentPhone = phone;
        await Session.LoadLayoutAsync(phone.Id);
    }

    private bool CanUndo() => Session.CanUndo;

    [RelayCommand(CanExecute = nameof(CanUndo))]
    private void Undo()
    {
        Session.Undo();
        StatusMessage = "";
    }

    private bool CanSave() => Session.IsDirty;

    [RelayCommand(CanExecute = nameof(CanSave))]
    private async Task Save() => await SaveCoreAsync();

    private async Task<bool> SaveCoreAsync()
    {
        try
        {
            var r = await Session.SaveAsync();
            StatusIsError = !r.Ok;
            if (r.Ok)
            {
                StatusMessage = Session.AgentOnline
                    ? Loc.F("Saved_Agent", Session.Layout.Version)
                    : Loc.F("Saved_File", Session.Paths.LayoutFile(Session.CurrentPhone?.Id ?? PhoneChoice.DraftId));
            }
            else
            {
                StatusMessage = r.Error switch
                {
                    IssueKinds.Overlap => Loc.F("SaveError_Overlap", string.Join(", ", r.Blocks ?? Array.Empty<string>())),
                    IssueKinds.Bounds => Loc.F("SaveError_Bounds", string.Join(", ", r.Blocks ?? Array.Empty<string>())),
                    IssueKinds.Screens => Loc.T("SaveError_Screens"),
                    IssueKinds.ProgramScreens => Loc.F("SaveError_ProgramScreens", LayoutScreen.MaxProgramScreens),
                    IssueKinds.NoMainScreen => Loc.T("SaveError_NoMainScreen"),
                    EditorSession.ActionsError => Loc.F("SaveError_Actions", string.Join(", ", ActionTitles(r.Blocks))),
                    _ => Loc.F("SaveError_Other", r.Error),
                };
                Select(r.Error == EditorSession.ActionsError ? "Buttons" : "Screens");
            }
            return r.Ok;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or Core.Admin.AdminException)
        {
            StatusIsError = true;
            StatusMessage = Loc.F("SaveError_Other", e.Message);
            return false;
        }
    }

    private IEnumerable<string> ActionTitles(IReadOnlyList<string>? ids) =>
        (ids ?? Array.Empty<string>()).Select(id =>
            Session.Config.Actions?.FirstOrDefault(a => a.Id == id) is { Title.Length: > 0 } a ? a.Title : id);

    // ---------- firewall (spec §13: «если отказали — агент работает, но телефон не достучится; редактор показывает это») ----------

    public FirewallNotice Firewall { get; } = new();

    public bool FirewallWarning => Firewall.ShowWarning;

    public string FirewallText => Loc.T(Firewall.State == FirewallState.Blocked ? "Firewall_Blocked" : "Firewall_Missing");

    /// <summary>Progress or the outcome of the last «Исправить»; empty when there is nothing to say.</summary>
    public string FirewallNote
    {
        get
        {
            if (Firewall.IsFixing) return Loc.T("Firewall_Fixing");
            var r = Firewall.LastResult;
            return r?.Outcome switch
            {
                FirewallFixOutcome.Declined => Loc.T("Firewall_Declined"),
                FirewallFixOutcome.AgentNotFound => Loc.T("Agent_NotFound"),
                FirewallFixOutcome.Fixed => Loc.T("Firewall_StillReported"),
                FirewallFixOutcome.Failed when r.ExitCode is int code => Loc.F("Firewall_FailedCode", code),
                FirewallFixOutcome.Failed => Loc.F("Firewall_FailedError", r.Error),
                _ => "",
            };
        }
    }

    public bool FirewallNoteIsError => Firewall.LastResult?.Outcome is FirewallFixOutcome.Failed or FirewallFixOutcome.AgentNotFound;

    private void NotifyFirewall()
    {
        OnPropertyChanged(nameof(FirewallWarning));
        OnPropertyChanged(nameof(FirewallText));
        OnPropertyChanged(nameof(FirewallNote));
        OnPropertyChanged(nameof(FirewallNoteIsError));
        FixFirewallCommand.NotifyCanExecuteChanged();
    }

    private bool CanFixFirewall() => Firewall.CanFix;

    /// <summary>«Исправить»: ven4docking-agent.exe --firewall add, elevated (UAC). Declining keeps the warning.</summary>
    [RelayCommand(CanExecute = nameof(CanFixFirewall))]
    private async Task FixFirewall()
    {
        await Firewall.FixAsync(FindAgentExe(), async () =>
        {
            await Session.RefreshStatusAsync();
            return Session.AgentOnline ? Session.AgentStatus : null;
        });
    }

    public static string? FindAgentExe()
    {
        var candidates = new[]
        {
            Path.Combine(AppContext.BaseDirectory, "ven4docking-agent.exe"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", "Ven4Docking", "ven4docking-agent.exe"),
        };
        return candidates.FirstOrDefault(File.Exists);
    }

    private bool CanStartAgent() => !Session.AgentOnline;

    /// <summary>«Агент не запущен — запустить?» (spec §11).</summary>
    [RelayCommand(CanExecute = nameof(CanStartAgent))]
    private async Task StartAgent()
    {
        string? exe = FindAgentExe();
        if (exe is null)
        {
            StatusIsError = true;
            StatusMessage = Loc.T("Agent_NotFound");
            return;
        }
        Process.Start(new ProcessStartInfo(exe) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(exe)! });
        StatusIsError = false;
        StatusMessage = Loc.T("Agent_Starting");
        for (int i = 0; i < 10 && !Session.AgentOnline; i++)
        {
            await Task.Delay(700);
            await Session.RefreshStatusAsync();
        }
        if (Session.AgentOnline && !Session.IsDirty) await Session.LoadAsync(Session.CurrentPhone?.Id);
        StatusMessage = Session.AgentOnline ? "" : Loc.T("Agent_StartFailed");
        StatusIsError = !Session.AgentOnline;
    }
}
