// Shared block frame: a card with a header, a body and an error line.
// A block that did not get its data shows the reason; other blocks keep working.
import { h } from '../util.js';

export function frame(el, type, title) {
  const card = h('div', 'card blk blk-' + type);
  const hd = h('div', 'hd');
  const lbl = h('span', 'lbl', title || '');
  const aux = h('span', 'aux num');
  hd.appendChild(lbl); hd.appendChild(aux);
  const body = h('div', 'body');
  const err = h('div', 'err');
  card.appendChild(hd); card.appendChild(body); card.appendChild(err);
  el.appendChild(card);
  let errShown = null;
  return {
    card, hd, lbl, aux, body,
    /** text → show the reason and fade the body; null → normal. */
    error(text) {
      if (text === errShown) return;
      errShown = text;
      err.textContent = text || '';
      card.className = 'card blk blk-' + type + (text ? ' has-err' : '');
    },
  };
}

/** Reason text for a block whose state key is null or missing. */
export function reason(state, errors, keys) {
  for (let i = 0; i < keys.length; i++) {
    if (errors && errors[keys[i]]) return String(errors[keys[i]]);
  }
  for (let i = 0; i < keys.length; i++) {
    if (state && state[keys[i]] != null) return null;
  }
  return 'Нет данных от ПК';
}

/** Font size for big numbers that fits a w×h box. */
export function bigSize(w, hgt, k) {
  return Math.max(16, Math.min(hgt * 0.42, w * (k || 0.2)));
}

// Ignore the mouse events a WebView synthesizes right after a touch (they would press twice).
let lastTouch = 0;

/**
 * Press tracking for touch (phone) and mouse (editor preview): Chrome 51 has no Pointer Events.
 * hd: {start(x, y, e) → false to ignore this press, move(x, y, e), end(cancelled, e)}.
 */
export function press(el, hd) {
  let active = false;
  function begin(x, y, e) { active = !(hd.start && hd.start(x, y, e) === false); }
  function finish(cancelled, e) { if (!active) return; active = false; if (hd.end) hd.end(cancelled, e); }

  el.addEventListener('touchstart', (e) => {
    lastTouch = Date.now();
    if (e.touches.length !== 1) { finish(true, e); return; }
    begin(e.touches[0].clientX, e.touches[0].clientY, e);
  });
  el.addEventListener('touchmove', (e) => {
    if (active && e.touches.length === 1 && hd.move) hd.move(e.touches[0].clientX, e.touches[0].clientY, e);
  });
  el.addEventListener('touchend', (e) => { lastTouch = Date.now(); finish(false, e); });
  el.addEventListener('touchcancel', (e) => finish(true, e));

  function onMove(e) { if (active && hd.move) hd.move(e.clientX, e.clientY, e); }
  function onUp(e) {
    window.removeEventListener('mousemove', onMove, true);
    window.removeEventListener('mouseup', onUp, true);
    finish(false, e);
  }
  el.addEventListener('mousedown', (e) => {
    if (e.button !== 0 || Date.now() - lastTouch < 1000) return;
    begin(e.clientX, e.clientY, e);
    if (!active) return;
    window.addEventListener('mousemove', onMove, true);
    window.addEventListener('mouseup', onUp, true);
  });
}

/** Best grid for n equal tiles in a w×h box, tiles close to the aspect (width / height): {cols, rows, tw, th}. */
export function tiles(n, w, hgt, aspect) {
  let best = null;
  for (let cols = 1; cols <= Math.max(1, n); cols++) {
    const rows = Math.ceil(n / cols);
    const tw = w / cols, th = hgt / rows;
    const score = Math.min(tw / aspect, th);
    if (!best || score > best.score + 0.01) best = { cols, rows, tw, th, score };
  }
  return best;
}

/** Icon picture from the agent (/icons/…) with a letter in its place when it does not load. */
export function iconEl(src, title, cls) {
  const ph = h('div', 'ph ' + (cls || ''), String(title || '?').charAt(0).toUpperCase());
  if (!src) return ph;
  const img = document.createElement('img');
  img.alt = '';
  img.className = cls || '';
  img.addEventListener('error', () => {
    if (!img.parentNode) return;
    ph.style.cssText = img.style.cssText;
    img.parentNode.replaceChild(ph, img);
  });
  img.src = src;
  return img;
}

/** "1 кадр", "2 кадра", "5 кадров". */
export function plural(n, one, few, many) {
  const a = Math.abs(n) % 100, b = a % 10;
  return a > 10 && a < 20 ? many : b === 1 ? one : b > 1 && b < 5 ? few : many;
}
