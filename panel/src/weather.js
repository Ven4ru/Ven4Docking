// Weather straight from the phone (open-meteo, no key); coordinates come from the layout.
// One fetch every 15 minutes, shared by every clock/weather block.

const WMO = {
  0: '☀️ ясно', 1: '🌤 малооблачно', 2: '⛅ облачно', 3: '☁️ пасмурно', 45: '🌫 туман', 48: '🌫 туман',
  51: '🌦 морось', 53: '🌦 морось', 55: '🌦 морось', 61: '🌧 дождь', 63: '🌧 дождь', 65: '🌧 ливень',
  66: '🌧 ледяной дождь', 67: '🌧 ледяной дождь', 71: '🌨 снег', 73: '🌨 снег', 75: '❄️ снегопад', 77: '🌨 снег',
  80: '🌦 ливень', 81: '🌦 ливень', 82: '⛈ ливень', 85: '🌨 снег', 86: '❄️ снегопад', 95: '⛈ гроза', 96: '⛈ гроза', 99: '⛈ гроза',
};
const EVERY_MS = 15 * 60 * 1000;

export function createWeather(demo) {
  let cfg = null, data = null, err = null, timer = null;
  const subs = [];
  const emit = () => subs.forEach((fn) => fn(data, err, cfg));

  function load() {
    if (demo) {
      data = { temp: -3, text: WMO[71], min: -6, max: -1, wind: 4 };
      err = null; emit(); return;
    }
    if (!cfg || cfg.lat == null || cfg.lon == null) { data = null; err = 'Погода: город не задан в Ven4Docking на ПК'; emit(); return; }
    const url = 'https://api.open-meteo.com/v1/forecast?latitude=' + encodeURIComponent(cfg.lat) + '&longitude=' + encodeURIComponent(cfg.lon)
      + '&current=temperature_2m,weather_code,wind_speed_10m&daily=temperature_2m_max,temperature_2m_min&forecast_days=1&timezone=auto';
    fetch(url, { cache: 'no-store' })
      .then((r) => { if (!r.ok) throw new Error(r.status); return r.json(); })
      .then((j) => {
        data = {
          temp: Math.round(j.current.temperature_2m), text: WMO[j.current.weather_code] || '',
          min: Math.round(j.daily.temperature_2m_min[0]), max: Math.round(j.daily.temperature_2m_max[0]),
          wind: Math.round(j.current.wind_speed_10m),
        };
        err = null; emit();
      })
      .catch(() => { err = 'Погода недоступна (нет интернета на телефоне?)'; emit(); });
  }

  return {
    configure(weatherCfg) {
      const same = cfg && weatherCfg && cfg.lat === weatherCfg.lat && cfg.lon === weatherCfg.lon;
      cfg = weatherCfg || null;
      if (same && data) return;
      if (timer) clearInterval(timer);
      load();
      timer = setInterval(load, EVERY_MS);
    },
    subscribe(fn) { subs.push(fn); fn(data, err, cfg); return () => { const i = subs.indexOf(fn); if (i >= 0) subs.splice(i, 1); }; },
  };
}
