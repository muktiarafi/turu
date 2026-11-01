//! Butler's own diagnostics (VibeSkua ButlerV5 `DebugLog`): a capped in-memory log of its
//! components (Broadcaster, Orders, Follower, QuestBypass, ...) for the Butler window's log
//! viewer, with Copy, Save and Clear. A repeat of the last line counts up instead of adding
//! one. With "Auto-save logs" on, every line is appended to a file as well.

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};

/// How many lines the viewer keeps (VibeSkua: 2000).
const MAX_LINES: usize = 2000;

/// A line's severity, which picks its color in the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub seq: u64,
    pub level: Level,
    /// `HH:MM:SS.mmm [Component] message`, with ` (xN)` for repeats.
    pub text: String,
}

#[derive(Default)]
struct Inner {
    lines: VecDeque<LogLine>,
    seq: u64,
    /// The last line without its time, and how often it came in a row.
    last: Option<(String, u32)>,
    /// Bumped on every change, so the viewer knows when to follow the tail.
    version: u64,
}

#[derive(Default)]
pub struct DebugLog {
    inner: Mutex<Inner>,
}

impl DebugLog {
    /// Adds a line; with `file`, appends it there too.
    pub fn push(&self, component: &str, message: &str, level: Level, file: Option<&Path>) {
        let time = chrono::Local::now().format("%H:%M:%S%.3f");
        let norm = format!("[{component}] {message}");
        let line = format!("{time} {norm}");
        if let Some(path) = file {
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                })
                .and_then(|mut f| writeln!(f, "{line}"));
            if let Err(e) = written {
                tracing::debug!("Couldn't append to the Butler log {path:?}: {e}");
            }
        }
        let mut guard = self.inner.lock();
        let inner = &mut *guard;
        inner.version += 1;
        let seq = inner.seq;
        if let Some((last, count)) = inner.last.as_mut()
            && *last == norm
            && let Some(entry) = inner.lines.back_mut()
            && entry.seq == seq
        {
            *count += 1;
            entry.text = format!("{line} (x{count})");
            return;
        }
        inner.seq += 1;
        let seq = inner.seq;
        inner.last = Some((norm, 1));
        inner.lines.push_back(LogLine {
            seq,
            level,
            text: line,
        });
        while inner.lines.len() > MAX_LINES {
            inner.lines.pop_front();
        }
    }

    pub fn lines(&self) -> Vec<LogLine> {
        self.inner.lock().lines.iter().cloned().collect()
    }

    /// Changes with every line, repeat or clear.
    pub fn version(&self) -> u64 {
        self.inner.lock().version
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.lines.clear();
        inner.last = None;
        inner.version += 1;
    }

    /// The whole log, one line each (Copy).
    pub fn text(&self) -> String {
        let lines = self.lines();
        lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Writes the log to `export_<who>_<date>.log` in `dir` (VibeSkua `SaveExport`); its path.
    pub fn save(&self, dir: &Path, who: &str) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let who = if who.is_empty() {
            format!("pid{}", std::process::id())
        } else {
            who.to_owned()
        };
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let path = dir.join(format!("export_{who}_{stamp}.log"));
        std::fs::write(&path, self.text())?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_count_up_and_the_log_is_capped() {
        let log = DebugLog::default();
        log.push("Follower", "goto Hero", Level::Info, None);
        log.push("Follower", "goto Hero", Level::Info, None);
        log.push("Follower", "goto Hero", Level::Info, None);
        log.push("Orders", "goto Hero", Level::Warning, None);
        let lines = log.lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].text.ends_with("[Follower] goto Hero (x3)"));
        assert_eq!(lines[1].level, Level::Warning);
        for i in 0..MAX_LINES + 10 {
            log.push("Follower", &i.to_string(), Level::Info, None);
        }
        assert_eq!(log.lines().len(), MAX_LINES);
        log.clear();
        assert!(log.lines().is_empty());
    }
}
