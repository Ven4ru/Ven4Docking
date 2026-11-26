using System.Globalization;
using System.Windows;
using System.Windows.Data;
using Ven4Docking.Editor.ViewModels;

namespace Ven4Docking.Editor.Services;

/// <summary>null / empty string / false / 0 → Collapsed; anything else → Visible. Parameter "invert" flips it.</summary>
public sealed class VisibleWhen : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        bool on = value switch
        {
            null => false,
            bool b => b,
            string s => s.Length > 0,
            int i => i != 0,
            double d => d > 0,
            _ => true,
        };
        if (parameter as string == "invert") on = !on;
        return on ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => throw new NotSupportedException();
}

public sealed class Not : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture) => value is not true;
    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => value is not true;
}

/// <summary>Brush resource key by state: ok → StatusSuccess, error → StatusDanger, warning → StatusWarning.</summary>
public sealed class StateBrush : IValueConverter
{
    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        string key = value switch
        {
            true => "StatusSuccess",
            false => "StatusDanger",
            SetupState.Ok => "StatusSuccess",
            SetupState.Failed or SetupState.Denied => "StatusDanger",
            SetupState.Manual or SetupState.Warning => "StatusWarning",
            SetupState.Running => "AccentColor",
            _ => "TextSecondary",
        };
        return Application.Current.TryFindResource(key);
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => throw new NotSupportedException();
}

/// <summary>ConverterParameter = key prefix; value = key suffix: ("Template_", "classic") → Loc.T("Template_classic").</summary>
public sealed class LocKey : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        Loc.T((parameter as string ?? "") + value);

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => throw new NotSupportedException();
}

/// <summary>Formats a value through a Loc key: ConverterParameter = key with {0}.</summary>
public sealed class LocFormat : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        Loc.F(parameter as string ?? "", value);

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => throw new NotSupportedException();
}
