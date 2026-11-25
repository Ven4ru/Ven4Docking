using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Windows.Data;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using Ven4Docking.Core.Apps;
using Ven4Docking.Core.Models;
using Ven4Docking.Editor.Services;

namespace Ven4Docking.Editor.ViewModels;

public sealed partial class AppItem : ObservableObject
{
    private readonly AppsViewModel _owner;

    public AppItem(AppsViewModel owner, AppEntry model)
    {
        _owner = owner;
        Model = model;
    }

    public AppEntry Model { get; }

    private void Edit(Action change, [System.Runtime.CompilerServices.CallerMemberName] string? prop = null)
    {
        _owner.Session.Checkpoint();
        change();
        OnPropertyChanged(prop);
    }

    public string Title { get => Model.Title; set => Edit(() => Model.Title = value); }
    public string Target { get => Model.Target; set => Edit(() => Model.Target = value); }
    public string Args { get => Model.Args ?? ""; set => Edit(() => Model.Args = string.IsNullOrEmpty(value) ? null : value); }
    public string Id => Model.Id;
    public string KindText => Loc.T("AppKind_" + Model.Kind);
    public bool HasArgs => Model.Kind is AppEntry.KindCommand or AppEntry.KindExe;

    public ImageSource? Icon
    {
        get
        {
            if (string.IsNullOrEmpty(Model.Icon)) return null;
            string path = Path.Combine(_owner.Session.Paths.Home, Model.Icon.Replace('/', Path.DirectorySeparatorChar));
            if (!File.Exists(path)) return null;
            try
            {
                var bmp = new BitmapImage();
                bmp.BeginInit();
                bmp.CacheOption = BitmapCacheOption.OnLoad;
                bmp.CreateOptions = BitmapCreateOptions.IgnoreImageCache;
                bmp.UriSource = new Uri(path);
                bmp.DecodePixelWidth = 64;
                bmp.EndInit();
                bmp.Freeze();
                return bmp;
            }
            catch (Exception e) when (e is IOException or NotSupportedException) { return null; }
        }
    }

    public string KindGlyph => Model.Kind switch
    {
        AppEntry.KindUrl => "",
        AppEntry.KindCommand => "",
        _ => "",
    };

    public void RefreshIcon() => OnPropertyChanged(nameof(Icon));
}

/// <summary>«Приложения»: launch list from the Start menu, exe, url or command; icons extracted automatically; drag to reorder.</summary>
public sealed partial class AppsViewModel : SectionBase
{
    private IReadOnlyList<StartMenuApp>? _startMenu;

    public AppsViewModel(EditorSession session) : base(session)
    {
        StartMenuView = CollectionViewSource.GetDefaultView(StartMenuApps);
        StartMenuView.Filter = o => o is StartMenuApp a && (string.IsNullOrWhiteSpace(Filter) || a.Name.Contains(Filter.Trim(), StringComparison.CurrentCultureIgnoreCase));
    }

    public ObservableCollection<AppItem> Apps { get; } = new();
    public ObservableCollection<StartMenuApp> StartMenuApps { get; } = new();
    public ICollectionView StartMenuView { get; }

    [ObservableProperty] private AppItem? _selected;
    [ObservableProperty] private bool _isStartMenuOpen;
    [ObservableProperty] private bool _scanning;
    [ObservableProperty] private string _filter = "";
    [ObservableProperty] private string _message = "";

    public bool HasApps => Apps.Count > 0;

    partial void OnFilterChanged(string value) => StartMenuView.Refresh();

    public override void Rebuild()
    {
        string? keep = Selected?.Id;
        Apps.Clear();
        foreach (var a in Session.Config.Apps) Apps.Add(new AppItem(this, a));
        Selected = Apps.FirstOrDefault(a => a.Id == keep);
        OnPropertyChanged(nameof(HasApps));
        // Icons missing (config edited by hand, icons folder cleaned): extract them again in the background.
        foreach (var item in Apps.Where(a => a.Icon is null && a.Model.Kind is AppEntry.KindExe or AppEntry.KindShortcut))
            _ = ExtractIconAsync(item);
    }

