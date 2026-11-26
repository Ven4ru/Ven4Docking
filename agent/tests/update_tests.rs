//! Update check without network: versions from release tags and «show the notification once per version».

use ven4docking_agent::update::{is_newer, load_notified, parse_version, should_notify, tag_from_release, tag_version, Notified, UpdateState};

#[test]
fn versions_from_tags() {
    assert_eq!(parse_version("v1.1.0"), Some(([1, 1, 0], None)));
    assert_eq!(parse_version("1.1.0-beta"), Some(([1, 1, 0], Some("beta".into()))));
    assert_eq!(parse_version("V2.0"), Some(([2, 0, 0], None)));
    for bad in ["", "v", "latest", "1.x.0", "1.2.3.4", "1..2", "1.1.0-", "-beta"] {
        assert_eq!(parse_version(bad), None, "{bad:?}");
    }
    assert_eq!(tag_version("v1.1.0"), "1.1.0");
    assert_eq!(tag_version("1.1.0-beta"), "1.1.0-beta");
}

#[test]
fn newer_follows_semver() {
    assert!(is_newer("v1.1.0", "1.0.0"));
    assert!(is_newer("1.0.10", "1.0.9"), "numbers, not text");
    assert!(is_newer("1.1.0-beta", "1.0.0"));
    assert!(!is_newer("1.1.0-beta", "1.1.0"), "a pre-release is older than its release");
    assert!(is_newer("1.1.0", "1.1.0-beta"));
    assert!(is_newer("1.1.0-rc", "1.1.0-beta"));
    assert!(!is_newer("v1.0.0", "1.0.0"));
    assert!(!is_newer("0.9.9", "1.0.0"));
    assert!(!is_newer("garbage", "1.0.0"));
}

#[test]
fn notification_once_per_version() {
    let none = Notified::default();
    assert!(should_notify("1.1.0", "1.0.0", &none));
    assert!(!should_notify("1.0.0", "1.0.0", &none), "not newer than the agent");
    let shown = Notified { notified: Some("1.1.0".into()) };
    assert!(!should_notify("1.1.0", "1.0.0", &shown), "already shown");
    assert!(should_notify("1.2.0", "1.0.0", &shown), "a newer one is shown again");
    assert!(!should_notify("1.1.0-beta", "1.0.0", &shown));

    let dir = std::env::temp_dir().join(format!("v4d-upd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("update.json");
    assert_eq!(load_notified(&f), none, "no file yet");
    std::fs::write(&f, serde_json::to_string(&shown).unwrap()).unwrap();
    assert_eq!(load_notified(&f), shown);
    std::fs::write(&f, "{broken").unwrap();
    assert_eq!(load_notified(&f), none, "a broken file means nothing was shown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn release_json_and_state() {
    let body = br#"{"tag_name": "v1.1.0", "name": "Ven4Docking 1.1.0", "assets": []}"#;
    assert_eq!(tag_from_release(body).as_deref(), Some("v1.1.0"));
    assert_eq!(tag_from_release(br#"{"message": "API rate limit exceeded"}"#), None);
    assert_eq!(tag_from_release(br#"{"tag_name": "nightly"}"#), None);
    assert_eq!(tag_from_release(b"<html>"), None);

    let mut u = UpdateState { latest: Some("999.0.0".into()), checked: Some(1.0) };
    assert_eq!(u.available(), Some("999.0.0"));
    u.latest = Some(ven4docking_agent::config::AGENT_VERSION.into());
    assert_eq!(u.available(), None, "same version: nothing to offer");
}

#[test]
fn update_check_is_on_by_default() {
    let cfg = ven4docking_agent::config::Config::parse(r#"{"version": 1}"#).unwrap();
    assert!(cfg.update_check);
    let off = ven4docking_agent::config::Config::parse(r#"{"version": 1, "updateCheck": false}"#).unwrap();
    assert!(!off.update_check);
}
