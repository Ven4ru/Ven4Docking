//! HTTP: panel API v1 (LAN, phone token), pairing, static panel/icons/APK, editor API (127.0.0.1 + X-Admin-Key).

pub mod admin;
pub mod files;
pub mod pair;
pub mod panel;

use crate::config::ct_eq;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

pub type St = State<Arc<AppState>>;

pub fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({"error": msg}))).into_response()
}

pub fn ok_json(v: Value) -> Response {
    Json(v).into_response()
}

/// Parse a JSON body ourselves so every error is JSON (`{"error":"badJson"}`).
pub fn parse_body<T: DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    let body = if body.is_empty() { b"{}".as_slice() } else { body };
    serde_json::from_slice(body).map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": "badJson", "detail": e.to_string()}))).into_response())
}

fn peer(req: &Request) -> Option<SocketAddr> {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0)
        .or_else(|| req.extensions().get::<axum::extract::connect_info::MockConnectInfo<SocketAddr>>().map(|c| c.0))
}

/// Everything: client must come from a private network (403 otherwise).
async fn private_only(req: Request, next: Next) -> Response {
    match peer(&req) {
        Some(a) if crate::util::is_private(a.ip()) => {
            let mut r = next.run(req).await;
            r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            r
        }
        _ => err(StatusCode::FORBIDDEN, "lan only"),
    }
}

/// /admin/*: only 127.0.0.1 (403) and a matching X-Admin-Key (401).
async fn admin_only(State(st): St, req: Request, next: Next) -> Response {
    match peer(&req) {
        Some(a) if crate::util::is_loopback(a.ip()) => {}
        _ => return err(StatusCode::FORBIDDEN, "localhost only"),
    }
    let key = req.headers().get("x-admin-key").and_then(|v| v.to_str().ok()).unwrap_or("");
    if key.is_empty() || !ct_eq(key.as_bytes(), st.admin_key.as_bytes()) {
        return err(StatusCode::UNAUTHORIZED, "bad admin key");
    }
    next.run(req).await
}

/// Phone id from `Authorization: Bearer <token>`; 401 when missing or unknown.
pub fn phone_from(st: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    let h = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    let token = h.strip_prefix("Bearer ").or_else(|| h.strip_prefix("bearer ")).unwrap_or("").trim();
    if token.is_empty() {
        return Err(err(StatusCode::UNAUTHORIZED, "no token"));
    }
    st.cfg.read().unwrap().phone_by_token(token).map(|p| p.id.clone()).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "bad token"))
}

pub fn router(st: Arc<AppState>) -> Router {
    let admin = admin::router().layer(middleware::from_fn_with_state(st.clone(), admin_only));
    Router::new()
        .route("/api/v1/layout", get(panel::layout))
        .route("/api/v1/state", get(panel::state))
        .route("/api/v1/launch", post(panel::launch))
        .route("/api/v1/media", post(panel::media))
        .route("/api/v1/phone", post(panel::phone))
        .route("/api/{*rest}", get(api_404).post(api_404))
        .route("/pair/start", post(pair::start))
        .route("/pair/{id}", get(pair::status))
        .route("/apk/{file}", get(files::apk))
        .route("/icons/{file}", get(files::icon))
        .nest("/admin", admin)
        .fallback(get(files::panel))
        .layer(middleware::from_fn(private_only))
        .with_state(st)
}

async fn api_404() -> Response {
    err(StatusCode::NOT_FOUND, "no such api")
}
