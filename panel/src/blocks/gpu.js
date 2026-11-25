// gpu: load, temperature, VRAM, fan, load graph.
// opts: {gpu: "<id>" (default: the busiest card with a sensor), show: ["temp","vram","fan","graph"], warn, hot}
import { h, loadColor, tempColor, fmtB, spark, sparkSvg } from '../util.js';
import { track } from '../history.js';
import { frame, reason, bigSize } from './common.js';

function pick(gpus, id) {
  if (!gpus || !gpus.length) return null;
  if (id) for (let i = 0; i < gpus.length; i++) if (gpus[i].id === id) return gpus[i];
  return gpus.slice().sort((a, b) => ((b.temp != null) - (a.temp != null)) || ((b.vramUsed || 0) - (a.vramUsed || 0)))[0];
}

export function render(el, opts) {
  const f = frame(el, 'gpu', 'Видеокарта');
  const show = opts.show || ['temp', 'vram', 'fan', 'graph'];
  const has = (k) => show.indexOf(k) >= 0;
  const row = h('div', 'row');
  const big = h('span', 'big num', '–');
  const sub = h('span', 'sub num');
  row.appendChild(big); row.appendChild(sub);
  f.body.appendChild(row);
  const sp = sparkSvg();
  if (has('graph')) f.card.appendChild(sp);
  const warm = opts.warn || 75, hot = opts.hot || 88;

  return {
    update(s, errors) {
      const why = reason(s, errors, ['gpus', 'gpu']);
      const g = why ? null : pick(s.gpus, opts.gpu);
      f.error(why || (g ? null : 'Видеокарта не найдена'));
      if (!g) return;
      f.lbl.textContent = String(g.name || 'Видеокарта').replace(/^(AMD|NVIDIA|Intel\(R\))\s*/, '').replace(/\s*Graphics$/, '');
      big.innerHTML = Math.round(g.load) + '<small>%</small>';
      big.style.color = loadColor(g.load);
      f.aux.innerHTML = has('temp') && g.temp != null ? Math.round(g.temp) + '<small>°C</small>' : '';
      f.aux.style.color = tempColor(g.temp, warm, hot);
      sub.textContent = [has('vram') && g.vramUsed != null ? 'VRAM ' + fmtB(g.vramUsed) : '', has('fan') && g.fanRpm ? g.fanRpm + ' об/м' : '']
        .filter(Boolean).join(' · ');
      // per card, kept across relayouts (history.js)
      const hist = track('gpu:' + (g.id || g.name) + '.load', g.load, s, 60);
      if (has('graph')) spark(sp, hist, 100);
    },
    resize(w, hh) { big.style.fontSize = Math.round(bigSize(w, hh, 0.22)) + 'px'; },
  };
}
