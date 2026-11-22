//! Combat > Stats (VibeSkua `ScriptStatsView`): the script clock, the counters and, with Advanced
//! on, the live gameplay rates (`turu_bot::stats::Gameplay`, sampled by the bot's background
//! thread) and the rate history every client shares (`StatsRateHistory.json`), with what Reset,
//! Advanced, the unit, Save, Clear and Export do. The words and numbers are formatted here, so
//! every shell reads them alike; colours are each shell's, by kind.

use super::scheduler::Tint;
use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use turu_bot::Bot;
use turu_bot::stats::{self, Gameplay, RateSnapshot, RateUnit};

/// How often a read looks for snapshots other clients saved.
const HISTORY_CHECK: Duration = Duration::from_secs(2);

/// A counter's kind: each shell gives it its colour (kills green, deaths red, pickups amber,
/// accepted blue, completed green, relogins purple, as VibeSkua's tiles).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum CounterKind {
    Kills,
    Deaths,
    Pickups,
    Accepted,
    Completed,
    Relogins,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    /// `hh:mm:ss`: the running script's clock, kept after it ends.
    pub script_time: String,
    pub counters: Vec<CounterView>,
    pub advanced: bool,
    /// The rate unit ("/hr") and the ones to choose from.
    pub unit: String,
    pub units: Vec<String>,
    /// With Advanced on.
    pub gameplay: Option<GameplayView>,
    /// The rate history, newest first; empty with Advanced off.
    pub history: Vec<SnapshotView>,
    /// Save, Export or Clear went wrong: why (VibeSkua's message box).
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CounterView {
    pub kind: CounterKind,
    /// "KILLS".
    pub label: String,
    /// "1,234".
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GameplayView {
    /// `hh:mm:ss` since login or the last reset.
    pub session: String,
    pub level: ProgressView,
    pub rank: ProgressView,
    /// Net gold, Items, Kills, Deaths, in the unit.
    pub rates: Vec<RateView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    /// "Level", "Rank 7".
    pub label: String,
    /// 0-100.
    pub percent: f64,
    /// "1,234 / 5,000 XP".
    pub value: String,
    /// "12,000 EXP /hr · next level in 25m".
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RateView {
    /// "Net gold /hr".
    pub label: String,
    pub value: String,
    pub tint: Tint,
    /// What it means, where it needs saying; empty otherwise.
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    pub saved: String,
    pub character: String,
    pub session: String,
    pub unit: String,
    pub exp: String,
    pub gold: String,
    pub class_points: String,
    pub rank: String,
    pub kills: String,
    pub deaths: String,
    pub items: String,
}

/// A tab's Stats state that isn't the bot's: the clock kept after a script ends, the history as
/// last read, and the last failure's note.
#[derive(Default)]
pub struct State {
    pub last_elapsed: Duration,
    history: Vec<RateSnapshot>,
    checked: Option<Instant>,
    modified: Option<SystemTime>,
    pub note: Option<String>,
}

pub static STATE: PerTab<State> = PerTab::new();

/// The script clock: running, or where it stopped.
pub fn script_time(bot: &Arc<Bot>) -> Duration {
    let active = bot.scripts.status().is_active();
    let started = bot.state.lock().stats.started;
    let state = STATE.of(bot);
    let mut state = lock(&state);
    if active && let Some(started) = started {
        state.last_elapsed = started.elapsed();
    }
    state.last_elapsed
}

/// VibeSkua `RefreshHistory`: what is saved now, other clients' snapshots included, looked at
/// every couple of seconds. Keeps the last read when the file can't be read.
pub fn history(bot: &Arc<Bot>) -> Vec<RateSnapshot> {
    let state = STATE.of(bot);
    let mut state = lock(&state);
    if state.checked.is_none_or(|t| t.elapsed() >= HISTORY_CHECK) {
        state.checked = Some(Instant::now());
        let modified = std::fs::metadata(bot.paths.stats_rate_history_file())
            .and_then(|m| m.modified())
            .ok();
        if modified.is_none() || modified != state.modified {
            if let Ok(history) = stats::load_history(&bot.paths) {
                state.history = history;
            }
            state.modified = modified;
        }
    }
    state.history.clone()
}

fn snapshot_view(s: &RateSnapshot) -> SnapshotView {
    SnapshotView {
        saved: s.saved_at_display(),
        character: s.character_display().to_owned(),
        session: s.session_display(),
        unit: s.rate_unit.clone(),
        exp: stats::format_number(s.experience_rate, 0),
        gold: stats::format_number(s.gold_rate, 0),
        class_points: stats::format_number(s.class_points_rate, 0),
        rank: s.class_rank.to_string(),
        kills: stats::format_number(s.kills_rate, 2),
        deaths: stats::format_number(s.deaths_rate, 2),
        items: stats::format_number(s.items_rate, 1),
    }
}

pub fn counters(s: &turu_bot::state::Stats) -> Vec<CounterView> {
    [
        (CounterKind::Kills, "KILLS", s.kills),
        (CounterKind::Deaths, "DEATHS", s.deaths),
        (CounterKind::Pickups, "PICKUPS", s.drops),
        (CounterKind::Accepted, "ACCEPTED", s.quests_accepted),
        (CounterKind::Completed, "COMPLETED", s.quests_completed),
        (CounterKind::Relogins, "RE-LOGINS", s.relogins),
    ]
    .into_iter()
    .map(|(kind, label, value)| CounterView {
        kind,
        label: label.to_owned(),
        value: stats::format_number(value as f64, 0),
    })
    .collect()
}

pub fn gameplay(g: &Gameplay, unit: RateUnit) -> GameplayView {
    let label = unit.label();
    let rate = |label: String, value: String, tint: Tint, note: &str| RateView {
        label,
        value,
        tint,
        note: note.to_owned(),
    };
    GameplayView {
        session: stats::format_hms(g.session_time),
        level: ProgressView {
            label: "Level".into(),
            percent: g.experience_progress,
            value: g.experience_text(),
            hint: format!(
                "{} EXP {label} · next level in {}",
                unit.format(g.experience_per_hour as f64, false),
                stats::format_eta(g.time_to_level)
            ),
        },
        rank: ProgressView {
            label: format!("Rank {}", g.current_class_rank),
            percent: g.class_rank10_progress,
            value: g.class_rank10_text(),
            hint: format!(
                "{} CP {label} · rank 10 in {}",
                unit.format(g.class_points_per_hour as f64, false),
                stats::format_eta(g.time_to_class_rank10)
            ),
        },
        rates: vec![
            rate(
                format!("Net gold {label}"),
                unit.format(g.gold_per_hour as f64, false),
                Tint::Warning,
                "Net gold balance change; spending reduces the measured rate.",
            ),
            rate(
                format!("Items {label}"),
                unit.format(g.items_per_hour as f64, false),
                Tint::Warning,
                "",
            ),
            rate(
                format!("Kills {label}"),
                unit.format(g.kills_per_minute, true),
                Tint::Success,
                "",
            ),
            rate(
                format!("Deaths {label}"),
                unit.format(g.deaths_per_minute, true),
                Tint::Danger,
                "",
            ),
        ],
    }
}

pub fn view(bot: &Arc<Bot>) -> StatsView {
    let elapsed = script_time(bot);
    let s = bot.state.lock().stats.clone();
    let (advanced, unit) = {
        let settings = bot.settings.read();
        (settings.stats_advanced_mode, RateUnit::parse(&settings.stats_rate_unit))
    };
    StatsView {
        script_time: stats::format_hms(elapsed),
        counters: counters(&s),
        advanced,
        unit: unit.label().to_owned(),
        units: RateUnit::ALL.iter().map(|u| u.label().to_owned()).collect(),
        gameplay: advanced.then(|| gameplay(&s.gameplay, unit)),
        history: if advanced {
            history(bot).iter().map(snapshot_view).collect()
        } else {
            Vec::new()
        },
        note: lock(&STATE.of(bot)).note.clone(),
    }
}

/// Reset: the counters and the rates start over (the clock too, from now while a script runs).
pub fn reset(bot: &Arc<Bot>) {
    let active = bot.scripts.status().is_active();
    let mut state = bot.state.lock();
    state.stats = Default::default();
    state.stats.started = active.then(Instant::now);
    lock(&STATE.of(bot)).last_elapsed = Duration::ZERO;
}

pub fn set_advanced(bot: &Arc<Bot>, on: bool) {
    bot.settings.write().stats_advanced_mode = on;
    bot.save_settings();
}

/// The rate unit, by its label ("/hr", "/min", "/sec").
pub fn set_unit(bot: &Arc<Bot>, unit: &str) -> Result<(), String> {
    let unit = RateUnit::ALL
        .into_iter()
        .find(|u| u.label() == unit)
        .ok_or_else(|| format!("no unit {unit}"))?;
    bot.settings.write().stats_rate_unit = unit.label().to_owned();
    bot.save_settings();
    Ok(())
}

fn forget_history(bot: &Arc<Bot>, history: Vec<RateSnapshot>, note: Option<String>) {
    let state = STATE.of(bot);
    let mut state = lock(&state);
    state.history = history;
    state.modified = None;
    state.checked = None;
    state.note = note;
}

/// VibeSkua `SaveRateSnapshot`: the current rates, in the unit shown, to the history.
pub fn save_snapshot(bot: &Arc<Bot>) {
    let unit = RateUnit::parse(&bot.settings.read().stats_rate_unit);
    let (gameplay, character) = {
        let state = bot.state.lock();
        let character = if state.in_game { state.username.clone() } else { None };
        (state.stats.gameplay.clone(), character)
    };
    let snapshot = RateSnapshot::capture(&gameplay, unit, character);
    match stats::save_snapshot(&bot.paths, snapshot) {
        Ok(history) => forget_history(bot, history, None),
        Err(e) => {
            tracing::warn!("Couldn't save the rate snapshot: {e}");
            let history = lock(&STATE.of(bot)).history.clone();
            forget_history(
                bot,
                history,
                Some(
                    "Couldn't save the snapshot: the history file couldn't be read or written. Nothing was \
                     changed; try again."
                        .into(),
                ),
            );
        }
    }
}

/// Clear Rate History: every client's snapshots go.
pub fn clear_history(bot: &Arc<Bot>) {
    match stats::clear_history(&bot.paths) {
        Ok(history) => forget_history(bot, history, None),
        Err(e) => {
            tracing::warn!("Couldn't clear the rate history: {e}");
            let history = lock(&STATE.of(bot)).history.clone();
            forget_history(
                bot,
                history,
                Some(
                    "Couldn't clear the history: the history file couldn't be read or written. Nothing was \
                     changed; try again."
                        .into(),
                ),
            );
        }
    }
}

/// VibeSkua `ExportHistory`: what is saved now, as CSV (`None`: nothing saved).
pub fn export(bot: &Arc<Bot>) -> Option<String> {
    let state = STATE.of(bot);
    lock(&state).checked = None;
    lock(&state).modified = None;
    let history = history(bot);
    (!history.is_empty()).then(|| stats::history_csv(&history))
}

/// The note a failure left, for a window to show.
pub fn view_note(bot: &Arc<Bot>) -> Option<String> {
    lock(&STATE.of(bot)).note.clone()
}

/// Closes the note a failure left.
pub fn dismiss_note(bot: &Arc<Bot>) {
    lock(&STATE.of(bot)).note = None;
}

/// Stats' actions (`POST /models/stats/do`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum StatsAct {
    Reset,
    /// Saves a snapshot to the history.
    Save,
    /// Clears the history.
    Clear,
    /// Dismisses the note.
    Dismiss,
    Advanced {
        on: bool,
    },
    /// The rates' unit, by its key.
    Unit {
        unit: String,
    },
    /// Answers the stats as CSV: `{ "csv": ... }` (null when there are none).
    Export,
}

pub fn act(bot: &Arc<Bot>, act: StatsAct) -> Result<serde_json::Value, String> {
    match act {
        StatsAct::Reset => reset(bot),
        StatsAct::Save => save_snapshot(bot),
        StatsAct::Clear => clear_history(bot),
        StatsAct::Dismiss => dismiss_note(bot),
        StatsAct::Advanced { on } => set_advanced(bot, on),
        StatsAct::Unit { unit } => set_unit(bot, &unit)?,
        StatsAct::Export => return Ok(serde_json::json!({ "csv": export(bot) })),
    }
    Ok(serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_and_reset() {
        let bot = Bot::new();
        bot.state.lock().stats.kills = 1234;
        let view = view(&bot);
        assert_eq!(view.counters[0].label, "KILLS");
        assert_eq!(view.counters[0].value, "1,234");
        assert_eq!(view.script_time, "0:00:00");
        assert_eq!(view.units, ["/hr", "/min", "/sec"]);
        reset(&bot);
        assert_eq!(super::view(&bot).counters[0].value, "0");
    }

    #[test]
    fn gameplay_words_follow_the_unit() {
        let g = Gameplay::default();
        let view = gameplay(&g, RateUnit::Minute);
        assert_eq!(view.rates[0].label, "Net gold /min");
        assert_eq!(view.rates[3].tint, Tint::Danger);
        assert!(view.level.hint.contains("EXP /min"));
        assert_eq!(view.rank.label, "Rank 0");
    }
}
