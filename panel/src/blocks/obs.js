// obs: OBS Studio through obs-websocket on the PC — stream and recording (lit while running, with the
// time), scenes as buttons (the current one highlighted), audio sources with mute and level,
// "потеряно N кадров · M кбит/с" in the header while streaming.
// opts: {stream: true, record: true, scenes: true, audio: true, stats: true}
import { h, esc, pad2 } from '../util.js';
import { frame, reason, plural } from './common.js';

const MUTE_HOLD_MS = 3000;
const MIC = '<path d="M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.9V21h2v-3.1a7 7 0 0 0 6-6.9z"/>';
const MIC_OFF = MIC + '<path d="M4.3 3 3 4.3 19.7 21l1.3-1.3z"/>';

function hms(s) {
  s = Math.max(0, Math.round(+s || 0));
  const hh = Math.floor(s / 3600), mm = Math.floor((s % 3600) / 60);
  return (hh ? hh + ':' + pad2(mm) : mm) + ':' + pad2(s % 60);
}

export function render(el, opts, ctx) {
  const f = frame(el, 'obs', 'OBS');
  const show = (k) => opts[k] !== false;
  const top = h('div', 'obs-top');
  const scenes = h('div', 'obs-scenes');
  const inputs = h('div', 'obs-inputs');
  const toggles = {};
  ['stream', 'record'].forEach((k) => {
    if (!show(k)) return;
    const b = h('div', 'obs-tg obs-' + k);
    b.setAttribute('role', 'button');
    b.innerHTML = '<i class="dot"></i><span class="nm">' + (k === 'stream' ? 'Стрим' : 'Запись') + '</span><span class="tm num"></span>';
    b.addEventListener('click', () => send({ op: k }, b));
    top.appendChild(b);
    toggles[k] = b;
  });
  if (top.children.length) f.body.appendChild(top);
  if (show('scenes')) f.body.appendChild(scenes);
  if (show('audio')) f.body.appendChild(inputs);
  let scSig = '', inSig = '', fitFor = '';
  const muteHold = {}; // source name → {until, muted}

  function send(body, btn) {
    if (btn) btn.className += ' busy';
    return ctx.post('/api/v1/obs', body).then(() => {
      if (btn) btn.className = btn.className.replace(/ busy/g, '');
    }, () => {
      if (btn) btn.className = btn.className.replace(/ busy/g, '') + ' fail';
      setTimeout(() => { if (btn) btn.className = btn.className.replace(/ fail/g, ''); }, 1500);
    });
  }

  scenes.addEventListener('click', (e) => {
    const t = e.target.closest ? e.target.closest('.chip') : null;
    if (!t || t.className.indexOf(' on') >= 0) return;
    const name = t.getAttribute('data-n');
    const was = scenes.querySelector('.chip.on');
    if (was) was.className = 'chip';
    t.className = 'chip on';
    send({ op: 'scene', name });
  });
  inputs.addEventListener('click', (e) => {
    const t = e.target.closest ? e.target.closest('.obs-in') : null;
    if (!t) return;
    const name = t.getAttribute('data-n');
    const muted = t.className.indexOf('muted') < 0;
    muteHold[name] = { until: Date.now() + MUTE_HOLD_MS, muted };
    paintInput(t, muted);
    send({ op: 'mute', name });
  });
  function paintInput(t, muted) {
    t.className = 'obs-in' + (muted ? ' muted' : '');
    t.querySelector('.ic').innerHTML = '<svg viewBox="0 0 24 24">' + (muted ? MIC_OFF : MIC) + '</svg>';
  }

  // A small cell keeps what matters most: tighter first, then without sources, then without scenes.
  function fit() {
    const bh = f.body.clientHeight;
    if (!bh) return;
    const tries = ['body', 'body tight', 'body tight noin', 'body tight noin nosc'];
    for (let i = 0; i < tries.length; i++) {
      f.body.className = tries[i];
      if (f.body.scrollHeight <= bh + 1) return;
    }
  }

  return {
    update(s, errors) {
      const why = reason(s, errors, ['obs']);
      const o = s.obs;
      if (why || !o.connected) {
        f.aux.textContent = '';
        f.error(why || 'OBS не подключён' + (o.error ? ': ' + o.error : ''));
        return;
      }
      f.error(null);
      if (toggles.stream) {
        toggles.stream.className = 'obs-tg obs-stream' + (o.streaming ? ' on' : '');
        toggles.stream.querySelector('.tm').textContent = o.streaming ? hms(o.streamTime) : '';
      }
      if (toggles.record) {
        toggles.record.className = 'obs-tg obs-record' + (o.recording ? ' on' : '');
        toggles.record.querySelector('.tm').textContent = o.recording ? hms(o.recordTime) : '';
      }
      if (show('stats') && o.streaming) {
        const n = o.droppedFrames | 0;
        f.aux.textContent = 'потеряно ' + n + ' ' + plural(n, 'кадр', 'кадра', 'кадров')
          + (o.kbps != null ? ' · ' + Math.round(o.kbps) + ' кбит/с' : '');
        f.aux.style.color = n > 0 && o.totalFrames && n / o.totalFrames > 0.01 ? 'var(--warn)' : '';
      } else {
        f.aux.textContent = '';
      }

      if (show('scenes')) {
        const list = o.scenes || [];
        const sig = list.join('\n') + '\n' + o.scene;
        if (sig !== scSig) {
          scSig = sig;
          scenes.innerHTML = list.length
            ? list.map((n) => '<span class="chip' + (n === o.scene ? ' on' : '') + '" data-n="' + esc(n) + '">' + esc(n) + '</span>').join('')
            : '<span class="empty">Сцен нет</span>';
        }
      }
      if (show('audio')) {
        const list = o.inputs || [];
        const sig = list.map((x) => x.name).join('\n');
        if (sig !== inSig) {
          inSig = sig;
          inputs.innerHTML = list.map((x) => '<span class="obs-in" data-n="' + esc(x.name) + '"><span class="ic"></span><span class="nm">'
            + esc(x.name) + '</span><i class="lv"><b></b></i></span>').join('');
        }
        const els = inputs.children;
        const now = Date.now();
        list.forEach((x, i) => {
          const t = els[i];
          if (!t) return;
          const held = muteHold[x.name];
          paintInput(t, held && now < held.until ? held.muted : !!x.muted);
          t.querySelector('.lv b').style.width = Math.round(Math.max(0, Math.min(1, +x.level || 0)) * 100) + '%';
        });
      }
      const key = scSig + '|' + inSig;
      if (key !== fitFor) { fitFor = key; fit(); }
    },
    resize(w) {
      top.className = 'obs-top' + (w < 300 ? ' narrow' : '');
      fitFor = '';
    },
  };
}
