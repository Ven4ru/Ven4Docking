using System.Text.Json;
using System.Text.Json.Serialization;

namespace Ven4Docking.Core.Models;

/// <summary>
/// An entry of <c>config.actions</c> (spec 1.1 §3), shown on the panel by the «Кнопки» block.
/// <see cref="Kind"/>: keys | text | macro | mic | audioOut; only the fields of that kind are written.
/// </summary>
public sealed class ActionEntry
{
    public const string KindKeys = "keys";
    public const string KindText = "text";
    public const string KindMacro = "macro";
    public const string KindMic = "mic";
    public const string KindAudioOut = "audioOut";

    public static readonly IReadOnlyList<string> Kinds = new[] { KindKeys, KindText, KindMacro, KindMic, KindAudioOut };

    public const int MaxTextLength = 2000;
    public const int MinSteps = 1;
    public const int MaxSteps = 20;

    public string Id { get; set; } = "";
    public string Title { get; set; } = "";
    public string? Icon { get; set; }
    public string Kind { get; set; } = KindKeys;
    public string? Keys { get; set; }
    public string? Text { get; set; }
    public List<MacroStep>? Steps { get; set; }
    public List<string>? Devices { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    /// <summary>Drops the fields that do not belong to <see cref="Kind"/> (after the kind was changed).</summary>
    public void KeepOnlyKindFields()
    {
        if (Kind != KindKeys) Keys = null;
        if (Kind != KindText) Text = null;
        if (Kind != KindMacro) Steps = null;
        if (Kind != KindAudioOut) Devices = null;
    }
}

/// <summary>
/// One step of a <c>macro</c> action: exactly one of <c>keys</c>, <c>text</c>, <c>wait</c> (ms 1–10000),
/// <c>launch</c> (id of an app from <c>config.apps</c>).
/// </summary>
public sealed class MacroStep
{
    public const string KindKeys = "keys";
    public const string KindText = "text";
    public const string KindWait = "wait";
    public const string KindLaunch = "launch";

    public static readonly IReadOnlyList<string> Kinds = new[] { KindKeys, KindText, KindWait, KindLaunch };

    public const int MinWait = 1;
    public const int MaxWait = 10000;

    public string? Keys { get; set; }
    public string? Text { get; set; }
    public int? Wait { get; set; }
    public string? Launch { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    /// <summary>Which field is set; null when none or more than one is (the step is invalid then).</summary>
    [JsonIgnore]
    public string? Kind
    {
        get
        {
            int n = (Keys is null ? 0 : 1) + (Text is null ? 0 : 1) + (Wait is null ? 0 : 1) + (Launch is null ? 0 : 1);
            if (n != 1) return null;
            return Keys is not null ? KindKeys : Text is not null ? KindText : Wait is not null ? KindWait : KindLaunch;
        }
    }

    public static MacroStep Create(string kind) => kind switch
    {
        KindKeys => new MacroStep { Keys = "" },
        KindText => new MacroStep { Text = "" },
        KindWait => new MacroStep { Wait = 1000 },
        KindLaunch => new MacroStep { Launch = "" },
        _ => throw new ArgumentException("Unknown step kind: " + kind, nameof(kind)),
    };
}
