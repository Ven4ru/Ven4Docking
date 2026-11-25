// Stub agent for testing the panel and the launcher without the Rust agent.
// Implements the phone side of the contract (docs/plans: «Контракты между частями») with demo data.
//
//   node dev-server.mjs [--port 18765] [--host 127.0.0.1] [--manual]
//
// --host 127.0.0.1 (default) is enough for the Android emulator (10.0.2.2 is the host's loopback)
// and does not trigger a Windows firewall prompt. Pairing requests are accepted automatically
// after 3 s unless --manual is given (then: POST /admin/pairs/{id}/accept from this PC).
// Env: STUB_ERRORS="net=Адаптер не найден;gpus=Нет датчиков" — blocks that return null + errors.<key>;
//      STUB_APK_VERSION / STUB_APK_CODE — what /apk/latest.json advertises (default 1.0.0 / 1);
//      STUB_MEDIA_UNSUPPORTED="next,prev" — /api/v1/media answers {"ok":false,"reason":"unsupported"}
//      for these actions even when state.media says they are supported (tests the notice).
// Demo media alternates every 20 s between a full player and one with canNext/canPrev = false.
import http from 'node:http';
import dgram from 'node:dgram';
import crypto from 'node:crypto';
import os from 'node:os';
import { readFileSync, writeFileSync, existsSync, mkdirSync, statSync, createReadStream } from 'node:fs';
import { dirname, join, normalize, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { demoLayout, demoState } from './src/demo.js';

const here = dirname(fileURLToPath(import.meta.url));
const arg = (name, def) => { const i = process.argv.indexOf(name); return i > 0 ? process.argv[i + 1] : def; };
const PORT = +arg('--port', process.env.PORT || 18765);
const HOST = arg('--host', '127.0.0.1');
const UDP_PORT = +arg('--udp', 18766);
const MANUAL = process.argv.includes('--manual');
const DIST = join(here, 'dist');
const APK = join(here, '..', 'android', 'app', 'build', 'outputs', 'apk', 'debug', 'app-debug.apk');
const STATE_DIR = join(os.tmpdir(), 'ven4docking-stub');
const TOKENS_FILE = join(STATE_DIR, 'tokens.json');

mkdirSync(STATE_DIR, { recursive: true });
const sha = (s) => crypto.createHash('sha256').update(s).digest('hex');
let tokenHashes = existsSync(TOKENS_FILE) ? JSON.parse(readFileSync(TOKENS_FILE, 'utf8')) : [];
const saveTokens = () => writeFileSync(TOKENS_FILE, JSON.stringify(tokenHashes));

let layout = demoLayout();
const pairs = new Map();      // pairId → {pairId, code, model, android, deviceId, ip, status, token, expires}
const attempts = new Map();   // ip → [times]
let lastPhone = null;

const STUB_ERRORS = {};
(process.env.STUB_ERRORS || '').split(';').filter(Boolean).forEach((kv) => {
  const i = kv.indexOf('=');
  STUB_ERRORS[kv.slice(0, i).trim()] = kv.slice(i + 1).trim();
});

const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

function isPrivate(ip) {
  ip = String(ip || '').replace(/^::ffff:/, '');
  if (ip === '::1') return true;
  const p = ip.split('.').map(Number);
  if (p.length !== 4) return false;
  return p[0] === 10 || p[0] === 127 || (p[0] === 172 && p[1] >= 16 && p[1] <= 31)
    || (p[0] === 192 && p[1] === 168) || (p[0] === 100 && p[1] >= 64 && p[1] <= 127);
}
const isLocal = (ip) => /^(::ffff:)?127\.|^::1$/.test(String(ip));

function send(res, code, body, type) {
  const data = typeof body === 'string' || Buffer.isBuffer(body) ? body : JSON.stringify(body);
  res.writeHead(code, { 'Content-Type': type || 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
  res.end(data);
}

function readBody(req) {
  return new Promise((resolve) => {
    let s = '';
    req.on('data', (c) => { s += c; if (s.length > 65536) req.destroy(); });
    req.on('end', () => { try { resolve(s ? JSON.parse(s) : {}); } catch { resolve(null); } });
  });
}

function authorized(req) {
  const m = /^Bearer\s+(\S+)$/.exec(req.headers.authorization || '');
  return !!m && tokenHashes.includes(sha(m[1]));
}

function apkInfo() {
  if (!existsSync(APK)) return null;
  const buf = readFileSync(APK);
  return {
    version: process.env.STUB_APK_VERSION || '1.0.0',
    versionCode: +(process.env.STUB_APK_CODE || 1),
    url: '/apk/ven4docking.apk',
    sha256: sha(buf),
  };
}

const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'application/javascript; charset=utf-8', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml', '.json': 'application/json' };

function serveStatic(res, path) {
  const rel = normalize(decodeURIComponent(path === '/' ? '/index.html' : path)).replace(/^([/\\])+/, '');
  const file = join(DIST, rel);
  if (!file.startsWith(DIST) || !existsSync(file) || !statSync(file).isFile()) return send(res, 404, { error: 'not found' });
  res.writeHead(200, { 'Content-Type': MIME[extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
  createReadStream(file).pipe(res);
}

const MEDIA = ['playpause', 'next', 'prev', 'stop', 'volup', 'voldown', 'mute'];
const MEDIA_FLAG = { next: 'canNext', prev: 'canPrev', playpause: 'canPlayPause' };
const STUB_MEDIA_UNSUPPORTED = (process.env.STUB_MEDIA_UNSUPPORTED || '').split(',').filter(Boolean);
const REQ_TO_KEY = { gpu: 'gpus' };

const server = http.createServer(async (req, res) => {
  const ip = req.socket.remoteAddress;
  const url = new URL(req.url, 'http://x');
  const p = url.pathname;
  if (!isPrivate(ip)) return send(res, 403, { error: 'forbidden' });

  // ---- pairing (no token) ----
  if (req.method === 'POST' && p === '/pair/start') {
    const now = Date.now();
    const list = (attempts.get(ip) || []).filter((t) => now - t < 60000);
    if (list.length >= 5) return send(res, 429, { error: 'too many attempts' });
    list.push(now); attempts.set(ip, list);
    const b = await readBody(req);
    if (!b) return send(res, 400, { error: 'bad json' });
    const pr = {
      pairId: crypto.randomBytes(8).toString('hex'), code: String(crypto.randomInt(0, 10000)).padStart(4, '0'),
      model: String(b.model || '?'), android: String(b.android || '?'), deviceId: String(b.deviceId || ''),
      ip, status: 'wait', token: null, expires: now + 120000,
    };
    pairs.set(pr.pairId, pr);
    log(`pair/start ${pr.model} Android ${pr.android} from ${ip} → code ${pr.code} (pairId ${pr.pairId})`);
    if (!MANUAL) setTimeout(() => accept(pr.pairId, 'auto'), 3000);
    return send(res, 200, { pairId: pr.pairId, code: pr.code });
  }
  const pm = /^\/pair\/([0-9a-f]+)$/.exec(p);
  if (req.method === 'GET' && pm) {
    const pr = pairs.get(pm[1]);
    if (!pr || (pr.status === 'wait' && Date.now() > pr.expires)) return send(res, 404, { error: 'unknown or expired' });
    if (pr.status === 'ok' && pr.token) {
      const t = pr.token; pr.token = null; // the token is handed out once
      return send(res, 200, { status: 'ok', token: t });
    }
    return send(res, 200, { status: pr.status === 'ok' ? 'ok' : pr.status });
  }

  // ---- admin (localhost only; the real agent also wants X-Admin-Key) ----
  if (p.startsWith('/admin/')) {
    if (!isLocal(ip)) return send(res, 403, { error: 'localhost only' });
    if (req.method === 'GET' && p === '/admin/pairs') {
      return send(res, 200, [...pairs.values()].filter((x) => x.status === 'wait').map(({ pairId, model, android, code, ip: pip }) => ({ pairId, model, android, code, ip: pip })));
    }
    const am = /^\/admin\/pairs\/([0-9a-f]+)\/(accept|deny)$/.exec(p);
    if (req.method === 'POST' && am) {
      const ok = am[2] === 'accept' ? accept(am[1], 'admin') : deny(am[1]);
      return send(res, ok ? 200 : 404, { ok });
    }
    if (req.method === 'GET' && p === '/admin/phones') return send(res, 200, lastPhone ? [lastPhone] : []);
    if (req.method === 'PUT' && /^\/admin\/layouts\/[\w-]+$/.test(p)) {
      const b = await readBody(req);
      if (!b || !Array.isArray(b.screens)) return send(res, 400, { error: 'bad layout' });
      layout = { ...layout, ...b, version: layout.version + 1 };
      log(`layout replaced → version ${layout.version}`);
      return send(res, 200, { version: layout.version });
    }
    if (req.method === 'POST' && p === '/admin/stub/bump') {
      // stub-only: rotate the screens so the panel visibly re-lays out without a reload
      layout = { ...layout, version: layout.version + 1, screens: layout.screens.slice(1).concat(layout.screens.slice(0, 1)) };
      log(`layout bumped → version ${layout.version}`);
      return send(res, 200, { version: layout.version });
    }
    return send(res, 404, { error: 'not found' });
  }

  // ---- updates ----
  if (req.method === 'GET' && p === '/apk/latest.json') {
    const info = apkInfo();
    return info ? send(res, 200, info) : send(res, 404, { error: 'no apk built' });
  }
  if (req.method === 'GET' && p === '/apk/ven4docking.apk') {
    if (!existsSync(APK)) return send(res, 404, { error: 'no apk built' });
    res.writeHead(200, { 'Content-Type': 'application/vnd.android.package-archive', 'Content-Length': statSync(APK).size });
    return createReadStream(APK).pipe(res);
  }

  // ---- phone API (token) ----
  if (p.startsWith('/api/')) {
    if (!authorized(req)) return send(res, 401, { error: 'unauthorized' });
    if (req.method === 'GET' && p === '/api/v1/layout') return send(res, 200, layout);
    if (req.method === 'GET' && p === '/api/v1/state') {
      const want = (url.searchParams.get('blocks') || '').split(',').filter(Boolean);
      const s = demoState(want, Date.now() / 1000);
      s.host = 'DEV-STUB';
      s.errors = {};
      want.forEach((b) => {
        const key = REQ_TO_KEY[b] || b;
        if (STUB_ERRORS[key]) { s[key] = null; s.errors[key] = STUB_ERRORS[key]; }
      });
      return send(res, 200, s);
    }
    if (req.method === 'POST' && p === '/api/v1/launch') {
      const b = await readBody(req);
      const app = b && (layout.apps || []).find((a) => a.id === b.app);
      log(`launch ${b && b.app} → ${app ? 'ok' : '404'}`);
      return app ? send(res, 200, { ok: true }) : send(res, 404, { error: 'unknown app' });
    }
    if (req.method === 'POST' && p === '/api/v1/media') {
      const b = await readBody(req);
      if (!b || !MEDIA.includes(b.action)) { log(`media ${b && b.action} → 400`); return send(res, 400, { error: 'bad action' }); }
      // the session can't do it (flag false in state.media), or forced via STUB_MEDIA_UNSUPPORTED
      const m = demoState(['media'], Date.now() / 1000).media;
      const flag = MEDIA_FLAG[b.action];
      const unsupported = (flag && m && m[flag] === false) || STUB_MEDIA_UNSUPPORTED.includes(b.action);
      log(`media ${b.action} → ${unsupported ? 'unsupported' : 'ok'}`);
      return send(res, 200, unsupported ? { ok: false, reason: 'unsupported' } : { ok: true });
    }
    if (req.method === 'POST' && p === '/api/v1/phone') {
      const b = await readBody(req);
      if (!b) return send(res, 400, { error: 'bad json' });
      lastPhone = { ...b, lastSeen: Date.now() / 1000, ip };
      log('phone report', JSON.stringify(b));
      return send(res, 200, { ok: true });
    }
    return send(res, 404, { error: 'not found' });
  }

  if (req.method === 'GET') return serveStatic(res, p);
  return send(res, 405, { error: 'method not allowed' });
});

function accept(pairId, by) {
  const pr = pairs.get(pairId);
  if (!pr || pr.status !== 'wait' || Date.now() > pr.expires) return false;
  const token = crypto.randomBytes(32).toString('hex');
  tokenHashes.push(sha(token)); saveTokens();
  pr.status = 'ok'; pr.token = token;
  log(`pair ${pairId} accepted (${by})`);
  return true;
}
function deny(pairId) {
  const pr = pairs.get(pairId);
  if (!pr || pr.status !== 'wait') return false;
  pr.status = 'denied';
  log(`pair ${pairId} denied`);
  return true;
}

server.listen(PORT, HOST, () => log(`Ven4Docking stub agent on http://${HOST}:${PORT}/ (dist: ${existsSync(DIST) ? 'ok' : 'MISSING — run npm run build'}; pairing: ${MANUAL ? 'manual' : 'auto-accept after 3 s'})`));

const udp = dgram.createSocket({ type: 'udp4', reuseAddr: true });
udp.on('message', (msg, rinfo) => {
  if (msg.toString().trim() !== 'V4D?1' || !isPrivate(rinfo.address)) return;
  udp.send(JSON.stringify({ name: 'DEV-STUB', port: PORT, ver: '1.0.0' }), rinfo.port, rinfo.address);
  log(`discovery from ${rinfo.address}:${rinfo.port}`);
});
udp.on('error', (e) => log('udp error', e.message));
udp.bind(UDP_PORT, HOST, () => log(`discovery on udp ${HOST}:${UDP_PORT}`));
