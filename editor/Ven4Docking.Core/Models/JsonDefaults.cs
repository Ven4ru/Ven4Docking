using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Ven4Docking.Core.Models;

/// <summary>
/// One place for the JSON shape used by config.json, layouts and the admin API:
/// camelCase names, nulls omitted, Cyrillic written as-is (files are read by people too).
/// </summary>
public static class JsonDefaults
{
    public static readonly JsonSerializerOptions Options = Create(indented: true);

    /// <summary>Compact form for HTTP bodies and snapshots.</summary>
    public static readonly JsonSerializerOptions Compact = Create(indented: false);

    private static JsonSerializerOptions Create(bool indented) => new(JsonSerializerDefaults.Web)
    {
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        WriteIndented = indented,
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    public static string Serialize<T>(T value, bool indented = true) =>
        JsonSerializer.Serialize(value, indented ? Options : Compact);

    public static T? Deserialize<T>(string json) => JsonSerializer.Deserialize<T>(json, Options);
}
