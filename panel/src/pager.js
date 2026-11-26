// Horizontal swipe between screens + dots + arrow keys. Chrome 51 has no Pointer Events,
// so touch events (phone) and mouse events (editor preview, desktop browser) are handled separately.
import { h, forEach } from './util.js';

export function createPager(pagerEl, trackEl, dotsEl, onChange) {
  let count = 1, page = 0;
  let sx = null, sy = 0, dx = 0, dragging = false, swallowClick = false;

  function setTransform(v) {
    trackEl.style.webkitTransform = v;
    trackEl.style.transform = v;
  }

  function go(p, animate) {
    page = Math.max(0, Math.min(count - 1, p));
    if (animate === false) trackEl.classList.add('drag'); else trackEl.classList.remove('drag');
    setTransform('translateX(' + (-page * 100 / count) + '%)');
    forEach(dotsEl.children, (d, i) => { d.className = i === page ? 'on' : ''; });
    if (onChange) onChange(page);
  }

  function setCount(n, keepPage) {
    count = Math.max(1, n);
    trackEl.style.width = count * 100 + '%';
    forEach(trackEl.children, (pg) => { pg.style.width = 100 / count + '%'; });
    dotsEl.innerHTML = '';
    if (count > 1) {
      for (let i = 0; i < count; i++) {
        const d = h('i');
        d.addEventListener('click', ((k) => () => go(k))(i));
        dotsEl.appendChild(d);
      }
    }
    go(keepPage == null ? 0 : keepPage, false);
  }

  function start(x, y) { sx = x; sy = y; dx = 0; dragging = false; swallowClick = false; }
  function move(x, y, ev) {
    if (sx === null) return;
    dx = x - sx;
    const dy = y - sy;
    if (!dragging) {
      if (Math.abs(dx) > 12 && Math.abs(dx) > Math.abs(dy) * 1.3 && count > 1) {
        dragging = true; swallowClick = true; trackEl.classList.add('drag');
      } else if (Math.abs(dy) > 12) { sx = null; return; }
    }
    if (dragging) {
      if (ev && ev.cancelable) ev.preventDefault();
      const edge = (page === 0 && dx > 0) || (page === count - 1 && dx < 0);
      setTransform('translateX(' + (-page * pagerEl.clientWidth + (edge ? dx / 3 : dx)) + 'px)');
    }
  }
  function end() {
    if (dragging) {
      const t = pagerEl.clientWidth * 0.15;
      go(dx < -t ? page + 1 : dx > t ? page - 1 : page);
    }
    sx = null; dragging = false;
  }

  // a horizontally scrollable strip (apps) keeps its own swipe, and so do sliders and swipeable
  // lists (.nodrag: mixer volume, notifications)
  function inStrip(t) {
    if (t && t.closest && t.closest('.nodrag')) return true;
    const s = t && t.closest ? t.closest('.strip') : null;
    return !!s && s.scrollWidth > s.clientWidth + 2;
  }

  pagerEl.addEventListener('touchstart', (e) => {
    if (e.touches.length === 1 && !inStrip(e.target)) start(e.touches[0].clientX, e.touches[0].clientY); else sx = null;
  }, true);
  pagerEl.addEventListener('touchmove', (e) => { if (e.touches.length === 1) move(e.touches[0].clientX, e.touches[0].clientY, e); }, true);
  pagerEl.addEventListener('touchend', end, true);
  pagerEl.addEventListener('touchcancel', end, true);

  let mouseDown = false;
  pagerEl.addEventListener('mousedown', (e) => {
    if (e.button === 0 && !(e.target && e.target.closest && e.target.closest('.nodrag'))) { mouseDown = true; start(e.clientX, e.clientY); }
  }, true);
  window.addEventListener('mousemove', (e) => { if (mouseDown) move(e.clientX, e.clientY, e); }, true);
  window.addEventListener('mouseup', () => { if (mouseDown) { mouseDown = false; end(); } }, true);

  pagerEl.addEventListener('click', (e) => {
    if (swallowClick) { e.stopPropagation(); e.preventDefault(); swallowClick = false; }
  }, true);
  window.addEventListener('keydown', (e) => {
    if (e.keyCode === 39) go(page + 1);
    if (e.keyCode === 37) go(page - 1);
  });
  window.addEventListener('resize', () => go(page, false));

  return { setCount, go, page: () => page, count: () => count };
}
