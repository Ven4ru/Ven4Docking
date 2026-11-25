// Talks to the agent. The token never travels in the URL: on the phone the launcher hands it over
// through the bridge (DockAndroid.token(), answered only for the paired agent's origin); in the
// editor's WebView2 preview the host may inject window.__V4D_TOKEN; for debugging in a desktop
// browser it can be put into localStorage "v4d.token".

const TIMEOUT_MS = 6000;

export function token() {
  try {
    if (window.DockAndroid && typeof window.DockAndroid.token === 'function') {
      return window.DockAndroid.token() || '';
    }
  } catch (e) { /* bridge without token() */ }
  if (window.__V4D_TOKEN) return String(window.__V4D_TOKEN);
  try { return window.localStorage.getItem('v4d.token') || ''; } catch (e) { return ''; }
}

function withTimeout(p, ms) {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error('timeout')), ms);
    p.then((v) => { clearTimeout(t); resolve(v); }, (e) => { clearTimeout(t); reject(e); });
  });
}

export class HttpError extends Error {
  constructor(status, body) { super('HTTP ' + status); this.status = status; this.body = body || null; }
}

/** Non-2xx: reject with HttpError carrying the JSON body if there is one ({"ok":false,"reason":…}). */
function fail(r) {
  return r.json().then((b) => { throw new HttpError(r.status, b); }, () => { throw new HttpError(r.status); });
}

function headers(json) {
  const hd = { Accept: 'application/json' };
  const tk = token();
  if (tk) hd.Authorization = 'Bearer ' + tk;
  if (json) hd['Content-Type'] = 'application/json';
  return hd;
}

export function getJson(path) {
  return withTimeout(fetch(path, { headers: headers(false), cache: 'no-store', credentials: 'same-origin' }), TIMEOUT_MS)
    .then((r) => { if (!r.ok) throw new HttpError(r.status); return r.json(); });
}

export function postJson(path, body) {
  return withTimeout(fetch(path, { method: 'POST', headers: headers(true), body: JSON.stringify(body), credentials: 'same-origin' }), TIMEOUT_MS)
    .then((r) => (r.ok ? r.json().catch(() => ({})) : fail(r)));
}
