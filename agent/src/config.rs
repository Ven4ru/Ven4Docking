//! config.json, admin.key and phone tokens.
//!
//! Data dir: `VEN4DOCKING_HOME` or `%APPDATA%\Ven4Docking`. Tokens are stored only as SHA-256 hex.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const AGENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_PORT: u16 = 8765;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    #[serde(default)]
    pub name: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub id: String,
    #[serde(default)]
    pub title: String,
    /// shortcut | exe | url | command
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub args: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

fn default_kind() -> String {
    "exe".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default = "default_user")]
    pub user: String,
    /// "auto" = %USERPROFILE%\.ssh\id_ed25519 / id_rsa, otherwise a path to a private key.
    #[serde(default = "default_key")]
    pub key: String,
}

fn default_ssh_port() -> u16 {
    22
}
fn default_user() -> String {
    "root".into()
}
fn default_key() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Night {
    pub from: String,
    pub to: String,
    pub brightness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Phone {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub token_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub night: Option<Night>,
    #[serde(default = "yes")]
    pub dim_when_pc_offline: bool,
    #[serde(default = "yes")]
    pub burn_in_shift: bool,
    /// Anything the editor stores that the agent does not know about is kept as is.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiCfg {
    #[serde(default = "default_ai")]
    pub enabled: Vec<String>,
    /// Claude Code status-line dump; default `%LOCALAPPDATA%\Ven4Docking\claude_rate_limits.json`,
    /// then `%USERPROFILE%\.claude\rate_limits.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_code_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lmstudio_port: Option<u16>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub fn default_ai() -> Vec<String> {
    ["claude-code", "claude-desktop", "codex", "ollama", "lmstudio"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

impl Default for AiCfg {
    fn default() -> Self {
        AiCfg { enabled: default_ai(), claude_code_file: None, lmstudio_port: None, extra: Map::new() }
    }
}

/// A button of the `keys` block (spec 1.1 §3). Only the fields of its `kind` are used.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// keys | text | macro | mic | audioOut
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    /// Core Audio endpoint ids for `audioOut`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One step of a `macro` action: exactly one of the fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// ms, 1–10000
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<u64>,
    /// app id from `apps`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<String>,
}

pub const ACTION_KINDS: &[&str] = &["keys", "text", "macro", "mic", "audioOut"];
pub const MAX_ACTIONS: usize = 100;
pub const MAX_STEPS: usize = 20;
pub const MAX_WAIT_MS: u64 = 10_000;

/// obs-websocket 5 connection. The password is DPAPI-encrypted by the editor (current user), base64.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObsCfg {
    #[serde(default = "default_obs_host")]
    pub host: String,
    #[serde(default = "default_obs_port")]
    pub port: u16,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password_dpapi: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn default_obs_host() -> String {
    "127.0.0.1".into()
}
fn default_obs_port() -> u16 {
    4455
}

impl Default for ObsCfg {
    fn default() -> Self {
        ObsCfg { host: default_obs_host(), port: default_obs_port(), password_dpapi: String::new(), extra: Map::new() }
    }
}

/// Windows notifications on the panel. `apps`: AppUserModelId -> count | title | full; other apps are hidden.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotifyCfg {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub wall_counts_only: bool,
    #[serde(default = "yes")]
    pub night_flash: bool,
    #[serde(default)]
    pub apps: std::collections::BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for NotifyCfg {
    fn default() -> Self {
        NotifyCfg { enabled: false, wall_counts_only: false, night_flash: true, apps: Default::default(), extra: Map::new() }
    }
}

pub const NOTIFY_MODES: &[&str] = &["count", "title", "full"];

/// Wake-on-LAN target given by hand; without it the adapter with the default route is used.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WolCfg {
    pub mac: String,
    pub broadcast: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather: Option<Weather>,
    #[serde(default)]
    pub apps: Vec<App>,
    #[serde(default)]
    pub servers: Vec<Server>,
    #[serde(default = "default_servers_interval")]
    pub servers_interval: u64,
    #[serde(default)]
    pub ai: AiCfg,
    #[serde(default)]
    pub phones: Vec<Phone>,
    /// Optional LibreHardwareMonitor web server (CPU temperature).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lhm_url: Option<String>,
    #[serde(default = "default_overheat")]
    pub overheat_temp: f64,
    /// Number for the next phone id (`p<N>`). Only grows: a revoked phone's id (and its cached
    /// layout file) is never given to another phone. Missing in older configs -> derived on first use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_phone_id: Option<u64>,
    /// Buttons of the `keys` block (1.1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obs: Option<ObsCfg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<NotifyCfg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wol: Option<WolCfg>,
    /// Daily check of GitHub releases (update.rs).
    #[serde(default = "yes")]
    pub update_check: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}
fn default_theme() -> String {
    "teal".into()
}
fn default_lang() -> String {
    "ru".into()
}
fn default_servers_interval() -> u64 {
    10
}
fn default_overheat() -> f64 {
    45.0
}

impl Default for Config {
    fn default() -> Self {
        serde_json::from_str(r#"{"version":1}"#).expect("default config")
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "io: {e}"),
            ConfigError::Json(e) => write!(f, "json: {e}"),
            ConfigError::Invalid(s) => write!(f, "{s}"),
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}
impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        ConfigError::Json(e)
    }
}

