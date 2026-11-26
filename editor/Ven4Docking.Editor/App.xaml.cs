using System.Windows;
using Ven4Docking.Core.Storage;
using Ven4Docking.Editor.Themes;
using Ven4Docking.Editor.ViewModels;

namespace Ven4Docking.Editor;

/// <summary>
/// Command line: <c>--section Phones|Screens|Apps|Buttons|Servers|Ai|Notify|Settings|About</c> opens a section,
/// <c>--wizard</c> opens the phone wizard (used for screenshots and by the tray menu of the agent),
/// <c>--update</c> opens «О программе» and checks for a newer release (tray menu of the agent; installing needs the button).
/// Environment: VEN4DOCKING_HOME (data folder), VEN4DOCKING_ADB_ONLY (adb serial prefix guard),
/// VEN4DOCKING_NO_UPDATE_CHECK=1 (no GitHub check), VEN4DOCKING_NO_WEBVIEW=1 (act as if WebView2 Runtime were missing).
/// </summary>
public partial class App : Application
{
    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        DispatcherUnhandledException += (_, ex) =>
        {
            // Keep the editor alive (unsaved edits!) and tell the user what failed.
            MessageBox.Show(ex.Exception.Message, "Ven4Docking", MessageBoxButton.OK, MessageBoxImage.Error);
            ex.Handled = true;
        };
        ThemeManager.Apply(ThemeManager.Teal);

        var session = new EditorSession(EditorPaths.FromEnvironment());
        var vm = new MainViewModel(session);
        var window = new MainWindow { DataContext = vm };
        MainWindow = window;
        window.Show();

        // --update (tray of the agent): open «О программе» and check; installing still waits for the button.
        bool update = e.Args.Contains("--update");
        await vm.InitializeAsync(forceUpdateCheck: update);

        string? section = Arg(e.Args, "--section");
        if (section is not null) vm.Select(section);
        if (update) vm.Select("About");
        if (e.Args.Contains("--wizard"))
        {
            vm.Select("Phones");
            vm.Phones.OpenWizardCommand.Execute(null);
        }
    }

    private static string? Arg(string[] args, string name)
    {
        int i = Array.IndexOf(args, name);
        return i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
    }
}
