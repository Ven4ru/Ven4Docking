using System.Text;
using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Apps;

/// <summary>A program found in the Start menu (.lnk / .url).</summary>
public sealed record StartMenuApp(string Name, string Path, string Folder)
{
    public bool IsUrlShortcut => Path.EndsWith(".url", StringComparison.OrdinalIgnoreCase);
}

/// <summary>
/// Lists Start-menu shortcuts of the current user and of all users. The agent launches a shortcut by
/// ShellExecute of the .lnk itself (kind "shortcut"), so the target does not need to be resolved here.
/// </summary>
public static class StartMenuScanner
{
    private static readonly string[] SkipWords =
    {
        "uninstall", "удалить", "удаление", "deinstall", "readme", "help", "справка", "license", "лицензия", "release notes",
    };

    public static IReadOnlyList<string> DefaultRoots() => new[]
    {
        System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.StartMenu), "Programs"),
        System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu), "Programs"),
    };

    public static IReadOnlyList<StartMenuApp> Scan(IEnumerable<string>? roots = null)
    {
        var byName = new Dictionary<string, StartMenuApp>(StringComparer.OrdinalIgnoreCase);
        foreach (var root in roots ?? DefaultRoots())
        {
            if (!Directory.Exists(root)) continue;
            IEnumerable<string> files;
            try
            {
                files = Directory.EnumerateFiles(root, "*.*", new EnumerationOptions
                {
                    RecurseSubdirectories = true, IgnoreInaccessible = true, MaxRecursionDepth = 4,
                });
            }
            catch (IOException) { continue; }

            foreach (var f in files)
            {
                string ext = System.IO.Path.GetExtension(f);
                if (!ext.Equals(".lnk", StringComparison.OrdinalIgnoreCase) && !ext.Equals(".url", StringComparison.OrdinalIgnoreCase))
                    continue;
                string name = System.IO.Path.GetFileNameWithoutExtension(f);
                if (SkipWords.Any(w => name.Contains(w, StringComparison.OrdinalIgnoreCase))) continue;
                string folder = System.IO.Path.GetRelativePath(root, System.IO.Path.GetDirectoryName(f)!);
                byName.TryAdd(name, new StartMenuApp(name, f, folder == "." ? "" : folder));
            }
        }
        return byName.Values.OrderBy(a => a.Name, StringComparer.CurrentCultureIgnoreCase).ToList();
    }

    /// <summary>Makes a short, unique, ASCII id for config.apps ("Telegram Desktop" → "telegram-desktop").</summary>
    public static string MakeId(string title, IEnumerable<string> existing)
    {
        var sb = new StringBuilder();
        foreach (char ch in title.ToLowerInvariant())
        {
            if (ch is >= 'a' and <= 'z' or >= '0' and <= '9') sb.Append(ch);
            else if (Translit.TryGetValue(ch, out var t)) sb.Append(t);
            else if (sb.Length > 0 && sb[^1] != '-') sb.Append('-');
        }
        string id = sb.ToString().Trim('-');
        if (id.Length > 32) id = id[..32].Trim('-');
        if (id.Length == 0) id = "app";

        var used = existing.ToHashSet(StringComparer.Ordinal);
        if (!used.Contains(id)) return id;
        for (int n = 2; ; n++)
            if (!used.Contains($"{id}-{n}")) return $"{id}-{n}";
    }

    private static readonly Dictionary<char, string> Translit = new()
    {
        ['а'] = "a", ['б'] = "b", ['в'] = "v", ['г'] = "g", ['д'] = "d", ['е'] = "e", ['ё'] = "e", ['ж'] = "zh",
        ['з'] = "z", ['и'] = "i", ['й'] = "y", ['к'] = "k", ['л'] = "l", ['м'] = "m", ['н'] = "n", ['о'] = "o",
        ['п'] = "p", ['р'] = "r", ['с'] = "s", ['т'] = "t", ['у'] = "u", ['ф'] = "f", ['х'] = "h", ['ц'] = "ts",
        ['ч'] = "ch", ['ш'] = "sh", ['щ'] = "sch", ['ъ'] = "", ['ы'] = "y", ['ь'] = "", ['э'] = "e", ['ю'] = "yu", ['я'] = "ya",
    };

    public static AppEntry ToEntry(StartMenuApp app, IEnumerable<string> existingIds) => new()
    {
        Id = MakeId(app.Name, existingIds),
        Title = app.Name,
        Kind = AppEntry.KindShortcut,
        Target = app.Path,
    };
}
