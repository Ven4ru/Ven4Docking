// Small helpers shared by all blocks. Everything here must work in Chrome 51.

export function $(id) { return document.getElementById(id); }

/** Creates an element: h('div', 'cls', 'text') */
export function h(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
}

export function esc(t) {
  return String(t == null ? '' : t).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}

const UNITS = ['Б', 'КБ', 'МБ', 'ГБ', 'ТБ'];
export function fmtB(b) {
  if (b == null || isNaN(b)) return '—';
  let i = 0;
  while (b >= 1024 && i < UNITS.length - 1) { b /= 1024; i++; }
  return (b < 10 && i ? b.toFixed(1) : Math.round(b)) + ' ' + UNITS[i];
}
export function fmtRate(b) { return fmtB(b) + '/с'; }

export function loadColor(p) {
  return p == null ? 'var(--mute)' : p >= 90 ? 'var(--hot)' : p >= 70 ? 'var(--warn)' : 'var(--acc)';
}
export function levelColor(p, warn, hot) {
  return p == null ? 'var(--line)' : p >= hot ? 'var(--hot)' : p >= warn ? 'var(--warn)' : 'var(--acc)';
}
export function tempColor(t, warm, hot) {
  return t == null ? '' : t >= hot ? 'var(--hot)' : t >= warm ? 'var(--warn)' : 'var(--ok)';
}

/** "2ч 14м" until a Unix time. */
export function until(ts, now) {
  if (!ts) return '';
  let s = Math.max(0, ts - now);
  const d = Math.floor(s / 86400); s -= d * 86400;
  const hh = Math.floor(s / 3600), mm = Math.floor((s % 3600) / 60);
  return d ? d + 'д ' + hh + 'ч' : hh ? hh + 'ч ' + mm + 'м' : mm + 'м';
}
export function ago(s) {
  return s < 90 ? Math.round(s) + ' с' : s < 5400 ? Math.round(s / 60) + ' мин' : s < 172800 ? Math.round(s / 3600) + ' ч' : Math.round(s / 86400) + ' д';
}
export function upFmt(s) {
  if (s == null) return '';
  return s >= 86400 ? Math.floor(s / 86400) + ' д ' + Math.floor((s % 86400) / 3600) + ' ч' : Math.floor(s / 3600) + ' ч';
}
export function pad2(n) { return n < 10 ? '0' + n : '' + n; }

export function clamp(v, lo, hi) { return v < lo ? lo : v > hi ? hi : v; }

/** Parses "23:00" into minutes after midnight; null when malformed. */
export function hhmm(s) {
  const m = /^(\d{1,2}):(\d{2})$/.exec(String(s || ''));
  return m ? (+m[1]) * 60 + (+m[2]) : null;
}

export function forEach(list, fn) { Array.prototype.forEach.call(list, fn); }

/** Keeps the last n values. */
export function pushHist(arr, v, n) { arr.push(v); while (arr.length > (n || 60)) arr.shift(); }

/** Sparkline polyline into an <svg viewBox="0 0 60 20">. */
export function spark(svg, arr, max) {
  let m = max || 1;
  if (!max) for (let i = 0; i < arr.length; i++) if (arr[i] > m) m = arr[i];
  if (arr.length < 2) { svg.innerHTML = ''; return; }
  const pts = arr.map((v, i) => ((i / (arr.length - 1)) * 60).toFixed(1) + ',' + (20 - (v / m) * 19).toFixed(1)).join(' ');
  svg.innerHTML = '<polyline points="' + pts + '" fill="none" stroke="currentColor" stroke-width="1.2" vector-effect="non-scaling-stroke"/>';
}

export const SVGNS = 'http://www.w3.org/2000/svg';
export function sparkSvg() {
  const s = document.createElementNS(SVGNS, 'svg');
  s.setAttribute('class', 'spark');
  s.setAttribute('viewBox', '0 0 60 20');
  s.setAttribute('preserveAspectRatio', 'none');
  return s;
}

/** Horizontal bar row: label, track, value. Returns {row, set(pct, html, color)}. */
export function barRow(label) {
  const row = h('div', 'bar');
  const l = h('span', 'bl', label);
  const tr = h('div', 'track');
  const fill = h('div', 'fill');
  const v = h('span', 'v num', '—');
  tr.appendChild(fill);
  row.appendChild(l); row.appendChild(tr); row.appendChild(v);
  return {
    row,
    set(pct, html, color) {
      fill.style.width = (pct == null ? 0 : clamp(pct, 0, 100)) + '%';
      fill.style.background = color || loadColor(pct);
      v.innerHTML = html;
    },
  };
}
