// weather: current weather on its own (open-meteo from the phone).
import { esc } from '../util.js';
import { frame } from './common.js';

export function render(el, opts, ctx) {
  const f = frame(el, 'weather', 'Погода');
  const big = document.createElement('div');
  big.className = 'big num';
  const sub = document.createElement('div');
  sub.className = 'sub';
  f.body.appendChild(big); f.body.appendChild(sub);

  const unsub = ctx.weather.subscribe((w, err, cfg) => {
    f.lbl.textContent = cfg && cfg.name ? 'Погода · ' + cfg.name : 'Погода';
    if (!w) { f.error(err || 'Жду погоду…'); return; }
    f.error(null);
    big.innerHTML = w.temp + '<small>°</small>';
    sub.innerHTML = esc(w.text) + ' · ' + w.min + '…' + w.max + '° · ветер ' + w.wind + ' км/ч';
  });

  return {
    update() {},
    resize(w, hh) { big.style.fontSize = Math.round(Math.max(18, Math.min(hh * 0.42, w * 0.2))) + 'px'; },
    destroy() { unsub(); },
  };
}
