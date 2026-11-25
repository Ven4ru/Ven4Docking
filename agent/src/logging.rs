//! tracing into <home>/agent.log, rotated at 1 MB, 3 files (agent.log, agent.log.1, agent.log.2).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub const MAX_BYTES: u64 = 1024 * 1024;
pub const KEEP: usize = 3;

pub struct Rotating {
    path: PathBuf,
    file: Option<File>,
    size: u64,
}

impl Rotating {
    pub fn new(path: PathBuf) -> Rotating {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Rotating { path, file: None, size }
    }

    fn rotate(&mut self) {
        self.file = None;
        for i in (1..KEEP).rev() {
            let from = if i == 1 { self.path.clone() } else { self.path.with_extension(format!("log.{}", i - 1)) };
            let to = self.path.with_extension(format!("log.{i}"));
            let _ = std::fs::rename(&from, &to);
        }
        self.size = 0;
    }

    pub fn write_line(&mut self, buf: &[u8]) {
        if self.size + buf.len() as u64 > MAX_BYTES {
            self.rotate();
        }
        if self.file.is_none() {
            self.file = OpenOptions::new().create(true).append(true).open(&self.path).ok();
        }
        if let Some(f) = &mut self.file {
            if f.write_all(buf).is_ok() {
                self.size += buf.len() as u64;
            }
        }
    }
}

pub struct LogWriter(pub &'static Mutex<Rotating>);

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write_line(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn init(path: PathBuf, also_stderr: bool) {
    let rot: &'static Mutex<Rotating> = Box::leak(Box::new(Mutex::new(Rotating::new(path))));
    let make = move || -> Box<dyn Write> {
        if also_stderr {
            Box::new(Tee(LogWriter(rot)))
        } else {
            Box::new(LogWriter(rot))
        }
    };
    let _ = tracing_subscriber::fmt()
        .with_writer(make)
        .with_ansi(false)
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .try_init();
}

struct Tee(LogWriter);

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        self.0.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