/// Safe identifier for phones, apps, servers (also used as a file name).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, ConfigError> {
        let text = text.trim_start_matches('\u{feff}');
        let cfg: Config = serde_json::from_str(text)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version == 0 {
            return Err(ConfigError::Invalid("version must be >= 1".into()));
        }
        let mut seen = std::collections::HashSet::new();
        for a in &self.apps {
            if !valid_id(&a.id) || !seen.insert(("app", a.id.clone())) {
                return Err(ConfigError::Invalid(format!("bad or duplicate app id: {}", a.id)));
            }
            if !matches!(a.kind.as_str(), "shortcut" | "exe" | "url" | "command") {
                return Err(ConfigError::Invalid(format!("bad app kind: {}", a.kind)));
            }
        }
        for s in &self.servers {
            if !valid_id(&s.id) || !seen.insert(("server", s.id.clone())) {
                return Err(ConfigError::Invalid(format!("bad or duplicate server id: {}", s.id)));
            }
            if s.host.trim().is_empty() {
                return Err(ConfigError::Invalid(format!("server {} has no host", s.id)));
            }
        }
        for p in &self.phones {
            if !valid_id(&p.id) || !seen.insert(("phone", p.id.clone())) {
                return Err(ConfigError::Invalid(format!("bad or duplicate phone id: {}", p.id)));
            }
        }
        if self.actions.len() > MAX_ACTIONS {
            return Err(ConfigError::Invalid(format!("more than {MAX_ACTIONS} actions")));
        }
        for a in &self.actions {
            if !valid_id(&a.id) || !seen.insert(("action", a.id.clone())) {
                return Err(ConfigError::Invalid(format!("bad or duplicate action id: {}", a.id)));
            }
            self.validate_action(a).map_err(|e| ConfigError::Invalid(format!("action {}: {e}", a.id)))?;
        }
        if let Some(o) = &self.obs {
            if !obs_host_allowed(&o.host) {
                return Err(ConfigError::Invalid(format!("obs.host must be this PC or a private network address: {}", o.host)));
            }
            if o.port == 0 {
                return Err(ConfigError::Invalid("obs.port must not be 0".into()));
            }
            if !o.password_dpapi.is_empty() && crate::obs::decode_b64(&o.password_dpapi).is_none() {
                return Err(ConfigError::Invalid("obs.passwordDpapi is not base64".into()));
            }
        }
        if let Some(n) = &self.notify {
            for (aumid, mode) in &n.apps {
                if aumid.trim().is_empty() || aumid.len() > 512 {
                    return Err(ConfigError::Invalid(format!("bad notify app id: {aumid}")));
                }
                if !NOTIFY_MODES.contains(&mode.as_str()) {
                    return Err(ConfigError::Invalid(format!("bad notify mode for {aumid}: {mode}")));
                }
            }
        }
        if let Some(w) = &self.wol {
            if crate::wol::parse_mac(&w.mac).is_none() {
                return Err(ConfigError::Invalid(format!("bad wol.mac: {}", w.mac)));
            }
            if w.broadcast.parse::<std::net::Ipv4Addr>().is_err() {
                return Err(ConfigError::Invalid(format!("bad wol.broadcast: {}", w.broadcast)));
            }
        }
        Ok(())
    }

    fn validate_action(&self, a: &Action) -> Result<(), String> {
        use crate::actions::keys::{parse_combo, MAX_TEXT};
        let text_ok = |t: &str| if t.chars().count() > MAX_TEXT { Err(format!("text longer than {MAX_TEXT} characters")) } else { Ok(()) };
        match a.kind.as_str() {
            "keys" => parse_combo(a.keys.as_deref().unwrap_or("")).map(|_| ()),
            "text" => text_ok(a.text.as_deref().unwrap_or("")),
            "macro" => {
                if a.steps.is_empty() || a.steps.len() > MAX_STEPS {
                    return Err(format!("a macro needs 1-{MAX_STEPS} steps"));
                }
                for (i, s) in a.steps.iter().enumerate() {
                    let n = [s.keys.is_some(), s.text.is_some(), s.wait.is_some(), s.launch.is_some()].iter().filter(|x| **x).count();
                    if n != 1 {
                        return Err(format!("step {}: exactly one of keys, text, wait, launch", i + 1));
                    }
                    if let Some(k) = &s.keys {
                        parse_combo(k).map_err(|e| format!("step {}: {e}", i + 1))?;
                    }
                    if let Some(t) = &s.text {
                        text_ok(t).map_err(|e| format!("step {}: {e}", i + 1))?;
                    }
                    if let Some(w) = s.wait {
                        if !(1..=MAX_WAIT_MS).contains(&w) {
                            return Err(format!("step {}: wait must be 1-{MAX_WAIT_MS} ms", i + 1));
                        }
                    }
                    if let Some(id) = &s.launch {
                        if !self.apps.iter().any(|x| &x.id == id) {
                            return Err(format!("step {}: unknown app {id}", i + 1));
                        }
                    }
                }
                Ok(())
            }
            "mic" => Ok(()),
            "audioOut" => {
                if a.devices.len() > 16 || a.devices.iter().any(|d| d.trim().is_empty() || d.len() > 512) {
                    return Err("bad device list".into());
                }
                Ok(())
            }
            k => Err(format!("bad action kind: {k}")),
        }
    }

    pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            return Config::parse(&text);
        }
        let cfg = Config::default();
        cfg.save(path)?;
        Ok(cfg)
    }

    /// Atomic write (tmp + rename).
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = serde_json::to_string_pretty(self)?;
        write_atomic(path, text.as_bytes())?;
        Ok(())
    }

    /// Phone whose token hashes to a stored hash (constant-time compare).
    pub fn phone_by_token(&self, token: &str) -> Option<&Phone> {
        if token.is_empty() {
            return None;
        }
        let h = hash_token(token);
        self.phones.iter().find(|p| !p.token_hash.is_empty() && ct_eq(p.token_hash.as_bytes(), h.as_bytes()))
    }

    /// Allocates a new phone id `p<N>` from the monotonic counter `nextPhoneId` and advances it.
    /// `taken` are ids used outside the config (e.g. layout files of phones revoked before the counter
    /// existed); for configs without the counter it starts at the highest known number + 1.
    pub fn alloc_phone_id<S: AsRef<str>>(&mut self, taken: &[S]) -> String {
        let highest = self
            .phones
            .iter()
            .map(|p| p.id.as_str())
            .chain(taken.iter().map(|s| s.as_ref()))
            .filter_map(phone_id_number)
            .max()
            .unwrap_or(0);
        let mut n = self.next_phone_id.unwrap_or(0).max(highest + 1).max(1);
        while self.phones.iter().any(|p| p.id == format!("p{n}")) {
            n += 1;
        }
        self.next_phone_id = Some(n + 1);
        format!("p{n}")
    }

    /// On PUT from the editor. The set of phones belongs to the agent (pairing, /phones/token and
    /// /phones/{id}/revoke change it), the editor only edits fields of known phones: a phone paired after
    /// the editor read the config is kept, a phone revoked meanwhile is not brought back. Phones that come
    /// without tokenHash keep the stored one (the editor never sees tokens). The phone id counter never
    /// goes back. An empty `obs.passwordDpapi` keeps the stored one.
    pub fn merge_secrets_from(&mut self, old: &Config) {
        let mut edited = std::mem::take(&mut self.phones);
        self.phones = old
            .phones
            .iter()
            .map(|o| match edited.iter().position(|p| p.id == o.id) {
                Some(i) => {
                    let mut p = edited.swap_remove(i);
                    if p.token_hash.is_empty() {
                        p.token_hash = o.token_hash.clone();
                    }
                    p
                }
                None => o.clone(),
            })
            .collect();
        self.next_phone_id = match (self.next_phone_id, old.next_phone_id) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        // the editor sends an empty OBS password when it was not changed
        if let (Some(new), Some(old)) = (self.obs.as_mut(), old.obs.as_ref()) {
            if new.password_dpapi.is_empty() {
                new.password_dpapi = old.password_dpapi.clone();
            }
        }
    }

    /// Notification settings (defaults when the config has none: off).
    pub fn notify_cfg(&self) -> NotifyCfg {
        self.notify.clone().unwrap_or_default()
    }
}

