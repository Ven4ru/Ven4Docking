//! OBS block (spec 1.1 §6): obs-websocket 5 client in a tokio task. The connection is held while a panel with
//! the `obs` block is active; without OBS — a new try every 10 s and `connected: false` with the reason.
//! Plain ws:// to this PC or the private network; the password comes DPAPI-encrypted from the config.

use crate::config::ObsCfg;
use crate::state::AppState;
use base64::prelude::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

/// Pause between tries while OBS is not there.
pub const RETRY: Duration = Duration::from_secs(10);
/// Connect, handshake and one request.
const TIMEOUT: Duration = Duration::from_secs(3);
/// InputVolumeMeters (a high-volume event, 1 << 16): levels of the audio sources, ~20 per second.
const EVENTS_METERS: u64 = 1 << 16;
/// Close codes of obs-websocket.
const CLOSE_AUTH_FAILED: u16 = 4009;
const CLOSE_RPC_VERSION: u16 = 4010;

pub fn decode_b64(s: &str) -> Option<Vec<u8>> {
    BASE64_STANDARD.decode(s.trim()).ok()
}

/// Protocol 5 authentication: base64(sha256(base64(sha256(password + salt)) + challenge)).
pub fn auth_string(password: &str, salt: &str, challenge: &str) -> String {
    let secret = BASE64_STANDARD.encode(Sha256::digest(format!("{password}{salt}").as_bytes()));
    BASE64_STANDARD.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}

/// The password from `passwordDpapi` (UTF-8 encrypted by the editor for the current user). None = no password.
#[cfg(windows)]
pub fn password(cfg: &ObsCfg) -> Result<Option<String>, String> {
    if cfg.password_dpapi.is_empty() {
        return Ok(None);
    }
    let bad = || "пароль OBS не расшифровывается — задайте его заново в редакторе".to_string();
    let raw = decode_b64(&cfg.password_dpapi).ok_or_else(bad)?;
    let plain = crate::platform::dpapi_unprotect(&raw).map_err(|_| bad())?;
    String::from_utf8(plain).map(Some).map_err(|_| bad())
}

/// POST /api/v1/obs.
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    Stream,
    Record,
    Scene(String),
    Mute(String),
}

impl Cmd {
    pub fn parse(op: &str, name: Option<&str>) -> Option<Cmd> {
        let name = name.map(str::trim).filter(|n| !n.is_empty() && n.len() <= 256).map(String::from);
        Some(match op {
            "stream" => Cmd::Stream,
            "record" => Cmd::Record,
            "scene" => Cmd::Scene(name?),
            "mute" => Cmd::Mute(name?),
            _ => return None,
        })
    }

    fn request(&self) -> (&'static str, Value) {
        match self {
            Cmd::Stream => ("ToggleStream", json!({})),
            Cmd::Record => ("ToggleRecord", json!({})),
            Cmd::Scene(n) => ("SetCurrentProgramScene", json!({"sceneName": n})),
            Cmd::Mute(n) => ("ToggleInputMute", json!({"inputName": n})),
        }
    }
}

/// A command and where its result goes.
pub type CmdMsg = (Cmd, oneshot::Sender<Result<(), String>>);

enum Fail {
    /// the connection is gone
    Lost(String),
    /// OBS answered «no» (e.g. no stream settings), the connection is fine
    Refused(String),
}

fn lost(e: impl std::fmt::Display) -> Fail {
    Fail::Lost(format!("связь с OBS потеряна ({e})"))
}

fn close_reason(code: Option<u16>) -> String {
    match code {
        Some(CLOSE_AUTH_FAILED) => "неверный пароль".into(),
        Some(CLOSE_RPC_VERSION) => "нужен OBS WebSocket 5 (OBS 28 и новее)".into(),
        _ => "OBS закрыл соединение".into(),
    }
}

async fn obs_running() -> bool {
    tokio::task::spawn_blocking(|| {
        let names = crate::platform::process_names();
        ["obs64.exe", "obs32.exe", "obs.exe"].iter().any(|n| names.contains(*n))
    })
    .await
    .unwrap_or(false)
}

