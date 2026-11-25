// Fake layout and state in the contract format. Used by the page in ?demo=1 mode
// (editor preview without an agent) and by dev-server.mjs. No browser or Node APIs here.

export function demoLayout() {
  return {
    version: 1,
    orientation: 'landscape',
    grid: { cols: 8, rows: 4 },
    screens: [
      {
        id: 's1', name: 'Главный', blocks: [
          { id: 'b1', type: 'clock', x: 0, y: 0, w: 3, h: 2, opts: { weather: true } },
          { id: 'b2', type: 'cpu', x: 3, y: 0, w: 2, h: 1, opts: {} },
          { id: 'b3', type: 'gpu', x: 5, y: 0, w: 2, h: 1, opts: {} },
          { id: 'b4', type: 'phone', x: 7, y: 0, w: 1, h: 1, opts: {} },
          { id: 'b5', type: 'ram', x: 3, y: 1, w: 2, h: 1, opts: {} },
          { id: 'b6', type: 'net', x: 5, y: 1, w: 2, h: 1, opts: {} },
          { id: 'b7', type: 'disks', x: 7, y: 1, w: 1, h: 1, opts: {} },
          { id: 'b8', type: 'ai', x: 0, y: 2, w: 3, h: 1, opts: {} },
          { id: 'b9', type: 'media', x: 3, y: 2, w: 5, h: 1, opts: {} },
          { id: 'b10', type: 'apps', x: 0, y: 3, w: 8, h: 1, opts: { apps: ['telegram', 'chrome', 'steam', 'discord', 'obs', 'vscode', 'explorer'] } },
        ],
      },
      {
        id: 's2', name: 'Серверы', blocks: [
          { id: 'b11', type: 'servers', x: 0, y: 0, w: 8, h: 4, opts: {} },
        ],
      },
      {
        id: 's3', name: 'Приложения', blocks: [
          { id: 'b12', type: 'weather', x: 0, y: 0, w: 3, h: 1, opts: {} },
          { id: 'b13', type: 'disks', x: 3, y: 0, w: 5, h: 1, opts: {} },
          { id: 'b14', type: 'apps', x: 0, y: 1, w: 8, h: 3, opts: {} },
        ],
      },
    ],
    night: { from: '23:00', to: '07:00', brightness: 0.02 },
    dimWhenPcOffline: true,
    burnInShift: true,
    weather: { name: 'Москва', lat: 55.75, lon: 37.62 },
    apps: [
      { id: 'telegram', title: 'Telegram', icon: '/icons/telegram.png' },
      { id: 'chrome', title: 'Chrome', icon: '/icons/chrome.png' },
      { id: 'steam', title: 'Steam', icon: '/icons/steam.png' },
      { id: 'discord', title: 'Discord', icon: '/icons/discord.png' },
      { id: 'obs', title: 'OBS Studio', icon: '/icons/obs.png' },
      { id: 'vscode', title: 'VS Code', icon: '/icons/vscode.png' },
      { id: 'explorer', title: 'Проводник', icon: '/icons/explorer.png' },
    ],
  };
}

const GB = 1073741824;

function wave(t, period, lo, hi, phase) {
  return lo + (hi - lo) * (0.5 + 0.5 * Math.sin((t / period) * 2 * Math.PI + (phase || 0)));
}
function r1(v) { return Math.round(v * 10) / 10; }

