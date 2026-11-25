//! RAM (sysinfo).

use crate::util::round1;
use serde_json::{json, Value};
use sysinfo::{MemoryRefreshKind, System};

pub struct Mem {
    sys: System,
}

impl Mem {
    pub fn new() -> Mem {
        Mem { sys: System::new().expect("sysinfo") }
    }

    pub fn sample(&mut self) -> Value {
        self.sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        let total = self.sys.total_memory();
        let used = total.saturating_sub(self.sys.available_memory());
        let pct = if total > 0 { round1(100.0 * used as f64 / total as f64) } else { 0.0 };
        json!({"total": total, "used": used, "pct": pct})
    }
}

impl Default for Mem {
    fn default() -> Self {
        Mem::new()
    }
}
