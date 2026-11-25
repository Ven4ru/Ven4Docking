using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;

namespace Ven4Docking.Editor.Themes;

/// <summary>
/// Palettes copied from Ven4Tools/Services/ThemeService.cs (same author, MIT): «Бирюзовая» (teal),
/// «Зелёная» (the «Как на ven4tools.ru» palette, green accent) and «Светлая». Keys match the Ven4Tools client so
/// styles look the same; derived shades are computed the same way.
/// </summary>
public static class ThemeManager
{
    public const string Teal = "teal";
    public const string Green = "green";
    public const string Light = "light";

    public static IReadOnlyList<string> All { get; } = new[] { Teal, Green, Light };

    public static string Current { get; private set; } = Teal;

    public static event Action? ThemeChanged;

    private sealed record Palette(Color Window, Color Sidebar, Color Content, Color Card, Color Raised,
        Color TextPrimary, Color TextSecondary, Color Border, Color Accent, Color Success, Color Warning, Color Danger, Color Info);

    private static Palette For(string? theme) => theme switch
    {
        Green => new Palette(Rgb(0x0A, 0x16, 0x28), Rgb(0x0D, 0x1F, 0x35), Rgb(0x08, 0x12, 0x20), Rgb(0x10, 0x1E, 0x34), Rgb(0x16, 0x29, 0x4A),
            Rgb(0xE8, 0xF0, 0xFE), Rgb(0x8A, 0x9B, 0xB5), Rgb(0x1E, 0x32, 0x50), Rgb(0x4A, 0xDE, 0x80),
            Rgb(0x4A, 0xDE, 0x80), Rgb(0xFB, 0xBF, 0x24), Rgb(0xF8, 0x71, 0x71), Rgb(0x38, 0xBD, 0xF8)),
        Light => new Palette(Rgb(0xF0, 0xF0, 0xF0), Rgb(0xF8, 0xF8, 0xF8), Rgb(0xF5, 0xF5, 0xF5), Rgb(0xFF, 0xFF, 0xFF), Rgb(0xE9, 0xE9, 0xE9),
            Rgb(0x1E, 0x1E, 0x1E), Rgb(0x64, 0x64, 0x64), Rgb(0xDC, 0xDC, 0xDC), Rgb(0x00, 0x78, 0xD4),
            Rgb(0x15, 0x7F, 0x35), Rgb(0xA4, 0x5A, 0x00), Rgb(0xC6, 0x28, 0x28), Rgb(0x03, 0x69, 0xA1)),
        _ => new Palette(Rgb(0x0A, 0x0A, 0x14), Rgb(0x0D, 0x10, 0x18), Rgb(0x0A, 0x0A, 0x14), Rgb(0x11, 0x16, 0x1F), Rgb(0x15, 0x1C, 0x27),
            Rgb(0xE2, 0xE8, 0xF0), Rgb(0xAA, 0xB8, 0xCE), Rgb(0x1E, 0x2A, 0x38), Rgb(0x00, 0xBC, 0xD4),
            Rgb(0x4A, 0xDE, 0x80), Rgb(0xFB, 0xBF, 0x24), Rgb(0xF8, 0x71, 0x71), Rgb(0x38, 0xBD, 0xF8)),
    };

    public static bool IsDark(string? theme) => theme != Light;

    public static void Apply(string? theme)
    {
        string t = All.Contains(theme) ? theme! : Teal;
        var p = For(t);
        var r = Application.Current.Resources;
        void Set(string key, Color c) => r[key] = Frozen(c);

        Set("WindowBackground", p.Window);
        Set("SidebarBackground", p.Sidebar);
        Set("ContentBackground", p.Content);
        Set("CardBackground", p.Card);
        Set("SurfaceRaised", p.Raised);
        Set("TextPrimary", p.TextPrimary);
        Set("TextSecondary", p.TextSecondary);
        Set("BorderBrush", p.Border);
        Set("AccentColor", p.Accent);
        Set("AccentForeground", ReadableOn(p.Accent));
        Set("AccentPressed", Color.FromRgb((byte)(p.Accent.R * 0.82), (byte)(p.Accent.G * 0.82), (byte)(p.Accent.B * 0.82)));
        Set("AccentHoverBackground", WithAlpha(p.Accent, 0x14));
        Set("AccentSoftBackground", WithAlpha(p.Accent, 0x24));
        Set("AccentSoftBorder", WithAlpha(p.Accent, 0x40));
        Set("StatusSuccess", p.Success);
        Set("StatusWarning", p.Warning);
        Set("StatusDanger", p.Danger);
        Set("StatusInfo", p.Info);
        Set("StatusDangerSoft", WithAlpha(p.Danger, 0x55));
        Set("StatusWarningSoft", WithAlpha(p.Warning, 0x22));
        Set("GridLine", WithAlpha(p.TextSecondary, 0x30));
        Set("BlockFill", WithAlpha(p.Accent, 0x30));
        r["AccentColorValue"] = p.Accent;

        Current = t;
        foreach (Window w in Application.Current.Windows) ApplyTitleBar(w);
        ThemeChanged?.Invoke();
    }

    /// <summary>Dark title bar on Windows 10 20H1+/11 to match dark themes.</summary>
    public static void ApplyTitleBar(Window window)
    {
        var hwnd = new WindowInteropHelper(window).Handle;
        if (hwnd == IntPtr.Zero) return;
        int dark = IsDark(Current) ? 1 : 0;
        DwmSetWindowAttribute(hwnd, 20, ref dark, sizeof(int));
    }

    private static Color Rgb(byte r, byte g, byte b) => Color.FromRgb(r, g, b);

    private static Color WithAlpha(Color c, byte a) => Color.FromArgb(a, c.R, c.G, c.B);

    private static SolidColorBrush Frozen(Color c)
    {
        var b = new SolidColorBrush(c);
        b.Freeze();
        return b;
    }

    private static Color ReadableOn(Color bg)
    {
        double l = Lum(bg);
        double white = (1.05) / (l + 0.05);
        double dark = (l + 0.05) / (Lum(Color.FromRgb(0x06, 0x13, 0x0D)) + 0.05);
        return dark >= white ? Color.FromRgb(0x06, 0x13, 0x0D) : Colors.White;
    }

    private static double Lum(Color c)
    {
        static double Ch(byte v)
        {
            double x = v / 255.0;
            return x <= 0.03928 ? x / 12.92 : Math.Pow((x + 0.055) / 1.055, 2.4);
        }
        return 0.2126 * Ch(c.R) + 0.7152 * Ch(c.G) + 0.0722 * Ch(c.B);
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int value, int size);
}