fn is_local(host: &str) -> bool {
    let h = host.trim_start_matches('[').trim_end_matches(']');
    h.eq_ignore_ascii_case("localhost") || h.parse::<std::net::IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false)
}

struct Conn {
    ws: WebSocketStream<TcpStream>,
    next: u64,
    /// audio sources from the last InputVolumeMeters: (name, level 0..1)
    inputs: Vec<(String, f64)>,
}

impl Conn {
    /// Connect + Hello/Identify. Returns the connection and the obs-websocket version.
    async fn open(cfg: &ObsCfg, events: u64) -> Result<(Conn, String), String> {
        if !crate::config::obs_host_allowed(&cfg.host) {
            return Err("адрес OBS должен быть в домашней сети".into());
        }
        #[cfg(windows)]
        let password = password(cfg)?;
        #[cfg(not(windows))]
        let password: Option<String> = None;
        let host = cfg.host.trim();
        let host = if host.contains(':') && !host.starts_with('[') { format!("[{host}]") } else { host.to_string() };
        let addr = format!("{host}:{}", cfg.port);
        let tcp = match tokio::time::timeout(TIMEOUT, TcpStream::connect(&addr)).await {
            Ok(Ok(s)) => s,
            _ if is_local(&host) => {
                return Err(if obs_running().await { "WebSocket выключен в OBS".into() } else { "OBS не запущен".into() });
            }
            _ => return Err(format!("OBS не отвечает по адресу {addr}")),
        };
        let _ = tcp.set_nodelay(true);
        let (ws, _) = tokio::time::timeout(TIMEOUT, tokio_tungstenite::client_async(format!("ws://{addr}/"), tcp))
            .await
            .map_err(|_| "OBS не отвечает".to_string())?
            .map_err(|_| "по этому адресу не OBS WebSocket".to_string())?;
        let mut c = Conn { ws, next: 0, inputs: Vec::new() };
        let hello = c.recv_op(0).await?;
        let mut identify = json!({"rpcVersion": 1, "eventSubscriptions": events});
        if let Some(auth) = hello.get("authentication") {
            let Some(pw) = password else { return Err("OBS просит пароль — укажите его в настройках".into()) };
            let salt = auth["salt"].as_str().unwrap_or("");
            let challenge = auth["challenge"].as_str().unwrap_or("");
            identify["authentication"] = json!(auth_string(&pw, salt, challenge));
        }
        c.send(json!({"op": 1, "d": identify})).await.map_err(|_| "OBS закрыл соединение".to_string())?;
        c.recv_op(2).await?;
        let version = hello["obsWebSocketVersion"].as_str().unwrap_or("").to_string();
        Ok((c, version))
    }

    async fn send(&mut self, v: Value) -> Result<(), Fail> {
        self.ws.send(Message::text(v.to_string())).await.map_err(lost)
    }

