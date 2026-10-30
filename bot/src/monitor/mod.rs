//! The web monitor (docs/turu/WEB_MONITOR_PROPOSAL.md): a dashboard of every bot in a browser,
//! in place of VibeSkua's Skua Nexus webhook.
//!
//! * Every bot has a [`Sampler`]: a thread that reads a [`Snapshot`] every
//!   [`SAMPLE_EVERY`] while someone watches the page, and nothing otherwise. The slow fields
//!   (items, quests) are read every [`SLOW_EVERY`], or at the next sample after an event that
//!   changes them.
//! * The [`Hub`] keeps every bot's latest snapshot, events and history: this window's tabs and
//!   the clients Army > Accounts launched.
//! * [`server`] serves the page, the hub's entries, and commands for them.

pub mod hub;
pub mod server;
pub mod snapshot;

pub use hub::Hub;
pub use snapshot::Snapshot;

use crate::Bot;
use crate::events::BotEvent;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use snapshot::{Session, now_ms};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

pub const DEFAULT_PORT: u16 = 8790;
pub const SAMPLE_EVERY: Duration = Duration::from_secs(2);
pub const SLOW_EVERY: Duration = Duration::from_secs(30);
/// A request keeps the samplers going this long; an open event stream keeps them going.
const WATCH_WINDOW_MS: u64 = 10_000;
const TICK: Duration = Duration::from_millis(250);

static STREAMS: AtomicUsize = AtomicUsize::new(0);
static LAST_REQUEST: AtomicU64 = AtomicU64::new(0);

/// Someone looks at the page: an event stream is open or a request came lately.
pub fn watched() -> bool {
    STREAMS.load(Ordering::Relaxed) > 0
        || now_ms().saturating_sub(LAST_REQUEST.load(Ordering::Relaxed)) < WATCH_WINDOW_MS
}

pub(crate) fn touch() {
    LAST_REQUEST.store(now_ms(), Ordering::Relaxed);
}

/// Counts an open event stream while it lives.
pub(crate) struct Watching;

impl Watching {
    pub(crate) fn new() -> Self {
        STREAMS.fetch_add(1, Ordering::Relaxed);
        Self
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        STREAMS.fetch_sub(1, Ordering::Relaxed);
        touch();
    }
}

/// A bot's latest snapshot and what happened to it lately.
#[derive(Default)]
pub struct Sampler {
    latest: Mutex<Option<Snapshot>>,
    /// The slow fields changed since the last read.
    dirty: AtomicBool,
    /// Held while reading the game: one sample at a time (the thread, or the control API's
    /// `/snapshot` for Army).
    reading: Mutex<Reading>,
    events: Mutex<Events>,
}

#[derive(Default)]
struct Reading {
    last_slow: Option<Instant>,
    session: Option<SessionStart>,
}

#[derive(Default)]
struct Events {
    next: u64,
    list: VecDeque<Event>,
}

/// Something that happened to a bot, for the page's event list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Counts up from 0 in the bot that recorded it.
    pub seq: u64,
    /// ms since 1970.
    pub at: u64,
    pub kind: EventKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    ScriptStarted,
    ScriptStopped,
    ScriptError,
    Drop,
    Death,
    Relogin,
    ConnectionLost,
    Login,
    Logout,
}

/// Events kept per bot.
pub const EVENTS_KEPT: usize = 200;

impl Sampler {
    pub fn latest(&self) -> Option<Snapshot> {
        self.latest.lock().clone()
    }

    /// Records an event; never blocks on the game (it runs inside `on_flash_call` too).
    pub fn event(&self, kind: EventKind, text: impl Into<String>) {
        let mut events = self.events.lock();
        let seq = events.next;
        events.next += 1;
        events.list.push_back(Event {
            seq,
            at: now_ms(),
            kind,
            text: text.into(),
        });
        if events.list.len() > EVENTS_KEPT {
            events.list.pop_front();
        }
    }

    /// The events from number `since` on, and the number to ask from next time.
    pub fn events_since(&self, since: u64) -> (Vec<Event>, u64) {
        let events = self.events.lock();
        let list = events
            .list
            .iter()
            .filter(|e| e.seq >= since)
            .cloned()
            .collect();
        (list, events.next)
    }
}

/// What the running script had when the sampler first saw it.
struct SessionStart {
    /// `Stats::started`: tells one run from the next.
    run: Option<Instant>,
    at: Instant,
    gold: i64,
    xp: i64,
}

