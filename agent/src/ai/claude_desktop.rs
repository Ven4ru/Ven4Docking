//! Claude Desktop: `%APPDATA%\Claude\plan-usage-history.json` written by the app itself.
//! Format: `{"version":2,"samples":[{"t":<ms>,"u":{"fh":<5h %>,"sd":<7d %>}}]}`; no reset times.

use super::{make_window, Reading, FIVE_H, WEEK};
use serde_json::Value;

/// `week_anchor` — Claude Code's `seven_day.resets_at` (same account), if known: weekly resets keep
/// the same weekday and hour, so the last reset before `now` can be computed.
pub fn parse(text: &str, now: f64, week_anchor: Option<f64>) -> Option<Reading> {
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let samples = v.get("samples")?.as_array()?;
    let x = samples
        .iter()
        .filter(|s| s.get("t").and_then(Value::as_f64).is_some())
        .max_by(|a, b| a["t"].as_f64().unwrap_or(0.0).total_cmp(&b["t"].as_f64().unwrap_or(0.0)))?;
    let t = x["t"].as_f64()? / 1000.0;
    let u = x.get("u")?;
    let mut windows = Vec::new();
    if let Some(fh) = u.get("fh").and_then(Value::as_f64) {
        windows.push(make_window("fiveHour", "5 часов", fh, None, t, FIVE_H, now));
    }
    if let Some(sd) = u.get("sd").and_then(Value::as_f64) {
        let mut w = make_window("sevenDay", "7 дней", sd, None, t, WEEK, now);
        if let Some(anchor) = week_anchor {
            let last_reset = anchor + WEEK * ((now - anchor) / WEEK).floor();
            if t < last_reset {
                w.stale = true;
            }
            w.resets_at = Some(last_reset + WEEK);
        }
        windows.push(w);
    }
    if windows.is_empty() {
        return None;
    }
    Some(Reading { t, windows, week_resets_at: None })
}
