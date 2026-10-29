//! `bot.handlers`: functions the script runs while it makes bot calls (VibeSkua `Bot.Handlers`,
//! run by `ScriptInterface.RunScriptHandlers`).
//!
//! The functions are JavaScript closures, so they stay in the script; the engine keeps their
//! timing. After each bot call, [`HandlerRunner::take_due`] names the handlers that are due and
//! `__turu` returns the names. `bot/js/turu.ts` runs them and reports back with `handlers.finish`.

use super::{Args, ok};
use crate::Bot;
use crate::error::{BotError, BotResult};
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// VibeSkua's timer runs the handlers every 20 ms, so a shorter interval wouldn't run any sooner.
const MIN_INTERVAL: Duration = Duration::from_millis(20);

struct Entry {
    name: String,
    interval: Duration,
    next: Instant,
    once: bool,
    running: bool,
}

#[derive(Default)]
pub struct HandlerRunner {
    entries: Mutex<Vec<Entry>>,
}

impl HandlerRunner {
    /// Adds a handler, replacing the one with the same name.
    pub fn add(&self, name: &str, interval: Duration, once: bool) {
        let interval = interval.max(MIN_INTERVAL);
        let mut entries = self.entries.lock();
        entries.retain(|e| e.name != name);
        entries.push(Entry {
            name: name.to_owned(),
            interval,
            next: Instant::now() + interval,
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
    pub fn take_due(&self, now: Instant) -> Vec<String> {
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

    /// The script ran the handler. It is due again one interval after the run ended (VibeSkua's
    /// `LimitedRun` stamps the end too), or removed when it was added as `once`. Returns whether
    /// the handler is still registered.
    pub fn finish(&self, name: &str) -> bool {
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
        entry.next = Instant::now() + entry.interval;
        true
    }
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    let handlers = &bot.handlers;
    match name {
        "add" => {
            let interval = Duration::from_millis(args.i64(1)?.max(0) as u64);
            handlers.add(args.str(0)?, interval, args.bool_or(2, false));
            ok(())
        }
        "remove" => ok(handlers.remove(args.str(0)?)),
        "clear" => {
            handlers.clear();
            ok(())
        }
        "finish" => ok(handlers.finish(args.str(0)?)),
        _ => Err(BotError::UnknownMethod(format!("handlers.{name}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handler_is_due_once_per_interval() {
        let handlers = HandlerRunner::default();
        handlers.add("a", Duration::from_millis(100), false);

        assert!(handlers.take_due(Instant::now()).is_empty());
        let later = Instant::now() + Duration::from_millis(200);
        assert_eq!(handlers.take_due(later), ["a"]);
        // Running: not due again until it finishes.
        assert!(handlers.take_due(later).is_empty());

        assert!(handlers.finish("a"));
        assert!(handlers.take_due(Instant::now()).is_empty());
        assert_eq!(
            handlers.take_due(Instant::now() + Duration::from_millis(200)),
            ["a"]
        );
    }

    #[test]
    fn a_once_handler_is_removed_when_it_finishes() {
        let handlers = HandlerRunner::default();
        handlers.add("once", Duration::from_millis(20), true);

        let later = Instant::now() + Duration::from_millis(200);
        assert_eq!(handlers.take_due(later), ["once"]);
        assert!(!handlers.finish("once"));
        assert!(handlers.take_due(later + Duration::from_secs(1)).is_empty());
    }

    #[test]
    fn a_once_handler_that_was_registered_again_during_its_run_stays() {
        let handlers = HandlerRunner::default();
        handlers.add("once", Duration::from_millis(20), true);
        let later = Instant::now() + Duration::from_millis(200);
        assert_eq!(handlers.take_due(later), ["once"]);

        // The run registers the name again; that new registration hasn't run yet.
        handlers.add("once", Duration::from_millis(20), true);
        assert!(handlers.finish("once"));
        assert_eq!(handlers.take_due(later + Duration::from_secs(1)), ["once"]);
    }

    #[test]
    fn adding_a_name_again_replaces_the_handler() {
        let handlers = HandlerRunner::default();
        handlers.add("a", Duration::from_secs(60), false);
        handlers.add("a", Duration::from_millis(20), false);

        assert_eq!(
            handlers.take_due(Instant::now() + Duration::from_millis(200)),
            ["a"]
        );
        assert!(handlers.remove("a"));
        assert!(!handlers.remove("a"));
    }

    #[test]
    fn the_interval_has_a_floor() {
        let handlers = HandlerRunner::default();
        handlers.add("fast", Duration::ZERO, false);
        let later = Instant::now() + Duration::from_millis(200);
        assert_eq!(handlers.take_due(later), ["fast"]);
        handlers.finish("fast");

        // Due again one floor interval after the run, not at once.
        assert!(handlers.take_due(Instant::now()).is_empty());
    }
}
