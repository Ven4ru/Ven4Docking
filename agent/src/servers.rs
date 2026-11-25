//! Servers over SSH inside the agent (russh): one long-lived session per server prints /proc numbers
//! every N seconds (prototype `ServerWatch`). Host keys are checked strictly against
//! `%USERPROFILE%\.ssh\known_hosts`; ping is ICMP (IcmpSendEcho), TCP connect time as a fallback.

use crate::config::Server;
use serde::Serialize;
use serde_json::{json, Value};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------- parsing

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerMetrics {
    pub cpu: Option<f64>,
    pub cores: u32,
    pub load: Option<[f64; 3]>,
    pub uptime: Option<f64>,
    pub ram_total: Option<u64>,
    pub ram_used: Option<u64>,
    pub ram_pct: Option<f64>,
    pub disk_total: Option<u64>,
    pub disk_used: Option<u64>,
    pub disk_pct: Option<f64>,
}

/// Line-by-line parser of the remote loop output (`N`, `B`, `cpu …`, `L`, `U`, `Mem…`, `D`, `E`).
#[derive(Debug, Default)]
pub struct ProcParser {
    ncpu: u32,
    prev: Option<Vec<u64>>,
    stat: Option<Vec<u64>>,
    block: ServerMetrics,
    mem_avail: Option<u64>,
}

fn kb(line: &str) -> Option<u64> {
    line.split_whitespace().nth(1)?.parse::<u64>().ok().map(|v| v * 1024)
}

impl ProcParser {
    /// Feed one line; returns metrics when a block ends (`E`).
    pub fn feed(&mut self, line: &str) -> Option<ServerMetrics> {
        let line = line.trim();
        if let Some(n) = line.strip_prefix("N ") {
            if let Ok(n) = n.trim().parse::<u32>() {
                self.ncpu = n.max(1);
            }
        } else if line == "B" {
            self.block = ServerMetrics::default();
            self.stat = None;
            self.mem_avail = None;
        } else if let Some(rest) = line.strip_prefix("cpu ") {
            let v: Result<Vec<u64>, _> = rest.split_whitespace().map(|x| x.parse::<u64>()).collect();
            self.stat = v.ok().filter(|v| v.len() >= 4);
        } else if let Some(rest) = line.strip_prefix("L ") {
            let v: Vec<f64> = rest.split_whitespace().take(3).filter_map(|x| x.parse().ok()).collect();
            if v.len() == 3 {
                self.block.load = Some([v[0], v[1], v[2]]);
            }
        } else if let Some(rest) = line.strip_prefix("U ") {
            self.block.uptime = rest.split_whitespace().next().and_then(|x| x.parse().ok());
        } else if line.starts_with("MemTotal:") {
            self.block.ram_total = kb(line);
        } else if line.starts_with("MemAvailable:") {
            self.mem_avail = kb(line);
        } else if let Some(rest) = line.strip_prefix("D ") {
            // D fs 1024-blocks used avail cap% mount
            let f: Vec<&str> = rest.split_whitespace().collect();
            if f.len() >= 5 {
                if let (Ok(t), Ok(u), Ok(p)) =
                    (f[1].parse::<u64>(), f[2].parse::<u64>(), f[4].trim_end_matches('%').parse::<f64>())
                {
                    self.block.disk_total = Some(t * 1024);
                    self.block.disk_used = Some(u * 1024);
                    self.block.disk_pct = Some(p);
                }
            }
        } else if line == "E" {
            let mut m = std::mem::take(&mut self.block);
            m.cores = self.ncpu.max(1);
            if let (Some(st), Some(prev)) = (&self.stat, &self.prev) {
                let idle = |v: &Vec<u64>| v[3] + v.get(4).copied().unwrap_or(0);
                let d_idle = idle(st).saturating_sub(idle(prev)) as f64;
                let tot = st.iter().sum::<u64>().saturating_sub(prev.iter().sum::<u64>()) as f64;
                m.cpu = Some(if tot > 0.0 { crate::util::round1((100.0 * (1.0 - d_idle / tot)).clamp(0.0, 100.0)) } else { 0.0 });
            }
            if self.stat.is_some() {
                self.prev = self.stat.take();
            }
            if let Some(total) = m.ram_total.filter(|t| *t > 0) {
                let used = total.saturating_sub(self.mem_avail.unwrap_or(0));
                m.ram_used = Some(used);
                m.ram_pct = Some(crate::util::round1(100.0 * used as f64 / total as f64));
            }
            return Some(m);
        }
        None
    }
}

