//! obs-websocket 5: authentication string and the connection test against a small fake OBS.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use ven4docking_agent::config::ObsCfg;
use ven4docking_agent::obs::{auth_string, Cmd};

const SALT: &str = "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=";
const CHALLENGE: &str = "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY=";

#[test]
fn auth_matches_protocol_example() {
    // the example from the obs-websocket 5 protocol description («Creating an authentication string»)
    assert_eq!(auth_string("supersecretpassword", SALT, CHALLENGE), "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4=");
}

#[test]
fn commands() {
    assert_eq!(Cmd::parse("stream", None), Some(Cmd::Stream));
    assert_eq!(Cmd::parse("record", Some("x")), Some(Cmd::Record));
    assert_eq!(Cmd::parse("scene", Some("Игра")), Some(Cmd::Scene("Игра".into())));
    assert_eq!(Cmd::parse("mute", Some("Mic/Aux")), Some(Cmd::Mute("Mic/Aux".into())));
    assert_eq!(Cmd::parse("scene", None), None, "scene needs a name");
    assert_eq!(Cmd::parse("mute", Some("  ")), None);
    assert_eq!(Cmd::parse("quit", None), None);
}

/// One connection of a fake OBS: Hello with a challenge, checks Identify, answers GetVersion.
async fn fake_obs(listener: TcpListener, password: &'static str) {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
    let hello = json!({"op": 0, "d": {"obsWebSocketVersion": "5.5.2", "rpcVersion": 1, "authentication": {"challenge": CHALLENGE, "salt": SALT}}});
    ws.send(Message::text(hello.to_string())).await.unwrap();
    while let Some(Ok(m)) = ws.next().await {
        let Message::Text(t) = m else { continue };
        let v: Value = serde_json::from_str(t.as_str()).unwrap();
        match v["op"].as_u64() {
            Some(1) => {
                if v["d"]["authentication"].as_str() != Some(auth_string(password, SALT, CHALLENGE).as_str()) {
                    let _ = ws.send(Message::Close(Some(CloseFrame { code: CloseCode::Library(4009), reason: "Authentication failed.".into() }))).await;
                    return;
                }
                ws.send(Message::text(json!({"op": 2, "d": {"negotiatedRpcVersion": 1}}).to_string())).await.unwrap();
            }
            Some(6) => {
                let d = &v["d"];
                let resp = json!({"op": 7, "d": {"requestType": d["requestType"], "requestId": d["requestId"],
                    "requestStatus": {"result": true, "code": 100}, "responseData": {"obsVersion": "31.0.1", "obsWebSocketVersion": "5.5.2"}}});
                ws.send(Message::text(resp.to_string())).await.unwrap();
            }
            _ => {}
        }
    }
}

fn cfg_for(port: u16, password: Option<&str>) -> ObsCfg {
    let mut c = ObsCfg { port, ..Default::default() };
    if let Some(p) = password {
        use base64::prelude::*;
        let enc = ven4docking_agent::platform::dpapi_protect(p.as_bytes()).unwrap();
        c.password_dpapi = BASE64_STANDARD.encode(enc);
    }
    c
}

#[tokio::test]
async fn connection_test_reports_version_and_bad_password() {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(fake_obs(l, "supersecretpassword"));
    let v = ven4docking_agent::obs::test(&cfg_for(port, Some("supersecretpassword"))).await;
    assert_eq!(v, json!({"ok": true, "version": "31.0.1"}));

    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(fake_obs(l, "supersecretpassword"));
    let v = ven4docking_agent::obs::test(&cfg_for(port, Some("wrong"))).await;
    assert_eq!(v, json!({"ok": false, "error": "неверный пароль"}));

    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(fake_obs(l, "supersecretpassword"));
    let v = ven4docking_agent::obs::test(&cfg_for(port, None)).await;
    assert_eq!(v["ok"], false);
    assert!(v["error"].as_str().unwrap().contains("пароль"), "{v}");
}

#[tokio::test]
async fn connection_test_without_obs_and_outside_lan() {
    // a port nobody listens on
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    let v = ven4docking_agent::obs::test(&cfg_for(port, None)).await;
    assert_eq!(v["ok"], false);
    let e = v["error"].as_str().unwrap();
    assert!(e == "OBS не запущен" || e == "WebSocket выключен в OBS", "{e}");

    let cfg = ObsCfg { host: "8.8.8.8".into(), ..Default::default() };
    let v = ven4docking_agent::obs::test(&cfg).await;
    assert_eq!(v["ok"], false, "no plain ws:// to the internet");

    let cfg = ObsCfg { password_dpapi: "AQAAAA==".into(), ..Default::default() };
    let v = ven4docking_agent::obs::test(&cfg).await;
    assert!(v["error"].as_str().unwrap().contains("не расшифровывается"), "{v}");
}