    private void Add(AppEntry entry)
    {
        Session.Checkpoint();
        Session.Config.Apps.Add(entry);
        var item = new AppItem(this, entry);
        Apps.Add(item);
        Selected = item;
        OnPropertyChanged(nameof(HasApps));
        if (entry.Kind is AppEntry.KindExe or AppEntry.KindShortcut or AppEntry.KindUrl)
            _ = ExtractIconAsync(item);
    }

    private async Task ExtractIconAsync(AppItem item)
    {
        string source = item.Model.Target;
        string rel = "icons/" + item.Model.Id + ".png";
        string png = Path.Combine(Session.Paths.IconsDir, item.Model.Id + ".png");
        bool ok = await Task.Run(() => File.Exists(source) && IconExtractor.TryExtractPng(source, png));
        if (!ok) return;
        item.Model.Icon = rel;
        item.RefreshIcon();
    }

    private IEnumerable<string> Ids => Session.Config.Apps.Select(a => a.Id);

    [RelayCommand]
    private async Task ShowStartMenu()
    {
        IsStartMenuOpen = !IsStartMenuOpen;
        if (!IsStartMenuOpen || _startMenu is not null) return;
        Scanning = true;
        try
        {
            _startMenu = await Task.Run(() => StartMenuScanner.Scan());
            StartMenuApps.Clear();
            foreach (var a in _startMenu) StartMenuApps.Add(a);
        }
        finally { Scanning = false; }
    }

    [RelayCommand]
    private void AddFromStartMenu(StartMenuApp? app)
    {
        if (app is null) return;
        Add(StartMenuScanner.ToEntry(app, Ids));
        Message = Loc.F("Apps_Added", app.Name);
    }

    [RelayCommand]
    private void AddExe()
    {
        var dlg = new OpenFileDialog { Filter = Loc.T("Apps_ExeFilter") + "|*.exe;*.lnk;*.bat;*.cmd", Title = Loc.T("Apps_AddExe") };
        if (dlg.ShowDialog() != true) return;
        string title = Path.GetFileNameWithoutExtension(dlg.FileName);
        Add(new AppEntry
        {
            Id = StartMenuScanner.MakeId(title, Ids),
            Title = title,
            Kind = dlg.FileName.EndsWith(".lnk", StringComparison.OrdinalIgnoreCase) ? AppEntry.KindShortcut : AppEntry.KindExe,
            Target = dlg.FileName,
        });
    }

    [RelayCommand]
    private void AddUrl()
    {
        string title = Loc.T("Apps_NewUrl");
        Add(new AppEntry { Id = StartMenuScanner.MakeId("link", Ids), Title = title, Kind = AppEntry.KindUrl, Target = "https://" });
    }

    [RelayCommand]
    private void AddCommand()
    {
        string title = Loc.T("Apps_NewCommand");
        Add(new AppEntry { Id = StartMenuScanner.MakeId("command", Ids), Title = title, Kind = AppEntry.KindCommand, Target = "", Args = "" });
    }

    [RelayCommand]
    private void Delete(AppItem? item)
    {
        item ??= Selected;
        if (item is null) return;
        Session.Checkpoint();
        Session.Config.Apps.Remove(item.Model);
        Apps.Remove(item);
        OnPropertyChanged(nameof(HasApps));
    }

    [RelayCommand]
    private void MoveUp(AppItem? item) => Move(item ?? Selected, -1);

    [RelayCommand]
    private void MoveDown(AppItem? item) => Move(item ?? Selected, +1);

    private void Move(AppItem? item, int delta)
    {
        if (item is null) return;
        int i = Apps.IndexOf(item);
        MoveTo(i, i + delta);
    }

    /// <summary>Drag-and-drop reorder (also changes the order of icons in «Приложения» blocks).</summary>
    public void MoveTo(int from, int to)
    {
        if (from < 0 || to < 0 || from >= Apps.Count || to >= Apps.Count || from == to) return;
        Session.Checkpoint();
        var item = Apps[from];
        Session.Config.Apps.RemoveAt(from);
        Session.Config.Apps.Insert(to, item.Model);
        Apps.Move(from, to);
        Selected = item;
    }

    [RelayCommand]
    private async Task RefreshIcon(AppItem? item)
    {
        item ??= Selected;
        if (item is not null) await ExtractIconAsync(item);
    }
}