/// Remote loop, same as the prototype.
pub fn remote_command(interval: u64) -> String {
    format!(
        "echo N $(nproc 2>/dev/null || grep -c ^processor /proc/cpuinfo); \
         while :; do echo B; head -1 /proc/stat; echo L $(cat /proc/loadavg); echo U $(cat /proc/uptime); \
         grep -E '^(MemTotal|MemAvailable):' /proc/meminfo; echo D $(df -P / | tail -1); echo E; \
         sleep {}; done",
        interval.max(5)
    )
}

/// Human-readable reason (editor «Проверить подключение» and panel errors).
pub fn explain_ssh_error(err: &str) -> String {
    let e = err.to_lowercase();
    if e.is_empty() {
        return "сессия оборвалась".into();
    }
    if e.contains("timed out") || e.contains("timeout") {
        return "SSH: таймаут".into();
    }
    if e.contains("refused") || e.contains("10061") {
        return "SSH: порт закрыт".into();
    }
    if e.contains("host key changed") || e.contains("keychanged") {
        return "SSH: ключ хоста не совпал".into();
    }
    if e.contains("unknown host key") {
        return "SSH: неизвестный ключ хоста (нет в known_hosts)".into();
    }
    if e.contains("auth rejected") || e.contains("permission denied") {
        return "SSH: ключ не принят".into();
    }
    if e.contains("no key") {
        return "SSH: нет ключа в %USERPROFILE%\\.ssh".into();
    }
    if e.contains("encrypted") {
        return "SSH: ключ защищён паролем".into();
    }
    if e.contains("resolve") || e.contains("no such host") || e.contains("11001") {
        return "SSH: имя не найдено".into();
    }
    let mut s: String = err.chars().take(60).collect();
    s.insert_str(0, "SSH: ");
    s
}

// ---------------------------------------------------------------------------------------- ping

/// ICMP echo through iphlpapi (no admin rights, no console windows). Blocking; run off the async runtime.
#[cfg(windows)]
pub fn icmp_ping(ip: std::net::Ipv4Addr, timeout_ms: u32) -> Option<u32> {
    use windows::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY};
    unsafe {
        let h = IcmpCreateFile().ok()?;
        let data = *b"ven4docking";
        let mut buf = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 64];
        let addr = u32::from_ne_bytes(ip.octets());
        let n = IcmpSendEcho(
            h,
            addr,
            data.as_ptr() as *const _,
            data.len() as u16,
            None,
            buf.as_mut_ptr() as *mut _,
            buf.len() as u32,
            timeout_ms,
        );
        let _ = IcmpCloseHandle(h);
        if n == 0 {
            return None;
        }
        let r = std::ptr::read_unaligned(buf.as_ptr() as *const ICMP_ECHO_REPLY);
        (r.Status == 0).then(|| r.RoundTripTime.max(1))
    }
}

#[cfg(not(windows))]
pub fn icmp_ping(_ip: std::net::Ipv4Addr, _timeout_ms: u32) -> Option<u32> {
    None
}

pub async fn tcp_ping(addr: SocketAddr, timeout: Duration) -> Option<u32> {
    let t = Instant::now();
    match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(addr)).await {
        Ok(Ok(_)) => Some((t.elapsed().as_millis() as u32).max(1)),
        _ => None,
    }
}

