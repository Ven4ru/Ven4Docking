// disks: fill level per disk. opts: {disks: ["C:", "D:"] (default: all), warn: %, hot: %}
import { h, fmtB, levelColor, barRow } from '../util.js';
import { frame, reason } from './common.js';

export function render(el, opts) {
  const f = frame(el, 'disks', 'Диски');
  const bars = h('div', 'bars');
  f.body.appendChild(bars);
  const warn = opts.warn || 80, hot = opts.hot || 90;
  let rows = {}, sig = '';

  return {
    update(s, errors) {
      const why = reason(s, errors, ['disks']);
      f.error(why);
      if (why) return;
      const list = (s.disks || []).filter((d) => !opts.disks || !opts.disks.length || opts.disks.indexOf(d.mount) >= 0);
      const nsig = list.map((d) => d.mount).join('|');
      if (nsig !== sig) {
        sig = nsig; rows = {}; bars.innerHTML = '';
        list.forEach((d) => { rows[d.mount] = barRow(d.mount); bars.appendChild(rows[d.mount].row); });
      }
      if (!list.length) { f.error('Дисков нет'); return; }
      list.forEach((d) => {
        rows[d.mount].set(d.pct, '<b>' + Math.round(d.pct) + '%</b><span class="sz"> ' + fmtB(d.total) + '</span>', levelColor(d.pct, warn, hot));
      });
    },
    resize(w) {
      // narrow block: hide sizes, keep percentages
      bars.className = w < 170 ? 'bars narrow' : 'bars';
    },
  };
}
