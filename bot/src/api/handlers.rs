//! `bot.handlers`: functions the script runs while it makes bot calls (VibeSkua `Bot.Handlers`,
//! run by `ScriptInterface.RunScriptHandlers`).
//!
//! The functions are JavaScript closures, so they stay in the script; the engine keeps their
//! timing. After each bot call, [`HandlerRunner::take_due`] names the handlers that are due and
//! `__turu` returns the names. `bot/js/turu.ts` runs them and reports back with `handlers.finish`.

use super::{Args, ok};
use crate::Bot;
use crate::control::ClockTime;
use crate::error::{BotError, BotResult};
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// VibeSkua's timer runs the handlers every 20 ms, so a shorter interval wouldn't run any sooner.
const MIN_INTERVAL: Duration = Duration::from_millis(20);

struct Entry {
    name: String,
    interval: Duration,
    next: ClockTime,
    once: bool,
    running: bool,
}

#[derive(Default)]
pub struct HandlerRunner {
    entries: Mutex<Vec<Entry>>,
}

impl HandlerRunner {
    /// Adds a handler at `now` (the bot's clock), replacing the one with the same name.
    pub fn add(&self, name: &str, interval: Duration, once: bool, now: ClockTime) {
        let interval = interval.max(MIN_INTERVAL);
        let mut entries = self.entries.lock();
        entries.retain(|e| e.name != name);
        entries.push(Entry {
            name: name.to_owned(),
            interval,
            next: now + interval,
            once,
            running: false,
        });
    }

    /// Removes a handler; false when there was none.
    pub fn remove(&self, name: &str) -> bool {
        let mut entries = self.entries.lock();
        let before = entries.len();
        entries.retain(|e| e.name != name);
        entries.len() != before
    }

    pub fn clear(&self) {
        self.entries.lock().clear();
    }

    /// Names of the handlers due at `now`. Each is marked running until [`finish`](Self::finish),
    /// so a handler never runs inside its own run.
    pub fn take_due(&self, now: ClockTime) -> Vec<String> {
        self.entries
            .lock()
            .iter_mut()
            .filter(|e| !e.running && now >= e.next)
            .map(|e| {
                e.running = true;
                e.name.clone()
            })
            .collect()
    }

    /// When the next handler that isn't running falls due, or `None` without one. A script's
    /// `bot.sleep` ends then, so the handlers keep their interval while the script sleeps.
    pub fn next_due(&self) -> Option<ClockTime> {
        self.entries
            .lock()
            .iter()
            .filter(|e| !e.running)
            .map(|e| e.next)
            .min()
    }

    /// The script ran the handler. It is due again one interval after the run ended (VibeSkua's
    /// `LimitedRun` stamps the end too), or removed when it was added as `once`. Returns whether
    /// the handler is still registered. `now` is when the run ended, on the bot's clock.
    pub fn finish(&self, name: &str, now: ClockTime) -> bool {
        let mut entries = self.entries.lock();
        let Some(index) = entries.iter().position(|e| e.name == name) else {
            return false;
        };
        let entry = &mut entries[index];
        // A `once` entry that a run replaced (registered again under the same name) hasn't run yet.
        if entry.once && entry.running {
            entries.remove(index);
            return false;
        }
        entry.running = false;
        entry.next = now + entry.interval;
        true
    }
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    let handlers = &bot.handlers;
    match name {
        "add" => {
            let interval = Duration::from_millis(args.i64(1)?.max(0) as u64);
            handlers.add(
                args.str(0)?,
                interval,
                args.bool_or(2, false),
                bot.control.now(),
            );
            ok(())
        }
        "remove" => ok(handlers.remove(args.str(0)?)),
        "clear" => {
            handlers.clear();
            ok(())
        }
        "finish" => ok(handlers.finish(args.str(0)?, bot.control.now())),
        _ => Err(BotError::UnknownMethod(format!("handlers.{name}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_handler_is_due_once_per_interval() {
        let handlers = HandlerRunner::default();
        let t0 = ClockTime::default();
        handlers.add("a", ms(100), false, t0);

        assert!(handlers.take_due(t0).is_empty());
        assert_eq!(handlers.take_due(t0 + ms(200)), ["a"]);
        // Running: not due again until it finishes.
        assert!(handlers.take_due(t0 + ms(200)).is_empty());

        assert!(handlers.finish("a", t0 + ms(200)));
        assert!(handlers.take_due(t0 + ms(250)).is_empty());
        assert_eq!(handlers.take_due(t0 + ms(400)), ["a"]);
    }

    #[test]
    fn next_due_skips_running_handlers() {
        let handlers = HandlerRunner::default();
        let t0 = ClockTime::default();
        assert_eq!(handlers.next_due(), None);

        handlers.add("slow", Duration::from_secs(60), false, t0);
        handlers.add("fast", ms(20), false, t0);
        assert_eq!(handlers.next_due(), Some(t0 + ms(20)));

        // "fast" runs: the next one due is "slow".
        assert_eq!(handlers.take_due(t0 + ms(200)), ["fast"]);
        assert_eq!(handlers.next_due(), Some(t0 + Duration::from_secs(60)));
    }

    #[test]
    fn a_once_handler_is_removed_when_it_finishes() {
        let handlers = HandlerRunner::default();
        let t0 = ClockTime::default();
        handlers.add("once", ms(20), true, t0);

        assert_eq!(handlers.take_due(t0 + ms(200)), ["once"]);
        assert!(!handlers.finish("once", t0 + ms(200)));
        assert!(handlers.take_due(t0 + ms(1200)).is_empty());
    }

    #[test]
    fn a_once_handler_that_was_registered_again_during_its_run_stays() {
        let handlers = HandlerRunner::default();
        let t0 = ClockTime::default();
        handlers.add("once", ms(20), true, t0);
        assert_eq!(handlers.take_due(t0 + ms(200)), ["once"]);

        // The run registers the name again; that new registration hasn't run yet.
        handlers.add("once", ms(20), true, t0 + ms(200));
        assert!(handlers.finish("once", t0 + ms(200)));
        assert_eq!(handlers.take_due(t0 + ms(1200)), ["once"]);
    }

    #[test]
    fn adding_a_name_again_replaces_the_handler() {
        let handlers = HandlerRunner::default();
        let t0 = ClockTime::default();
        handlers.add("a", Duration::from_secs(60), false, t0);
        handlers.add("a", ms(20), false, t0);

        assert_eq!(handlers.take_due(t0 + ms(200)), ["a"]);
        assert!(handlers.remove("a"));
        assert!(!handlers.remove("a"));
    }

    #[test]
    fn the_interval_has_a_floor() {
        let handlers = HandlerRunner::default();
        let t0 = ClockTime::default();
        handlers.add("fast", Duration::ZERO, false, t0);
        assert_eq!(handlers.take_due(t0 + ms(200)), ["fast"]);
        handlers.finish("fast", t0 + ms(200));

        // Due again one floor interval after the run, not at once.
        assert!(handlers.take_due(t0 + ms(200)).is_empty());
        assert_eq!(handlers.take_due(t0 + ms(220)), ["fast"]);
    }
}
