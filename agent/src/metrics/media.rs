//! What is playing (WinRT GlobalSystemMediaTransportControlsSessionManager) with the controls the session
//! supports; media control through the session itself (TrySkipNext/Previous/TogglePlayPause/Stop), virtual
//! media keys only without a session and for volume; app launch (ShellExecuteW).
//! Called through the `Actions` trait (crate::actions).

use crate::config::App;
use serde::Serialize;

/// Media-key virtual codes.
pub fn media_vk(action: &str) -> Option<u8> {
    Some(match action {
        "playpause" => 0xB3,
        "next" => 0xB0,
        "prev" => 0xB1,
        "stop" => 0xB2,
        "volup" => 0xAF,
        "voldown" => 0xAE,
        "mute" => 0xAD,
        _ => return None,
    })
}

/// Actions the current media session handles itself (volume always goes through virtual keys).
pub fn is_session_action(action: &str) -> bool {
    matches!(action, "playpause" | "next" | "prev" | "stop")
}

/// `media` in /api/v1/state.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub app: String,
    pub playing: bool,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_play_pause: bool,
}

/// Result of POST /api/v1/media.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaOutcome {
    Done,
    /// the session says this control is disabled (e.g. VK Music in a browser: no next/prev)
    Unsupported,
}

/// "Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic" -> "ZuneMusic", "chrome.exe" -> "chrome",
/// hash-like ids (Firefox) -> "".
pub fn app_name(aumid: &str) -> String {
    let a = aumid.split('!').next().unwrap_or("");
    let a = a.rsplit(['\\', '/']).next().unwrap_or("");
    let a = a.strip_suffix(".exe").or_else(|| a.strip_suffix(".EXE")).unwrap_or(a);
    let a = a.rsplit('.').next().unwrap_or(a);
    let a = a.split('_').next().unwrap_or(a);
    if !a.is_empty() && a.chars().all(|c| c.is_ascii_hexdigit()) && a.len() >= 8 {
        return String::new();
    }
    a.to_string()
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Media::Control::*;

    pub fn manager() -> Result<GlobalSystemMediaTransportControlsSessionManager, String> {
        GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|op| op.join())
            .map_err(|e| format!("медиа недоступно: {e}"))
    }

    pub fn info(s: &GlobalSystemMediaTransportControlsSession) -> Result<MediaInfo, String> {
        let props = s.TryGetMediaPropertiesAsync().and_then(|op| op.join()).map_err(|e| e.to_string())?;
        let pb = s.GetPlaybackInfo().ok();
        let playing = pb
            .as_ref()
            .and_then(|i| i.PlaybackStatus().ok())
            .map(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
            .unwrap_or(false);
        let ctl = pb.as_ref().and_then(|i| i.Controls().ok());
        let flag = |f: &dyn Fn(&GlobalSystemMediaTransportControlsSessionPlaybackControls) -> windows::core::Result<bool>| {
            ctl.as_ref().and_then(|c| f(c).ok()).unwrap_or(false)
        };
        Ok(MediaInfo {
            title: props.Title().map(|h| h.to_string_lossy()).unwrap_or_default(),
            artist: props.Artist().map(|h| h.to_string_lossy()).unwrap_or_default(),
            app: s.SourceAppUserModelId().map(|h| app_name(&h.to_string_lossy())).unwrap_or_default(),
            playing,
            can_next: flag(&|c| c.IsNextEnabled()),
            can_prev: flag(&|c| c.IsPreviousEnabled()),
            can_play_pause: flag(&|c| c.IsPlayPauseToggleEnabled()),
        })
    }

    /// Ok(None) = no current session -> caller falls back to virtual keys.
    pub fn session_control(action: &str) -> Result<Option<MediaOutcome>, String> {
        let mgr = manager()?;
        let Ok(s) = mgr.GetCurrentSession() else { return Ok(None) };
        let ctl = s.GetPlaybackInfo().and_then(|i| i.Controls()).map_err(|e| e.to_string())?;
        let enabled = match action {
            "next" => ctl.IsNextEnabled(),
            "prev" => ctl.IsPreviousEnabled(),
            "playpause" => ctl.IsPlayPauseToggleEnabled(),
            "stop" => ctl.IsStopEnabled(),
            _ => return Ok(None),
        }
        .unwrap_or(false);
        if !enabled {
            return Ok(Some(MediaOutcome::Unsupported));
        }
        let op = match action {
            "next" => s.TrySkipNextAsync(),
            "prev" => s.TrySkipPreviousAsync(),
            "playpause" => s.TryTogglePlayPauseAsync(),
            _ => s.TryStopAsync(),
        };
        let ok = op.and_then(|o| o.join()).map_err(|e| e.to_string())?;
        Ok(Some(if ok { MediaOutcome::Done } else { MediaOutcome::Unsupported }))
    }
}

#[cfg(windows)]
pub struct Media {
    mgr: Option<windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager>,
    tried: bool,
}

#[cfg(windows)]
impl Media {
    pub fn new() -> Media {
        Media { mgr: None, tried: false }
    }

    /// Ok(None) = nothing is playing; Err = WinRT unavailable.
    pub fn now(&mut self) -> Result<Option<MediaInfo>, String> {
        if self.mgr.is_none() {
            if self.tried {
                return Err("медиа недоступно".into());
            }
            self.tried = true;
            self.mgr = Some(win::manager()?);
        }
        let Ok(s) = self.mgr.as_ref().unwrap().GetCurrentSession() else { return Ok(None) };
        win::info(&s).map(Some)
    }
}

#[cfg(windows)]
impl Default for Media {
    fn default() -> Self {
        Media::new()
    }
}

pub use crate::actions::{Actions, RecordActions, WinActions};

#[cfg(windows)]
fn press_vk(vk: u8) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP};
    unsafe {
        keybd_event(vk, 0, KEYEVENTF_EXTENDEDKEY, 0);
        keybd_event(vk, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
    }
}

/// Starts an app from the config: ShellExecute for shortcut/exe/url, hidden `cmd /c` for a command.
#[cfg(windows)]
pub fn launch_app(app: &App) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};
    let (file, params, show) = if app.kind == "command" {
        ("cmd.exe".to_string(), format!("/c {} {}", app.target, app.args), SW_HIDE)
    } else {
        (app.target.clone(), app.args.clone(), SW_SHOWNORMAL)
    };
    if file.trim().is_empty() {
        return Err("пустая цель".into());
    }
    let (f, p, verb) = (HSTRING::from(file), HSTRING::from(params.trim()), HSTRING::from("open"));
    let r = unsafe { ShellExecuteW(None, &verb, &f, &p, PCWSTR::null(), show) };
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecute: {}", r.0 as isize))
    }
}

/// Media control: through the current session when it has one, otherwise (and for volume) a virtual key.
#[cfg(windows)]
pub fn media_control(action: &str) -> Result<MediaOutcome, String> {
    let vk = media_vk(action).ok_or("unknown action")?;
    if is_session_action(action) {
        match win::session_control(action) {
            Ok(Some(outcome)) => return Ok(outcome),
            Ok(None) => {} // no session: virtual key below
            Err(e) => tracing::debug!("media session control failed, using keys: {e}"),
        }
    }
    press_vk(vk);
    Ok(MediaOutcome::Done)
}
