//! UDP discovery: request `V4D?1` -> `{"name":"<PC>","port":<http port>,"ver":"1.0.0"}`, private networks only.

use serde_json::json;
use std::net::SocketAddr;

pub const QUERY: &str = "V4D?1";

/// Reply for a datagram, or None when it must be ignored.
pub fn reply(payload: &[u8], from: SocketAddr, name: &str, port: u16) -> Option<Vec<u8>> {
    if !crate::util::is_private(from.ip()) {
        return None;
    }
    let text = std::str::from_utf8(payload).ok()?.trim();
    if text != QUERY {
        return None;
    }
    Some(json!({"name": name, "port": port, "ver": crate::config::AGENT_VERSION}).to_string().into_bytes())
}

pub async fn run(udp_port: u16, http_port: u16) {
    let sock = match tokio::net::UdpSocket::bind(("0.0.0.0", udp_port)).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("discovery: cannot bind UDP {udp_port}: {e}");
            return;
        }
    };
    tracing::info!("discovery on UDP {udp_port}");
    let name = crate::util::host_name();
    let mut buf = [0u8; 64];
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((n, from)) => {
                if let Some(r) = reply(&buf[..n], from, &name, http_port) {
                    let _ = sock.send_to(&r, from).await;
                }
            }
            Err(e) => {
                // Windows reports ICMP port-unreachable of a previous send as an error here; keep going
                tracing::debug!("discovery recv: {e}");
            }
        }
    }
}
