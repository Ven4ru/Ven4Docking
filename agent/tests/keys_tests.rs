//! Key combinations, typed text and macros (through the recording `Actions`, nothing is pressed).

use serde_json::json;
use ven4docking_agent::actions::keys::{combo_events, is_extended, parse_combo, text_events, KeyEvent, TextEvent, VK_CONTROL, VK_LWIN, VK_MENU, VK_RETURN, VK_SHIFT, VK_TAB};
use ven4docking_agent::actions::{run_action, AudioOp, RecordActions};
use ven4docking_agent::config::{Action, App};

#[test]
fn combos_parse_case_insensitive() {
    let c = parse_combo("Ctrl+Shift+M").unwrap();
    assert_eq!((c.mods.clone(), c.key, c.text.as_str()), (vec![VK_CONTROL, VK_SHIFT], 0x4D, "Ctrl+Shift+M"));
    assert_eq!(parse_combo("ctrl + alt + f9").unwrap().text, "Ctrl+Alt+F9");
    assert_eq!(parse_combo("WIN+d").unwrap().mods, vec![VK_LWIN]);
    for (s, vk) in [
        ("F1", 0x70),
        ("F24", 0x87),
        ("A", 0x41),
        ("z", 0x5A),
        ("0", 0x30),
        ("9", 0x39),
        ("Enter", 0x0D),
        ("Esc", 0x1B),
        ("Tab", 0x09),
        ("Space", 0x20),
        ("Backspace", 0x08),
        ("Delete", 0x2E),
        ("Insert", 0x2D),
        ("Home", 0x24),
        ("End", 0x23),
        ("PageUp", 0x21),
        ("PageDown", 0x22),
        ("Up", 0x26),
        ("Down", 0x28),
        ("Left", 0x25),
        ("Right", 0x27),
        ("PrintScreen", 0x2C),
        ("Pause", 0x13),
        ("NumPad0", 0x60),
        ("numpad9", 0x69),
    ] {
        let c = parse_combo(s).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!((c.key, c.mods.len()), (vk, 0), "{s}");
    }
    assert_eq!(parse_combo("alt+numpad5").unwrap().text, "Alt+NumPad5");
}

#[test]
fn bad_combos_are_rejected() {
    for s in ["", "Ctrl", "Ctrl+Shift", "Ctrl+A+B", "Ctrl+Ctrl+A", "Ctrl++A", "A+", "F0", "F25", "F01", "NumPad10", "Hyper+A", "Ctrl+Ф", "AB", "Ctrl-A"] {
        assert!(parse_combo(s).is_err(), "{s:?} must not parse");
    }
}

#[test]
fn modifiers_are_released_in_reverse() {
    let c = parse_combo("Ctrl+Alt+Delete").unwrap();
    let k = |vk, up| KeyEvent { vk, up };
    assert_eq!(
        combo_events(&c),
        vec![k(VK_CONTROL, false), k(VK_MENU, false), k(0x2E, false), k(0x2E, true), k(VK_MENU, true), k(VK_CONTROL, true)]
    );
    assert!(is_extended(0x2E) && is_extended(0x26) && is_extended(VK_LWIN) && !is_extended(0x41) && !is_extended(VK_CONTROL));
}

#[test]
fn text_line_breaks_are_enter() {
    let u = |c: char| TextEvent::Unicode(c as u16);
    assert_eq!(text_events("Да\r\nok\n\tx\r"), vec![
        u('Д'),
        u('а'),
        TextEvent::Vk(VK_RETURN),
        u('o'),
        u('k'),
        TextEvent::Vk(VK_RETURN),
        TextEvent::Vk(VK_TAB),
        u('x'),
        TextEvent::Vk(VK_RETURN)
    ]);
    // outside the BMP: a surrogate pair
    assert_eq!(text_events("😀"), vec![TextEvent::Unicode(0xD83D), TextEvent::Unicode(0xDE00)]);
}

fn action(v: serde_json::Value) -> Action {
    serde_json::from_value(v).unwrap()
}

#[test]
fn actions_run_through_the_trait() {
    let apps: Vec<App> = vec![serde_json::from_value(json!({"id": "obs", "title": "OBS", "kind": "exe", "target": "obs64.exe"})).unwrap()];
    let rec = RecordActions::default();
    let m = action(json!({"id": "day", "kind": "macro", "steps": [{"launch": "obs"}, {"wait": 3000}, {"keys": "ctrl+alt+f9"}, {"text": "привет"}]}));
    run_action(&rec, &m, &apps).unwrap();
    run_action(&rec, &action(json!({"id": "k", "kind": "keys", "keys": "Ctrl+Shift+M"})), &apps).unwrap();
    run_action(&rec, &action(json!({"id": "t", "kind": "text", "text": "С уважением"})), &apps).unwrap();
    run_action(&rec, &action(json!({"id": "m", "kind": "mic"})), &apps).unwrap();
    run_action(&rec, &action(json!({"id": "o", "kind": "audioOut", "devices": ["a", "b"]})), &apps).unwrap();
    assert!(run_action(&rec, &action(json!({"id": "o2", "kind": "audioOut"})), &apps).is_err(), "no devices");
    assert_eq!(
        *rec.log.lock().unwrap(),
        vec![
            "launch:obs",
            "wait:3000",
            "keys:Ctrl+Alt+F9",
            "text:привет",
            "keys:Ctrl+Shift+M",
            "text:С уважением",
            &format!("audio:{}", AudioOp::MicToggle),
            &format!("audio:{}", AudioOp::NextOutput(vec!["a".into(), "b".into()])),
        ]
    );
}

#[test]
fn next_output_goes_round_connected_devices() {
    use ven4docking_agent::metrics::audio::next_output;
    let list: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
    let all = list.clone();
    assert_eq!(next_output(&list, Some("a"), &all).as_deref(), Some("b"));
    assert_eq!(next_output(&list, Some("c"), &all).as_deref(), Some("a"), "round");
    assert_eq!(next_output(&list, Some("x"), &all).as_deref(), Some("a"), "current outside the list -> first");
    assert_eq!(next_output(&list, None, &all).as_deref(), Some("a"));
    let without_b = vec!["a".to_string(), "c".to_string()];
    assert_eq!(next_output(&list, Some("a"), &without_b).as_deref(), Some("c"), "unplugged device is skipped");
    assert_eq!(next_output(&list, Some("a"), &["a".to_string()]), None, "nothing else connected");
    assert_eq!(next_output(&[], Some("a"), &all), None);
}
