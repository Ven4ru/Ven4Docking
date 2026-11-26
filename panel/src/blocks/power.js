// power: lock, sleep, restart and shutdown of the PC. A button fires only after a 2 s hold (the ring
// around it fills); letting go earlier does nothing. Restart / shutdown leave a band over the panel
// with the countdown and «Отмена» (POST /api/v1/power {"action":"cancel"}).
// opts: {buttons: ["lock", "sleep", "restart", "shutdown"]}
import { h } from '../util.js';
import { press, tiles } from './common.js';

const HOLD_MS = 2000;
const MOVE_PX = 14;   // a finger that slides further is swiping, not holding
const BUTTONS = {
  lock: { title: 'Блокировка', icon: '<path d="M12 2a5 5 0 0 0-5 5v3H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-1V7a5 5 0 0 0-5-5zM9 10V7a3 3 0 0 1 6 0v3z"/>' },
  sleep: { title: 'Сон', icon: '<path d="M14.5 2.5a9.5 9.5 0 1 0 7 13A8 8 0 0 1 14.5 2.5z"/>' },
  restart: { title: 'Перезагрузка', icon: '<path d="M12 5V2L7.5 6.5 12 11V7a5 5 0 1 1-5 5H5a7 7 0 1 0 7-7z"/>' },
  shutdown: { title: 'Выключение', icon: '<path d="M12 3v8" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/><path d="M7.4 6.3a7 7 0 1 0 9.2 0" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>' },
};
const DEFAULT = ['lock', 'sleep', 'restart', 'shutdown'];

// ---------- the countdown band (one for the whole panel, survives a layout rebuild) ----------

let band = null;
function showBand(ctx, action, delay) {
  if (!band) {
    band = { el: h('div', 'pw-band'), text: h('span', 'tx'), cancel: h('span', 'cancel', 'Отмена'), timer: null, hide: null };
    band.cancel.setAttribute('role', 'button');
    band.el.appendChild(band.text);
    band.el.appendChild(band.cancel);
    document.body.appendChild(band.el);
  }
  const b = band;
  const what = action === 'restart' ? 'Перезагрузка' : 'Выключение';
  let left = delay > 0 ? delay : 10;
  clearInterval(b.timer); clearTimeout(b.hide);
  const close = (ms) => { b.hide = setTimeout(() => { b.el.className = 'pw-band'; }, ms); };
  const tick = () => {
    if (left <= 0) {
      clearInterval(b.timer);
      b.text.textContent = what + '…';
      b.cancel.style.display = 'none';
      close(15000);
      return;
    }
    b.text.textContent = what + ' через ' + left + ' с';
    left--;
  };
  b.cancel.style.display = '';
  b.cancel.onclick = () => {
    clearInterval(b.timer);
    b.text.textContent = 'Отменяю…';
    b.cancel.style.display = 'none';
    ctx.post('/api/v1/power', { action: 'cancel' }).then(() => {
      b.text.textContent = what + ' отменено';
      close(2500);
    }, () => {
      b.text.textContent = 'Не удалось отменить';
      b.cancel.style.display = '';
      close(10000);
    });
  };
  b.el.className = 'pw-band on';
  tick();
  b.timer = setInterval(tick, 1000);
}

// ---------- the block ----------

export function render(el, opts, ctx) {
  const card = h('div', 'card blk blk-power');
  const wrap = h('div', 'pw-wrap');
  card.appendChild(wrap);
  el.appendChild(card);
  const names = (opts.buttons && opts.buttons.length ? opts.buttons : DEFAULT).filter((n) => BUTTONS[n]);
  const items = [];

  if (!names.length) wrap.appendChild(h('div', 'empty', 'Кнопки питания не выбраны'));
  names.forEach((name) => {
    const def = BUTTONS[name];
    const it = h('div', 'pw pw-' + name);
    const btn = h('div', 'pw-btn');
    btn.setAttribute('role', 'button');
    btn.setAttribute('aria-label', def.title);
    btn.innerHTML = '<svg class="ic" viewBox="0 0 24 24">' + def.icon + '</svg>'
      + '<svg class="ring" viewBox="0 0 40 40"><circle cx="20" cy="20" r="18"/></svg>';
    const cap = h('span', 'cap', def.title);
    it.appendChild(btn); it.appendChild(cap);
    wrap.appendChild(it);
    items.push({ it, btn, cap });

    let timer = null, sx = 0, sy = 0, fired = false, flashTimer = null;
    const flash = (cls, text, ms) => {
      clearTimeout(flashTimer);
      it.className = 'pw pw-' + name + (cls ? ' ' + cls : '');
      cap.textContent = text || def.title;
      flashTimer = setTimeout(() => { it.className = 'pw pw-' + name; cap.textContent = def.title; }, ms);
    };
    const fire = () => {
      fired = true;
      timer = null;
      it.className = 'pw pw-' + name + ' busy';
      ctx.post('/api/v1/power', { action: name }).then((r) => {
        flash('done', null, 900);
        if (name === 'restart' || name === 'shutdown') showBand(ctx, name, r && r.delay ? +r.delay : 10);
      }, (e) => {
        flash('fail', e && e.status === 403 ? 'Не разрешено' : 'Не вышло', 2000);
      });
    };
    const stop = () => {
      clearTimeout(timer); timer = null;
      if (!fired) it.className = 'pw pw-' + name;
    };
    press(btn, {
      start(x, y, e) {
        if (it.className.indexOf('busy') >= 0) return false;
        if (e && e.type === 'mousedown') e.preventDefault();
        sx = x; sy = y; fired = false;
        clearTimeout(flashTimer); cap.textContent = def.title;
        it.className = 'pw pw-' + name + ' holding';
        timer = setTimeout(fire, HOLD_MS);
        return true;
      },
      move(x, y) { if (timer && (Math.abs(x - sx) > MOVE_PX || Math.abs(y - sy) > MOVE_PX)) stop(); },
      end(cancelled) {
        const early = !!timer;
        stop();
        if (early && !cancelled) flash('hint', 'Удерживайте 2 с', 1500);
      },
    });
    // the long press must not open a context menu in the WebView
    btn.addEventListener('contextmenu', (e) => e.preventDefault());
  });

  return {
    update() {},
    resize(w, hh) {
      if (!items.length) return;
      const t = tiles(items.length, w - 4, hh - 4, 0.95);
      const fontPx = Math.max(10, Math.min(15, Math.round(t.th * 0.15)));
      const caps = t.th >= fontPx * 5 && t.tw >= fontPx * 5;
      const d = Math.max(26, Math.round(Math.min(t.tw * 0.66, (t.th - (caps ? fontPx * 1.7 : 0)) * 0.84)));
      items.forEach((x) => {
        x.it.style.width = (100 / t.cols) + '%';
        x.it.style.height = (100 / t.rows) + '%';
        x.btn.style.width = x.btn.style.height = d + 'px';
        x.cap.style.display = caps ? '' : 'none';
        x.cap.style.fontSize = fontPx + 'px';
        x.cap.style.maxWidth = Math.floor(t.tw - 4) + 'px';
      });
    },
  };
}
