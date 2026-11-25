//! Static files without a token: panel (VEN4DOCKING_PANEL_DIR or ../panel/dist next to the exe),
//! icons (<home>/icons), APK (VEN4DOCKING_APK_DIR or ../apk next to the exe).

use super::{err, St};
use axum::extract::Path;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use std::path::{Path as FsPath, PathBuf};

pub fn mime(p: &FsPath) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "webmanifest" => "application/manifest+json",
        "apk" => "application/vnd.android.package-archive",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Join a URL path onto a base dir; refuses `..`, drive letters, backslashes, empty segments.
pub fn safe_join(base: &FsPath, rel: &str) -> Option<PathBuf> {
    let mut p = base.to_path_buf();
    for seg in rel.split('/') {
        if seg.is_empty() {
            continue;
        }
        if seg == "." || seg == ".." || seg.contains('\\') || seg.contains(':') || seg.starts_with('.') {
            return None;
        }
        p.push(seg);
    }
    Some(p)
}

async fn send_file(p: Option<PathBuf>) -> Response {
    let Some(p) = p else { return err(StatusCode::NOT_FOUND, "not found") };
    match tokio::task::spawn_blocking({ let p = p.clone(); move || std::fs::read(p) }).await.map_err(|e| std::io::Error::other(e.to_string())).and_then(|r| r) {
        Ok(bytes) => ([(header::CONTENT_TYPE, mime(&p))], bytes).into_response(),
        Err(_) => err(StatusCode::NOT_FOUND, "not found"),
    }
}

pub async fn panel(st: St, uri: Uri) -> Response {
    let path = uri.path();
    let rel = if path == "/" || path.is_empty() { "index.html" } else { path.trim_start_matches('/') };
    let rel = percent_decode(rel);
    send_file(safe_join(&st.paths.panel, &rel)).await
}

pub async fn icon(st: St, Path(file): Path<String>) -> Response {
    send_file(safe_join(&st.paths.icons, &file)).await
}

pub async fn apk(st: St, Path(file): Path<String>) -> Response {
    send_file(safe_join(&st.paths.apk, &file)).await
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
