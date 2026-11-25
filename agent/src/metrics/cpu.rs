//! CPU: load per core, frequency (sysinfo), board temperature (PDH Thermal Zone),
//! CPU temperature from LibreHardwareMonitor's web server if the user runs it.

use super::pdh::Pdh;
use crate::util::round1;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, System};

pub struct Cpu {
    sys: System,
    thermal: Option<Pdh>,
    thermal_tried: bool,
    lhm_url_port: Option<(u16, String)>,
    lhm_next: Instant,
    lhm_temp: Option<f64>,
}

impl Cpu {
    pub fn new(lhm_url: Option<&str>) -> Cpu {
        let mut sys = System::new().expect("sysinfo");
        sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_usage().with_frequency());
        Cpu { sys, thermal: None, thermal_tried: false, lhm_url_port: lhm_url.and_then(parse_local_url), lhm_next: Instant::now(), lhm_temp: None }
    }

    pub fn ncpu(&self) -> usize {
        self.sys.cpus().len().max(1)
    }

    pub fn sample(&mut self) -> Value {
        self.sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_usage().with_frequency());
        let cores: Vec<f64> = self.sys.cpus().iter().map(|c| c.usage().round() as f64).collect();
        let load = round1(self.sys.cpus().iter().map(|c| c.usage() as f64).sum::<f64>() / self.ncpu() as f64);
        let freq = self.sys.cpus().iter().map(|c| c.frequency()).max().filter(|f| *f > 0);
        if !self.thermal_tried {
            self.thermal_tried = true;
            self.thermal = Pdh::open(&[r"\Thermal Zone Information(*)\Temperature"]);
        }
        let board = self.thermal.as_ref().and_then(|p| {
            p.collect();
            p.array(0).into_iter().map(|(_, v)| v).filter(|v| *v > 200.0).map(|v| v - 273.15).reduce(f64::max).map(round1)
        });
        json!({"load": load, "cores": cores, "freqMhz": freq, "temp": self.lhm(), "boardTemp": board})
    }

    /// LibreHardwareMonitor data.json (only 127.0.0.1); every 5 s, 60 s back-off when it is not running.
    fn lhm(&mut self) -> Option<f64> {
        let (port, path) = self.lhm_url_port.clone()?;
        if Instant::now() < self.lhm_next {
            return self.lhm_temp;
        }
        match crate::util::http_get_local(port, &path, Duration::from_millis(300)) {
            Some(body) => {
                self.lhm_temp = serde_json::from_str::<Value>(&body).ok().and_then(|v| lhm_cpu_temp(&v));
                self.lhm_next = Instant::now() + Duration::from_secs(5);
            }
            None => {
                self.lhm_temp = None;
                self.lhm_next = Instant::now() + Duration::from_secs(60);
            }
        }
        self.lhm_temp
    }
}

/// "http://127.0.0.1:8085/data.json" -> (8085, "/data.json"); only loopback addresses are accepted.
fn parse_local_url(u: &str) -> Option<(u16, String)> {
    let rest = u.strip_prefix("http://")?;
    let (hostport, path) = rest.split_once('/').map(|(a, b)| (a, format!("/{b}"))).unwrap_or((rest, "/".into()));
    let (host, port) = hostport.split_once(':').unwrap_or((hostport, "80"));
    if host != "127.0.0.1" && host != "localhost" {
        return None;
    }
    Some((port.parse().ok()?, path))
}

/// Max of CPU "Package"/"Tctl"/"Core Max" temperatures in the LHM sensor tree.
pub fn lhm_cpu_temp(tree: &Value) -> Option<f64> {
    fn walk(n: &Value, path: &mut Vec<String>, best: &mut Vec<f64>) {
        let text = n.get("Text").and_then(Value::as_str).unwrap_or("");
        let val = n.get("Value").and_then(Value::as_str).unwrap_or("");
        if n.get("Type").and_then(Value::as_str) == Some("Temperature") || val.ends_with("°C") {
            let p = path.join(" / ").to_lowercase();
            let t = text.to_lowercase();
            if let Ok(v) = val.replace("°C", "").replace(',', ".").trim().parse::<f64>() {
                if (p.contains("cpu") || p.contains("intel") || p.contains("amd ryzen"))
                    && (t.contains("package") || t.contains("tctl") || t.contains("core max"))
                {
                    best.push(v);
                }
            }
        }
        if let Some(ch) = n.get("Children").and_then(Value::as_array) {
            path.push(text.to_string());
            for c in ch {
                walk(c, path, best);
            }
            path.pop();
        }
    }
    let mut best = Vec::new();
    walk(tree, &mut Vec::new(), &mut best);
    best.into_iter().reduce(f64::max)
}
