//! Key combinations and typed text (spec 1.1 §3): "Ctrl+Shift+M" -> virtual keys, text -> Unicode key events,
//! both sent with SendInput. Modifiers are always released, even when sending stops half-way.

/// Parsed combination: modifiers (in the given order) and one key, as virtual-key codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combo {
    pub mods: Vec<u16>,
    pub key: u16,
    /// Normalised spelling ("Ctrl+Shift+M"), for logs.
    pub text: String,
}

/// One key event of a combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub vk: u16,
    pub up: bool,
}

/// One character of typed text: a UTF-16 unit sent as KEYEVENTF_UNICODE, or a real key (Enter, Tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEvent {
    Unicode(u16),
    Vk(u16),
}

pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_LWIN: u16 = 0x5B;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_TAB: u16 = 0x09;

/// Longest text of a `text` action or step (characters).
pub const MAX_TEXT: usize = 2000;

fn modifier(name: &str) -> Option<(u16, &'static str)> {
    Some(match name {
        "ctrl" => (VK_CONTROL, "Ctrl"),
        "alt" => (VK_MENU, "Alt"),
        "shift" => (VK_SHIFT, "Shift"),
        "win" => (VK_LWIN, "Win"),
        _ => return None,
    })
}

const NAMED: &[(&str, u16, &str)] = &[
    ("enter", 0x0D, "Enter"),
    ("esc", 0x1B, "Esc"),
    ("tab", 0x09, "Tab"),
    ("space", 0x20, "Space"),
    ("backspace", 0x08, "Backspace"),
    ("delete", 0x2E, "Delete"),
    ("insert", 0x2D, "Insert"),
    ("home", 0x24, "Home"),
    ("end", 0x23, "End"),
    ("pageup", 0x21, "PageUp"),
    ("pagedown", 0x22, "PageDown"),
    ("up", 0x26, "Up"),
    ("down", 0x28, "Down"),
    ("left", 0x25, "Left"),
    ("right", 0x27, "Right"),
    ("printscreen", 0x2C, "PrintScreen"),
    ("pause", 0x13, "Pause"),
];

/// `prefix` + number in lo..=hi without leading zeros ("f12", "numpad0").
fn numbered(name: &str, prefix: &str, lo: u16, hi: u16) -> Option<u16> {
    let d = name.strip_prefix(prefix)?;
    if d.is_empty() || d.len() > 2 || !d.bytes().all(|c| c.is_ascii_digit()) || (d.len() == 2 && d.starts_with('0')) {
        return None;
    }
    let n: u16 = d.parse().ok()?;
    (lo..=hi).contains(&n).then_some(n)
}

/// Key name (lower case) -> (virtual key, normalised name).
fn key(name: &str) -> Option<(u16, String)> {
    if let Some((_, vk, n)) = NAMED.iter().find(|(k, _, _)| *k == name) {
        return Some((*vk, n.to_string()));
    }
    let b = name.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit()) {
        return Some((b[0].to_ascii_uppercase() as u16, name.to_ascii_uppercase()));
    }
    if let Some(n) = numbered(name, "numpad", 0, 9) {
        return Some((0x60 + n, format!("NumPad{n}")));
    }
    if let Some(n) = numbered(name, "f", 1, 24) {
        return Some((0x70 + n - 1, format!("F{n}")));
    }
    None
}

/// "Ctrl+Shift+M": modifiers Ctrl, Alt, Shift, Win and exactly one key, case-insensitive, separator `+`.
pub fn parse_combo(s: &str) -> Result<Combo, String> {
    if s.trim().is_empty() {
        return Err("empty key combination".into());
    }
    let mut mods: Vec<u16> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut main: Option<(u16, String)> = None;
    for part in s.split('+') {
        let p = part.trim().to_ascii_lowercase();
        if p.is_empty() {
            return Err(format!("bad key combination: {s}"));
        }
        if let Some((vk, n)) = modifier(&p) {
            if mods.contains(&vk) {
                return Err(format!("repeated modifier in {s}"));
            }
            mods.push(vk);
            names.push(n.to_string());
        } else if let Some(k) = key(&p) {
            if main.is_some() {
                return Err(format!("more than one key in {s}"));
            }
            main = Some(k);
        } else {
            return Err(format!("unknown key '{}' in {s}", part.trim()));
        }
    }
    let Some((vk, n)) = main else { return Err(format!("no key in {s}")) };
    names.push(n);
    Ok(Combo { mods, key: vk, text: names.join("+") })
}

