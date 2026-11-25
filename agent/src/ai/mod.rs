//! AI plan limits — ONLY files that official apps write themselves and local model servers.
//! No account files, no tokens, no network calls to vendors (spec §6, test `forbidden_sources`).

pub mod claude_code;
pub mod claude_desktop;
pub mod codex;
pub mod lmstudio;
pub mod ollama;

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const FIVE_H: f64 = 5.0 * 3600.0;
pub const WEEK: f64 = 7.0 * 86400.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub id: String,
    pub label: String,
    pub pct: f64,
    pub resets_at: Option<f64>,
    pub stale: bool,
}

/// One reading: time of the measurement + windows.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub t: f64,
    pub windows: Vec<Window>,
    /// Claude Code only: seven_day.resets_at, used to anchor weekly resets of Claude Desktop samples.
    pub week_resets_at: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    pub t: Option<f64>,
    pub windows: Vec<Window>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub last_data: Option<f64>,
}

/// Window is stale when the reading is older than the window or its reset already happened.
pub fn make_window(id: &str, label: &str, pct: f64, resets_at: Option<f64>, t: f64, span: f64, now: f64) -> Window {
    let mut stale = now - t > span;
    if let Some(r) = resets_at {
        if now >= r {
            stale = true;
        }
    }
    Window { id: id.into(), label: label.into(), pct, resets_at, stale }
}

pub fn ru_plural(n: i64, one: &str, few: &str, many: &str) -> String {
    let (m10, m100) = (n % 10, n % 100);
    let w = if m10 == 1 && m100 != 11 {
        one
    } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
        few
    } else {
        many
    };
    format!("{n} {w}")
}

/// Label for a window given in minutes: "5 часов", "7 дней", "30 дней".
pub fn window_label(minutes: i64) -> String {
    if minutes > 0 && minutes % 1440 == 0 {
        ru_plural(minutes / 1440, "день", "дня", "дней")
    } else if minutes > 0 && minutes % 60 == 0 {
        ru_plural(minutes / 60, "час", "часа", "часов")
    } else {
        ru_plural(minutes, "минута", "минуты", "минут")
    }
}

