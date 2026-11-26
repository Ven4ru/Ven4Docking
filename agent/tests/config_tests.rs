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
fn editor_put_does_not_add_or_drop_phones() {
    // agent: p1 (renamed by nobody), p3 paired after the editor read the config; p2 was revoked meanwhile
    let old: Config = serde_json::from_value(serde_json::json!({"version": 1, "phones": [
        {"id": "p1", "name": "Kitchen", "tokenHash": "h1"},
        {"id": "p3", "name": "Desk", "tokenHash": "h3"}
    ]})).unwrap();
    // editor saves its stale copy: p1 renamed, p2 still listed, p3 unknown to it
    let mut new: Config = serde_json::from_value(serde_json::json!({"version": 1, "phones": [
        {"id": "p1", "name": "Hall"},
        {"id": "p2", "name": "Old"}
    ]})).unwrap();
    new.merge_secrets_from(&old);
    let ids: Vec<&str> = new.phones.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["p1", "p3"]);
    assert_eq!(new.phones[0].name, "Hall");
    assert_eq!(new.phones[0].token_hash, "h1");
    assert_eq!(new.phones[1].name, "Desk");
    assert_eq!(new.phones[1].token_hash, "h3");
}

#[test]
fn static_dirs_prefer_installed_layout() {
    let root = std::env::temp_dir().join(format!("v4d-static-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let exe_dir = root.join("Ven4Docking");
    std::fs::create_dir_all(&exe_dir).unwrap();

    // nothing built yet -> installed layout
    let (panel, apk) = default_static_dirs(&exe_dir);
    assert_eq!(panel, exe_dir.join("panel"));
    assert_eq!(apk, exe_dir.join("apk"));

    // side-by-side dev layout: <exe>\..\panel\dist, <exe>\..\apk
    std::fs::create_dir_all(root.join("panel").join("dist")).unwrap();
    std::fs::write(root.join("panel").join("dist").join("index.html"), "<html>").unwrap();
    std::fs::create_dir_all(root.join("apk")).unwrap();
    let (panel, apk) = default_static_dirs(&exe_dir);
    assert_eq!(panel, exe_dir.join("..").join("panel").join("dist"));
    assert_eq!(apk, exe_dir.join("..").join("apk"));

    // installed layout wins when present
    std::fs::create_dir_all(exe_dir.join("panel")).unwrap();
    std::fs::write(exe_dir.join("panel").join("index.html"), "<html>").unwrap();
    std::fs::create_dir_all(exe_dir.join("apk")).unwrap();
    let (panel, apk) = default_static_dirs(&exe_dir);
    assert_eq!(panel, exe_dir.join("panel"));
    assert_eq!(apk, exe_dir.join("apk"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn phone_ids_are_never_reused() {
    let phone = |id: &str| -> Phone { serde_json::from_value(serde_json::json!({"id": id, "tokenHash": "h"})).unwrap() };

    // fresh config: p1, p2, p3; revoking p2 and p3 does not give their ids back
    let mut cfg = Config::default();
    for want in ["p1", "p2", "p3"] {
        let id = cfg.alloc_phone_id::<&str>(&[]);
        assert_eq!(id, want);
        cfg.phones.push(phone(&id));
    }
    cfg.phones.retain(|p| p.id == "p1");
    assert_eq!(cfg.alloc_phone_id::<&str>(&[]), "p4");
    assert_eq!(cfg.next_phone_id, Some(5));

    // the counter survives save/load
    let text = serde_json::to_string(&cfg).unwrap();
    assert!(text.contains("\"nextPhoneId\":5"), "{text}");
    let mut back = Config::parse(&text).unwrap();
    assert_eq!(back.alloc_phone_id::<&str>(&[]), "p5");

    // config from before the counter: starts after the highest id in use, ids of other shapes ignored
    let mut old: Config = serde_json::from_value(serde_json::json!({"version": 1, "phones": [{"id": "p3"}, {"id": "kitchen"}, {"id": "p1"}]})).unwrap();
    assert_eq!(old.next_phone_id, None);
    assert_eq!(old.alloc_phone_id::<&str>(&[]), "p4");
    // ... and after layout files of phones revoked before the counter existed
    let mut old: Config = serde_json::from_value(serde_json::json!({"version": 1, "phones": [{"id": "p1"}]})).unwrap();
    assert_eq!(old.alloc_phone_id(&["p1", "p7", "notes"]), "p8");

    // the editor PUTs a config it read before the last pairing: the counter does not go back
    let mut stale: Config = serde_json::from_value(serde_json::json!({"version": 1, "nextPhoneId": 2, "phones": [{"id": "p1"}]})).unwrap();
    stale.merge_secrets_from(&cfg);
    assert_eq!(stale.next_phone_id, Some(5));
    let mut none: Config = serde_json::from_value(serde_json::json!({"version": 1, "phones": [{"id": "p1"}]})).unwrap();
    none.merge_secrets_from(&cfg);
    assert_eq!(none.alloc_phone_id::<&str>(&[]), "p5");

    assert_eq!(phone_id_number("p12"), Some(12));
    assert_eq!(phone_id_number("p"), None);
    assert_eq!(phone_id_number("p1x"), None);
    assert_eq!(phone_id_number("x1"), None);
}

#[test]
fn ids_are_checked() {
    assert!(valid_id("p1") && valid_id("my-phone_2"));
    assert!(!valid_id("") && !valid_id("a/b") && !valid_id("..") && !valid_id("a b"));
}

// ---------- 1.1: actions, obs, notify, wol ----------

#[test]
fn parses_1_1_spec_example() {
    let text = r#"{
      "version": 1,
      "apps": [{"id": "obs", "title": "OBS", "kind": "exe", "target": "obs64.exe"}],
      "actions": [
        {"id": "mute-discord", "title": "Discord: микрофон", "icon": null, "kind": "keys", "keys": "Ctrl+Shift+M"},
        {"id": "sign", "title": "Подпись", "kind": "text", "text": "С уважением, …"},
        {"id": "stream-start", "title": "Начать день", "kind": "macro", "steps": [
          {"launch": "obs"}, {"wait": 3000}, {"keys": "Ctrl+Alt+F9"}]},
        {"id": "mic", "title": "Микрофон", "kind": "mic"},
        {"id": "out", "title": "Наушники / колонки", "kind": "audioOut", "devices": ["{0.0.0.00000000}.{a}", "{0.0.0.00000000}.{b}"]}
      ],
      "obs": {"host": "127.0.0.1", "port": 4455, "passwordDpapi": "AQAAANCMnd8B"},
      "notify": {"enabled": false, "wallCountsOnly": false, "nightFlash": true,
                 "apps": {"TelegramDesktop": "title", "Discord": "full", "Microsoft.Office.OUTLOOK.EXE.15": "count"}},
      "wol": {"mac": "AA:BB:CC:DD:EE:FF", "broadcast": "192.168.1.255"}
    }"#;
    let cfg = Config::parse(text).unwrap();
    assert_eq!(cfg.actions.len(), 5);
    assert_eq!(cfg.actions[2].steps[1].wait, Some(3000));
    assert_eq!(cfg.obs.as_ref().unwrap().port, 4455);
    assert_eq!(cfg.notify.as_ref().unwrap().apps["Discord"], "full");
    let back = serde_json::to_value(&cfg).unwrap();
    assert_eq!(back["actions"][2]["steps"][0], serde_json::json!({"launch": "obs"}), "a step keeps only its field");
    assert_eq!(back["obs"]["passwordDpapi"], "AQAAANCMnd8B");
    assert_eq!(back["wol"]["broadcast"], "192.168.1.255");
    assert_eq!(Config::parse(&back.to_string()).unwrap(), cfg, "round trip");
    // 1.0 configs stay as they were
    let plain = serde_json::to_value(Config::parse(r#"{"version":1}"#).unwrap()).unwrap();
    for k in ["actions", "obs", "notify", "wol"] {
        assert!(plain.get(k).is_none(), "{k} not written when absent");
    }
}

#[test]
fn bad_1_1_fields_are_rejected() {
    let base = |extra: &str| format!(r#"{{"version": 1, "apps": [{{"id": "obs", "kind": "exe", "target": "obs64.exe"}}], {extra}}}"#);
    let bad = [
        r#""actions": [{"id": "a b", "kind": "mic"}]"#,
        r#""actions": [{"id": "a", "kind": "mic"}, {"id": "a", "kind": "mic"}]"#,
        r#""actions": [{"id": "a", "kind": "shell"}]"#,
        r#""actions": [{"id": "a", "kind": "keys", "keys": "Ctrl+Shift"}]"#,
        r#""actions": [{"id": "a", "kind": "keys"}]"#,
        r#""actions": [{"id": "a", "kind": "macro", "steps": []}]"#,
        r#""actions": [{"id": "a", "kind": "macro", "steps": [{"wait": 0}]}]"#,
        r#""actions": [{"id": "a", "kind": "macro", "steps": [{"wait": 10001}]}]"#,
        r#""actions": [{"id": "a", "kind": "macro", "steps": [{"launch": "nope"}]}]"#,
        r#""actions": [{"id": "a", "kind": "macro", "steps": [{"keys": "Ctrl+A", "wait": 5}]}]"#,
        r#""actions": [{"id": "a", "kind": "macro", "steps": [{}]}]"#,
        r#""actions": [{"id": "a", "kind": "macro", "steps": [{"keys": "Hyper+A"}]}]"#,
        r#""notify": {"apps": {"Discord": "everything"}}"#,
        r#""wol": {"mac": "AA:BB", "broadcast": "192.168.1.255"}"#,
        r#""wol": {"mac": "AA:BB:CC:DD:EE:FF", "broadcast": "nowhere"}"#,
        r#""obs": {"host": "8.8.8.8"}"#,
        r#""obs": {"port": 0}"#,
        r#""obs": {"passwordDpapi": "%%%"}"#,
    ];
    for b in bad {
        assert!(Config::parse(&base(b)).is_err(), "must fail: {b}");
    }
    let long = "я".repeat(2001);
    assert!(Config::parse(&base(&format!(r#""actions": [{{"id": "a", "kind": "text", "text": "{long}"}}]"#))).is_err());
    let ok_text = "я".repeat(2000);
    assert!(Config::parse(&base(&format!(r#""actions": [{{"id": "a", "kind": "text", "text": "{ok_text}"}}]"#))).is_ok(), "2000 characters is fine");
    let steps: Vec<String> = (0..21).map(|_| r#"{"wait": 1}"#.to_string()).collect();
    assert!(Config::parse(&base(&format!(r#""actions": [{{"id": "a", "kind": "macro", "steps": [{}]}}]"#, steps.join(",")))).is_err());
    let steps: Vec<String> = (0..20).map(|_| r#"{"wait": 1}"#.to_string()).collect();
    assert!(Config::parse(&base(&format!(r#""actions": [{{"id": "a", "kind": "macro", "steps": [{}]}}]"#, steps.join(",")))).is_ok());
    // an action may share its id with an app
    assert!(Config::parse(&base(r#""actions": [{"id": "obs", "kind": "mic"}]"#)).is_ok());
    for host in ["localhost", "127.0.0.1", "192.168.1.10", "::1"] {
        assert!(Config::parse(&base(&format!(r#""obs": {{"host": "{host}"}}"#))).is_ok(), "{host}");
    }
}

#[test]
fn editor_put_keeps_obs_password() {
    let old: Config = serde_json::from_value(serde_json::json!({"version": 1, "obs": {"port": 4455, "passwordDpapi": "c2VjcmV0"}})).unwrap();
    let mut new: Config = serde_json::from_value(serde_json::json!({"version": 1, "obs": {"port": 4456}})).unwrap();
    new.merge_secrets_from(&old);
    assert_eq!(new.obs.as_ref().unwrap().password_dpapi, "c2VjcmV0", "empty from the editor = unchanged");
    assert_eq!(new.obs.as_ref().unwrap().port, 4456);
    let mut changed: Config = serde_json::from_value(serde_json::json!({"version": 1, "obs": {"passwordDpapi": "bmV3"}})).unwrap();
    changed.merge_secrets_from(&old);
    assert_eq!(changed.obs.unwrap().password_dpapi, "bmV3");
    let mut removed: Config = serde_json::from_value(serde_json::json!({"version": 1})).unwrap();
    removed.merge_secrets_from(&old);
    assert!(removed.obs.is_none(), "OBS settings removed in the editor stay removed");
}
