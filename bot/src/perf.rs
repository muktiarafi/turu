//! What a bot asks of its game: every Flash call, by the thread that made it and the callback
//! (with its path, for the callbacks that take one), how long the caller waited for it and how
//! long the player took to run it; and how long the tab's player spent ticking. Every tab's game
//! runs, and every call is answered, on one thread (the event loop, or the windowless loop), so
//! these say what keeps that thread busy.
//!
//! Always collected (a lock and a map lookup per call, next to a trip to another thread). Read
//! with the control API's `GET /debug/perf`, or logged every n seconds for each tab with
//! `TURU_PERF_LOG=<n>`.

use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// Callbacks whose first argument is a game path: counted per path.
const PATH_CALLS: &[&str] = &[
    "getGameObject",
    "getGameObjectS",
    "getGameObjectKey",
    "setGameObject",
    "setGameObjectKey",
    "getArrayObject",
    "setArrayObject",
    "callGameFunction",
    "callGameFunction0",
    "selectArrayObjects",
    "isNull",
];

/// The callback's first argument is a game path, counted apart.
pub fn takes_path(name: &str) -> bool {
    PATH_CALLS.contains(&name)
}

/// Calls counted apart per thread; past this, a thread's other calls share one row (scripts
/// can read any path).
const MAX_ROWS_PER_THREAD: usize = 400;
const OTHER: &str = "(other)";

pub struct PerfStats {
    since: Instant,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// Thread name -> call label -> totals.
    calls: HashMap<String, HashMap<String, CallTotals>>,
    ticks: Totals,
}

#[derive(Default, Clone, Copy)]
struct CallTotals {
    calls: u64,
    errors: u64,
    wait: Duration,
    wait_max: Duration,
    run: Duration,
    reply_bytes: u64,
}

#[derive(Default, Clone, Copy)]
struct Totals {
    count: u64,
    total: Duration,
    max: Duration,
}

impl Default for PerfStats {
    fn default() -> Self {
        Self {
            since: Instant::now(),
            inner: Mutex::default(),
        }
    }
}

impl PerfStats {
    /// A call that went to the player: `wait` from asking to the answer, `run` the part the
    /// player took (`None` when the host can't tell), `reply` the answer (`None` on failure).
    pub fn record_call(
        &self,
        name: &str,
        path: Option<&str>,
        wait: Duration,
        run: Option<Duration>,
        reply: Option<usize>,
    ) {
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("(unnamed)");
        let mut inner = self.inner.lock();
        let rows = match inner.calls.get_mut(thread) {
            Some(rows) => rows,
            None => inner.calls.entry(thread.to_owned()).or_default(),
        };
        let label = match path {
            Some(path) if takes_path(name) => format!("{name}({path})"),
            _ => name.to_owned(),
        };
        let label = if rows.len() >= MAX_ROWS_PER_THREAD && !rows.contains_key(&label) {
            OTHER.to_owned()
        } else {
            label
        };
        let row = rows.entry(label).or_default();
        row.calls += 1;
        row.wait += wait;
        row.wait_max = row.wait_max.max(wait);
        row.run += run.unwrap_or_default();
        match reply {
            Some(bytes) => row.reply_bytes += bytes as u64,
            None => row.errors += 1,
        }
    }

    /// The tab's player ticked for `took`.
    pub fn record_tick(&self, took: Duration) {
        let mut inner = self.inner.lock();
        let ticks = &mut inner.ticks;
        ticks.count += 1;
        ticks.total += took;
        ticks.max = ticks.max.max(took);
    }

    /// Everything since the bot started.
    pub fn snapshot(&self) -> PerfSnapshot {
        let inner = self.inner.lock();
        let mut calls: Vec<CallRow> = inner
            .calls
            .iter()
            .flat_map(|(thread, rows)| {
                rows.iter().map(|(call, t)| CallRow {
                    thread: thread.clone(),
                    call: call.clone(),
                    calls: t.calls,
                    errors: t.errors,
                    wait_ms: ms(t.wait),
                    wait_max_ms: ms(t.wait_max),
                    run_ms: ms(t.run),
                    reply_bytes: t.reply_bytes,
                })
            })
            .collect();
        calls.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.call.cmp(&b.call)));
        PerfSnapshot {
            seconds: self.since.elapsed().as_secs_f64(),
            ticks: inner.ticks.count,
            tick_ms: ms(inner.ticks.total),
            tick_max_ms: ms(inner.ticks.max),
            calls,
        }
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// [`PerfStats`] at one moment: totals since the bot started (times in milliseconds).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfSnapshot {
    /// Since the bot started.
    pub seconds: f64,
    /// The tab's player's ticks and the time they took.
    pub ticks: u64,
    pub tick_ms: f64,
    pub tick_max_ms: f64,
    /// Per thread and call, the most made first.
    pub calls: Vec<CallRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRow {
    /// The thread that made the calls (`Script Thread`, `ScriptInterface` for the 20 ms timer,
    /// `Skill Timer`, ...).
    pub thread: String,
    /// The callback, with its path for the ones that take one: `getGameObject(world.myAvatar)`.
    pub call: String,
    pub calls: u64,
    pub errors: u64,
    /// From asking to the answer, in all: the trip to the player's thread, waiting there
    /// behind ticks and other calls, and running.
    pub wait_ms: f64,
    pub wait_max_ms: f64,
    /// The part the player took to run them.
    pub run_ms: f64,
    pub reply_bytes: u64,
}

