// Ven4Docking panel: builds screens from the phone's layout (/api/v1/layout), polls only the data
// its blocks need (/api/v1/state?blocks=…), rebuilds without a reload when layout.version changes.
// Screens with forApp (program screens) stay out of the swipe: a plate at the bottom opens one while
// its program is in state.running, «Назад» or a swipe down returns to the screen it was opened from.
// ?demo=1 — fake data, no network (editor preview without an agent).
import { $, h, clamp, forEach } from './util.js';
import { getJson, postJson } from './api.js';
import { demoLayout, demoState } from './demo.js';
import { BLOCKS, reqOf } from './blocks/index.js';
import { press } from './blocks/common.js';
import { setupScreen, placeCell, fitBlock, useGrid } from './layout.js';
import { createPager } from './pager.js';
import { createNight } from './night.js';
import { createWeather } from './weather.js';

const POLL_MS = 1500;          // agent treats a panel as active while it polls within 10 s
const OFFLINE_RETRY_MS = 5000; // spec §11: retry every 5 s while the PC is away
const LAYOUT_CHECK_MS = 15000;
const FAILS_TO_OFFLINE = 3;
const MAX_APP_SCREENS = 5;
const SWIPE_DOWN_PX = 70;
const THEMES = { teal: 1, green: 1, light: 1 };

const params = new URLSearchParams(location.search);
const demo = params.get('demo') === '1';

const track = $('track');
const appLayer = $('appscr');
const appBar = $('appbar');
const pager = createPager($('pager'), track, $('dots'));
const night = createNight($('shift'));
const weather = createWeather(demo);

let layout = null;
let blocks = [];      // {type, req, inst, cell, card}
let reqKeys = [];
let lastState = null;
let fails = 0;
let offline = false;
let authFailed = false;
let pollTimer = null;
let appScreens = [];  // {sc, page} — screens with forApp, outside the pager
let screenMap = [];   // index in layout.screens → {page: i} | {app: i} | null
let appOpen = -1;     // open program screen, -1: none
let running = {};     // lower-case exe names from state.running

const ctx = {
  demo,
  layout: null,
  weather,
  night,
  post(path, body) { return demo ? Promise.resolve({ ok: true }) : postJson(path, body); },
};

// ---------- building ----------

function destroyAll() {
  blocks.forEach((b) => { try { if (b.inst && b.inst.destroy) b.inst.destroy(); } catch (e) { /* ignore */ } });
  blocks = [];
}

function errorCard(cell, text) {
  const c = h('div', 'card blk has-err');
  const e = h('div', 'err', text);
  c.appendChild(e);
  cell.appendChild(c);
}

/** One screen: a page with its blocks inside parent; the state keys its blocks need go into req. */
function buildScreen(sc, parent, cls, cols, rows, req) {
  const page = h('div', cls);
  const box = h('div', 'box');
  page.appendChild(box);
  parent.appendChild(page);
  setupScreen(box, cols, rows);
  if (!sc.blocks || !sc.blocks.length) {
    box.appendChild(h('div', 'empty-screen', 'Пустой экран — добавьте блоки в Ven4Docking на ПК'));
  }
  (sc.blocks || []).forEach((b) => {
    const pos = fitBlock(b, cols, rows);
    if (!pos) return;
    const cell = h('div', 'cell');
    placeCell(cell, pos, cols, rows);
    box.appendChild(cell);
    const def = BLOCKS[b.type];
    if (!def) { errorCard(cell, 'Неизвестный блок «' + b.type + '» — обновите лаунчер'); return; }
    try {
      const key = reqOf(def, b.opts, layout);
      const inst = def.mod.render(cell, b.opts || {}, ctx);
      blocks.push({ type: b.type, req: key, inst, cell, box, cols, rows });
      if (key) req[key] = true;
    } catch (e) {
      cell.innerHTML = '';
      errorCard(cell, 'Ошибка блока: ' + (e && e.message));
    }
  });
  return page;
}