/// Press order: modifiers down, key down, key up, modifiers up in reverse.
pub fn combo_events(c: &Combo) -> Vec<KeyEvent> {
    let mut v: Vec<KeyEvent> = c.mods.iter().map(|m| KeyEvent { vk: *m, up: false }).collect();
    v.push(KeyEvent { vk: c.key, up: false });
    v.push(KeyEvent { vk: c.key, up: true });
    v.extend(c.mods.iter().rev().map(|m| KeyEvent { vk: *m, up: true }));
    v
}

/// Text -> key events: line breaks (\n, \r\n, \r) are Enter, tab is Tab, the rest goes as UTF-16 units.
pub fn text_events(s: &str) -> Vec<TextEvent> {
    let mut out = Vec::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(TextEvent::Vk(VK_RETURN));
            }
            '\n' => out.push(TextEvent::Vk(VK_RETURN)),
            '\t' => out.push(TextEvent::Vk(VK_TAB)),
            _ => {
                let mut buf = [0u16; 2];
                out.extend(c.encode_utf16(&mut buf).iter().map(|u| TextEvent::Unicode(*u)));
            }
        }
    }
    out
}

/// Keys that need KEYEVENTF_EXTENDEDKEY (the E0-prefixed ones).
pub fn is_extended(vk: u16) -> bool {
    matches!(vk, 0x21..=0x28 | 0x2C | 0x2D | 0x2E | VK_LWIN)
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, VIRTUAL_KEY,
    };

    fn input(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        }
    }

    fn vk_input(e: KeyEvent) -> INPUT {
        let scan = unsafe { MapVirtualKeyW(e.vk as u32, MAPVK_VK_TO_VSC) } as u16;
        let mut f = KEYBD_EVENT_FLAGS(0);
        if is_extended(e.vk) {
            f |= KEYEVENTF_EXTENDEDKEY;
        }
        if e.up {
            f |= KEYEVENTF_KEYUP;
        }
        input(e.vk, scan, f)
    }

    fn send(list: &[INPUT]) -> bool {
        list.is_empty() || unsafe { SendInput(list, std::mem::size_of::<INPUT>() as i32) } as usize == list.len()
    }

    pub fn send_combo(c: &Combo) -> Result<(), String> {
        let ev = combo_events(c);
        let downs = c.mods.len() + 1;
        let down: Vec<INPUT> = ev[..downs].iter().map(|e| vk_input(*e)).collect();
        let up: Vec<INPUT> = ev[downs..].iter().map(|e| vk_input(*e)).collect();
        let pressed = send(&down);
        // always release, also when the press did not go through
        let released = send(&up);
        if pressed && released {
            Ok(())
        } else {
            Err("Windows не приняла нажатие (окно от имени администратора?)".into())
        }
    }

    pub fn send_text(s: &str) -> Result<(), String> {
        let mut list = Vec::new();
        for e in text_events(s) {
            match e {
                TextEvent::Unicode(u) => {
                    list.push(input(0, u, KEYEVENTF_UNICODE));
                    list.push(input(0, u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
                }
                TextEvent::Vk(vk) => {
                    list.push(vk_input(KeyEvent { vk, up: false }));
                    list.push(vk_input(KeyEvent { vk, up: true }));
                }
            }
        }
        // small portions: some programs lose characters from one huge batch
        for (i, chunk) in list.chunks(64).enumerate() {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if !send(chunk) {
                return Err("Windows не приняла ввод текста (окно от имени администратора?)".into());
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use win::{send_combo, send_text};
