//! /api/v1/* for the phone (token required).

use super::{err, ok_json, parse_body, phone_from, St};
use crate::actions::{run_action, AudioOp, PowerAction, SHUTDOWN_DELAY};
use crate::ai::unix_now;
use crate::layout::Layout;
use crate::poll::{state_key, DATA_BLOCKS};
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// URL of an app/action icon: the configured file, else `icons/<id>.png` when it exists.
fn icon_url(st: &AppState, id: &str, icon: &Option<String>) -> Option<String> {
    match icon {
        Some(i) if !i.is_empty() => Some(format!("/{}", i.trim_start_matches('/').replace('\\', "/"))),
        _ => {
            let f = st.paths.icons.join(format!("{id}.png"));
            f.exists().then(|| format!("/icons/{id}.png"))
        }
    }
}

/// GET /api/v1/layout — the phone's layout + phone-level settings + theme, weather, apps, actions,
/// wake-on-LAN target and notification settings for the panel.
pub async fn layout(st: St, headers: HeaderMap) -> Response {
    let phone_id = match phone_from(&st, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let st2 = st.0.clone();
    let wol = tokio::task::spawn_blocking(move || st2.wol_target()).await.ok().flatten();
    let l = st.layout_for(&phone_id);
    let cfg = st.cfg.read().unwrap();
    let phone = cfg.phones.iter().find(|p| p.id == phone_id);
    let mut v = serde_json::to_value(&l).unwrap_or(Value::Null);
    let o = v.as_object_mut().unwrap();
    o.insert("night".into(), phone.and_then(|p| p.night.clone()).map(|n| serde_json::to_value(n).unwrap()).unwrap_or(Value::Null));
    o.insert("dimWhenPcOffline".into(), json!(phone.map(|p| p.dim_when_pc_offline).unwrap_or(true)));
    o.insert("burnInShift".into(), json!(phone.map(|p| p.burn_in_shift).unwrap_or(true)));
    o.insert("theme".into(), json!(cfg.theme));
    o.insert("weather".into(), cfg.weather.as_ref().map(|w| serde_json::to_value(w).unwrap()).unwrap_or(Value::Null));
    let apps: Vec<Value> = cfg.apps.iter().map(|a| json!({"id": a.id, "title": a.title, "icon": icon_url(&st, &a.id, &a.icon)})).collect();
    o.insert("apps".into(), Value::Array(apps));
    // the panel needs only what to draw; keys, text and steps stay on the PC
    let actions: Vec<Value> =
        cfg.actions.iter().map(|a| json!({"id": a.id, "title": a.title, "icon": icon_url(&st, &a.id, &a.icon), "kind": a.kind})).collect();
    o.insert("actions".into(), Value::Array(actions));
    o.insert("wol".into(), wol.map(|(mac, broadcast)| json!({"mac": mac, "broadcast": broadcast})).unwrap_or(Value::Null));
    o.insert("notify".into(), json!({"nightFlash": cfg.notify_cfg().night_flash}));
    ok_json(v)
}

/// Data keys a phone may get: sound only with `mixer`/`keys`, OBS and notifications only with their block,
/// `running` only with program screens.
fn key_allowed(l: &Layout, key: &str) -> bool {
    match key {
        "audio" => l.has_block("mixer") || l.has_block("keys"),
        "obs" => l.has_block("obs"),
        "notify" => l.has_block("notify"),
        "running" => !l.app_screens().is_empty(),
        _ => true,
    }
}

/// GET /api/v1/state?blocks=cpu,gpu,... — only the requested keys; missing data -> null + errors.<key>.
pub async fn state(st: St, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    let phone_id = match phone_from(&st, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let l = st.layout_for(&phone_id);
    let mut blocks: Vec<&'static str> = match q.get("blocks") {
        Some(b) => {
            let mut v: Vec<&'static str> = b.split(',').filter_map(|x| DATA_BLOCKS.iter().find(|d| **d == x.trim()).copied()).collect();
            v.dedup();
            v
        }
        None => DATA_BLOCKS.to_vec(),
    };
    blocks.retain(|b| state_key(b).is_some_and(|k| key_allowed(&l, k)));
    // program screens need `running` whatever the panel asked for
    if key_allowed(&l, "running") && !blocks.contains(&"running") {
        blocks.push("running");
    }
    st.demand.touch(&phone_id, &blocks);
    st.phones.lock().unwrap().entry(phone_id.clone()).or_default().last_state = Some(std::time::Instant::now());

    let mut out = Map::new();
    out.insert("time".into(), json!(unix_now()));
    out.insert("host".into(), json!(crate::util::host_name()));
    out.insert("uptime".into(), json!(sysinfo::System::uptime().unwrap_or(0)));
    out.insert("layoutVersion".into(), json!(l.version));
    let mut errors = Map::new();
    let paused = st.paused();
    let data = st.data.lock().unwrap().clone();
    let mut done: Vec<&str> = Vec::new();
    for b in blocks {
        let key = state_key(b).unwrap();
        if done.contains(&key) {
            continue;
        }
        done.push(key);
        if paused {
            out.insert(key.into(), Value::Null);
            errors.insert(key.into(), json!("агент на паузе"));
            continue;
        }
        if key == "servers" {
            let cfg_has = !st.cfg.read().unwrap().servers.is_empty();
            let snap = st.watch.snapshot();
            if snap.is_empty() && cfg_has {
                out.insert(key.into(), Value::Null);
                errors.insert(key.into(), json!("серверы подключаются…"));
            } else {
                out.insert(key.into(), Value::Array(snap));
            }
            continue;
        }
        if key == "running" {
            // only names from this phone's program screens, spelled as in its layout
            let on: Vec<String> = data.get(key).and_then(|d| d.value.as_array().cloned()).unwrap_or_default().iter().filter_map(|x| x.as_str().map(String::from)).collect();
            let list: Vec<Value> = l.app_screens().into_iter().filter(|n| on.iter().any(|r| r.eq_ignore_ascii_case(n))).map(Value::from).collect();
            out.insert(key.into(), Value::Array(list));
            continue;
        }
        match data.get(key) {
            Some(d) => {
                out.insert(key.into(), d.value.clone());
                if let Some(e) = &d.err {
                    errors.insert(key.into(), json!(e));
                }
            }
            None => {
                out.insert(key.into(), Value::Null);
                errors.insert(key.into(), json!("данные собираются"));
            }
        }
    }
    out.insert("errors".into(), Value::Object(errors));
    ok_json(Value::Object(out))
}

#[derive(Deserialize)]
struct LaunchReq {
    app: String,
}

/// POST /api/v1/launch {"app":"id"} — only apps from the config list.
pub async fn launch(st: St, headers: HeaderMap, body: Bytes) -> Response {
    let phone_id = match phone_from(&st, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let req: LaunchReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let app = st.cfg.read().unwrap().apps.iter().find(|a| a.id == req.app).cloned();
    let Some(app) = app else { return err(StatusCode::NOT_FOUND, "unknown app") };
    let st2 = st.0.clone();
    let res = tokio::task::spawn_blocking(move || st2.actions.launch(&app)).await.unwrap_or_else(|e| Err(e.to_string()));
    match res {
        Ok(()) => {
            tracing::info!("launch {} (phone {phone_id})", req.app);
            ok_json(json!({"ok": true}))
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

#[derive(Deserialize)]
struct MediaReq {
    action: String,
}

/// POST /api/v1/media {"action": playpause|next|prev|stop|volup|voldown|mute}
pub async fn media(st: St, headers: HeaderMap, body: Bytes) -> Response {
    if let Err(r) = phone_from(&st, &headers) {
        return r;
    }
    let req: MediaReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    if crate::metrics::media::media_vk(&req.action).is_none() {
        return err(StatusCode::BAD_REQUEST, "unknown action");
    }
    let st2 = st.0.clone();
    let action = req.action.clone();
    let res = tokio::task::spawn_blocking(move || st2.actions.media(&action)).await.unwrap_or_else(|e| Err(e.to_string()));
    match res {
        Ok(crate::metrics::media::MediaOutcome::Done) => ok_json(json!({"ok": true})),
        Ok(crate::metrics::media::MediaOutcome::Unsupported) => ok_json(json!({"ok": false, "reason": "unsupported"})),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

/// POST /api/v1/phone {battery, temp, charging, webview, android, model, launcherVersion}
pub async fn phone(st: St, headers: HeaderMap, body: Bytes) -> Response {
    let phone_id = match phone_from(&st, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let v: Value = match parse_body(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(obj) = v.as_object() else { return err(StatusCode::BAD_REQUEST, "badJson") };
    const KEYS: &[&str] = &["battery", "temp", "charging", "webview", "android", "model", "launcherVersion"];
    let info: Map<String, Value> = obj.iter().filter(|(k, _)| KEYS.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
    let overheat = st.cfg.read().unwrap().overheat_temp;
    if let Some(t) = info.get("temp").and_then(Value::as_f64) {
        if t >= overheat {
            tracing::warn!("phone {phone_id} overheated: {t} °C");
        }
    }
    let mut phones = st.phones.lock().unwrap();
    let e = phones.entry(phone_id).or_default();
    e.info.extend(info);
    e.last_seen = Some(unix_now());
    ok_json(json!({"ok": true}))
}

/// Phone id and its layout, or the 401 response.
fn phone_layout(st: &AppState, headers: &HeaderMap) -> Result<(String, Layout), Response> {
    let id = phone_from(st, headers)?;
    let l = st.layout_for(&id);
    Ok((id, l))
}

/// Runs a blocking side effect off the runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.unwrap_or_else(|e| Err(e.to_string()))
}

#[derive(Deserialize)]
struct PowerReq {
    action: String,
}

/// POST /api/v1/power {"action": lock|sleep|restart|shutdown|cancel} — only buttons of the phone's `power` block.
pub async fn power(st: St, headers: HeaderMap, body: Bytes) -> Response {
    let (phone_id, l) = match phone_layout(&st, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let req: PowerReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let Some(action) = PowerAction::parse(&req.action) else { return err(StatusCode::BAD_REQUEST, "unknown action") };
    if !l.power_allowed(action.as_str()) {
        return err(StatusCode::FORBIDDEN, "not in layout");
    }
    tracing::info!("power {} (phone {phone_id})", action.as_str());
    let st2 = st.0.clone();
    match blocking(move || st2.actions.power(action)).await {
        Ok(()) if matches!(action, PowerAction::Restart | PowerAction::Shutdown) => ok_json(json!({"ok": true, "delay": SHUTDOWN_DELAY})),
        Ok(()) => ok_json(json!({"ok": true})),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

#[derive(Deserialize)]
struct ActionReq {
    id: String,
}

/// POST /api/v1/action {"id"} — an action from `config.actions` listed in the phone's `keys` block.
pub async fn action(st: St, headers: HeaderMap, body: Bytes) -> Response {
    let (phone_id, l) = match phone_layout(&st, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let req: ActionReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let (a, apps) = {
        let cfg = st.cfg.read().unwrap();
        (cfg.actions.iter().find(|a| a.id == req.id).cloned(), cfg.apps.clone())
    };
    let Some(a) = a else { return err(StatusCode::NOT_FOUND, "unknown action") };
    if !l.action_allowed(&a.id) {
        return err(StatusCode::FORBIDDEN, "not in layout");
    }
    let st2 = st.0.clone();
    let kind = a.kind.clone();
    match blocking(move || run_action(st2.actions.as_ref(), &a, &apps)).await {
        Ok(()) => {
            tracing::info!("action {} (phone {phone_id})", req.id);
            if kind == "mic" || kind == "audioOut" {
                st.audio_dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                st.demand.wake();
            }
            ok_json(json!({"ok": true}))
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

#[derive(Deserialize)]
struct AudioReq {
    op: String,
    id: Option<String>,
    pid: Option<u32>,
    value: Option<f64>,
    muted: Option<bool>,
}

/// POST /api/v1/audio {"op": output|volume|appMute|mic, ...} — only with a `mixer` block.
pub async fn audio(st: St, headers: HeaderMap, body: Bytes) -> Response {
    let (_, l) = match phone_layout(&st, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let req: AudioReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    if !l.has_block("mixer") {
        return err(StatusCode::FORBIDDEN, "not in layout");
    }
    let op = match (req.op.as_str(), req.id, req.pid, req.value, req.muted) {
        ("output", Some(id), ..) if !id.is_empty() && id.len() <= 512 => AudioOp::Output(id),
        ("volume", _, Some(pid), Some(v), _) if v.is_finite() => AudioOp::Volume { pid, value: v.clamp(0.0, 1.0) as f32 },
        ("appMute", _, Some(pid), _, Some(m)) => AudioOp::AppMute { pid, muted: m },
        ("mic", _, _, _, Some(m)) => AudioOp::Mic(m),
        ("mic", _, _, _, None) => AudioOp::MicToggle,
        _ => return err(StatusCode::BAD_REQUEST, "bad op"),
    };
    let st2 = st.0.clone();
    match blocking(move || st2.actions.audio(&op)).await {
        Ok(()) => {
            st.audio_dirty.store(true, std::sync::atomic::Ordering::Relaxed);
            st.demand.wake();
            ok_json(json!({"ok": true}))
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

#[derive(Deserialize)]
struct ObsReq {
    op: String,
    name: Option<String>,
}

/// POST /api/v1/obs {"op": stream|record|scene|mute, "name"?} — only with an `obs` block.
pub async fn obs(st: St, headers: HeaderMap, body: Bytes) -> Response {
    let (_, l) = match phone_layout(&st, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let req: ObsReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    if !l.has_block("obs") {
        return err(StatusCode::FORBIDDEN, "not in layout");
    }
    let Some(cmd) = crate::obs::Cmd::parse(&req.op, req.name.as_deref()) else { return err(StatusCode::BAD_REQUEST, "bad op") };
    let (tx, rx) = tokio::sync::oneshot::channel();
    if st.obs_tx.try_send((cmd, tx)).is_err() {
        return err(StatusCode::SERVICE_UNAVAILABLE, "OBS занят, повторите");
    }
    match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(()))) => ok_json(json!({"ok": true})),
        Ok(Ok(Err(e))) => err(StatusCode::CONFLICT, &e),
        _ => err(StatusCode::GATEWAY_TIMEOUT, "OBS не ответил"),
    }
}

#[derive(Deserialize)]
struct NotifyReq {
    op: String,
    id: Value,
}

/// POST /api/v1/notify {"op": open|dismiss, "id"} — only items on the panel, only with a `notify` block.
pub async fn notify(st: St, headers: HeaderMap, body: Bytes) -> Response {
    let (_, l) = match phone_layout(&st, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let req: NotifyReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    if !l.has_block("notify") {
        return err(StatusCode::FORBIDDEN, "not in layout");
    }
    let id = match &req.id {
        Value::String(s) => s.parse::<u32>().ok(),
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    };
    let Some(id) = id else { return err(StatusCode::BAD_REQUEST, "bad id") };
    match req.op.as_str() {
        "dismiss" => {
            if crate::notify::dismiss(&st, id) {
                ok_json(json!({"ok": true}))
            } else {
                err(StatusCode::NOT_FOUND, "unknown notification")
            }
        }
        "open" => {
            let Some(aumid) = crate::notify::app_of(&st, id) else { return err(StatusCode::NOT_FOUND, "unknown notification") };
            let st2 = st.0.clone();
            match blocking(move || st2.actions.notify_open(&aumid)).await {
                Ok(()) => ok_json(json!({"ok": true})),
                Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
            }
        }
        _ => err(StatusCode::BAD_REQUEST, "bad op"),
    }
}
