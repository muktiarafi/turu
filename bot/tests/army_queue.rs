//! Army Control's map travel settings and Army Queue: what they save, and how a queue runs
//! its squads against a fake launcher.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::army::jump::{self, Destination, JumpSettings, Location, Visits};
use turu_bot::army::queue::{
    Launched, MemberStatus, Outcome, Presets, QueueAccount, QueueConfig, QueueLauncher,
    QueueRunner, QueueStep, StepStatus,
};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("turu-army-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn location(map: &str, room: &str, user: &str) -> Location {
    Location {
        map: map.into(),
        room: room.into(),
        cell: "Enter".into(),
        pad: "Spawn".into(),
        username: user.into(),
    }
}

#[test]
fn history_records_only_new_visits_and_persists() {
    let dir = temp_dir("history");
    let file = dir.join(jump::FILE_NAME);
    let mut settings = JumpSettings::default();
    let mut visits = Visits::default();
    assert!(visits.record(&mut settings, "alt1", &location("yulgar", "12345", "Alt1")));
    // Standing still or walking to another cell isn't a visit.
    assert!(!visits.record(&mut settings, "alt1", &location("yulgar", "12345", "Alt1")));
    let mut moved = location("yulgar", "12345", "Alt1");
    moved.cell = "Upstairs".into();
    assert!(!visits.record(&mut settings, "alt1", &moved));
    // Another client in the same room moves it to the top, once.
    assert!(visits.record(&mut settings, "alt2", &location("battleon", "", "Alt2")));
    assert!(visits.record(&mut settings, "alt2", &location("YULGAR", "12345", "Alt2")));
    assert_eq!(settings.history.len(), 2);
    assert_eq!(
        settings.history[0].display_name(),
        "YULGAR-12345 - Enter, Spawn"
    );
    assert_eq!(settings.history[0].name, "Alt2");
    for i in 0..20 {
        visits.record(
            &mut settings,
            "alt3",
            &location(&format!("map{i}"), "", "Alt3"),
        );
    }
    assert_eq!(settings.history.len(), jump::HISTORY_LEN);
    assert_eq!(settings.history[0].map, "map19");

    settings.save(&file).unwrap();
    assert_eq!(JumpSettings::load(&file), settings);
    // VibeSkua's own file reads as it is.
    std::fs::write(
        &file,
        r#"{"Map":"tercessuinotlim","Room":"","Cell":"m2","Player":"Artix",
            "History":[{"Name":"Alt","Map":"yulgar","Room":"1","Cell":"Enter","Pad":"Spawn"}],
            "Presets":[{"Name":"Farm","Map":"tercessuinotlim","Room":"","Cell":"m2","Pad":"Left"}]}"#,
    )
    .unwrap();
    let read = JumpSettings::load(&file);
    assert_eq!(read.player, "Artix");
    assert_eq!(read.manual_text(), "tercessuinotlim,,m2");
    assert_eq!(
        read.presets[0].preset_label(),
        "Farm — tercessuinotlim - m2, Left"
    );
    settings.clear_history();
    assert!(settings.history.is_empty());
}