async fn resolve(host: &str, port: u16) -> Option<SocketAddr> {
    let mut it = tokio::net::lookup_host((host, port)).await.ok()?;
    let all: Vec<SocketAddr> = it.by_ref().collect();
    all.iter().find(|a| a.is_ipv4()).or(all.first()).copied()
}

/// (ms, kind) — kind is "icmp" or "tcp".
pub async fn ping(addr: SocketAddr) -> (Option<u32>, Option<&'static str>) {
    if let IpAddr::V4(v4) = addr.ip() {
        if let Ok(Some(ms)) = tokio::task::spawn_blocking(move || icmp_ping(v4, 1500)).await {
            return (Some(ms), Some("icmp"));
        }
    }
    match tcp_ping(addr, Duration::from_secs(2)).await {
        Some(ms) => (Some(ms), Some("tcp")),
        None => (None, None),
    }
}

// ---------------------------------------------------------------------------------------- ssh

struct Client {
    host: String,
    port: u16,
    known_hosts: PathBuf,
    host_err: Arc<Mutex<Option<String>>>,
}

#[async_trait::async_trait]
impl russh::client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &russh_keys::key::PublicKey) -> Result<bool, Self::Error> {
        match russh_keys::check_known_hosts_path(&self.host, self.port, key, &self.known_hosts) {
            Ok(true) => Ok(true),
            Ok(false) => {
                *self.host_err.lock().unwrap() = Some("unknown host key".into());
                Ok(false)
            }
            Err(_) => {
                *self.host_err.lock().unwrap() = Some("host key changed".into());
                Ok(false)
            }
        }
    }
}

pub fn ssh_dir() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default().join(".ssh")
}

fn key_paths(key: &str) -> Vec<PathBuf> {
    if key.is_empty() || key == "auto" {
        let d = ssh_dir();
        vec![d.join("id_ed25519"), d.join("id_rsa"), d.join("id_ecdsa")]
    } else {
        vec![PathBuf::from(key)]
    }
}

type Session = russh::client::Handle<Client>;

/// Connect + authenticate with the system key. Errors are raw strings (see `explain_ssh_error`).
async fn connect(s: &Server, known_hosts: PathBuf) -> Result<Session, String> {
    let addr = resolve(&s.host, s.port).await.ok_or_else(|| "could not resolve host".to_string())?;
    let config = Arc::new(russh::client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(15)),
        keepalive_max: 3,
        ..Default::default()
    });
    let host_err = Arc::new(Mutex::new(None));
    let handler = Client { host: s.host.clone(), port: s.port, known_hosts, host_err: host_err.clone() };
    let fut = russh::client::connect(config, addr, handler);
    let mut session = match tokio::time::timeout(Duration::from_secs(10), fut).await {
        Err(_) => return Err("connection timed out".into()),
        Ok(Err(e)) => {
            if let Some(h) = host_err.lock().unwrap().clone() {
                return Err(h);
            }
            return Err(e.to_string());
        }
        Ok(Ok(sess)) => sess,
    };
    let mut last = "no key".to_string();
    for p in key_paths(&s.key) {
        if !p.exists() {
            continue;
        }
        let kp = match russh_keys::load_secret_key(&p, None) {
            Ok(k) => k,
            Err(e) => {
                last = if matches!(e, russh_keys::Error::KeyIsEncrypted) { "key is encrypted".into() } else { e.to_string() };
                continue;
            }
        };
        match tokio::time::timeout(Duration::from_secs(10), session.authenticate_publickey(s.user.clone(), Arc::new(kp))).await {
            Ok(Ok(true)) => return Ok(session),
            Ok(Ok(false)) => last = "auth rejected".into(),
            Ok(Err(e)) => last = e.to_string(),
            Err(_) => last = "auth timed out".into(),
        }
    }
    Err(last)
}

