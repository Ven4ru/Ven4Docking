use ven4docking_agent::config::*;

#[test]
fn token_hash_is_sha256_hex() {
    // sha256("abc")
    assert_eq!(hash_token("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let (t, h) = new_token();
    assert_eq!(t.len(), 64);
    assert_eq!(hash_token(&t), h);
    let (t2, _) = new_token();
    assert_ne!(t, t2);
}

#[test]
fn phone_lookup_by_token_uses_hash_only() {
    let (tok, hash) = new_token();
    let mut cfg = Config::default();
    cfg.phones.push(serde_json::from_value(serde_json::json!({"id": "p1", "name": "realme", "tokenHash": hash})).unwrap());
    assert_eq!(cfg.phone_by_token(&tok).map(|p| p.id.as_str()), Some("p1"));
    assert!(cfg.phone_by_token("wrong").is_none());
    assert!(cfg.phone_by_token("").is_none());
    let text = serde_json::to_string(&cfg).unwrap();
    assert!(!text.contains(&tok), "raw token must never be stored");
    assert!(text.contains("\"tokenHash\""));
}

#[test]
fn parses_spec_example_and_keeps_unknown_fields() {
    let text = r#"{
      "version": 1, "port": 18765, "theme": "teal", "language": "ru",
      "weather": {"name": "Москва", "lat": 55.75, "lon": 37.62},
      "apps": [{"id": "telegram", "title": "Telegram", "kind": "shortcut", "target": "x.lnk", "args": "", "icon": "icons/telegram.png"}],
      "servers": [{"id": "site", "name": "Сайт", "host": "203.0.113.10", "port": 22, "user": "root", "key": "auto"}],
      "ai": {"enabled": ["claude-code", "codex"]},
      "phones": [{"id": "p1", "name": "realme C11", "tokenHash": "00", "night": {"from": "23:00", "to": "07:00", "brightness": 0.02}, "dimWhenPcOffline": true, "burnInShift": false, "editorColor": "red"}],
      "futureField": {"a": 1}
    }"#;
    let cfg = Config::parse(text).unwrap();
    assert_eq!(cfg.port, 18765);
    assert_eq!(cfg.servers[0].host, "203.0.113.10");
    assert!(!cfg.phones[0].burn_in_shift);
    let back = serde_json::to_value(&cfg).unwrap();
    assert_eq!(back["futureField"]["a"], 1);
    assert_eq!(back["phones"][0]["editorColor"], "red");
    assert_eq!(back["phones"][0]["dimWhenPcOffline"], true);
    assert!(back.get("servers_interval").is_none() && back.get("serversInterval").is_some(), "camelCase only");
}

#[test]
fn defaults_and_validation() {
    let cfg = Config::parse(r#"{"version":1}"#).unwrap();
    assert_eq!(cfg.port, 8765);
    assert_eq!(cfg.ai.enabled.len(), 5);
    assert!(Config::parse(r#"{"version":0}"#).is_err());
    assert!(Config::parse(r#"{"port":1}"#).is_err(), "version is required");
    assert!(Config::parse(r#"{"version":1,"apps":[{"id":"../x","kind":"exe"}]}"#).is_err());
    assert!(Config::parse(r#"{"version":1,"apps":[{"id":"a","kind":"rm"}]}"#).is_err());
    assert!(Config::parse(r#"{"version":1,"servers":[{"id":"a","host":""}]}"#).is_err());
    assert!(Config::parse("\u{feff}{\"version\":1}").is_ok(), "BOM tolerated");
}

#[test]
fn save_load_roundtrip_and_admin_key() {
    let dir = std::env::temp_dir().join(format!("v4d-cfg-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("config.json");
    let _ = std::fs::remove_file(&p);
    let cfg = Config::load_or_create(&p).unwrap();
    assert!(p.exists());
    assert_eq!(Config::load_or_create(&p).unwrap(), cfg);

    let k = dir.join("admin.key");
    let _ = std::fs::remove_file(&k);
    let key = load_or_create_admin_key(&k).unwrap();
    assert_eq!(key.len(), 64, "32 random bytes as hex");
    assert_eq!(load_or_create_admin_key(&k).unwrap(), key, "stable across restarts");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn editor_put_keeps_token_hashes() {
    let mut old = Config::default();
    old.phones.push(serde_json::from_value(serde_json::json!({"id": "p1", "tokenHash": "abc"})).unwrap());
    let mut new: Config = serde_json::from_value(serde_json::json!({"version": 1, "phones": [{"id": "p1", "name": "renamed"}]})).unwrap();
    new.merge_secrets_from(&old);
    assert_eq!(new.phones[0].token_hash, "abc");
    assert_eq!(new.phones[0].name, "renamed");
}

#[test]
fn ids_are_checked() {
    assert!(valid_id("p1") && valid_id("my-phone_2"));
    assert!(!valid_id("") && !valid_id("a/b") && !valid_id("..") && !valid_id("a b"));
}