#[test]
fn presets_are_created_edited_and_deleted() {
    let dir = temp_dir("presets");
    let file = dir.join(jump::FILE_NAME);
    let mut settings = JumpSettings::default();
    let farm = Destination {
        name: "Farm".into(),
        map: " tercessuinotlim ".into(),
        cell: "m2".into(),
        pad: String::new(),
        ..Default::default()
    };
    assert_eq!(settings.save_preset(None, farm.clone()), Ok(0));
    assert_eq!(settings.presets[0].pad, "Spawn");
    assert_eq!(settings.presets[0].map, "tercessuinotlim");
    let boss = Destination {
        name: "Boss".into(),
        map: "ultradage".into(),
        room: "9999".into(),
        cell: "Boss".into(),
        pad: "Left".into(),
    };
    assert_eq!(settings.save_preset(None, boss.clone()), Ok(1));
    assert!(settings.save_preset(None, Destination::default()).is_err());
    // A new preset with an existing name replaces it.
    let mut farm2 = farm.clone();
    farm2.name = "FARM".into();
    farm2.cell = "m3".into();
    assert_eq!(settings.save_preset(None, farm2), Ok(1));
    assert_eq!(settings.presets.len(), 2);
    assert_eq!(settings.presets[0].name, "Boss");
    // Editing keeps its place.
    let mut edited = boss.clone();
    edited.cell = "r1".into();
    assert_eq!(settings.save_preset(Some(0), edited), Ok(0));
    assert_eq!(settings.presets[0].cell, "r1");
    assert_eq!(
        settings.presets[0].command(),
        turu_bot::army::Command::JumpMap {
            map: "ultradage-9999".into(),
            cell: "r1".into(),
            pad: "Left".into()
        }
    );
    settings.player = "Artix".into();
    settings.remember_jump(&settings.presets[0].clone());
    settings.save(&file).unwrap();
    let read = JumpSettings::load(&file);
    assert_eq!(read, settings);
    assert_eq!(read.manual_text(), "ultradage,9999,r1");
    let mut read = read;
    read.delete_preset(0);
    read.delete_preset(5);
    assert_eq!(read.presets.len(), 1);
}

#[test]
fn manual_destinations_and_locations_parse() {
    let d = jump::parse_manual("yulgar, 12345, Upstairs").unwrap();
    assert_eq!(
        (d.map.as_str(), d.room.as_str(), d.cell.as_str()),
        ("yulgar", "12345", "Upstairs")
    );
    let d = jump::parse_manual("yulgar,Upstairs").unwrap();
    assert_eq!((d.room.as_str(), d.cell.as_str()), ("", "Upstairs"));
    assert_eq!(jump::parse_manual("battleon").unwrap().cell, "Enter");
    assert!(jump::parse_manual(" ,x").is_none());
    let l = Location::from_full_name("doom-wood-42", "Enter", "", "Alt").unwrap();
    assert_eq!(
        (l.map.as_str(), l.room.as_str(), l.pad.as_str()),
        ("doom-wood", "42", "Spawn")
    );
    assert_eq!(
        Location::from_full_name("battleon", "Enter", "Left", "Alt")
            .unwrap()
            .room,
        ""
    );
    assert!(Location::from_full_name("battleon", "", "Left", "Alt").is_none());
    assert_eq!(l.to_destination().name, "doom-wood-42");
}