    /// Waits for a message with opcode `op` during the handshake; returns its `d`.
    async fn recv_op(&mut self, op: u64) -> Result<Value, String> {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let m = tokio::time::timeout_at(deadline, self.ws.next()).await.map_err(|_| "OBS не отвечает".to_string())?;
            match m {
                Some(Ok(Message::Text(t))) => {
                    let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                    if v["op"].as_u64() == Some(op) {
                        return Ok(v["d"].clone());
                    }
                }
                Some(Ok(Message::Close(f))) => return Err(close_reason(f.map(|f| u16::from(f.code)))),
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return Err("OBS закрыл соединение".into()),
            }
        }
    }

    /// A message outside a request: events (levels), close.
    fn on_message(&mut self, m: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>) -> Result<(), Fail> {
        match m {
            Some(Ok(Message::Text(t))) => {
                if let Ok(v) = serde_json::from_str::<Value>(t.as_str()) {
                    if v["op"].as_u64() == Some(5) {
                        self.on_event(&v["d"]);
                    }
                }
                Ok(())
            }
            Some(Ok(Message::Close(f))) => Err(Fail::Lost(close_reason(f.map(|f| u16::from(f.code))))),
            Some(Ok(_)) => Ok(()),
            Some(Err(e)) => Err(lost(e)),
            None => Err(Fail::Lost("OBS закрыл соединение".into())),
        }
    }

    fn on_event(&mut self, d: &Value) {
        if d["eventType"].as_str() != Some("InputVolumeMeters") {
            return;
        }
        let Some(list) = d["eventData"]["inputs"].as_array() else { return };
        self.inputs = list
            .iter()
            .filter_map(|i| {
                let name = i["inputName"].as_str()?.to_string();
                // channels of [magnitude, peak, input peak], multiplier 0..1: the loudest peak
                let level = i["inputLevelsMul"]
                    .as_array()
                    .map(|chs| chs.iter().filter_map(|c| c.get(1).and_then(Value::as_f64)).fold(0.0f64, f64::max))
                    .unwrap_or(0.0);
                Some((name, level.clamp(0.0, 1.0)))
            })
            .take(32)
            .collect();
    }

    async fn request(&mut self, ty: &str, data: Value) -> Result<Value, Fail> {
        self.next += 1;
        let id = self.next.to_string();
        self.send(json!({"op": 6, "d": {"requestType": ty, "requestId": id, "requestData": data}})).await?;
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let m = tokio::time::timeout_at(deadline, self.ws.next()).await.map_err(|_| Fail::Lost("OBS не отвечает".into()))?;
            if let Some(Ok(Message::Text(t))) = &m {
                let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                if v["op"].as_u64() == Some(7) && v["d"]["requestId"].as_str() == Some(id.as_str()) {
                    let st = &v["d"]["requestStatus"];
                    if st["result"].as_bool() == Some(true) {
                        return Ok(v["d"]["responseData"].clone());
                    }
                    let why = st["comment"].as_str().map(String::from).unwrap_or_else(|| format!("код {}", st["code"]));
                    return Err(Fail::Refused(format!("OBS: {why}")));
                }
            }
            self.on_message(m)?;
        }
    }

    /// A request whose «no» is just missing data.
    async fn ask(&mut self, ty: &str, data: Value) -> Result<Value, String> {
        match self.request(ty, data).await {
            Ok(v) => Ok(v),
            Err(Fail::Refused(_)) => Ok(Value::Null),
            Err(Fail::Lost(e)) => Err(e),
        }
    }

    /// The `obs` value. `bytes` keeps the previous stream byte counter for kbit/s.
    async fn poll(&mut self, bytes: &mut Option<(Instant, u64)>) -> Result<Value, String> {
        let s = self.ask("GetStreamStatus", json!({})).await?;
        let r = self.ask("GetRecordStatus", json!({})).await?;
        let sc = self.ask("GetSceneList", json!({})).await?;
        let mut inputs = Vec::new();
        for (name, level) in self.inputs.clone() {
            let m = self.ask("GetInputMute", json!({"inputName": name})).await?;
            inputs.push(json!({"name": name, "muted": m["inputMuted"].as_bool().unwrap_or(false), "level": (level * 100.0).round() / 100.0}));
        }
        let streaming = s["outputActive"].as_bool().unwrap_or(false);
        let now = Instant::now();
        let total = s["outputBytes"].as_u64().unwrap_or(0);
        let kbps = match *bytes {
            Some((t, b)) if streaming && total >= b => ((total - b) as f64 * 8.0 / 1000.0 / now.duration_since(t).as_secs_f64().max(0.1)).round(),
            _ => 0.0,
        };
        *bytes = streaming.then_some((now, total));
        // GetSceneList lists the bottom scene first; the panel shows them as OBS does, top first
        let scenes: Vec<Value> = sc["scenes"].as_array().map(|l| l.iter().rev().filter_map(|x| x["sceneName"].as_str()).map(|n| json!(n)).collect()).unwrap_or_default();
        Ok(json!({
            "connected": true,
            "streaming": streaming,
            "recording": r["outputActive"].as_bool().unwrap_or(false),
            "streamTime": s["outputDuration"].as_u64().unwrap_or(0) / 1000,
            "recordTime": r["outputDuration"].as_u64().unwrap_or(0) / 1000,
            "droppedFrames": s["outputSkippedFrames"].as_u64().unwrap_or(0),
            "totalFrames": s["outputTotalFrames"].as_u64().unwrap_or(0),
            "kbps": kbps,
            "scenes": scenes,
            "scene": sc["currentProgramSceneName"],
            "inputs": inputs,
        }))
    }
}