/** State for the requested block keys (contract names), at Unix time t. */
export function demoState(blocks, t) {
  const want = {};
  (blocks || []).forEach((b) => { want[b] = true; });
  const s = { time: t, host: 'DEMO-PC', uptime: 822000 + Math.round(t % 100000) };
  if (want.cpu) {
    const cores = [];
    for (let i = 0; i < 12; i++) cores.push(Math.round(wave(t + i * 7, 9 + i, 2, i % 5 === 0 ? 85 : 35)));
    s.cpu = { load: r1(wave(t, 30, 4, 38)), cores, freqMhz: Math.round(wave(t, 17, 3400, 4700)), temp: r1(wave(t, 40, 41, 63)), boardTemp: 27.9 };
  }
  if (want.gpu) {
    s.gpus = [{ id: '0x00000000_0x0000D1C2', name: 'NVIDIA GeForce RTX 5070', load: r1(wave(t, 25, 1, 64)), temp: r1(wave(t, 50, 38, 66)), vramUsed: Math.round(wave(t, 60, 1.1, 5.4) * GB), fanRpm: Math.round(wave(t, 50, 0, 1400)) || null }];
  }
  if (want.ram) {
    const used = Math.round(wave(t, 80, 16.2, 19.8) * GB);
    s.ram = { total: 32 * GB, used, pct: r1((used / (32 * GB)) * 100) };
  }
  if (want.disks) {
    s.disks = [
      { mount: 'C:', total: 1000000000000, used: 570000000000, pct: 57.0 },
      { mount: 'D:', total: 2000000000000, used: 1830000000000, pct: 91.5 },
      { mount: 'E:', total: 500000000000, used: 120000000000, pct: 24.0 },
    ];
  }
  if (want.net) {
    s.net = { down: Math.round(wave(t, 11, 200, 2400000)), up: Math.round(wave(t, 7, 900, 160000)), topProcess: { name: 'chrome', cpu: r1(wave(t, 13, 2, 14)) } };
  }
  if (want.media) {
    // every 20 s: a full-featured player, then a browser player without next/prev (VK Music)
    s.media = Math.floor(t / 20) % 2 === 0
      ? { title: 'Lofi beats to code to', artist: 'Chillhop Music', app: 'Spotify', playing: true, canNext: true, canPrev: true, canPlayPause: true }
      : { title: 'Кино — Группа крови', artist: 'VK Музыка', app: 'Chrome', playing: false, canNext: false, canPrev: false, canPlayPause: true };
  }
  if (want.servers) {
    s.servers = [
      { id: 'site', name: 'Сайт', host: '203.0.113.10', status: 'ok', ping: 49, pingKind: 'icmp',
        m: { cpu: r1(wave(t, 20, 2, 18)), cores: 1, load: [0.13, 0.1, 0.05], uptime: 604800, ramTotal: 2040000000, ramUsed: 490000000, ramPct: 24.0, diskTotal: 31000000000, diskUsed: 25400000000, diskPct: 82.0 }, age: 3, err: null },
      { id: 'cdn', name: 'CDN', host: '203.0.113.20', status: 'ok', ping: 172, pingKind: 'tcp',
        m: { cpu: r1(wave(t, 15, 30, 96)), cores: 2, load: [1.42, 1.1, 0.95], uptime: 2592000, ramTotal: 4080000000, ramUsed: 3600000000, ramPct: 88.2, diskTotal: 62000000000, diskUsed: 21000000000, diskPct: 33.9 }, age: 2, err: null },
      { id: 'mon', name: 'Мониторинг', host: '203.0.113.30', status: 'noagent', ping: 61, pingKind: 'icmp', m: null, age: 840, err: 'нет данных от сервера' },
      { id: 'old', name: 'Старый', host: '203.0.113.40', status: 'down', ping: null, pingKind: null, m: null, age: 7200, err: 'таймаут подключения' },
    ];
  }
  if (want.ai) {
    s.ai = [
      { id: 'claude-code', name: 'Claude Code', t: t - 95, windows: [
        { id: 'fiveHour', label: '5 часов', pct: r1(wave(t, 300, 10, 78)), resetsAt: t + 7900, stale: false },
        { id: 'sevenDay', label: 'Неделя', pct: 41.0, resetsAt: t + 280000, stale: false },
      ], model: null },
      { id: 'codex', name: 'Codex', t: t - 5400, windows: [
        { id: 'primary', label: '5 часов', pct: 8.0, resetsAt: t + 3000, stale: true },
      ], model: null },
      { id: 'ollama', name: 'Ollama', t: t - 2, windows: [], model: 'qwen3:8b' },
    ];
  }
  return s;
}
