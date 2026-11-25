// disks: fill level per disk. opts: {disks: ["C:", "D:"] (default: all), warn: %, hot: %}
// Any number of disks fits the cell: full rows when there is room, then tighter rows, then tiles in
// 2…6 columns (name + percent over a thin bar), then one vertical bar per disk. Every candidate is
// measured in place (fonts differ between phones); only when nothing fits, tight rows scroll, cut
// at a whole row so no row is clipped in the middle.
import { h, fmtB, levelColor, barRow } from '../util.js';
import { frame, reason } from './common.js';

const MAX_COLS = 6;

export function render(el, opts) {
  const f = frame(el, 'disks', 'Диски');
  const bars = h('div', 'bars');
  f.body.appendChild(bars);
  const warn = opts.warn || 80, hot = opts.hot || 90;
  let rows = {}, sig = '', count = 0;
  let pitch = 0, visible = 0; // scroll mode: row step in px, whole rows in view

  /** Scroll mode: "+N" in the header while N disks are below the visible rows. */
  function hint() {
    const below = pitch ? count - visible - Math.round(bars.scrollTop / pitch) : 0;
    f.aux.textContent = below > 0 ? '+' + below : '';
  }
  bars.addEventListener('scroll', hint);

  /** Some name or percent does not fit its box (ellipsis). */
  function clipped() {
    const els = bars.querySelectorAll('.bl, .v');
    for (let i = 0; i < els.length; i++) if (els[i].scrollWidth > els[i].clientWidth + 1) return true;
    return false;
  }

  /** Picks the roomiest arrangement whose rows all fit the body. */
  function fit() {
    const bw = f.body.clientWidth, bh = f.body.clientHeight;
    if (!count) f.aux.textContent = '';
    if (!count || !bw || !bh) return;
    bars.style.maxHeight = '';
    pitch = 0;
    const narrow = bw < 170 ? ' narrow' : '';
    const modes = [['bars' + narrow, false], ['bars tight' + narrow, false]];
    for (let c = 2; c <= Math.min(MAX_COLS, count); c++) modes.push(['bars tiles c' + c, true]);
    if (count > 1) modes.push(['bars cols', true]);
    let chosen = null;
    for (let i = 0; i < modes.length && !chosen; i++) {
      bars.className = modes[i][0] + ' fitting';
      if (bars.scrollHeight <= bh + 1 && !(modes[i][1] && clipped())) chosen = modes[i][0];
    }
    if (!chosen) {
      // too many for the cell: tight rows that scroll, cut at a whole row
      chosen = 'bars tight narrow scroll';
      bars.className = chosen + ' fitting';
      const r = bars.children;
      if (r.length > 1 && r[1].offsetTop > r[0].offsetTop) {
        pitch = r[1].offsetTop - r[0].offsetTop;
        const gap = pitch - r[0].offsetHeight;
        visible = Math.max(1, Math.floor((bh + gap) / pitch));
        bars.style.maxHeight = (visible * pitch - gap) + 'px';
      }
    }
    // settle the chosen mode with transitions off, so the bars don't animate from the probed modes
    bars.className = chosen + ' fitting';
    bars.getBoundingClientRect();
    bars.className = chosen;
    hint();
  }

  return {
    update(s, errors) {
      const why = reason(s, errors, ['disks']);
      f.error(why);
      if (why) return;
      const list = (s.disks || []).filter((d) => !opts.disks || !opts.disks.length || opts.disks.indexOf(d.mount) >= 0);
      const nsig = list.map((d) => d.mount).join('|');
      const changed = nsig !== sig;
      if (changed) {
        sig = nsig; rows = {}; bars.innerHTML = ''; count = list.length;
        list.forEach((d) => { rows[d.mount] = barRow(d.mount); bars.appendChild(rows[d.mount].row); });
      }
      if (!list.length) { f.error('Дисков нет'); return; }
      list.forEach((d) => {
        const r = rows[d.mount];
        r.set(d.pct, '<b>' + Math.round(d.pct) + '<i>%</i></b><span class="sz"> ' + fmtB(d.total) + '</span>', levelColor(d.pct, warn, hot));
        // vertical bars (mode "cols") take the level from --pct
        r.row.style.setProperty('--pct', (d.pct == null ? 0 : Math.max(0, Math.min(100, d.pct))) + '%');
      });
      if (changed) fit();
    },
    resize() { fit(); },
  };
}
