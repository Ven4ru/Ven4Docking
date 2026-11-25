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
