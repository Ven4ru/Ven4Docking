using System.Text;
using Ven4Docking.Core.Models;
using Ven4Docking.Core.Validation;

namespace Ven4Docking.Core.Storage;

/// <summary>
/// File fallback used when the agent is not running: config.json and layouts/&lt;phone&gt;.json in
/// <see cref="EditorPaths.Home"/>. Writes are atomic (temp file + replace).
/// </summary>
public sealed class LocalStore
{
    private static readonly UTF8Encoding Utf8NoBom = new(false);

    public LocalStore(EditorPaths paths) => Paths = paths;

    public EditorPaths Paths { get; }

    public DockConfig LoadConfig()
    {
        if (!File.Exists(Paths.ConfigFile)) return new DockConfig();
        return JsonDefaults.Deserialize<DockConfig>(File.ReadAllText(Paths.ConfigFile)) ?? new DockConfig();
    }

    public void SaveConfig(DockConfig config) => WriteAtomic(Paths.ConfigFile, JsonDefaults.Serialize(config));

    public PanelLayout? LoadLayout(string phoneId)
    {
        string file = Paths.LayoutFile(phoneId);
        return File.Exists(file) ? JsonDefaults.Deserialize<PanelLayout>(File.ReadAllText(file)) : null;
    }

    /// <summary>Validates, bumps <c>version</c> past the stored one, writes. Same contract as PUT /admin/layouts.</summary>
    public SaveResult SaveLayout(string phoneId, PanelLayout layout)
    {
        var issues = LayoutValidator.Validate(layout);
        if (issues.Count > 0)
        {
            var first = issues[0];
            return new SaveResult(false, first.Kind, issues.SelectMany(i => i.BlockIds).Distinct().ToList());
        }

        int stored = LoadLayout(phoneId)?.Version ?? 0;
        layout.Version = Math.Max(stored, layout.Version) + 1;
        WriteAtomic(Paths.LayoutFile(phoneId), JsonDefaults.Serialize(layout));
        return SaveResult.Success(layout.Version);
    }

    public string? ReadAdminKey()
    {
        try
        {
            return File.Exists(Paths.AdminKeyFile) ? File.ReadAllText(Paths.AdminKeyFile).Trim() : null;
        }
        catch (IOException) { return null; }
        catch (UnauthorizedAccessException) { return null; }
    }

    public static void WriteAtomic(string path, string content)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        string tmp = path + ".tmp";
        File.WriteAllText(tmp, content, Utf8NoBom);
        File.Move(tmp, path, overwrite: true);
    }
}
