//! Combat > Stats' Advanced Mode: live gameplay rates tracked apart from scripts (VibeSkua
//! `ScriptBotStats.UpdateGameplaySnapshot`), the rate unit, and the manual rate snapshots every
//! client shares (VibeSkua `StatsRateSnapshot`, `ScriptStatsViewModel`), kept in
//! `StatsRateHistory.json` in the data folder.
//!
//! The background thread reads the player once a second ([`PlayerReading`]) and hands it to
//! [`Gameplay::update`] under the state lock; the window only reads the result.

use crate::paths::Paths;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::time::{Duration, Instant};

/// Class points from rank 1 to rank 10 (VibeSkua `ClassPointsForRankTen`).
pub const CLASS_POINTS_FOR_RANK_TEN: i64 = 302_500;

/// Cumulative class points at ranks 1-10 (VibeSkua `ClassRankCumulativePoints`).
const CLASS_RANK_CUMULATIVE_POINTS: [i64; 10] = [
    0, 900, 3_600, 10_000, 22_500, 44_100, 78_400, 129_600, 202_500, 302_500,
];

/// Rates show only once the session is this old (VibeSkua: 5 s).
const WARM_UP: Duration = Duration::from_secs(5);

/// The most snapshots kept (VibeSkua `MaximumRateSnapshots`).
pub const MAXIMUM_RATE_SNAPSHOTS: usize = 100;

/// What the rates are shown in (VibeSkua `StatsRateUnit`: `/hr`, `/min`, `/sec`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RateUnit {
    #[default]
    Hour,
    Minute,
    Second,
}

impl RateUnit {
    pub const ALL: [RateUnit; 3] = [RateUnit::Hour, RateUnit::Minute, RateUnit::Second];

    pub fn label(self) -> &'static str {
        match self {
            RateUnit::Hour => "/hr",
            RateUnit::Minute => "/min",
            RateUnit::Second => "/sec",
        }
    }

    /// A saved unit; anything else is `/hr` (as VibeSkua reads `StatsRateUnit`).
    pub fn parse(text: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|u| u.label() == text)
            .unwrap_or_default()
    }

    /// An hourly rate in this unit (VibeSkua `ConvertHourlyRate`).
    pub fn from_hourly(self, value: f64) -> f64 {
        match self {
            RateUnit::Hour => value,
            RateUnit::Minute => value / 60.0,
            RateUnit::Second => value / 3600.0,
        }
    }

    /// A per-minute rate in this unit (VibeSkua `ConvertPerMinuteRate`).
    pub fn from_per_minute(self, value: f64) -> f64 {
        match self {
            RateUnit::Hour => value * 60.0,
            RateUnit::Minute => value,
            RateUnit::Second => value / 60.0,
        }
    }

    /// VibeSkua `RatePerUnitConverter`: an hourly rate (a per-minute one for kills and deaths,
    /// which get one more decimal) in this unit, with thousands separators.
    pub fn format(self, value: f64, per_minute: bool) -> String {
        let hourly = value * if per_minute { 60.0 } else { 1.0 };
        let decimals = match (self, per_minute) {
            (RateUnit::Hour, false) => 0,
            (RateUnit::Minute, false) => 1,
            (RateUnit::Second, false) => 2,
            (RateUnit::Hour, true) => 1,
            (RateUnit::Minute, true) => 2,
            (RateUnit::Second, true) => 3,
        };
        format_number(self.from_hourly(hourly), decimals)
    }
}

/// .NET's `N<decimals>` (invariant): `1,234,567.89`.
pub fn format_number(value: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, value.abs());
    let (whole, fraction) = match text.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (text.as_str(), None),
    };
    let mut grouped = String::with_capacity(text.len() + whole.len() / 3);
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if let Some(fraction) = fraction {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    // No "-0" when a small negative rounds to zero.
    if value < 0.0 && grouped.chars().any(|c| c.is_ascii_digit() && c != '0') {
        grouped.insert(0, '-');
    }
    grouped
}