/// OBS may live on this PC or in the private network (plain ws://, never over the internet).
pub fn obs_host_allowed(host: &str) -> bool {
    let h = host.trim().trim_start_matches('[').trim_end_matches(']');
    if h.eq_ignore_ascii_case("localhost") {
        return true;
    }
    h.parse::<std::net::IpAddr>().map(crate::util::is_private).unwrap_or(false)
}

/// `p12` -> 12; ids of another shape -> None.
pub fn phone_id_number(id: &str) -> Option<u64> {
    let digits = id.strip_prefix('p')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

pub fn hash_token(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

pub fn hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 15) as usize] as char);
    }
    s
}

pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut d = 0u8;
    for (x, y) in a.iter().zip(b) {
        d |= x ^ y;
    }
    d == 0
}

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("os random");
    b
}

/// New phone token: 32 random bytes as hex. Returns (token, hash).
pub fn new_token() -> (String, String) {
    let t = hex(&random_bytes::<32>());
    let h = hash_token(&t);
    (t, h)
}

/// admin.key: 32 random bytes written as hex text; created once, readable only by the user.
pub fn load_or_create_admin_key(path: &Path) -> std::io::Result<String> {
    if let Ok(s) = std::fs::read_to_string(path) {
        let s = s.trim().to_string();
        if s.len() >= 32 {
            return Ok(s);
        }
    }
    let key = hex(&random_bytes::<32>());
    write_atomic(path, key.as_bytes())?;
    restrict_to_user(path);
    Ok(key)
}

