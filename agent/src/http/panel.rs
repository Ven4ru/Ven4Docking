//! /api/v1/* for the phone (token required).

use super::{err, ok_json, parse_body, phone_from, St};
use crate::ai::unix_now;
use crate::poll::{state_key, DATA_BLOCKS};
use axum::body::Bytes;
use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// GET /api/v1/layout — the phone's layout + phone-level settings + theme, weather, apps.
pub async fn layout(st: St, headers: HeaderMap) -> Response {
    let phone_id = match phone_from(&st, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
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
    let apps: Vec<Value> = cfg
        .apps
        .iter()
        .map(|a| {
            let icon = match &a.icon {
                Some(i) if !i.is_empty() => Some(format!("/{}", i.trim_start_matches('/').replace('\\', "/"))),
                _ => {
                    let f = st.paths.icons.join(format!("{}.png", a.id));
                    f.exists().then(|| format!("/icons/{}.png", a.id))
                }
            };
            json!({"id": a.id, "title": a.title, "icon": icon})
        })
        .collect();
    o.insert("apps".into(), Value::Array(apps));
    ok_json(v)
}

/// GET /api/v1/state?blocks=cpu,gpu,... — only the requested keys; missing data -> null + errors.<key>.
pub async fn state(st: St, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    let phone_id = match phone_from(&st, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let blocks: Vec<&'static str> = match q.get("blocks") {
        Some(b) => {
            let mut v: Vec<&'static str> = b.split(',').filter_map(|x| DATA_BLOCKS.iter().find(|d| **d == x.trim()).copied()).collect();
            v.dedup();
            v
        }
        None => DATA_BLOCKS.to_vec(),
    };
    st.demand.touch(&phone_id, &blocks);
    st.phones.lock().unwrap().entry(phone_id.clone()).or_default().last_state = Some(std::time::Instant::now());

    let mut out = Map::new();
    out.insert("time".into(), json!(unix_now()));
    out.insert("host".into(), json!(crate::util::host_name()));
    out.insert("uptime".into(), json!(sysinfo::System::uptime().unwrap_or(0)));
    out.insert("layoutVersion".into(), json!(st.layout_for(&phone_id).version));
    let mut errors = Map::new();
    let paused = st.paused();
    let data = st.data.lock().unwrap().clone();
    for b in blocks {
        let key = state_key(b).unwrap();
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