pub(crate) fn start(bot: &Arc<Bot>) {
    let weak = Arc::downgrade(bot);
    // Runs inside `on_flash_call`: only marks the slow fields and notes events.
    bot.events.subscribe(move |event| {
        let Some(bot) = weak.upgrade() else { return };
        let monitor = &bot.monitor;
        match event {
            BotEvent::ItemAdded { .. }
            | BotEvent::ItemBought
            | BotEvent::ItemSold
            | BotEvent::QuestAccepted { .. }
            | BotEvent::QuestTurnedIn { .. }
            | BotEvent::MapChanged { .. } => monitor.dirty.store(true, Ordering::Relaxed),
            BotEvent::ItemDropped { item } => {
                monitor.event(EventKind::Drop, format!("{} x{}", item.name, item.quantity));
            }
            BotEvent::PlayerDeath => monitor.event(EventKind::Death, "Died"),
            BotEvent::ReloginTriggered { was_kicked } => monitor.event(
                EventKind::Relogin,
                if *was_kicked {
                    "Kicked, relogging"
                } else {
                    "Disconnected, relogging"
                },
            ),
            BotEvent::ConnectionLost => monitor.event(EventKind::ConnectionLost, "Connection lost"),
            BotEvent::Login { username } => {
                monitor.dirty.store(true, Ordering::Relaxed);
                monitor.event(EventKind::Login, format!("Logged in as {username}"));
            }
            BotEvent::Logout => monitor.event(EventKind::Logout, "Logged out"),
            _ => {}
        }
    });
    let weak = Arc::downgrade(bot);
    std::thread::Builder::new()
        .name("Turu Monitor".into())
        .spawn(move || run(weak))
        .expect("couldn't start the monitor thread");
}

fn run(weak: Weak<Bot>) {
    let mut last_sample: Option<Instant> = None;
    loop {
        std::thread::sleep(TICK);
        let Some(bot) = weak.upgrade() else { break };
        if !watched() || last_sample.is_some_and(|t| t.elapsed() < SAMPLE_EVERY) {
            continue;
        }
        last_sample = Some(Instant::now());
        sample(&bot, true);
    }
}

/// Reads the game and stores the snapshot: the slow fields too when `slow` and they're due,
/// else the last ones. The snapshot lock is never held across a game call.
pub fn sample(bot: &Bot, slow: bool) -> Snapshot {
    let mut reading = bot.monitor.reading.lock();
    let mut snapshot = Snapshot::read(bot);
    if snapshot.playing() {
        let earlier = bot.monitor.latest();
        let due = bot.monitor.dirty.load(Ordering::Relaxed)
            || earlier.as_ref().is_none_or(|e| !e.playing())
            || reading.last_slow.is_none_or(|t| t.elapsed() >= SLOW_EVERY);
        if slow && due {
            bot.monitor.dirty.store(false, Ordering::Relaxed);
            snapshot.read_slow(bot);
            reading.last_slow = Some(Instant::now());
        } else if let Some(earlier) = earlier {
            snapshot.keep_slow(&earlier);
        }
    }
    let run = bot.state.lock().stats.started;
    if !snapshot.script.running {
        reading.session = None;
    } else if snapshot.playing() {
        if reading.session.as_ref().is_none_or(|s| s.run != run) {
            reading.session = Some(SessionStart {
                run,
                at: Instant::now(),
                gold: snapshot.gold,
                xp: snapshot.xp,
            });
        }
        if let Some(start) = &reading.session {
            snapshot.session = Some(Session {
                seconds: start.at.elapsed().as_secs(),
                gold: snapshot.gold - start.gold,
                xp: snapshot.xp - start.xp,
            });
        }
    }
    *bot.monitor.latest.lock() = Some(snapshot.clone());
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_without_a_game() {
        let bot = Bot::new();
        let snapshot = sample(&bot, true);
        assert_eq!(snapshot.state, snapshot::BotState::NotLoaded);
        assert!(snapshot.session.is_none());
        assert_eq!(bot.monitor.latest().unwrap().state, snapshot.state);
    }

    #[test]
    fn keeps_the_last_events() {
        let bot = Bot::new();
        for i in 0..EVENTS_KEPT + 5 {
            bot.monitor.event(EventKind::Drop, format!("Item {i}"));
        }
        let (all, next) = bot.monitor.events_since(0);
        assert_eq!(all.len(), EVENTS_KEPT);
        assert_eq!(all[0].text, "Item 5");
        assert_eq!(next, EVENTS_KEPT as u64 + 5);
        let (new, _) = bot.monitor.events_since(next - 2);
        assert_eq!(new.len(), 2);
    }
}
