// cpu: load, per-core bars, frequency, temperature. opts: {cores: bool, warn: °C, hot: °C}
import { h, loadColor, tempColor } from '../util.js';
import { frame, reason, bigSize } from './common.js';

export function render(el, opts) {
  const f = frame(el, 'cpu', 'Процессор');
  const row = h('div', 'row');
  const big = h('span', 'big num', '–');
  const freq = h('span', 'sub num');
  row.appendChild(big); row.appendChild(freq);
  const cores = h('div', 'cores');
  f.body.appendChild(row);
  if (opts.cores !== false) f.body.appendChild(cores);
  const warm = opts.warn || 70, hot = opts.hot || 85;
  let coreEls = [];

  return {
    update(s, errors) {
      const why = reason(s, errors, ['cpu']);
      f.error(why);
      if (why) return;
      const c = s.cpu;
      big.innerHTML = Math.round(c.load) + '<small>%</small>';
      big.style.color = loadColor(c.load);
      freq.textContent = c.freqMhz ? (c.freqMhz / 1000).toFixed(1) + ' ГГц' : '';
      const t = c.temp != null ? c.temp : null;
      if (t != null) f.aux.innerHTML = Math.round(t) + '<small>°C</small>';
      else if (c.boardTemp != null) f.aux.innerHTML = '<small>плата</small> ' + Math.round(c.boardTemp) + '<small>°C</small>';
      else f.aux.textContent = '';
      f.aux.style.color = tempColor(t != null ? t : c.boardTemp, warm, hot);
      const list = c.cores || [];
      if (coreEls.length !== list.length) {
        cores.innerHTML = '';
        coreEls = list.map(() => { const i = h('i'); const b = h('b'); i.appendChild(b); cores.appendChild(i); return b; });
      }
      list.forEach((v, i) => { coreEls[i].style.height = Math.max(6, Math.min(100, v)) + '%'; coreEls[i].style.background = loadColor(v); });
    },
    resize(w, hh) { big.style.fontSize = Math.round(bigSize(w, hh, 0.22)) + 'px'; },
  };
}
