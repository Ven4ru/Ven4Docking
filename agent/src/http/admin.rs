//! /admin/* for the editor (127.0.0.1 + X-Admin-Key; checked by the middleware in http/mod.rs).

use super::{err, ok_json, pair, parse_body, St};
use crate::config::{new_token, valid_id, write_atomic, Config, Server, AGENT_VERSION};
use crate::layout::Layout;
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/config", get(get_config).put(put_config))
        .route("/layouts/{phone}", get(get_layout).put(put_layout))
        .route("/phones", get(phones))
        .route("/phones/token", post(phone_token))
        .route("/phones/{id}/revoke", post(revoke))
        .route("/pairs", get(pairs))
        .route("/pairs/{id}/{action}", post(pair_action))
        .route("/servers/test", post(servers_test))
        .route("/ai/sources", get(ai_sources))
        .route("/status", get(status))
        .route("/reload", post(reload))
        .fallback(|| async { err(StatusCode::NOT_FOUND, "no such admin api") })
}

async fn get_config(st: St) -> Response {
    ok_json(serde_json::to_value(&*st.cfg.read().unwrap()).unwrap_or(Value::Null))
}

async fn put_config(st: St, body: Bytes) -> Response {
    let text = String::from_utf8_lossy(&body);
    let mut cfg = match Config::parse(&text) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "badConfig", "detail": e.to_string()}))).into_response(),
    };
    {
        let old = st.cfg.read().unwrap().clone();
        cfg.merge_secrets_from(&old);
    }
    *st.cfg.write().unwrap() = cfg;
    if let Err(e) = st.save_config() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
    }
    st.load_layouts();
    st.reconcile_servers();
    ok_json(json!({"ok": true}))
}

async fn get_layout(st: St, Path(phone): Path<String>) -> Response {
    if !valid_id(&phone) {
        return err(StatusCode::BAD_REQUEST, "bad phone id");
    }
    let known = st.cfg.read().unwrap().phones.iter().any(|p| p.id == phone);
    let saved = st.layouts.read().unwrap().get(&phone).cloned();
    match saved {
        Some(l) => ok_json(serde_json::to_value(l).unwrap()),
        None if known => ok_json(serde_json::to_value(Layout::classic()).unwrap()),
        None => err(StatusCode::NOT_FOUND, "unknown phone"),
    }
}

/// Validates (overlap/bounds -> 400 with the blocks), bumps `version`, saves layouts/<phone>.json.
async fn put_layout(st: St, Path(phone): Path<String>, body: Bytes) -> Response {
    if !valid_id(&phone) {
        return err(StatusCode::BAD_REQUEST, "bad phone id");
    }
    let mut l: Layout = match parse_body(&body) {
        Ok(l) => l,
        Err(r) => return r,
    };
    if let Err(e) = l.validate() {
        return (StatusCode::BAD_REQUEST, Json(e.payload())).into_response();
    }
    let prev = st.layouts.read().unwrap().get(&phone).map(|o| o.version).unwrap_or(0);
    l.version = prev.max(l.version) + 1;
    let text = serde_json::to_string_pretty(&l).unwrap();
    if let Err(e) = write_atomic(&st.paths.layout_file(&phone), text.as_bytes()) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
    }
    st.layouts.write().unwrap().insert(phone.clone(), l.clone());
    st.reconcile_servers();
    tracing::info!("layout {phone} saved, version {}", l.version);
    ok_json(json!({"ok": true, "version": l.version}))
}

async fn phones(st: St) -> Response {
    let cfg = st.cfg.read().unwrap().clone();
    let rt = st.phones.lock().unwrap().clone();
    let now = crate::ai::unix_now();
    let list: Vec<Value> = cfg
        .phones
        .iter()
        .map(|p| {
            let r = rt.get(&p.id).cloned().unwrap_or_default();
            let g = |k: &str| r.info.get(k).cloned().unwrap_or(Value::Null);
            let model = r.info.get("model").cloned().unwrap_or_else(|| json!(p.model));
            let android = r.info.get("android").cloned().unwrap_or_else(|| json!(p.android));
            let online = st.demand.is_active(&p.id) || r.last_seen.map(|t| now - t < 90.0).unwrap_or(false);
            json!({
                "id": p.id, "name": p.name, "model": model, "android": android, "webview": g("webview"),
                "battery": g("battery"), "temp": g("temp"), "charging": g("charging"),
                "launcherVersion": g("launcherVersion"), "lastSeen": r.last_seen, "online": online,
            })
        })
        .collect();
    ok_json(Value::Array(list))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TokenReq {
    name: String,
    model: String,
}

/// For the adb wizard: create a phone and return its token (once).
async fn phone_token(st: St, body: Bytes) -> Response {
    let req: TokenReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let (token, hash) = new_token();
    let id = {
        let mut cfg = st.cfg.write().unwrap();
        let id = st.paths.alloc_phone_id(&mut cfg);
        let name = if req.name.is_empty() { req.model.clone() } else { req.name.clone() };
        cfg.phones.push(pair::new_phone(&id, &name, Some(&req.model), None, None, hash));
        id
    };
    if let Err(e) = st.save_config() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
    }
    st.reconcile_servers();
    ok_json(json!({"id": id, "token": token}))
}

