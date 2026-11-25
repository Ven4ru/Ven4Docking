//! HTTP contract: auth (401/403), layout validation payloads, pairing, state shape, media outcome.

use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceExt;
use ven4docking_agent::config::{new_token, Config, Paths};
use ven4docking_agent::metrics::media::{MediaInfo, RecordActions};
use ven4docking_agent::state::AppState;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Env {
    st: Arc<AppState>,
    token: String,
    dir: std::path::PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn env(name: &str) -> Env {
    let dir = std::env::temp_dir().join(format!("v4d-http-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let paths = Paths::from_home(dir.clone());
    let (token, hash) = new_token();
    let cfg: Config = serde_json::from_value(json!({
        "version": 1, "port": 18765,
        "weather": {"name": "Москва", "lat": 55.75, "lon": 37.62},
        "apps": [{"id": "notepad", "title": "Блокнот", "kind": "exe", "target": "notepad.exe"}],
        "phones": [{"id": "p1", "name": "Test", "tokenHash": hash, "night": {"from": "23:00", "to": "07:00", "brightness": 0.02}}]
    }))
    .unwrap();
    let actions = RecordActions { unsupported: vec!["next".into()], ..Default::default() };
    let st = Arc::new(AppState::new(paths, cfg, KEY.into(), Box::new(actions)));
    Env { st, token, dir }
}

async fn call(st: &Arc<AppState>, from: &str, method: &str, uri: &str, headers: &[(&str, &str)], body: Option<Value>) -> (StatusCode, Value) {
    let addr: SocketAddr = format!("{from}:50000").parse().unwrap();
    let app = ven4docking_agent::http::router(st.clone()).layer(MockConnectInfo(addr));
    let mut req = Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let req = req.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v)
}

fn bearer(t: &str) -> String {
    format!("Bearer {t}")
}

#[tokio::test]
async fn public_address_gets_403_everywhere() {
    let e = env("403");
    for (m, u) in [("GET", "/api/v1/state"), ("GET", "/"), ("POST", "/pair/start"), ("GET", "/admin/status")] {
        let (s, v) = call(&e.st, "8.8.8.8", m, u, &[("authorization", &bearer(&e.token)), ("x-admin-key", KEY)], None).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{m} {u}");
        assert_eq!(v["error"], "lan only");
    }
    // private ranges pass the network check
    for ip in ["10.1.2.3", "172.20.0.5", "192.168.1.50", "100.100.1.1", "127.0.0.1"] {
        let (s, _) = call(&e.st, ip, "GET", "/api/v1/layout", &[("authorization", &bearer(&e.token))], None).await;
        assert_eq!(s, StatusCode::OK, "{ip}");
    }
    for ip in ["172.32.0.1", "192.169.0.1", "100.128.0.1", "169.254.1.1"] {
        let (s, _) = call(&e.st, ip, "GET", "/api/v1/layout", &[("authorization", &bearer(&e.token))], None).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{ip}");
    }
}

#[tokio::test]
async fn panel_api_needs_token() {
    let e = env("401");
    for u in ["/api/v1/layout", "/api/v1/state"] {
        let (s, _) = call(&e.st, "192.168.1.50", "GET", u, &[], None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{u} without token");
        let (s, _) = call(&e.st, "192.168.1.50", "GET", u, &[("authorization", "Bearer nope")], None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{u} wrong token");
    }
    let (s, _) = call(&e.st, "192.168.1.50", "POST", "/api/v1/launch", &[], Some(json!({"app": "notepad"}))).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_is_localhost_and_key_only() {
    let e = env("admin");
    let (s, _) = call(&e.st, "192.168.1.50", "GET", "/admin/status", &[("x-admin-key", KEY)], None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "LAN cannot reach admin even with the key");
    let (s, _) = call(&e.st, "127.0.0.1", "GET", "/admin/status", &[], None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(&e.st, "127.0.0.1", "GET", "/admin/status", &[("x-admin-key", "wrong")], None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, v) = call(&e.st, "127.0.0.1", "GET", "/admin/status", &[("x-admin-key", KEY)], None).await;
    assert_eq!(s, StatusCode::OK);
    for k in ["version", "port", "cpuPct", "ramBytes", "activePanels", "blockErrors", "firewall"] {
        assert!(v.get(k).is_some(), "status.{k}");
    }
    // the test exe has no rules of its own; the COM check works without elevation
    assert!(matches!(v["firewall"].as_str(), Some("missing" | "blocked")), "{}", v["firewall"]);
}

#[tokio::test]
async fn layout_put_validates_and_bumps_version() {
    let e = env("layout");
    let h = [("x-admin-key", KEY)];
    let bad = json!({"version": 1, "grid": {"cols": 8, "rows": 4}, "screens": [{"id": "s1", "name": "a", "blocks": [
        {"id": "b1", "type": "cpu", "x": 0, "y": 0, "w": 3, "h": 2, "opts": {}},
        {"id": "b2", "type": "ram", "x": 2, "y": 1, "w": 2, "h": 2, "opts": {}}]}]});
    let (s, v) = call(&e.st, "127.0.0.1", "PUT", "/admin/layouts/p1", &h, Some(bad)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v, json!({"error": "overlap", "blocks": ["b1", "b2"]}));

    let oob = json!({"version": 1, "grid": {"cols": 8, "rows": 4}, "screens": [{"id": "s1", "name": "a", "blocks": [
        {"id": "b9", "type": "cpu", "x": 7, "y": 0, "w": 2, "h": 1}]}]});
    let (s, v) = call(&e.st, "127.0.0.1", "PUT", "/admin/layouts/p1", &h, Some(oob)).await;
    assert_eq!((s, v), (StatusCode::BAD_REQUEST, json!({"error": "bounds", "blocks": ["b9"]})));

    let good = json!({"version": 1, "grid": {"cols": 8, "rows": 4}, "screens": [{"id": "s1", "name": "Главный", "blocks": [
        {"id": "b1", "type": "cpu", "x": 0, "y": 0, "w": 2, "h": 2, "opts": {}}]}]});
    let (s, v) = call(&e.st, "127.0.0.1", "PUT", "/admin/layouts/p1", &h, Some(good.clone())).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["version"], 2);
    let (_, v) = call(&e.st, "127.0.0.1", "PUT", "/admin/layouts/p1", &h, Some(good)).await;
    assert_eq!(v["version"], 3, "each save increases version");
    assert!(e.st.paths.layout_file("p1").exists());

    // the phone sees it with its settings
    let (s, v) = call(&e.st, "192.168.1.50", "GET", "/api/v1/layout", &[("authorization", &bearer(&e.token))], None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["version"], 3);
    assert_eq!(v["screens"][0]["blocks"][0]["type"], "cpu");
    assert_eq!(v["night"]["brightness"], 0.02);
    assert_eq!(v["dimWhenPcOffline"], true);
    assert_eq!(v["theme"], "teal");
    assert_eq!(v["weather"]["name"], "Москва");
    assert_eq!(v["apps"][0], json!({"id": "notepad", "title": "Блокнот", "icon": null}));
}

#[tokio::test]
async fn state_returns_only_requested_keys_with_errors() {
    let e = env("state");
    let media = MediaInfo { title: "T".into(), artist: "A".into(), app: "chrome".into(), playing: true, can_next: false, can_prev: false, can_play_pause: true };
    e.st.set_block("media", serde_json::to_value(&media).unwrap(), None);
    e.st.set_block("cpu", json!({"load": 6.1, "cores": [5, 12], "freqMhz": 3500, "temp": null, "boardTemp": 27.9}), None);
    let (s, v) = call(&e.st, "192.168.1.50", "GET", "/api/v1/state?blocks=cpu,gpu,media", &[("authorization", &bearer(&e.token))], None).await;
    assert_eq!(s, StatusCode::OK);
    let o = v.as_object().unwrap();
    assert_eq!(v["layoutVersion"], 1, "classic template until the editor saves one");
    assert!(v["errors"].get("gpu").is_none(), "errors use the data key (gpus)");
    for k in ["time", "host", "uptime", "layoutVersion", "cpu", "gpus", "media", "errors"] {
        assert!(o.contains_key(k), "missing {k}: {v}");
    }
    assert!(!o.contains_key("ram") && !o.contains_key("servers"), "only requested blocks");
    assert!(v["time"].is_f64());
    assert_eq!(v["cpu"]["load"], 6.1);
    assert_eq!(v["gpus"], Value::Null);
    assert!(v["errors"]["gpus"].is_string(), "no data -> null + errors.<key>");
    // media carries the session's supported controls
    assert_eq!(
        v["media"],
        json!({"title": "T", "artist": "A", "app": "chrome", "playing": true, "canNext": false, "canPrev": false, "canPlayPause": true})
    );
    assert_eq!(e.st.demand.active().0, 1, "phone is now an active panel");
}

#[tokio::test]
async fn launch_and_media() {
    let e = env("launch");
    let auth = bearer(&e.token);
    let h = [("authorization", auth.as_str())];
    let (s, v) = call(&e.st, "192.168.1.50", "POST", "/api/v1/launch", &h, Some(json!({"app": "nope"}))).await;
    assert_eq!((s, v["error"].as_str()), (StatusCode::NOT_FOUND, Some("unknown app")));
    let (s, v) = call(&e.st, "192.168.1.50", "POST", "/api/v1/launch", &h, Some(json!({"app": "notepad"}))).await;
    assert_eq!((s, v), (StatusCode::OK, json!({"ok": true})));
    let (s, _) = call(&e.st, "192.168.1.50", "POST", "/api/v1/media", &h, Some(json!({"action": "explode"}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, v) = call(&e.st, "192.168.1.50", "POST", "/api/v1/media", &h, Some(json!({"action": "playpause"}))).await;
    assert_eq!((s, v), (StatusCode::OK, json!({"ok": true})));
    let (s, v) = call(&e.st, "192.168.1.50", "POST", "/api/v1/media", &h, Some(json!({"action": "next"}))).await;
    assert_eq!((s, v), (StatusCode::OK, json!({"ok": false, "reason": "unsupported"})));
    let (s, _) = call(&e.st, "192.168.1.50", "POST", "/api/v1/phone", &h, Some(json!({"battery": 82, "temp": 31.0, "charging": true, "model": "RMX2185"}))).await;
    assert_eq!(s, StatusCode::OK);
    let (_, v) = call(&e.st, "127.0.0.1", "GET", "/admin/phones", &[("x-admin-key", KEY)], None).await;
    assert_eq!(v[0]["battery"], 82);
    assert_eq!(v[0]["online"], true);
}

#[tokio::test]
async fn pairing_flow_token_once() {
    let e = env("pair");
    let (s, v) = call(&e.st, "192.168.1.77", "POST", "/pair/start", &[], Some(json!({"model": "RMX2185", "android": "10", "deviceId": "dev-1"}))).await;
    assert_eq!(s, StatusCode::OK);
    let pair_id = v["pairId"].as_str().unwrap().to_string();
    let code = v["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 4);
    assert!(code.chars().all(|c| c.is_ascii_digit()));

    let (_, v) = call(&e.st, "192.168.1.77", "GET", &format!("/pair/{pair_id}"), &[], None).await;
    assert_eq!(v, json!({"status": "wait"}));
    let (_, v) = call(&e.st, "127.0.0.1", "GET", "/admin/pairs", &[("x-admin-key", KEY)], None).await;
    assert_eq!(v[0]["code"], code.as_str());
    assert_eq!(v[0]["ip"], "192.168.1.77");

    let (s, v) = call(&e.st, "127.0.0.1", "POST", &format!("/admin/pairs/{pair_id}/accept"), &[("x-admin-key", KEY)], None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (_, v) = call(&e.st, "192.168.1.77", "GET", &format!("/pair/{pair_id}"), &[], None).await;
    assert_eq!(v["status"], "ok");
    let token = v["token"].as_str().unwrap().to_string();
    let (_, v) = call(&e.st, "192.168.1.77", "GET", &format!("/pair/{pair_id}"), &[], None).await;
    assert_eq!(v, json!({"status": "ok"}), "token handed out only once");

    let (s, _) = call(&e.st, "192.168.1.77", "GET", "/api/v1/state?blocks=cpu", &[("authorization", &bearer(&token))], None).await;
    assert_eq!(s, StatusCode::OK);
    let saved = std::fs::read_to_string(&e.st.paths.config).unwrap();
    assert!(!saved.contains(&token), "only the hash is stored");

    // deny + rate limit (5 per minute per address)
    let (_, v) = call(&e.st, "192.168.1.88", "POST", "/pair/start", &[], Some(json!({"model": "X"}))).await;
    let id2 = v["pairId"].as_str().unwrap().to_string();
    call(&e.st, "127.0.0.1", "POST", &format!("/admin/pairs/{id2}/deny"), &[("x-admin-key", KEY)], None).await;
    let (_, v) = call(&e.st, "192.168.1.88", "GET", &format!("/pair/{id2}"), &[], None).await;
    assert_eq!(v, json!({"status": "denied"}));
    for _ in 0..4 {
        call(&e.st, "192.168.1.88", "POST", "/pair/start", &[], Some(json!({}))).await;
    }
    let (s, _) = call(&e.st, "192.168.1.88", "POST", "/pair/start", &[], Some(json!({}))).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn admin_token_revoke_and_config() {
    let e = env("revoke");
    let h = [("x-admin-key", KEY)];
    let (s, v) = call(&e.st, "127.0.0.1", "POST", "/admin/phones/token", &h, Some(json!({"name": "Кухня", "model": "SM-A105"}))).await;
    assert_eq!(s, StatusCode::OK);
    let (id, tok) = (v["id"].as_str().unwrap().to_string(), v["token"].as_str().unwrap().to_string());
    let (s, _) = call(&e.st, "10.0.0.5", "GET", "/api/v1/layout", &[("authorization", &bearer(&tok))], None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&e.st, "127.0.0.1", "POST", &format!("/admin/phones/{id}/revoke"), &h, None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&e.st, "10.0.0.5", "GET", "/api/v1/layout", &[("authorization", &bearer(&tok))], None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "revoked token");
    // the next phone (by adb token or by pairing code) never gets the revoked id back
    let (s, v) = call(&e.st, "127.0.0.1", "POST", "/admin/phones/token", &h, Some(json!({"name": "Спальня", "model": "SM-A105"}))).await;
    assert_eq!(s, StatusCode::OK);
    let id2 = v["id"].as_str().unwrap().to_string();
    assert_ne!(id2, id, "revoked phone id reused");
    let (s, v) = call(&e.st, "192.168.1.60", "POST", "/pair/start", &[], Some(json!({"model": "RMX2185", "android": "10", "deviceId": "dev-x"}))).await;
    assert_eq!(s, StatusCode::OK);
    let (s, a) = call(&e.st, "127.0.0.1", "POST", &format!("/admin/pairs/{}/accept", v["pairId"].as_str().unwrap()), &h, None).await;
    assert_eq!(s, StatusCode::OK);
    let id3 = a["phoneId"].as_str().unwrap().to_string();
    assert!(id3 != id && id3 != id2, "{id} {id2} {id3}");

    let (_, mut cfg) = call(&e.st, "127.0.0.1", "GET", "/admin/config", &h, None).await;
    cfg["theme"] = json!("green");
    cfg["phones"][0].as_object_mut().unwrap().remove("tokenHash");
    let (s, _) = call(&e.st, "127.0.0.1", "PUT", "/admin/config", &h, Some(cfg)).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&e.st, "10.0.0.5", "GET", "/api/v1/layout", &[("authorization", &bearer(&e.token))], None).await;
    assert_eq!(s, StatusCode::OK, "editor PUT without tokenHash keeps the phone paired");
    let (s, v) = call(&e.st, "127.0.0.1", "PUT", "/admin/config", &h, Some(json!({"version": 0}))).await;
    assert_eq!((s, v["error"].as_str()), (StatusCode::BAD_REQUEST, Some("badConfig")));
}

#[tokio::test]
async fn static_files_and_discovery() {
    let e = env("static");
    let (s, _) = call(&e.st, "192.168.1.50", "GET", "/../config.json", &[], None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(&e.st, "192.168.1.50", "GET", "/%2e%2e/config.json", &[], None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(&e.st, "192.168.1.50", "GET", "/api/v1/nope", &[("authorization", &bearer(&e.token))], None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    use ven4docking_agent::discovery::reply;
    let lan: SocketAddr = "192.168.1.9:4000".parse().unwrap();
    let wan: SocketAddr = "8.8.8.8:4000".parse().unwrap();
    let r = reply(b"V4D?1", lan, "PC", 18765).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&r).unwrap(), json!({"name": "PC", "port": 18765, "ver": "1.0.0"}));
    assert!(reply(b"V4D?1", wan, "PC", 18765).is_none());
    assert!(reply(b"hello", lan, "PC", 18765).is_none());
}

#[test]
fn media_helpers() {
    use ven4docking_agent::metrics::media::{app_name, is_session_action, media_vk};
    assert_eq!(app_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "ZuneMusic");
    assert_eq!(app_name("chrome.exe"), "chrome");
    assert_eq!(app_name("Spotify.exe"), "Spotify");
    assert_eq!(app_name("308046B0AF4A39CB"), "");
    assert!(is_session_action("next") && is_session_action("playpause") && !is_session_action("volup"));
    assert_eq!(media_vk("mute"), Some(0xAD));
    assert_eq!(media_vk("x"), None);
}
