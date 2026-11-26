// mixer: volume of the programs that play sound, the output device and the microphone.
// opts: {output: true, mic: true, apps: true}. state.audio: {output: {id, name}, outputs: [{id, name}],
// mic: {name, muted} | null, apps: [{pid, name, title, volume 0..1, muted}]}.
// A slider follows the finger and sends at most 5 times a second (POST /api/v1/audio).
import { h, esc, clamp } from '../util.js';
import { frame, reason, press } from './common.js';

const SEND_MS = 200;
const HOLD_MS = 1500; // after a change the panel keeps its own value until the PC has caught up
const SPEAKER = '<path d="M4 9v6h4l5 5V4L8 9z"/><path d="M15.5 8.5a5 5 0 0 1 0 7l-1.4-1.4a3 3 0 0 0 0-4.2z"/>';
const SPEAKER_OFF = '<path d="M4 9v6h4l5 5V4L8 9z"/><path d="m15 9.4 1.4-1.4 2.1 2.1 2.1-2.1 1.4 1.4-2.1 2.1 2.1 2.1-1.4 1.4-2.1-2.1-2.1 2.1-1.4-1.4 2.1-2.1z"/>';
const MIC = '<path d="M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.9V21h2v-3.1a7 7 0 0 0 6-6.9z"/>';
const MIC_OFF = MIC + '<path d="M4.3 3 3 4.3 19.7 21l1.3-1.3z"/>';

function svg(p) { return '<svg viewBox="0 0 24 24">' + p + '</svg>'; }

