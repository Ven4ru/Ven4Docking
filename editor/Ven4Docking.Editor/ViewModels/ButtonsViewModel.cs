using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Apps;
using Ven4Docking.Core.Models;
using Ven4Docking.Core.Validation;
using Ven4Docking.Editor.Services;

namespace Ven4Docking.Editor.ViewModels;

/// <summary>One step of a «Цепочка» action.</summary>
public sealed partial class MacroStepItem : ObservableObject
{
    private readonly ActionItem _owner;

    public MacroStepItem(ActionItem owner, MacroStep model)
    {
        _owner = owner;
        Model = model;
    }

    public MacroStep Model { get; }

    private void Edit(Action change, [System.Runtime.CompilerServices.CallerMemberName] string? prop = null)
    {
        _owner.Owner.Session.Checkpoint();
        change();
        OnPropertyChanged(prop);
        _owner.StepsChanged();
    }

    public string Kind => Model.Kind ?? "";
    public string KindText => Loc.T("StepKind_" + (Model.Kind ?? "bad"));
    public bool IsKeys => Model.Kind == MacroStep.KindKeys;
    public bool IsText => Model.Kind == MacroStep.KindText;
    public bool IsWait => Model.Kind == MacroStep.KindWait;
    public bool IsLaunch => Model.Kind == MacroStep.KindLaunch;
    public int Number => _owner.Steps.IndexOf(this) + 1;

    public string Keys { get => Model.Keys ?? ""; set => Edit(() => Model.Keys = Hotkey.Normalize(value) ?? value); }
    public string Text { get => Model.Text ?? ""; set => Edit(() => Model.Text = ActionItem.NormalizeText(value)); }

    [RelayCommand]
    private void ClearKeys() => Keys = "";

    public string Wait
    {
        get => Model.Wait?.ToString() ?? "";
        set
        {
            if (!int.TryParse(value.Trim(), out int ms)) { OnPropertyChanged(); return; }
            ms = Math.Clamp(ms, MacroStep.MinWait, MacroStep.MaxWait);
            if (ms == Model.Wait) { OnPropertyChanged(); return; }
            Edit(() => Model.Wait = ms);
        }
    }

    public IReadOnlyList<Choice> Apps => _owner.Owner.AppChoices;

    public string? Launch
    {
        get => Model.Launch;
        set
        {
            if (value is null || value == Model.Launch) return;
            Edit(() => Model.Launch = value);
        }
    }

    [ObservableProperty] private string _problem = "";

    public void RefreshNumber() => OnPropertyChanged(nameof(Number));

    public void RefreshTexts()
    {
        OnPropertyChanged(nameof(KindText));
        OnPropertyChanged(nameof(Apps));
        OnPropertyChanged(nameof(Launch));
    }
}

/// <summary>An action of <c>config.actions</c> in the «Кнопки» list.</summary>
public sealed partial class ActionItem : ObservableObject
{
    public ActionItem(ButtonsViewModel owner, ActionEntry model)
    {
        Owner = owner;
        Model = model;
        foreach (var s in model.Steps ?? new List<MacroStep>()) Steps.Add(new MacroStepItem(this, s));
    }

    public ButtonsViewModel Owner { get; }
    public ActionEntry Model { get; }
    public ObservableCollection<MacroStepItem> Steps { get; } = new();
    public ObservableCollection<CheckItem> DeviceChoices { get; } = new();

    private void Edit(Action change, [System.Runtime.CompilerServices.CallerMemberName] string? prop = null)
    {
        Owner.Session.Checkpoint();
        change();
        OnPropertyChanged(prop);
        Changed();
    }

    /// <summary>Windows line breaks become «\n»: the agent types every line break as one Enter.</summary>
    public static string NormalizeText(string value) => value.Replace("\r\n", "\n").Replace('\r', '\n');

    public string Id => Model.Id;
    public string Title { get => Model.Title; set => Edit(() => Model.Title = value.Trim()); }
    public string Kind => Model.Kind;
    public string KindText => Loc.T("ActionKind_" + Model.Kind);
    public bool IsKeys => Model.Kind == ActionEntry.KindKeys;
    public bool IsText => Model.Kind == ActionEntry.KindText;
    public bool IsMacro => Model.Kind == ActionEntry.KindMacro;
    public bool IsMic => Model.Kind == ActionEntry.KindMic;
    public bool IsAudioOut => Model.Kind == ActionEntry.KindAudioOut;
    public bool IsDiscord => Model.Id.StartsWith("discord-", StringComparison.Ordinal);

