//! Windows notifications on the panel (spec 1.1 §6): UserNotificationListener, polled every 3 s while a panel
//! with the `notify` block is active. Only apps listed in `notify.apps` are shown, title/text only when the
//! app's mode allows it. `dismiss` hides an item on the panel; the notification in Windows is not touched.
//! Item ids are Windows notification ids as strings (stable between polls), `time` is unix seconds.

use crate::config::NotifyCfg;
use crate::state::AppState;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Items sent to the panel at most (the block shows `opts.max` of them).
pub const MAX_ITEMS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Access {
    Allowed,
    Denied,
    #[default]
    Unspecified,
}

impl Access {
    pub fn as_str(self) -> &'static str {
        match self {
            Access::Allowed => "allowed",
            Access::Denied => "denied",
            Access::Unspecified => "unspecified",
        }
    }
}

/// One notification as Windows has it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawNote {
    pub id: u32,
    pub aumid: String,
    pub app_name: String,
    /// unix seconds
    pub time: f64,
    pub title: String,
    pub text: String,
}

/// Result of one read: access state and the notifications (empty without access).
#[derive(Debug, Clone, Default)]
pub struct NotifyRead {
    pub access: Access,
    pub notes: Vec<RawNote>,
}

/// Item shown on the panel and the notifications behind it (one for title/full, all of the app for count).
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub value: Value,
    pub aumid: String,
    pub ids: Vec<u32>,
}

/// `notify-<16 hex of sha256(aumid)>.png` in the icons folder.
pub fn icon_file(aumid: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("notify-{}.png", &crate::config::hex(&Sha256::digest(aumid.as_bytes()))[..16])
}

/// Panel items: apps from `cfg.apps` only, newest first. `count` mode (and `wallCountsOnly`) folds all
/// notifications of an app into one item without title/text; `title` gives the title, `full` title and text.
/// `icon(aumid)` returns the icon URL when the logo file exists.
pub fn filter(notes: &[RawNote], cfg: &NotifyCfg, dismissed: &HashSet<u32>, icon: &dyn Fn(&str) -> Option<String>) -> Vec<Item> {
    let mut sorted: Vec<&RawNote> = notes.iter().filter(|n| !dismissed.contains(&n.id) && cfg.apps.contains_key(&n.aumid)).collect();
    sorted.sort_by(|a, b| b.time.total_cmp(&a.time).then(b.id.cmp(&a.id)));
    let mut items: Vec<Item> = Vec::new();
    let mut folded: HashMap<&str, usize> = HashMap::new();
    for n in sorted {
        let mode = if cfg.wall_counts_only { "count" } else { cfg.apps[&n.aumid].as_str() };
        if mode == "count" {
            if let Some(&i) = folded.get(n.aumid.as_str()) {
                let it = &mut items[i];
                it.ids.push(n.id);
                it.value["count"] = json!(it.ids.len());
                continue;
            }
            folded.insert(n.aumid.as_str(), items.len());
        }
        let mut v = json!({"id": n.id.to_string(), "app": n.aumid, "appName": n.app_name, "time": n.time, "count": 1});
        if let Some(i) = icon(&n.aumid) {
            v["icon"] = json!(i);
        }
        if mode == "title" || mode == "full" {
            v["title"] = json!(n.title);
        }
        if mode == "full" {
            v["text"] = json!(n.text);
        }
        items.push(Item { value: v, aumid: n.aumid.clone(), ids: vec![n.id] });
    }
    items.truncate(MAX_ITEMS);
    items
}

/// What the agent keeps between polls.
#[derive(Debug, Default)]
pub struct NotifyState {
    pub access: Option<Access>,
    pub last: Vec<RawNote>,
    pub items: Vec<Item>,
    pub dismissed: HashSet<u32>,
    /// apps seen since start: aumid -> (name, newest notification time)
    pub seen: BTreeMap<String, (String, f64)>,
}

impl NotifyState {
    fn remember(&mut self, notes: &[RawNote]) {
        for n in notes {
            let e = self.seen.entry(n.aumid.clone()).or_insert_with(|| (n.app_name.clone(), n.time));
            if !n.app_name.is_empty() {
                e.0 = n.app_name.clone();
            }
            e.1 = e.1.max(n.time);
        }
    }
}

