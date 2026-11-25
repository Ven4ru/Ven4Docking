//! Re-finding the PC after its address changed: POST /api/v1/hello {nonce} -> {proofs:[...]}.
//! No token travels: for every paired phone the agent returns HMAC-SHA256(key = tokenHash as stored
//! (hex text), message = nonce). The phone computes the same from sha256hex(its token) and trusts the
//! host only when its value is in the list. Nothing else (ids, names) is revealed.

use super::{err, ok_json, parse_body, St};
use crate::config::hex;
use axum::body::Bytes;
use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

/// Requests per second per address; the rest get a cheap 429.
pub const HELLO_PER_SEC: u32 = 10;
const NONCE_MIN: usize = 32;
const NONCE_MAX: usize = 128;
/// Addresses remembered before old windows are dropped.
const LIMIT_TRACKED: usize = 256;

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    let mut out = [0u8; 32];
    out.copy_from_slice(&outer.finalize());
    out
}

/// Proof for one phone: hex(HMAC-SHA256(key = stored tokenHash text, message = nonce)).
pub fn proof(token_hash: &str, nonce: &str) -> String {
    hex(&hmac_sha256(token_hash.as_bytes(), nonce.as_bytes()))
}

/// 32–128 hex characters.
pub fn valid_nonce(n: &str) -> bool {
    (NONCE_MIN..=NONCE_MAX).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Fixed one-second window per address.
#[derive(Default)]
pub struct HelloLimit {
    map: HashMap<IpAddr, (Instant, u32)>,
}

impl HelloLimit {
    pub fn allow(&mut self, ip: IpAddr, now: Instant) -> bool {
        let window = Duration::from_secs(1);
        if self.map.len() >= LIMIT_TRACKED {
            self.map.retain(|_, (t, _)| now.duration_since(*t) < window);
        }
        let e = self.map.entry(ip).or_insert((now, 0));
        if now.duration_since(e.0) >= window {
            *e = (now, 0);
        }
        e.1 = e.1.saturating_add(1);
        e.1 <= HELLO_PER_SEC
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct HelloReq {
    nonce: String,
}

/// POST /api/v1/hello {"nonce":"<hex>"} -> {"proofs":["<hex>", ...]}; no Authorization.
pub async fn hello(st: St, ConnectInfo(addr): ConnectInfo<SocketAddr>, body: Bytes) -> Response {
    if !st.hello.lock().unwrap().allow(addr.ip(), Instant::now()) {
        return err(StatusCode::TOO_MANY_REQUESTS, "too many requests");
    }
    if body.len() > 1024 {
        return err(StatusCode::BAD_REQUEST, "badNonce");
    }
    let req: HelloReq = match parse_body(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    if !valid_nonce(&req.nonce) {
        return err(StatusCode::BAD_REQUEST, "badNonce");
    }
    let proofs: Vec<String> = st
        .cfg
        .read()
        .unwrap()
        .phones
        .iter()
        .filter(|p| !p.token_hash.is_empty())
        .map(|p| proof(&p.token_hash, &req.nonce))
        .collect();
    ok_json(json!({ "proofs": proofs }))
}
