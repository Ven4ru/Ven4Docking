use std::path::PathBuf;
use ven4docking_agent::ai::{self, claude_code, claude_desktop, codex, lmstudio, ollama, AiReader, AiRoots};

const FULL: &str = include_str!("fixtures/ai/claude_statusline_full.json");
const DIRECT: &str = include_str!("fixtures/ai/claude_rate_limits.json");
const HISTORY: &str = include_str!("fixtures/ai/plan-usage-history.json");
const ROLLOUT: &str = include_str!("fixtures/ai/rollout-2026-11-22T07-10-00-00000000-0000-0000-0000-000000000002.jsonl");
const ROLLOUT_OLD: &str = include_str!("fixtures/ai/rollout-old-format.jsonl");

const NOW: f64 = 1795572000.0;
const MTIME: f64 = 1795571806.9;

#[test]
fn iso_time_parser() {
    assert_eq!(ai::parse_iso8601("1970-01-01T00:00:00Z"), Some(0.0));
    assert_eq!(ai::parse_iso8601("2026-11-22T07:15:02.500Z"), Some(1795331702.5));
    assert_eq!(ai::parse_iso8601("2026-11-22T10:15:02+03:00"), Some(1795331702.0));
    assert_eq!(ai::parse_iso8601("not a date"), None);
}

#[test]
fn claude_code_full_status_line_json() {
    let r = claude_code::parse(FULL, MTIME, NOW).unwrap();
    assert_eq!(r.t, MTIME);
    assert_eq!(r.windows.len(), 2);
    let fh = &r.windows[0];
    assert_eq!((fh.id.as_str(), fh.label.as_str()), ("fiveHour", "5 часов"));
    assert_eq!(fh.pct, 15.0);
    assert_eq!(fh.resets_at, Some(1795600800.0));
    assert!(!fh.stale);
    let sd = &r.windows[1];
    assert_eq!((sd.id.as_str(), sd.pct), ("sevenDay", 40.5));
    assert_eq!(r.week_resets_at, Some(1795981200.0));
    let v = serde_json::to_value(fh).unwrap();
    assert!(v.get("resetsAt").is_some() && v.get("resets_at").is_none());
}

#[test]
fn claude_code_window_goes_stale_after_reset_or_age() {
    // reset time passed
    let r = claude_code::parse(FULL, MTIME, 1795600800.0).unwrap();
    assert!(r.windows[0].stale);
    assert!(!r.windows[1].stale);
    // reading older than the window itself
    let r = claude_code::parse(FULL, MTIME - 6.0 * 3600.0, NOW).unwrap();
    assert!(r.windows[0].stale, "5h window, reading 6h old");
    assert!(!r.windows[1].stale);
}

#[test]
fn claude_code_direct_rate_limits_file() {
    let r = claude_code::parse(DIRECT, 1795300000.0, 1795381300.0).unwrap();
    assert_eq!(r.windows.len(), 1);
    assert_eq!(r.windows[0].id, "sevenDay");
    assert_eq!(r.windows[0].pct, 77.0);
    assert!(r.windows[0].stale, "resets_at already passed");
    assert!(claude_code::parse("{}", 0.0, NOW).is_none());
    assert!(claude_code::parse("garbage", 0.0, NOW).is_none());
}

