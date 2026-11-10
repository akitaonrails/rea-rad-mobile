//! `launcher.log` in the data folder, plus stderr.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG: Mutex<Option<File>> = Mutex::new(None);

/// Start a fresh log at `path` (the previous run's is kept as `.old`).
pub fn init(path: &Path) {
    if path.exists() {
        let _ = std::fs::rename(path, path.with_extension("log.old"));
    }
    if let Ok(f) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        && let Ok(mut g) = LOG.lock()
    {
        *g = Some(f);
    }
}

pub fn line(msg: &str) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let text = format!("[{secs:.3}] {msg}\n");
    eprint!("{text}");
    if let Ok(mut g) = LOG.lock()
        && let Some(f) = g.as_mut()
    {
        let _ = f.write_all(text.as_bytes());
    }
}
