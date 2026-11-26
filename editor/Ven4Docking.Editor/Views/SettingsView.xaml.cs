using System.Windows;
using System.Windows.Controls;
using Ven4Docking.Editor.ViewModels;

namespace Ven4Docking.Editor.Views;

public partial class SettingsView : UserControl
{
    public SettingsView() => InitializeComponent();

    /// <summary>The OBS password is encrypted as soon as the box is left and the box is emptied: it is not kept in the clear.</summary>
    private void ObsPassword_LostFocus(object sender, RoutedEventArgs e)
    {
        if (ObsPassword.Password.Length == 0 || DataContext is not SettingsViewModel vm) return;
        vm.SetObsPassword(ObsPassword.Password);
        ObsPassword.Clear();
    }
}
