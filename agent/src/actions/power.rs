//! Power buttons (spec 1.1 §5): lock — LockWorkStation, sleep — SetSuspendState, restart/shutdown —
//! `shutdown.exe /r|/s /t 10` (the user can still cancel), cancel — `shutdown.exe /a`.

use super::{PowerAction, SHUTDOWN_DELAY};

/// `shutdown.exe /a` when nothing is scheduled.
const ERROR_NO_SHUTDOWN_IN_PROGRESS: i32 = 1116;

pub fn run(action: PowerAction) -> Result<(), String> {
    match action {
        PowerAction::Lock => unsafe { windows::Win32::System::Shutdown::LockWorkStation() }.map_err(|e| format!("не удалось заблокировать: {e}")),
        PowerAction::Sleep => {
            // SetSuspendState returns only after the PC wakes up: answer the phone first
            std::thread::Builder::new()
                .name("sleep".into())
                .spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if !unsafe { windows::Win32::System::Power::SetSuspendState(false, false, false) } {
                        tracing::warn!("SetSuspendState failed: {}", std::io::Error::last_os_error());
                    }
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        PowerAction::Restart | PowerAction::Shutdown => {
            let (flag, what) = if action == PowerAction::Restart { ("/r", "перезагрузка") } else { ("/s", "выключение") };
            let delay = SHUTDOWN_DELAY.to_string();
            let comment = format!("Ven4Docking: {what} по запросу с телефона");
            match crate::platform::run_hidden("shutdown.exe", &[flag, "/t", &delay, "/c", &comment])? {
                (0, _) => Ok(()),
                (code, _) => Err(format!("shutdown.exe: код {code}")),
            }
        }
        PowerAction::Cancel => match crate::platform::run_hidden("shutdown.exe", &["/a"])? {
            (0, _) | (ERROR_NO_SHUTDOWN_IN_PROGRESS, _) => Ok(()),
            (code, _) => Err(format!("shutdown.exe: код {code}")),
        },
    }
}
