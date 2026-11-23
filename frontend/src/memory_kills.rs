//! Turu: Android's process tabs after a kill for memory (docs/TAB_PROCESSES_PROPOSAL.md, "Risks",
//! restart storms). A slot Android ends for memory comes straight back (`START_STICKY`), logs in
//! and runs its script again (`android/src/restart.rs`); while memory is still short, that can push
//! out another slot, which comes back in turn. So each slot's kills for memory are counted in the
//! data folder's `crash/memory-kills-c<N>.log` (a kill's time, seconds since 1970, one a line), and
//! after more than [`RESTARTS`] in [`WINDOW`] the slot is held: it comes back at the login screen,
//! its script loaded but not started, and says why, until it's started again by hand or its tab
//! closes. The hold is kept in the file (`held <kills>`), not worked out from the window again: a
//! held slot killed again later still has no more memory to log in with.
//!
//! The rules are here, apart from Android, so the host's tests run them; the Android crate reads
//! how the slot's process last ended and keeps the file (`android/src/memory_kills.rs`).

use std::path::{Path, PathBuf};
use std::time::Duration;

/// A slot comes back by itself after a kill for memory at most this many times in [`WINDOW`]:
/// the next kill holds it.
pub const RESTARTS: usize = 3;
pub const WINDOW: Duration = Duration::from_secs(30 * 60);

/// `ApplicationExitInfo.REASON_SIGNALED`.
pub const REASON_SIGNALED: i32 = 2;
/// `ApplicationExitInfo.REASON_LOW_MEMORY`.
pub const REASON_LOW_MEMORY: i32 = 3;
/// `SIGKILL`.
const SIGKILL: i32 = 9;

/// Whether a process's exit (Android's `ApplicationExitInfo` reason and status) was a kill for
/// memory: Android's low-memory killer says so (`REASON_LOW_MEMORY`); a plain `SIGKILL` (phones
/// whose killer doesn't report itself) counts when the phone is still `low` on memory as the slot
/// comes back, so a battery saver's kill isn't called one. `any_kill`: `--test-memory-kills`,
/// every `SIGKILL` (an emulator can't be made to kill for memory).
pub fn memory_kill(reason: i32, status: i32, low: bool, any_kill: bool) -> bool {
    reason == REASON_LOW_MEMORY || (reason == REASON_SIGNALED && status == SIGKILL && (low || any_kill))
}

/// Slot `slot`'s file, in the crash folder.
pub fn file(crash_dir: &Path, slot: u32) -> PathBuf {
    crash_dir.join(format!("memory-kills-c{slot}.log"))
}

/// Forgets slot `slot`'s kills and its hold: started again by hand, or its tab closed.
pub fn clear(crash_dir: &Path, slot: u32) {
    match std::fs::remove_file(file(crash_dir, slot)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            tracing::warn!("Slot {slot}: couldn't forget its kills for memory: {e}");
        }
        _ => {}
    }
}

/// A slot's kills for memory, as its file keeps them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Kills {
    /// The kills' times (seconds since 1970).
    pub times: Vec<u64>,
    /// Held after this many kills in [`WINDOW`].
    pub held: Option<usize>,
}

impl Kills {
    pub fn parse(text: &str) -> Self {
        let mut kills = Self::default();
        for line in text.lines().map(str::trim) {
            if let Some(count) = line.strip_prefix("held ") {
                kills.held = count.trim().parse().ok();
            } else if let Ok(time) = line.parse() {
                kills.times.push(time);
            }
        }
        kills
    }

    pub fn to_text(&self) -> String {
        let mut text: String = self.times.iter().map(|time| format!("{time}\n")).collect();
        if let Some(count) = self.held {
            text.push_str(&format!("held {count}\n"));
        }
        text
    }
}

/// How a slot starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// As always: its first start, or back after anything but a kill for memory.
    Plain,
    /// Back after a kill for memory, the `kills`th in [`WINDOW`]: it logs in and runs its script.
    AfterKill { kills: usize },
    /// Held after `kills` kills for memory in [`WINDOW`]: at the login screen, its script not
    /// started.
    Held { kills: usize },
}

/// How a slot starts, from its kills so far and, when its process last ended in a kill for memory,
/// that kill's time (`kill`); `kills` keeps it. The same kill seen at two starts counts once.
/// Kills older than [`WINDOW`], or later than `now` (a clock set back), are let go.
pub fn at_start(kills: &mut Kills, kill: Option<u64>, now: u64) -> Start {
    let new = kill.is_some_and(|time| !kills.times.contains(&time));
    if new && let Some(time) = kill {
        kills.times.push(time);
    }
    kills
        .times
        .retain(|time| *time <= now && now - *time < WINDOW.as_secs());
    if let Some(count) = kills.held {
        return Start::Held { kills: count };
    }
    let count = kills.times.len();
    if !new {
        Start::Plain
    } else if count > RESTARTS {
        kills.held = Some(count);
        Start::Held { kills: count }
    } else {
        Start::AfterKill { kills: count }
    }
}

