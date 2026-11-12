//! `launcher.log` in the data folder, plus stderr.
//!
//! [`rea_launcher::log::Log`] with one file per run: the first line is
//! always this run's version (support advice and the README rely on it),
//! and the previous run is kept as `launcher.log.old`. Every line is
//! timestamped and also echoed to stderr, for `--play` from a terminal.

use std::path::Path;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use rea_launcher::log::Log;

static LOG: OnceLock<Log> = OnceLock::new();

/// This run's log at `path`: per run, timestamped, and without the SDK's
/// size cap (a run's log was never cut short).
fn open(path: &Path) -> Log {
    Log::new(path).per_run().timestamps(true).without_limit()
}

/// Log to `path` from now on; the first line moves the previous run's
/// file to `.old`.
pub fn init(path: &Path) {
    let _ = LOG.set(open(path));
}

pub fn line(msg: &str) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    eprintln!("[{secs:.3}] {msg}");
    if let Some(log) = LOG.get() {
        log.line(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first line is this run's version, and the run before is `.old`.
    #[test]
    fn each_run_starts_afresh_and_keeps_the_last_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("launcher.log");
        let read = |p: &Path| std::fs::read_to_string(p).unwrap();
        let strip = |text: String| -> Vec<String> {
            text.lines()
                .map(|l| {
                    let (stamp, rest) = l.split_once("] ").unwrap();
                    let secs: f64 = stamp.strip_prefix('[').unwrap().parse().unwrap();
                    assert!(secs > 1.7e9, "{l}");
                    rest.to_string()
                })
                .collect()
        };
        let run1 = open(&path);
        let input_thread = run1.clone();
        run1.line("Rad Mobile launcher 1");
        input_thread.line("input: started");
        let run2 = open(&path);
        run2.line("Rad Mobile launcher 2");
        assert_eq!(strip(read(&path)), ["Rad Mobile launcher 2"]);
        assert_eq!(
            strip(read(&dir.path().join("launcher.log.old"))),
            ["Rad Mobile launcher 1", "input: started"]
        );
    }
}
