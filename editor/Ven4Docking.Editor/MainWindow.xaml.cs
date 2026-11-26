using System.ComponentModel;
using System.Windows;
using Ven4Docking.Editor.Services;
using Ven4Docking.Editor.Themes;
using Ven4Docking.Editor.ViewModels;

namespace Ven4Docking.Editor;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        FitToWorkArea();
        SourceInitialized += (_, _) => ThemeManager.ApplyTitleBar(this);
    }

    /// 1440×900 does not fit 1600×900 or 1366×768 screens: the title bar ended up above the screen edge
    /// and the bottom under the taskbar. Shrink to the primary work area and centre in it.
    private void FitToWorkArea()
    {
        var wa = SystemParameters.WorkArea;
        if (Width <= wa.Width && Height <= wa.Height) return;
        MinWidth = Math.Min(MinWidth, wa.Width);
        MinHeight = Math.Min(MinHeight, wa.Height);
        Width = Math.Min(Width, wa.Width);
        Height = Math.Min(Height, wa.Height);
        WindowStartupLocation = WindowStartupLocation.Manual;
        Left = wa.Left + (wa.Width - Width) / 2;
        Top = wa.Top + (wa.Height - Height) / 2;
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        if (DataContext is MainViewModel { Session.IsDirty: true, Session.ClosingForUpdate: false } vm)
        {
            var r = MessageBox.Show(this, Loc.T("Ask_SaveOnExit"), "Ven4Docking", MessageBoxButton.YesNoCancel, MessageBoxImage.Question);
            if (r == MessageBoxResult.Cancel)
            {
                e.Cancel = true;
            }
            else if (r == MessageBoxResult.Yes)
            {
                e.Cancel = true;
                _ = SaveThenCloseAsync(vm);
            }
        }
        base.OnClosing(e);
    }

    private async Task SaveThenCloseAsync(MainViewModel vm)
    {
        await vm.SaveCommand.ExecuteAsync(null);
        if (!vm.Session.IsDirty) Close();
    }
}
