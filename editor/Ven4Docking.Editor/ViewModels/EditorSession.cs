using System.Collections.ObjectModel;
using System.IO;
using System.Text.Json.Nodes;
using CommunityToolkit.Mvvm.ComponentModel;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Brands;
using Ven4Docking.Core.Models;
using Ven4Docking.Core.Storage;
using Ven4Docking.Core.Validation;
using Ven4Docking.Editor.Services;

namespace Ven4Docking.Editor.ViewModels;

/// <summary>A phone in the top-bar selector: from the agent, from config.json, or the offline draft.</summary>
public sealed class PhoneChoice
{
    public const string DraftId = "draft";

    public required string Id { get; init; }
    public required string Name { get; init; }
    public PhoneInfo? Info { get; init; }
    public bool IsDraft => Id == DraftId;
    public override string ToString() => Name;
}

/// <summary>
/// Everything the sections edit: config + the current phone's layout, where they are stored (agent API or files in
/// VEN4DOCKING_HOME), the agent status and the undo history («Отменить»).
/// </summary>
public sealed partial class EditorSession : ObservableObject
{
    private readonly Stack<string> _undo = new();
    private BrandCatalog? _brands;
    private AdminClient _admin;
    private bool _restoring;

    public EditorSession(EditorPaths paths)
    {
        Paths = paths;
        Store = new LocalStore(paths);
        _config = Store.LoadConfig();
        _layout = LayoutTemplates.Create(LayoutTemplates.Classic, _config.Language ?? "ru");
        _admin = CreateAdmin(_config.EffectivePort);
    }

    public EditorPaths Paths { get; }
    public LocalStore Store { get; }
    public AdminClient Admin => _admin;
    public HttpClient Http { get; } = new() { Timeout = TimeSpan.FromSeconds(20) };

    public BrandCatalog Brands => _brands ??= BrandCatalog.Load(Path.Combine(AppContext.BaseDirectory, "Assets", "brands.json"));

    [ObservableProperty] private DockConfig _config;
    [ObservableProperty] private PanelLayout _layout;
    [ObservableProperty] private PhoneChoice? _currentPhone;
    [ObservableProperty] private bool _agentOnline;
    [ObservableProperty] private AgentStatus? _agentStatus;
    [ObservableProperty] private bool _isDirty;
    [ObservableProperty] private bool _canUndo;
    [ObservableProperty] private IReadOnlyList<LayoutIssue> _issues = Array.Empty<LayoutIssue>();

    public ObservableCollection<PhoneChoice> Phones { get; } = new();

    /// <summary>Raised after undo or a reload: sections rebuild their view models from <see cref="Config"/>/<see cref="Layout"/>.</summary>
    public event Action? Restored;

    /// <summary>Raised whenever the layout changed (preview refresh, validation).</summary>
    public event Action? LayoutChanged;

    public int Port => Config.EffectivePort;

    /// <summary>The installer of an update was started: the window closes without asking about unsaved edits again.</summary>
    public bool ClosingForUpdate { get; set; }

    public string StorageDescription => AgentOnline
        ? Loc.F("Storage_Agent", Port)
        : Loc.F("Storage_Files", Paths.Home);

    private AdminClient CreateAdmin(int port) => AdminClient.Create(port, Store.ReadAdminKey);

    // ---------- undo ----------

    /// <summary>Call before every user change.</summary>
    public void Checkpoint()
    {
        if (_restoring) return;
        _undo.Push(Snapshot());
        if (_undo.Count > 200) TrimUndo();
        CanUndo = true;
        IsDirty = true;
    }

    private void TrimUndo()
    {
        var keep = _undo.Take(150).Reverse().ToList();
        _undo.Clear();
        foreach (var s in keep) _undo.Push(s);
    }

    public void Undo()
    {
        if (_undo.Count == 0) return;
        var snap = JsonNode.Parse(_undo.Pop())!;
        _restoring = true;
        try
        {
            Config = JsonDefaults.Deserialize<DockConfig>(snap["config"]!.ToJsonString())!;
            Layout = JsonDefaults.Deserialize<PanelLayout>(snap["layout"]!.ToJsonString())!;
        }
        finally { _restoring = false; }
        CanUndo = _undo.Count > 0;
        IsDirty = CanUndo;
        NotifyLayoutChanged();
        Restored?.Invoke();
    }

    private string Snapshot() => new JsonObject
    {
        ["config"] = JsonNode.Parse(JsonDefaults.Serialize(Config, indented: false)),
        ["layout"] = JsonNode.Parse(JsonDefaults.Serialize(Layout, indented: false)),
    }.ToJsonString();

