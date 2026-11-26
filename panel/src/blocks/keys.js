// keys: buttons for the PC actions from the config (shortcut, text, chain of steps, microphone,
// output switch). opts: {actions: ["mute-discord", ...]} — ids; title, icon and kind come from
// layout.actions [{id, title, icon, kind}]. A mic button is red while the default microphone is
// muted (state.audio.mic.muted), an output button shows the current device (state.audio.output.name).
import { h } from '../util.js';
import { tiles, iconEl } from './common.js';

const GLYPHS = {
  keys: '<path fill-rule="evenodd" d="M3 6h18a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zm2 3v2h2V9zm4 0v2h2V9zm4 0v2h2V9zm4 0v2h2V9zM7 13.5v2h10v-2z"/>',
  text: '<path d="M5 4h14v3.5h-2V6h-4v12h2v2H9v-2h2V6H7v1.5H5z"/>',
  macro: '<path d="M13.5 2 4 14h7l-1.5 8L19 10h-7z"/>',
  mic: '<path d="M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.9V21h2v-3.1a7 7 0 0 0 6-6.9z"/>',
  micOff: '<path d="M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.9V21h2v-3.1a7 7 0 0 0 6-6.9z"/><path d="M4.3 3 3 4.3 19.7 21l1.3-1.3z"/>',
  audioOut: '<path d="M12 3a9 9 0 0 0-9 9v7a2 2 0 0 0 2 2h2v-7H5v-2a7 7 0 0 1 14 0v2h-2v7h2a2 2 0 0 0 2-2v-7a9 9 0 0 0-9-9z"/>',
};

function chosen(opts, layout) {
  const byId = {};
  (layout.actions || []).forEach((a) => { if (a && a.id) byId[a.id] = a; });
  return (opts.actions || []).map((id) => byId[id]).filter(Boolean);
}

/** The block needs audio state only when it has a microphone or output button. */
export function req(opts, layout) {
  return chosen(opts, layout).some((a) => a.kind === 'mic' || a.kind === 'audioOut') ? 'audio' : null;
}

export function render(el, opts, ctx) {
  const card = h('div', 'card blk blk-keys');
  const wrap = h('div', 'ks-wrap');
  card.appendChild(wrap);
  el.appendChild(card);
  const list = chosen(opts, ctx.layout || {});
  const items = [];
  let micMuted = null, micHold = 0;

  if (!list.length) wrap.appendChild(h('div', 'empty', 'Кнопки не выбраны: добавьте их в Ven4Docking на ПК'));
  list.forEach((a) => {
    const it = h('div', 'ks ks-' + a.kind);
    it.setAttribute('role', 'button');
    const pic = a.icon ? iconEl(a.icon, a.title || a.id, 'pic') : h('span', 'pic glyph');
    if (!a.icon) pic.innerHTML = '<svg viewBox="0 0 24 24">' + (GLYPHS[a.kind] || GLYPHS.keys) + '</svg>';
    const txt = h('div', 'tx');
    const title = h('div', 'ttl', a.title || a.id);
    const sub = h('div', 'sub');
    txt.appendChild(title); txt.appendChild(sub);
    it.appendChild(pic); it.appendChild(txt);
    wrap.appendChild(it);
    const x = { a, it, pic, title, sub, base: 'ks ks-' + a.kind, state: '' };
    items.push(x);
    it.addEventListener('click', () => {
      if (x.busy) return;
      x.busy = true;
      paint(x, 'busy');
      ctx.post('/api/v1/action', { id: a.id }).then(() => {
        // the microphone answers at once on the panel; the next poll confirms
        if (a.kind === 'mic' && micMuted != null) { micMuted = !micMuted; micHold = Date.now() + 3000; paintAll(); }
        paint(x, 'done');
        setTimeout(() => { x.busy = false; paint(x, ''); }, 700);
      }, () => {
        paint(x, 'fail');
        setTimeout(() => { x.busy = false; paint(x, ''); }, 1500);
      });
    });
  });

  function paint(x, flash) {
    if (flash != null) x.state = flash;
    const off = x.a.kind === 'mic' && micMuted === true;
    x.it.className = x.base + (off ? ' off' : '') + (x.state ? ' ' + x.state : '');
    if (x.a.kind === 'mic' && !x.a.icon) x.pic.firstChild.innerHTML = off ? GLYPHS.micOff : GLYPHS.mic;
  }
  function paintAll() { items.forEach((x) => paint(x)); }

  return {
    update(s) {
      const au = s.audio;
      if (Date.now() > micHold) micMuted = au && au.mic ? !!au.mic.muted : null;
      items.forEach((x) => {
        if (x.a.kind === 'mic') x.sub.textContent = micMuted == null ? '' : micMuted ? 'выключен' : 'включён';
        else if (x.a.kind === 'audioOut') x.sub.textContent = au && au.output && au.output.name ? au.output.name : '';
        paint(x);
      });
    },
    resize(w, hh) {
      if (!items.length) return;
      const t = tiles(items.length, w - 4, hh - 4, 2.1);
      const fontPx = Math.max(10, Math.min(16, Math.round(Math.min(t.th * 0.2, t.tw * 0.09))));
      const iconPx = Math.round(Math.min(t.th * 0.5, t.tw * 0.3, fontPx * 2.6));
      // short and wide: icon on the left; tall: icon above the text
      const stacked = t.th > iconPx + fontPx * 3.6 && t.tw < fontPx * 10;
      items.forEach((x) => {
        x.it.style.width = 'calc(' + (100 / t.cols) + '% - 6px)';
        x.it.style.height = 'calc(' + (100 / t.rows) + '% - 6px)';
        x.it.style.fontSize = fontPx + 'px';
        x.it.className = x.it.className.replace(/ stacked/g, '') + (stacked ? ' stacked' : '');
        x.base = 'ks ks-' + x.a.kind + (stacked ? ' stacked' : '');
        x.pic.style.width = x.pic.style.height = iconPx + 'px';
        x.pic.style.display = iconPx >= 14 ? '' : 'none';
        x.title.style.fontSize = '';
      });
      // a long title gets a smaller font before it is cut with an ellipsis
      items.forEach((x) => {
        if (x.title.scrollWidth > x.title.clientWidth + 1) x.title.style.fontSize = '.85em';
        if (x.title.scrollWidth > x.title.clientWidth + 1) x.title.style.fontSize = '.74em';
      });
    },
  };
}
