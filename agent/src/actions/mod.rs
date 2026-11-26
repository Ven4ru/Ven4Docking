//! Side effects requested by the panel, behind the `Actions` trait: app launch, media keys, power, key
//! combinations and text, sound, opening the app of a notification. The notification reader lives here too:
//! it is not a side effect, but tests replace it with a fixed list. Tests use `RecordActions`, which only records.

pub mod keys;
#[cfg(windows)]
pub mod power;

use crate::config::{Action, App};
use crate::metrics::media::MediaOutcome;
use crate::notify::NotifyRead;
use keys::Combo;
use std::sync::Mutex;

/// POST /api/v1/power.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Lock,
    Sleep,
    Restart,
    Shutdown,
    Cancel,
}

/// Seconds between the request and restart/shutdown (`shutdown.exe /t`), the panel shows a cancel bar.
pub const SHUTDOWN_DELAY: u32 = 10;

impl PowerAction {
    pub fn parse(s: &str) -> Option<PowerAction> {
        Some(match s {
            "lock" => PowerAction::Lock,
            "sleep" => PowerAction::Sleep,
            "restart" => PowerAction::Restart,
            "shutdown" => PowerAction::Shutdown,
            "cancel" => PowerAction::Cancel,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PowerAction::Lock => "lock",
            PowerAction::Sleep => "sleep",
            PowerAction::Restart => "restart",
            PowerAction::Shutdown => "shutdown",
            PowerAction::Cancel => "cancel",
        }
    }
}

/// Sound changes (POST /api/v1/audio and the `mic` / `audioOut` actions).
#[derive(Debug, Clone, PartialEq)]
pub enum AudioOp {
    /// default output device for all roles
    Output(String),
    Volume { pid: u32, value: f32 },
    AppMute { pid: u32, muted: bool },
    Mic(bool),
    MicToggle,
    /// the next device of the list after the current default one (`audioOut`)
    NextOutput(Vec<String>),
}

impl std::fmt::Display for AudioOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioOp::Output(id) => write!(f, "output:{id}"),
            AudioOp::Volume { pid, value } => write!(f, "volume:{pid}:{value}"),
            AudioOp::AppMute { pid, muted } => write!(f, "appMute:{pid}:{muted}"),
            AudioOp::Mic(m) => write!(f, "mic:{m}"),
            AudioOp::MicToggle => write!(f, "micToggle"),
            AudioOp::NextOutput(list) => write!(f, "nextOutput:{}", list.join(",")),
        }
    }
}

/// Side effects requested by the panel. Tests use a recorder instead of the real one.
pub trait Actions: Send + Sync {
    fn launch(&self, app: &App) -> Result<(), String>;
    fn media(&self, action: &str) -> Result<MediaOutcome, String>;
    fn power(&self, action: PowerAction) -> Result<(), String>;
    fn keys(&self, combo: &Combo) -> Result<(), String>;
    fn text(&self, text: &str) -> Result<(), String>;
    /// Pause between macro steps.
    fn wait(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    fn audio(&self, op: &AudioOp) -> Result<(), String>;
    /// Opens the app a notification came from (`shell:AppsFolder\<AUMID>`).
    fn notify_open(&self, aumid: &str) -> Result<(), String>;
    /// Windows notifications right now; `icons` is where app logos are saved (`notify-<hash>.png`).
    fn notifications(&self, icons: &std::path::Path) -> Result<NotifyRead, String>;
}

/// One action at a time: two macros must not mix their keys.
static RUNNING: Mutex<()> = Mutex::new(());

/// Runs an action from `config.actions` (already checked by the config validation).
pub fn run_action(actions: &dyn Actions, a: &Action, apps: &[App]) -> Result<(), String> {
    let _one = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    match a.kind.as_str() {
        "keys" => actions.keys(&keys::parse_combo(a.keys.as_deref().unwrap_or(""))?),
        "text" => actions.text(a.text.as_deref().unwrap_or("")),
        "mic" => actions.audio(&AudioOp::MicToggle),
        "audioOut" => {
            if a.devices.is_empty() {
                return Err("не выбраны устройства вывода".into());
            }
            actions.audio(&AudioOp::NextOutput(a.devices.clone()))
        }
        "macro" => {
            for s in &a.steps {
                if let Some(k) = &s.keys {
                    actions.keys(&keys::parse_combo(k)?)?;
                } else if let Some(t) = &s.text {
                    actions.text(t)?;
                } else if let Some(ms) = s.wait {
                    actions.wait(ms);
                } else if let Some(id) = &s.launch {
                    let app = apps.iter().find(|x| &x.id == id).ok_or_else(|| format!("нет приложения {id}"))?;
                    actions.launch(app)?;
                }
            }
            Ok(())
        }
        k => Err(format!("неизвестный тип действия {k}")),
    }
}

pub struct WinActions;

#[cfg(windows)]
impl Actions for WinActions {
    fn launch(&self, app: &App) -> Result<(), String> {
        crate::metrics::media::launch_app(app)
    }