    public void NotifyLayoutChanged()
    {
        Issues = LayoutValidator.Validate(Layout);
        LayoutChanged?.Invoke();
    }

    // ---------- load / save ----------

    public async Task RefreshStatusAsync()
    {
        var status = await Admin.TryGetStatusAsync();
        AgentStatus = status;
        bool online = status is not null;
        if (online != AgentOnline)
        {
            AgentOnline = online;
            OnPropertyChanged(nameof(StorageDescription));
        }
    }

    /// <summary>Loads config, phones and the layout of the selected phone (agent first, files as fallback).</summary>
    public async Task LoadAsync(string? preferPhoneId = null)
    {
        await RefreshStatusAsync();
        DockConfig? cfg = null;
        IReadOnlyList<PhoneInfo> agentPhones = Array.Empty<PhoneInfo>();
        if (AgentOnline)
        {
            try
            {
                cfg = await Admin.GetConfigAsync();
                agentPhones = await Admin.GetPhonesAsync();
            }
            catch (AdminException) { /* stays offline-capable */ }
        }
        Config = cfg ?? Store.LoadConfig();
        if (_admin.BaseAddress.Port != Config.EffectivePort)
        {
            _admin.Dispose();
            _admin = CreateAdmin(Config.EffectivePort);
        }

        Phones.Clear();
        foreach (var p in agentPhones)
            Phones.Add(new PhoneChoice { Id = p.Id, Name = p.Name ?? p.Model ?? p.Id, Info = p });
        foreach (var p in Config.Phones.Where(p => Phones.All(x => x.Id != p.Id)))
            Phones.Add(new PhoneChoice { Id = p.Id, Name = string.IsNullOrEmpty(p.Name) ? p.Id : p.Name });
        if (Phones.Count == 0)
            Phones.Add(new PhoneChoice { Id = PhoneChoice.DraftId, Name = Loc.T("Phone_Draft") });

        var phone = Phones.FirstOrDefault(p => p.Id == preferPhoneId) ?? Phones[0];
        _restoring = true;
        try { CurrentPhone = phone; }
        finally { _restoring = false; }
        await LoadLayoutAsync(phone.Id);
    }

    public async Task LoadLayoutAsync(string phoneId)
    {
        PanelLayout? layout = null;
        if (AgentOnline)
        {
            try { layout = await Admin.GetLayoutAsync(phoneId); }
            catch (AdminException) { }
        }
        layout ??= Store.LoadLayout(phoneId) ?? LayoutTemplates.Create(LayoutTemplates.Classic, Loc.Instance.Language);
        Layout = layout;
        _undo.Clear();
        CanUndo = false;
        IsDirty = false;
        NotifyLayoutChanged();
        Restored?.Invoke();
    }

    /// <summary>Error of <see cref="SaveAsync"/> when an action of «Кнопки» would be refused by the agent.</summary>
    public const string ActionsError = "actions";

    /// <summary>Saves config and the current layout. Refuses (and highlights) an invalid layout, spec §11.</summary>
    public async Task<SaveResult> SaveAsync()
    {
        NotifyLayoutChanged();
        if (Issues.Count > 0)
            return new SaveResult(false, Issues[0].Kind, LayoutValidator.InvalidBlockIds(Issues).ToList());
        var actionIssues = ActionValidator.Validate(Config);
        if (actionIssues.Count > 0)
            return new SaveResult(false, ActionsError, actionIssues.Select(i => i.ActionId).Distinct().ToList());

        string phoneId = CurrentPhone?.Id ?? PhoneChoice.DraftId;
        SaveResult result;
        if (AgentOnline)
        {
            try
            {
                await Admin.PutConfigAsync(Config);
                result = await Admin.PutLayoutAsync(phoneId, Layout);
                if (result.Ok && result.NewVersion is int v) Layout.Version = v;
                else if (result.Ok) Layout.Version++;
            }
            catch (AdminException e) when (e.AgentUnreachable)
            {
                AgentOnline = false;
                OnPropertyChanged(nameof(StorageDescription));
                return SaveToFiles(phoneId);
            }
        }
        else
        {
            result = SaveToFiles(phoneId);
        }
        if (result.Ok) IsDirty = false;
        return result;
    }

    private SaveResult SaveToFiles(string phoneId)
    {
        Store.SaveConfig(Config);
        var r = Store.SaveLayout(phoneId, Layout);
        if (r.Ok) IsDirty = false;
        return r;
    }

    partial void OnConfigChanged(DockConfig value) => OnPropertyChanged(nameof(Port));
}
