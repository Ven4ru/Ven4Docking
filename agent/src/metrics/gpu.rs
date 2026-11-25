//! GPU: names, temperature and fan via D3DKMT (same source as Task Manager, no vendor driver SDK),
//! load and dedicated VRAM via PDH "GPU Engine" / "GPU Adapter Memory" (opened once).

use super::pdh::Pdh;
use crate::util::round1;
use serde_json::{json, Value};
use std::collections::HashMap;

struct Adapter {
    handle: u32,
    luid: String,
    name: String,
}

pub struct Gpu {
    adapters: Vec<Adapter>,
    pdh: Option<Pdh>,
}

/// "pid_1_luid_0x00000000_0x0000D1C2_phys_0_eng_0_engtype_3D" -> ("0x00000000_0x0000D1C2", "3D")
pub fn parse_engine_instance(name: &str) -> Option<(String, String)> {
    let luid = name.split("luid_").nth(1)?.split("_phys").next()?;
    let et = name.split("_engtype_").nth(1)?;
    Some((norm_luid(luid), et.to_string()))
}

/// "luid_0x00000000_0x0000D1C2_phys_0" -> "0x00000000_0x0000D1C2"
pub fn parse_memory_instance(name: &str) -> Option<String> {
    Some(norm_luid(name.split("luid_").nth(1)?.split("_phys").next()?))
}

fn norm_luid(s: &str) -> String {
    s.to_uppercase().replace("0X", "0x")
}

#[cfg(windows)]
fn enum_adapters() -> Vec<Adapter> {
    use windows::Wdk::Graphics::Direct3D::*;
    let mut out: Vec<Adapter> = Vec::new();
    unsafe {
        let mut e = D3DKMT_ENUMADAPTERS2::default();
        if D3DKMTEnumAdapters2(&mut e).is_err() || e.NumAdapters == 0 {
            return out;
        }
        let mut arr = vec![D3DKMT_ADAPTERINFO::default(); e.NumAdapters as usize];
        e.pAdapters = arr.as_mut_ptr();
        if D3DKMTEnumAdapters2(&mut e).is_err() {
            return out;
        }
        for a in arr.iter().take(e.NumAdapters as usize) {
            let mut reg = D3DKMT_ADAPTERREGISTRYINFO::default();
            if !query(a.hAdapter, KMTQAITYPE_ADAPTERREGISTRYINFO, &mut reg) {
                continue;
            }
            let end = reg.AdapterString.iter().position(|c| *c == 0).unwrap_or(reg.AdapterString.len());
            let name = String::from_utf16_lossy(&reg.AdapterString[..end]).trim().to_string();
            let luid = format!("0x{:08X}_0x{:08X}", a.AdapterLuid.HighPart as u32, a.AdapterLuid.LowPart);
            if name.is_empty() || name.to_lowercase().contains("basic render") || out.iter().any(|o| o.luid == luid) {
                continue;
            }
            out.push(Adapter { handle: a.hAdapter, luid, name });
        }
    }
    out
}

#[cfg(windows)]
unsafe fn query<T>(h: u32, typ: windows::Wdk::Graphics::Direct3D::KMTQUERYADAPTERINFOTYPE, data: &mut T) -> bool {
    use windows::Wdk::Graphics::Direct3D::*;
    let mut q = D3DKMT_QUERYADAPTERINFO {
        hAdapter: h,
        Type: typ,
        pPrivateDriverData: data as *mut T as *mut core::ffi::c_void,
        PrivateDriverDataSize: std::mem::size_of::<T>() as u32,
    };
    D3DKMTQueryAdapterInfo(&mut q).is_ok()
}

#[cfg(windows)]
fn perf(h: u32) -> (Option<f64>, Option<u32>) {
    use windows::Wdk::Graphics::Direct3D::*;
    let mut p = D3DKMT_ADAPTER_PERFDATA::default();
    if !unsafe { query(h, KMTQAITYPE_ADAPTERPERFDATA, &mut p) } {
        return (None, None);
    }
    let temp = (p.Temperature > 0 && p.Temperature < 2000).then(|| round1(p.Temperature as f64 / 10.0));
    let fan = (p.FanRPM > 0).then_some(p.FanRPM);
    (temp, fan)
}

impl Gpu {
    pub fn new() -> Gpu {
        let adapters = enum_adapters();
        let pdh = if adapters.is_empty() {
            None
        } else {
            Pdh::open(&[r"\GPU Engine(*)\Utilization Percentage", r"\GPU Adapter Memory(*)\Dedicated Usage"])
        };
        Gpu { adapters, pdh }
    }

    pub fn sample(&mut self) -> Option<Value> {
        if self.adapters.is_empty() {
            return None;
        }
        let mut eng: HashMap<String, HashMap<String, f64>> = HashMap::new();
        let mut vram: HashMap<String, f64> = HashMap::new();
        if let Some(p) = &self.pdh {
            p.collect();
            for (name, v) in p.array(0) {
                if let Some((luid, et)) = parse_engine_instance(&name) {
                    *eng.entry(luid).or_default().entry(et).or_default() += v;
                }
            }
            for (name, v) in p.array(1) {
                if let Some(luid) = parse_memory_instance(&name) {
                    *vram.entry(luid).or_default() += v;
                }
            }
        }
        let list: Vec<Value> = self
            .adapters
            .iter()
            .map(|a| {
                let load = eng.get(&a.luid).map(|e| e.values().cloned().fold(0.0, f64::max)).unwrap_or(0.0).min(100.0);
                let (temp, fan) = perf(a.handle);
                json!({
                    "id": a.luid, "name": a.name, "load": round1(load), "temp": temp,
                    "vramUsed": vram.get(&a.luid).map(|v| *v as u64), "fanRpm": fan,
                })
            })
            .collect();
        Some(Value::Array(list))
    }
}

impl Default for Gpu {
    fn default() -> Self {
        Gpu::new()
    }
}