    public string KindGlyph => Model.Kind switch
    {
        ActionEntry.KindText => "",
        ActionEntry.KindMacro => "",
        ActionEntry.KindMic => "",
        ActionEntry.KindAudioOut => "",
        _ => "",
    };

    public string Keys { get => Model.Keys ?? ""; set => Edit(() => Model.Keys = Hotkey.Normalize(value) ?? value); }
    public string Text { get => Model.Text ?? ""; set => Edit(() => Model.Text = NormalizeText(value)); }
    public string TextCounter => Loc.F("Buttons_TextCounter", Model.Text?.Length ?? 0, ActionEntry.MaxTextLength);

    [RelayCommand]
    private void ClearKeys() => Keys = "";

    public string Summary => Model.Kind switch
    {
        ActionEntry.KindKeys => string.IsNullOrEmpty(Model.Keys) ? Loc.T("Buttons_NoKeys") : Model.Keys,
        ActionEntry.KindText => FirstLine(Model.Text),
        ActionEntry.KindMacro => Loc.F("Buttons_StepsCount", Model.Steps?.Count ?? 0),
        ActionEntry.KindMic => Loc.T("Buttons_MicSummary"),
        ActionEntry.KindAudioOut => Loc.F("Buttons_DevicesCount", Model.Devices?.Count ?? 0),
        _ => Model.Kind,
    };

    private static string FirstLine(string? s)
    {
        if (string.IsNullOrEmpty(s)) return Loc.T("Buttons_NoText");
        int nl = s.IndexOf('\n');
        return nl < 0 ? s : s[..nl] + " …";
    }

    public bool FewDevices => IsAudioOut && (Model.Devices?.Count ?? 0) < 2;

    [ObservableProperty] private string _problem = "";

    public bool HasProblem => Problem.Length > 0;
    partial void OnProblemChanged(string value) => OnPropertyChanged(nameof(HasProblem));

    private void Changed()
    {
        OnPropertyChanged(nameof(Summary));
        OnPropertyChanged(nameof(TextCounter));
        OnPropertyChanged(nameof(FewDevices));
        Owner.RefreshProblems();
    }

    public void StepsChanged()
    {
        for (int i = 0; i < Steps.Count; i++) Steps[i].RefreshNumber();
        Changed();
    }

    // ---------- audioOut devices ----------

    /// <summary>Checkboxes for the outputs the agent reported; ids in config that are not connected now stay checked.</summary>
    public void FillDevices(IReadOnlyList<AudioDeviceInfo>? outputs)
    {
        DeviceChoices.Clear();
        var selected = Model.Devices ?? new List<string>();
        foreach (var d in outputs ?? Array.Empty<AudioDeviceInfo>())
        {
            string title = d.Default ? Loc.F("Buttons_DeviceDefault", d.Name) : d.Name;
            DeviceChoices.Add(new CheckItem(d.Id, title, selected.Contains(d.Id), OnDeviceChecked));
        }
        foreach (var id in selected.Where(id => DeviceChoices.All(c => c.Id != id)))
            DeviceChoices.Add(new CheckItem(id, Loc.F("Buttons_DeviceMissing", id), true, OnDeviceChecked));
    }

    private void OnDeviceChecked(CheckItem item)
    {
        Owner.Session.Checkpoint();
        var list = Model.Devices ??= new List<string>();
        list.Remove(item.Id);
        if (item.IsChecked) list.Add(item.Id);
        Changed();
    }

    public void RefreshTexts()
    {
        foreach (var p in new[] { nameof(KindText), nameof(Summary), nameof(TextCounter) }) OnPropertyChanged(p);
        foreach (var s in Steps) s.RefreshTexts();
    }

    // ListBoxItem automation name (screen readers, UI tests) — otherwise it is the type name.
    public override string ToString() => Title;
}

/// <summary>
/// «Кнопки» (spec 1.1 §9): actions for the «Кнопки» block — key combinations recorded from the keyboard, text,
/// chains of steps, microphone, output device switching; Discord template.
/// </summary>
public sealed partial class ButtonsViewModel : SectionBase
{
    private AudioDevicesResponse? _devices;

    public ButtonsViewModel(EditorSession session) : base(session) { }

    public ObservableCollection<ActionItem> Actions { get; } = new();

    [ObservableProperty] private ActionItem? _selected;
    [ObservableProperty] private string _message = "";
    [ObservableProperty] private string _devicesMessage = "";
    [ObservableProperty] private bool _loadingDevices;

    public bool HasActions => Actions.Count > 0;

    /// <summary>Apps of «Приложения» for the «Запустить приложение» step.</summary>
    public IReadOnlyList<Choice> AppChoices => Session.Config.Apps.Select(a => new Choice(a.Id, string.IsNullOrEmpty(a.Title) ? a.Id : a.Title)).ToList();

