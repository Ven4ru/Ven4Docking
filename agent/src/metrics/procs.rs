//! Busiest process (sysinfo), for the net block's `topProcess`. Refreshed every 5 s, only while a panel is active.

use crate::util::round1;
use serde_json::{json, Value};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

pub struct Procs {
    sys: System,
    ncpu: f64,
    primed: bool,
}

impl Procs {
    pub fn new(ncpu: usize) -> Procs {
        Procs { sys: System::new().expect("sysinfo"), ncpu: ncpu.max(1) as f64, primed: false }
    }

    pub fn sample(&mut self) -> Option<Value> {
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cpu());
        if !self.primed {
            self.primed = true;
            return None; // cpu usage needs two samples
        }
        let me = std::process::id();
        self.sys
            .processes()
            .iter()
            .filter(|(pid, p)| pid.as_u32() != 0 && pid.as_u32() != me && !is_idle(&p.name().to_string_lossy()))
            .max_by(|a, b| a.1.cpu_usage().total_cmp(&b.1.cpu_usage()))
            .map(|(_, p)| {
                let name = p.name().to_string_lossy();
                let name = name.strip_suffix(".exe").unwrap_or(&name).to_string();
                json!({"name": name, "cpu": round1(p.cpu_usage() as f64 / self.ncpu)})
            })
    }
}

fn is_idle(n: &str) -> bool {
    n == "System Idle Process" || n == "Idle"
}