impl PerfSnapshot {
    /// What happened between `earlier` and this one (the maxima stay this one's: since start).
    pub fn since(&self, earlier: &PerfSnapshot) -> PerfSnapshot {
        let before: HashMap<(&str, &str), &CallRow> = earlier
            .calls
            .iter()
            .map(|r| ((r.thread.as_str(), r.call.as_str()), r))
            .collect();
        let mut calls: Vec<CallRow> = self
            .calls
            .iter()
            .filter_map(|r| {
                let Some(b) = before.get(&(r.thread.as_str(), r.call.as_str())) else {
                    return Some(r.clone());
                };
                (r.calls > b.calls).then(|| CallRow {
                    calls: r.calls - b.calls,
                    errors: r.errors.saturating_sub(b.errors),
                    wait_ms: r.wait_ms - b.wait_ms,
                    run_ms: r.run_ms - b.run_ms,
                    reply_bytes: r.reply_bytes.saturating_sub(b.reply_bytes),
                    ..r.clone()
                })
            })
            .collect();
        calls.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.call.cmp(&b.call)));
        PerfSnapshot {
            seconds: self.seconds - earlier.seconds,
            ticks: self.ticks.saturating_sub(earlier.ticks),
            tick_ms: self.tick_ms - earlier.tick_ms,
            tick_max_ms: self.tick_max_ms,
            calls,
        }
    }

    pub fn call_count(&self) -> u64 {
        self.calls.iter().map(|r| r.calls).sum()
    }

    /// Milliseconds the player's thread spent on this bot: ticking its player and running its
    /// calls.
    pub fn busy_ms(&self) -> f64 {
        self.tick_ms + self.calls.iter().map(|r| r.run_ms).sum::<f64>()
    }

    /// A few lines for the log: the player thread's time per second, the calls per second and
    /// their average wait, then the `top` rows by calls.
    pub fn report(&self, label: &str, top: usize) -> String {
        let secs = self.seconds.max(0.001);
        let calls = self.call_count();
        let wait: f64 = self.calls.iter().map(|r| r.wait_ms).sum();
        let run: f64 = self.calls.iter().map(|r| r.run_ms).sum();
        let mut out = format!(
            "{label}: player thread {:.0} ms/s (ticks {:.0}, calls {:.0}); {:.1} calls/s, {:.2} ms wait avg",
            self.busy_ms() / secs,
            self.tick_ms / secs,
            run / secs,
            calls as f64 / secs,
            if calls > 0 { wait / calls as f64 } else { 0.0 },
        );
        for r in self.calls.iter().take(top) {
            let n = r.calls as f64;
            let _ = write!(
                out,
                "\n  {:>6.1}/s  wait {:>6.2} ms  run {:>5.2} ms  {:>7} B  {:<18} {}{}",
                n / secs,
                r.wait_ms / n,
                r.run_ms / n,
                r.reply_bytes / r.calls,
                r.thread,
                r.call,
                if r.errors > 0 {
                    format!("  ({} failed)", r.errors)
                } else {
                    String::new()
                },
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_are_counted_per_thread_and_path_and_diffed() {
        let stats = PerfStats::default();
        let ms = Duration::from_millis;
        stats.record_call(
            "getGameObject",
            Some("world.a"),
            ms(4),
            Some(ms(1)),
            Some(10),
        );
        stats.record_call("getGameObject", Some("world.a"), ms(2), Some(ms(1)), None);
        stats.record_call("getGameObject", Some("world.b"), ms(1), None, Some(3));
        // Not a path callback: counted by name.
        stats.record_call("walkTo", Some("1"), ms(1), None, Some(4));
        stats.record_tick(ms(5));
        let first = stats.snapshot();
        let a = &first.calls[0];
        assert_eq!(a.call, "getGameObject(world.a)");
        assert_eq!((a.calls, a.errors, a.reply_bytes), (2, 1, 10));
        assert!((a.wait_ms - 6.0).abs() < 0.01 && (a.wait_max_ms - 4.0).abs() < 0.01);
        assert!(first.calls.iter().any(|r| r.call == "walkTo"));
        assert!((first.busy_ms() - 7.0).abs() < 0.01);

        stats.record_call("getGameObject", Some("world.b"), ms(1), None, Some(3));
        let window = stats.snapshot().since(&first);
        assert_eq!(window.calls.len(), 1);
        assert_eq!(window.calls[0].call, "getGameObject(world.b)");
        assert_eq!((window.calls[0].calls, window.ticks), (1, 0));
        assert!(window.report("tab", 5).contains("getGameObject(world.b)"));
    }

    #[test]
    fn a_thread_reading_many_paths_shares_a_row_past_the_cap() {
        let stats = PerfStats::default();
        for i in 0..MAX_ROWS_PER_THREAD + 5 {
            stats.record_call(
                "getGameObject",
                Some(&format!("p{i}")),
                Duration::ZERO,
                None,
                Some(1),
            );
        }
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.calls.len(), MAX_ROWS_PER_THREAD + 1);
        assert_eq!(snapshot.calls[0].call, OTHER);
        assert_eq!(snapshot.calls[0].calls, 5);
    }
}
