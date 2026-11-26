namespace Ven4Docking.Core.Models;

[Flags]
public enum HotkeyModifiers
{
    None = 0,
    Ctrl = 1,
    Alt = 2,
    Shift = 4,
    Win = 8,
}

/// <summary>
/// A key combination as the agent parses it (spec 1.1 §3): modifiers Ctrl, Alt, Shift, Win and exactly one key,
/// separated by «+», case-insensitive. <see cref="ToString"/> writes the canonical form «Ctrl+Alt+Shift+Win+Key».
/// </summary>
public sealed record Hotkey(HotkeyModifiers Modifiers, string Key)
{
    private static readonly (HotkeyModifiers Flag, string Name)[] ModifierNames =
    {
        (HotkeyModifiers.Ctrl, "Ctrl"), (HotkeyModifiers.Alt, "Alt"), (HotkeyModifiers.Shift, "Shift"), (HotkeyModifiers.Win, "Win"),
    };

    /// <summary>Key name → Windows virtual-key code, in the spelling of spec 1.1 §3.</summary>
    public static readonly IReadOnlyDictionary<string, int> Keys = BuildKeys();

    private static readonly Dictionary<string, string> CanonicalByUpper =
        Keys.Keys.ToDictionary(k => k.ToUpperInvariant(), k => k, StringComparer.Ordinal);

    private static readonly Dictionary<int, string> NameByVk = Keys.ToDictionary(p => p.Value, p => p.Key);

    private static Dictionary<string, int> BuildKeys()
    {
        var map = new Dictionary<string, int>(StringComparer.Ordinal);
        for (char c = 'A'; c <= 'Z'; c++) map[c.ToString()] = c;
        for (char c = '0'; c <= '9'; c++) map[c.ToString()] = c;
        for (int i = 1; i <= 24; i++) map["F" + i] = 0x6F + i;
        map["Enter"] = 0x0D;
        map["Esc"] = 0x1B;
        map["Tab"] = 0x09;
        map["Space"] = 0x20;
        map["Backspace"] = 0x08;
        map["Delete"] = 0x2E;
        map["Insert"] = 0x2D;
        map["Home"] = 0x24;
        map["End"] = 0x23;
        map["PageUp"] = 0x21;
        map["PageDown"] = 0x22;
        map["Up"] = 0x26;
        map["Down"] = 0x28;
        map["Left"] = 0x25;
        map["Right"] = 0x27;
        map["PrintScreen"] = 0x2C;
        map["Pause"] = 0x13;
        for (int i = 0; i <= 9; i++) map["NumPad" + i] = 0x60 + i;
        return map;
    }

    public override string ToString() => Format(Modifiers, Key);

    public static string Format(HotkeyModifiers modifiers, string key) =>
        string.Join('+', ModifierNames.Where(m => modifiers.HasFlag(m.Flag)).Select(m => m.Name).Append(key));

    /// <summary>Modifiers only, for the recorder while the user still holds them: «Ctrl+Shift+».</summary>
    public static string FormatPartial(HotkeyModifiers modifiers) =>
        modifiers == HotkeyModifiers.None ? "" : string.Concat(ModifierNames.Where(m => modifiers.HasFlag(m.Flag)).Select(m => m.Name + "+"));

    public static bool TryParse(string? text, out Hotkey hotkey)
    {
        hotkey = null!;
        if (string.IsNullOrWhiteSpace(text)) return false;
        // Like the agent: parts in any order, each modifier once, exactly one key.
        var mods = HotkeyModifiers.None;
        string? key = null;
        foreach (var raw in text.Split('+'))
        {
            string part = raw.Trim();
            var flag = ParseModifier(part);
            if (flag != HotkeyModifiers.None)
            {
                if (mods.HasFlag(flag)) return false;
                mods |= flag;
            }
            else if (key is null && CanonicalByUpper.TryGetValue(part.ToUpperInvariant(), out var name)) key = name;
            else return false;
        }
        if (key is null) return false;
        hotkey = new Hotkey(mods, key);
        return true;
    }

    public static Hotkey? Parse(string? text) => TryParse(text, out var h) ? h : null;

    /// <summary>The canonical spelling, or null when the agent would not accept the string.</summary>
    public static string? Normalize(string? text) => TryParse(text, out var h) ? h.ToString() : null;

    private static HotkeyModifiers ParseModifier(string name) => name.ToUpperInvariant() switch
    {
        "CTRL" => HotkeyModifiers.Ctrl,
        "ALT" => HotkeyModifiers.Alt,
        "SHIFT" => HotkeyModifiers.Shift,
        "WIN" => HotkeyModifiers.Win,
        _ => HotkeyModifiers.None,
    };

    /// <summary>Key name of a Windows virtual-key code; null for modifiers and keys the agent does not know.</summary>
    public static string? KeyName(int virtualKey) => NameByVk.TryGetValue(virtualKey, out var name) ? name : null;

    /// <summary>Shift, Ctrl, Alt (and their left/right codes) and both Win keys.</summary>
    public static bool IsModifierKey(int virtualKey) =>
        virtualKey is 0x10 or 0x11 or 0x12 or 0x5B or 0x5C or >= 0xA0 and <= 0xA5;
}
