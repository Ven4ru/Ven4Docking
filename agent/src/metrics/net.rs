//! Network speed, bytes/s (sysinfo). Virtual adapters that mirror physical traffic are skipped.

use crate::util::round1;
use std::time::Instant;
use sysinfo::Networks;

pub struct Net {
    nets: Networks,
    t: Instant,
}

/// Loopback, Hyper-V vEthernet, tunnels — they duplicate or never carry real LAN/WAN traffic.
pub fn skip_iface(name: &str) -> bool {
    let n = name.to_lowercase();
    ["loopback", "vethernet", "isatap", "teredo", "6to4", "tailscale", "wireguard", "pseudo"].iter().any(|s| n.contains(s))
}

impl Net {
    pub fn new() -> Net {
        Net { nets: Networks::new_with_refreshed_list().expect("sysinfo"), t: Instant::now() }
    }

    /// (down, up) bytes per second since the previous call.
    pub fn sample(&mut self) -> (f64, f64) {
        self.nets.refresh(true);
        let dt = self.t.elapsed().as_secs_f64().max(0.2);
        self.t = Instant::now();
        let (mut down, mut up) = (0u64, 0u64);
        for (name, d) in self.nets.iter() {
            if skip_iface(&name.to_string_lossy()) {
                continue;
            }
            down += d.received();
            up += d.transmitted();
        }
        (round1(down as f64 / dt), round1(up as f64 / dt))
    }
}

impl Default for Net {
    fn default() -> Self {
        Net::new()
    }
}
