// apps: launcher icons for PC programs; icons grow with the block, up to the whole screen.
// opts: {apps: ["telegram", ...]} — ids from the layout's app list, in this order (default: all)
import { h } from '../util.js';

export function render(el, opts, ctx) {
  const card = h('div', 'card blk blk-apps');
  const wrap = h('div', 'apps');
  card.appendChild(wrap);
  el.appendChild(card);
  const all = ctx.layout.apps || [];
  const byId = {};
  all.forEach((a) => { byId[a.id] = a; });
  const list = (opts.apps && opts.apps.length ? opts.apps.map((id) => byId[id]).filter(Boolean) : all);
  const items = [];
  let iconPx = 34;

  if (!list.length) {
    const e = h('div', 'empty', 'Приложения не выбраны: добавьте их в Ven4Docking на ПК');
    wrap.appendChild(e);
  }
  list.forEach((a) => {
    const it = h('div', 'app');
    const ph = h('div', 'ph', String(a.title || a.id).charAt(0).toUpperCase());
    if (a.icon) {
      const img = document.createElement('img');
      img.alt = '';
      img.addEventListener('error', () => {
        if (!img.parentNode) return;
        ph.style.width = ph.style.height = iconPx + 'px';
        ph.style.fontSize = Math.round(iconPx * 0.5) + 'px';
        img.parentNode.replaceChild(ph, img);
      });
      img.src = a.icon;
      it.appendChild(img);
    } else {
      it.appendChild(ph);
    }
    it.appendChild(h('span', 'cap', a.title || a.id));
    it.addEventListener('click', () => {
      it.className = 'app busy';
      ctx.post('/api/v1/launch', { app: a.id }).then(() => {
        it.className = 'app done';
        setTimeout(() => { it.className = 'app'; }, 900);
      }, () => {
        it.className = 'app fail';
        setTimeout(() => { it.className = 'app'; }, 1500);
      });
    });
    wrap.appendChild(it);
    items.push(it);
  });

  return {
    update() {},
    resize(w, hh) {
      const n = items.length;
      if (!n) return;
      // largest square cell (icon + caption) that fits all items
      let best = 40;
      for (let s = 200; s >= 40; s -= 2) {
        const cols = Math.floor((w - 8) / s), rows = Math.floor((hh - 8) / (s * 1.1));
        if (cols >= 1 && rows >= 1 && cols * rows >= n) { best = s; break; }
      }
      const icon = iconPx = Math.round(best * 0.6);
      const font = Math.max(10, Math.min(16, Math.round(best * 0.14)));
      items.forEach((it) => {
        it.style.width = best + 'px';
        const pic = it.firstChild;
        pic.style.width = pic.style.height = icon + 'px';
        if (pic.className === 'ph') pic.style.fontSize = Math.round(icon * 0.5) + 'px';
        it.lastChild.style.fontSize = font + 'px';
        it.lastChild.style.maxWidth = (best - 4) + 'px';
      });
      // single row → horizontal scroll strip, otherwise wrap into a grid
      const fitsOneRow = Math.floor((hh - 8) / (best * 1.1)) <= 1;
      wrap.className = fitsOneRow ? 'apps strip' : 'apps wrapped';
    },
  };
}