#[test]
fn claude_desktop_latest_sample_and_week_anchor() {
    let r = claude_desktop::parse(HISTORY, NOW, None).unwrap();
    assert_eq!(r.t, 1795571806.9, "newest sample by t, not by position");
    assert_eq!(r.windows[0].id, "fiveHour");
    assert_eq!(r.windows[0].pct, 15.0);
    assert_eq!(r.windows[0].resets_at, None);
    assert_eq!(r.windows[1].pct, 41.0);
    assert!(!r.windows[0].stale && !r.windows[1].stale);

    // weekly reset anchored by Claude Code's seven_day.resets_at (same account)
    let anchor = 1795981200.0;
    let r = claude_desktop::parse(HISTORY, NOW, Some(anchor)).unwrap();
    assert_eq!(r.windows[1].resets_at, Some(anchor));
    assert!(!r.windows[1].stale);

    // a weekly reset happened after the sample -> stale
    let r = claude_desktop::parse(HISTORY, NOW, Some(1795571900.0)).unwrap();
    assert!(r.windows[1].stale);

    // 6 hours later the 5h window is stale
    let r = claude_desktop::parse(HISTORY, NOW + 6.0 * 3600.0, None).unwrap();
    assert!(r.windows[0].stale);
    assert!(claude_desktop::parse(r#"{"version":2,"samples":[]}"#, NOW, None).is_none());
}

#[test]
fn codex_rollout_last_rate_limits_wins_and_partial_line_ignored() {
    let r = codex::parse_rollout(ROLLOUT, NOW).unwrap();
    assert_eq!(r.t, 1795331702.5);
    assert_eq!(r.windows.len(), 1);
    let w = &r.windows[0];
    assert_eq!(w.id, "primary");
    assert_eq!(w.label, "30 дней");
    assert_eq!(w.pct, 36.0);
    assert_eq!(w.resets_at, Some(1795918502.0));
    assert!(!w.stale);
    // after reset
    let r = codex::parse_rollout(ROLLOUT, 1795918502.0).unwrap();
    assert!(r.windows[0].stale);
}

#[test]
fn codex_old_format_resets_in_seconds_and_over_100() {
    let t = 1795331702.0;
    let r = codex::parse_rollout(ROLLOUT_OLD, t + 60.0).unwrap();
    assert_eq!(r.windows.len(), 2);
    assert_eq!((r.windows[0].id.as_str(), r.windows[0].label.as_str()), ("fiveHour", "5 часов"));
    assert_eq!(r.windows[0].pct, 112.5, "may exceed 100, panel clips");
    assert_eq!(r.windows[0].resets_at, Some(t + 3600.0));
    assert_eq!((r.windows[1].id.as_str(), r.windows[1].label.as_str()), ("sevenDay", "7 дней"));
    assert!(codex::parse_rollout("{\"type\":\"session_meta\"}\n", NOW).is_none());
}

#[test]
fn local_model_apis() {
    assert_eq!(ollama::parse_ps(include_str!("fixtures/ai/ollama_ps.json")), Some("gemma3:12b".to_string()));
    assert_eq!(ollama::parse_ps(r#"{"models":[]}"#), None);
    assert_eq!(lmstudio::parse_models(include_str!("fixtures/ai/lmstudio_models.json")), Some("qwen2.5-7b-instruct".to_string()));
    assert_eq!(lmstudio::parse_models(r#"{"data":[]}"#), None);
}

#[test]
fn reader_collects_sources_from_files_only() {
    let root = std::env::temp_dir().join(format!("v4d-ai-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let profile = root.join("profile");
    let appdata = root.join("appdata");
    let local = root.join("local");
    std::fs::create_dir_all(profile.join(".claude")).unwrap();
    std::fs::create_dir_all(appdata.join("Claude")).unwrap();
    let day = profile.join(".codex").join("sessions").join("2026").join("11").join("22");
    std::fs::create_dir_all(&day).unwrap();
    std::fs::create_dir_all(&local).unwrap();
    std::fs::write(profile.join(".claude").join("rate_limits.json"), FULL).unwrap();
    std::fs::write(appdata.join("Claude").join("plan-usage-history.json"), HISTORY).unwrap();
    std::fs::write(day.join("rollout-2026-11-22T07-10-00-x.jsonl"), ROLLOUT).unwrap();

    let roots = AiRoots { profile: profile.clone(), appdata: appdata.clone(), local_appdata: local.clone() };
    let enabled = vec!["claude-code".to_string(), "claude-desktop".to_string(), "codex".to_string()];
    let mut r = AiReader::new(roots, enabled, None, None);
    let out = r.poll(NOW);
    let ids: Vec<_> = out.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["claude-code", "claude-desktop", "codex"]);
    assert_eq!(out[0].name, "Claude Code");
    assert!(out[0].t.is_some());
    assert_eq!(out[2].windows[0].pct, 36.0);
    let v = serde_json::to_value(&out[0]).unwrap();
    assert!(v.get("model").is_some(), "model key always present");

    let srcs = r.sources();
    assert!(srcs.iter().filter(|s| ["claude-code", "claude-desktop", "codex"].contains(&s.id.as_str())).all(|s| s.installed));
    assert_eq!(srcs.len(), 5, "editor sees every known source");
    assert!(srcs.iter().any(|s| s.id == "codex" && s.last_data == Some(1795331702.5)));

    // disabled source disappears
    let mut r = AiReader::new(
        AiRoots { profile, appdata, local_appdata: local },
        vec!["codex".to_string()],
        None,
        None,
    );
    assert_eq!(r.poll(NOW).len(), 1);
    let _ = std::fs::remove_dir_all(&root);
    let _ = PathBuf::new();
}
