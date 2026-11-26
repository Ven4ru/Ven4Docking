// Night / offline dimming (screen brightness through the launcher, CSS opacity as a fallback)
// and a tiny periodic shift against burn-in. Settings come from the layout (phone settings).
// flash(ms): a new notification lifts the night dimming for a moment.
import { hhmm } from './util.js';

const WAKE_MS = 120000;     // a touch wakes the panel for 2 minutes
const SHIFT_MS = 180000;    // burn-in shift every 3 minutes
const SHIFTS = [[0, 0], [2, 1], [1, 2], [-1, 1]];

export function createNight(shiftEl) {
  let cfg = { night: null, dimWhenPcOffline: true, burnInShift: true };
  let wakeUntil = 0, flashUntil = 0, flashTimer = null, pcOffline = false, lastDim = null, shift = 0;

  function inNight(now) {
    const n = cfg.night;
    if (!n) return false;
    const from = hhmm(n.from), to = hhmm(n.to);
    if (from == null || to == null || from === to) return false;
    const m = now.getHours() * 60 + now.getMinutes();
    return from < to ? m >= from && m < to : m >= from || m < to;
  }

  function apply() {
    const night = inNight(new Date());
    const now = Date.now();
    const dim = (night || (pcOffline && cfg.dimWhenPcOffline)) && now > wakeUntil && now > flashUntil;
    if (dim === lastDim) return;
    lastDim = dim;
    const level = cfg.night && cfg.night.brightness != null ? +cfg.night.brightness : 0.02;
    let bridged = false;
    try {
      if (window.DockAndroid && window.DockAndroid.setBrightness) {
        window.DockAndroid.setBrightness(dim ? level : -1);
        bridged = true;
      }
    } catch (e) { /* old launcher */ }
    document.body.style.opacity = dim ? (bridged ? '0.7' : '0.45') : '1';
  }

  function doShift() {
    if (!cfg.burnInShift) { shiftEl.style.webkitTransform = shiftEl.style.transform = ''; return; }
    shift = (shift + 1) % SHIFTS.length;
    const t = 'translate(' + SHIFTS[shift][0] + 'px,' + SHIFTS[shift][1] + 'px)';
    shiftEl.style.webkitTransform = t;
    shiftEl.style.transform = t;
  }

  const wake = () => { wakeUntil = Date.now() + WAKE_MS; apply(); };
  window.addEventListener('touchstart', wake, true);
  window.addEventListener('mousedown', wake, true);
  setInterval(apply, 5000);
  setInterval(doShift, SHIFT_MS);

  return {
    configure(layout) {
      cfg = {
        night: layout.night || null,
        dimWhenPcOffline: layout.dimWhenPcOffline !== false,
        burnInShift: layout.burnInShift !== false,
      };
      lastDim = null;
      apply();
    },
    setOffline(v) { if (pcOffline !== v) { pcOffline = v; apply(); } },
    /** Undims for ms (only matters while dimmed), then the usual rule applies again. */
    flash(ms) {
      flashUntil = Date.now() + ms;
      apply();
      clearTimeout(flashTimer);
      flashTimer = setTimeout(apply, ms + 50);
    },
  };
}
