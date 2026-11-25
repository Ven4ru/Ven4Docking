// clock: time and date from the phone, optional weather line. opts: {weather: bool, h12: bool}
import { h, pad2, esc } from '../util.js';

const DAYS = ['воскресенье', 'понедельник', 'вторник', 'среда', 'четверг', 'пятница', 'суббота'];
const MONTHS = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря'];

export function render(el, opts, ctx) {
  const card = h('div', 'card blk blk-clock');
  const time = h('div', 'time num', '--:--');
  const date = h('div', 'date');
  const wx = h('div', 'wx hint');
  card.appendChild(time); card.appendChild(date);
  if (opts.weather !== false) card.appendChild(wx);
  el.appendChild(card);

  function tick() {
    const n = new Date();
    let hr = n.getHours();
    let suffix = '';
    if (opts.h12) { suffix = hr < 12 ? ' AM' : ' PM'; hr = hr % 12 || 12; }
    time.innerHTML = (opts.h12 ? hr : pad2(hr)) + ':' + pad2(n.getMinutes()) + (suffix ? '<small>' + suffix + '</small>' : '');
    const d = DAYS[n.getDay()] + ', ' + n.getDate() + ' ' + MONTHS[n.getMonth()];
    date.textContent = d.charAt(0).toUpperCase() + d.slice(1);
  }
  tick();
  const timer = setInterval(tick, 1000);

  let unsub = null;
  if (opts.weather !== false) {
    unsub = ctx.weather.subscribe((w, err) => {
      if (!w) { wx.className = 'wx hint'; wx.textContent = err || ''; return; }
      wx.className = 'wx';
      wx.innerHTML = '<span class="t num">' + w.temp + '°</span><span>' + esc(w.text) + '</span><span class="sub num">' + w.min + '…' + w.max + '°</span>';
    });
  }

  return {
    update() {},
    resize(w, hh) {
      time.style.fontSize = Math.round(Math.max(28, Math.min(hh * 0.36, w * 0.25))) + 'px';
    },
    destroy() { clearInterval(timer); if (unsub) unsub(); },
  };
}