fn icon_url(st: &AppState, aumid: &str) -> Option<String> {
    let f = icon_file(aumid);
    st.paths.icons.join(&f).is_file().then(|| format!("/icons/{f}"))
}

/// The `notify` value from the last read (also after `dismiss`).
fn publish(st: &AppState, ns: &mut NotifyState, cfg: &NotifyCfg) {
    ns.items = filter(&ns.last, cfg, &ns.dismissed, &|a| icon_url(st, a));
    let access = ns.access.unwrap_or_default().as_str();
    let items: Vec<Value> = ns.items.iter().map(|i| i.value.clone()).collect();
    st.set_block("notify", json!({"access": access, "items": items}), None);
}

/// One poll (sampler, every 3 s with an active `notify` panel). With `notify.enabled: false` nothing is read.
pub fn refresh(st: &AppState) {
    let cfg = st.cfg.read().unwrap().notify_cfg();
    if !cfg.enabled {
        let mut ns = st.notify.lock().unwrap();
        ns.last.clear();
        ns.items.clear();
        st.set_block("notify", json!({"access": "off", "items": []}), None);
        return;
    }
    match st.actions.notifications(&st.paths.icons) {
        Ok(r) => {
            let mut ns = st.notify.lock().unwrap();
            ns.access = Some(r.access);
            ns.remember(&r.notes);
            // forget dismissed ids that Windows no longer has
            let alive: HashSet<u32> = r.notes.iter().map(|n| n.id).collect();
            ns.dismissed.retain(|id| alive.contains(id));
            ns.last = r.notes;
            publish(st, &mut ns, &cfg);
        }
        Err(e) => {
            st.set_block("notify", json!({"access": "unspecified", "items": []}), Some(e));
        }
    }
}

/// `dismiss`: hides the item (all notifications folded into it). False when the id is not on the panel.
pub fn dismiss(st: &AppState, id: u32) -> bool {
    let cfg = st.cfg.read().unwrap().notify_cfg();
    let mut ns = st.notify.lock().unwrap();
    let Some(ids) = ns.items.iter().find(|i| i.ids.contains(&id)).map(|i| i.ids.clone()) else { return false };
    ns.dismissed.extend(ids);
    publish(st, &mut ns, &cfg);
    true
}

/// `open`: the app of an item on the panel (never an id the phone made up).
pub fn app_of(st: &AppState, id: u32) -> Option<String> {
    st.notify.lock().unwrap().items.iter().find(|i| i.ids.contains(&id)).map(|i| i.aumid.clone())
}

/// GET /admin/notify/apps: reads Windows once (also when notifications are off), keeps only app ids and names.
pub fn apps(st: &AppState) -> Value {
    let read = st.actions.notifications(&st.paths.icons);
    let mut ns = st.notify.lock().unwrap();
    let access = match &read {
        Ok(r) => {
            ns.remember(&r.notes);
            r.access.as_str()
        }
        Err(_) => "unspecified",
    };
    let apps: Vec<Value> = ns.seen.iter().map(|(aumid, (name, last))| json!({"aumid": aumid, "name": name, "lastSeen": last})).collect();
    json!({"access": access, "apps": apps})
}

/// Windows FILETIME-based DateTime (100 ns since 1601) -> unix seconds.
pub fn unix_from_universal(t: i64) -> f64 {
    (t - 116_444_736_000_000_000) as f64 / 1e7
}

#[cfg(windows)]
pub use win::read as read_windows;

