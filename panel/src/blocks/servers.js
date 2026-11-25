// servers: ping, CPU, RAM, disk, LA and uptime of the user's servers (SSH inside the agent).
// opts: {servers: ["site", ...] (default: all), cpuWarn, cpuHot, ramWarn, ramHot, diskWarn, diskHot}
import { h, esc, fmtB, levelColor, ago, upFmt, clamp } from '../util.js';
import { frame, reason } from './common.js';

function sbar(lbl, pct, txt, warn, hot) {
  return '<div class="bar"><span class="bl">' + lbl + '</span><div class="track"><div class="fill" style="width:'
    + (pct == null ? 0 : clamp(pct, 0, 100)) + '%;background:' + levelColor(pct, warn, hot) + '"></div></div><span class="v num">' + txt + '</span></div>';
}

export function render(el, opts) {
  const f = frame(el, 'servers', 'Серверы');
  const grid = h('div', 'sv-grid');
  f.body.appendChild(grid);
  let html = '', cardW = '100%', baseCols = 1, fitFor = '';
  const o = (k, d) => (opts[k] != null ? opts[k] : d);

  // All servers must fit the cell: columns by width first, then without the LA line, one more column,
  // and smaller text. Measured in place (fonts differ between phones), only when the count or size changes.
  function fit(n) {
    const bw = f.body.clientWidth, bh = f.body.clientHeight;
    if (!bw || !bh) return;
    const fontPx = parseFloat(window.getComputedStyle(f.card).fontSize) || 12;
    const more = Math.min(n, baseCols + 1);
    const tries = [[baseCols, false, 1], [baseCols, true, 1]];
    if (more > baseCols && bw / more >= fontPx * 12) tries.push([more, true, 1], [more, true, 0.9], [more, true, 0.8]);
    else tries.push([baseCols, true, 0.9], [baseCols, true, 0.8]);
    const cells = grid.children;
    for (let i = 0; i < tries.length; i++) {
      const t = tries[i];
      cardW = (100 / t[0]).toFixed(4) + '%';
      grid.className = 'sv-grid' + (t[1] ? ' compact' : '');
      grid.style.fontSize = t[2] === 1 ? '' : t[2] + 'em';
      for (let j = 0; j < cells.length; j++) cells[j].style.width = cardW;
      if (grid.scrollHeight <= bh + 1) return;
    }
    // still too many: the smallest variant stays and the grid scrolls
  }

  return {
    update(s, errors) {
      const why = reason(s, errors, ['servers']);
      f.error(why);
      if (why) return;
      const list = (s.servers || []).filter((x) => !opts.servers || !opts.servers.length || opts.servers.indexOf(x.id) >= 0);
      if (!list.length) {
        f.aux.textContent = '';
        const e = '<div class="empty">Серверы не заданы: добавьте их в Ven4Docking на ПК</div>';
        if (e !== html) grid.innerHTML = html = e;
        return;
      }
      let ok = 0;
      list.forEach((x) => { if (x.status === 'ok') ok++; });
      f.aux.textContent = ok === list.length ? 'все ' + list.length + ' в норме' : 'в норме ' + ok + ' из ' + list.length;
      f.aux.style.color = ok === list.length ? 'var(--ok)' : 'var(--warn)';
      const out = list.map((x) => {
        const m = x.m || {};
        const col = x.status === 'ok' ? 'var(--ok)' : x.status === 'noagent' ? 'var(--warn)' : 'var(--hot)';
        const pc = x.ping == null || x.ping >= 400 ? 'var(--hot)' : x.ping >= 150 ? 'var(--warn)' : 'var(--text)';
        const ping = x.ping == null ? 'нет пинга' : x.ping + ' мс' + (x.pingKind === 'tcp' ? '*' : '');
        const bars = sbar('ЦП', m.cpu, m.cpu == null ? '—' : '<b>' + Math.round(m.cpu) + '%</b>', o('cpuWarn', 70), o('cpuHot', 90))
          + sbar('ОЗУ', m.ramPct, m.ramPct == null ? '—' : '<b>' + Math.round(m.ramPct) + '%</b> ' + fmtB(m.ramTotal), o('ramWarn', 85), o('ramHot', 95))
          + sbar('Диск', m.diskPct, m.diskPct == null ? '—' : '<b>' + Math.round(m.diskPct) + '%</b> ' + fmtB(m.diskTotal), o('diskWarn', 80), o('diskHot', 90));
        const ft = x.status === 'ok'
          ? '<div class="ft num">LA ' + (m.load ? (+m.load[0]).toFixed(2) : '—') + ' · ядер ' + (m.cores || '—') + ' · вкл. ' + upFmt(m.uptime) + '</div>'
          : '<div class="ft bad">' + esc(x.err || (x.status === 'down' ? 'не отвечает' : 'нет данных'))
            + (x.age != null ? ' · данные ' + ago(x.age) + ' назад' : '') + '</div>';
        return '<div class="sv-cell" style="width:' + cardW + '"><div class="sv' + (x.status === 'down' ? ' down' : '') + (x.status !== 'ok' ? ' old' : '') + '">'
          + '<div class="sv-hd"><i class="dot" style="background:' + col + '"></i><span class="nm">' + esc(x.name) + '</span>'
          + '<span class="pg num" style="color:' + pc + '">' + ping + '</span></div><div class="bars">' + bars + '</div>' + ft + '</div></div>';
      }).join('');
      if (out !== html) grid.innerHTML = html = out;
      const key = String(list.length);
      if (key !== fitFor) {
        fitFor = key;
        fit(list.length);
        html = ''; // the next update renders with the chosen width
      }
    },
    resize(w) {
      const fontPx = parseFloat(window.getComputedStyle(f.card).fontSize) || 12;
      baseCols = Math.max(1, Math.floor((w - 8) / (fontPx * 17)));
      cardW = (100 / baseCols).toFixed(4) + '%';
      fitFor = ''; // re-fit on the next update
      html = ''; // force re-render with the new width on next update
    },
  };
}