/// Minimal RFC 3339 parser: `YYYY-MM-DDTHH:MM:SS[.frac](Z|±HH:MM)`.
pub fn parse_iso8601(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> { s.get(r)?.parse().ok() };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut i = 19;
    let mut frac = 0.0;
    if i < b.len() && b[i] == b'.' {
        let start = i + 1;
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac = format!("0.{}", &s[start..i]).parse().ok()?;
    }
    let offset = match s.get(i..) {
        Some("Z") | Some("z") | Some("") => 0,
        Some(tz) if tz.len() == 6 && (tz.starts_with('+') || tz.starts_with('-')) && &tz[3..4] == ":" => {
            let sign = if tz.starts_with('-') { -1 } else { 1 };
            sign * (tz[1..3].parse::<i64>().ok()? * 3600 + tz[4..6].parse::<i64>().ok()? * 60)
        }
        _ => return None,
    };
    let days = days_from_civil(y, mo, d);
    Some((days * 86400 + h * 3600 + mi * 60 + se - offset) as f64 + frac)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn unix_now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn mtime(p: &Path) -> Option<f64> {
    let m = std::fs::metadata(p).ok()?.modified().ok()?;
    Some(m.duration_since(UNIX_EPOCH).ok()?.as_secs_f64())
}

/// Text file cached by modification time: re-read only when mtime changes.
#[derive(Default)]
struct Cached {
    path: Option<PathBuf>,
    mtime: Option<f64>,
    text: String,
}

impl Cached {
    fn refresh(&mut self, path: Option<PathBuf>, tail: Option<u64>) -> bool {
        let Some(p) = path else {
            self.path = None;
            self.mtime = None;
            self.text.clear();
            return false;
        };
        let mt = mtime(&p);
        if mt.is_none() {
            self.path = None;
            self.text.clear();
            return false;
        }
        if self.path.as_deref() == Some(p.as_path()) && self.mtime == mt {
            return true;
        }
        let text = match tail {
            Some(n) => read_tail(&p, n),
            None => std::fs::read_to_string(&p).ok(),
        };
        match text {
            Some(t) => {
                self.text = t;
                self.path = Some(p);
                self.mtime = mt;
                true
            }
            None => false,
        }
    }
}

/// Last `n` bytes of a file, starting at a line boundary (std opens with full sharing on Windows).
pub fn read_tail(p: &Path, n: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(p).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(n);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::with_capacity((len - start) as usize);
    f.read_to_end(&mut buf).ok()?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if start > 0 {
        if let Some(i) = text.find('\n') {
            text.drain(..=i);
        }
    }
    Some(text)
}

#[derive(Debug, Clone)]
pub struct AiRoots {
    pub profile: PathBuf,
    pub appdata: PathBuf,
    pub local_appdata: PathBuf,
}

impl AiRoots {
    pub fn from_env() -> AiRoots {
        let get = |k: &str| std::env::var_os(k).map(PathBuf::from).unwrap_or_default();
        AiRoots { profile: get("USERPROFILE"), appdata: get("APPDATA"), local_appdata: get("LOCALAPPDATA") }
    }
}

pub const SOURCES: &[(&str, &str)] = &[
    ("claude-code", "Claude Code"),
    ("claude-desktop", "Claude Desktop"),
    ("codex", "Codex"),
    ("ollama", "Ollama"),
    ("lmstudio", "LM Studio"),
];

pub struct AiReader {
    roots: AiRoots,
    enabled: Vec<String>,
    claude_code_file: Option<PathBuf>,
    lmstudio_port: u16,
    cc: Cached,
    cd: Cached,
    cx: Cached,
    codex_files: Vec<PathBuf>,
    codex_scan_t: f64,
    local_t: f64,
    ollama_model: Option<String>,
    ollama_up: bool,
    lms_model: Option<String>,
    lms_up: bool,
    last: Vec<Source>,
}

impl AiReader {
    pub fn new(roots: AiRoots, enabled: Vec<String>, claude_code_file: Option<PathBuf>, lmstudio_port: Option<u16>) -> AiReader {
        AiReader {
            roots,
            enabled,
            claude_code_file,
            lmstudio_port: lmstudio_port.unwrap_or(1234),
            cc: Cached::default(),
            cd: Cached::default(),
            cx: Cached::default(),
            codex_files: Vec::new(),
            codex_scan_t: f64::MIN,
            local_t: f64::MIN,
            ollama_model: None,
            ollama_up: false,
            lms_model: None,
            lms_up: false,
            last: Vec::new(),
        }
    }

    fn on(&self, id: &str) -> bool {
        self.enabled.iter().any(|e| e == id)
    }

    fn claude_code_candidates(&self) -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Some(p) = &self.claude_code_file {
            v.push(p.clone());
        }
        v.push(self.roots.local_appdata.join("Ven4Docking").join("claude_rate_limits.json"));
        v.push(self.roots.profile.join(".claude").join("rate_limits.json"));
        v
    }

    fn claude_code_path(&self) -> Option<PathBuf> {
        self.claude_code_candidates()
            .into_iter()
            .filter_map(|p| mtime(&p).map(|t| (t, p)))
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p)
    }

    fn desktop_path(&self) -> PathBuf {
        self.roots.appdata.join("Claude").join("plan-usage-history.json")
    }

    fn codex_root(&self) -> PathBuf {
        std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| self.roots.profile.join(".codex"))
    }

    fn scan_codex(&mut self) {
        self.codex_files = codex::newest_rollouts(&self.codex_root().join("sessions"), 5);
    }

    fn installed(&self, id: &str) -> bool {
        match id {
            "claude-code" => self.roots.profile.join(".claude").is_dir() || self.claude_code_path().is_some(),
            "claude-desktop" => self.roots.appdata.join("Claude").is_dir(),
            "codex" => self.codex_root().is_dir(),
            "ollama" => self.ollama_up || self.roots.local_appdata.join("Programs").join("Ollama").is_dir(),
            "lmstudio" => {
                self.lms_up
                    || self.roots.profile.join(".lmstudio").is_dir()
                    || self.roots.local_appdata.join("Programs").join("LM Studio").is_dir()
            }
            _ => false,
        }
    }

    /// Re-read what changed and return enabled + installed sources. Call at most every 5 s.
    pub fn poll(&mut self, now: f64) -> Vec<Source> {
        let mut out = Vec::new();
        let mut week_anchor = None;
        if self.on("claude-code") || self.on("claude-desktop") {
            let p = self.claude_code_path();
            if self.cc.refresh(p, None) {
                let t = self.cc.mtime.unwrap_or(now);
                let r = claude_code::parse(&self.cc.text, t, now);
                week_anchor = r.as_ref().and_then(|r| r.week_resets_at);
                if self.on("claude-code") {
                    out.push(source("claude-code", r.map(|r| (r.t, r.windows)), None));
                }
            } else if self.on("claude-code") && self.installed("claude-code") {
                out.push(source("claude-code", None, None));
            }
        }
        if self.on("claude-desktop") {
            let p = self.desktop_path();
            if self.cd.refresh(Some(p), None) {
                let r = claude_desktop::parse(&self.cd.text, now, week_anchor);
                out.push(source("claude-desktop", r.map(|r| (r.t, r.windows)), None));
            } else if self.installed("claude-desktop") {
                out.push(source("claude-desktop", None, None));
            }
        }
        if self.on("codex") {
            if now - self.codex_scan_t > 30.0 || self.codex_files.is_empty() {
                self.codex_scan_t = now;
                self.scan_codex();
            }
            let mut got = None;
            for f in self.codex_files.clone() {
                if self.cx.refresh(Some(f), Some(256 * 1024)) {
                    if let Some(r) = codex::parse_rollout(&self.cx.text, now) {
                        got = Some((r.t, r.windows));
                        break;
                    }
                }
            }
            if got.is_some() || self.installed("codex") {
                out.push(source("codex", got, None));
            }
        }
        if (self.on("ollama") || self.on("lmstudio")) && now - self.local_t >= 10.0 {
            self.local_t = now;
            if self.on("ollama") {
                let r = crate::util::http_get_local(11434, "/api/ps", Duration::from_millis(300));
                self.ollama_up = r.is_some();
                self.ollama_model = r.and_then(|b| ollama::parse_ps(&b));
            }
            if self.on("lmstudio") {
                let r = crate::util::http_get_local(self.lmstudio_port, "/api/v0/models", Duration::from_millis(300));
                self.lms_up = r.is_some();
                self.lms_model = r.and_then(|b| lmstudio::parse_models(&b));
            }
        }
        if self.on("ollama") && self.installed("ollama") {
            let t = self.ollama_up.then_some(now);
            let mut s = source("ollama", None, self.ollama_model.clone());
            s.t = t;
            out.push(s);
        }
        if self.on("lmstudio") && self.installed("lmstudio") {
            let t = self.lms_up.then_some(now);
            let mut s = source("lmstudio", None, self.lms_model.clone());
            s.t = t;
            out.push(s);
        }
        self.last = out.clone();
        out
    }

    /// For the editor: every known source, installed or not, with the time of its last data.
    pub fn sources(&self) -> Vec<SourceInfo> {
        SOURCES
            .iter()
            .map(|(id, name)| SourceInfo {
                id: id.to_string(),
                name: name.to_string(),
                installed: self.installed(id),
                last_data: self.last.iter().find(|s| s.id == *id).and_then(|s| s.t),
            })
            .collect()
    }
}

fn source(id: &str, r: Option<(f64, Vec<Window>)>, model: Option<String>) -> Source {
    let name = SOURCES.iter().find(|(i, _)| *i == id).map(|(_, n)| *n).unwrap_or(id);
    let (t, windows) = match r {
        Some((t, w)) => (Some(t), w),
        None => (None, Vec::new()),
    };
    Source { id: id.into(), name: name.into(), t, windows, model }
}