/// VibeSkua `TimeSpanTotalHoursConverter`: total hours, then minutes and seconds (`27:04:09`).
pub fn format_hms(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// [`format_hms`], or `—` without a value.
pub fn format_eta(time: Option<Duration>) -> String {
    time.map_or_else(|| "—".to_owned(), format_hms)
}

/// One reading of the player (VibeSkua `UpdateGameplaySnapshot`'s arguments).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerReading {
    /// `objData.intExp`: XP into the current level.
    pub experience: i64,
    /// `objData.intExpToLevel`: the current level's requirement.
    pub required_experience: i64,
    pub level: i64,
    pub gold: i64,
    /// The equipped class's item ID; 0 without one.
    pub class_id: i64,
    /// `objData.iCurCP`: points into the current rank.
    pub class_points: i64,
    /// `objData.iCPToRank`: the current rank's requirement.
    pub class_points_to_rank: i64,
    /// `objData.iRank`.
    pub class_rank: i64,
}

/// A .NET `Stopwatch` driven by the caller's clock.
#[derive(Debug, Clone, Copy, Default)]
struct Stopwatch {
    elapsed: Duration,
    since: Option<Instant>,
}

impl Stopwatch {
    fn elapsed(&self, now: Instant) -> Duration {
        self.elapsed
            + self
                .since
                .map_or(Duration::ZERO, |s| now.saturating_duration_since(s))
    }

    fn is_running(&self) -> bool {
        self.since.is_some()
    }

    fn start(&mut self, now: Instant) {
        if self.since.is_none() {
            self.since = Some(now);
        }
    }

    fn stop(&mut self, now: Instant) {
        self.elapsed = self.elapsed(now);
        self.since = None;
    }

    fn restart(&mut self, now: Instant) {
        self.elapsed = Duration::ZERO;
        self.since = Some(now);
    }
}

/// C#'s `(int)Math.Round(x)`: halves to even.
fn round(value: f64) -> i64 {
    if value.is_finite() {
        value.round_ties_even() as i64
    } else {
        0
    }
}

/// The live gameplay metrics (VibeSkua `ScriptBotStats`' gameplay part). Counted while connected,
/// whether or not a script runs; Reset (Combat > Stats) starts them over.
#[derive(Debug, Clone, Default)]
pub struct Gameplay {
    session: Stopwatch,
    class: Stopwatch,
    has_snapshot: bool,
    last_experience: i64,
    last_required_experience: i64,
    last_level: i64,
    last_gold: i64,
    last_kills: u64,
    last_deaths: u64,
    last_class_id: i64,
    last_class_points: i64,
    last_class_points_to_rank: i64,
    last_class_rank: i64,
    earned_class_points: i64,
    session_kills: u64,
    session_deaths: u64,
    session_items: i64,
    earned_experience: i64,
    earned_gold: i64,

    /// Connected time since login or the last reset (VibeSkua `SessionTime`).
    pub session_time: Duration,
    /// XP into the level and the level's requirement (for "1,234 / 5,000 XP").
    pub experience: i64,
    pub required_experience: i64,
    /// 0-100.
    pub experience_progress: f64,
    /// 0-100, toward rank 10.
    pub class_rank10_progress: f64,
    /// Class points from rank 1 (VibeSkua `CurrentClassPoints`).
    pub current_class_points: i64,
    pub current_class_rank: i64,
    pub experience_per_hour: i64,
    /// Net: spending lowers it.
    pub gold_per_hour: i64,
    pub kills_per_minute: f64,
    pub deaths_per_minute: f64,
    pub items_per_hour: i64,
    pub class_points_per_hour: i64,
    pub time_to_level: Option<Duration>,
    pub time_to_class_rank10: Option<Duration>,
}

