using System.ComponentModel;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using Microsoft.Web.WebView2.Core;
using Ven4Docking.Editor.Services;
using Ven4Docking.Editor.ViewModels;

namespace Ven4Docking.Editor.Views;

/// <summary>
/// Grid editor: palette drag-and-drop, moving and resizing blocks on the 8×4 grid, and the WebView2 preview that
/// shows the real panel (agent → its panel, else panel/dist/index.html?demo=1, else a built-in skeleton page).
/// The current layout is posted to the page as <c>{"type":"v4d-layout","layout":…,"screen":n}</c>.
/// </summary>
public partial class ScreensView : UserControl
{
    private const string PaletteFormat = "v4d-block-type";

    private ScreensViewModel? _vm;
    private bool _webReady;
    private Point _paletteDown;

    // pointer gesture on a block
    private BlockItem? _dragBlock;
    private bool _dragResize;
    private bool _dragStarted;
    private Point _dragOrigin;
    private (int X, int Y, int W, int H) _dragStart;

    public ScreensView()
    {
        InitializeComponent();
        DataContextChanged += (_, _) => Attach(DataContext as ScreensViewModel);
        Loaded += async (_, _) =>
        {
            Attach(DataContext as ScreensViewModel);
            DrawGrid();
            await InitWebViewAsync();
        };
        Unloaded += (_, _) => Attach(null);
    }

    private void Attach(ScreensViewModel? vm)
    {
        if (ReferenceEquals(vm, _vm)) return;
        if (_vm is not null)
        {
            _vm.PreviewLayout -= OnPreviewLayout;
            _vm.PropertyChanged -= OnVmPropertyChanged;
        }
        _vm = vm;
        if (_vm is not null)
        {
            _vm.PreviewLayout += OnPreviewLayout;
            _vm.PropertyChanged += OnVmPropertyChanged;
        }
    }

