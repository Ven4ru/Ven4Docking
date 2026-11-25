//! Small helpers: private-network check, tiny localhost HTTP GET, host name.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Private networks allowed to talk to the agent (spec §3.3):
/// 10/8, 172.16/12, 192.168/16, 100.64/10, 127/8 (+ IPv6 loopback and IPv4-mapped forms).
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => {
            if v6.is_loopback() {
                return true;
            }
            match v6.to_ipv4_mapped() {
                Some(v4) => is_private_v4(v4),
                None => false,
            }
        }
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 10
        || o[0] == 127
        || (o[0] == 172 && (16..=31).contains(&o[1]))
        || (o[0] == 192 && o[1] == 168)
        || (o[0] == 100 && (64..=127).contains(&o[1]))
}

pub fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.to_ipv4_mapped().map(|v| v.is_loopback()).unwrap_or(false),
    }
}

/// GET http://127.0.0.1:<port><path>; body on HTTP 200, None otherwise. Only for local model servers.
pub fn http_get_local(port: u16, path: &str, timeout: Duration) -> Option<String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    s.set_write_timeout(Some(Duration::from_secs(1))).ok()?;
    let req = format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    s.take(2 * 1024 * 1024).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status = head.lines().next()?.split_whitespace().nth(1)?;
    if status != "200" {
        return None;
    }
    Some(body.to_string())
}

pub fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_else(|_| std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".into()))
}

pub fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
