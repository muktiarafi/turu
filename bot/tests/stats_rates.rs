//! Combat > Stats' Advanced Mode (VibeSkua `ScriptBotStats` gameplay metrics, `RatePerUnitConverter`,
//! `StatsRateSnapshot`): the rate math, the formats and the shared rate history.

use std::time::{Duration, Instant};
use turu_bot::paths::Paths;
use turu_bot::stats::{self, Gameplay, PlayerReading, RateSnapshot, RateUnit};

fn reading(experience: i64, level: i64, gold: i64) -> PlayerReading {
    PlayerReading {
        experience,
        required_experience: 10_000,
        level,
        gold,
        ..Default::default()
    }
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn rates_count_xp_net_gold_kills_deaths_and_items_per_connected_hour() {
    let t0 = Instant::now();
    let mut g = Gameplay::default();
    g.update(Some(reading(1_000, 50, 5_000)), 3, 1, t0);
    // The first reading is the baseline: nothing earned yet.
    assert_eq!(g.experience_per_hour, 0);
    g.record_item_pickup(4);
    g.record_item_pickup(-2);
    // Half an hour: +2,500 XP, +3,000 gold earned and 1,000 spent, 30 kills, 2 deaths.
    g.update(Some(reading(3_500, 50, 7_000)), 33, 3, t0 + secs(1800));
    assert_eq!(g.session_time, secs(1800));
    assert_eq!(g.experience_per_hour, 5_000);
    assert_eq!(g.gold_per_hour, 4_000, "net: spending lowers it");
    assert!((g.kills_per_minute - 1.0).abs() < 1e-9);
    assert!((g.deaths_per_minute - 2.0 / 30.0).abs() < 1e-9);
    assert_eq!(g.items_per_hour, 8);
    // 6,500 XP left at 5,000/h.
    assert_eq!(g.time_to_level, Some(Duration::from_secs_f64(1.3 * 3600.0)));
    assert_eq!(g.experience_text(), "3,500 / 10,000 XP");
    assert!((g.experience_progress - 35.0).abs() < 1e-9);
}

#[test]
fn spending_more_than_earned_gives_a_negative_gold_rate() {
    let t0 = Instant::now();
    let mut g = Gameplay::default();
    g.update(Some(reading(0, 10, 10_000)), 0, 0, t0);
    g.update(Some(reading(0, 10, 4_000)), 0, 0, t0 + secs(3600));
    assert_eq!(g.gold_per_hour, -6_000);
    assert_eq!(g.time_to_level, None, "no XP: no estimate");
}

#[test]
fn a_level_up_carries_the_rest_of_the_old_level() {
    let t0 = Instant::now();
    let mut g = Gameplay::default();
    g.update(Some(reading(9_000, 50, 0)), 0, 0, t0);
    let mut next = reading(500, 51, 0);
    next.required_experience = 12_000;
    g.update(Some(next), 0, 0, t0 + secs(3600));
    // 1,000 to finish level 50, then 500 into 51.
    assert_eq!(g.experience_per_hour, 1_500);
}

#[test]
fn rates_wait_for_five_seconds_of_session() {
    let t0 = Instant::now();
    let mut g = Gameplay::default();
    g.update(Some(reading(0, 1, 0)), 0, 0, t0);
    g.update(Some(reading(100, 1, 0)), 1, 0, t0 + secs(4));
    assert_eq!(g.experience_per_hour, 0);
    assert_eq!(g.kills_per_minute, 0.0);
    assert_eq!(g.session_time, secs(4));
    g.update(Some(reading(100, 1, 0)), 1, 0, t0 + secs(6));
    assert_eq!(g.experience_per_hour, 60_000);
}

#[test]
fn time_offline_and_what_changed_meanwhile_are_left_out() {
    let t0 = Instant::now();
    let mut g = Gameplay::default();
    g.update(Some(reading(0, 1, 0)), 0, 0, t0);
    g.update(Some(reading(1_000, 1, 100)), 10, 0, t0 + secs(1800));
    // Disconnected for an hour: the clock stops.
    g.update(None, 10, 0, t0 + secs(1801));
    assert_eq!(g.session_time, secs(1801));
    // Back with XP, gold and kills from elsewhere: the new baseline.
    g.update(Some(reading(5_000, 1, 9_000)), 50, 3, t0 + secs(5401));
    g.update(Some(reading(6_000, 1, 9_100)), 60, 3, t0 + secs(7200));
    // Connected 1,801 + 1,799 s = one hour: 2,000 XP, 200 gold, 20 kills, no deaths.
    assert_eq!(g.session_time, secs(3600));
    assert_eq!(g.experience_per_hour, 2_000);
    assert_eq!(g.gold_per_hour, 200);
    assert!((g.kills_per_minute - 20.0 / 60.0).abs() < 1e-9);
    assert_eq!(g.deaths_per_minute, 0.0);
}

fn class(id: i64, rank: i64, points: i64, to_rank: i64) -> PlayerReading {
    PlayerReading {
        class_id: id,
        class_rank: rank,
        class_points: points,
        class_points_to_rank: to_rank,
        ..reading(0, 1, 0)
    }
}

#[test]
fn class_points_count_across_rank_ups_toward_rank_ten() {
    let t0 = Instant::now();
    let mut g = Gameplay::default();
    // Rank 3, 6,000 of 6,400 into it (10,000 - 3,600).
    g.update(Some(class(7, 3, 6_000, 6_400)), 0, 0, t0);
    assert_eq!(g.current_class_rank, 3);
    assert_eq!(g.current_class_points, 9_600);
    assert_eq!(g.class_rank10_text(), "9,600 / 302,500 CP");
    // An hour later rank 5, 1,000 into it: 400 + 12,500 (rank 4) + 1,000.
    g.update(Some(class(7, 5, 1_000, 21_600)), 0, 0, t0 + secs(3600));
    assert_eq!(g.class_points_per_hour, 13_900);
    assert_eq!(g.current_class_points, 23_500);
    let left = (302_500 - 23_500) as f64 / 13_900.0 * 3600.0;
    assert_eq!(g.time_to_class_rank10, Some(Duration::from_secs_f64(left)));
    assert!((g.class_rank10_progress - 23_500.0 / 302_500.0 * 100.0).abs() < 1e-9);
}

#[test]
fn another_class_restarts_the_class_clock_and_rank_ten_needs_nothing() {
    let t0 = Instant::now();
    let mut g = Gameplay::default();
    g.update(Some(class(7, 3, 0, 6_400)), 0, 0, t0);
    g.update(Some(class(7, 3, 3_000, 6_400)), 0, 0, t0 + secs(3600));
    assert_eq!(g.class_points_per_hour, 3_000);
    g.update(Some(class(8, 10, 0, 0)), 0, 0, t0 + secs(3601));
    assert_eq!(g.class_points_per_hour, 0);
    assert_eq!(g.time_to_class_rank10, Some(Duration::ZERO));
    assert_eq!(g.class_rank10_text(), "302,500 / 302,500 CP");
    // No class: the class metrics clear.
    g.update(Some(class(0, 0, 0, 0)), 0, 0, t0 + secs(3602));
    assert_eq!(g.current_class_rank, 0);
    assert_eq!(g.time_to_class_rank10, None);
}

#[test]
fn class_points_gained_while_disconnected_are_not_counted() {
    let t0 = Instant::now();
    let mut g = Gameplay::default();
    g.update(Some(class(7, 2, 0, 2_700)), 0, 0, t0);
    g.update(Some(class(7, 2, 1_000, 2_700)), 0, 0, t0 + secs(1800));
    g.update(None, 0, 0, t0 + secs(1800));
    // Back an hour later with 1,500 more points: rebaselined.
    g.update(Some(class(7, 2, 2_500, 2_700)), 0, 0, t0 + secs(5400));
    g.update(Some(class(7, 3, 800, 6_400)), 0, 0, t0 + secs(7200));
    // 1,000 + 200 + 800 in one connected hour.
    assert_eq!(g.class_points_per_hour, 2_000);
}

#[test]
fn rates_format_in_the_chosen_unit() {
    assert_eq!(RateUnit::parse("/min"), RateUnit::Minute);
    assert_eq!(RateUnit::parse("/day"), RateUnit::Hour);
    assert_eq!(RateUnit::Hour.format(1_234_567.0, false), "1,234,567");
    assert_eq!(RateUnit::Minute.format(9_000.0, false), "150.0");
    assert_eq!(RateUnit::Second.format(9_000.0, false), "2.50");
    // Kills and deaths are per minute, with one more decimal.
    assert_eq!(RateUnit::Hour.format(1.5, true), "90.0");
    assert_eq!(RateUnit::Minute.format(1.5, true), "1.50");
    assert_eq!(RateUnit::Second.format(1.5, true), "0.025");
    assert_eq!(RateUnit::Hour.format(-6_000.0, false), "-6,000");
    assert_eq!(stats::format_number(-0.001, 2), "0.00");
    assert_eq!(stats::format_number(999.0, 0), "999");
    assert_eq!(stats::format_number(1_000.0, 0), "1,000");
    assert_eq!(
        stats::format_hms(Duration::from_secs(27 * 3600 + 4 * 60 + 9)),
        "27:04:09"
    );
    assert_eq!(stats::format_eta(None), "—");
    assert_eq!(RateUnit::Minute.from_hourly(120.0), 2.0);
    assert_eq!(RateUnit::Hour.from_per_minute(2.0), 120.0);
    assert_eq!(RateUnit::Second.from_per_minute(30.0), 0.5);
}

fn snapshot(character: Option<&str>, xp: f64) -> RateSnapshot {
    let mut g = Gameplay::default();
    g.experience_per_hour = xp as i64;
    g.kills_per_minute = 2.0;
    g.session_time = secs(3725);
    RateSnapshot::capture(&g, RateUnit::Minute, character.map(str::to_owned))
}

#[test]
fn snapshots_hold_the_rates_in_the_unit_they_were_saved_in() {
    let s = snapshot(Some("Hero"), 6_000.0);
    assert_eq!(s.rate_unit, "/min");
    assert_eq!(s.experience_rate, 100.0);
    assert_eq!(s.kills_rate, 2.0);
    assert_eq!(s.session_display(), "1:02:05");
    assert_eq!(s.character_display(), "Hero");
    assert_eq!(snapshot(None, 0.0).character_display(), "—");
}

#[test]
fn the_shared_history_keeps_newest_first_unknown_fields_and_one_hundred_rows() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths {
        root: dir.path().to_owned(),
    };
    assert!(stats::load_history(&paths).unwrap().is_empty());
    // A newer client's row with a field this one doesn't know.
    let mut newer = serde_json::to_value(snapshot(Some("Other"), 60.0)).unwrap();
    newer["futureField"] = serde_json::json!(42);
    std::fs::write(
        paths.stats_rate_history_file(),
        serde_json::to_string(&vec![newer]).unwrap(),
    )
    .unwrap();
    let history = stats::save_snapshot(&paths, snapshot(Some("Hero"), 120.0)).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].character.as_deref(), Some("Hero"));
    let text = std::fs::read_to_string(paths.stats_rate_history_file()).unwrap();
    assert!(text.contains("futureField"), "kept: {text}");

    for i in 0..120 {
        stats::save_snapshot(&paths, snapshot(None, i as f64)).unwrap();
    }
    let history = stats::load_history(&paths).unwrap();
    assert_eq!(history.len(), stats::MAXIMUM_RATE_SNAPSHOTS);
    assert!((history[0].experience_rate - 119.0 / 60.0).abs() < 1e-9);
    assert!(stats::clear_history(&paths).unwrap().is_empty());
    assert!(stats::load_history(&paths).unwrap().is_empty());
}

#[test]
fn an_unreadable_history_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths {
        root: dir.path().to_owned(),
    };
    std::fs::write(paths.stats_rate_history_file(), "{ not json").unwrap();
    assert!(stats::save_snapshot(&paths, snapshot(None, 1.0)).is_err());
    assert!(stats::clear_history(&paths).is_err());
    let text = std::fs::read_to_string(paths.stats_rate_history_file()).unwrap();
    assert_eq!(text, "{ not json");
}

#[test]
fn the_csv_matches_vibeskuas_export() {
    let mut s = snapshot(Some("Hero, \"the\" Bold"), 6_000.0);
    s.saved_at = "2026-11-07T10:00:00+07:00".into();
    let csv = stats::history_csv(&[s]);
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some(
            "Saved At,Character,Session Seconds,Rate Unit,Experience Rate,Gold Rate,Kills Rate,Deaths Rate,Items Rate,Class Points Rate,Class Rank"
        )
    );
    assert_eq!(
        lines.next(),
        Some(
            "2026-11-07T10:00:00+07:00,\"Hero, \"\"the\"\" Bold\",3725,/min,100.00,0.00,2.00,0.00,0.00,0.00,0"
        )
    );
}
