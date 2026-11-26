using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using Ven4Docking.Core.Models;
using Ven4Docking.Editor.Services;

namespace Ven4Docking.Editor.Views;

/// <summary>
/// Records a key combination: focus the box and press it. <see cref="Combination"/> gets the string in the agent's
/// format (spec 1.1 §3, «Ctrl+Shift+M»); keys the agent does not know are ignored with a hint.
/// </summary>
public sealed class HotkeyBox : TextBox
{
    public static readonly DependencyProperty CombinationProperty = DependencyProperty.Register(
        nameof(Combination), typeof(string), typeof(HotkeyBox),
        new FrameworkPropertyMetadata("", FrameworkPropertyMetadataOptions.BindsTwoWayByDefault, (d, _) => ((HotkeyBox)d).ShowValue()));

    public HotkeyBox()
    {
        IsReadOnly = true;
        IsReadOnlyCaretVisible = false;
        IsUndoEnabled = false;
        Cursor = Cursors.Hand;
        ContextMenu = null;
        InputMethod.SetIsInputMethodEnabled(this, false);
        SetResourceReference(StyleProperty, typeof(TextBox));
        Loaded += (_, _) => ShowValue();
    }

    public string Combination
    {
        get => (string)GetValue(CombinationProperty);
        set => SetValue(CombinationProperty, value);
    }

    private static Key RealKey(KeyEventArgs e) => e.Key switch
    {
        Key.System => e.SystemKey,
        Key.ImeProcessed => e.ImeProcessedKey,
        Key.DeadCharProcessed => e.DeadCharProcessedKey,
        _ => e.Key,
    };

    private static HotkeyModifiers CurrentModifiers()
    {
        var m = HotkeyModifiers.None;
        var k = Keyboard.Modifiers;
        if (k.HasFlag(ModifierKeys.Control)) m |= HotkeyModifiers.Ctrl;
        if (k.HasFlag(ModifierKeys.Alt)) m |= HotkeyModifiers.Alt;
        if (k.HasFlag(ModifierKeys.Shift)) m |= HotkeyModifiers.Shift;
        if (k.HasFlag(ModifierKeys.Windows) || Keyboard.IsKeyDown(Key.LWin) || Keyboard.IsKeyDown(Key.RWin)) m |= HotkeyModifiers.Win;
        return m;
    }

    protected override void OnPreviewKeyDown(KeyEventArgs e)
    {
        // Every key goes to the recorder: Tab, Enter, Esc and Alt+… must not move focus or reach the window.
        e.Handled = true;
        Record(RealKey(e));
    }

    protected override void OnPreviewKeyUp(KeyEventArgs e)
    {
        e.Handled = true;
        var key = RealKey(e);
        // PrintScreen reaches WPF only as a key-up.
        if (key == Key.Snapshot) { Record(key); return; }
        if (CurrentModifiers() == HotkeyModifiers.None) ShowValue();
    }

    private void Record(Key key)
    {
        int vk = KeyInterop.VirtualKeyFromKey(key);
        var mods = CurrentModifiers();
        if (Hotkey.IsModifierKey(vk))
        {
            Text = Hotkey.FormatPartial(mods);
            return;
        }
        string? name = Hotkey.KeyName(vk);
        if (name is null)
        {
            Text = Loc.T("Hotkey_Unsupported");
            return;
        }
        string value = Hotkey.Format(mods, name);
        if (value == Combination) ShowValue();
        else Combination = value;
    }

    protected override void OnLostKeyboardFocus(KeyboardFocusChangedEventArgs e)
    {
        base.OnLostKeyboardFocus(e);
        ShowValue();
    }

    protected override void OnGotKeyboardFocus(KeyboardFocusChangedEventArgs e)
    {
        base.OnGotKeyboardFocus(e);
        if (string.IsNullOrEmpty(Combination)) Text = Loc.T("Hotkey_Press");
    }

    private void ShowValue() =>
        Text = string.IsNullOrEmpty(Combination) ? (IsKeyboardFocused ? Loc.T("Hotkey_Press") : Loc.T("Hotkey_Empty")) : Combination;
}
