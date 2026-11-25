// phone: battery level, battery temperature and charging of the panel phone itself,
// read from the launcher bridge DockAndroid.battery(). opts: {hot: °C (default 45)}
import { h } from '../util.js';
import { frame, bigSize } from './common.js';

function readBattery(demo) {
  if (demo) return { level: 82, temp: 31.5, charging: true };
  try {
    if (window.DockAndroid && window.DockAndroid.battery) return JSON.parse(window.DockAndroid.battery());
  } catch (e) { /* bridge error */ }
  return null;
}

export function render(el, opts, ctx) {
  const f = frame(el, 'phone', 'Телефон');
  const big = h('div', 'big num', '–');
  const sub = h('div', 'sub num');
  f.body.appendChild(big); f.body.appendChild(sub);
  const hot = opts.hot || 45;

  function refresh() {
    const b = readBattery(ctx.demo);
    if (!b) { f.error('Только в лаунчере Ven4Docking'); return; }
    f.error(null);
    const overheat = b.temp != null && b.temp >= hot;
    f.card.className = 'card blk blk-phone' + (overheat ? ' overheat' : '');
    big.innerHTML = (b.level != null ? Math.round(b.level) : '–') + '<small>%</small>';
    f.aux.textContent = b.charging ? '⚡' : '';
    sub.textContent = b.temp != null ? Math.round(b.temp) + ' °C' + (overheat ? ' — перегрев' : '') : '';
  }
  refresh();
  const timer = setInterval(refresh, 20000);

  return {
    update() {},
    resize(w, hh) { big.style.fontSize = Math.round(bigSize(w, hh, 0.3)) + 'px'; },
    destroy() { clearInterval(timer); },
  };
}
