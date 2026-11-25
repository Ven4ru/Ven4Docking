//! Disk fill (sysinfo).

use crate::util::round1;
use serde_json::{json, Value};
use sysinfo::Disks as SDisks;

pub struct Disks {
    disks: SDisks,
}

impl Disks {
    pub fn new() -> Disks {
        Disks { disks: SDisks::new_with_refreshed_list().expect("sysinfo") }
    }

    pub fn sample(&mut self) -> Value {
        self.disks.refresh(true);
        let mut v: Vec<Value> = self
            .disks
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| {
                let mount = d.mount_point().to_string_lossy().trim_end_matches('\\').to_string();
                let total = d.total_space();
                let used = total.saturating_sub(d.available_space());
                json!({"mount": mount, "total": total, "used": used, "pct": round1(100.0 * used as f64 / total as f64)})
            })
            .collect();
        v.sort_by(|a, b| a["mount"].as_str().cmp(&b["mount"].as_str()));
        v.dedup_by(|a, b| a["mount"] == b["mount"]);
        Value::Array(v)
    }
}

impl Default for Disks {
    fn default() -> Self {
        Disks::new()
    }
}
