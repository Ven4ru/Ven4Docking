using System.Collections.ObjectModel;
using System.IO;
using System.Text.Json;
using System.Text.Json.Nodes;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Models;
using Ven4Docking.Core.Validation;
using Ven4Docking.Editor.Services;

namespace Ven4Docking.Editor.ViewModels;

public sealed class PaletteItem
{
    public required BlockType Type { get; init; }
    public string Glyph => Type.Glyph;
    public string Title => Loc.T("Block_" + Type.Id);
    public string SizeText => $"{Type.DefaultW}×{Type.DefaultH}";
}

/// <summary>A checkbox bound to one id inside a block option array (apps, servers, sources, buttons).</summary>
public sealed partial class CheckItem : ObservableObject
{
    private readonly Action<CheckItem> _changed;

    public CheckItem(string id, string title, bool isChecked, Action<CheckItem> changed)
    {
        Id = id;
        Title = title;
        _isChecked = isChecked;
        _changed = changed;
    }

    public string Id { get; }
    public string Title { get; }

    [ObservableProperty] private bool _isChecked;

    partial void OnIsCheckedChanged(bool value) => _changed(this);
}

public sealed partial class ScreenItem : ObservableObject
{
    private readonly ScreensViewModel _owner;

    public ScreenItem(ScreensViewModel owner, LayoutScreen model)
    {
        _owner = owner;
        Model = model;
        foreach (var b in model.Blocks) Blocks.Add(new BlockItem(owner, b));
    }

    public LayoutScreen Model { get; }
    public ObservableCollection<BlockItem> Blocks { get; } = new();

    public string Name
    {
        get => Model.Name;
        set
        {
            if (value == Model.Name) return;
            _owner.Session.Checkpoint();
            Model.Name = value;
            OnPropertyChanged();
            _owner.Session.NotifyLayoutChanged();
        }
    }

    public string BlockCountText => Loc.F("Screens_BlockCount", Blocks.Count);

    public void RefreshCount() => OnPropertyChanged(nameof(BlockCountText));

    /// <summary>«Экран программы» (spec 1.1 §4): exe name; empty → a normal screen.</summary>
    public string ForApp
    {
        get => Model.ForApp ?? "";
        set
        {
            string? v = LayoutScreen.NormalizeForApp(value);
            if (v == Model.ForApp)
            {
                OnPropertyChanged();
                return;
            }
            _owner.Session.Checkpoint();
            Model.ForApp = v;
            OnPropertyChanged();
            OnPropertyChanged(nameof(IsProgram));
            _owner.Session.NotifyLayoutChanged();
        }
    }

    public bool IsProgram => Model.IsProgramScreen;
}

/// <summary>A running program offered for «Экран программы» (GET /admin/processes).</summary>
public sealed record ProcessChoice(string Name, string? Title)
{
    public string Display => string.IsNullOrWhiteSpace(Title) ? Name : $"{Name} — {Title}";
}

public sealed partial class BlockItem : ObservableObject
{
    private readonly ScreensViewModel _owner;

    public BlockItem(ScreensViewModel owner, LayoutBlock model)
    {
        _owner = owner;
        Model = model;
    }

    public LayoutBlock Model { get; }
    public string Id => Model.Id;
    public string Type => Model.Type;
    public string Title => Loc.T("Block_" + Model.Type);
    public string Glyph => BlockTypes.Find(Model.Type)?.Glyph ?? "";

    public int X { get => Model.X; set => _owner.SetGeometry(this, value, Model.Y, Model.W, Model.H); }
    public int Y { get => Model.Y; set => _owner.SetGeometry(this, Model.X, value, Model.W, Model.H); }
    public int W { get => Model.W; set => _owner.SetGeometry(this, Model.X, Model.Y, value, Model.H); }
    public int H { get => Model.H; set => _owner.SetGeometry(this, Model.X, Model.Y, Model.W, value); }

    public double PxLeft => Model.X * ScreensViewModel.CellW;
    public double PxTop => Model.Y * ScreensViewModel.CellH;
    public double PxWidth => Model.W * ScreensViewModel.CellW;
    public double PxHeight => Model.H * ScreensViewModel.CellH;