async fn revoke(st: St, Path(id): Path<String>) -> Response {
    let removed = {
        let mut cfg = st.cfg.write().unwrap();
        let n = cfg.phones.len();
        cfg.phones.retain(|p| p.id != id);
        n != cfg.phones.len()
    };
    if !removed {
        return err(StatusCode::NOT_FOUND, "unknown phone");
    }
    if let Err(e) = st.save_config() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
    }
    st.phones.lock().unwrap().remove(&id);
    st.layouts.write().unwrap().remove(&id);
    st.reconcile_servers();
    tracing::info!("phone {id} revoked");
    ok_json(json!({"ok": true}))
}

async fn pairs(st: St) -> Response {
    let list: Vec<Value> = st
        .pairs
        .lock()
        .unwrap()
        .waiting()
        .into_iter()
        .map(|p| json!({"pairId": p.pair_id, "model": p.model, "android": p.android, "code": p.code, "ip": p.ip.to_string()}))
        .collect();
    ok_json(Value::Array(list))
}

async fn pair_action(st: St, Path((id, action)): Path<(String, String)>) -> Response {
    match action.as_str() {
        "accept" => match pair::accept(&st, &id) {
            Ok(phone) => {
                st.reconcile_servers();
                ok_json(json!({"ok": true, "phoneId": phone}))
            }
            Err(r) => r,
        },
        "deny" => match pair::deny(&st, &id) {
            Ok(()) => ok_json(json!({"ok": true})),
            Err(r) => r,
        },
        _ => err(StatusCode::NOT_FOUND, "no such action"),
    }
}

/// Body: the server itself (or {"server": {...}}) -> {ok, ping, err}.
async fn servers_test(body: Bytes) -> Response {
    let v: Value = match parse_body(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let v = v.get("server").cloned().unwrap_or(v);
    let mut v = v;
    if let Some(o) = v.as_object_mut() {
        o.entry("id").or_insert(json!("test"));
    }
    let s: Server = match serde_json::from_value(v) {
        Ok(s) => s,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "badServer", "detail": e.to_string()}))).into_response(),
    };
    ok_json(crate::servers::test_server(&s).await)
}

async fn ai_sources(st: St) -> Response {
    let mut list = st.ai_sources.lock().unwrap().clone();
    if list.is_empty() {
        // no panel asked yet: detect without reading limits
        let cfg = st.cfg.read().unwrap().ai.clone();
        let r = crate::ai::AiReader::new(crate::ai::AiRoots::from_env(), Vec::new(), cfg.claude_code_file.map(Into::into), cfg.lmstudio_port);
        list = r.sources();
    }
    ok_json(serde_json::to_value(list).unwrap())
}

async fn status(st: St) -> Response {
    let (cpu, ram) = st.self_usage();
    let (active, _) = st.demand.active();
    let mut block_errors = serde_json::Map::new();
    for (k, d) in st.data.lock().unwrap().iter() {
        if let Some(e) = &d.err {
            block_errors.insert(k.to_string(), json!(e));
        }
    }
    for s in st.watch.snapshot() {
        if let (Some(id), Some(e)) = (s["id"].as_str(), s["err"].as_str()) {
            block_errors.insert(format!("servers.{id}"), json!(e));
        }
    }
    if let Some(e) = st.load_error.lock().unwrap().clone() {
        block_errors.insert("config".into(), json!(e));
    }
    let firewall = firewall_state(&st).await;
    ok_json(json!({
        "version": AGENT_VERSION, "port": st.port, "cpuPct": cpu, "ramBytes": ram,
        "activePanels": active, "blockErrors": block_errors, "paused": st.paused(),
        "uptime": st.started.elapsed().as_secs(), "firewall": firewall,
    }))
}

/// How long a firewall check is reused; the check itself runs only when the editor asks for the status.
const FIREWALL_CACHE: std::time::Duration = std::time::Duration::from_secs(30);

/// `ok | missing | blocked | unknown` (see firewall::classify). Only `ok` is cached (30 s): a problem state is
/// re-checked on every request (~10 ms), so the editor sees the result of «Исправить» on its next poll.
/// POST /admin/reload drops the cache.
async fn firewall_state(st: &AppState) -> &'static str {
    if let Some((t, s)) = *st.firewall.lock().unwrap() {
        if t.elapsed() < FIREWALL_CACHE {
            return s;
        }
    }
    let s = tokio::task::spawn_blocking(crate::firewall::check_self).await.unwrap_or("unknown");
    *st.firewall.lock().unwrap() = if s == "ok" { Some((std::time::Instant::now(), s)) } else { None };
    s
}

async fn reload(st: St) -> Response {
    *st.firewall.lock().unwrap() = None;
    match Config::load_or_create(&st.paths.config) {
        Ok(c) => {
            *st.cfg.write().unwrap() = c;
            *st.load_error.lock().unwrap() = None;
            st.load_layouts();
            st.reconcile_servers();
            ok_json(json!({"ok": true}))
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": "badConfig", "detail": e.to_string()}))).into_response(),
    }
}
