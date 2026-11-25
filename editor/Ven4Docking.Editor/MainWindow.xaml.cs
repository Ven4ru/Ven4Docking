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
        SourceInitialized += (_, _) => ThemeManager.ApplyTitleBar(this);
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        if (DataContext is MainViewModel { Session.IsDirty: true } vm)
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
