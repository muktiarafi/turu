//! Stop and pause for the running script (VibeSkua: `ShouldExit`, `ScriptPauseEvent`).

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

#[derive(Default)]
pub struct ScriptControl {
    stop: AtomicBool,
    paused: Mutex<bool>,
    wake: Condvar,
}

impl ScriptControl {
    /// True when the running script should exit (Stop was pressed).
    pub fn should_exit(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
        *self.paused.lock() = false;
        self.wake.notify_all();
    }

    /// Called when a new script starts.
    pub fn reset(&self) {
        self.stop.store(false, Ordering::Release);
        *self.paused.lock() = false;
    }

    pub fn set_paused(&self, paused: bool) {
        *self.paused.lock() = paused;
        self.wake.notify_all();
    }

    pub fn is_paused(&self) -> bool {
        *self.paused.lock()
    }

    /// On a script thread: blocks while paused, then fails if the script must stop.
    /// Elsewhere (UI, background timer) it never blocks or fails.
    pub fn check(&self) -> BotResult<()> {
        if !is_script_thread() {
            return Ok(());
        }
        let mut paused = self.paused.lock();
        while *paused && !self.should_exit() {
            self.wake.wait(&mut paused);
        }
        drop(paused);
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
        let mut paused = self.paused.lock();
        loop {
            if self.should_exit() {
                return Err(BotError::Stopped);
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            self.wake.wait_for(&mut paused, deadline - now);
        }
        drop(paused);
        self.check()
    }
}
