// Places blocks of one screen on the layout grid. CSS Grid where the WebView has it (Chrome 57+),
// otherwise absolute positioning by cell percentages (Chrome 51–56, stock Android 7 WebView).
// Grid properties are set from JS so the stylesheet itself stays Chrome 51-clean.

const GAP = 8; // px, same as --gap

let gridSupported = null;
export function useGrid() {
  if (gridSupported === null) {
    const force = /[?&]layout=abs\b/.test(location.search);
    gridSupported = !force && !!(window.CSS && CSS.supports && CSS.supports('display', 'grid'));
  }
  return gridSupported;
}

/** Validates a block against the grid; returns a clamped copy or null. */
export function fitBlock(b, cols, rows) {
  const x = Math.max(0, b.x | 0), y = Math.max(0, b.y | 0);
  if (x >= cols || y >= rows) return null;
  const w = Math.max(1, Math.min(cols - x, b.w | 0 || 1));
  const hh = Math.max(1, Math.min(rows - y, b.h | 0 || 1));
  return { x, y, w, h: hh };
}

/** Prepares the box that holds the cells (absolutely positioned inside the page). */
export function setupScreen(boxEl, cols, rows) {
  const st = boxEl.style;
  st.position = 'absolute';
  if (useGrid()) {
    st.top = st.right = st.bottom = st.left = '0';
    st.display = 'grid';
    st.gridTemplateColumns = 'repeat(' + cols + ', minmax(0, 1fr))';
    st.gridTemplateRows = 'repeat(' + rows + ', minmax(0, 1fr))';
    st.gridGap = GAP + 'px';
    st.padding = GAP + 'px';
  } else {
    // cells carry half the gap as padding on each side, the box the other half
    st.top = st.right = st.bottom = st.left = GAP / 2 + 'px';
    st.display = 'block';
  }
}

/** Positions one cell element (the wrapper around a block card). */
export function placeCell(cellEl, pos, cols, rows) {
  const st = cellEl.style;
  if (useGrid()) {
    st.gridColumn = (pos.x + 1) + ' / span ' + pos.w;
    st.gridRow = (pos.y + 1) + ' / span ' + pos.h;
    st.position = 'relative';
  } else {
    st.position = 'absolute';
    st.left = (pos.x / cols) * 100 + '%';
    st.top = (pos.y / rows) * 100 + '%';
    st.width = (pos.w / cols) * 100 + '%';
    st.height = (pos.h / rows) * 100 + '%';
    st.padding = GAP / 2 + 'px';
  }
}

export const LAYOUT_GAP = GAP;
