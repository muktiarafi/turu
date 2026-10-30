//! The web monitor (docs/turu/WEB_MONITOR_PROPOSAL.md): a dashboard of every bot in a browser,
//! in place of VibeSkua's Skua Nexus webhook.
//!
//! * Every bot has a [`Sampler`]: a thread that reads a [`Snapshot`] every
//!   [`SAMPLE_EVERY`] while someone watches the page, and nothing otherwise. The slow fields
//!   (items, quests) are read every [`SLOW_EVERY`], or at the next sample after an event that
//!   changes them.
//! * [`server`] serves the page and the snapshots; the app tells it which bots there are.

pub mod server;
pub mod snapshot;

pub use snapshot::Snapshot;

use crate::Bot;
use crate::events::BotEvent;
use parking_lot::Mutex;
use snapshot::{Session, now_ms};
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

/// A bot's latest snapshot.
#[derive(Default)]
pub struct Sampler {
    latest: Mutex<Option<Snapshot>>,
    /// The slow fields changed since the last read.
    dirty: AtomicBool,
}

impl Sampler {
    pub fn latest(&self) -> Option<Snapshot> {
        self.latest.lock().clone()
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
    // Runs inside `on_flash_call`: only marks the slow fields, the sampler reads them.
    bot.events.subscribe(move |event| {
        let Some(bot) = weak.upgrade() else { return };
        if matches!(
            event,
            BotEvent::ItemAdded { .. }
                | BotEvent::ItemBought
                | BotEvent::ItemSold
                | BotEvent::QuestAccepted { .. }
                | BotEvent::QuestTurnedIn { .. }
                | BotEvent::MapChanged { .. }
                | BotEvent::Login { .. }
        ) {
            bot.monitor.dirty.store(true, Ordering::Relaxed);
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
    let mut last_slow: Option<Instant> = None;
    let mut session: Option<SessionStart> = None;
    loop {
        std::thread::sleep(TICK);
        let Some(bot) = weak.upgrade() else { break };
        if !watched() || last_sample.is_some_and(|t| t.elapsed() < SAMPLE_EVERY) {
            continue;
        }
        last_sample = Some(Instant::now());
        sample(&bot, &mut last_slow, &mut session);
    }
}

/// Reads the game, then stores the result: the lock is never held across a game call.
fn sample(bot: &Bot, last_slow: &mut Option<Instant>, session: &mut Option<SessionStart>) {
    let mut snapshot = Snapshot::read(bot);
    if snapshot.playing() {
        let dirty = bot.monitor.dirty.swap(false, Ordering::Relaxed);
        let earlier = bot.monitor.latest();
        if dirty || earlier.is_none() || last_slow.is_none_or(|t| t.elapsed() >= SLOW_EVERY) {
            snapshot.read_slow(bot);
            *last_slow = Some(Instant::now());
        } else if let Some(earlier) = earlier {
            snapshot.keep_slow(&earlier);
        }
    }
    let run = bot.state.lock().stats.started;
    if !snapshot.script.running {
        *session = None;
    } else if snapshot.playing() {
        if session.as_ref().is_none_or(|s| s.run != run) {
            *session = Some(SessionStart {
                run,
                at: Instant::now(),
                gold: snapshot.gold,
                xp: snapshot.xp,
            });
        }
        if let Some(start) = session {
            snapshot.session = Some(Session {
                seconds: start.at.elapsed().as_secs(),
                gold: snapshot.gold - start.gold,
                xp: snapshot.xp - start.xp,
            });
        }
    }
    *bot.monitor.latest.lock() = Some(snapshot);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_without_a_game() {
        let bot = Bot::new();
        let (mut slow, mut session) = (None, None);
        sample(&bot, &mut slow, &mut session);
        let snapshot = bot.monitor.latest().expect("sampled");
        assert_eq!(snapshot.state, snapshot::BotState::NotLoaded);
        assert!(slow.is_none() && session.is_none());
    }
}
