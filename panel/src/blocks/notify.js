// notify: the latest Windows notifications (icon, app, time, title / text as the app's mode allows,
// counter). A tap opens the app on the PC, a swipe to the left removes the item from the panel
// (POST /api/v1/notify). At night a new notification lifts the brightness for 5 s (nightFlash).
// opts: {max: 5}. state.notify: {access: "allowed|denied|unspecified|off", items: [{id, app, appName,
// icon?, time, title?, text?, count}]}
import { h, pad2 } from '../util.js';
import { frame, reason, press, iconEl } from './common.js';

const FLASH_MS = 5000;
const SWIPE_PX = 12;
const ACCESS = {
  denied: 'Windows не даёт читать уведомления: Параметры → Конфиденциальность и защита → Уведомления → Доступ к уведомлениям',
  unspecified: 'Разрешите доступ к уведомлениям в Ven4Docking на ПК',
  off: 'Уведомления выключены в Ven4Docking на ПК',
};

function timeText(t, now) {
  if (!t) return '';
  if (t > 1e12) t /= 1000;
  const age = now - t;
  if (age < 60) return 'сейчас';
  if (age < 3600) return Math.round(age / 60) + ' мин';
  const d = new Date(t * 1000);
  const hm = pad2(d.getHours()) + ':' + pad2(d.getMinutes());
  return d.toDateString() === new Date(now * 1000).toDateString() ? hm : pad2(d.getDate()) + '.' + pad2(d.getMonth() + 1) + ' ' + hm;
}

export function render(el, opts, ctx) {
  const f = frame(el, 'notify', 'Уведомления');
  const list = h('div', 'nt-list nodrag');
  f.body.appendChild(list);
  const max = Math.max(1, Math.min(20, opts.max | 0 || 5));
  const gone = {};        // id → true: removed on the panel, the PC may still list it for a poll or two
  let seen = null;        // ids already shown (null until the first answer: nothing is "new" then)
  let sig = '', fitFor = '';

  function nightFlash(s) {
    const l = ctx.layout || {};
    const v = l.notify && l.notify.nightFlash != null ? l.notify.nightFlash : s.notify && s.notify.nightFlash;
    return v !== false;
  }

  function row(it, now) {
    const r = h('div', 'nt');
    const ic = iconEl(it.icon, it.appName || it.app, 'nt-ic');
    const tx = h('div', 'nt-tx');
    const hd = h('div', 'nt-hd');
    hd.appendChild(h('span', 'an', it.appName || it.app || 'Приложение'));
    hd.appendChild(h('span', 'tm num', timeText(it.time, now)));
    tx.appendChild(hd);
    if (it.title) tx.appendChild(h('div', 'ttl', it.title));
    if (it.text) tx.appendChild(h('div', 'txt', it.text));
    r.appendChild(ic); r.appendChild(tx);
    if (it.count > 1) r.appendChild(h('span', 'cnt num', it.count > 99 ? '99+' : String(it.count)));
    if (!it.title && !it.text) r.className = 'nt short';

    let sx = 0, sy = 0, dx = 0, swiping = false, dead = false;
    press(r, {
      start(x, y) { sx = x; sy = y; dx = 0; swiping = false; dead = false; },
      move(x, y, e) {
        if (dead) return;
        dx = x - sx;
        if (!swiping) {
          if (Math.abs(y - sy) > SWIPE_PX && Math.abs(y - sy) > Math.abs(dx)) { dead = true; return; }
          if (dx < -SWIPE_PX) swiping = true; else return;
        }
        if (e && e.cancelable) e.preventDefault();
        r.style.webkitTransform = r.style.transform = 'translateX(' + Math.min(0, dx) + 'px)';
        r.style.opacity = String(Math.max(0.2, 1 + dx / r.clientWidth));
      },
      end(cancelled) {
        if (swiping && !cancelled && dx < -r.clientWidth * 0.3) {
          gone[it.id] = true;
          r.className += ' out';
          setTimeout(() => { if (r.parentNode) r.parentNode.removeChild(r); sig = ''; fitFor = ''; }, 220);
          ctx.post('/api/v1/notify', { op: 'dismiss', id: it.id }).catch(() => {});
        } else if (swiping) {
          r.style.webkitTransform = r.style.transform = '';
          r.style.opacity = '';
        } else if (!dead && !cancelled && Math.abs(dx) < SWIPE_PX) {
          r.className += ' busy';
          ctx.post('/api/v1/notify', { op: 'open', id: it.id }).then(
            () => { r.className = r.className.replace(/ busy/g, ''); },
            () => { r.className = r.className.replace(/ busy/g, '') + ' fail'; });
        }
        swiping = false;
      },
    });
    return r;
  }

  // Whole rows only: the ones that do not fit are counted in the header.
  function fit(total) {
    const els = list.children;
    for (let i = 0; i < els.length; i++) els[i].style.display = '';
    let hidden = total - els.length;
    if (list.clientHeight) {
      const bottom = list.getBoundingClientRect().bottom + 1;
      for (let i = 1; i < els.length; i++) {
        if (els[i].getBoundingClientRect().bottom > bottom) { els[i].style.display = 'none'; hidden++; }
      }
    }
    f.aux.textContent = hidden > 0 ? 'ещё ' + hidden : '';
  }

  return {
    update(s, errors) {
      const why = reason(s, errors, ['notify']);
      const n = s.notify;
      if (why || ACCESS[n.access]) {
        f.aux.textContent = '';
        f.error(why || ACCESS[n.access]);
        return;
      }
      f.error(null);
      const now = s.time || Date.now() / 1000;
      const all = (n.items || []).filter(Boolean);
      Object.keys(gone).forEach((id) => { if (!all.some((it) => String(it.id) === id)) delete gone[id]; });
      const items = all.filter((it) => !gone[it.id]);
      // a new id after the first answer: at night lift the brightness for a moment
      const ids = {};
      let fresh = false;
      items.forEach((it) => { ids[it.id] = true; if (seen && !seen[it.id]) fresh = true; });
      if (fresh && nightFlash(s) && ctx.night) ctx.night.flash(FLASH_MS);
      seen = ids;
      const shown = items.slice(0, max);
      const nsig = shown.map((it) => it.id + ':' + it.count + ':' + timeText(it.time, now)).join('|');
      if (nsig !== sig) {
        sig = nsig;
        list.innerHTML = '';
        if (!shown.length) list.appendChild(h('div', 'empty', 'Новых уведомлений нет'));
        shown.forEach((it) => list.appendChild(row(it, now)));
      }
      const key = nsig + '|' + items.length;
      if (key !== fitFor) { fitFor = key; fit(shown.length ? items.length : 0); }
    },
    resize() { fitFor = ''; },
  };
}
