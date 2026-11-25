//! Ollama local server (`127.0.0.1:11434/api/ps`): which models are loaded. No account, no limits.

use serde_json::Value;

pub fn parse_ps(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let names: Vec<&str> = v
        .get("models")?
        .as_array()?
        .iter()
        .filter_map(|m| m.get("name").or_else(|| m.get("model")).and_then(Value::as_str))
        .collect();
    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}
