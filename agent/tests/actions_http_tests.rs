//! 1.1 panel API: power, actions, sound, OBS, notifications, program screens — allowed only by the phone's
//! layout (403 otherwise). Side effects go to the recording `Actions`, nothing happens on this PC.

use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceExt;
use ven4docking_agent::actions::RecordActions;
use ven4docking_agent::config::{new_token, Config, Paths};
use ven4docking_agent::notify::{Access, NotifyRead, RawNote};
use ven4docking_agent::state::AppState;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const LAN: &str = "192.168.1.50";

struct Env {
    st: Arc<AppState>,
    rec: Arc<RecordActions>,
    /// p1: layout with all 1.1 blocks; p2: the classic template
    t1: String,
    t2: String,
    dir: std::path::PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn env(name: &str) -> Env {
    let dir = std::env::temp_dir().join(format!("v4d-act-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (t1, h1) = new_token();
    let (t2, h2) = new_token();
    let cfg: Config = serde_json::from_value(json!({
        "version": 1, "port": 18765,
        "apps": [{"id": "obs", "title": "OBS", "kind": "exe", "target": "obs64.exe"}],
        "actions": [
            {"id": "mute", "title": "Discord: микрофон", "kind": "keys", "keys": "Ctrl+Shift+M"},
            {"id": "sign", "title": "Подпись", "kind": "text", "text": "С уважением"},
            {"id": "day", "title": "Начать день", "kind": "macro", "steps": [{"launch": "obs"}, {"wait": 3000}, {"keys": "Ctrl+Alt+F9"}]},
            {"id": "mic", "title": "Микрофон", "kind": "mic"},
            {"id": "secret", "title": "Не на этой панели", "kind": "text", "text": "x"}
        ],
        "notify": {"enabled": true, "nightFlash": false, "apps": {"TelegramDesktop": "title", "Discord": "full", "Outlook": "count"}},
        "wol": {"mac": "AA:BB:CC:DD:EE:FF", "broadcast": "192.168.1.255"},
        "phones": [{"id": "p1", "name": "Стена", "tokenHash": h1}, {"id": "p2", "name": "Кухня", "tokenHash": h2}]
    }))
    .unwrap();
    let rec = Arc::new(RecordActions::default());
    let st = Arc::new(AppState::new(Paths::from_home(dir.clone()), cfg, KEY.into(), Box::new(rec.clone())));
    let layout = json!({"version": 1, "grid": {"cols": 8, "rows": 4}, "screens": [
        {"id": "s1", "name": "Главный", "blocks": [
            {"id": "b1", "type": "power", "x": 0, "y": 0, "w": 4, "h": 1, "opts": {"buttons": ["lock", "restart"]}},
            {"id": "b2", "type": "keys", "x": 4, "y": 0, "w": 4, "h": 1, "opts": {"actions": ["mute", "sign", "day", "mic"]}},
            {"id": "b3", "type": "mixer", "x": 0, "y": 1, "w": 3, "h": 2},
            {"id": "b4", "type": "notify", "x": 3, "y": 1, "w": 3, "h": 2, "opts": {"max": 5}}
        ]},
        {"id": "s2", "name": "OBS", "forApp": "OBS64.exe", "blocks": [{"id": "b5", "type": "obs", "x": 0, "y": 0, "w": 4, "h": 2}]}
    ]});
    let (s, v) = call(&st, "127.0.0.1", "PUT", "/admin/layouts/p1", &[("x-admin-key", KEY)], Some(layout)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    Env { st, rec, t1, t2, dir }
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
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

impl Env {
    async fn post(&self, token: &str, uri: &str, body: Value) -> (StatusCode, Value) {
        let auth = format!("Bearer {token}");
        call(&self.st, LAN, "POST", uri, &[("authorization", auth.as_str())], Some(body)).await
    }
    async fn get(&self, token: &str, uri: &str) -> (StatusCode, Value) {
        let auth = format!("Bearer {token}");
        call(&self.st, LAN, "GET", uri, &[("authorization", auth.as_str())], None).await
    }
    fn log(&self) -> Vec<String> {
        self.rec.log.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn layout_carries_actions_wol_and_notify_settings() {
    let e = env("layout").await;
    let (s, v) = e.get(&e.t1, "/api/v1/layout").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["actions"][0], json!({"id": "mute", "title": "Discord: микрофон", "icon": null, "kind": "keys"}), "no keys/text/steps for the phone");
    assert_eq!(v["actions"][2], json!({"id": "day", "title": "Начать день", "icon": null, "kind": "macro"}));
    assert_eq!(v["actions"].as_array().unwrap().len(), 5);
    assert_eq!(v["wol"], json!({"mac": "AA:BB:CC:DD:EE:FF", "broadcast": "192.168.1.255"}));
    assert_eq!(v["notify"], json!({"nightFlash": false}));
    assert_eq!(v["screens"][1]["forApp"], "OBS64.exe");
    // icon file next to the config, as for apps
    std::fs::create_dir_all(&e.st.paths.icons).unwrap();
    std::fs::write(e.st.paths.icons.join("sign.png"), b"png").unwrap();
    let (_, v) = e.get(&e.t2, "/api/v1/layout").await;
    assert_eq!(v["actions"][1]["icon"], "/icons/sign.png");
}

#[tokio::test]
async fn power_only_by_layout() {
    let e = env("power").await;
    let (s, v) = e.post(&e.t2, "/api/v1/power", json!({"action": "lock"})).await;
    assert_eq!((s, v["error"].as_str()), (StatusCode::FORBIDDEN, Some("not in layout")), "classic layout has no power block");
    let (s, _) = e.post(&e.t1, "/api/v1/power", json!({"action": "sleep"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "sleep is not among this block's buttons");
    let (s, _) = e.post(&e.t1, "/api/v1/power", json!({"action": "shutdown"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = e.post(&e.t1, "/api/v1/power", json!({"action": "explode"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(&e.st, LAN, "POST", "/api/v1/power", &[], Some(json!({"action": "lock"}))).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert!(e.log().is_empty(), "nothing ran yet");

    let (s, v) = e.post(&e.t1, "/api/v1/power", json!({"action": "lock"})).await;
    assert_eq!((s, v), (StatusCode::OK, json!({"ok": true})));
    let (s, v) = e.post(&e.t1, "/api/v1/power", json!({"action": "restart"})).await;
    assert_eq!((s, v), (StatusCode::OK, json!({"ok": true, "delay": 10})));
    let (s, v) = e.post(&e.t1, "/api/v1/power", json!({"action": "cancel"})).await;
    assert_eq!((s, v), (StatusCode::OK, json!({"ok": true})));
    assert_eq!(e.log(), ["power:lock", "power:restart", "power:cancel"]);
}

#[tokio::test]
async fn actions_only_by_keys_block() {
    let e = env("action").await;
    let (s, _) = e.post(&e.t1, "/api/v1/action", json!({"id": "nope"})).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = e.post(&e.t1, "/api/v1/action", json!({"id": "secret"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "in config.actions but not in the keys block");
    let (s, _) = e.post(&e.t2, "/api/v1/action", json!({"id": "mute"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert!(e.log().is_empty());
    for id in ["mute", "sign", "day", "mic"] {
        let (s, v) = e.post(&e.t1, "/api/v1/action", json!({"id": id})).await;
        assert_eq!((s, v), (StatusCode::OK, json!({"ok": true})), "{id}");
    }
    assert_eq!(e.log(), ["keys:Ctrl+Shift+M", "text:С уважением", "launch:obs", "wait:3000", "keys:Ctrl+Alt+F9", "audio:micToggle"]);
}

#[tokio::test]
async fn audio_only_with_mixer() {
    let e = env("audio").await;
    let (s, _) = e.post(&e.t2, "/api/v1/audio", json!({"op": "mic", "muted": true})).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    for bad in [json!({"op": "volume", "pid": 10}), json!({"op": "output"}), json!({"op": "boost"}), json!({"op": "appMute", "pid": 1})] {
        let (s, _) = e.post(&e.t1, "/api/v1/audio", bad.clone()).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}");
    }
    let ok = [
        json!({"op": "volume", "pid": 1234, "value": 1.5}),
        json!({"op": "appMute", "pid": 1234, "muted": true}),
        json!({"op": "mic", "muted": false}),
        json!({"op": "output", "id": "{0.0.0.00000000}.{x}"}),
    ];
    for b in ok {
        let (s, v) = e.post(&e.t1, "/api/v1/audio", b.clone()).await;
        assert_eq!((s, v), (StatusCode::OK, json!({"ok": true})), "{b}");
    }
    assert_eq!(e.log(), ["audio:volume:1234:1", "audio:appMute:1234:true", "audio:mic:false", "audio:output:{0.0.0.00000000}.{x}"]);
}

#[tokio::test]
async fn obs_only_with_block_and_reports_no_connection() {
    let e = env("obs").await;
    let (s, _) = e.post(&e.t2, "/api/v1/obs", json!({"op": "stream"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = e.post(&e.t1, "/api/v1/obs", json!({"op": "scene"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "scene needs a name");
    // the OBS task without any panel asking for obs does not connect and answers at once
    tokio::spawn(ven4docking_agent::obs::run(e.st.clone()));
    let (s, v) = e.post(&e.t1, "/api/v1/obs", json!({"op": "stream"})).await;
    assert_eq!((s, v["error"].as_str()), (StatusCode::CONFLICT, Some("OBS не подключён")));
}

fn note(id: u32, aumid: &str, time: f64) -> RawNote {
    RawNote { id, aumid: aumid.into(), app_name: aumid.into(), time, title: format!("t{id}"), text: format!("x{id}") }
}

#[tokio::test]
async fn notifications_filter_dismiss_open() {
    let e = env("notify").await;
    *e.rec.notes.lock().unwrap() = NotifyRead {
        access: Access::Allowed,
        notes: vec![note(11, "TelegramDesktop", 100.0), note(12, "Discord", 200.0), note(13, "Spam", 300.0), note(14, "Outlook", 50.0), note(15, "Outlook", 60.0)],
    };
    ven4docking_agent::notify::refresh(&e.st);
    let (_, v) = e.get(&e.t1, "/api/v1/state?blocks=notify").await;
    assert_eq!(v["notify"]["access"], "allowed");
    let items = v["notify"]["items"].as_array().unwrap();
    let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["12", "11", "15"], "unlisted app hidden, Outlook folded");
    assert_eq!(items[0]["text"], "x12");
    assert!(items[1].get("text").is_none() && items[1]["title"] == "t11");
    assert_eq!(items[2]["count"], 2);
    assert!(items[2].get("title").is_none());

    // a phone without the notify block gets nothing, even when it asks
    let (_, v) = e.get(&e.t2, "/api/v1/state?blocks=notify,cpu").await;
    assert!(v.get("notify").is_none() && v.get("cpu").is_some(), "{v}");
    let (s, _) = e.post(&e.t2, "/api/v1/notify", json!({"op": "dismiss", "id": "12"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    let (s, _) = e.post(&e.t1, "/api/v1/notify", json!({"op": "open", "id": "13"})).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "hidden app cannot be opened");
    let (s, _) = e.post(&e.t1, "/api/v1/notify", json!({"op": "open", "id": "12"})).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = e.post(&e.t1, "/api/v1/notify", json!({"op": "dismiss", "id": "15"})).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = e.post(&e.t1, "/api/v1/notify", json!({"op": "dismiss", "id": 11})).await;
    assert_eq!(s, StatusCode::OK, "a number id works too");
    let (_, v) = e.get(&e.t1, "/api/v1/state?blocks=notify").await;
    assert_eq!(v["notify"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["notify"]["items"][0]["id"], "12");
    // dismissed items stay hidden after the next read
    ven4docking_agent::notify::refresh(&e.st);
    let (_, v) = e.get(&e.t1, "/api/v1/state?blocks=notify").await;
    assert_eq!(v["notify"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(e.log(), ["open:Discord"]);

    // the editor sees app names, never texts
    let (s, v) = call(&e.st, "127.0.0.1", "GET", "/admin/notify/apps", &[("x-admin-key", KEY)], None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["access"], "allowed");
    let apps: Vec<&str> = v["apps"].as_array().unwrap().iter().map(|a| a["aumid"].as_str().unwrap()).collect();
    assert_eq!(apps, ["Discord", "Outlook", "Spam", "TelegramDesktop"]);
    assert_eq!(v["apps"][1]["lastSeen"], 60.0);
    assert!(!v.to_string().contains("x12") && !v.to_string().contains("t12"));

    // switched off: nothing is read
    e.st.cfg.write().unwrap().notify.as_mut().unwrap().enabled = false;
    ven4docking_agent::notify::refresh(&e.st);
    let (_, v) = e.get(&e.t1, "/api/v1/state?blocks=notify").await;
    assert_eq!(v["notify"], json!({"access": "off", "items": []}));
}

#[tokio::test]
async fn running_only_for_program_screens_of_the_phone() {
    let e = env("running").await;
    e.st.set_block("running", json!(["obs64.exe"]), None);
    let (_, v) = e.get(&e.t1, "/api/v1/state?blocks=cpu").await;
    assert_eq!(v["running"], json!(["OBS64.exe"]), "added without asking, spelled as in the layout");
    let (_, v) = e.get(&e.t2, "/api/v1/state?blocks=cpu,running").await;
    assert!(v.get("running").is_none(), "no program screens -> no process names");
    e.st.set_block("running", json!([]), None);
    let (_, v) = e.get(&e.t1, "/api/v1/state?blocks=running").await;
    assert_eq!(v["running"], json!([]));
    // the sampler is asked for running and sound (mixer)
    let (_, v) = e.get(&e.t1, "/api/v1/state?blocks=mixer,audio,keys").await;
    assert!(v.get("audio").is_some() && v["errors"]["audio"].is_string(), "{v}");
    let (_, want) = e.st.demand.active();
    assert!(want.contains("running") && want.contains("mixer"), "{want:?}");
}
