//! Several tabs' players on one thread: what the desktop's window (`app/src/player.rs`) and the
//! windowless loop (`crate::engine`, desktop `--headless` and Android) keep per tab. Each tab has
//! its player and, while it isn't the active one, its own thread-local state ([`ThreadState`]):
//! Ruffle keeps a player's deferred shapes, replays, interpolation and the wgpu renderer's
//! unsubmitted command segments in the thread, so a player that isn't the active one has its
//! own swapped in while its code runs ([`TabPlayer`]). The active tab's player runs at its frame
//! rate; the others are ticked when their next frame or timer is due, at least every
//! [`HIDDEN_TICK_MAX`] ([`Players::tick_hidden`]), at the hidden tab's frame rate the bot sets
//! (`turu_bot::api::options::set_hidden_tab_fps`).

use crate::turu::{self, TabId};
use ruffle_core::{FloatDuration, Player, PlayerThreadState};
use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use turu_bot::perf::PerfStats;

/// The longest a hidden tab's player goes without a tick, so its sockets (the game's packets)
/// and timers are serviced at least this often.
pub const HIDDEN_TICK_MAX: Duration = Duration::from_millis(50);

/// What one player keeps in the thread: the core's (`PlayerThreadState`) and its renderer's
/// command segments not submitted yet (`ruffle_render_wgpu::segments`), which another player's
/// submit must not take.
#[derive(Default)]
struct ThreadState {
    core: PlayerThreadState,
    segments: ruffle_render_wgpu::segments::ThreadState,
}

impl ThreadState {
    /// Swaps this state with the thread's.
    fn swap(&mut self) {
        self.core.swap();
        ruffle_render_wgpu::segments::swap(&mut self.segments);
    }
}

/// While alive, a player that isn't the active one has its own thread-local state.
struct SwappedIn<'a>(&'a RefCell<ThreadState>);

impl<'a> SwappedIn<'a> {
    fn new(state: &'a RefCell<ThreadState>) -> Self {
        state.borrow_mut().swap();
        Self(state)
    }
}

impl Drop for SwappedIn<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().swap();
    }
}

/// A tab's locked player. Unless it's the active tab's, its own thread-local state is swapped
/// in until this drops, as its code may run.
pub struct TabPlayer<'a> {
    player: MutexGuard<'a, Player>,
    _swapped: Option<SwappedIn<'a>>,
}

impl Deref for TabPlayer<'_> {
    type Target = Player;

    fn deref(&self) -> &Player {
        &self.player
    }
}

impl DerefMut for TabPlayer<'_> {
    fn deref_mut(&mut self) -> &mut Player {
        &mut self.player
    }
}

/// One tab's player, with what the frontend keeps beside it (`extra`).
pub struct Slot<E> {
    /// The tab this player is the game of (when it plays AQW).
    tab: Option<TabId>,
    player: Arc<Mutex<Player>>,
    /// This player's thread-local state while another player is the active one; the active
    /// player's is the thread's, and this is left default.
    thread_state: RefCell<ThreadState>,
    /// While it's a hidden tab's player, when it was last ticked and when it's due again
    /// ([`Players::tick_hidden`]).
    hidden_tick: Cell<Option<(Instant, Instant)>>,
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
            thread_state: RefCell::default(),
            hidden_tick: Cell::new(None),
            perf: tab
                .and_then(turu::tab_bot)
                .map(|bot| bot.flash.perf().clone()),
            extra,
        }
    }

    pub fn tab(&self) -> Option<TabId> {
        self.tab
    }

    /// Locks the player, swapping its thread-local state in unless it's the active one.
    fn lock(&self, active: bool) -> TabPlayer<'_> {
        TabPlayer {
            _swapped: (!active).then(|| SwappedIn::new(&self.thread_state)),
            player: self
                .player
                .try_lock()
                .expect("Player lock must be available"),
        }
    }

    /// Detaches the tab's bot, which would otherwise take the closed game for a disconnect.
    fn detach_bot(&self) {
        if let Some(bot) = self.tab.and_then(turu::tab_bot) {
            bot.set_host(None);
        }
    }
}

/// The tabs' players, in tab order, and which one is active (drawn, heard, given the input).
pub struct Players<E> {
    slots: Vec<Slot<E>>,
    /// Index of the active player in `slots`.
    active: usize,
}

impl<E> Default for Players<E> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            active: 0,
        }
    }
}