/// Remove inherited ACEs and grant the current user only (icacls, no console window).
#[cfg(windows)]
pub fn restrict_to_user(path: &Path) {
    use std::os::windows::process::CommandExt;
    let user = std::env::var("USERNAME").unwrap_or_default();
    if user.is_empty() {
        return;
    }
    let _ = std::process::Command::new("icacls")
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .arg(format!("{user}:F"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000)
        .status();
}

#[cfg(not(windows))]
pub fn restrict_to_user(_path: &Path) {}

/// Where everything lives.
#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub config: PathBuf,
    pub layouts: PathBuf,
    pub icons: PathBuf,
    pub admin_key: PathBuf,
    pub log: PathBuf,
    pub panel: PathBuf,
    pub apk: PathBuf,
}

/// Default panel and APK folders for an agent exe in `exe_dir`:
/// installed layout first (`<exe_dir>\panel\index.html`, `<exe_dir>\apk\`),
/// then the older side-by-side layout (`<exe_dir>\..\panel\dist`, `<exe_dir>\..\apk`).
/// When nothing exists yet, the installed layout is returned.
pub fn default_static_dirs(exe_dir: &Path) -> (PathBuf, PathBuf) {
    let panel_cands = [exe_dir.join("panel"), exe_dir.join("..").join("panel").join("dist")];
    let panel = panel_cands
        .iter()
        .find(|d| d.join("index.html").is_file())
        .unwrap_or(&panel_cands[0])
        .clone();
    let apk_cands = [exe_dir.join("apk"), exe_dir.join("..").join("apk")];
    let apk = apk_cands.iter().find(|d| d.is_dir()).unwrap_or(&apk_cands[0]).clone();
    (panel, apk)
}

impl Paths {
    pub fn from_home(home: PathBuf) -> Paths {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let (panel_default, apk_default) = default_static_dirs(&exe_dir);
        let panel = std::env::var_os("VEN4DOCKING_PANEL_DIR").map(PathBuf::from).unwrap_or(panel_default);
        let apk = std::env::var_os("VEN4DOCKING_APK_DIR").map(PathBuf::from).unwrap_or(apk_default);
        Paths {
            config: home.join("config.json"),
            layouts: home.join("layouts"),
            icons: home.join("icons"),
            admin_key: home.join("admin.key"),
            log: home.join("agent.log"),
            panel,
            apk,
            home,
        }
    }

    pub fn from_env() -> Paths {
        let home = std::env::var_os("VEN4DOCKING_HOME").map(PathBuf::from).unwrap_or_else(|| {
            let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            appdata.join("Ven4Docking")
        });
        Paths::from_home(home)
    }

    pub fn layout_file(&self, phone: &str) -> PathBuf {
        self.layouts.join(format!("{phone}.json"))
    }

    /// Phone ids that have a layout file (also of revoked phones: the files are kept).
    pub fn layout_ids(&self) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(&self.layouts) else { return Vec::new() };
        rd.filter_map(|e| e.ok())
            .filter_map(|e| {
                let p = e.path();
                let is_json = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json"));
                if is_json {
                    p.file_stem().and_then(|s| s.to_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect()
    }

    /// New phone id for `cfg` that was never used here before (counter + existing layout files).
    pub fn alloc_phone_id(&self, cfg: &mut Config) -> String {
        cfg.alloc_phone_id(&self.layout_ids())
    }
}