    public IReadOnlyList<Choice> StepKinds => MacroStep.Kinds.Select(k => new Choice(k, Loc.T("StepKind_" + k))).ToList();

    public override void Rebuild()
    {
        string? keep = Selected?.Id;
        Actions.Clear();
        foreach (var a in Session.Config.Actions ?? new List<ActionEntry>()) Actions.Add(new ActionItem(this, a));
        Selected = Actions.FirstOrDefault(a => a.Id == keep) ?? Actions.FirstOrDefault();
        OnPropertyChanged(nameof(HasActions));
        OnPropertyChanged(nameof(AppChoices));
        OnPropertyChanged(nameof(StepKinds));
        foreach (var a in Actions) a.RefreshTexts();
        FillDevices();
        RefreshProblems();
    }

    public override void Activated()
    {
        // «Приложения» may have changed meanwhile: launch steps pick from that list.
        OnPropertyChanged(nameof(AppChoices));
        foreach (var a in Actions) a.RefreshTexts();
        if (Actions.Any(a => a.IsAudioOut)) _ = LoadDevicesAsync();
    }

    partial void OnSelectedChanged(ActionItem? value)
    {
        if (value is { IsAudioOut: true } && _devices is null && !LoadingDevices) _ = LoadDevicesAsync();
    }

    /// <summary>Marks every action the agent would refuse (spec 1.1 §3 rules).</summary>
    public void RefreshProblems()
    {
        var issues = ActionValidator.Validate(Session.Config);
        foreach (var a in Actions)
        {
            var mine = issues.Where(i => i.ActionId == a.Id).ToList();
            a.Problem = string.Join("\n", mine.Where(i => i.Step is null).Select(i => Loc.T("ActionIssue_" + i.Kind)).Distinct());
            for (int s = 0; s < a.Steps.Count; s++)
                a.Steps[s].Problem = string.Join("\n", mine.Where(i => i.Step == s).Select(i => Loc.T("ActionIssue_" + i.Kind)).Distinct());
            if (a.Problem.Length == 0 && a.Steps.Any(s => s.Problem.Length > 0)) a.Problem = Loc.T("ActionIssue_steps");
        }
    }

    // ---------- add / delete / order ----------

    private IEnumerable<string> Ids => (Session.Config.Actions ?? new List<ActionEntry>()).Select(a => a.Id);

    private ActionItem Add(ActionEntry entry)
    {
        Session.Checkpoint();
        (Session.Config.Actions ??= new List<ActionEntry>()).Add(entry);
        var item = new ActionItem(this, entry);
        Actions.Add(item);
        Selected = item;
        OnPropertyChanged(nameof(HasActions));
        if (item.IsAudioOut) FillDevices();
        RefreshProblems();
        return item;
    }

    private ActionEntry New(string kind, string? idBase = null)
    {
        string title = Loc.T("ActionKind_" + kind);
        return new ActionEntry { Id = StartMenuScanner.MakeId(idBase ?? kind, Ids), Title = title, Kind = kind };
    }

    [RelayCommand]
    private void AddKeys()
    {
        var e = New(ActionEntry.KindKeys);
        e.Keys = "";
        Add(e);
        Message = Loc.T("Buttons_RecordHint");
    }

    [RelayCommand]
    private void AddText()
    {
        var e = New(ActionEntry.KindText);
        e.Text = "";
        Add(e);
        Message = "";
    }

    [RelayCommand]
    private void AddMacro()
    {
        var e = New(ActionEntry.KindMacro, "chain");
        e.Steps = new List<MacroStep> { MacroStep.Create(MacroStep.KindKeys) };
        Add(e);
        Message = "";
    }

    [RelayCommand]
    private void AddMic()
    {
        Add(New(ActionEntry.KindMic));
        Message = "";
    }

    [RelayCommand]
    private void AddAudioOut()
    {
        var e = New(ActionEntry.KindAudioOut, "audio-out");
        e.Devices = new List<string>();
        Add(e);
        Message = "";
    }

    /// <summary>Two actions with Discord's default shortcuts: toggle mute and toggle deafen.</summary>
    [RelayCommand]
    private void AddDiscord()
    {
        Session.Checkpoint();
        var list = Session.Config.Actions ??= new List<ActionEntry>();
        ActionItem? first = null;
        foreach (var (idBase, titleKey, keys) in new[]
                 {
                     ("discord-mic", "Buttons_DiscordMic", "Ctrl+Shift+M"),
                     ("discord-sound", "Buttons_DiscordSound", "Ctrl+Shift+D"),
                 })
        {
            var entry = new ActionEntry
            {
                Id = StartMenuScanner.MakeId(idBase, Ids),
                Title = Loc.T(titleKey),
                Kind = ActionEntry.KindKeys,
                Keys = keys,
            };
            list.Add(entry);
            var item = new ActionItem(this, entry);
            Actions.Add(item);
            first ??= item;
        }
        Selected = first;
        OnPropertyChanged(nameof(HasActions));
        RefreshProblems();
        Message = Loc.T("Buttons_DiscordAdded");
    }

