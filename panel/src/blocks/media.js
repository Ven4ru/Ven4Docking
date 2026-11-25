// media: what is playing on the PC and media keys.
// opts: {buttons: ["prev","playpause","next","stop","voldown","volup","mute"]}
// state.media.canNext / canPrev / canPlayPause (Windows media session): false → that button is
// disabled (dimmed, not clickable, aria-disabled). Missing flag (older agent) → enabled.
// /api/v1/media answering {"ok":false,"reason":"unsupported"} → a short notice in the block.
import { h } from '../util.js';
import { reason } from './common.js';

const ICONS = {
  prev: '<path d="M6 6h2v12H6zM9.5 12 18 18V6z"/>',
  play: '<path d="M8 5v14l11-7z"/>',
  pause: '<path d="M7 5h4v14H7zM13 5h4v14h-4z"/>',
  next: '<path d="M16 6h2v12h-2zM6 18l8.5-6L6 6z"/>',
  stop: '<path d="M6 6h12v12H6z"/>',
  voldown: '<path d="M5 9v6h4l5 5V4L9 9zM16 11h6v2h-6z"/>',
  volup: '<path d="M5 9v6h4l5 5V4L9 9zM18 8h2v3h3v2h-3v3h-2v-3h-3v-2h3z"/>',
  mute: '<path d="M5 9v6h4l5 5V4L9 9zM16.6 8.6 18 7.2l2 2 2-2 1.4 1.4-2 2 2 2-1.4 1.4-2-2-2 2-1.4-1.4 2-2z"/>',
};
const DEFAULT = ['prev', 'playpause', 'next', 'voldown', 'volup'];
/** button → capability flag in state.media */
const FLAG = { next: 'canNext', prev: 'canPrev', playpause: 'canPlayPause' };
const UNSUPPORTED = 'Не поддерживается этим плеером';
const NOTICE_MS = 2500;

export function render(el, opts, ctx) {
  const card = h('div', 'card blk blk-media');
  const meta = h('div', 'meta');
  const title = h('div', 'title', 'Ничего не играет');
  const artist = h('div', 'artist');
  meta.appendChild(title); meta.appendChild(artist);
  const btns = h('div', 'btns');
  card.appendChild(meta); card.appendChild(btns);
  el.appendChild(card);
  let ppSvg = null;
  const buttons = {};
  let noticeUntil = 0, artistText = '', noticeTimer = null;

  function showArtist() {
    const notice = Date.now() < noticeUntil;
    artist.textContent = notice ? UNSUPPORTED : artistText;
    artist.className = notice ? 'artist notice' : 'artist';
  }
  function notify() {
    noticeUntil = Date.now() + NOTICE_MS;
    showArtist();
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(showArtist, NOTICE_MS + 50);
  }

  (opts.buttons && opts.buttons.length ? opts.buttons : DEFAULT).forEach((name) => {
    const b = h('div', name === 'playpause' ? 'btn main' : 'btn');
    b.setAttribute('role', 'button');
    b.innerHTML = '<svg viewBox="0 0 24 24">' + (ICONS[name === 'playpause' ? 'play' : name] || '') + '</svg>';
    if (name === 'playpause') ppSvg = b.firstChild;
    b.addEventListener('click', () => {
      if (b.getAttribute('aria-disabled') === 'true') return;
      ctx.post('/api/v1/media', { action: name }).then((r) => {
        if (r && r.ok === false && r.reason === 'unsupported') notify();
      }, (e) => {
        if (e && e.body && e.body.reason === 'unsupported') notify();
      });
    });
    btns.appendChild(b);
    buttons[name] = b;
  });

  function setEnabled(name, on) {
    const b = buttons[name];
    if (!b) return;
    b.setAttribute('aria-disabled', on ? 'false' : 'true');
    b.className = (name === 'playpause' ? 'btn main' : 'btn') + (on ? '' : ' disabled');
  }

  return {
    update(s, errors) {
      const why = reason(s, errors, ['media']);
      const m = why ? null : s.media;
      if (why && errors && errors.media) { title.textContent = 'Медиа недоступны'; artistText = why; showArtist(); return; }
      title.textContent = m && m.title ? m.title : 'Ничего не играет';
      artistText = m ? [m.artist, m.app].filter(Boolean).join(' · ') : '';
      showArtist();
      if (ppSvg) ppSvg.innerHTML = m && m.playing ? ICONS.pause : ICONS.play;
      Object.keys(FLAG).forEach((name) => setEnabled(name, !(m && m[FLAG[name]] === false)));
    },
    resize(w, hh) {
      card.className = 'card blk blk-media' + (w < 330 ? ' compact' : '') + (hh > 150 ? ' tall' : '');
    },
    destroy() { clearTimeout(noticeTimer); },
  };
}
