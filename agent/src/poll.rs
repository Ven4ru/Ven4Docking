//! Smart polling (spec §3.2): a panel is "active" while its phone asked for state in the last 10 s.
//! Only blocks somebody needs are polled; with no active panel — CPU/GPU/RAM/net every 10 s, nothing else.

use crate::ai::{unix_now, AiReader, AiRoots};
use crate::metrics::{audio::Audio, cpu::Cpu, disks::Disks, gpu::Gpu, media::Media, mem::Mem, net::Net, procs::Procs};
use crate::state::AppState;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::Thread;
use std::time::{Duration, Instant};

pub const ACTIVE_WINDOW: Duration = Duration::from_secs(10);
const TRIM_ABOVE: u64 = 12 * 1024 * 1024;

/// Data blocks the agent can fill. 1.1: `audio` (blocks `mixer` and `keys` may ask by their own names),
/// `obs`, `notify`, `running` (program screens).
pub const DATA_BLOCKS: &[&str] =
    &["cpu", "gpu", "ram", "disks", "net", "media", "servers", "ai", "audio", "mixer", "keys", "obs", "notify", "running"];

/// Response key of a block in /api/v1/state.
pub fn state_key(block: &str) -> Option<&'static str> {
    Some(match block {
        "cpu" => "cpu",
        "gpu" | "gpus" => "gpus",
        "ram" => "ram",
        "disks" => "disks",
        "net" => "net",
        "media" => "media",
        "servers" => "servers",
        "ai" => "ai",
        "audio" | "mixer" | "keys" => "audio",
        "obs" => "obs",
        "notify" => "notify",
        "running" => "running",
        _ => return None,
    })
}

#[derive(Default)]
pub struct Demand {
    inner: Mutex<HashMap<String, (Instant, HashSet<String>)>>,
    sampler: OnceLock<Thread>,
}

impl Demand {
    /// A phone asked for state. Wakes the sampler when this phone was not active.
    pub fn touch(&self, phone: &str, blocks: &[&'static str]) {
        let mut m = self.inner.lock().unwrap();
        let now = Instant::now();
        let was_active = m.get(phone).map(|(t, b)| now.duration_since(*t) < ACTIVE_WINDOW && blocks.iter().all(|x| b.contains(*x))).unwrap_or(false);
        m.insert(phone.to_string(), (now, blocks.iter().map(|s| s.to_string()).collect()));
        drop(m);
        if !was_active {
            if let Some(t) = self.sampler.get() {
                t.unpark();
            }
        }
    }

    /// (number of active panels, union of their blocks).
    pub fn active(&self) -> (usize, HashSet<String>) {
        let now = Instant::now();
        let m = self.inner.lock().unwrap();
        let mut n = 0;
        let mut set = HashSet::new();
        for (t, b) in m.values() {
            if now.duration_since(*t) < ACTIVE_WINDOW {
                n += 1;
                set.extend(b.iter().cloned());
            }
        }
        (n, set)
    }

    pub fn is_active(&self, phone: &str) -> bool {
        self.inner.lock().unwrap().get(phone).map(|(t, _)| t.elapsed() < ACTIVE_WINDOW).unwrap_or(false)
    }

    /// Something changed on demand (e.g. sound from the panel): poll now.
    pub fn wake(&self) {
        if let Some(t) = self.sampler.get() {
            t.unpark();
        }
    }

    pub fn set_sampler(&self, t: Thread) {
        let _ = self.sampler.set(t);
    }
}

/// Collectors, created on first use.
struct Sampler {
    st: Arc<AppState>,
    cpu: Option<Cpu>,
    mem: Option<Mem>,
    net: Option<Net>,
    gpu: Option<Gpu>,
    disks: Option<Disks>,
    procs: Option<Procs>,
    media: Option<Media>,
    ai: Option<AiReader>,
    ai_cfg: Option<crate::config::AiCfg>,
    audio: Option<Audio>,
    last: HashMap<&'static str, Instant>,
    top: Value,
}

impl Sampler {
    fn due(&mut self, key: &'static str, every: Duration) -> bool {
        let now = Instant::now();
        match self.last.get(key) {
            Some(t) if now.duration_since(*t) < every => false,
            _ => {
                self.last.insert(key, now);
                true
            }
        }
    }

