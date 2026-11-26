using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using Ven4Docking.Editor.ViewModels;

namespace Ven4Docking.Editor.Views;

public partial class ButtonsView : UserControl
{
    private const string Format = "v4d-action";
    private Point _down;
    private ActionItem? _dragItem;

    public ButtonsView() => InitializeComponent();

    private static T? Ancestor<T>(DependencyObject? d) where T : DependencyObject
    {
        while (d is not null and not T) d = VisualTreeHelper.GetParent(d);
        return d as T;
    }

    private void List_PreviewMouseDown(object sender, MouseButtonEventArgs e)
    {
        _down = e.GetPosition(List);
        _dragItem = Ancestor<ListBoxItem>(e.OriginalSource as DependencyObject)?.DataContext as ActionItem;
    }

    private void List_PreviewMouseMove(object sender, MouseEventArgs e)
    {
        if (_dragItem is null || e.LeftButton != MouseButtonState.Pressed) return;
        var d = e.GetPosition(List) - _down;
        if (Math.Abs(d.Y) < SystemParameters.MinimumVerticalDragDistance * 2) return;
        var item = _dragItem;
        _dragItem = null;
        DragDrop.DoDragDrop(List, new DataObject(Format, item), DragDropEffects.Move);
    }

    private void List_Drop(object sender, DragEventArgs e)
    {
        if (DataContext is not ButtonsViewModel vm || e.Data.GetData(Format) is not ActionItem item) return;
        var target = Ancestor<ListBoxItem>(e.OriginalSource as DependencyObject)?.DataContext as ActionItem;
        int to = target is null ? vm.Actions.Count - 1 : vm.Actions.IndexOf(target);
        vm.MoveTo(vm.Actions.IndexOf(item), to);
    }
}
