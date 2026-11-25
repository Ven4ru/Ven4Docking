// net: download/upload speed, graph, busiest process. opts: {adapter} (chosen on the PC side)
import { h, fmtRate, spark, sparkSvg, pushHist } from '../util.js';
import { frame, reason } from './common.js';

export function render(el) {
  const f = frame(el, 'net', 'Сеть');
  const row = h('div', 'row bottom');
  const d = h('span', 'pair');
  const u = h('span', 'pair');
  d.innerHTML = '<span class="sub">↓</span> <span class="temp num">–</span>';
  u.innerHTML = '<span class="sub">↑</span> <span class="temp num">–</span>';
  row.appendChild(d); row.appendChild(u);
  f.body.appendChild(row);
  const dv = d.lastChild, uv = u.lastChild;
  const sp = sparkSvg();
  f.card.appendChild(sp);
  const hist = [];

  return {
    update(s, errors) {
      const why = reason(s, errors, ['net']);
      f.error(why);
      if (why) return;
      dv.textContent = fmtRate(s.net.down);
      uv.textContent = fmtRate(s.net.up);
      const tp = s.net.topProcess;
      f.aux.textContent = tp && tp.cpu >= 5 ? String(tp.name).replace(/\.exe$/i, '') + ' ' + Math.round(tp.cpu) + '%' : '';
      pushHist(hist, s.net.down, 60);
      spark(sp, hist);
    },
    resize(w, hh) {
      const fs = Math.round(Math.max(13, Math.min(hh * 0.22, w * 0.09))) + 'px';
      dv.style.fontSize = fs; uv.style.fontSize = fs;
    },
  };
}
