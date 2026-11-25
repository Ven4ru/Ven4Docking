//! PC metrics. Each collector is created lazily, only when a block needs it (spec §3.2).

pub mod cpu;
pub mod disks;
pub mod gpu;
pub mod media;
pub mod mem;
pub mod net;
pub mod pdh;
pub mod procs;

pub use media::{Actions, RecordActions, WinActions};