function build(l) {
  const keepPage = layout ? pager.page() : 0;
  const keepApp = appOpen >= 0 && appScreens[appOpen] ? appScreens[appOpen].sc.id : null;
  destroyAll();
  layout = l;
  ctx.layout = l;
  track.innerHTML = '';
  appLayer.innerHTML = '';
  document.body.className = (THEMES[l.theme] ? 'theme-' + l.theme : '') + (useGrid() ? ' grid' : ' nogrid');
  const cols = clamp((l.grid && l.grid.cols) | 0 || 8, 1, 24);
  const rows = clamp((l.grid && l.grid.rows) | 0 || 4, 1, 24);
  const screens = [];
  appScreens = [];
  screenMap = [];
  (l.screens || []).slice(0, 10).forEach((sc) => {
    if (sc && sc.forApp && appScreens.length < MAX_APP_SCREENS) {
      screenMap.push({ app: appScreens.length });
      appScreens.push({ sc, page: null });
    } else if (sc && !sc.forApp) {
      screenMap.push({ page: screens.length });
      screens.push(sc);
    } else {
      screenMap.push(null);
    }
  });
  if (!screens.length) screens.push({ id: 'empty', name: '', blocks: [] });
  const req = {};

  screens.forEach((sc) => buildScreen(sc, track, 'page', cols, rows, req));
  appScreens.forEach((a) => { a.page = buildScreen(a.sc, appLayer, 'page app-page', cols, rows, req); });
  if (appScreens.length) {
    const back = h('div', 'app-back', '‹ Назад');
    back.setAttribute('role', 'button');
    back.addEventListener('click', closeApp);
    appLayer.appendChild(back);
    req.running = true;
  }

  reqKeys = Object.keys(req);
  pager.setCount(screens.length, Math.min(keepPage, screens.length - 1));
  let reopen = -1;
  appScreens.forEach((a, i) => { if (a.sc.id === keepApp) reopen = i; });
  if (reopen >= 0) openApp(reopen); else closeApp();
  passWake(l);
  // demo (editor preview): never dim or shift — the preview must stay readable at night
  night.configure(demo ? Object.assign({}, l, { night: null, dimWhenPcOffline: false, burnInShift: false }) : l);
  weather.configure(l.weather);
  resizeAll();
}

/** Pins the panel to the real viewport (documentElement client size), re-run on every resize. */
function fitViewport() {
  const de = document.documentElement;
  const st = $('shift').style;
  st.width = de.clientWidth + 'px';
  st.height = de.clientHeight + 'px';
}

function resizeAll() {
  fitViewport();
  blocks.forEach((b) => {
    // base font from the grid cell size, so text scales with the phone screen
    const unit = Math.min(b.box.clientWidth / b.cols, b.box.clientHeight / b.rows);
    b.cell.style.fontSize = clamp(Math.round(unit * 0.13), 10, 18) + 'px';
    if (!b.inst.resize) return;
    const card = b.cell.firstChild;
    try { b.inst.resize(card.clientWidth, card.clientHeight); } catch (e) { /* keep going */ }
  });
  if (lastState) updateAll(lastState);
}

let resizeTimer = null;
function onViewportChange() {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => { resizeAll(); pager.go(pager.page(), false); }, 150);
}
window.addEventListener('resize', onViewportChange);
window.addEventListener('orientationchange', onViewportChange);

function updateAll(state) {
  lastState = state;
  setRunning(state.running);
  const errors = state.errors || {};
  blocks.forEach((b) => {
    try { b.inst.update(state, errors); } catch (e) {
      const card = b.cell.firstChild;
      if (card && card.className.indexOf('has-err') < 0) card.className += ' has-err';
      const err = card && card.querySelector('.err');
      if (err) err.textContent = 'Ошибка блока: ' + (e && e.message);
    }
  });
}