#[test]
fn queues_and_presets_persist() {
    let dir = temp_dir("queues");
    let active = dir.join(turu_bot::army::queue::ACTIVE_FILE);
    // Nothing saved: one empty "Squad 1".
    let config = QueueConfig::load_active(&active);
    assert_eq!(config.name, "Daily Ultras");
    assert_eq!(config.steps.len(), 1);
    assert_eq!(config.steps[0].name, "Squad 1");

    let mut config = config;
    let second = config.add_step();
    assert_eq!(config.steps[second].name, "Squad 2");
    config.set_members(
        0,
        &[
            ("Alt1".into(), "Main".into()),
            ("Alt2".into(), "Alt2".into()),
        ],
    );
    config.steps[0].step_script_path = Some("C:/scripts/farm/Ultra.ts".into());
    config.steps[0].accounts[1].script_path = Some("LoO.ts".into());
    // Unchecked accounts leave, new ones join at the end.
    config.set_members(
        0,
        &[("alt2".into(), "x".into()), ("Alt3".into(), "Alt3".into())],
    );
    let names: Vec<_> = config.steps[0]
        .accounts
        .iter()
        .map(|a| a.username.as_str())
        .collect();
    assert_eq!(names, ["Alt2", "Alt3"]);
    assert_eq!(config.steps[0].accounts[0].script_label(), "LoO.ts");
    assert_eq!(
        config.steps[0].accounts[1].script_label(),
        "(Uses Squad Script)"
    );
    assert_eq!(
        config.steps[0].script_for(&config.steps[0].accounts[1].clone()),
        (
            "C:/scripts/farm/Ultra.ts".into(),
            Some("ArmyQueue_Squad_1_Ultra".into())
        )
    );
    assert_eq!(
        config.steps[0]
            .script_for(&config.steps[0].accounts[0].clone())
            .1,
        Some("ArmyQueue_Squad_1_Alt2_LoO".into())
    );
    config.save(&active).unwrap();
    assert_eq!(QueueConfig::load_active(&active), config);

    // Drag-drop reorder and close.
    assert_eq!(config.move_step(0, 1), 1);
    assert_eq!(config.steps[1].name, "Squad 1");
    assert_eq!(config.remove_step(0, 1), 0);
    assert_eq!(config.steps.len(), 1);
    // Closing the last squad empties it instead.
    assert_eq!(config.remove_step(0, 0), 0);
    assert_eq!(config.steps[0], QueueStep::default());

    let file = dir.join(turu_bot::army::queue::PRESETS_FILE);
    let mut presets = Presets::load(&file);
    assert!(presets.names().is_empty());
    let mut named = QueueConfig::load_active(&active);
    named.name = "  Weekly  ".into();
    assert_eq!(presets.put(&named), Ok("Weekly".into()));
    named.name = String::new();
    assert!(presets.put(&named).is_err());
    presets.save(&file).unwrap();
    let read = Presets::load(&file);
    assert_eq!(read.names(), ["Weekly"]);
    let loaded = read.load_preset("weekly").unwrap();
    assert_eq!(loaded.steps.len(), 2);
    assert_eq!(loaded.delay_between_steps_seconds, 5);
    let mut read = read;
    read.remove("WEEKLY");
    assert!(read.names().is_empty());
}

/// Clients that quit when told to.
#[derive(Default)]
struct FakeLauncher {
    launched: Mutex<Vec<(String, String, Option<String>, Instant)>>,
    running: Mutex<HashMap<String, bool>>,
    missing: Vec<String>,
    failing: Vec<String>,
}

impl FakeLauncher {
    fn quit(&self, username: &str) {
        self.running.lock().insert(username.to_lowercase(), false);
    }

    fn launched(&self) -> Vec<String> {
        self.launched.lock().iter().map(|l| l.0.clone()).collect()
    }
}

impl QueueLauncher for FakeLauncher {
    fn launch(&self, username: &str, script: &str, storage: Option<&str>) -> Launched {
        if self.missing.iter().any(|m| m == username) {
            return Launched::Missing;
        }
        if self.failing.iter().any(|m| m == username) {
            return Launched::Failed("no".into());
        }
        self.launched.lock().push((
            username.into(),
            script.into(),
            storage.map(str::to_owned),
            Instant::now(),
        ));
        self.running.lock().insert(username.to_lowercase(), true);
        Launched::Started
    }

    fn exited(&self, username: &str) -> bool {
        !self
            .running
            .lock()
            .get(&username.to_lowercase())
            .copied()
            .unwrap_or(false)
    }
}

fn squad(name: &str, accounts: &[&str]) -> QueueStep {
    QueueStep {
        name: name.into(),
        step_script_path: Some("Farm.ts".into()),
        accounts: accounts.iter().map(|a| QueueAccount::new(a, a)).collect(),
    }
}

fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ok() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn queue_runs_squads_one_after_another() {
    let runner = QueueRunner::with_second(Duration::from_millis(10));
    let launcher = Arc::new(FakeLauncher {
        missing: vec!["Gone".into()],
        failing: vec!["Broken".into()],
        ..Default::default()
    });
    let config = QueueConfig {
        steps: vec![
            squad("Squad 1", &["A1", "Gone", "Broken", "A2"]),
            squad("Squad 2", &["A1", "B1"]),
        ],
        delay_between_steps_seconds: 2,
        delay_between_launches_seconds: 1,
        ..Default::default()
    };
    // Empty queues don't start.
    let empty = QueueConfig::default();
    assert!(runner.start(empty, launcher.clone()).is_err());

    runner.start(config, launcher.clone()).unwrap();
    assert!(
        runner
            .start(QueueConfig::default(), launcher.clone())
            .is_err()
    );
    wait_for("squad 1 to launch", || launcher.launched().len() == 2);
    let status = runner.status();
    assert!(status.running && status.can_pause());
    assert_eq!(status.active_step, 1);
    assert_eq!(status.steps, [StepStatus::Running, StepStatus::Pending]);
    wait_for("the statuses", || {
        let s = runner.status();
        s.member(0, 1) == MemberStatus::AccountMissing
            && s.member(0, 2) == MemberStatus::LaunchFailed
            && s.member(0, 3) == MemberStatus::Running
    });
    assert_eq!(runner.status().member(1, 0), MemberStatus::Waiting);
    let launched = launcher.launched.lock().clone();
    assert_eq!(launched[0].2.as_deref(), Some("ArmyQueue_Squad_1_Farm"));
    // The launch delay: one second between two launches (the missing account has none).
    assert!(launched[1].3 - launched[0].3 >= Duration::from_millis(10));

    // Pause: the squad finishes, the next one waits.
    runner.pause();
    assert!(runner.status().can_resume());
    launcher.quit("A1");
    wait_for("A1 finished", || {
        runner.status().member(0, 0) == MemberStatus::Finished
    });
    assert!(
        runner.status().text.contains("1/2 accounts finished"),
        "{}",
        runner.status().text
    );
    launcher.quit("A2");
    wait_for("the pause", || {
        runner.status().text.contains("Queue is PAUSED")
    });
    let status = runner.status();
    assert_eq!(status.steps[0], StepStatus::Completed);
    assert_eq!(status.progress, 50.0);
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(launcher.launched().len(), 2, "paused: squad 2 waits");

    runner.resume();
    wait_for("squad 2", || launcher.launched().len() == 4);
    assert_eq!(launcher.launched()[2..], ["A1", "B1"]);
    assert_eq!(runner.status().active_step, 2);
    launcher.quit("A1");
    launcher.quit("B1");
    wait_for("the end", || !runner.is_running());
    let status = runner.status();
    assert_eq!(status.text, "All Army Queue squads completed successfully!");
    assert_eq!(status.progress, 100.0);
    assert_eq!(status.steps, [StepStatus::Completed, StepStatus::Completed]);
    assert_eq!(status.member(1, 1), MemberStatus::Finished);
    assert_eq!(runner.take_outcome(), Some(Outcome::Finished));
    assert_eq!(runner.take_outcome(), None);
}

#[test]
fn stop_ends_the_queue() {
    let runner = QueueRunner::with_second(Duration::from_millis(10));
    let launcher = Arc::new(FakeLauncher::default());
    let config = QueueConfig {
        steps: vec![squad("One", &["A1"]), squad("Two", &["B1"])],
        ..Default::default()
    };
    runner.start(config.clone(), launcher.clone()).unwrap();
    wait_for("the launch", || launcher.launched().len() == 1);
    runner.stop();
    wait_for("the stop", || !runner.is_running());
    assert_eq!(runner.status().text, "Army Queue was stopped.");
    assert_eq!(runner.take_outcome(), Some(Outcome::Stopped));
    assert_eq!(launcher.launched(), ["A1"], "squad 2 never starts");

    // Stopped while paused between squads.
    let launcher = Arc::new(FakeLauncher::default());
    runner.start(config, launcher.clone()).unwrap();
    wait_for("the launch", || launcher.launched().len() == 1);
    runner.pause();
    launcher.quit("A1");
    wait_for("the pause", || {
        runner.status().text.contains("PAUSED. Click")
    });
    runner.stop();
    wait_for("the stop", || !runner.is_running());
    assert_eq!(launcher.launched(), ["A1"]);
    assert!(!runner.status().paused);
}
