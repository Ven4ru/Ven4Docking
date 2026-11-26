//! Wake-on-LAN (spec 1.1 §6): the agent only tells the phone the MAC and broadcast address of the adapter with
//! the default route; the magic packet is sent by the phone. `magic_packet` is the reference for the launcher.

use serde::Serialize;
use std::net::Ipv4Addr;

/// "AA:BB:CC:DD:EE:FF" or "aa-bb-cc-dd-ee-ff".
pub fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = s.trim().split([':', '-']).collect();
    if parts.len() != 6 {
        return None;
    }
    let mut mac = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        if p.len() != 2 {
            return None;
        }
        mac[i] = u8::from_str_radix(p, 16).ok()?;
    }
    Some(mac)
}

pub fn format_mac(mac: &[u8]) -> String {
    mac.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

/// 6 × FF, then the MAC 16 times.
pub fn magic_packet(mac: [u8; 6]) -> [u8; 102] {
    let mut p = [0xFFu8; 102];
    for i in 0..16 {
        p[6 + i * 6..12 + i * 6].copy_from_slice(&mac);
    }
    p
}

/// Directed broadcast of a network: 192.168.1.20/24 -> 192.168.1.255.
pub fn broadcast(ip: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix.min(32) as u32) };
    Ipv4Addr::from(u32::from(ip) | !mask)
}

/// What `/admin/wol` reports and `/api/v1/layout` falls back to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Adapter {
    pub mac: String,
    pub broadcast: String,
    /// "Ethernet" (connection name)
    pub adapter: String,
    /// device name ("Realtek PCIe GbE Family Controller")
    #[serde(skip)]
    pub description: String,
}

/// Adapter that carries IPv4 traffic to the internet (GetBestInterface) and its first IPv4 address.
#[cfg(windows)]
pub fn detect() -> Option<Adapter> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GetBestInterface, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    unsafe {
        let mut idx = 0u32;
        // any public address works: only the route matters
        if GetBestInterface(u32::from_ne_bytes([8, 8, 8, 8]), &mut idx) != 0 {
            return None;
        }
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        let mut size = 16 * 1024u32;
        let mut buf: Vec<u64> = Vec::new();
        for _ in 0..3 {
            buf = vec![0u64; size as usize / 8 + 1];
            let r = GetAdaptersAddresses(AF_INET.0 as u32, flags, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size);
            if r == 0 {
                break;
            }
            if r != 111 {
                // anything but ERROR_BUFFER_OVERFLOW
                return None;
            }
        }
        let mut a = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !a.is_null() {
            let ad = &*a;
            if ad.Anonymous1.Anonymous.IfIndex == idx && ad.PhysicalAddressLength == 6 {
                let mut ip = None;
                let mut u = ad.FirstUnicastAddress;
                while !u.is_null() && ip.is_none() {
                    let sa = (*u).Address.lpSockaddr;
                    if !sa.is_null() && (*sa).sa_family == AF_INET {
                        let sin = &*(sa as *const SOCKADDR_IN);
                        let o = sin.sin_addr.S_un.S_addr.to_ne_bytes();
                        ip = Some((Ipv4Addr::new(o[0], o[1], o[2], o[3]), (*u).OnLinkPrefixLength));
                    }
                    u = (*u).Next;
                }
                let (ip, prefix) = ip?;
                return Some(Adapter {
                    mac: format_mac(&ad.PhysicalAddress[..6]),
                    broadcast: broadcast(ip, prefix).to_string(),
                    adapter: ad.FriendlyName.to_string().unwrap_or_default(),
                    description: ad.Description.to_string().unwrap_or_default(),
                });
            }
            a = ad.Next;
        }
        None
    }
}

#[cfg(not(windows))]
pub fn detect() -> Option<Adapter> {
    None
}

/// Is the adapter allowed to wake the PC (`powercfg /devicequery wake_armed`)? None when unknown.
#[cfg(windows)]
pub fn wake_enabled(description: &str) -> Option<bool> {
    // the tool prints in the OEM code page: only ASCII names can be compared reliably
    if description.is_empty() || !description.is_ascii() {
        return None;
    }
    let (code, out) = crate::platform::run_hidden("powercfg.exe", &["/devicequery", "wake_armed"]).ok()?;
    if code != 0 {
        return None;
    }
    let d = description.trim().to_ascii_lowercase();
    Some(out.lines().any(|l| l.trim().to_ascii_lowercase() == d))
}

#[cfg(not(windows))]
pub fn wake_enabled(_description: &str) -> Option<bool> {
    None
}