    public string GeometryText => $"{Model.X},{Model.Y} · {Model.W}×{Model.H}";

    [ObservableProperty] private bool _isInvalid;
    [ObservableProperty] private bool _isSelected;

    public void RefreshGeometry()
    {
        foreach (var p in new[] { nameof(X), nameof(Y), nameof(W), nameof(H), nameof(PxLeft), nameof(PxTop), nameof(PxWidth), nameof(PxHeight), nameof(GeometryText) })
            OnPropertyChanged(p);
    }

    public void RefreshTexts()
    {
        OnPropertyChanged(nameof(Title));
    }
}

/// <summary>«Экраны»: screens list, block palette, live preview with the 8×4 grid, block properties.</summary>
public sealed partial class ScreensViewModel : SectionBase
{
    public const double CellW = 100;
    public const double CellH = 90;

    public ScreensViewModel(EditorSession session) : base(session)
    {
        Palette = BlockTypes.All.Select(t => new PaletteItem { Type = t }).ToList();
        session.LayoutChanged += OnLayoutChanged;
        session.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(EditorSession.AgentOnline)) OnPropertyChanged(nameof(PreviewSource));
        };
    }

    public IReadOnlyList<PaletteItem> Palette { get; private set; }
    public ObservableCollection<ScreenItem> Screens { get; } = new();

    [ObservableProperty] private ScreenItem? _selectedScreen;
    [ObservableProperty] private BlockItem? _selectedBlock;
    [ObservableProperty] private string _issuesText = "";
    [ObservableProperty] private string _optsText = "";
    [ObservableProperty] private string _optsError = "";
    [ObservableProperty] private string _message = "";

    public ObservableCollection<CheckItem> OptionChoices { get; } = new();

    public double PreviewWidth => Session.Layout.Grid.Cols * CellW;
    public double PreviewHeight => Session.Layout.Grid.Rows * CellH;
    public int Cols => Session.Layout.Grid.Cols;
    public int Rows => Session.Layout.Grid.Rows;
    public bool HasIssues => Session.Issues.Count > 0;

    public IReadOnlyList<Choice> Templates => LayoutTemplates.All.Select(t => new Choice(t, Loc.T("Template_" + t))).ToList();

    /// <summary>Raised with the layout JSON whenever the preview should re-render.</summary>
    public event Action<string>? PreviewLayout;

    // ---------- preview ----------

    /// <summary>Agent → its own panel; otherwise panel/dist/index.html?demo=1 from the repo or the install dir; null → placeholder.</summary>
    public Uri? PreviewSource
    {
        get
        {
            if (Session.AgentOnline) return new Uri($"http://127.0.0.1:{Session.Port}/?demo=1");
            string? local = FindLocalPanel();
            return local is null ? null : new Uri(new Uri(local).AbsoluteUri + "?demo=1");
        }
    }

    public static string? FindLocalPanel()
    {
        var installed = Path.Combine(AppContext.BaseDirectory, "panel", "index.html");
        if (File.Exists(installed)) return installed;
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        for (int i = 0; i < 8 && dir is not null; i++, dir = dir.Parent)
        {
            var candidate = Path.Combine(dir.FullName, "panel", "dist", "index.html");
            if (File.Exists(candidate)) return candidate;
        }
        return null;
    }

    public string LayoutJson => JsonDefaults.Serialize(Session.Layout, indented: false);

    // ---------- rebuild ----------

    public override void Rebuild()
    {
        string? keepScreen = SelectedScreen?.Model.Id;
        string? keepBlock = SelectedBlock?.Id;
        Palette = BlockTypes.All.Select(t => new PaletteItem { Type = t }).ToList();
        OnPropertyChanged(nameof(Palette));
        OnPropertyChanged(nameof(Templates));
        Screens.Clear();
        foreach (var s in Session.Layout.Screens) Screens.Add(new ScreenItem(this, s));
        SelectedScreen = Screens.FirstOrDefault(s => s.Model.Id == keepScreen) ?? Screens.FirstOrDefault();
        SelectedBlock = SelectedScreen?.Blocks.FirstOrDefault(b => b.Id == keepBlock);
        OnPropertyChanged(nameof(PreviewWidth));
        OnPropertyChanged(nameof(PreviewHeight));
        OnPropertyChanged(nameof(PreviewSource));
        OnLayoutChanged();
    }

    private void OnLayoutChanged()
    {
        var invalid = LayoutValidator.InvalidBlockIds(Session.Issues);
        foreach (var s in Screens)
            foreach (var b in s.Blocks)
                b.IsInvalid = invalid.Contains(b.Id);
        IssuesText = Session.Issues.Count == 0 ? "" : string.Join("\n", Session.Issues.Select(DescribeIssue).Distinct());
        OnPropertyChanged(nameof(HasIssues));
        PreviewLayout?.Invoke(LayoutJson);
    }

    private string DescribeIssue(LayoutIssue i)
    {
        string screen = Session.Layout.Screens.FirstOrDefault(s => s.Id == i.ScreenId)?.Name ?? "";
        return i.Kind switch
        {
            IssueKinds.Overlap => Loc.F("Issue_Overlap", screen, string.Join(" + ", i.BlockIds.Select(BlockName))),
            IssueKinds.Bounds => Loc.F("Issue_Bounds", screen, BlockName(i.BlockIds[0])),
            IssueKinds.Screens => Loc.T("Issue_Screens"),
            IssueKinds.ProgramScreens => Loc.F("Issue_ProgramScreens", LayoutScreen.MaxProgramScreens),
            IssueKinds.NoMainScreen => Loc.T("Issue_NoMainScreen"),
            _ => Loc.F("Issue_Other", i.Kind),
        };
    }

    private string BlockName(string id)
    {
        var b = Session.Layout.AllBlocks().FirstOrDefault(x => x.Id == id);
        return b is null ? id : $"{Loc.T("Block_" + b.Type)} ({id})";
    }

    // ---------- «Экран программы»: running programs (GET /admin/processes) ----------

    public ObservableCollection<ProcessChoice> RunningPrograms { get; } = new();

    [ObservableProperty] private ProcessChoice? _pickedProgram;
    [ObservableProperty] private string _programsMessage = "";

    public override void Activated()
    {
        // «Кнопки», «Приложения», «Серверы» may have changed meanwhile: the option lists come from them.
        LoadOptions();
        _ = LoadProgramsAsync();
    }

    [RelayCommand]
    private async Task LoadProgramsAsync()
    {
        RunningPrograms.Clear();
        if (!Session.AgentOnline)
        {
            ProgramsMessage = Loc.T("Screens_ProgramsOffline");
            return;
        }
        try
        {
            var list = await Session.Admin.GetProcessesAsync();
            foreach (var p in list.Where(p => !string.IsNullOrWhiteSpace(p.Name))
                         .GroupBy(p => p.Name, StringComparer.OrdinalIgnoreCase).Select(g => g.First())
                         .OrderBy(p => p.Name, StringComparer.OrdinalIgnoreCase))
                RunningPrograms.Add(new ProcessChoice(p.Name, p.Title));
            ProgramsMessage = "";
        }
        catch (AdminException e)
        {
            ProgramsMessage = e.AgentTooOld ? Loc.T("Agent_TooOld") : e.AgentUnreachable ? Loc.T("Screens_ProgramsOffline") : e.Message;
        }
    }

    partial void OnPickedProgramChanged(ProcessChoice? value)
    {
        if (value is not null && SelectedScreen is not null) SelectedScreen.ForApp = value.Name;
    }

    partial void OnSelectedScreenChanged(ScreenItem? value)
    {
        PickedProgram = null;
        SelectedBlock = null;
        DeleteScreenCommand.NotifyCanExecuteChanged();
        MoveScreenUpCommand.NotifyCanExecuteChanged();
        MoveScreenDownCommand.NotifyCanExecuteChanged();
        PreviewScreenIndex = value is null ? 0 : Screens.IndexOf(value);
    }

    /// <summary>Which screen the preview should show (posted to the panel together with the layout).</summary>
    [ObservableProperty] private int _previewScreenIndex;

    partial void OnSelectedBlockChanged(BlockItem? oldValue, BlockItem? newValue)
    {
        if (oldValue is not null) oldValue.IsSelected = false;
        if (newValue is not null) newValue.IsSelected = true;
        LoadOptions();
        DeleteBlockCommand.NotifyCanExecuteChanged();
        OnPropertyChanged(nameof(HasSelection));
        OnPropertyChanged(nameof(ClockWeather));
        OnPropertyChanged(nameof(Clock24));
        OnPropertyChanged(nameof(IsClock));
        OnPropertyChanged(nameof(OptionChoicesTitle));
        OnPropertyChanged(nameof(IsNotify));
        OnPropertyChanged(nameof(NotifyMax));
        OnPropertyChanged(nameof(FlagChoicesTitle));
        OnPropertyChanged(nameof(OptionsHint));
    }

    public bool HasSelection => SelectedBlock is not null;

    // ---------- screens ----------

    [RelayCommand]
    private void AddScreen()
    {
        if (Session.Layout.Screens.Count >= PanelLayout.MaxScreens)
        {
            Message = Loc.T("Screens_Max");
            return;
        }
        Session.Checkpoint();
        var s = new LayoutScreen
        {
            Id = LayoutEditing.NextScreenId(Session.Layout),
            Name = Loc.F("Screens_NewName", Session.Layout.Screens.Count + 1),
        };
        Session.Layout.Screens.Add(s);
        var item = new ScreenItem(this, s);
        Screens.Add(item);
        SelectedScreen = item;
        Session.NotifyLayoutChanged();
    }

    private bool CanDeleteScreen() => SelectedScreen is not null && Screens.Count > PanelLayout.MinScreens;

    [RelayCommand(CanExecute = nameof(CanDeleteScreen))]
    private void DeleteScreen()
    {
        if (SelectedScreen is null) return;
        Session.Checkpoint();
        int i = Screens.IndexOf(SelectedScreen);
        Session.Layout.Screens.Remove(SelectedScreen.Model);
        Screens.RemoveAt(i);
        SelectedScreen = Screens[Math.Min(i, Screens.Count - 1)];
        Session.NotifyLayoutChanged();
    }

    private bool CanMoveScreenUp() => SelectedScreen is not null && Screens.IndexOf(SelectedScreen) > 0;
    private bool CanMoveScreenDown() => SelectedScreen is not null && Screens.IndexOf(SelectedScreen) < Screens.Count - 1;

    [RelayCommand(CanExecute = nameof(CanMoveScreenUp))]
    private void MoveScreenUp() => MoveScreen(-1);

    [RelayCommand(CanExecute = nameof(CanMoveScreenDown))]
    private void MoveScreenDown() => MoveScreen(+1);

    private void MoveScreen(int delta)
    {
        if (SelectedScreen is null) return;
        int i = Screens.IndexOf(SelectedScreen), j = i + delta;
        if (j < 0 || j >= Screens.Count) return;
        Session.Checkpoint();
        var item = SelectedScreen;
        Session.Layout.Screens.RemoveAt(i);
        Session.Layout.Screens.Insert(j, item.Model);
        Screens.Move(i, j);
        SelectedScreen = item;
        MoveScreenUpCommand.NotifyCanExecuteChanged();
        MoveScreenDownCommand.NotifyCanExecuteChanged();
        Session.NotifyLayoutChanged();
    }

    [RelayCommand]
    private void ApplyTemplate(string template)
    {
        Session.Checkpoint();
        var t = LayoutTemplates.Create(template, Loc.Instance.Language);
        t.Version = Session.Layout.Version;
        Session.Layout = t;
        Rebuild();
        Session.NotifyLayoutChanged();
    }

    // ---------- blocks (called from the grid overlay) ----------

    public void AddBlockAt(string type, int col, int row)
    {
        if (SelectedScreen is null) return;
        Session.Checkpoint();
        var block = LayoutEditing.AddBlock(Session.Layout, SelectedScreen.Model, type, col, row);
        if (block is null)
        {
            Message = Loc.T("Screens_NoRoom");
            return;
        }
        Message = "";
        var item = new BlockItem(this, block);
        SelectedScreen.Blocks.Add(item);
        SelectedScreen.RefreshCount();
        SelectedBlock = item;
        Session.NotifyLayoutChanged();
    }

    /// <summary>Start of a drag: one undo step per gesture.</summary>
    public void BeginGesture() => Session.Checkpoint();

    public void DragTo(BlockItem block, int col, int row)
    {
        if (block.X == col && block.Y == row) return;
        LayoutEditing.Move(block.Model, Session.Layout.Grid, col, row);
        block.RefreshGeometry();
        Session.NotifyLayoutChanged();
    }

    public void ResizeTo(BlockItem block, int w, int h)
    {
        if (block.W == w && block.H == h) return;
        LayoutEditing.Resize(block.Model, Session.Layout.Grid, w, h);
        block.RefreshGeometry();
        Session.NotifyLayoutChanged();
    }

    /// <summary>From the property panel: one undo step per edit.</summary>
    public void SetGeometry(BlockItem block, int x, int y, int w, int h)
    {
        if (block.X == x && block.Y == y && block.W == w && block.H == h) return;
        Session.Checkpoint();
        var grid = Session.Layout.Grid;
        LayoutEditing.Resize(block.Model, grid, w, h);
        LayoutEditing.Move(block.Model, grid, x, y);
        LayoutEditing.Resize(block.Model, grid, w, h);
        block.RefreshGeometry();
        Session.NotifyLayoutChanged();
    }

    private bool CanDeleteBlock() => SelectedBlock is not null;

    [RelayCommand(CanExecute = nameof(CanDeleteBlock))]
    private void DeleteBlock()
    {
        if (SelectedBlock is null || SelectedScreen is null) return;
        Session.Checkpoint();
        SelectedScreen.Model.Blocks.Remove(SelectedBlock.Model);
        SelectedScreen.Blocks.Remove(SelectedBlock);
        SelectedScreen.RefreshCount();
        SelectedBlock = null;
        Session.NotifyLayoutChanged();
    }

    // ---------- block options ----------

    public bool IsClock => SelectedBlock?.Type == BlockTypes.Clock;

    public bool ClockWeather
    {
        get => GetBool("weather", true);
        set => SetOpt("weather", value);
    }

    public bool Clock24
    {
        get => GetBool("h24", true);
        set => SetOpt("h24", value);
    }

    public string OptionChoicesTitle => SelectedBlock?.Type switch
    {
        BlockTypes.Apps => Loc.T("Opts_Apps"),
        BlockTypes.Servers => Loc.T("Opts_Servers"),
        BlockTypes.Ai => Loc.T("Opts_Sources"),
        BlockTypes.Media => Loc.T("Opts_Buttons"),
        BlockTypes.Power => Loc.T("Opts_PowerButtons"),
        BlockTypes.Keys => Loc.T("Opts_Actions"),
        _ => "",
    };

    /// <summary>On/off parts of «Микшер» and «OBS»: each is a boolean in opts, true when absent.</summary>
    public ObservableCollection<CheckItem> FlagChoices { get; } = new();

    public string FlagChoicesTitle => SelectedBlock?.Type is BlockTypes.Mixer or BlockTypes.Obs ? Loc.T("Opts_Show") : "";

    /// <summary>A note under the options of the new blocks (what the block needs to work).</summary>
    public string OptionsHint => SelectedBlock?.Type switch
    {
        BlockTypes.Power => Loc.T("Opts_PowerHint"),
        BlockTypes.Keys => Loc.T("Opts_KeysHint"),
        BlockTypes.Obs => Loc.T("Opts_ObsHint"),
        BlockTypes.Notify => Loc.T("Opts_NotifyHint"),
        _ => "",
    };

    public bool IsNotify => SelectedBlock?.Type == BlockTypes.Notify;

    /// <summary>«Сколько уведомлений» of the notify block (opts.max).</summary>
    public string NotifyMax
    {
        get => (SelectedBlock?.Model.Opts["max"] is JsonValue v && v.TryGetValue<int>(out int n) ? n : BlockTypes.NotifyDefaultMax).ToString();
        set
        {
            if (!int.TryParse(value, out int n))
            {
                OnPropertyChanged();
                return;
            }
            n = Math.Clamp(n, 1, BlockTypes.NotifyMaxLimit);
            SetOpt("max", n);
            OnPropertyChanged();
        }
    }

    private bool GetBool(string key, bool fallback)
    {
        if (SelectedBlock?.Model.Opts[key] is JsonValue v && v.TryGetValue<bool>(out bool b)) return b;
        return fallback;
    }

    private void SetOpt(string key, JsonNode? value)
    {
        if (SelectedBlock is null) return;
        Session.Checkpoint();
        SelectedBlock.Model.Opts[key] = value;
        OptsText = SelectedBlock.Model.Opts.ToJsonString(new JsonSerializerOptions { WriteIndented = true });
        Session.NotifyLayoutChanged();
    }

    private void LoadOptions()
    {
        OptionChoices.Clear();
        FlagChoices.Clear();
        OptsError = "";
        var b = SelectedBlock;
        if (b is null)
        {
            OptsText = "";
            return;
        }
        OptsText = b.Model.Opts.ToJsonString(new JsonSerializerOptions { WriteIndented = true });

        var flags = b.Type switch
        {
            BlockTypes.Mixer => BlockTypes.MixerParts,
            BlockTypes.Obs => BlockTypes.ObsParts,
            _ => Array.Empty<string>(),
        };
        foreach (var f in flags)
        {
            bool on = b.Model.Opts[f] is not JsonValue fv || !fv.TryGetValue<bool>(out bool fb) || fb;
            FlagChoices.Add(new CheckItem(f, Loc.T($"Opts_{b.Type}_{f}"), on, c => SetOpt(c.Id, c.IsChecked)));
        }

        (string Key, IEnumerable<(string Id, string Title)> All)? spec = b.Type switch
        {
            BlockTypes.Apps => ("apps", Session.Config.Apps.Select(a => (a.Id, a.Title))),
            BlockTypes.Servers => ("servers", Session.Config.Servers.Select(s => (s.Id, s.Name))),
            BlockTypes.Ai => ("sources", AiSettings.KnownSources.Select(s => (s, Loc.T("Ai_" + s)))),
            BlockTypes.Media => ("buttons", new[] { "prev", "playpause", "next", "stop", "volup", "voldown", "mute" }.Select(s => (s, Loc.T("Media_" + s)))),
            BlockTypes.Power => ("buttons", BlockTypes.PowerButtons.Select(s => (s, Loc.T("Power_" + s)))),
            BlockTypes.Keys => ("actions", (Session.Config.Actions ?? new List<ActionEntry>())
                .Select(a => (a.Id, string.IsNullOrEmpty(a.Title) ? a.Id : a.Title))),
            _ => null,
        };
        if (spec is null) return;

        string key = spec.Value.Key;
        var selected = (b.Model.Opts[key] as JsonArray)?.Select(n => n?.ToString() ?? "").ToHashSet() ?? new HashSet<string>();
        foreach (var (id, title) in spec.Value.All)
            OptionChoices.Add(new CheckItem(id, title, selected.Contains(id), _ => WriteChoices(key)));
    }

    private void WriteChoices(string key)
    {
        var arr = new JsonArray(OptionChoices.Where(c => c.IsChecked).Select(c => (JsonNode?)JsonValue.Create(c.Id)).ToArray());
        SetOpt(key, arr);
    }

    [RelayCommand]
    private void ApplyOpts()
    {
        if (SelectedBlock is null) return;
        try
        {
            if (JsonNode.Parse(OptsText) is not JsonObject o)
            {
                OptsError = Loc.T("Opts_NotObject");
                return;
            }
            Session.Checkpoint();
            SelectedBlock.Model.Opts = o;
            OptsError = "";
            LoadOptions();
            OnPropertyChanged(nameof(ClockWeather));
            OnPropertyChanged(nameof(Clock24));
            OnPropertyChanged(nameof(NotifyMax));
            Session.NotifyLayoutChanged();
        }
        catch (JsonException e)
        {
            OptsError = e.Message;
        }
    }
}