    [RelayCommand]
    private void Delete(ActionItem? item)
    {
        item ??= Selected;
        if (item is null) return;
        Session.Checkpoint();
        Session.Config.Actions?.Remove(item.Model);
        int i = Actions.IndexOf(item);
        Actions.Remove(item);
        Selected = Actions.Count == 0 ? null : Actions[Math.Min(i, Actions.Count - 1)];
        OnPropertyChanged(nameof(HasActions));
        RefreshProblems();
    }

    [RelayCommand]
    private void MoveUp(ActionItem? item) => Move(item ?? Selected, -1);

    [RelayCommand]
    private void MoveDown(ActionItem? item) => Move(item ?? Selected, +1);

    private void Move(ActionItem? item, int delta)
    {
        if (item is null) return;
        int i = Actions.IndexOf(item);
        MoveTo(i, i + delta);
    }

    /// <summary>Drag-and-drop reorder of the list (and of config.actions).</summary>
    public void MoveTo(int from, int to)
    {
        var list = Session.Config.Actions;
        if (list is null || from < 0 || to < 0 || from >= Actions.Count || to >= Actions.Count || from == to) return;
        Session.Checkpoint();
        var item = Actions[from];
        list.Remove(item.Model);
        list.Insert(to, item.Model);
        Actions.Move(from, to);
        Selected = item;
    }

    // ---------- macro steps ----------

    [RelayCommand]
    private void AddStep(string? kind)
    {
        var a = Selected;
        if (a is null || !a.IsMacro || kind is null) return;
        var steps = a.Model.Steps ??= new List<MacroStep>();
        if (steps.Count >= ActionEntry.MaxSteps)
        {
            Message = Loc.F("Buttons_MaxSteps", ActionEntry.MaxSteps);
            return;
        }
        Session.Checkpoint();
        var step = MacroStep.Create(kind);
        if (kind == MacroStep.KindLaunch) step.Launch = Session.Config.Apps.FirstOrDefault()?.Id ?? "";
        steps.Add(step);
        a.Steps.Add(new MacroStepItem(a, step));
        a.StepsChanged();
        Message = "";
    }

    [RelayCommand]
    private void DeleteStep(MacroStepItem? step)
    {
        var a = Selected;
        if (a is null || step is null) return;
        Session.Checkpoint();
        a.Model.Steps?.Remove(step.Model);
        a.Steps.Remove(step);
        a.StepsChanged();
    }

    [RelayCommand]
    private void StepUp(MacroStepItem? step) => MoveStep(step, -1);

    [RelayCommand]
    private void StepDown(MacroStepItem? step) => MoveStep(step, +1);

    private void MoveStep(MacroStepItem? step, int delta)
    {
        var a = Selected;
        if (a is null || step is null || a.Model.Steps is null) return;
        int i = a.Steps.IndexOf(step), j = i + delta;
        if (i < 0 || j < 0 || j >= a.Steps.Count) return;
        Session.Checkpoint();
        a.Model.Steps.RemoveAt(i);
        a.Model.Steps.Insert(j, step.Model);
        a.Steps.Move(i, j);
        a.StepsChanged();
    }

    // ---------- output devices (GET /admin/audio/devices) ----------

    [RelayCommand]
    private async Task LoadDevicesAsync()
    {
        if (!Session.AgentOnline)
        {
            _devices = null;
            DevicesMessage = Loc.T("Buttons_DevicesOffline");
            FillDevices();
            return;
        }
        LoadingDevices = true;
        try
        {
            _devices = await Session.Admin.GetAudioDevicesAsync();
            DevicesMessage = _devices.Outputs.Count == 0 ? Loc.T("Buttons_DevicesNone") : "";
        }
        catch (AdminException e)
        {
            _devices = null;
            DevicesMessage = e.AgentTooOld ? Loc.T("Agent_TooOld") : e.AgentUnreachable ? Loc.T("Buttons_DevicesOffline") : e.Message;
        }
        finally { LoadingDevices = false; }
        FillDevices();
    }

    private void FillDevices()
    {
        foreach (var a in Actions.Where(a => a.IsAudioOut)) a.FillDevices(_devices?.Outputs);
    }
}
