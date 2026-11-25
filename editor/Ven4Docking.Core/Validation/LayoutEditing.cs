using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Validation;

/// <summary>Grid operations used by the «Экраны» editor; pure functions so they are unit-tested.</summary>
public static class LayoutEditing
{
    public static string NextBlockId(PanelLayout layout)
    {
        var used = layout.AllBlocks().Select(b => b.Id).ToHashSet(StringComparer.Ordinal);
        for (int n = 1; ; n++)
        {
            string id = "b" + n;
            if (!used.Contains(id)) return id;
        }
    }

    public static string NextScreenId(PanelLayout layout)
    {
        var used = layout.Screens.Select(s => s.Id).ToHashSet(StringComparer.Ordinal);
        for (int n = 1; ; n++)
        {
            string id = "s" + n;
            if (!used.Contains(id)) return id;
        }
    }

    public static bool IsFree(LayoutScreen screen, GridSize grid, int x, int y, int w, int h, LayoutBlock? ignore = null)
    {
        var probe = new LayoutBlock { X = x, Y = y, W = w, H = h };
        if (!LayoutValidator.InBounds(probe, grid)) return false;
        return screen.Blocks.All(b => ReferenceEquals(b, ignore) || !LayoutValidator.Overlaps(b, probe));
    }

    /// <summary>
    /// Finds a place for a w×h block: the preferred cell if it is free, otherwise the nearest free cell
    /// (row-major scan), shrinking the block down to 1×1 if nothing fits. Null when the screen is full.
    /// </summary>
    public static (int X, int Y, int W, int H)? FindPlace(LayoutScreen screen, GridSize grid, int prefX, int prefY, int w, int h)
    {
        w = Math.Clamp(w, 1, grid.Cols);
        h = Math.Clamp(h, 1, grid.Rows);
        for (int ww = w, hh = h; ww >= 1 && hh >= 1; )
        {
            int px = Math.Clamp(prefX, 0, grid.Cols - ww);
            int py = Math.Clamp(prefY, 0, grid.Rows - hh);
            if (IsFree(screen, grid, px, py, ww, hh)) return (px, py, ww, hh);

            (int X, int Y)? best = null;
            int bestDist = int.MaxValue;
            for (int y = 0; y + hh <= grid.Rows; y++)
            for (int x = 0; x + ww <= grid.Cols; x++)
            {
                if (!IsFree(screen, grid, x, y, ww, hh)) continue;
                int d = Math.Abs(x - px) + Math.Abs(y - py);
                if (d < bestDist) { bestDist = d; best = (x, y); }
            }
            if (best is { } p) return (p.X, p.Y, ww, hh);

            if (ww >= hh && ww > 1) ww--; else if (hh > 1) hh--; else break;
        }
        return null;
    }

    /// <summary>Adds a block of the given type near (x, y); returns null when there is no free cell.</summary>
    public static LayoutBlock? AddBlock(PanelLayout layout, LayoutScreen screen, string type, int x, int y)
    {
        var bt = BlockTypes.Find(type) ?? throw new ArgumentException("Unknown block type: " + type, nameof(type));
        var place = FindPlace(screen, layout.Grid, x, y, bt.DefaultW, bt.DefaultH);
        if (place is null) return null;
        var block = new LayoutBlock
        {
            Id = NextBlockId(layout),
            Type = type,
            X = place.Value.X,
            Y = place.Value.Y,
            W = place.Value.W,
            H = place.Value.H,
            Opts = bt.CreateDefaultOpts(),
        };
        screen.Blocks.Add(block);
        return block;
    }

    /// <summary>Moves a block keeping it inside the grid (overlap is allowed here and reported by the validator).</summary>
    public static void Move(LayoutBlock block, GridSize grid, int x, int y)
    {
        block.X = Math.Clamp(x, 0, Math.Max(0, grid.Cols - block.W));
        block.Y = Math.Clamp(y, 0, Math.Max(0, grid.Rows - block.H));
    }

    public static void Resize(LayoutBlock block, GridSize grid, int w, int h)
    {
        block.W = Math.Clamp(w, 1, Math.Max(1, grid.Cols - block.X));
        block.H = Math.Clamp(h, 1, Math.Max(1, grid.Rows - block.Y));
    }
}