// ---------- program screens ----------

function openApp(i) {
  if (!appScreens[i]) return;
  appOpen = i;
  appScreens.forEach((a, k) => { a.page.className = 'page app-page' + (k === i ? ' on' : ''); });
  appLayer.className = 'on';
  $('dots').style.visibility = 'hidden';
  renderBar();
}

function closeApp() {
  appOpen = -1;
  appLayer.className = '';
  $('dots').style.visibility = '';
  renderBar();
}

function setRunning(list) {
  const next = {};
  (Array.isArray(list) ? list : []).forEach((n) => { next[String(n).toLowerCase()] = true; });
  if (Object.keys(next).sort().join('|') === Object.keys(running).sort().join('|')) return;
  running = next;
  renderBar();
}

/** Plates "<screen> · открыть" for the program screens whose program is running. */
function renderBar() {
  appBar.innerHTML = '';
  if (appOpen < 0) {
    appScreens.forEach((a, i) => {
      if (!running[String(a.sc.forApp).toLowerCase()]) return;
      const p = h('div', 'plate');
      p.setAttribute('role', 'button');
      p.appendChild(h('b', null, a.sc.name || a.sc.forApp));
      p.appendChild(document.createTextNode(' · открыть'));
      p.addEventListener('click', () => openApp(i));
      appBar.appendChild(p);
    });
  }
  appBar.className = appBar.children.length ? 'on' : '';
}

// a swipe down on a program screen goes back (not from a slider, a swipeable list or a scrolled list)
(function swipeDown() {
  let sx = 0, sy = 0, armed = false;
  press(appLayer, {
    start(x, y, e) {
      const t = e && e.target;
      if (t && t.closest && t.closest('.nodrag')) return false;
      for (let n = t; n && n !== appLayer; n = n.parentNode) if (n.scrollTop > 0) return false;
      sx = x; sy = y; armed = true;
      return true;
    },
    move(x, y) {
      if (armed && y - sy > SWIPE_DOWN_PX && y - sy > Math.abs(x - sx) * 1.5) { armed = false; closeApp(); }
    },
    end() { armed = false; },
  });
}());
window.addEventListener('keydown', (e) => { if (e.keyCode === 27 && appOpen >= 0) closeApp(); });

// ---------- wake on LAN (the launcher sends the magic packet) ----------

function bridge(name) {
  return window.DockAndroid && typeof window.DockAndroid[name] === 'function' ? window.DockAndroid : null;
}

/** Hands the PC's MAC and broadcast address from the layout to the launcher. */
function passWake(l) {
  if (demo || !l.wol || !l.wol.mac) return;
  try {
    const b = bridge('setWake');
    if (b) b.setWake(String(l.wol.mac), String(l.wol.broadcast || ''));
  } catch (e) { /* older launcher without setWake() */ }
}

function canWake() {
  try {
    const b = bridge('canWake');
    return !!(b && bridge('wake') && b.canWake());
  } catch (e) { return false; }
}

let wakeNoteTimer = null;
$('offWake').addEventListener('click', () => {
  const btn = $('offWake');
  if (btn.className === 'busy') return;
  let sent;
  try { const b = bridge('wake'); sent = !!(b && b.wake() !== false); } catch (e) { sent = false; }
  btn.className = 'busy';
  setTimeout(() => { btn.className = ''; }, 5000);
  $('offNote').textContent = sent
    ? 'Сигнал отправлен. Если ПК не включился за минуту — проверьте «Wake on LAN» в BIOS и в параметрах сетевой карты.'
    : 'Не удалось отправить сигнал включения.';
  clearTimeout(wakeNoteTimer);
  wakeNoteTimer = setTimeout(() => { $('offNote').textContent = ''; }, 60000);
});

// ---------- overlay ----------

