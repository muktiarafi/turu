//! Diagnostics > Spammer: sends a queue of packets over and over (VibeSkua
//! `PacketSpammerViewModel`).
//!
//! The loop runs on its own thread and stops on Stop, when the queue is emptied, or when a send
//! fails (no game: the tab closed). Queue files are VibeSkua's: one packet per line.

use crate::api::send;
use crate::{Bot, BotResult};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// One tab's spammer.
pub struct Spammer {
    /// The queue, sent top to bottom. The loop reads it live, so edits apply at once.
    pub packets: Mutex<Vec<String>>,
    delay_ms: AtomicU64,
    to_client: AtomicBool,
    running: AtomicBool,
    /// Bumped by every Start and Stop, so an old loop never outlives its Stop.
    run: AtomicU64,
    /// The row being sent and "Sending n of m", while it runs.
    progress: Mutex<Option<(usize, String)>>,
}

impl Default for Spammer {
    fn default() -> Self {
        Self {
            packets: Mutex::new(Vec::new()),
            delay_ms: AtomicU64::new(1000),
            to_client: AtomicBool::new(false),
            running: AtomicBool::new(false),
            run: AtomicU64::new(0),
            progress: Mutex::new(None),
        }
    }
}

impl Spammer {
    /// "Every n ms": the wait after each packet.
    pub fn delay_ms(&self) -> u64 {
        self.delay_ms.load(Ordering::Relaxed)
    }

    pub fn set_delay_ms(&self, ms: u64) {
        self.delay_ms.store(ms, Ordering::Relaxed);
    }

    /// "Send to client": the client handles the packets as if the server sent them.
    pub fn to_client(&self) -> bool {
        self.to_client.load(Ordering::Relaxed)
    }

    pub fn set_to_client(&self, on: bool) {
        self.to_client.store(on, Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// The queue row being sent and "Sending n of m", while it runs.
    pub fn progress(&self) -> Option<(usize, String)> {
        self.progress.lock().clone()
    }

    /// Start: sends the queue in a loop on a thread of its own; nothing when it is empty or
    /// already running.
    pub fn start(self: &Arc<Self>, bot: &Arc<Bot>) {
        if self.packets.lock().is_empty() || self.running.swap(true, Ordering::AcqRel) {
            return;
        }
        let run = self.run.fetch_add(1, Ordering::AcqRel) + 1;
        let (spammer, bot) = (self.clone(), bot.clone());
        let spawned = std::thread::Builder::new()
            .name("Turu Spammer".into())
            .spawn(move || spammer.spam(&bot, run));
        if let Err(e) = spawned {
            tracing::error!("Couldn't start the spammer: {e}");
            self.running.store(false, Ordering::Release);
        }
    }

    pub fn stop(&self) {
        self.run.fetch_add(1, Ordering::AcqRel);
        self.running.store(false, Ordering::Release);
        *self.progress.lock() = None;
    }

    fn live(&self, run: u64) -> bool {
        self.run.load(Ordering::Acquire) == run
    }

    fn spam(&self, bot: &Bot, run: u64) {
        'outer: while self.live(run) {
            let mut i = 0;
            loop {
                let (packet, count) = {
                    let packets = self.packets.lock();
                    (packets.get(i).cloned(), packets.len())
                };
                if count == 0 {
                    break 'outer;
                }
                let Some(packet) = packet else { break };
                if !self.live(run) {
                    break 'outer;
                }
                *self.progress.lock() = Some((i, format!("Sending {} of {count}", i + 1)));
                if let Err(e) = self.send(bot, &packet) {
                    bot.logs.debug.push(format!("Spammer stopped: {e}"));
                    break 'outer;
                }
                let until = Instant::now() + Duration::from_millis(self.delay_ms());
                while Instant::now() < until {
                    if !self.live(run) {
                        break 'outer;
                    }
                    std::thread::sleep((until - Instant::now()).min(Duration::from_millis(25)));
                }
                i += 1;
            }
        }
        if self.live(run) {
            self.running.store(false, Ordering::Release);
            *self.progress.lock() = None;
        }
    }

    /// Send now, and each queued packet: to the server (`sfc.sendString` / `sfc.sendJson`), or
    /// with "Send to client" to the client (`str` / `json`). A packet starting with `{` is JSON.
    pub fn send(&self, bot: &Bot, packet: &str) -> BotResult<()> {
        let json = packet.starts_with('{');
        if self.to_client() {
            send::client_packet(bot, packet, if json { "json" } else { "str" })
        } else {
            send::packet(bot, packet, if json { "Json" } else { "String" })
        }
    }

    /// Add: appends `packet` unless it is blank.
    pub fn add(&self, packet: &str) -> bool {
        if packet.trim().is_empty() {
            return false;
        }
        self.packets.lock().push(packet.to_owned());
        true
    }

    /// Load: replaces the queue with a saved list.
    pub fn load(&self, path: &Path) -> std::io::Result<()> {
        let text = std::fs::read_to_string(path)?;
        *self.packets.lock() = parse_queue(&text);
        Ok(())
    }

    /// Save: writes the queue, one packet per line.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, format_queue(&self.packets.lock()))
    }
}

/// A queue file's packets: its lines (VibeSkua `File.ReadAllLines`).
pub fn parse_queue(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

/// A queue file: each packet and a line break (VibeSkua `File.WriteAllLines`).
pub fn format_queue(packets: &[String]) -> String {
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    packets.iter().map(|p| format!("{p}{newline}")).collect()
}
