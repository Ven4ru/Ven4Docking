//! Claude Code: JSON that Claude Code passes to its documented status-line command (`rate_limits`),
//! dumped to a file by the user's status-line script. Accepts the full status-line JSON or just `rate_limits`.

use super::{make_window, Reading, FIVE_H, WEEK};
use serde_json::Value;

fn window_meta(key: &str) -> (String, String, f64) {
    match key {
        "five_hour" => ("fiveHour".into(), "5 часов".into(), FIVE_H),
        "seven_day" => ("sevenDay".into(), "7 дней".into(), WEEK),
        "seven_day_opus" => ("sevenDayOpus".into(), "7 дней (Opus)".into(), WEEK),
        "seven_day_sonnet" => ("sevenDaySonnet".into(), "7 дней (Sonnet)".into(), WEEK),
        other => {
            let mut id = String::new();
            let mut up = false;
            for c in other.chars() {
                if c == '_' {
                    up = true;
                } else if up {
                    id.extend(c.to_uppercase());
                    up = false;
                } else {
                    id.push(c);
                }
            }
            let span = if other.starts_with("five_hour") { FIVE_H } else { WEEK };
            (id, other.replace('_', " "), span)
        }
    }
}

/// `t` = file modification time (time of the reading).
pub fn parse(text: &str, t: f64, now: f64) -> Option<Reading> {
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let rl = v.get("rate_limits").filter(|x| x.is_object()).unwrap_or(&v);
    let obj = rl.as_object()?;
    let mut keys: Vec<&String> = obj.keys().collect();
    // stable order: five_hour, seven_day, then the rest alphabetically
    keys.sort_by_key(|k| match k.as_str() {
        "five_hour" => (0, String::new()),
        "seven_day" => (1, String::new()),
        o => (2, o.to_string()),
    });
    let mut windows = Vec::new();
    let mut week = None;
    for k in keys {
        let w = &obj[k];
        let Some(pct) = w.get("used_percentage").and_then(Value::as_f64) else { continue };
        let resets_at = w.get("resets_at").and_then(Value::as_f64);
        if k == "seven_day" {
            week = resets_at;
        }
        let (id, label, span) = window_meta(k);
        windows.push(make_window(&id, &label, pct, resets_at, t, span, now));
    }
    if windows.is_empty() {
        return None;
    }
    Some(Reading { t, windows, week_resets_at: week })
}