function overlay(on, title, why, wake) {
  const off = $('off');
  if (on) { $('offH').textContent = title; $('offWhy').textContent = why; }
  $('offWake').style.display = on && wake ? '' : 'none';
  if (!on || !wake) $('offNote').textContent = '';
  off.className = on ? 'on' : '';
}

function setOffline(v) {
  // the launcher starts looking for the PC on the LAN when its address stops answering
  if (v !== offline) {
    try {
      if (window.DockAndroid && typeof window.DockAndroid.offline === 'function') window.DockAndroid.offline(v);
    } catch (e) { /* older launcher without offline() */ }
  }
  offline = v;
  night.setOffline(v);
  if (!authFailed) overlay(v, 'ПК не отвечает', 'Жду, когда компьютер появится в сети… Повтор каждые 5 секунд.', v && canWake());
}

function setAuthFailed() {
  authFailed = true;
  overlay(true, 'Нет доступа', 'Телефон отвязан от ПК. Привяжите его заново: 5 касаний в левый верхний угол → «Привязать заново».');
}

// ---------- live mode ----------

function loadLayout() {
  return getJson('/api/v1/layout').then((l) => {
    authFailed = false;
    if (!layout || l.version !== layout.version) build(l);
    return l;
  });
}

function schedulePoll() {
  clearTimeout(pollTimer);
  pollTimer = setTimeout(poll, offline ? OFFLINE_RETRY_MS : POLL_MS);
}

function poll() {
  const p = layout ? getJson('/api/v1/state?blocks=' + reqKeys.join(',')) : loadLayout().then(() => null);
  p.then((s) => {
    fails = 0;
    if (authFailed) { authFailed = false; overlay(false); }
    setOffline(false);
    if (s) {
      // optional hint from the agent: saves waiting for the next layout check
      if (s.layoutVersion != null && layout && s.layoutVersion !== layout.version) loadLayout().catch(() => {});
      updateAll(s);
    }
  }, (e) => {
    if (e && e.status === 401) { setAuthFailed(); night.setOffline(false); } else if (++fails >= FAILS_TO_OFFLINE || !layout) setOffline(true);
  }).then(schedulePoll, schedulePoll);
}

function startLive() {
  poll();
  setInterval(() => { if (layout && !offline) loadLayout().catch(() => {}); }, LAYOUT_CHECK_MS);
}

// ---------- demo mode ----------

let demoTimer = null;
function startDemo() {
  build(demoLayout());
  const tick = () => {
    const s = demoState(reqKeys, Date.now() / 1000, layout);
    s.errors = {};
    updateAll(s);
  };
  tick();
  demoTimer = setInterval(tick, POLL_MS);
}

// ---------- host API (editor preview: WebView2 ExecuteScript / PostWebMessage) ----------

window.Ven4Panel = {
  /** Shows a layout (editor preview). Rebuilds even when version is unchanged. */
  setLayout(l) { layout = null; build(typeof l === 'string' ? JSON.parse(l) : l); },
  /** Pushes a state object (stops the demo generator). */
  setState(s) { if (demoTimer) { clearInterval(demoTimer); demoTimer = null; } updateAll(typeof s === 'string' ? JSON.parse(s) : s); },
  /** Switches to a screen by its index in layout.screens (a program screen opens over the others). */
  go(i) {
    const m = screenMap[i];
    if (m && m.app != null) { openApp(m.app); return; }
    closeApp();
    pager.go(m ? m.page : i);
  },
};
if (window.chrome && window.chrome.webview && window.chrome.webview.addEventListener) {
  window.chrome.webview.addEventListener('message', (ev) => {
    const m = ev.data || {};
    if (m.type === 'layout') window.Ven4Panel.setLayout(m.layout);
    else if (m.type === 'state') window.Ven4Panel.setState(m.state);
    else if (m.type === 'go') window.Ven4Panel.go(m.index | 0);
  });
}

forEach(document.querySelectorAll('.boot'), (n) => n.parentNode.removeChild(n));
if (demo) startDemo(); else startLive();