#[cfg(windows)]
mod win {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;
    use windows::core::Interface;
    use windows::ApplicationModel::AppInfo;
    use windows::Foundation::Size;
    use windows::Graphics::Imaging::{BitmapDecoder, BitmapEncoder};
    use windows::Storage::Streams::{DataReader, IRandomAccessStream, InMemoryRandomAccessStream};
    use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
    use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};

    /// Apps whose logo was already tried in this run (a failed logo is not retried every 3 s).
    static LOGO_TRIED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

    pub fn read(icons: &Path) -> Result<NotifyRead, String> {
        let l = UserNotificationListener::Current().map_err(|e| format!("уведомления недоступны: {e}"))?;
        let access = match l.GetAccessStatus().map_err(|e| e.to_string())? {
            UserNotificationListenerAccessStatus::Allowed => Access::Allowed,
            UserNotificationListenerAccessStatus::Denied => Access::Denied,
            _ => Access::Unspecified,
        };
        if access != Access::Allowed {
            return Ok(NotifyRead { access, notes: Vec::new() });
        }
        let list = l.GetNotificationsAsync(NotificationKinds::Toast).and_then(|op| op.join()).map_err(|e| format!("уведомления: {e}"))?;
        let mut notes = Vec::new();
        for n in list {
            let Ok(info) = n.AppInfo() else { continue };
            let aumid = info.AppUserModelId().map(|h| h.to_string_lossy()).unwrap_or_default();
            if aumid.is_empty() {
                continue;
            }
            let name = info.DisplayInfo().and_then(|d| d.DisplayName()).map(|h| h.to_string_lossy()).unwrap_or_default();
            let (title, text) = texts(&n);
            notes.push(RawNote {
                id: n.Id().unwrap_or(0),
                aumid: aumid.clone(),
                app_name: name,
                time: n.CreationTime().map(|t| unix_from_universal(t.UniversalTime)).unwrap_or(0.0),
                title,
                text,
            });
            save_logo(&info, &aumid, icons);
        }
        Ok(NotifyRead { access, notes })
    }

    /// ToastGeneric texts: the first is the title, the rest is the text (one per line).
    fn texts(n: &UserNotification) -> (String, String) {
        let Ok(list) = n
            .Notification()
            .and_then(|x| x.Visual())
            .and_then(|v| v.GetBinding(&KnownNotificationBindings::ToastGeneric()?))
            .and_then(|b| b.GetTextElements())
        else {
            return (String::new(), String::new());
        };
        let all: Vec<String> = list.into_iter().filter_map(|t| t.Text().ok()).map(|h| h.to_string_lossy()).collect();
        let mut it = all.into_iter();
        let title = it.next().unwrap_or_default();
        (title, it.collect::<Vec<_>>().join("\n"))
    }

    fn save_logo(info: &AppInfo, aumid: &str, icons: &Path) {
        let path = icons.join(icon_file(aumid));
        if path.is_file() {
            return;
        }
        {
            let mut tried = LOGO_TRIED.lock().unwrap_or_else(|e| e.into_inner());
            if !tried.get_or_insert_with(HashSet::new).insert(aumid.to_string()) {
                return;
            }
        }
        match logo_png(info) {
            Ok(png) => {
                if let Err(e) = crate::config::write_atomic(&path, &png) {
                    tracing::debug!("notify icon {}: {e}", path.display());
                }
            }
            Err(e) => tracing::debug!("notify icon for {aumid}: {e}"),
        }
    }

    fn read_all(stream: &IRandomAccessStream) -> windows::core::Result<Vec<u8>> {
        let size = stream.Size()? as u32;
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
        reader.LoadAsync(size)?.join()?;
        let mut buf = vec![0u8; size as usize];
        reader.ReadBytes(&mut buf)?;
        Ok(buf)
    }

    /// App logo 64×64 as PNG (converted when Windows gives another format).
    fn logo_png(info: &AppInfo) -> windows::core::Result<Vec<u8>> {
        let r = info.DisplayInfo()?.GetLogo(Size { Width: 64.0, Height: 64.0 })?;
        let stream: IRandomAccessStream = r.OpenReadAsync()?.join()?.cast()?;
        let bytes = read_all(&stream)?;
        if bytes.starts_with(b"\x89PNG") {
            return Ok(bytes);
        }
        stream.Seek(0)?;
        let dec = BitmapDecoder::CreateAsync(&stream)?.join()?;
        let bmp = dec.GetSoftwareBitmapAsync()?.join()?;
        let out = InMemoryRandomAccessStream::new()?;
        let enc = BitmapEncoder::CreateAsync(BitmapEncoder::PngEncoderId()?, &out)?.join()?;
        enc.SetSoftwareBitmap(&bmp)?;
        enc.FlushAsync()?.join()?;
        let out: IRandomAccessStream = out.cast()?;
        out.Seek(0)?;
        read_all(&out)
    }
}