impl<E> Players<E> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The index of the active tab's player in tab order.
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// The active tab's player's slot.
    pub fn active(&self) -> Option<&Slot<E>> {
        self.slots.get(self.active)
    }

    /// Every slot, in tab order.
    pub fn slots(&self) -> &[Slot<E>] {
        &self.slots
    }

    /// The tab of the player at `index` in tab order.
    pub fn tab_at(&self, index: usize) -> Option<TabId> {
        self.slots.get(index)?.tab
    }

    /// Puts `slot` in the active one's place (the old one's player is closed), or adds it as the
    /// active one when there's none.
    pub fn replace_active(&mut self, slot: Slot<E>) {
        if let Some(old) = self.slots.get_mut(self.active) {
            old.detach_bot();
            // The thread gets a new player's state; the old one's goes with it.
            old.thread_state.borrow_mut().swap();
            *old = slot;
        } else {
            self.slots.push(slot);
            self.active = self.slots.len() - 1;
        }
    }

    /// Adds `slot` (a new tab's player) and makes it the active one.
    pub fn add(&mut self, slot: Slot<E>) {
        let tab = slot.tab;
        self.slots.push(slot);
        match tab {
            Some(tab) => {
                self.select_tab(tab);
            }
            None => self.select_index(self.slots.len() - 1),
        }
    }

    /// Makes the tab's player the active one; false when it has none.
    pub fn select_tab(&mut self, tab: TabId) -> bool {
        let Some(i) = self.slots.iter().position(|p| p.tab == Some(tab)) else {
            return false;
        };
        self.select_index(i);
        true
    }

    fn select_index(&mut self, i: usize) {
        if i == self.active || i >= self.slots.len() {
            return;
        }
        // The old active player's state goes into its slot, the new one's into the thread.
        if let Some(old) = self.slots.get(self.active) {
            old.thread_state.borrow_mut().swap();
            // Hidden from now on: `tick_hidden` starts timing it afresh.
            old.hidden_tick.set(None);
        }
        self.slots[i].thread_state.borrow_mut().swap();
        self.active = i;
    }

    /// Closes the tab's player (detaching its bot) and gives back its slot.
    pub fn remove_tab(&mut self, tab: TabId) -> Option<Slot<E>> {
        let i = self.slots.iter().position(|p| p.tab == Some(tab))?;
        let was_active = i == self.active;
        if was_active {
            // The thread's state is the closed player's: it goes with it.
            self.slots[i].thread_state.borrow_mut().swap();
        }
        let slot = self.slots.remove(i);
        slot.detach_bot();
        if self.active > i || self.active >= self.slots.len() {
            self.active = self.active.saturating_sub(1);
        }
        if was_active && let Some(new) = self.slots.get(self.active) {
            new.thread_state.borrow_mut().swap();
        }
        Some(slot)
    }

    /// Closes every player.
    pub fn clear(&mut self) {
        // The thread's state is the active player's: it goes with it.
        if let Some(active) = self.slots.get(self.active) {
            active.thread_state.borrow_mut().swap();
        }
        for slot in self.slots.drain(..) {
            slot.detach_bot();
        }
        self.active = 0;
    }

    /// Every player (to tick them all), the active one first. Use one at a time: each other
    /// player's state is swapped in while its guard lives.
    pub fn all(&self) -> impl Iterator<Item = (bool, TabPlayer<'_>)> {
        let active = self.active;
        let order =
            std::iter::once(active).chain((0..self.slots.len()).filter(move |i| *i != active));
        order.filter_map(move |i| Some((i == active, self.slots.get(i)?.lock(i == active))))
    }

    /// Every player in tab order, with its tab and whether it's the active one. Use one at a
    /// time, as [`Self::all`].
    pub fn in_order(&self) -> impl Iterator<Item = (Option<TabId>, bool, TabPlayer<'_>)> {
        let active = self.active;
        self.slots
            .iter()
            .enumerate()
            .map(move |(i, slot)| (slot.tab, i == active, slot.lock(i == active)))
    }

    /// The tab's player.
    pub fn get_tab(&self, tab: TabId) -> Option<TabPlayer<'_>> {
        let i = self.slots.iter().position(|p| p.tab == Some(tab))?;
        Some(self.slots[i].lock(i == self.active))
    }

    /// The player at `index` in tab order (its state swapped in unless it's the active one).
    pub fn get_index(&self, index: usize) -> Option<TabPlayer<'_>> {
        Some(self.slots.get(index)?.lock(index == self.active))
    }

    /// The active tab's player.
    pub fn get(&self) -> Option<MutexGuard<'_, Player>> {
        // A lock that fails is a fatal error, not a lack of player.
        let slot = self.slots.get(self.active)?;
        Some(
            slot.player
                .try_lock()
                .expect("Player lock must be available"),
        )
    }

    /// Runs `work` with the thread-local state of the player at `index` (a future of its).
    pub fn run_for<R>(&self, index: usize, work: impl FnOnce() -> R) -> R {
        let _swapped = (index != self.active)
            .then(|| {
                self.slots
                    .get(index)
                    .map(|slot| SwappedIn::new(&slot.thread_state))
            })
            .flatten();
        work()
    }

    /// The active tab's player ticked for `took` (counted in its bot's stats).
    pub fn record_tick(&self, took: Duration) {
        if let Some(perf) = self.slots.get(self.active).and_then(|p| p.perf.as_ref()) {
            perf.record_tick(took);
        }
    }

    /// Ticks the hidden tabs' players whose next frame or timer is due, or that waited
    /// [`HIDDEN_TICK_MAX`], rather than all of them on every pass of the loop; returns when the
    /// next one is due. `ran_frames` is called with a player that just ran frames (to draw it, in
    /// Grid View) and says whether it drew; the bool returned says whether one did.
    pub fn tick_hidden(
        &self,
        now: Instant,
        mut ran_frames: impl FnMut(&mut Player) -> bool,
    ) -> (Option<Instant>, bool) {
        let mut drew = false;
        let mut soonest: Option<Instant> = None;
        for (i, slot) in self.slots.iter().enumerate() {
            if i == self.active {
                continue;
            }
            let next = match slot.hidden_tick.get() {
                Some((last, due)) if now >= due => {
                    let mut player = slot.lock(false);
                    let started = Instant::now();
                    let frames_before = player.frames_run();
                    player.tick(FloatDuration::from_std(now - last));
                    if let Some(perf) = &slot.perf {
                        perf.record_tick(started.elapsed());
                    }
                    if player.frames_run() != frames_before && ran_frames(&mut player) {
                        drew = true;
                    }
                    now + player.time_til_next_frame().min(HIDDEN_TICK_MAX)
                }
                Some((_, due)) => {
                    soonest = Some(soonest.map_or(due, |t| t.min(due)));
                    continue;
                }
                // Just hidden: the active tab's loop ticked it until now.
                None => now + HIDDEN_TICK_MAX,
            };
            slot.hidden_tick.set(Some((now, next)));
            soonest = Some(soonest.map_or(next, |t| t.min(next)));
        }
        (soonest, drew)
    }
}