/// A held slot's startup flags from its usual ones: its script or playlist loaded, not run. (Its
/// password is left out where it's added, `android/src/slots.rs`.)
pub fn held_args(args: &[String]) -> Vec<String> {
    args.iter()
        .map(|arg| {
            let (flag, value) = arg.split_once('=').map_or((arg.as_str(), None), |(f, v)| (f, Some(v)));
            let flag = match flag {
                "--run-script" => "--load-script",
                "--run-playlist" => "--load-playlist",
                other => other,
            };
            value.map_or_else(|| flag.to_owned(), |value| format!("{flag}={value}"))
        })
        .collect()
}

/// What a held tab says, in its notification and the tab list.
pub fn held_text(kills: usize) -> String {
    format!(
        "Android closed this tab {kills} times for memory. Lower Tab Limit or close other apps, then start it again."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: u64 = 60;

    #[test]
    fn a_slot_killed_for_memory_a_fourth_time_in_half_an_hour_stays_down() {
        let mut kills = Kills::default();
        let start = 1_000_000;
        assert_eq!(
            at_start(&mut kills, Some(start), start + 5),
            Start::AfterKill { kills: 1 }
        );
        assert_eq!(
            at_start(&mut kills, Some(start + 5 * MINUTE), start + 5 * MINUTE + 5),
            Start::AfterKill { kills: 2 }
        );
        assert_eq!(
            at_start(&mut kills, Some(start + 10 * MINUTE), start + 10 * MINUTE + 5),
            Start::AfterKill { kills: 3 }
        );
        assert_eq!(
            at_start(&mut kills, Some(start + 20 * MINUTE), start + 20 * MINUTE + 5),
            Start::Held { kills: 4 }
        );
        assert_eq!(kills.held, Some(4));
    }

    #[test]
    fn kills_older_than_half_an_hour_let_the_slot_come_back() {
        let mut kills = Kills::parse("1000\n1100\n1200\n");
        // The first two are over 30 minutes old at the fourth kill: two in the window.
        let now = 1100 + 30 * MINUTE;
        assert_eq!(at_start(&mut kills, Some(now - 5), now), Start::AfterKill { kills: 2 });
        assert_eq!(kills.times, vec![1200, now - 5]);
        // A kill from a clock set back doesn't count.
        let mut kills = Kills::parse("5000\n");
        assert_eq!(at_start(&mut kills, Some(100), 200), Start::AfterKill { kills: 1 });
        assert_eq!(kills.times, vec![100]);
    }

    #[test]
    fn the_same_kill_seen_twice_counts_once() {
        let mut kills = Kills::parse("1000\n1100\n1200\n");
        assert_eq!(at_start(&mut kills, Some(1200), 1300), Start::Plain);
        assert_eq!(kills.times, vec![1000, 1100, 1200]);
        assert_eq!(at_start(&mut kills, None, 1300), Start::Plain);
    }

    #[test]
    fn a_held_slot_stays_held_until_started_by_hand() {
        let mut kills = Kills::parse("1000\n1100\n1200\n1300\nheld 4\n");
        // Hours later, after anything but a kill: still held.
        assert_eq!(at_start(&mut kills, None, 50_000), Start::Held { kills: 4 });
        assert!(kills.times.is_empty());
        // Killed again while held: still held.
        assert_eq!(at_start(&mut kills, Some(50_100), 50_200), Start::Held { kills: 4 });
        let text = kills.to_text();
        assert_eq!(text, "50100\nheld 4\n");
        assert_eq!(Kills::parse(&text), kills);
    }

    #[test]
    fn a_held_slot_loads_its_script_without_running_it() {
        let args: Vec<String> = [
            "--user",
            "Questwalker",
            "--server",
            "Sir Ver",
            "--run-script",
            "farm/Example.ts",
            "--run-playlist=daily.json",
            "--script-options-storage",
            "Squad A",
        ]
        .map(String::from)
        .into();
        assert_eq!(
            held_args(&args),
            [
                "--user",
                "Questwalker",
                "--server",
                "Sir Ver",
                "--load-script",
                "farm/Example.ts",
                "--load-playlist=daily.json",
                "--script-options-storage",
                "Squad A",
            ]
        );
    }

    #[test]
    fn tells_a_kill_for_memory_from_other_exits() {
        assert!(memory_kill(REASON_LOW_MEMORY, 0, false, false));
        // A plain SIGKILL: for memory only while memory is low (or the test switch is on).
        assert!(!memory_kill(REASON_SIGNALED, 9, false, false));
        assert!(memory_kill(REASON_SIGNALED, 9, true, false));
        assert!(memory_kill(REASON_SIGNALED, 9, false, true));
        assert!(!memory_kill(REASON_SIGNALED, 15, true, true));
        // EXIT_SELF (1), a crash (4): never.
        assert!(!memory_kill(1, 9, true, true));
        assert!(!memory_kill(4, 0, true, true));
    }
}
