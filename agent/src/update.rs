//! Update check: once a day (the first two minutes after start) the latest GitHub release is read over
//! WinHTTP. A newer version adds «Обновить до X.Y.Z» to the tray menu and shows one Windows notification per
//! version (`update.json` remembers which). Off with `updateCheck: false` or `VEN4DOCKING_NO_UPDATE_CHECK=1`.

use crate::config::AGENT_VERSION;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub const FIRST_CHECK: Duration = Duration::from_secs(120);
pub const EVERY: Duration = Duration::from_secs(24 * 3600);
/// After a failed check (no network, GitHub limit).
pub const RETRY: Duration = Duration::from_secs(3600);
const HOST: &str = "api.github.com";
const PATH: &str = "/repos/Ven4ru/Ven4Docking/releases/latest";
/// AppUserModelId for the notification (registered under HKCU\Software\Classes\AppUserModelId).
pub const AUMID: &str = "Ven4ru.Ven4Docking";

/// What /admin/status and the tray know.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateState {
    /// version of the latest release ("1.1.0"), None until a check succeeded
    pub latest: Option<String>,
    /// unix time of the last successful check
    pub checked: Option<f64>,
}

impl UpdateState {
    /// The latest release when it is newer than this agent.
    pub fn available(&self) -> Option<&str> {
        self.latest.as_deref().filter(|l| is_newer(l, AGENT_VERSION))
    }
}

/// "v1.1.0", "1.1.0-beta", "1.2" -> (numbers, pre-release). None for anything else.
pub fn parse_version(tag: &str) -> Option<([u64; 3], Option<String>)> {
    let t = tag.trim();
    let t = t.strip_prefix(['v', 'V']).unwrap_or(t);
    let (core, pre) = match t.split_once('-') {
        Some((c, p)) if !p.is_empty() => (c, Some(p.to_string())),
        Some(_) => return None,
        None => (t, None),
    };
    let core = core.split('+').next().unwrap_or(core);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut n = [0u64; 3];
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        n[i] = p.parse().ok()?;
    }
    Some((n, pre))
}

/// Version from a tag without the leading `v`.
pub fn tag_version(tag: &str) -> String {
    let t = tag.trim();
    t.strip_prefix(['v', 'V']).unwrap_or(t).to_string()
}

/// Semver order: numbers first; with equal numbers a pre-release is older than the release.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (Some((a, ap)), Some((b, bp))) = (parse_version(candidate), parse_version(current)) else { return false };
    if a != b {
        return a > b;
    }
    match (ap, bp) {
        (None, Some(_)) => true,
        (Some(x), Some(y)) => x > y,
        _ => false,
    }
}

/// `update.json`: the version a notification was already shown for.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Notified {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notified: Option<String>,
}

pub fn load_notified(path: &Path) -> Notified {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()).unwrap_or_default()
}

/// Show a notification for `latest`? Only when it is newer than this agent and than the version already shown.
pub fn should_notify(latest: &str, current: &str, n: &Notified) -> bool {
    is_newer(latest, current) && n.notified.as_deref().is_none_or(|shown| is_newer(latest, shown))
}

/// `tag_name` of the release JSON.
pub fn tag_from_release(body: &[u8]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    v["tag_name"].as_str().filter(|t| parse_version(t).is_some()).map(String::from)
}

fn disabled(st: &AppState) -> bool {
    std::env::var("VEN4DOCKING_NO_UPDATE_CHECK").is_ok_and(|v| v == "1") || !st.cfg.read().unwrap().update_check
}

/// One check: reads the release, updates the state, shows the notification once per version.
#[cfg(windows)]
fn check(st: &AppState) -> Result<(), String> {
    let ua = format!("Ven4Docking-Agent/{AGENT_VERSION}");
    let (code, body) = win::https_get(HOST, PATH, &ua, Duration::from_secs(15))?;
    if code != 200 {
        return Err(format!("HTTP {code}"));
    }
    let tag = tag_from_release(&body).ok_or("no tag_name")?;
    let latest = tag_version(&tag);
    {
        let mut u = st.update.lock().unwrap();
        u.latest = Some(latest.clone());
        u.checked = Some(crate::ai::unix_now());
    }
    let file = st.paths.home.join("update.json");
    let n = load_notified(&file);
    if should_notify(&latest, AGENT_VERSION, &n) {
        tracing::info!("update {latest} available");
        match win::toast(&format!("Доступна Ven4Docking {latest}"), "Откройте трей → Обновить") {
            Ok(()) => {
                let text = serde_json::to_string_pretty(&Notified { notified: Some(latest) }).unwrap_or_default();
                if let Err(e) = crate::config::write_atomic(&file, text.as_bytes()) {
                    tracing::warn!("update.json: {e}");
                }
            }
            Err(e) => tracing::warn!("update notification: {e}"),
        }
    }
    Ok(())
}

