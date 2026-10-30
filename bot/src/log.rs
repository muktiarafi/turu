//! Log buffers the UI shows (Script Loader's "SCRIPT LOG", Diagnostics > Logs, Packet Logger).

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    /// `bot.log(...)` and script lifecycle messages.
    Script,
    /// Flash `debug` messages and bot internals.
    Debug,
    /// Outgoing `%xt%zm%...%` packets (VibeSkua: the `packet` event).
    Packet,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub time: chrono::DateTime<chrono::Local>,
    pub text: String,
}

impl LogLine {
    pub fn formatted(&self) -> String {
        format!("[{}] {}", self.time.format("%H:%M:%S"), self.text)
    }
}

pub struct LogBuffer {
    name: &'static str,
    lines: Mutex<VecDeque<LogLine>>,
    capacity: usize,
    /// Bumped on every change, so the UI can tell when to scroll.
    version: AtomicU64,
    /// Lines pushed so far: the next line's number (the control API's `since`).
    pushed: AtomicU64,
}

impl LogBuffer {
    pub fn new(name: &'static str, capacity: usize) -> Self {
        Self {
            name,
            lines: Mutex::new(VecDeque::new()),
            capacity,
            version: AtomicU64::new(0),
            pushed: AtomicU64::new(0),
        }
    }

    pub fn push(&self, text: impl Into<String>) {
        let text = text.into();
        if self.name == "debug" {
            tracing::info!(target: "turu", "{text}");
        }
        let mut lines = self.lines.lock();
        if lines.len() >= self.capacity {
            lines.pop_front();
        }
        lines.push_back(LogLine {
            time: chrono::Local::now(),
            text,
        });
        self.version.fetch_add(1, Ordering::Relaxed);
        self.pushed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn clear(&self) {
        self.lines.lock().clear();
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> Vec<LogLine> {
        self.lines.lock().iter().cloned().collect()
    }

    /// The lines numbered `from` and later that are still kept, and the next line's number.
    /// Lines are numbered from 0 in the order they were pushed.
    pub fn since(&self, from: u64) -> (Vec<LogLine>, u64) {
        let lines = self.lines.lock();
        let next = self.pushed.load(Ordering::Relaxed);
        let first = next - lines.len() as u64;
        let skip = from.saturating_sub(first) as usize;
        (lines.iter().skip(skip).cloned().collect(), next)
    }

    pub fn len(&self) -> usize {
        self.lines.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    pub fn text(&self) -> String {
        let lines = self.lines.lock();
        let mut out = String::new();
        for line in lines.iter() {
            out.push_str(&line.formatted());
            out.push('\n');
        }
        out
    }
}

pub struct Logs {
    pub script: LogBuffer,
    pub debug: LogBuffer,
    pub packets: LogBuffer,
    /// Diagnostics > Logger is on: outgoing packets go to `packets`.
    pub packet_logging: std::sync::atomic::AtomicBool,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            script: LogBuffer::new("script", 5000),
            debug: LogBuffer::new("debug", 5000),
            packets: LogBuffer::new("packets", 1000),
            packet_logging: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Logs {
    pub fn get(&self, kind: LogKind) -> &LogBuffer {
        match kind {
            LogKind::Script => &self.script,
            LogKind::Debug => &self.debug,
            LogKind::Packet => &self.packets,
        }
    }
}