/// One-shot check for the editor: ping + ssh login + `echo ok`.
pub async fn test_server(s: &Server) -> Value {
    let addr = resolve(&s.host, s.port).await;
    let (ms, _) = match addr {
        Some(a) => ping(a).await,
        None => (None, None),
    };
    let res = async {
        let sess = connect(s, ssh_dir().join("known_hosts")).await?;
        let mut ch = sess.channel_open_session().await.map_err(|e| e.to_string())?;
        ch.exec(true, "echo ok").await.map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        let read = async {
            while let Some(msg) = ch.wait().await {
                match msg {
                    russh::ChannelMsg::Data { data } => out.extend_from_slice(&data),
                    russh::ChannelMsg::Eof | russh::ChannelMsg::Close => break,
                    _ => {}
                }
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(5), read).await;
        let _ = sess.disconnect(russh::Disconnect::ByApplication, "", "").await;
        Ok::<_, String>(String::from_utf8_lossy(&out).trim().to_string())
    }
    .await;
    match res {
        Ok(_) => json!({"ok": true, "ping": ms, "err": null}),
        Err(e) => json!({"ok": false, "ping": ms, "err": explain_ssh_error(&e)}),
    }
}

// ---------------------------------------------------------------------------------------- watcher

#[derive(Debug, Clone)]
struct Live {
    server: Server,
    m: Option<ServerMetrics>,
    m_t: Option<Instant>,
    ping: Option<u32>,
    ping_kind: Option<&'static str>,
    err: Option<String>,
}

#[derive(Clone, Default)]
pub struct Watch {
    inner: Arc<Mutex<Vec<Live>>>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    current: Arc<Mutex<Option<(Vec<Server>, u64)>>>,
    interval: Arc<Mutex<u64>>,
}

const STALE: f64 = 3.5;

impl Watch {
    /// Start/restart sessions when the list changes. Must be called inside the tokio runtime.
    pub fn reconcile(&self, servers: Vec<Server>, interval: u64) {
        let interval = interval.max(5);
        {
            let mut cur = self.current.lock().unwrap();
            if cur.as_ref() == Some(&(servers.clone(), interval)) {
                return;
            }
            *cur = Some((servers.clone(), interval));
        }
        for t in self.tasks.lock().unwrap().drain(..) {
            t.abort();
        }
        *self.interval.lock().unwrap() = interval;
        *self.inner.lock().unwrap() = servers
            .iter()
            .map(|s| Live { server: s.clone(), m: None, m_t: None, ping: None, ping_kind: None, err: None })
            .collect();
        let mut tasks = self.tasks.lock().unwrap();
        for (i, _) in servers.iter().enumerate() {
            let w = self.clone();
            tasks.push(tokio::spawn(async move { w.session_loop(i, interval).await }));
        }
        if !servers.is_empty() {
            let w = self.clone();
            tasks.push(tokio::spawn(async move { w.ping_loop(interval).await }));
        }
        tracing::info!("servers watcher: {} server(s), every {interval}s", servers.len());
    }

    fn set<F: FnOnce(&mut Live)>(&self, i: usize, f: F) {
        if let Some(l) = self.inner.lock().unwrap().get_mut(i) {
            f(l);
        }
    }

    async fn session_loop(&self, i: usize, interval: u64) {
        let mut backoff = 5u64;
        loop {
            let started = Instant::now();
            let err = self.run_session(i, interval).await;
            tracing::info!("ssh #{i} ended: {err}");
            self.set(i, |l| l.err = Some(explain_ssh_error(&err)));
            if started.elapsed() > Duration::from_secs(120) {
                backoff = 5;
            }
            tokio::time::sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(120);
        }
    }

    async fn run_session(&self, i: usize, interval: u64) -> String {
        let Some(server) = self.inner.lock().unwrap().get(i).map(|l| l.server.clone()) else {
            return "gone".into();
        };
        let sess = match connect(&server, ssh_dir().join("known_hosts")).await {
            Ok(s) => s,
            Err(e) => return e,
        };
        let mut ch = match sess.channel_open_session().await {
            Ok(c) => c,
            Err(e) => return e.to_string(),
        };
        if let Err(e) = ch.exec(true, remote_command(interval)).await {
            return e.to_string();
        }
        let mut parser = ProcParser::default();
        let mut buf: Vec<u8> = Vec::new();
        let mut stderr = String::new();
        let stale = Duration::from_secs_f64(STALE * interval as f64);
        loop {
            match tokio::time::timeout(stale, ch.wait()).await {
                Err(_) => {
                    let _ = sess.disconnect(russh::Disconnect::ByApplication, "", "").await;
                    return "stream timed out".into();
                }
                Ok(None) => break,
                Ok(Some(msg)) => match msg {
                    russh::ChannelMsg::Data { data } => {
                        buf.extend_from_slice(&data);
                        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                            let line: Vec<u8> = buf.drain(..=pos).collect();
                            let line = String::from_utf8_lossy(&line);
                            if let Some(m) = parser.feed(&line) {
                                self.set(i, |l| {
                                    let mut m = m;
                                    if m.cpu.is_none() {
                                        m.cpu = l.m.as_ref().and_then(|o| o.cpu);
                                    }
                                    l.m = Some(m);
                                    l.m_t = Some(Instant::now());
                                    l.err = None;
                                });
                            }
                        }
                        if buf.len() > 64 * 1024 {
                            buf.clear();
                        }
                    }
                    russh::ChannelMsg::ExtendedData { data, .. } => {
                        if stderr.len() < 512 {
                            stderr.push_str(&String::from_utf8_lossy(&data));
                        }
                    }
                    russh::ChannelMsg::ExitStatus { .. } | russh::ChannelMsg::Eof | russh::ChannelMsg::Close => {}
                    _ => {}
                },
            }
        }
        let _ = sess.disconnect(russh::Disconnect::ByApplication, "", "").await;
        let last = stderr.lines().last().unwrap_or("").trim().to_string();
        if last.is_empty() {
            "session closed (remote is not Linux?)".into()
        } else {
            last
        }
    }

    async fn ping_loop(&self, interval: u64) {
        let mut addrs: Vec<Option<SocketAddr>> = Vec::new();
        loop {
            let servers: Vec<Server> = self.inner.lock().unwrap().iter().map(|l| l.server.clone()).collect();
            addrs.resize(servers.len(), None);
            for (i, s) in servers.iter().enumerate() {
                if addrs[i].is_none() {
                    addrs[i] = resolve(&s.host, s.port).await;
                }
                let (ms, kind) = match addrs[i] {
                    Some(a) => ping(a).await,
                    None => (None, None),
                };
                self.set(i, |l| {
                    l.ping = ms;
                    l.ping_kind = kind;
                });
            }
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }

    /// Contract shape: id, name, host, status ok|noagent|down, ping, pingKind, m, age, err.
    pub fn snapshot(&self) -> Vec<Value> {
        let interval = *self.interval.lock().unwrap() as f64;
        self.inner
            .lock()
            .unwrap()
            .iter()
            .map(|l| {
                let age = l.m_t.map(|t| t.elapsed().as_secs_f64());
                let fresh = l.m.is_some() && age.map(|a| a < STALE * interval).unwrap_or(false);
                let status = if fresh {
                    "ok"
                } else if l.ping.is_some() {
                    "noagent"
                } else {
                    "down"
                };
                let name = if l.server.name.is_empty() { l.server.host.clone() } else { l.server.name.clone() };
                json!({
                    "id": l.server.id, "name": name, "host": l.server.host, "status": status,
                    "ping": l.ping, "pingKind": l.ping_kind, "m": l.m,
                    "age": age.map(|a| a.round() as u64),
                    "err": if fresh { None } else { l.err.clone().or_else(|| Some("подключение…".to_string())) },
                })
            })
            .collect()
    }
}
