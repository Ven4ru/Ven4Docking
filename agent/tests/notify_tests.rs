//! Notification filter by app modes (on a fixed list, Windows is not read) and wake-on-LAN helpers.

use serde_json::json;
use std::collections::HashSet;
use ven4docking_agent::config::NotifyCfg;
use ven4docking_agent::notify::{filter, icon_file, unix_from_universal, RawNote};

fn note(id: u32, aumid: &str, time: f64) -> RawNote {
    RawNote { id, aumid: aumid.into(), app_name: format!("{aumid} app"), time, title: format!("title {id}"), text: format!("text {id}") }
}

fn cfg(v: serde_json::Value) -> NotifyCfg {
    serde_json::from_value(v).unwrap()
}

fn notes() -> Vec<RawNote> {
    vec![note(1, "TelegramDesktop", 100.0), note(2, "Discord", 110.0), note(3, "Outlook", 120.0), note(4, "Outlook", 130.0), note(5, "Spam", 140.0), note(6, "TelegramDesktop", 150.0)]
}

#[test]
fn modes_decide_what_the_panel_sees() {
    let c = cfg(json!({"enabled": true, "apps": {"TelegramDesktop": "title", "Discord": "full", "Outlook": "count"}}));
    let items = filter(&notes(), &c, &HashSet::new(), &|a| (a == "Discord").then(|| "/icons/d.png".to_string()));
    let v: Vec<_> = items.iter().map(|i| i.value.clone()).collect();
    assert_eq!(
        v,
        vec![
            json!({"id": "6", "app": "TelegramDesktop", "appName": "TelegramDesktop app", "time": 150.0, "count": 1, "title": "title 6"}),
            json!({"id": "4", "app": "Outlook", "appName": "Outlook app", "time": 130.0, "count": 2}),
            json!({"id": "2", "app": "Discord", "appName": "Discord app", "time": 110.0, "count": 1, "icon": "/icons/d.png", "title": "title 2", "text": "text 2"}),
            json!({"id": "1", "app": "TelegramDesktop", "appName": "TelegramDesktop app", "time": 100.0, "count": 1, "title": "title 1"}),
        ],
        "newest first, unlisted apps hidden, count folds the app, title/text by mode"
    );
    assert_eq!(items[1].ids, vec![4, 3], "folded item covers both notifications");
}

#[test]
fn wall_counts_only_and_dismissed() {
    let c = cfg(json!({"enabled": true, "wallCountsOnly": true, "apps": {"TelegramDesktop": "full", "Discord": "full"}}));
    let items = filter(&notes(), &c, &HashSet::new(), &|_| None);
    let v: Vec<_> = items.iter().map(|i| i.value.clone()).collect();
    assert_eq!(v, vec![
        json!({"id": "6", "app": "TelegramDesktop", "appName": "TelegramDesktop app", "time": 150.0, "count": 2}),
        json!({"id": "2", "app": "Discord", "appName": "Discord app", "time": 110.0, "count": 1}),
    ]);
    for i in &items {
        assert!(i.value.get("title").is_none() && i.value.get("text").is_none(), "no text on the wall");
    }
    let dismissed: HashSet<u32> = [6, 2].into_iter().collect();
    let items = filter(&notes(), &c, &dismissed, &|_| None);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].value["id"], "1");
    assert_eq!(items[0].value["count"], 1);
}

#[test]
fn empty_app_list_shows_nothing() {
    let c = cfg(json!({"enabled": true}));
    assert!(filter(&notes(), &c, &HashSet::new(), &|_| None).is_empty());
    assert!(c.night_flash && !c.wall_counts_only, "defaults");
}

#[test]
fn helpers() {
    let f = icon_file("Microsoft.Office.OUTLOOK.EXE.15");
    assert!(f.starts_with("notify-") && f.ends_with(".png") && f.len() == "notify-".len() + 16 + 4, "{f}");
    assert_eq!(f, icon_file("Microsoft.Office.OUTLOOK.EXE.15"), "stable");
    assert_ne!(f, icon_file("Discord"));
    // 2026-11-26 00:00:00 UTC
    assert_eq!(unix_from_universal(1_764_115_200 * 10_000_000 + 116_444_736_000_000_000), 1_764_115_200.0);
}

#[test]
fn magic_packet_and_mac() {
    use ven4docking_agent::wol::{broadcast, format_mac, magic_packet, parse_mac};
    let mac = parse_mac("AA:BB:CC:DD:EE:0f").unwrap();
    assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x0F]);
    assert_eq!(parse_mac("aa-bb-cc-dd-ee-0f"), Some(mac));
    assert_eq!(format_mac(&mac), "AA:BB:CC:DD:EE:0F");
    for bad in ["", "AA:BB:CC:DD:EE", "AA:BB:CC:DD:EE:FF:00", "AAB:B:CC:DD:EE:FF", "GG:BB:CC:DD:EE:FF", "AABBCCDDEEFF"] {
        assert!(parse_mac(bad).is_none(), "{bad}");
    }
    let p = magic_packet(mac);
    assert_eq!(p.len(), 102);
    assert_eq!(&p[..6], &[0xFF; 6]);
    for i in 0..16 {
        assert_eq!(&p[6 + i * 6..12 + i * 6], &mac, "repeat {i}");
    }
    assert_eq!(broadcast("192.168.1.20".parse().unwrap(), 24).to_string(), "192.168.1.255");
    assert_eq!(broadcast("10.1.2.3".parse().unwrap(), 8).to_string(), "10.255.255.255");
    assert_eq!(broadcast("172.16.5.4".parse().unwrap(), 20).to_string(), "172.16.15.255");
    assert_eq!(broadcast("192.168.1.20".parse().unwrap(), 32).to_string(), "192.168.1.20");
}
