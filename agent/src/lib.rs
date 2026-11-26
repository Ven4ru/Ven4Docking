//! Ven4Docking agent: PC metrics, servers over SSH, AI limits from local files, panel HTTP API.

pub mod actions;
pub mod ai;
pub mod config;
pub mod discovery;
pub mod firewall;
pub mod http;
pub mod layout;
pub mod logging;
pub mod metrics;
pub mod notify;
pub mod obs;
pub mod platform;
pub mod poll;
pub mod servers;
pub mod state;
#[cfg(windows)]
pub mod tray;
pub mod update;
pub mod util;
pub mod wol;
