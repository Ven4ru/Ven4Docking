//! LM Studio local server (`127.0.0.1:1234/api/v0/models`): which model is loaded. No account, no limits.

use serde_json::Value;

pub fn parse_models(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let names: Vec<&str> = v
        .get("data")?
        .as_array()?
        .iter()
        .filter(|m| m.get("state").and_then(Value::as_str) == Some("loaded"))
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .collect();
    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}
