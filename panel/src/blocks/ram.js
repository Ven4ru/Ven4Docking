// ram: memory use and PC uptime. opts: {warn: %, hot: %}
import { h, fmtB, levelColor, upFmt } from '../util.js';
import { frame, reason, bigSize } from './common.js';

export function render(el, opts) {
  const f = frame(el, 'ram', 'Память');
  const row = h('div', 'row');
  const big = h('span', 'big num', '–');
  const sub = h('span', 'sub num');
  row.appendChild(big); row.appendChild(sub);
  f.body.appendChild(row);
  const warn = opts.warn || 70, hot = opts.hot || 90;

  return {
    update(s, errors) {
      const why = reason(s, errors, ['ram']);
      f.error(why);
      if (why) return;
      big.innerHTML = Math.round(s.ram.pct) + '<small>%</small>';
      big.style.color = levelColor(s.ram.pct, warn, hot);
      sub.textContent = fmtB(s.ram.used) + ' из ' + fmtB(s.ram.total);
      f.aux.textContent = s.uptime != null ? 'вкл. ' + upFmt(s.uptime) : '';
    },
    resize(w, hh) { big.style.fontSize = Math.round(bigSize(w, hh, 0.22)) + 'px'; },
  };
}
