//! Tray icon: «Открыть редактор», «Пауза», «Выход», and «Обновить до X.Y.Z» once the update check finds a newer
//! release. Runs the Win32 message loop on the calling thread.

use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

/// The exe's own icon (group id 1, embedded by build.rs from assets/icon/ven4docking.ico);
/// fallback (resource missing): a 32×32 teal square with a white "phone" drawn in code.
fn icon() -> Icon {
    #[cfg(windows)]
    if let Ok(i) = Icon::from_resource(1, None) {
        return i;
    }
    let n = 32usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let i = (y * n + x) * 4;
            let phone = (10..22).contains(&x) && (5..27).contains(&y);
            let screen = (12..20).contains(&x) && (8..22).contains(&y);
            let (r, g, b) = if phone && !screen { (255, 255, 255) } else { (0x00, 0xBC, 0xD4) };
            rgba[i..i + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    Icon::from_rgba(rgba, n as u32, n as u32).expect("icon")
}

fn open_editor() {
    let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else { return };
    let cands = [dir.join("Ven4Docking.exe"), dir.join("..").join("editor").join("Ven4Docking.exe")];
    match cands.iter().find(|p| p.exists()) {
        Some(p) => {
            if let Err(e) = std::process::Command::new(p).spawn() {
                tracing::warn!("cannot start editor: {e}");
            }
        }
        None => tracing::warn!("editor not found next to the agent"),
    }
}

/// Blocks until «Выход». Returns Err when the tray cannot be created (no shell).
#[cfg(windows)]
pub fn run(st: Arc<AppState>) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, SetTimer, TranslateMessage, MSG};
    let menu = Menu::new();
    let open = MenuItem::new("Открыть редактор", true, None);
    let pause = CheckMenuItem::new("Пауза", true, false, None);
    let exit = MenuItem::new("Выход", true, None);
    menu.append_items(&[&open, &pause, &PredefinedMenuItem::separator(), &exit]).map_err(|e| e.to_string())?;
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_tooltip(format!("Ven4Docking {} — порт {}", crate::config::AGENT_VERSION, st.port))
        .with_icon(icon())
        .build()
        .map_err(|e| e.to_string())?;
    let rx = MenuEvent::receiver();
    let update = MenuItem::new("", true, None);
    let mut update_shown: Option<String> = None;
    unsafe {
        // the loop also wakes up every 30 s to show a found update in the menu
        let _ = SetTimer(None, 0, 30_000, None);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            let avail = st.update.lock().unwrap().available().map(String::from);
            if avail.is_some() && avail != update_shown {
                let v = avail.clone().unwrap_or_default();
                update.set_text(format!("Обновить до {v}"));
                if update_shown.is_none() && menu.insert(&update, 0).is_err() {
                    tracing::warn!("tray: cannot add the update item");
                }
                update_shown = avail;
            }
            while let Ok(ev) = rx.try_recv() {
                if ev.id == update.id() {
                    crate::update::start_updater();
                } else if ev.id == open.id() {
                    open_editor();
                } else if ev.id == pause.id() {
                    let p = pause.is_checked();
                    st.paused.store(p, Ordering::Relaxed);
                    tracing::info!("paused: {p}");
                } else if ev.id == exit.id() {
                    tracing::info!("exit from tray");
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}
