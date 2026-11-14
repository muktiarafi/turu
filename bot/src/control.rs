//! Stop and pause for the running script (VibeSkua: `ShouldExit`, `ScriptPauseEvent`), and the
//! pulse that wakes a wait when the game reports what it waits for (docs/HOST_SWF_PROPOSAL.md
//! M5, docs/EVENT_WAITS_PROPOSAL.md's `Pulse`).

use crate::error::{BotError, BotResult};
use parking_lot::{Condvar, Mutex};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

thread_local! {
    /// Set on threads that belong to the running script (the script thread and the
    /// script-owned helper loops). Bot calls on those threads honour Stop and Pause.
    static SCRIPT_THREAD: Cell<bool> = const { Cell::new(false) };
}

/// Marks the current thread as belonging to the script.
pub fn mark_script_thread(value: bool) {
    SCRIPT_THREAD.with(|c| c.set(value));
}

pub fn is_script_thread() -> bool {
    SCRIPT_THREAD.with(Cell::get)
}

/// What a wait can wake on: a set of topics, each fired by a packet or a turu.swf event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Topics(u32);

impl Topics {
    pub const NONE: Self = Self(0);
    /// `buyItem`, `sellItem`, `loadBank` replies (`state.signals`).
    pub const SIGNALS: Self = Self(1 << 0);
    /// turu.swf `mapLoaded`: a map finished loading (`mapLoadInProgress` went false).
    pub const MAP: Self = Self(1 << 1);
    /// turu.swf `cellChanged`: `world.strFrame` changed.
    pub const CELL: Self = Self(1 << 2);
    /// turu.swf `arrived`: the avatar stopped walking.
    pub const ARRIVED: Self = Self(1 << 3);
    /// turu.swf `loading`: the loading screen (`mcConnDetail`) came up or went.
    pub const LOADING: Self = Self(1 << 4);
    /// turu.swf `world`: the world came or went (login, logout, lost connection).
    pub const SESSION: Self = Self(1 << 5);

    const COUNT: usize = 6;

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    fn has(self, index: usize) -> bool {
        self.0 & (1 << index) != 0
    }
}

impl std::ops::BitOr for Topics {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// The topics' generations when a wait looked at the game ([`ScriptControl::mark`]).
#[derive(Debug, Clone, Copy)]
pub struct Mark([u64; Topics::COUNT]);

#[derive(Default)]
struct Inner {
    paused: bool,
    /// How often each topic fired.
    generations: [u64; Topics::COUNT],
}

#[derive(Default)]
pub struct ScriptControl {
    stop: AtomicBool,
    inner: Mutex<Inner>,
    wake: Condvar,
}

impl ScriptControl {
    /// True when the running script should exit (Stop was pressed).
    pub fn should_exit(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.inner.lock().paused = false;
        self.wake.notify_all();
    }

    /// Called when a new script starts.
    pub fn reset(&self) {
        self.stop.store(false, Ordering::Release);
        self.inner.lock().paused = false;
    }

    pub fn set_paused(&self, paused: bool) {
        self.inner.lock().paused = paused;
        self.wake.notify_all();
    }

    pub fn is_paused(&self) -> bool {
        self.inner.lock().paused
    }

    /// On a script thread: blocks while paused, then fails if the script must stop.
    /// Elsewhere (UI, background timer) it never blocks or fails.
    pub fn check(&self) -> BotResult<()> {
        if !is_script_thread() {
            return Ok(());
        }
        let mut inner = self.inner.lock();
        while inner.paused && !self.should_exit() {
            self.wake.wait(&mut inner);
        }
        drop(inner);
        if self.should_exit() {
            Err(BotError::Stopped)
        } else {
            Ok(())
        }
    }

    /// Sleeps `duration`, waking early (with [`BotError::Stopped`]) when the script stops.
    pub fn sleep(&self, duration: Duration) -> BotResult<()> {
        self.check()?;
        if !is_script_thread() {
            std::thread::sleep(duration);
            return Ok(());
        }
        let deadline = Instant::now() + duration;
        let mut inner = self.inner.lock();
        loop {
            if self.should_exit() {
                return Err(BotError::Stopped);
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            self.wake.wait_for(&mut inner, deadline - now);
        }
        drop(inner);
        self.check()
    }

    /// Wakes every wait on one of `topics`. From packet and event handlers on the player's
    /// thread: holds the lock for a few instructions and never waits.
    pub fn fire(&self, topics: Topics) {
        let mut inner = self.inner.lock();
        for (index, generation) in inner.generations.iter_mut().enumerate() {
            if topics.has(index) {
                *generation += 1;
            }
        }
        drop(inner);
        self.wake.notify_all();
    }

    /// The generations now. A wait takes it *before* it looks at the game, so whatever fires
    /// between the look and the sleep still wakes it.
    pub fn mark(&self) -> Mark {
        Mark(self.inner.lock().generations)
    }

    /// One of `topics` fired after `mark`.
    pub fn fired_since(&self, mark: &Mark, topics: Topics) -> bool {
        let inner = self.inner.lock();
        (0..Topics::COUNT).any(|i| topics.has(i) && inner.generations[i] != mark.0[i])
    }

    /// Sleeps until `deadline`, or until one of `topics` fired after `mark` (then true). On a
    /// script thread Stop ends it with [`BotError::Stopped`] and Pause holds it, as
    /// [`ScriptControl::sleep`]; elsewhere (the background loops, Butler) neither does.
    pub fn sleep_until(&self, deadline: Instant, topics: Topics, mark: &Mark) -> BotResult<bool> {
        self.check()?;
        let script = is_script_thread();
        let mut inner = self.inner.lock();
        loop {
            if script && self.should_exit() {
                return Err(BotError::Stopped);
            }
            let fired =
                (0..Topics::COUNT).any(|i| topics.has(i) && inner.generations[i] != mark.0[i]);
            if fired {
                drop(inner);
                self.check()?;
                return Ok(true);
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            self.wake.wait_for(&mut inner, deadline - now);
        }
        drop(inner);
        self.check()?;
        Ok(false)
    }
}
