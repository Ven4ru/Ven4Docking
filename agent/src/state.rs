//! Shared agent state: config, layouts, pairing, phone status, latest metrics.

use crate::config::{Config, Paths};
use crate::layout::Layout;
use crate::actions::Actions;
use crate::http::hello::HelloLimit;
use crate::http::pair::Pairs;
use crate::notify::NotifyState;
use crate::obs::CmdMsg;
use crate::poll::Demand;
use crate::servers::Watch;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::Instant;

/// What a phone reported about itself (POST /api/v1/phone), kept in memory.
#[derive(Debug, Clone, Default)]
pub struct PhoneStatus {
    pub info: serde_json::Map<String, Value>,
    pub last_seen: Option<f64>,
    pub last_state: Option<Instant>,
}

/// Latest value of a data block + error text when there is no value.
#[derive(Debug, Clone)]
pub struct BlockData {
    pub value: Value,
    pub err: Option<String>,
}

pub struct AppState {
    pub paths: Paths,
    pub port: u16,
    pub discovery_port: u16,
    pub cfg: RwLock<Config>,
    pub layouts: RwLock<HashMap<String, Layout>>,
    pub admin_key: String,
    pub pairs: Mutex<Pairs>,
    /// Rate limit of POST /api/v1/hello per address.
    pub hello: Mutex<HelloLimit>,
    pub phones: Mutex<HashMap<String, PhoneStatus>>,
    pub demand: Demand,
    pub data: Mutex<HashMap<&'static str, BlockData>>,
    pub watch: Watch,
    pub ai_sources: Mutex<Vec<crate::ai::SourceInfo>>,
    pub paused: AtomicBool,
    pub actions: Box<dyn Actions>,
    pub self_cpu: Mutex<Option<(Instant, f64)>>,
    pub started: Instant,
    pub load_error: Mutex<Option<String>>,
    /// Last firewall check for /admin/status: (when, ok|missing|blocked|unknown).
    pub firewall: Mutex<Option<(Instant, &'static str)>>,
    /// Notifications: last read, dismissed ids, apps seen since start.
    pub notify: Mutex<NotifyState>,
    /// Sound was changed from the panel: the sampler reads it again without waiting 2 s.
    pub audio_dirty: AtomicBool,
    /// Commands for the OBS task (obs::run takes the receiver).
    pub obs_tx: tokio::sync::mpsc::Sender<CmdMsg>,
    pub obs_rx: Mutex<Option<tokio::sync::mpsc::Receiver<CmdMsg>>>,
    /// Adapter for wake-on-LAN, detected at most every 10 min.
    pub wol: Mutex<Option<(Instant, Option<crate::wol::Adapter>)>>,
    /// Latest release seen by the update check.
    pub update: Mutex<crate::update::UpdateState>,
}

impl AppState {
    pub fn new(paths: Paths, cfg: Config, admin_key: String, actions: Box<dyn Actions>) -> AppState {
        let port = cfg.port;
        let (obs_tx, obs_rx) = tokio::sync::mpsc::channel(8);
        let st = AppState {
            paths,
            port,
            discovery_port: port.wrapping_add(1),
            cfg: RwLock::new(cfg),
            layouts: RwLock::new(HashMap::new()),
            admin_key,
            pairs: Mutex::new(Pairs::default()),
            hello: Mutex::new(HelloLimit::default()),
            phones: Mutex::new(HashMap::new()),
            demand: Demand::default(),
            data: Mutex::new(HashMap::new()),
            watch: Watch::default(),
            ai_sources: Mutex::new(Vec::new()),
            paused: AtomicBool::new(false),
            actions,
            self_cpu: Mutex::new(None),
            started: Instant::now(),
            load_error: Mutex::new(None),
            firewall: Mutex::new(None),
            notify: Mutex::new(NotifyState::default()),
            audio_dirty: AtomicBool::new(false),
            obs_tx,
            obs_rx: Mutex::new(Some(obs_rx)),
            wol: Mutex::new(None),
            update: Mutex::new(Default::default()),
        };
        st.load_layouts();
        st
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Read layouts/<phone>.json for every phone in the config; invalid files are logged and skipped.
    pub fn load_layouts(&self) {
        let ids: Vec<String> = self.cfg.read().unwrap().phones.iter().map(|p| p.id.clone()).collect();
        let mut map = HashMap::new();
        for id in ids {
            let p = self.paths.layout_file(&id);
            if let Ok(text) = std::fs::read_to_string(&p) {
                match serde_json::from_str::<Layout>(text.trim_start_matches('\u{feff}')) {
                    Ok(l) => {
                        map.insert(id, l);
                    }
                    Err(e) => tracing::warn!("layout {}: {e}", p.display()),
                }
            }
        }
        *self.layouts.write().unwrap() = map;
    }

    /// Layout of a phone: saved one or the «Классика» template.
    pub fn layout_for(&self, phone: &str) -> Layout {
        self.layouts.read().unwrap().get(phone).cloned().unwrap_or_else(Layout::classic)
    }

    /// Block types used in the layouts of all paired phones.
    pub fn layout_blocks(&self) -> HashSet<String> {
        let phones: Vec<String> = self.cfg.read().unwrap().phones.iter().map(|p| p.id.clone()).collect();
        let mut set = HashSet::new();
        for p in phones {
            for t in self.layout_for(&p).block_types() {
                set.insert(t.to_string());
            }
        }
        set
    }

    /// Servers watched only when some layout shows the servers block.
    pub fn reconcile_servers(&self) {
        let cfg = self.cfg.read().unwrap().clone();
        let want = if self.layout_blocks().contains("servers") { cfg.servers.clone() } else { Vec::new() };
        self.watch.reconcile(want, cfg.servers_interval);
    }

    /// Wake-on-LAN target for the phone: `config.wol`, else the detected adapter (cached for 10 min).
    pub fn wol_target(&self) -> Option<(String, String)> {
        if let Some(w) = self.cfg.read().unwrap().wol.clone() {
            return Some((w.mac, w.broadcast));
        }
        self.detected_adapter().map(|a| (a.mac, a.broadcast))
    }

    pub fn detected_adapter(&self) -> Option<crate::wol::Adapter> {
        let mut c = self.wol.lock().unwrap();
        if let Some((t, a)) = c.as_ref() {
            if t.elapsed() < std::time::Duration::from_secs(600) {
                return a.clone();
            }
        }
        let a = crate::wol::detect();
        *c = Some((Instant::now(), a.clone()));
        a
    }

    pub fn save_config(&self) -> Result<(), String> {
        let cfg = self.cfg.read().unwrap().clone();
        cfg.save(&self.paths.config).map_err(|e| e.to_string())
    }

    pub fn set_block(&self, key: &'static str, value: Value, err: Option<String>) {
        self.data.lock().unwrap().insert(key, BlockData { value, err });
    }

    /// Agent's own CPU (% of the whole machine since the previous call) and working set.
    pub fn self_usage(&self) -> (f64, u64) {
        let (cpu_s, ws) = crate::platform::process_times();
        let now = Instant::now();
        let ncpu = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
        let mut last = self.self_cpu.lock().unwrap();
        let (t0, c0) = last.unwrap_or((self.started, 0.0));
        *last = Some((now, cpu_s));
        let wall = now.duration_since(t0).as_secs_f64().max(0.001);
        let pct = 100.0 * (cpu_s - c0) / wall / ncpu;
        ((pct * 100.0).round() / 100.0, ws)
    }
}
