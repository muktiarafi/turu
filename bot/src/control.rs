//! Stop and pause for the running script (VibeSkua: `ShouldExit`, `ScriptPauseEvent`), the
//! pulse that wakes a wait when the game reports what it waits for (docs/HOST_SWF_PROPOSAL.md
//! M5, docs/EVENT_WAITS_PROPOSAL.md's `Pulse`), and the bot's clock.
//!
//! **The clock** ([`ScriptControl::now`], [`ClockTime`]) is real time unless a test makes it
//! virtual ([`ScriptControl::set_virtual`], docs/REPLAY_TESTS_PROPOSAL.md, decision 2). Virtual
//! time doesn't pass by itself: it jumps to the next wake-up when every *participant* is waiting
//! on the clock. Participants are the threads that run the script (the script thread, its helper
//! loops: [`ScriptControl::participate`]) and a replay's own thread; others (the background timer,
//! Butler, the test) run in real time beside it. A participant counts as running from the moment
//! anything wakes it (a time jump, a topic, Stop, Pause, [`ScriptControl::notify`]) until it
//! decides to wait again, so time never jumps past a thread that is about to act.

use crate::error::{BotError, BotResult};
use parking_lot::{Condvar, Mutex, MutexGuard};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

thread_local! {
    /// Set on threads that belong to the running script (the script thread and the
    /// script-owned helper loops). Bot calls on those threads honour Stop and Pause.
    #[allow(clippy::missing_const_for_thread_local, reason = "it is const: clippy misreads Android's expansion")]
    static SCRIPT_THREAD: Cell<bool> = const { Cell::new(false) };
    /// Set while the thread takes part in a virtual clock ([`ScriptControl::participate`]).
    #[allow(clippy::missing_const_for_thread_local, reason = "it is const: clippy misreads Android's expansion")]
    static PARTICIPANT: Cell<bool> = const { Cell::new(false) };
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
    /// turu.swf `kicked`: the login screen's warning (`isKicked`) came up or went.
    pub const KICKED: Self = Self(1 << 6);

    const COUNT: usize = 7;

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

/// A moment on the bot's clock: the time since the clock started, real or virtual. Its own type,
/// so it can't be compared with a real [`Instant`] by mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ClockTime(Duration);

impl ClockTime {
    /// The time since the clock started.
    pub fn since_start(self) -> Duration {
        self.0
    }

    /// How long after `earlier` this is; zero when it isn't after it.
    pub fn since(self, earlier: Self) -> Duration {
        self.0.saturating_sub(earlier.0)
    }
}

impl std::ops::Add<Duration> for ClockTime {
    type Output = Self;
    fn add(self, d: Duration) -> Self {
        Self(self.0.saturating_add(d))
    }
}

/// A participant waiting on the virtual clock.
struct Waiter {
    id: u64,
    deadline: Option<Duration>,
    /// Counted in [`Virtual::running`]: it was woken and hasn't decided to wait again.
    counted: bool,
    thread: String,
}

/// The virtual clock's state, under [`ScriptControl`]'s lock.
struct Virtual {
    now: Duration,
    /// Participants not waiting on the clock (running, or woken and about to look).
    running: usize,
    waiters: Vec<Waiter>,
    next_id: u64,
}

impl Virtual {
    /// Counts every waiter as running again: they were woken and will look before waiting again.
    fn wake_all(&mut self) {
        for waiter in &mut self.waiters {
            if !waiter.counted {
                waiter.counted = true;
                self.running += 1;
            }
        }
    }
}

#[derive(Default)]
struct Inner {
    paused: bool,
    /// How often each topic fired.
    generations: [u64; Topics::COUNT],
    /// The virtual clock, when a test made the clock virtual.
    virt: Option<Virtual>,
}

pub struct ScriptControl {
    stop: AtomicBool,
    inner: Mutex<Inner>,
    wake: Condvar,
    /// When the clock started, in real time.
    origin: Instant,
    /// The same moment in milliseconds since the Unix epoch (`Date.now()` on a virtual clock).
    origin_unix_ms: u64,
    /// Raised by [`ScriptControl::notify`]: something a clock wait looks at changed.
    notified: AtomicU64,
}

impl Default for ScriptControl {
    fn default() -> Self {
        Self {
            stop: AtomicBool::new(false),
            inner: Mutex::default(),
            wake: Condvar::new(),
            origin: Instant::now(),
            origin_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
            notified: AtomicU64::new(0),
        }
    }
}

/// A thread taking part in a virtual clock, until it is dropped ([`ScriptControl::participate`]).
pub struct Participant {
    control: Option<Arc<ScriptControl>>,
}

/// A participant's place, counted, before its thread runs ([`ScriptControl::participate_for_new_thread`]).
pub struct Joining {
    control: Option<Arc<ScriptControl>>,
}

impl Joining {
    /// On the new thread: takes the place.
    pub fn enter(mut self) -> Participant {
        let control = self.control.take();
        if control.is_some() {
            PARTICIPANT.with(|p| p.set(true));
        }
        Participant { control }
    }
}

impl Drop for Joining {
    /// Never entered (the spawn failed): gives the place back.
    fn drop(&mut self) {
        if let Some(control) = self.control.take() {
            control.leave();
        }
    }
}

impl Drop for Participant {
    fn drop(&mut self) {
        if let Some(control) = self.control.take() {
            PARTICIPANT.with(|p| p.set(false));
            control.leave();
        }
    }
}

impl ScriptControl {
    /// True when the running script should exit (Stop was pressed).
    pub fn should_exit(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
        let mut inner = self.inner.lock();
        inner.paused = false;
        if let Some(v) = inner.virt.as_mut() {
            v.wake_all();
        }
        drop(inner);
        self.wake.notify_all();
    }

    /// Called when a new script starts.
    pub fn reset(&self) {
        self.stop.store(false, Ordering::Release);
        self.inner.lock().paused = false;
    }

    pub fn set_paused(&self, paused: bool) {
        let mut inner = self.inner.lock();
        inner.paused = paused;
        if let Some(v) = inner.virt.as_mut() {
            v.wake_all();
        }
        drop(inner);
        self.wake.notify_all();
    }

    pub fn is_paused(&self) -> bool {
        self.inner.lock().paused
    }

    // ---- The clock -------------------------------------------------------------------------

    /// Now, on the bot's clock.
    pub fn now(&self) -> ClockTime {
        match &self.inner.lock().virt {
            Some(v) => ClockTime(v.now),
            None => ClockTime(self.origin.elapsed()),
        }
    }

    /// Now, in milliseconds since the Unix epoch, on the bot's clock (`Date.now()` for scripts).
    pub fn unix_ms(&self) -> u64 {
        self.origin_unix_ms + self.now().since_start().as_millis() as u64
    }

    /// Whether the clock is virtual.
    pub fn is_virtual(&self) -> bool {
        self.inner.lock().virt.is_some()
    }

    /// Makes the clock virtual, from now on (for tests: a replay, the draft run). Call it before
    /// the script starts: participants already running aren't counted.
    #[doc(hidden)]
    pub fn set_virtual(&self) {
        let mut inner = self.inner.lock();
        if inner.virt.is_none() {
            inner.virt = Some(Virtual {
                now: self.origin.elapsed(),
                running: 0,
                waiters: Vec::new(),
                next_id: 0,
            });
        }
    }

    /// Takes part in the virtual clock until the guard is dropped: from the thread that runs the
    /// script, or a replay's. Nothing on a real clock.
    pub fn participate(self: &Arc<Self>) -> Participant {
        self.participate_for_new_thread().enter()
    }

    /// A place in the virtual clock for a thread about to be spawned: counted as running from now,
    /// so time can't jump before the thread runs. The thread calls [`Joining::enter`] first.
    pub fn participate_for_new_thread(self: &Arc<Self>) -> Joining {
        let mut inner = self.inner.lock();
        let Some(v) = inner.virt.as_mut() else {
            return Joining { control: None };
        };
        v.running += 1;
        drop(inner);
        Joining {
            control: Some(self.clone()),
        }
    }

    /// A participant is gone: no longer counted as running.
    fn leave(&self) {
        let mut inner = self.inner.lock();
        if let Some(v) = inner.virt.as_mut() {
            v.running = v.running.saturating_sub(1);
            // The others may all be waiting now: they look again, and the last moves time on.
            v.wake_all();
        }
        drop(inner);
        self.wake.notify_all();
    }

    /// Wakes the clock's waits to look again: something they wait for, outside the topics,
    /// changed (a worker's cancel flag, a replay's position).
    pub fn notify(&self) {
        self.notified.fetch_add(1, Ordering::AcqRel);
        let mut inner = self.inner.lock();
        if let Some(v) = inner.virt.as_mut() {
            v.wake_all();
        }
        drop(inner);
        self.wake.notify_all();
    }

    /// On a virtual clock, a participant spends `cost` of virtual time (a call into Flash: in the
    /// game each takes the player's thread about 0.8 ms). Without it a loop that polls the game
    /// without sleeping would hold the time still. Nothing on a real clock or another thread.
    pub fn spend(&self, cost: Duration) {
        if !PARTICIPANT.with(Cell::get) {
            return;
        }
        let mut inner = self.inner.lock();
        let Some(v) = &inner.virt else {
            return;
        };
        let deadline = ClockTime(v.now) + cost;
        self.clock_wait(&mut inner, Some(deadline), &mut |_| false);
    }

    /// Who waits on the virtual clock and who runs, for a test that hangs.
    pub fn clock_report(&self) -> String {
        let inner = self.inner.lock();
        let Some(v) = &inner.virt else {
            return "the clock is real".into();
        };
        let mut out = format!("virtual clock at {:?}, {} running", v.now, v.running);
        for w in &v.waiters {
            out.push_str(&format!(
                "\n  {} waits until {:?}{}",
                w.thread,
                w.deadline,
                if w.counted { " (woken)" } else { "" }
            ));
        }
        out
    }

    /// Waits until `deadline` (none: until woken) or until `woken` says so (looked at under the
    /// lock, after every wake). True when woken. On a virtual clock a participant's wait moves the
    /// time on when it is the last to wait; anyone else's waits in real time for the virtual
    /// deadline.
    fn clock_wait(
        &self,
        inner: &mut MutexGuard<'_, Inner>,
        deadline: Option<ClockTime>,
        woken: &mut dyn FnMut(&Inner) -> bool,
    ) -> bool {
        let participant = PARTICIPANT.with(Cell::get);
        if inner.virt.is_none() || !participant {
            loop {
                if woken(inner) {
                    return true;
                }
                let now = match &inner.virt {
                    Some(v) => v.now,
                    None => self.origin.elapsed(),
                };
                match deadline {
                    Some(d) if now >= d.0 => return false,
                    // On a virtual clock, look again now and then: the time jumps without this
                    // thread being told how far.
                    Some(d) if inner.virt.is_none() => {
                        self.wake.wait_for(inner, d.0 - now);
                    }
                    _ => {
                        self.wake.wait_for(inner, Duration::from_millis(100));
                    }
                }
            }
        }
        let id = {
            let v = inner.virt.as_mut().expect("virtual");
            let id = v.next_id;
            v.next_id += 1;
            v.waiters.push(Waiter {
                id,
                deadline: deadline.map(|d| d.0),
                counted: true,
                thread: std::thread::current().name().unwrap_or("?").to_owned(),
            });
            id
        };
        let result = loop {
            if woken(inner) {
                break true;
            }
            let v = inner.virt.as_mut().expect("virtual");
            if deadline.is_some_and(|d| v.now >= d.0) {
                break false;
            }
            // It waits: no longer running.
            let me = v.waiters.iter_mut().find(|w| w.id == id).expect("waiter");
            if me.counted {
                me.counted = false;
                v.running -= 1;
            }
            if v.running == 0 {
                // Everyone waits: the time jumps to the first wake-up, and they all look again.
                let next = v.waiters.iter().filter_map(|w| w.deadline).min();
                if let Some(next) = next {
                    v.now = v.now.max(next);
                    v.wake_all();
                    self.wake.notify_all();
                    continue;
                }
            }
            self.wake.wait(inner);
        };
        let v = inner.virt.as_mut().expect("virtual");
        let index = v.waiters.iter().position(|w| w.id == id).expect("waiter");
        if !v.waiters.remove(index).counted {
            v.running += 1;
        }
        result
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
            if self.is_virtual() && PARTICIPANT.with(Cell::get) {
                let deadline = self.now() + duration;
                let mut inner = self.inner.lock();
                self.clock_wait(&mut inner, Some(deadline), &mut |_| false);
                return Ok(());
            }
            std::thread::sleep(duration);
            return Ok(());
        }
        let deadline = self.now() + duration;
        let mut inner = self.inner.lock();
        let stopped = self.clock_wait(&mut inner, Some(deadline), &mut |_| self.should_exit());
        drop(inner);
        if stopped {
            return Err(BotError::Stopped);
        }
        self.check()
    }

    /// Waits until `deadline` or until `woken` (a condition outside the topics: a cancel flag),
    /// whoever raises it calling [`ScriptControl::notify`]. On a script thread Stop ends it with
    /// [`BotError::Stopped`]. True when woken.
    pub fn wait_until(
        &self,
        deadline: Option<ClockTime>,
        mut woken: impl FnMut() -> bool,
    ) -> BotResult<bool> {
        self.check()?;
        let script = is_script_thread();
        let mut inner = self.inner.lock();
        let mut stopped = false;
        let woke = self.clock_wait(&mut inner, deadline, &mut |_| {
            stopped = script && self.should_exit();
            stopped || woken()
        });
        drop(inner);
        if stopped {
            return Err(BotError::Stopped);
        }
        self.check()?;
        Ok(woke)
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
        if let Some(v) = inner.virt.as_mut() {
            v.wake_all();
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
    pub fn sleep_until(&self, deadline: ClockTime, topics: Topics, mark: &Mark) -> BotResult<bool> {
        self.check()?;
        let script = is_script_thread();
        let mut inner = self.inner.lock();
        let mut stopped = false;
        let fired = self.clock_wait(&mut inner, Some(deadline), &mut |inner| {
            stopped = script && self.should_exit();
            stopped
                || (0..Topics::COUNT).any(|i| topics.has(i) && inner.generations[i] != mark.0[i])
        });
        drop(inner);
        if stopped {
            return Err(BotError::Stopped);
        }
        self.check()?;
        Ok(fired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn virtual_control() -> Arc<ScriptControl> {
        let control = Arc::new(ScriptControl::default());
        control.set_virtual();
        control
    }

    /// Runs `f` on a participant thread, its place taken before it starts.
    fn spawn(
        control: &Arc<ScriptControl>,
        name: &str,
        f: impl FnOnce() + Send + 'static,
    ) -> std::thread::JoinHandle<()> {
        let joining = control.participate_for_new_thread();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let _p = joining.enter();
                f();
            })
            .unwrap()
    }

    #[test]
    fn a_virtual_sleep_takes_no_real_time() {
        let control = virtual_control();
        let start = control.now();
        let real = Instant::now();
        let c = control.clone();
        spawn(&control, "sleeper", move || {
            c.sleep(Duration::from_secs(3600)).unwrap()
        })
        .join()
        .unwrap();
        assert!(
            real.elapsed() < Duration::from_secs(2),
            "{:?}",
            real.elapsed()
        );
        assert_eq!(control.now().since(start), Duration::from_secs(3600));
    }

    #[test]
    fn sleepers_wake_in_the_order_of_their_deadlines() {
        let control = virtual_control();
        let start = control.now();
        let log = Arc::new(Mutex::new(Vec::new()));
        let threads: Vec<_> = [5u64, 2, 9]
            .into_iter()
            .map(|secs| {
                let (c, log) = (control.clone(), log.clone());
                spawn(&control, "sleeper", move || {
                    c.sleep(Duration::from_secs(secs)).unwrap();
                    log.lock().push((secs, c.now().since(start).as_secs()));
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(*log.lock(), [(2, 2), (5, 5), (9, 9)]);
    }

    /// The race the clock must not lose: a thread fires a topic and at once sleeps long. The
    /// waiter it woke counts as running from the fire, so time can't jump to the long sleep's end
    /// before the waiter looks.
    #[test]
    fn a_woken_waiter_runs_before_time_jumps_on() {
        for _ in 0..50 {
            let control = virtual_control();
            let start = control.now();
            let woke_at = Arc::new(Mutex::new(None));
            let (c, at) = (control.clone(), woke_at.clone());
            let mark = control.mark();
            let waiter = spawn(&control, "waiter", move || {
                let fired = c
                    .sleep_until(start + Duration::from_secs(1000), Topics::SIGNALS, &mark)
                    .unwrap();
                assert!(fired);
                *at.lock() = Some(c.now().since(start));
            });
            let c = control.clone();
            let firer = spawn(&control, "firer", move || {
                c.sleep(Duration::from_secs(1)).unwrap();
                c.fire(Topics::SIGNALS);
                c.sleep(Duration::from_secs(100)).unwrap();
            });
            waiter.join().unwrap();
            firer.join().unwrap();
            assert_eq!(*woke_at.lock(), Some(Duration::from_secs(1)));
        }
    }

    /// A participant that is busy (not waiting on the clock) holds the time still.
    #[test]
    fn a_busy_participant_holds_the_time() {
        let control = virtual_control();
        let start = control.now();
        let busy = control.participate();
        let c = control.clone();
        let sleeper = spawn(&control, "sleeper", move || {
            c.sleep(Duration::from_secs(5)).unwrap()
        });
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            control.now(),
            start,
            "time moved while a participant was busy"
        );
        assert!(!sleeper.is_finished());
        drop(busy);
        sleeper.join().unwrap();
        assert_eq!(control.now().since(start), Duration::from_secs(5));
    }

    /// Someone outside the clock (the game, a test) wakes a participant waiting with no deadline.
    #[test]
    fn notify_wakes_a_wait_without_a_deadline() {
        let control = virtual_control();
        let flag = Arc::new(AtomicBool::new(false));
        let (c, f) = (control.clone(), flag.clone());
        let waiter = spawn(&control, "waiter", move || {
            assert!(c.wait_until(None, || f.load(Ordering::Acquire)).unwrap());
        });
        std::thread::sleep(Duration::from_millis(50));
        flag.store(true, Ordering::Release);
        control.notify();
        waiter.join().unwrap();
    }

    #[test]
    fn the_real_clock_is_untouched() {
        let control = ScriptControl::default();
        assert!(!control.is_virtual());
        let before = control.now();
        let real = Instant::now();
        control.sleep(Duration::from_millis(30)).unwrap();
        assert!(real.elapsed() >= Duration::from_millis(30));
        assert!(control.now().since(before) >= Duration::from_millis(30));
    }
}
