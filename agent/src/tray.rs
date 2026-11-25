//! Tray icon: «Открыть редактор», «Пауза», «Выход». Runs the Win32 message loop on the calling thread.

use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

/// 32×32 teal circle with a dark "phone" in it — placeholder until the real icon (spec §14).
fn icon() -> Icon {
    let n = 32usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            let i = (y * n + x) * 4;
            if dx * dx + dy * dy <= 15.5 * 15.5 {
                let phone = (10..22).contains(&x) && (6..26).contains(&y);
                let (r, g, b) = if phone { (20, 24, 35) } else { (0x2A, 0xB3, 0xA6) };
                rgba[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
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
    use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage, MSG};
    let menu = Menu::new();
    let open = MenuItem::new("Открыть редактор", true, None);
    let pause = CheckMenuItem::new("Пауза", true, false, None);
    let exit = MenuItem::new("Выход", true, None);
    menu.append_items(&[&open, &pause, &PredefinedMenuItem::separator(), &exit]).map_err(|e| e.to_string())?;
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(format!("Ven4Docking {} — порт {}", crate::config::AGENT_VERSION, st.port))
        .with_icon(icon())
        .build()
        .map_err(|e| e.to_string())?;
    let rx = MenuEvent::receiver();
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            while let Ok(ev) = rx.try_recv() {
                if ev.id == open.id() {
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
