//! Turu: what the tabs were doing over the run, for the report of an exit Turu didn't ask for.
//! Android's exit records say a process ran out of memory but not how many game clients it held,
//! and processes of their own help only when there were several (docs/TAB_PROCESSES_PROPOSAL.md,
//! M0). Both Android apps call [`record`] every 2 s; when the tabs change it writes
//! `crash/tabs-<pid>.log`: a line per change with the time, how many tabs are open, the most this
//! run, and each tab's state (no account or character names). `CrashReports.kt` adds the file to
//! that process's exit report and tags the Sentry event with the counts (`tabs`, `tabs_most`).

use crate::turu;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;

/// The changes kept in the file.
const LINES: usize = 100;

/// What a tab is doing, as far as an exit report needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabState {
    LoginScreen,
    LoggingBackIn,
    InGame,
    Farming,
}

impl TabState {
    fn describe(self) -> &'static str {
        match self {
            Self::LoginScreen => "not logged in",
            Self::LoggingBackIn => "logging back in",
            Self::InGame => "in game",
            Self::Farming => "in game, script running",
        }
    }
}

struct Log {
    header: String,
    /// The last summary written, to write only changes.
    last: String,
    most: usize,
    lines: VecDeque<String>,
}

static LOG: Mutex<Option<Log>> = Mutex::new(None);

/// The tabs' states and whether each is the one shown, in order.
fn tabs_now() -> Vec<(bool, TabState)> {
    let active = turu::active_tab();
    turu::tabs()
        .into_iter()
        .map(|(id, bot)| {
            let in_game = bot.state.lock().in_game;
            let state = match (in_game, bot.scripts.is_running()) {
                (true, true) => TabState::Farming,
                (true, false) => TabState::InGame,
                (false, _) if bot.servers.is_relogging() => TabState::LoggingBackIn,
                (false, _) => TabState::LoginScreen,
            };
            (id == active, state)
        })
        .collect()
}

/// "2 tabs, at most 3 this run: tab 1 (shown) in game, script running; tab 2 not logged in".
/// `CrashReports.kt` reads the two counts from the start (`(\d+) tabs?, at most (\d+)`).
pub fn summary(tabs: &[(bool, TabState)], most: usize) -> String {
    let count = tabs.len();
    let mut text = format!(
        "{count} tab{}, at most {most} this run",
        if count == 1 { "" } else { "s" }
    );
    for (i, (shown, state)) in tabs.iter().enumerate() {
        text.push_str(if i == 0 { ": " } else { "; " });
        text.push_str(&format!(
            "tab {}{} {}",
            i + 1,
            if *shown && count > 1 { " (shown)" } else { "" },
            state.describe()
        ));
    }
    text
}

/// Writes `dir/tabs-<pid>.log` when the tabs changed since the last call. Any thread.
pub fn record(dir: &Path) {
    let tabs = tabs_now();
    let mut guard = LOG.lock().unwrap_or_else(|e| e.into_inner());
    let log = guard.get_or_insert_with(|| Log {
        header: format!(
            "Turu {} tabs, process {}, started {}\n",
            turu::VERSION,
            std::process::id(),
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z"),
        ),
        last: String::new(),
        most: 0,
        lines: VecDeque::new(),
    });
    log.most = log.most.max(tabs.len());
    let summary = summary(&tabs, log.most);
    if summary == log.last {
        return;
    }
    if log.lines.len() >= LINES {
        log.lines.pop_front();
    }
    log.lines.push_back(format!(
        "{} {summary}",
        chrono::Local::now().format("%H:%M:%S")
    ));
    log.last = summary;
    let mut text = log.header.clone();
    for line in &log.lines {
        text.push_str(line);
        text.push('\n');
    }
    // `.log`: the crash list only shows `.txt` reports.
    let path = dir.join(format!("tabs-{}.log", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, text)) {
        tracing::warn!("Couldn't write {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tab_says_no_shown() {
        assert_eq!(
            summary(&[(true, TabState::Farming)], 1),
            "1 tab, at most 1 this run: tab 1 in game, script running"
        );
    }

    #[test]
    fn several_tabs_name_the_shown_one_and_the_most() {
        assert_eq!(
            summary(
                &[(false, TabState::InGame), (true, TabState::LoginScreen)],
                3
            ),
            "2 tabs, at most 3 this run: tab 1 in game; tab 2 (shown) not logged in"
        );
    }
}