    fn tick(&mut self) {
        let (n_active, active) = self.st.demand.active();
        let idle = n_active == 0;
        let want: HashSet<String> = if idle {
            self.st.layout_blocks().into_iter().filter(|b| matches!(b.as_str(), "cpu" | "gpu" | "ram" | "net" | "disks")).collect()
        } else {
            active
        };
        let fast = if idle { Duration::from_secs(10) } else { Duration::from_secs(2) };

        if want.contains("cpu") && self.due("cpu", fast) {
            let lhm = self.st.cfg.read().unwrap().lhm_url.clone();
            let c = self.cpu.get_or_insert_with(|| Cpu::new(lhm.as_deref()));
            let v = c.sample();
            self.st.set_block("cpu", v, None);
        }
        if want.contains("ram") && self.due("ram", fast) {
            let v = self.mem.get_or_insert_with(Mem::new).sample();
            self.st.set_block("ram", v, None);
        }
        if want.contains("net") && self.due("net", fast) {
            let (down, up) = self.net.get_or_insert_with(Net::new).sample();
            if !idle && self.due("procs", Duration::from_secs(5)) {
                let ncpu = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
                self.top = self.procs.get_or_insert_with(|| Procs::new(ncpu)).sample().unwrap_or(Value::Null);
            }
            self.st.set_block("net", json!({"down": down, "up": up, "topProcess": self.top}), None);
        }
        if idle {
            // processes are not polled without a panel; drop the list to free memory
            self.procs = None;
        }
        if want.contains("gpu") && self.due("gpu", fast.max(Duration::from_secs(2))) {
            match self.gpu.get_or_insert_with(Gpu::new).sample() {
                Some(v) => self.st.set_block("gpus", v, None),
                None => self.st.set_block("gpus", Value::Null, Some("видеокарта не найдена".into())),
            }
        }
        if want.contains("disks") && self.due("disks", if idle { Duration::from_secs(60) } else { Duration::from_secs(15) }) {
            let v = self.disks.get_or_insert_with(Disks::new).sample();
            self.st.set_block("disks", v, None);
        }
        if !idle && want.contains("media") && self.due("media", Duration::from_secs(2)) {
            match self.media.get_or_insert_with(Media::new).now() {
                Ok(v) => self.st.set_block("media", v.map(|m| serde_json::to_value(m).unwrap_or(Value::Null)).unwrap_or(Value::Null), None),
                Err(e) => self.st.set_block("media", Value::Null, Some(e)),
            }
        }
        if !idle && want.contains("ai") && self.due("ai", Duration::from_secs(5)) {
            let cfg = self.st.cfg.read().unwrap().ai.clone();
            if self.ai_cfg.as_ref() != Some(&cfg) {
                self.ai = Some(AiReader::new(AiRoots::from_env(), cfg.enabled.clone(), cfg.claude_code_file.clone().map(Into::into), cfg.lmstudio_port));
                self.ai_cfg = Some(cfg);
            }
            let reader = self.ai.as_mut().unwrap();
            let list = reader.poll(unix_now());
            *self.st.ai_sources.lock().unwrap() = reader.sources();
            let v = serde_json::to_value(list).unwrap_or(Value::Null);
            self.st.set_block("ai", v, None);
        }
        let wants_audio = ["audio", "mixer", "keys"].iter().any(|k| want.contains(*k));
        if idle || !wants_audio {
            self.audio = None;
        } else {
            let dirty = self.st.audio_dirty.swap(false, std::sync::atomic::Ordering::Relaxed);
            let due = self.due("audio", Duration::from_secs(2));
            if dirty || due {
                match self.audio.get_or_insert_with(Audio::new).sample() {
                    Ok(v) => self.st.set_block("audio", v, None),
                    Err(e) => self.st.set_block("audio", Value::Null, Some(e)),
                }
            }
        }
        if !idle && want.contains("notify") && self.due("notify", Duration::from_secs(3)) {
            crate::notify::refresh(&self.st);
        }
        if !idle && want.contains("running") && self.due("running", Duration::from_secs(5)) {
            self.running();
        }
    }

    /// Program screens: which `forApp` names of the layouts run now (only those, not the whole process list).
    fn running(&mut self) {
        let phones: Vec<String> = self.st.cfg.read().unwrap().phones.iter().map(|p| p.id.clone()).collect();
        let mut names: Vec<String> = Vec::new();
        for p in phones {
            for n in self.st.layout_for(&p).app_screens() {
                let n = n.to_lowercase();
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        let running = if names.is_empty() { Default::default() } else { crate::platform::process_names() };
        let v: Vec<Value> = names.into_iter().filter(|n| running.contains(n)).map(Value::from).collect();
        self.st.set_block("running", Value::Array(v), None);
    }
}

/// Sampler thread: blocking collectors live here, off the async runtime.
pub fn spawn_sampler(st: Arc<AppState>) -> std::thread::JoinHandle<()> {
    let st2 = st.clone();
    let h = std::thread::Builder::new()
        .name("sampler".into())
        .stack_size(512 * 1024)
        .spawn(move || {
            // Core Audio and WinRT (media, notifications) for the whole life of the thread
            #[cfg(windows)]
            let _com = crate::platform::Com::mta();
            let mut s = Sampler {
                st: st.clone(),
                cpu: None,
                mem: None,
                net: None,
                gpu: None,
                disks: None,
                procs: None,
                media: None,
                ai: None,
                ai_cfg: None,
                audio: None,
                last: HashMap::new(),
                top: Value::Null,
            };
            let mut trim_check = Instant::now();
            loop {
                if !st.paused() {
                    if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.tick())) {
                        tracing::error!("sampler tick panicked: {e:?}");
                    }
                }
                // memory budget (spec §3.2): every 30 s, trim cold pages when the working set exceeds 12 MB
                if trim_check.elapsed() >= Duration::from_secs(30) {
                    trim_check = Instant::now();
                    if crate::platform::process_times().1 > TRIM_ABOVE {
                        crate::platform::trim_working_set();
                    }
                }
                std::thread::park_timeout(Duration::from_secs(1));
            }
        })
        .expect("sampler thread");
    st2.demand.set_sampler(h.thread().clone());
    h
}