impl Gameplay {
    /// VibeSkua `UpdateGameplaySnapshot`: `None` while not in game (the clocks stop, and the
    /// next reading becomes the new baseline, so what changed meanwhile isn't counted). Positive
    /// XP and the net gold change, spending included, are counted; `kills` and `deaths` are the
    /// Stats counters.
    pub fn update(
        &mut self,
        reading: Option<PlayerReading>,
        kills: u64,
        deaths: u64,
        now: Instant,
    ) {
        let Some(r) = reading else {
            self.session.stop(now);
            self.class.stop(now);
            self.has_snapshot = false;
            self.session_time = self.session.elapsed(now);
            return;
        };
        self.session.start(now);
        self.experience_progress = if r.required_experience > 0 {
            (r.experience as f64 / r.required_experience as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        self.experience = r.experience.max(0);
        self.required_experience = r.required_experience.max(0);

        if !self.has_snapshot {
            self.set_baseline(&r, kills, deaths);
            self.update_class(&r, now);
            return;
        }

        let gained = if r.level == self.last_level {
            (r.experience - self.last_experience).max(0)
        } else if r.level > self.last_level {
            // intExpToLevel is the old level's requirement: carry the rest across the level-up.
            (self.last_required_experience - self.last_experience).max(0) + r.experience.max(0)
        } else {
            0
        };
        if gained > 0 {
            self.earned_experience += gained;
        }
        self.earned_gold += r.gold - self.last_gold;
        self.session_kills += kills.saturating_sub(self.last_kills);
        self.session_deaths += deaths.saturating_sub(self.last_deaths);
        self.set_baseline(&r, kills, deaths);
        self.update_class(&r, now);

        let elapsed = self.session.elapsed(now);
        self.session_time = elapsed;
        if elapsed < WARM_UP {
            return;
        }
        let hours = elapsed.as_secs_f64() / 3600.0;
        let minutes = elapsed.as_secs_f64() / 60.0;
        self.experience_per_hour = round(self.earned_experience as f64 / hours);
        self.gold_per_hour = round(self.earned_gold as f64 / hours);
        self.kills_per_minute = self.session_kills as f64 / minutes;
        self.deaths_per_minute = self.session_deaths as f64 / minutes;
        self.items_per_hour = round(self.session_items as f64 / hours);
        let remaining = (r.required_experience - r.experience).max(0);
        self.time_to_level = (self.experience_per_hour > 0 && r.required_experience > r.experience)
            .then(|| {
                Duration::from_secs_f64(remaining as f64 / self.experience_per_hour as f64 * 3600.0)
            });
    }

    /// VibeSkua `RecordItemPickup`: an accepted `getDrop`'s quantity.
    pub fn record_item_pickup(&mut self, quantity: i64) {
        if quantity > 0 {
            self.session_items += quantity;
        }
    }

    fn set_baseline(&mut self, r: &PlayerReading, kills: u64, deaths: u64) {
        self.last_experience = r.experience;
        self.last_required_experience = r.required_experience;
        self.last_level = r.level;
        self.last_gold = r.gold;
        self.last_kills = kills;
        self.last_deaths = deaths;
        self.has_snapshot = true;
    }

    /// VibeSkua `UpdateClassMetrics`: the class clock restarts with another class, and resumes
    /// after a disconnect without counting the points gained meanwhile.
    fn update_class(&mut self, r: &PlayerReading, now: Instant) {
        if r.class_id <= 0 {
            self.class.stop(now);
            self.last_class_id = 0;
            self.last_class_points = 0;
            self.last_class_points_to_rank = 0;
            self.last_class_rank = 0;
            self.current_class_points = 0;
            self.current_class_rank = 0;
            self.class_rank10_progress = 0.0;
            self.class_points_per_hour = 0;
            self.time_to_class_rank10 = None;
            return;
        }
        if self.last_class_id != r.class_id
            || (!self.class.is_running() && self.class.elapsed(now) == Duration::ZERO)
        {
            self.class.restart(now);
            self.last_class_id = r.class_id;
            self.last_class_points = r.class_points;
            self.last_class_points_to_rank = r.class_points_to_rank;
            self.last_class_rank = r.class_rank;
            self.earned_class_points = 0;
            self.class_points_per_hour = 0;
            self.time_to_class_rank10 = (r.class_rank >= 10).then_some(Duration::ZERO);
        } else if !self.class.is_running() {
            self.class.start(now);
            self.last_class_points = r.class_points;
            self.last_class_points_to_rank = r.class_points_to_rank;
            self.last_class_rank = r.class_rank;
        } else {
            if r.class_rank == self.last_class_rank {
                self.earned_class_points += (r.class_points - self.last_class_points).max(0);
            } else if r.class_rank > self.last_class_rank {
                let previous_requirement = if self.last_class_points_to_rank > 0 {
                    self.last_class_points_to_rank
                } else {
                    points_required_for_rank(self.last_class_rank)
                };
                self.earned_class_points += (previous_requirement - self.last_class_points).max(0);
                for rank in self.last_class_rank + 1..r.class_rank {
                    self.earned_class_points += points_required_for_rank(rank);
                }
                self.earned_class_points += r.class_points.max(0);
            }
            self.last_class_points = r.class_points;
            self.last_class_points_to_rank = r.class_points_to_rank;
            self.last_class_rank = r.class_rank;
        }

        self.current_class_rank = r.class_rank.max(0);
        self.current_class_points = rank_base_points(r.class_rank) + r.class_points.max(0);
        self.class_rank10_progress =
            (self.current_class_points as f64 / CLASS_POINTS_FOR_RANK_TEN as f64 * 100.0)
                .clamp(0.0, 100.0);

        let elapsed = self.class.elapsed(now);
        if elapsed < WARM_UP {
            return;
        }
        self.class_points_per_hour =
            round(self.earned_class_points as f64 / (elapsed.as_secs_f64() / 3600.0));
        let remaining = if r.class_rank >= 10 {
            0
        } else {
            (CLASS_POINTS_FOR_RANK_TEN - self.current_class_points).max(0)
        };
        self.time_to_class_rank10 = if r.class_rank >= 10 {
            Some(Duration::ZERO)
        } else if self.class_points_per_hour > 0 {
            Some(Duration::from_secs_f64(
                remaining as f64 / self.class_points_per_hour as f64 * 3600.0,
            ))
        } else {
            None
        };
    }

    /// "1,234 / 5,000 XP" (VibeSkua `ExperienceProgressText`).
    pub fn experience_text(&self) -> String {
        format!(
            "{} / {} XP",
            format_number(self.experience as f64, 0),
            format_number(self.required_experience as f64, 0)
        )
    }

    /// "12,345 / 302,500 CP" (VibeSkua `ClassRank10ProgressText`).
    pub fn class_rank10_text(&self) -> String {
        format!(
            "{} / {} CP",
            format_number(
                self.current_class_points.min(CLASS_POINTS_FOR_RANK_TEN) as f64,
                0
            ),
            format_number(CLASS_POINTS_FOR_RANK_TEN as f64, 0)
        )
    }
}

fn rank_base_points(rank: i64) -> i64 {
    let index = (rank - 1).clamp(0, CLASS_RANK_CUMULATIVE_POINTS.len() as i64 - 1);
    CLASS_RANK_CUMULATIVE_POINTS[index as usize]
}

fn points_required_for_rank(rank: i64) -> i64 {
    if rank < 1 || rank >= CLASS_RANK_CUMULATIVE_POINTS.len() as i64 {
        return 0;
    }
    CLASS_RANK_CUMULATIVE_POINTS[rank as usize] - CLASS_RANK_CUMULATIVE_POINTS[rank as usize - 1]
}

/// A manually saved row of the rates (VibeSkua `StatsRateSnapshot`), in the unit shown when it
/// was saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateSnapshot {
    /// RFC 3339, local offset.
    pub saved_at: String,
    pub session_seconds: f64,
    pub rate_unit: String,
    pub experience_rate: f64,
    pub gold_rate: f64,
    pub kills_rate: f64,
    pub deaths_rate: f64,
    pub items_rate: f64,
    pub class_points_rate: f64,
    pub class_rank: i64,
    /// Who saved it (logged in), if anyone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    /// Fields a newer Turu added, kept when this one rewrites the file.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl RateSnapshot {
    /// The rates shown now, converted to `unit` (VibeSkua `SaveRateSnapshot`).
    pub fn capture(gameplay: &Gameplay, unit: RateUnit, character: Option<String>) -> Self {
        Self {
            saved_at: chrono::Local::now().to_rfc3339(),
            session_seconds: gameplay.session_time.as_secs_f64(),
            rate_unit: unit.label().to_owned(),
            experience_rate: unit.from_hourly(gameplay.experience_per_hour as f64),
            gold_rate: unit.from_hourly(gameplay.gold_per_hour as f64),
            kills_rate: unit.from_per_minute(gameplay.kills_per_minute),
            deaths_rate: unit.from_per_minute(gameplay.deaths_per_minute),
            items_rate: unit.from_hourly(gameplay.items_per_hour as f64),
            class_points_rate: unit.from_hourly(gameplay.class_points_per_hour as f64),
            class_rank: gameplay.current_class_rank,
            character,
            extra: Map::new(),
        }
    }

    /// "MM-dd HH:mm", local time (VibeSkua `SavedAtDisplay`).
    pub fn saved_at_display(&self) -> String {
        chrono::DateTime::parse_from_rfc3339(&self.saved_at).map_or_else(
            |_| self.saved_at.clone(),
            |t| {
                t.with_timezone(&chrono::Local)
                    .format("%m-%d %H:%M")
                    .to_string()
            },
        )
    }

    pub fn character_display(&self) -> &str {
        match self.character.as_deref() {
            Some(c) if !c.trim().is_empty() => c,
            _ => "—",
        }
    }

    pub fn session_display(&self) -> String {
        format_hms(Duration::from_secs_f64(self.session_seconds.max(0.0)))
    }
}

/// VibeSkua `ExportHistory`'s CSV.
pub fn history_csv(history: &[RateSnapshot]) -> String {
    let mut csv = String::from(
        "Saved At,Character,Session Seconds,Rate Unit,Experience Rate,Gold Rate,Kills Rate,Deaths Rate,Items Rate,Class Points Rate,Class Rank\n",
    );
    for s in history {
        csv.push_str(&format!(
            "{},{},{:.0},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{}\n",
            s.saved_at,
            csv_field(s.character.as_deref()),
            s.session_seconds,
            s.rate_unit,
            s.experience_rate,
            s.gold_rate,
            s.kills_rate,
            s.deaths_rate,
            s.items_rate,
            s.class_points_rate,
            s.class_rank
        ));
    }
    csv
}

fn csv_field(value: Option<&str>) -> String {
    match value {
        None | Some("") => String::new(),
        Some(v) if v.contains([',', '"', '\n', '\r']) => format!("\"{}\"", v.replace('"', "\"\"")),
        Some(v) => v.to_owned(),
    }
}

/// How long Save and Clear wait for another client to finish writing the history.
const LOCK_TIMEOUT: Duration = Duration::from_secs(2);

/// The saved snapshots, newest first; none when the file is missing. An error when it can't
/// be read or parsed.
pub fn load_history(paths: &Paths) -> io::Result<Vec<RateSnapshot>> {
    let text = match fs::read_to_string(paths.stats_rate_history_file()) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut history: Vec<RateSnapshot> =
        serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    history.truncate(MAXIMUM_RATE_SNAPSHOTS);
    Ok(history)
}

/// Reloads the history, changes it and saves it while holding its lock, so a snapshot another
/// client saves at the same moment is kept (VibeSkua `TryUpdate`). Nothing is written when the
/// file can't be read. Returns the history as saved.
pub fn update_history(
    paths: &Paths,
    change: impl FnOnce(Vec<RateSnapshot>) -> Vec<RateSnapshot>,
) -> io::Result<Vec<RateSnapshot>> {
    let _lock = lock_history(paths)?;
    let mut history = change(load_history(paths)?);
    history.truncate(MAXIMUM_RATE_SNAPSHOTS);
    let text = serde_json::to_string_pretty(&history).map_err(io::Error::other)?;
    crate::paths::write_atomically(&paths.stats_rate_history_file(), &text)?;
    Ok(history)
}

/// Puts `snapshot` first (keeping [`MAXIMUM_RATE_SNAPSHOTS`]).
pub fn save_snapshot(paths: &Paths, snapshot: RateSnapshot) -> io::Result<Vec<RateSnapshot>> {
    update_history(paths, |history| {
        std::iter::once(snapshot)
            .chain(history.into_iter().take(MAXIMUM_RATE_SNAPSHOTS - 1))
            .collect()
    })
}

/// Deletes every snapshot, for every client.
pub fn clear_history(paths: &Paths) -> io::Result<Vec<RateSnapshot>> {
    update_history(paths, |_| Vec::new())
}

fn lock_history(paths: &Paths) -> io::Result<File> {
    fs::create_dir_all(&paths.root)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.root.join(".StatsRateHistory.lock"))?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if started.elapsed() < LOCK_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another client kept the rate history locked",
                ));
            }
            Err(TryLockError::Error(e)) => return Err(e),
        }
    }
}
