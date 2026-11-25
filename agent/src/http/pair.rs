//! Pairing by a 4-digit code: phone POST /pair/start -> editor accepts -> phone GET /pair/{id} gets the token once.

use super::{err, ok_json, parse_body, St};
use crate::config::{hex, new_token, random_bytes, Phone};
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

pub const CODE_TTL: Duration = Duration::from_secs(120);
pub const MAX_STARTS_PER_MIN: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub enum PairStatus {
    Wait,
    /// token is Some until the phone picked it up
    Ok(Option<String>),
    Denied,
}

#[derive(Debug, Clone)]
pub struct Pending {
    pub pair_id: String,
    pub code: String,
    pub model: String,
    pub android: String,
    pub device_id: String,
    pub ip: IpAddr,
    pub created: Instant,
    pub decided: Option<Instant>,
    pub status: PairStatus,
}

#[derive(Default)]
pub struct Pairs {
    pub map: HashMap<String, Pending>,
    starts: HashMap<IpAddr, Vec<Instant>>,
}

impl Pairs {
    fn purge(&mut self) {
        let now = Instant::now();
        self.map.retain(|_, p| match p.decided {
            Some(d) => now.duration_since(d) < CODE_TTL,
            None => now.duration_since(p.created) < CODE_TTL,
        });
        for v in self.starts.values_mut() {
            v.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
        }
        self.starts.retain(|_, v| !v.is_empty());
    }

    /// Rate limit: at most 5 starts per minute per address.
    pub fn allow_start(&mut self, ip: IpAddr) -> bool {
        self.purge();
        let v = self.starts.entry(ip).or_default();
        if v.len() >= MAX_STARTS_PER_MIN {
            return false;
        }
        v.push(Instant::now());
        true
    }

    pub fn create(&mut self, ip: IpAddr, model: String, android: String, device_id: String) -> (String, String) {
        let pair_id = hex(&random_bytes::<12>());
        let code = loop {
            let r = u16::from_le_bytes(random_bytes::<2>()) % 10000;
            let c = format!("{r:04}");
            if !self.map.values().any(|p| p.code == c && p.status == PairStatus::Wait) {
                break c;
            }
        };
        self.map.insert(
            pair_id.clone(),
            Pending { pair_id: pair_id.clone(), code: code.clone(), model, android, device_id, ip, created: Instant::now(), decided: None, status: PairStatus::Wait },
        );
        (pair_id, code)
    }

    pub fn waiting(&mut self) -> Vec<Pending> {
        self.purge();
        let mut v: Vec<Pending> = self.map.values().filter(|p| p.status == PairStatus::Wait).cloned().collect();
        v.sort_by_key(|p| p.created);
        v
    }

    /// Phone polls; the token is handed out exactly once.
    pub fn poll(&mut self, id: &str) -> Option<Value> {
        self.purge();
        let p = self.map.get_mut(id)?;
        Some(match &mut p.status {
            PairStatus::Wait => json!({"status": "wait"}),
            PairStatus::Denied => json!({"status": "denied"}),
            PairStatus::Ok(tok) => match tok.take() {
                Some(t) => json!({"status": "ok", "token": t}),
                None => json!({"status": "ok"}),
            },
        })
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StartReq {
    model: String,
    android: String,
    device_id: String,
}

/// POST /pair/start {model, android, deviceId} -> {pairId, code}
pub async fn start(st: St, ConnectInfo(addr): ConnectInfo<SocketAddr>, body: Bytes) -> Response {
    let req: StartReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let clip = |s: String| s.chars().take(64).collect::<String>();
    let mut pairs = st.pairs.lock().unwrap();
    if !pairs.allow_start(addr.ip()) {
        return err(StatusCode::TOO_MANY_REQUESTS, "too many attempts");
    }
    let (pair_id, code) = pairs.create(addr.ip(), clip(req.model), clip(req.android), clip(req.device_id));
    tracing::info!("pair request {pair_id} from {}", addr.ip());
    ok_json(json!({"pairId": pair_id, "code": code}))
}

/// GET /pair/{pairId} -> {status: wait|ok|denied, token?}
pub async fn status(st: St, Path(id): Path<String>) -> Response {
    match st.pairs.lock().unwrap().poll(&id) {
        Some(v) => ok_json(v),
        None => err(StatusCode::NOT_FOUND, "unknown or expired pair"),
    }
}

/// Editor accepted: create (or re-key, same deviceId) the phone and give the token to the pair.
pub fn accept(st: &crate::state::AppState, id: &str) -> Result<String, Response> {
    let mut pairs = st.pairs.lock().unwrap();
    pairs.purge();
    let Some(p) = pairs.map.get_mut(id).filter(|p| p.status == PairStatus::Wait) else {
        return Err(err(StatusCode::NOT_FOUND, "unknown or expired pair"));
    };
    let (token, hash) = new_token();
    let phone_id = {
        let mut cfg = st.cfg.write().unwrap();
        let existing = cfg.phones.iter_mut().find(|ph| !p.device_id.is_empty() && ph.device_id.as_deref() == Some(p.device_id.as_str()));
        match existing {
            Some(ph) => {
                ph.token_hash = hash;
                ph.model = Some(p.model.clone()).filter(|s| !s.is_empty()).or(ph.model.take());
                ph.android = Some(p.android.clone()).filter(|s| !s.is_empty()).or(ph.android.take());
                ph.id.clone()
            }
            None => {
                let pid = cfg.next_phone_id();
                cfg.phones.push(new_phone(&pid, &p.model, Some(&p.model), Some(&p.android), Some(&p.device_id), hash));
                pid
            }
        }
    };
    st.save_config().map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    p.status = PairStatus::Ok(Some(token));
    p.decided = Some(Instant::now());
    tracing::info!("pair {id} accepted -> phone {phone_id}");
    Ok(phone_id)
}

pub fn deny(st: &crate::state::AppState, id: &str) -> Result<(), Response> {
    let mut pairs = st.pairs.lock().unwrap();
    pairs.purge();
    let Some(p) = pairs.map.get_mut(id).filter(|p| p.status == PairStatus::Wait) else {
        return Err(err(StatusCode::NOT_FOUND, "unknown or expired pair"));
    };
    p.status = PairStatus::Denied;
    p.decided = Some(Instant::now());
    Ok(())
}

pub fn new_phone(id: &str, name: &str, model: Option<&str>, android: Option<&str>, device_id: Option<&str>, hash: String) -> Phone {
    let opt = |s: Option<&str>| s.filter(|x| !x.is_empty()).map(String::from);
    Phone {
        id: id.into(),
        name: if name.is_empty() { id.into() } else { name.into() },
        token_hash: hash,
        device_id: opt(device_id),
        model: opt(model),
        android: opt(android),
        night: Some(crate::config::Night { from: "23:00".into(), to: "07:00".into(), brightness: 0.02 }),
        dim_when_pc_offline: true,
        burn_in_shift: true,
        extra: Default::default(),
    }
}
