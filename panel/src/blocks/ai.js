// ai: neural-network limits read from files on the PC, with the age of every reading.
// opts: {sources: ["claude-code", "codex", ...]} (default: everything the agent sends)
import { h, esc, until, loadColor, pad2 } from '../util.js';
import { frame, reason } from './common.js';

const MONTHS = ['янв', 'фев', 'мар', 'апр', 'мая', 'июн', 'июл', 'авг', 'сен', 'окт', 'ноя', 'дек'];

function ageText(t, now) {
  if (!t) return { text: 'нет данных', old: true };
  const age = now - t;
  const at = new Date(t * 1000);
  const hm = pad2(at.getHours()) + ':' + pad2(at.getMinutes());
  if (age < 90) return { text: 'данные сейчас', old: false };
  if (age < 3600) return { text: 'данные ' + Math.round(age / 60) + ' мин назад', old: false };
  if (at.toDateString() === new Date(now * 1000).toDateString()) return { text: 'данные от ' + hm, old: true };
  return { text: 'данные от ' + at.getDate() + ' ' + MONTHS[at.getMonth()] + ' ' + hm, old: true };
}

export function render(el, opts) {
  const f = frame(el, 'ai', 'Нейросети');
  const list = h('div', 'ai-list');
  f.body.appendChild(list);
  let html = '';

  return {
    update(s, errors) {
      const why = reason(s, errors, ['ai']);
      f.error(why);
      if (why) return;
      const now = s.time || Date.now() / 1000;
      const src = (s.ai || []).filter((a) => !opts.sources || !opts.sources.length || opts.sources.indexOf(a.id) >= 0);
      if (!src.length) { f.error('Источники не найдены'); return; }
      const out = src.map((a) => {
        const age = ageText(a.t, now);
        let rows = '';
        (a.windows || []).forEach((w) => {
          const fresh = !w.stale && w.pct != null;
          rows += '<div class="bar"><span class="bl">' + esc(w.label) + '</span><div class="track"><div class="fill" style="width:'
            + (fresh ? Math.min(100, w.pct) : 0) + '%;background:' + loadColor(w.pct) + '"></div></div><span class="v num">'
            + (fresh ? '<b>' + Math.round(w.pct) + '%</b> ' + until(w.resetsAt, now) : 'нет свежих') + '</span></div>';
        });
        if (a.model) rows += '<div class="sub">модель: ' + esc(a.model) + '</div>';
        if (!rows) rows = '<div class="sub">установлено, лимиты недоступны</div>';
        return '<div class="ai-src"><div class="ai-hd"><span class="nm">' + esc(a.name) + '</span><span class="age' + (age.old ? ' old' : '')
          + '">' + age.text + '</span></div><div class="bars">' + rows + '</div></div>';
      }).join('');
      if (out !== html) { list.innerHTML = html = out; fit(); }
    },
    resize() { fit(); },
  };

  // A small cell must not cut a row in half: tighter rows first, then whole rows only and "ещё N" in the header.
  function fit() {
    f.aux.textContent = '';
    list.className = 'ai-list';
    const shown = list.querySelectorAll('.ai-src, .ai-hd, .bar, .sub');
    for (let i = 0; i < shown.length; i++) shown[i].style.display = '';
    // the list scrolls inside the body: it overflows when its content is taller than its own box
    const over = () => list.scrollHeight > list.clientHeight + 1;
    const bottom = () => list.getBoundingClientRect().bottom + 1;
    if (!list.clientHeight || !over()) return;
    list.className = 'ai-list tight';
    if (!over()) return;
    list.scrollTop = 0;
    const rows = list.querySelectorAll('.bar, .sub');
    let hidden = 0;
    for (let i = 0; i < rows.length; i++) {
      if (rows[i].getBoundingClientRect().bottom > bottom()) { rows[i].style.display = 'none'; hidden++; }
    }
    const srcs = list.querySelectorAll('.ai-src');
    for (let i = 0; i < srcs.length; i++) {
      const hd = srcs[i].querySelector('.ai-hd');
      const left = srcs[i].querySelectorAll('.bar:not([style*="none"]), .sub:not([style*="none"])').length;
      if (!left || (hd && hd.getBoundingClientRect().bottom > bottom())) srcs[i].style.display = 'none';
    }
    if (hidden) f.aux.textContent = 'ещё ' + hidden;
  }
}
