using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Validation;

public static class IssueKinds
{
    public const string Overlap = "overlap";
    public const string Bounds = "bounds";
    public const string Size = "size";
    public const string Screens = "screens";
    public const string DuplicateId = "duplicateId";
    public const string Grid = "grid";
    /// <summary>More than <see cref="LayoutScreen.MaxProgramScreens"/> program screens.</summary>
    public const string ProgramScreens = "programScreens";
    /// <summary>Every screen is a program screen: nothing to show in the normal swipe order.</summary>
    public const string NoMainScreen = "noMainScreen";
}

/// <summary>One problem that blocks saving. <see cref="BlockIds"/> are the blocks to highlight.</summary>
public sealed record LayoutIssue(string Kind, string? ScreenId, IReadOnlyList<string> BlockIds);

/// <summary>
/// Same rules as the agent (spec §4): blocks do not overlap and stay inside the grid, 1–10 screens;
/// up to 5 of them are program screens (spec 1.1 §4) and at least one is a normal screen.
/// </summary>
public static class LayoutValidator
{
    public static bool Overlaps(LayoutBlock a, LayoutBlock b) =>
        a.X < b.Right && b.X < a.Right && a.Y < b.Bottom && b.Y < a.Bottom;

    public static bool InBounds(LayoutBlock b, GridSize grid) =>
        b.X >= 0 && b.Y >= 0 && b.W >= 1 && b.H >= 1 && b.Right <= grid.Cols && b.Bottom <= grid.Rows;

    public static IReadOnlyList<LayoutIssue> Validate(PanelLayout layout)
    {
        var issues = new List<LayoutIssue>();

        if (layout.Grid.Cols < 1 || layout.Grid.Rows < 1)
            issues.Add(new LayoutIssue(IssueKinds.Grid, null, Array.Empty<string>()));

        if (layout.Screens.Count is < PanelLayout.MinScreens or > PanelLayout.MaxScreens)
            issues.Add(new LayoutIssue(IssueKinds.Screens, null, Array.Empty<string>()));

        var programScreens = layout.Screens.Where(s => s.IsProgramScreen).ToList();
        if (programScreens.Count > LayoutScreen.MaxProgramScreens)
            issues.Add(new LayoutIssue(IssueKinds.ProgramScreens, programScreens[LayoutScreen.MaxProgramScreens].Id, Array.Empty<string>()));
        if (layout.Screens.Count > 0 && programScreens.Count == layout.Screens.Count)
            issues.Add(new LayoutIssue(IssueKinds.NoMainScreen, null, Array.Empty<string>()));

        var ids = new HashSet<string>(StringComparer.Ordinal);
        foreach (var screen in layout.Screens)
        {
            foreach (var b in screen.Blocks)
            {
                if (!ids.Add(b.Id))
                    issues.Add(new LayoutIssue(IssueKinds.DuplicateId, screen.Id, new[] { b.Id }));
                if (b.W < 1 || b.H < 1)
                    issues.Add(new LayoutIssue(IssueKinds.Size, screen.Id, new[] { b.Id }));
                else if (!InBounds(b, layout.Grid))
                    issues.Add(new LayoutIssue(IssueKinds.Bounds, screen.Id, new[] { b.Id }));
            }

            for (int i = 0; i < screen.Blocks.Count; i++)
            for (int j = i + 1; j < screen.Blocks.Count; j++)
            {
                if (Overlaps(screen.Blocks[i], screen.Blocks[j]))
                    issues.Add(new LayoutIssue(IssueKinds.Overlap, screen.Id,
                        new[] { screen.Blocks[i].Id, screen.Blocks[j].Id }));
            }
        }
        return issues;
    }

    /// <summary>Ids of every block that takes part in some issue.</summary>
    public static ISet<string> InvalidBlockIds(IEnumerable<LayoutIssue> issues) =>
        issues.SelectMany(i => i.BlockIds).ToHashSet(StringComparer.Ordinal);
}