    fn media(&self, action: &str) -> Result<MediaOutcome, String> {
        crate::metrics::media::media_control(action)
    }

    fn power(&self, action: PowerAction) -> Result<(), String> {
        power::run(action)
    }

    fn keys(&self, combo: &Combo) -> Result<(), String> {
        keys::send_combo(combo)
    }

    fn text(&self, text: &str) -> Result<(), String> {
        keys::send_text(text)
    }

    fn audio(&self, op: &AudioOp) -> Result<(), String> {
        let _com = crate::platform::Com::mta();
        crate::metrics::audio::apply(op)
    }

    fn notify_open(&self, aumid: &str) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let sys = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
        std::process::Command::new(sys.join("explorer.exe"))
            .arg(format!("shell:AppsFolder\\{aumid}"))
            .creation_flags(0x0800_0000)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("не удалось открыть приложение: {e}"))
    }

    fn notifications(&self, icons: &std::path::Path) -> Result<NotifyRead, String> {
        let _com = crate::platform::Com::mta();
        crate::notify::read_windows(icons)
    }
}

/// Shared actions (tests keep a handle to read the log).
impl<T: Actions + ?Sized> Actions for std::sync::Arc<T> {
    fn launch(&self, app: &App) -> Result<(), String> {
        (**self).launch(app)
    }
    fn media(&self, action: &str) -> Result<MediaOutcome, String> {
        (**self).media(action)
    }
    fn power(&self, action: PowerAction) -> Result<(), String> {
        (**self).power(action)
    }
    fn keys(&self, combo: &Combo) -> Result<(), String> {
        (**self).keys(combo)
    }
    fn text(&self, text: &str) -> Result<(), String> {
        (**self).text(text)
    }
    fn wait(&self, ms: u64) {
        (**self).wait(ms)
    }
    fn audio(&self, op: &AudioOp) -> Result<(), String> {
        (**self).audio(op)
    }
    fn notify_open(&self, aumid: &str) -> Result<(), String> {
        (**self).notify_open(aumid)
    }
    fn notifications(&self, icons: &std::path::Path) -> Result<NotifyRead, String> {
        (**self).notifications(icons)
    }
}

/// For tests: records requests, does nothing. Actions listed in `unsupported` report "unsupported";
/// `notes` is what the notification reader returns.
#[derive(Default)]
pub struct RecordActions {
    pub log: Mutex<Vec<String>>,
    pub unsupported: Vec<String>,
    pub notes: Mutex<NotifyRead>,
}

impl RecordActions {
    fn rec(&self, s: String) {
        self.log.lock().unwrap().push(s);
    }
}

impl Actions for RecordActions {
    fn launch(&self, app: &App) -> Result<(), String> {
        self.rec(format!("launch:{}", app.id));
        Ok(())
    }
    fn media(&self, action: &str) -> Result<MediaOutcome, String> {
        self.rec(format!("media:{action}"));
        if self.unsupported.iter().any(|a| a == action) {
            Ok(MediaOutcome::Unsupported)
        } else {
            Ok(MediaOutcome::Done)
        }
    }
    fn power(&self, action: PowerAction) -> Result<(), String> {
        self.rec(format!("power:{}", action.as_str()));
        Ok(())
    }
    fn keys(&self, combo: &Combo) -> Result<(), String> {
        self.rec(format!("keys:{}", combo.text));
        Ok(())
    }
    fn text(&self, text: &str) -> Result<(), String> {
        self.rec(format!("text:{text}"));
        Ok(())
    }
    fn wait(&self, ms: u64) {
        self.rec(format!("wait:{ms}"));
    }
    fn audio(&self, op: &AudioOp) -> Result<(), String> {
        self.rec(format!("audio:{op}"));
        Ok(())
    }
    fn notify_open(&self, aumid: &str) -> Result<(), String> {
        self.rec(format!("open:{aumid}"));
        Ok(())
    }
    fn notifications(&self, _icons: &std::path::Path) -> Result<NotifyRead, String> {
        Ok(self.notes.lock().unwrap().clone())
    }
}
