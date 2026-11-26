using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Validation;

public static class ActionIssueKinds
{
    public const string BadId = "badId";
    public const string DuplicateId = "duplicateId";
    public const string BadKind = "badKind";
    public const string BadKeys = "badKeys";
    public const string EmptyText = "emptyText";
    public const string TextTooLong = "textTooLong";
    public const string StepCount = "stepCount";
    public const string BadStep = "badStep";
    public const string BadWait = "badWait";
    public const string UnknownApp = "unknownApp";
    public const string NoDevices = "noDevices";
}

/// <summary>A problem of one action; <see cref="Step"/> is the 0-based macro step, null for the action itself.</summary>
public sealed record ActionIssue(string ActionId, string Kind, int? Step = null);

/// <summary>
/// The rules the agent applies to <c>config.actions</c> (spec 1.1 §3): a config that breaks them is refused on save,
/// so the editor checks first and points at the action.
/// </summary>
public static class ActionValidator
{
    /// <summary>Same as <c>valid_id</c> of the agent: 1–64 of [A-Za-z0-9_-].</summary>
    public static bool IsValidId(string? id) =>
        !string.IsNullOrEmpty(id) && id.Length <= 64 && id.All(c => c is (>= 'a' and <= 'z') or (>= 'A' and <= 'Z') or (>= '0' and <= '9') or '-' or '_');

    public static IReadOnlyList<ActionIssue> Validate(DockConfig config)
    {
        var issues = new List<ActionIssue>();
        if (config.Actions is null) return issues;
        var apps = config.Apps.Select(a => a.Id).ToHashSet(StringComparer.Ordinal);
        var ids = new HashSet<string>(StringComparer.Ordinal);
        foreach (var a in config.Actions)
        {
            if (!IsValidId(a.Id)) issues.Add(new ActionIssue(a.Id, ActionIssueKinds.BadId));
            else if (!ids.Add(a.Id)) issues.Add(new ActionIssue(a.Id, ActionIssueKinds.DuplicateId));

            switch (a.Kind)
            {
                case ActionEntry.KindKeys:
                    if (!Hotkey.TryParse(a.Keys, out _)) issues.Add(new ActionIssue(a.Id, ActionIssueKinds.BadKeys));
                    break;
                case ActionEntry.KindText:
                    CheckText(a.Id, a.Text, null, issues);
                    break;
                case ActionEntry.KindMacro:
                    int n = a.Steps?.Count ?? 0;
                    if (n is < ActionEntry.MinSteps or > ActionEntry.MaxSteps)
                        issues.Add(new ActionIssue(a.Id, ActionIssueKinds.StepCount));
                    for (int i = 0; i < n; i++) CheckStep(a.Id, a.Steps![i], i, apps, issues);
                    break;
                case ActionEntry.KindMic:
                    break;
                case ActionEntry.KindAudioOut:
                    if (a.Devices is null || a.Devices.Count(d => !string.IsNullOrWhiteSpace(d)) == 0)
                        issues.Add(new ActionIssue(a.Id, ActionIssueKinds.NoDevices));
                    break;
                default:
                    issues.Add(new ActionIssue(a.Id, ActionIssueKinds.BadKind));
                    break;
            }
        }
        return issues;
    }

    private static void CheckText(string id, string? text, int? step, List<ActionIssue> issues)
    {
        if (string.IsNullOrEmpty(text)) issues.Add(new ActionIssue(id, ActionIssueKinds.EmptyText, step));
        else if (text.Length > ActionEntry.MaxTextLength) issues.Add(new ActionIssue(id, ActionIssueKinds.TextTooLong, step));
    }

    private static void CheckStep(string id, MacroStep s, int i, HashSet<string> apps, List<ActionIssue> issues)
    {
        switch (s.Kind)
        {
            case MacroStep.KindKeys:
                if (!Hotkey.TryParse(s.Keys, out _)) issues.Add(new ActionIssue(id, ActionIssueKinds.BadKeys, i));
                break;
            case MacroStep.KindText:
                CheckText(id, s.Text, i, issues);
                break;
            case MacroStep.KindWait:
                if (s.Wait is < MacroStep.MinWait or > MacroStep.MaxWait) issues.Add(new ActionIssue(id, ActionIssueKinds.BadWait, i));
                break;
            case MacroStep.KindLaunch:
                if (!apps.Contains(s.Launch!)) issues.Add(new ActionIssue(id, ActionIssueKinds.UnknownApp, i));
                break;
            default:
                issues.Add(new ActionIssue(id, ActionIssueKinds.BadStep, i));
                break;
        }
    }
}