export function render(el, opts, ctx) {
  const f = frame(el, 'mixer', 'Микшер');
  const show = (k) => opts[k] !== false;
  const out = h('div', 'mx-row mx-out');
  out.setAttribute('role', 'button');
  out.innerHTML = '<span class="ic">' + svg(SPEAKER) + '</span><span class="nm"></span><span class="chev">▾</span>';
  const mic = h('div', 'mx-row mx-mic');
  mic.setAttribute('role', 'button');
  mic.innerHTML = '<span class="ic">' + svg(MIC) + '</span><span class="nm"></span><span class="tg"></span>';
  const list = h('div', 'mx-apps');
  const pick = h('div', 'mx-pick');
  if (show('output')) f.body.appendChild(out);
  if (show('mic')) f.body.appendChild(mic);
  if (show('apps')) f.body.appendChild(list);
  f.card.appendChild(pick);

  let audio = null, sig = '', fitFor = null, micHold = 0, micMuted = false;
  const rows = {}; // pid → row

  // ---------- output and microphone ----------

  out.addEventListener('click', () => {
    if (!audio || !audio.outputs || !audio.outputs.length) return;
    let html = '<div class="pk-hd">Вывод звука</div>';
    audio.outputs.forEach((o, i) => {
      const on = audio.output && audio.output.id === o.id;
      html += '<div class="pk-it' + (on ? ' on' : '') + '" data-i="' + i + '">' + esc(o.name) + '</div>';
    });
    pick.innerHTML = html;
    pick.className = 'mx-pick on';
  });
  pick.addEventListener('click', (e) => {
    const t = e.target.closest ? e.target.closest('.pk-it') : null;
    pick.className = 'mx-pick';
    if (!t || !audio) return;
    const o = audio.outputs[+t.getAttribute('data-i')];
    if (!o || (audio.output && audio.output.id === o.id)) return;
    out.querySelector('.nm').textContent = o.name;
    ctx.post('/api/v1/audio', { op: 'output', id: o.id }).catch(() => {});
  });
  mic.addEventListener('click', () => {
    if (!audio || !audio.mic) return;
    micMuted = !micMuted;
    micHold = Date.now() + HOLD_MS * 2;
    paintMic();
    ctx.post('/api/v1/audio', { op: 'mic', muted: micMuted }).catch(() => { micHold = 0; });
  });
  function paintMic() {
    mic.className = 'mx-row mx-mic' + (micMuted ? ' off' : '');
    mic.querySelector('.ic').innerHTML = svg(micMuted ? MIC_OFF : MIC);
    mic.querySelector('.tg').textContent = micMuted ? 'выкл' : 'вкл';
  }

  // ---------- program rows ----------

  function makeRow(a) {
    const r = { pid: a.pid, hold: 0, value: 0, muted: false, last: 0, timer: null, pending: null };
    r.el = h('div', 'mx-app');
    r.name = h('span', 'nm');
    r.sl = h('div', 'mx-sl nodrag');
    r.fill = h('div', 'fill');
    r.knob = h('div', 'knob');
    r.sl.appendChild(r.fill); r.sl.appendChild(r.knob);
    r.pct = h('span', 'pct num');
    r.mute = h('span', 'mute');
    r.mute.setAttribute('role', 'button');
    r.el.appendChild(r.name); r.el.appendChild(r.sl); r.el.appendChild(r.pct); r.el.appendChild(r.mute);

    function send() {
      r.timer = null;
      if (r.pending == null) return;
      r.last = Date.now();
      const v = r.pending;
      r.pending = null;
      ctx.post('/api/v1/audio', { op: 'volume', pid: r.pid, value: v }).catch(() => {});
    }
    function set(x) {
      const rect = r.sl.getBoundingClientRect();
      if (!rect.width) return;
      r.value = Math.round(clamp((x - rect.left) / rect.width, 0, 1) * 100) / 100;
      paintRow(r);
      r.pending = r.value;
      const wait = SEND_MS - (Date.now() - r.last);
      if (wait <= 0) send(); else if (!r.timer) r.timer = setTimeout(send, wait);
    }
    press(r.sl, {
      start(x, y, e) { if (e && e.type === 'mousedown') e.preventDefault(); r.hold = Infinity; r.el.className = 'mx-app drag'; set(x); },
      move(x, y, e) { if (e && e.cancelable) e.preventDefault(); set(x); },
      end() { r.hold = Date.now() + HOLD_MS; r.el.className = 'mx-app'; },
    });
    r.mute.addEventListener('click', () => {
      r.muted = !r.muted;
      r.muteHold = Date.now() + HOLD_MS * 2;
      paintRow(r);
      ctx.post('/api/v1/audio', { op: 'appMute', pid: r.pid, muted: r.muted }).catch(() => { r.muteHold = 0; });
    });
    return r;
  }

  function paintRow(r) {
    const pct = Math.round(r.value * 100);
    r.fill.style.width = pct + '%';
    r.knob.style.left = pct + '%';
    r.pct.textContent = pct;
    r.mute.innerHTML = svg(r.muted ? SPEAKER_OFF : SPEAKER);
    r.mute.className = 'mute' + (r.muted ? ' off' : '');
    r.sl.className = 'mx-sl nodrag' + (r.muted ? ' muted' : '');
  }

  // A small cell shows whole rows only: the rest is counted as "ещё N" in the header.
  function fit() {
    f.aux.textContent = '';
    const els = list.children;
    for (let i = 0; i < els.length; i++) els[i].style.display = '';
    if (!list.clientHeight) return;
    const bottom = list.getBoundingClientRect().bottom + 1;
    let hidden = 0;
    for (let i = 0; i < els.length; i++) {
      if (els[i].getBoundingClientRect().bottom > bottom) { els[i].style.display = 'none'; hidden++; }
    }
    if (hidden) f.aux.textContent = 'ещё ' + hidden;
  }

  return {
    update(s, errors) {
      const why = reason(s, errors, ['audio']);
      f.error(why);
      if (why) return;
      audio = s.audio;
      const now = Date.now();
      out.querySelector('.nm').textContent = audio.output && audio.output.name ? audio.output.name : 'Нет устройства вывода';
      mic.style.display = audio.mic ? '' : 'none';
      if (audio.mic) {
        mic.querySelector('.nm').textContent = audio.mic.name || 'Микрофон';
        if (now > micHold) micMuted = !!audio.mic.muted;
        paintMic();
      }
      if (!show('apps')) return;
      const apps = audio.apps || [];
      const nsig = apps.map((a) => a.pid).join(',');
      if (nsig !== sig) {
        sig = nsig;
        list.innerHTML = '';
        Object.keys(rows).forEach((k) => { if (!apps.some((a) => String(a.pid) === k)) delete rows[k]; });
        if (!apps.length) list.appendChild(h('div', 'empty', 'Сейчас ничего не звучит'));
        apps.forEach((a) => {
          if (!rows[a.pid]) rows[a.pid] = makeRow(a);
          list.appendChild(rows[a.pid].el);
        });
      }
      apps.forEach((a) => {
        const r = rows[a.pid];
        r.name.textContent = a.title || a.name || String(a.pid);
        if (now > r.hold) r.value = clamp(+a.volume || 0, 0, 1);
        if (!(now < r.muteHold)) r.muted = !!a.muted;
        paintRow(r);
      });
      if (nsig !== fitFor) { fitFor = nsig; fit(); }
    },
    resize(w) {
      f.body.className = 'body' + (w < 230 ? ' narrow' : '');
      fitFor = null;
      fit();
    },
    destroy() { Object.keys(rows).forEach((k) => clearTimeout(rows[k].timer)); },
  };
}