/// The update task (spawned once on the runtime).
#[cfg(windows)]
pub async fn run(st: Arc<AppState>) {
    let mut wait = FIRST_CHECK;
    loop {
        tokio::time::sleep(wait).await;
        if disabled(&st) || st.paused() {
            wait = RETRY;
            continue;
        }
        let st2 = st.clone();
        let r = tokio::task::spawn_blocking(move || {
            let _com = crate::platform::Com::mta();
            check(&st2)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        wait = match r {
            Ok(()) => EVERY,
            Err(e) => {
                tracing::info!("update check: {e}");
                RETRY
            }
        };
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::time::Duration;
    use windows::core::{HSTRING, PCWSTR};
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Win32::Networking::WinHttp::*;
    use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

    /// WinHTTP handle closed on drop.
    struct H(*mut c_void);
    impl Drop for H {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let _ = WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    fn last(what: &str) -> String {
        format!("{what}: {}", std::io::Error::last_os_error())
    }

    /// GET https://host/path: (status, body up to 1 MB). System proxy settings are used.
    pub fn https_get(host: &str, path: &str, ua: &str, timeout: Duration) -> Result<(u32, Vec<u8>), String> {
        let ms = timeout.as_millis() as i32;
        unsafe {
            let s = H(WinHttpOpen(&HSTRING::from(ua), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0));
            if s.0.is_null() {
                return Err(last("WinHttpOpen"));
            }
            WinHttpSetTimeouts(s.0, ms, ms, ms, ms).map_err(|e| e.to_string())?;
            let c = H(WinHttpConnect(s.0, &HSTRING::from(host), 443, 0));
            if c.0.is_null() {
                return Err(last("WinHttpConnect"));
            }
            let r = H(WinHttpOpenRequest(c.0, &HSTRING::from("GET"), &HSTRING::from(path), PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE));
            if r.0.is_null() {
                return Err(last("WinHttpOpenRequest"));
            }
            let headers: Vec<u16> = "Accept: application/vnd.github+json\r\n".encode_utf16().collect();
            WinHttpSendRequest(r.0, Some(&headers), None, 0, 0, 0).map_err(|e| format!("нет связи с GitHub: {}", e.message()))?;
            WinHttpReceiveResponse(r.0, std::ptr::null_mut()).map_err(|e| format!("GitHub не ответил: {}", e.message()))?;
            let mut code = 0u32;
            let mut len = 4u32;
            WinHttpQueryHeaders(r.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, PCWSTR::null(), Some(&mut code as *mut u32 as *mut c_void), &mut len, std::ptr::null_mut())
                .map_err(|e| e.to_string())?;
            let mut body = Vec::new();
            loop {
                let mut avail = 0u32;
                WinHttpQueryDataAvailable(r.0, &mut avail).map_err(|e| e.to_string())?;
                if avail == 0 {
                    break;
                }
                let start = body.len();
                body.resize(start + avail as usize, 0);
                let mut got = 0u32;
                WinHttpReadData(r.0, body[start..].as_mut_ptr() as *mut c_void, avail, &mut got).map_err(|e| e.to_string())?;
                body.truncate(start + got as usize);
                if body.len() > 1024 * 1024 {
                    return Err("ответ GitHub слишком большой".into());
                }
            }
            Ok((code, body))
        }
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    /// Windows notification from a plain exe: the AppUserModelId needs a DisplayName in HKCU first.
    pub fn toast(title: &str, text: &str) -> Result<(), String> {
        let key = HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{}", super::AUMID));
        let name: Vec<u16> = "Ven4Docking".encode_utf16().chain(std::iter::once(0)).collect();
        let r = unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, &key, &HSTRING::from("DisplayName"), REG_SZ.0, Some(name.as_ptr() as *const c_void), (name.len() * 2) as u32) };
        if r.is_err() {
            return Err(format!("реестр: {}", r.0));
        }
        let xml = format!(
            "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
            xml_escape(title),
            xml_escape(text)
        );
        let show = || -> windows::core::Result<()> {
            let doc = XmlDocument::new()?;
            doc.LoadXml(&HSTRING::from(xml))?;
            let t = ToastNotification::CreateToastNotification(&doc)?;
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(super::AUMID))?.Show(&t)
        };
        show().map_err(|e| e.message().to_string())
    }
}

/// Starts the editor's updater (`Ven4Docking.exe --update` next to the agent).
pub fn start_updater() {
    let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else { return };
    let exe = dir.join("Ven4Docking.exe");
    if !exe.exists() {
        tracing::warn!("Ven4Docking.exe not found next to the agent");
        return;
    }
    if let Err(e) = std::process::Command::new(&exe).arg("--update").spawn() {
        tracing::warn!("cannot start the updater: {e}");
    }
}
