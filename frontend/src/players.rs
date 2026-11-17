//! A process's player: what the desktop's window for a movie given on the command line
//! (`app/src/player.rs`) and the windowless loop (`crate::engine`: a process tab's client, desktop
//! `--headless` and an Android slot) keep. Each tab is a process of its own
//! (docs/TAB_PROCESSES_PROPOSAL.md, M6), so a process holds one player: Ruffle's thread-local
//! state (deferred shapes, replays, interpolation, the renderer's unsubmitted command segments)
//! is its alone, with nothing to swap. (Until M6 several tabs' players shared one thread, each
//! with its own state swapped in while its code ran.)

use crate::turu::{self, TabId};
use ruffle_core::Player;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use turu_bot::perf::PerfStats;

/// The longest the loop goes without servicing the player's sockets and timers while it has no
/// frame due (`crate::engine`).
pub const HIDDEN_TICK_MAX: Duration = Duration::from_millis(50);

/// The player, locked.
pub type TabPlayer<'a> = MutexGuard<'a, Player>;

/// The player, with what the frontend keeps beside it (`extra`).
pub struct Slot<E> {
    /// The tab this player is the game of (when it plays AQW).
    tab: Option<TabId>,
    player: Arc<Mutex<Player>>,
    /// The tab's bot's stats, which count this player's ticks.
    perf: Option<Arc<PerfStats>>,
    pub extra: E,
}

impl<E> Slot<E> {
    /// The tab's player (`tab` is `None` when the player doesn't play AQW).
    pub fn new(tab: Option<TabId>, player: Arc<Mutex<Player>>, extra: E) -> Self {
        Self {
            tab,
            player,
            perf: tab
                .and_then(turu::tab_bot)
                .map(|bot| bot.flash.perf().clone()),
            extra,
        }
    }

    pub fn tab(&self) -> Option<TabId> {
        self.tab
    }

    fn lock(&self) -> TabPlayer<'_> {
        // A lock that fails is a fatal error, not a lack of player.
        self.player
            .try_lock()
            .expect("Player lock must be available")
    }

    /// Detaches the tab's bot, which would otherwise take the closed game for a disconnect.
    fn detach_bot(&self) {
        if let Some(bot) = self.tab.and_then(turu::tab_bot) {
            bot.set_host(None);
        }
    }
}

/// The process's player, once there is one.
pub struct Players<E> {
    slot: Option<Slot<E>>,
}

impl<E> Default for Players<E> {
    fn default() -> Self {
        Self { slot: None }
    }
}

impl<E> Players<E> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        usize::from(self.slot.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.slot.is_none()
    }

    /// The player's slot.
    pub fn active(&self) -> Option<&Slot<E>> {
        self.slot.as_ref()
    }

    /// The slot, as a list (none or one).
    pub fn slots(&self) -> &[Slot<E>] {
        self.slot.as_slice()
    }

    /// Makes `slot` the player (the old one's player is closed, its bot detached).
    pub fn replace_active(&mut self, slot: Slot<E>) {
        if let Some(old) = self.slot.replace(slot) {
            old.detach_bot();
        }
    }

    /// Makes `slot` the player, as [`Self::replace_active`].
    pub fn add(&mut self, slot: Slot<E>) {
        self.replace_active(slot);
    }

    /// Closes the player.
    pub fn clear(&mut self) {
        if let Some(slot) = self.slot.take() {
            slot.detach_bot();
        }
    }

    /// The player, as an iterator (to tick it, as before M6 every tab's), marked active.
    pub fn all(&self) -> impl Iterator<Item = (bool, TabPlayer<'_>)> {
        self.slot.iter().map(|slot| (true, slot.lock()))
    }

    /// The player, if it's the tab's.
    pub fn get_tab(&self, tab: TabId) -> Option<TabPlayer<'_>> {
        let slot = self.slot.as_ref().filter(|slot| slot.tab == Some(tab))?;
        Some(slot.lock())
    }

    /// The player.
    pub fn get(&self) -> Option<MutexGuard<'_, Player>> {
        self.slot.as_ref().map(Slot::lock)
    }

    /// The player ticked for `took` (counted in its bot's stats).
    pub fn record_tick(&self, took: Duration) {
        if let Some(perf) = self.slot.as_ref().and_then(|p| p.perf.as_ref()) {
            perf.record_tick(took);
        }
    }
}
