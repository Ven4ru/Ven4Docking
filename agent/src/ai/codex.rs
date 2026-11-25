//! Codex CLI: `token_count` events with `rate_limits` in `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
//! (written by Codex itself). The newest event with non-null `rate_limits` wins.

use super::{make_window, parse_iso8601, window_label, Reading};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn window(id_default: &str, w: &Value, t: f64, now: f64) -> Option<super::Window> {
    let pct = w.get("used_percent").and_then(Value::as_f64)?;
    let minutes = w.get("window_minutes").and_then(Value::as_i64).unwrap_or(0);
    let resets_at = w
        .get("resets_at")
        .and_then(Value::as_f64)
        .or_else(|| w.get("resets_in_seconds").and_then(Value::as_f64).map(|s| t + s));
    let id = match minutes {
        300 => "fiveHour",
        10080 => "sevenDay",
        _ => id_default,
    };
    let label = if minutes > 0 { window_label(minutes) } else { id_default.to_string() };
    let span = if minutes > 0 { minutes as f64 * 60.0 } else { f64::INFINITY };
    Some(make_window(id, &label, pct, resets_at, t, span, now))
}

/// Parse a rollout file (or its tail). Incomplete lines are ignored.
pub fn parse_rollout(text: &str, now: f64) -> Option<Reading> {
    for line in text.lines().rev() {
        if !line.contains("token_count") || !line.contains("rate_limits") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let p = &v["payload"];
        if p["type"] != "token_count" {
            continue;
        }
        let rl = &p["rate_limits"];
        if !rl.is_object() {
            continue;
        }
        let t = v.get("timestamp").and_then(Value::as_str).and_then(parse_iso8601).unwrap_or(now);
        let windows: Vec<_> = [("primary", &rl["primary"]), ("secondary", &rl["secondary"])]
            .into_iter()
            .filter_map(|(id, w)| window(id, w, t, now))
            .collect();
        if windows.is_empty() {
            continue;
        }
        return Some(Reading { t, windows, week_resets_at: None });
    }
    None
}

fn sorted_dirs(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(p)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    v.sort();
    v.reverse();
    v
}

/// Up to `limit` newest rollout files (by mtime), walking YYYY/MM/DD folders from the newest, at most 14 days back.
pub fn newest_rollouts(sessions: &Path, limit: usize) -> Vec<PathBuf> {
    let mut files: Vec<(f64, PathBuf)> = Vec::new();
    let mut days_seen = 0;
    'outer: for y in sorted_dirs(sessions) {
        for m in sorted_dirs(&y) {
            for d in sorted_dirs(&m) {
                days_seen += 1;
                if let Ok(rd) = std::fs::read_dir(&d) {
                    for e in rd.filter_map(|e| e.ok()) {
                        let p = e.path();
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                            if let Some(t) = super::mtime(&p) {
                                files.push((t, p));
                            }
                        }
                    }
                }
                if files.len() >= limit || days_seen >= 14 {
                    break 'outer;
                }
            }
        }
    }
    files.sort_by(|a, b| b.0.total_cmp(&a.0));
    files.into_iter().take(limit).map(|(_, p)| p).collect()
}