fn disconnected(e: &str) -> Value {
    json!({"connected": false, "error": e})
}

/// Some active panel shows the `obs` block.
fn wanted(st: &AppState) -> bool {
    !st.paused() && st.demand.active().1.contains("obs")
}

fn current_cfg(st: &AppState) -> ObsCfg {
    st.cfg.read().unwrap().obs.clone().unwrap_or_default()
}

/// Polls OBS once a second and runs commands until no panel needs it (Ok) or the connection breaks (Err).
async fn session(st: &AppState, c: &mut Conn, cfg: &ObsCfg, rx: &mut mpsc::Receiver<CmdMsg>) -> Result<(), String> {
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut bytes = None;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if !wanted(st) || current_cfg(st) != *cfg {
                    let _ = c.ws.close(None).await;
                    return Ok(());
                }
                let v = c.poll(&mut bytes).await?;
                st.set_block("obs", v, None);
            }
            m = rx.recv() => {
                let Some((cmd, reply)) = m else { return Ok(()) };
                let (ty, data) = cmd.request();
                match c.request(ty, data).await {
                    Ok(_) => {
                        let _ = reply.send(Ok(()));
                    }
                    Err(Fail::Refused(e)) => {
                        let _ = reply.send(Err(e));
                    }
                    Err(Fail::Lost(e)) => {
                        let _ = reply.send(Err(e.clone()));
                        return Err(e);
                    }
                }
                let v = c.poll(&mut bytes).await?;
                st.set_block("obs", v, None);
            }
            m = c.ws.next() => {
                if let Err(Fail::Lost(e) | Fail::Refused(e)) = c.on_message(m) {
                    return Err(e);
                }
            }
        }
    }
}

/// The OBS task (spawned once on the runtime).
pub async fn run(st: Arc<AppState>) {
    let Some(mut rx) = st.obs_rx.lock().unwrap().take() else { return };
    let mut next_try = Instant::now();
    loop {
        tokio::select! {
            m = rx.recv() => {
                match m {
                    Some((_, reply)) => {
                        let _ = reply.send(Err("OBS не подключён".into()));
                    }
                    None => return,
                }
                continue;
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
        if !wanted(&st) || Instant::now() < next_try {
            continue;
        }
        let cfg = current_cfg(&st);
        match Conn::open(&cfg, EVENTS_METERS).await {
            Err(e) => {
                st.set_block("obs", disconnected(&e), None);
                next_try = Instant::now() + RETRY;
            }
            Ok((mut c, version)) => {
                tracing::info!("obs-websocket {version} connected at {}:{}", cfg.host, cfg.port);
                match session(&st, &mut c, &cfg, &mut rx).await {
                    Ok(()) => next_try = Instant::now(),
                    Err(e) => {
                        tracing::info!("obs: {e}");
                        st.set_block("obs", disconnected(&e), None);
                        next_try = Instant::now() + RETRY;
                    }
                }
            }
        }
    }
}

/// POST /admin/obs/test: `{"ok": true, "version": "<OBS version>"}` or `{"ok": false, "error": "…"}`.
pub async fn test(cfg: &ObsCfg) -> Value {
    match Conn::open(cfg, 0).await {
        Err(e) => json!({"ok": false, "error": e}),
        Ok((mut c, ws_version)) => {
            let v = c.request("GetVersion", json!({})).await.ok();
            let _ = c.ws.close(None).await;
            let version = v.and_then(|v| v["obsVersion"].as_str().map(String::from)).unwrap_or(ws_version);
            json!({"ok": true, "version": version})
        }
    }
}