    private void OnVmPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(ScreensViewModel.PreviewWidth) or nameof(ScreensViewModel.PreviewHeight)) DrawGrid();
        if (e.PropertyName == nameof(ScreensViewModel.PreviewSource)) _ = NavigateAsync();
        if (e.PropertyName == nameof(ScreensViewModel.PreviewScreenIndex) && _vm is not null) OnPreviewLayout(_vm.LayoutJson);
    }

    private void DrawGrid()
    {
        GridLines.Children.Clear();
        if (_vm is null) return;
        var brush = (Brush)FindResource("GridLine");
        for (int c = 1; c < _vm.Cols; c++)
            GridLines.Children.Add(new Line { X1 = c * ScreensViewModel.CellW, X2 = c * ScreensViewModel.CellW, Y1 = 0, Y2 = _vm.PreviewHeight, Stroke = brush, StrokeThickness = 1, StrokeDashArray = new DoubleCollection { 4, 4 } });
        for (int r = 1; r < _vm.Rows; r++)
            GridLines.Children.Add(new Line { X1 = 0, X2 = _vm.PreviewWidth, Y1 = r * ScreensViewModel.CellH, Y2 = r * ScreensViewModel.CellH, Stroke = brush, StrokeThickness = 1, StrokeDashArray = new DoubleCollection { 4, 4 } });
    }

    // ---------------- WebView2 preview ----------------

    private async Task InitWebViewAsync()
    {
        if (_vm is null || _webReady) return;
        try
        {
            Preview.CreationProperties = new Microsoft.Web.WebView2.Wpf.CoreWebView2CreationProperties
            {
                UserDataFolder = _vm.Session.Paths.WebViewDataDir,
            };
            await Preview.EnsureCoreWebView2Async();
            var core = Preview.CoreWebView2;
            core.Settings.AreDefaultContextMenusEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
            core.Settings.AreDevToolsEnabled = true;
            // The phone renders at twice the preview size (e.g. 1600×720 for an 800×360 preview).
            Preview.ZoomFactor = 0.5;
            core.NavigationCompleted += (_, _) => { if (_vm is not null) OnPreviewLayout(_vm.LayoutJson); };
            _webReady = true;
            await NavigateAsync();
        }
        catch (Exception ex) when (ex is WebView2RuntimeNotFoundException or InvalidOperationException or System.Runtime.InteropServices.COMException)
        {
            NoWebView.Visibility = Visibility.Visible;
            Preview.Visibility = Visibility.Collapsed;
        }
    }

    private Task NavigateAsync()
    {
        if (!_webReady || _vm is null) return Task.CompletedTask;
        var src = _vm.PreviewSource;
        if (src is null) Preview.CoreWebView2.NavigateToString(SkeletonHtml());
        else Preview.CoreWebView2.Navigate(src.AbsoluteUri);
        return Task.CompletedTask;
    }

    private void OnPreviewLayout(string layoutJson)
    {
        if (!_webReady || _vm is null) return;
        // Panel host API (panel/src/main.js): {type:"layout",layout} → Ven4Panel.setLayout, {type:"go",index} → pager.
        var layout = new JsonObject { ["type"] = "layout", ["layout"] = JsonNode.Parse(layoutJson) };
        var go = new JsonObject { ["type"] = "go", ["index"] = _vm.PreviewScreenIndex };
        try
        {
            Preview.CoreWebView2.PostWebMessageAsJson(layout.ToJsonString());
            Preview.CoreWebView2.PostWebMessageAsJson(go.ToJsonString());
        }
        catch (InvalidOperationException) { }
    }

    /// <summary>Shown when no panel build is available: draws the blocks as cards so the layout is still visible.</summary>
    private static string SkeletonHtml()
    {
        var names = new JsonObject();
        foreach (var t in Core.Models.BlockTypes.All) names[t.Id] = Loc.T("Block_" + t.Id);
        string note = JsonSerializer.Serialize(Loc.T("Screens_SkeletonNote"));
        return """
<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;height:100%;background:#0B0E14;font-family:Segoe UI,sans-serif;color:#E2E8F0;overflow:hidden}
#g{position:absolute;left:0;top:0;right:0;bottom:0}
.b{position:absolute;box-sizing:border-box;padding:18px 22px;border-radius:18px;background:#141823;border:1px solid #1E2A38}
.t{font-size:26px;color:#5AA9FF;font-weight:600}.v{font-size:56px;font-weight:700;margin-top:10px;color:#E2E8F0}
#n{position:absolute;right:16px;bottom:10px;font-size:18px;color:#5B6B82}
</style></head><body><div id="g"></div><div id="n"></div><script>
var names=__NAMES__;document.getElementById('n').textContent=__NOTE__;
var sample={clock:'12:45',weather:'+3°',cpu:'12%',gpu:'40°C',ram:'53%',disks:'57%',net:'0.9 MB/s',ai:'15%',apps:'',media:'▶',servers:'49 ms',phone:'82%'};
var cur=null,idx=0;
function render(){if(!cur)return;var l=cur,s=l.screens[idx]||l.screens[0],g=document.getElementById('g');g.innerHTML='';if(!s)return;
var cw=window.innerWidth/l.grid.cols,ch=window.innerHeight/l.grid.rows;
s.blocks.forEach(function(b){var d=document.createElement('div');d.className='b';
d.style.left=(b.x*cw+6)+'px';d.style.top=(b.y*ch+6)+'px';d.style.width=(b.w*cw-12)+'px';d.style.height=(b.h*ch-12)+'px';
d.innerHTML='<div class="t"></div><div class="v"></div>';d.firstChild.textContent=names[b.type]||b.type;d.lastChild.textContent=sample[b.type]||'';g.appendChild(d);});}
window.chrome.webview.addEventListener('message',function(e){var m=e.data||{};if(m.type==='layout'){cur=m.layout;render();}else if(m.type==='go'){idx=m.index|0;render();}});
</script></body></html>
""".Replace("__NAMES__", names.ToJsonString()).Replace("__NOTE__", note);
    }

    // ---------------- palette drag ----------------

    private void Palette_MouseDown(object sender, MouseButtonEventArgs e)
    {
        _paletteDown = e.GetPosition(this);
        if (e.ClickCount == 2 && sender is FrameworkElement { Tag: PaletteItem p })
            _vm?.AddBlockAt(p.Type.Id, 0, 0);
    }

    private void Palette_MouseMove(object sender, MouseEventArgs e)
    {
        if (e.LeftButton != MouseButtonState.Pressed || sender is not FrameworkElement { Tag: PaletteItem p } fe) return;
        var d = e.GetPosition(this) - _paletteDown;
        if (Math.Abs(d.X) < SystemParameters.MinimumHorizontalDragDistance && Math.Abs(d.Y) < SystemParameters.MinimumVerticalDragDistance) return;
        DragDrop.DoDragDrop(fe, new DataObject(PaletteFormat, p.Type.Id), DragDropEffects.Copy);
    }

    private void Overlay_DragOver(object sender, DragEventArgs e)
    {
        e.Effects = e.Data.GetDataPresent(PaletteFormat) ? DragDropEffects.Copy : DragDropEffects.None;
        e.Handled = true;
    }

    private void Overlay_Drop(object sender, DragEventArgs e)
    {
        if (_vm is null || e.Data.GetData(PaletteFormat) is not string type) return;
        var p = e.GetPosition(Overlay);
        _vm.AddBlockAt(type, (int)(p.X / ScreensViewModel.CellW), (int)(p.Y / ScreensViewModel.CellH));
    }

    private void Overlay_MouseDown(object sender, MouseButtonEventArgs e)
    {
        if (ReferenceEquals(e.OriginalSource, Overlay) && _vm is not null) _vm.SelectedBlock = null;
    }

    // ---------------- move / resize ----------------

    private void Block_MouseDown(object sender, MouseButtonEventArgs e)
    {
        if (_vm is null || sender is not FrameworkElement { DataContext: BlockItem block } fe) return;
        _vm.SelectedBlock = block;
        _dragBlock = block;
        _dragResize = (fe.Tag as string) == "resize";
        _dragStarted = false;
        _dragOrigin = e.GetPosition(Overlay);
        _dragStart = (block.X, block.Y, block.W, block.H);
        fe.CaptureMouse();
        e.Handled = true;
    }

    private void Block_MouseMove(object sender, MouseEventArgs e)
    {
        if (_vm is null || _dragBlock is null || e.LeftButton != MouseButtonState.Pressed) return;
        var d = e.GetPosition(Overlay) - _dragOrigin;
        int dx = (int)Math.Round(d.X / ScreensViewModel.CellW);
        int dy = (int)Math.Round(d.Y / ScreensViewModel.CellH);
        if (!_dragStarted)
        {
            if (dx == 0 && dy == 0) return;
            _dragStarted = true;
            _vm.BeginGesture();
        }
        if (_dragResize) _vm.ResizeTo(_dragBlock, _dragStart.W + dx, _dragStart.H + dy);
        else _vm.DragTo(_dragBlock, _dragStart.X + dx, _dragStart.Y + dy);
    }

    private void Block_MouseUp(object sender, MouseButtonEventArgs e)
    {
        (sender as FrameworkElement)?.ReleaseMouseCapture();
        _dragBlock = null;
        e.Handled = true;
    }
}
