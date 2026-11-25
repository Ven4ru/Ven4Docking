using Ven4Docking.Core.Models;
using Ven4Docking.Core.Validation;

namespace Ven4Docking.Core.Tests;

public class LayoutValidatorTests
{
    private static LayoutBlock B(string id, int x, int y, int w, int h) => new() { Id = id, Type = "cpu", X = x, Y = y, W = w, H = h };

    private static PanelLayout L(params LayoutBlock[] blocks) =>
        new() { Screens = { new LayoutScreen { Id = "s1", Name = "Главный", Blocks = blocks.ToList() } } };

    [Fact]
    public void Valid_layout_has_no_issues() =>
        Assert.Empty(LayoutValidator.Validate(L(B("b1", 0, 0, 4, 2), B("b2", 4, 0, 4, 2), B("b3", 0, 2, 8, 2))));

    [Fact]
    public void Touching_edges_do_not_overlap() =>
        Assert.False(LayoutValidator.Overlaps(B("a", 0, 0, 2, 2), B("b", 2, 0, 2, 2)));

    [Fact]
    public void Overlap_is_reported_with_both_blocks()
    {
        var issues = LayoutValidator.Validate(L(B("b1", 0, 0, 3, 2), B("b2", 2, 1, 2, 2)));
        var issue = Assert.Single(issues);
        Assert.Equal(IssueKinds.Overlap, issue.Kind);
        Assert.Equal(new[] { "b1", "b2" }, issue.BlockIds);
        Assert.Equal("s1", issue.ScreenId);
    }

    [Fact]
    public void Blocks_on_different_screens_may_share_cells()
    {
        var layout = L(B("b1", 0, 0, 8, 4));
        layout.Screens.Add(new LayoutScreen { Id = "s2", Name = "2", Blocks = { B("b2", 0, 0, 8, 4) } });
        Assert.Empty(LayoutValidator.Validate(layout));
    }

    [Theory]
    [InlineData(7, 0, 2, 1)]   // right edge
    [InlineData(0, 3, 1, 2)]   // bottom edge
    [InlineData(-1, 0, 1, 1)]  // negative x
    [InlineData(0, -1, 1, 1)]  // negative y
    [InlineData(0, 0, 9, 1)]   // wider than grid
    public void Out_of_bounds_is_reported(int x, int y, int w, int h)
    {
        var issue = Assert.Single(LayoutValidator.Validate(L(B("b1", x, y, w, h))));
        Assert.Equal(IssueKinds.Bounds, issue.Kind);
        Assert.Equal(new[] { "b1" }, issue.BlockIds);
    }

    [Fact]
    public void Full_grid_block_is_in_bounds() => Assert.Empty(LayoutValidator.Validate(L(B("b1", 0, 0, 8, 4))));

    [Fact]
    public void Zero_size_is_reported() =>
        Assert.Contains(LayoutValidator.Validate(L(B("b1", 0, 0, 0, 1))), i => i.Kind == IssueKinds.Size);

    [Theory]
    [InlineData(0, true)]
    [InlineData(1, false)]
    [InlineData(10, false)]
    [InlineData(11, true)]
    public void Screen_count_must_be_1_to_10(int count, bool invalid)
    {
        var layout = new PanelLayout();
        for (int i = 0; i < count; i++) layout.Screens.Add(new LayoutScreen { Id = "s" + i, Name = "S" + i });
        Assert.Equal(invalid, LayoutValidator.Validate(layout).Any(x => x.Kind == IssueKinds.Screens));
    }

    [Fact]
    public void Duplicate_block_ids_across_screens_are_reported()
    {
        var layout = L(B("b1", 0, 0, 1, 1));
        layout.Screens.Add(new LayoutScreen { Id = "s2", Name = "2", Blocks = { B("b1", 0, 0, 1, 1) } });
        Assert.Contains(LayoutValidator.Validate(layout), i => i.Kind == IssueKinds.DuplicateId);
    }

    [Fact]
    public void Spec_example_layout_is_flagged_as_overlapping()
    {
        // docs/specs §4 example: b3 (0,0,8,4) covers the whole screen together with b1 and b2.
        var layout = JsonDefaults.Deserialize<PanelLayout>(Samples.SpecLayout)!;
        var ids = LayoutValidator.InvalidBlockIds(LayoutValidator.Validate(layout));
        Assert.Equal(new[] { "b1", "b2", "b3" }, ids.OrderBy(x => x));
    }

    [Theory]
    [InlineData(LayoutTemplates.Classic)]
    [InlineData(LayoutTemplates.PcMonitor)]
    [InlineData(LayoutTemplates.AppsOnly)]
    public void Templates_are_valid(string template)
    {
        foreach (var lang in new[] { "ru", "en" })
            Assert.Empty(LayoutValidator.Validate(LayoutTemplates.Create(template, lang)));
    }
}

public class LayoutEditingTests
{
    [Fact]
    public void AddBlock_uses_default_size_and_free_cell()
    {
        var layout = new PanelLayout { Screens = { new LayoutScreen { Id = "s1", Name = "x" } } };
        var screen = layout.Screens[0];
        var a = LayoutEditing.AddBlock(layout, screen, BlockTypes.Clock, 0, 0)!;
        var b = LayoutEditing.AddBlock(layout, screen, BlockTypes.Clock, 0, 0)!;
        Assert.Equal((3, 2), (a.W, a.H));
        Assert.Equal("b1", a.Id);
        Assert.Equal("b2", b.Id);
        Assert.False(LayoutValidator.Overlaps(a, b));
        Assert.Empty(LayoutValidator.Validate(layout));
        Assert.True(a.Opts["weather"]!.GetValue<bool>());
    }

    [Fact]
    public void AddBlock_shrinks_to_fit_and_returns_null_when_full()
    {
        var layout = new PanelLayout { Screens = { new LayoutScreen { Id = "s1", Name = "x" } } };
        var screen = layout.Screens[0];
        screen.Blocks.Add(new LayoutBlock { Id = "big", Type = "apps", X = 0, Y = 0, W = 8, H = 4 });
        Assert.Null(LayoutEditing.AddBlock(layout, screen, BlockTypes.Cpu, 3, 3));

        screen.Blocks[0].W = 7; // one free column
        var b = LayoutEditing.AddBlock(layout, screen, BlockTypes.Cpu, 0, 0)!;
        Assert.Equal((7, 1), (b.X, b.W));
        Assert.Empty(LayoutValidator.Validate(layout));
    }

    [Fact]
    public void Move_and_resize_stay_inside_grid()
    {
        var grid = new GridSize();
        var b = new LayoutBlock { Id = "b1", Type = "cpu", X = 0, Y = 0, W = 3, H = 2 };
        LayoutEditing.Move(b, grid, 10, 10);
        Assert.Equal((5, 2), (b.X, b.Y));
        LayoutEditing.Resize(b, grid, 9, 9);
        Assert.Equal((3, 2), (b.W, b.H));
        LayoutEditing.Resize(b, grid, 0, 0);
        Assert.Equal((1, 1), (b.W, b.H));
    }
}
