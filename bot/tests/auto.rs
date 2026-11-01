//! Auto Attack / Auto Hunt (`turu_bot::auto`) against a fake skua.swf whose monsters live in
//! several cells and die on the first hit.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::auto::{self, AutoMode};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

struct Mon {
    id: i64,
    map_id: i64,
    name: &'static str,
    cell: &'static str,
    hp: i64,
}

impl Mon {
    fn json(&self) -> String {
        format!(
            r#"{{"MonID":{},"MonMapID":{},"strMonName":"{}","strFrame":"{}","intHP":{},"intHPMax":100,"intState":{}}}"#,
            self.id,
            self.map_id,
            self.name,
            self.cell,
            self.hp,
            i64::from(self.hp > 0)
        )
    }
}

/// A logged-in player in `Enter`; attacking a monster of your cell kills it and targets it.
#[derive(Default)]
struct FakeGame {
    values: Mutex<HashMap<String, String>>,
    monsters: Mutex<Vec<Mon>>,
    /// The targeted map ID (shown only while it's alive).
    target: Mutex<Option<i64>>,
    /// Map IDs in the order they died.
    kills: Mutex<Vec<i64>>,
    /// Cells jumped to.
    jumps: Mutex<Vec<String>>,
}

impl FakeGame {
    fn new(monsters: Vec<Mon>) -> Arc<Self> {
        let game = Self::default();
        for (path, value) in [
            ("world", "{}"),
            ("world.myAvatar", "{}"),
            ("world.myAvatar.dataLeaf.intState", "1"),
            ("world.mapLoadInProgress", "false"),
            ("world.strMapName", "\"battleon\""),
            ("world.strFrame", "\"Enter\""),
            ("world.strPad", "\"Spawn\""),
            ("loginInfo.strUsername", "\"Hero\""),
        ] {
            game.values.lock().insert(path.into(), value.into());
        }
        *game.monsters.lock() = monsters;
        Arc::new(game)
    }

    fn cell(&self) -> String {
        let frame = self.values.lock()["world.strFrame"].clone();
        frame.trim_matches('"').to_owned()
    }

    fn alive(&self, map_id: i64) -> bool {
        self.monsters
            .lock()
            .iter()
            .any(|m| m.map_id == map_id && m.hp > 0)
    }

    /// Kills the first living monster of your cell that `hit` picks.
    fn hit(&self, hit: impl Fn(&Mon) -> bool) -> bool {
        let cell = self.cell();
        let mut monsters = self.monsters.lock();
        let Some(m) = monsters
            .iter_mut()
            .find(|m| m.cell == cell && m.hp > 0 && hit(m))
        else {
            return false;
        };
        m.hp = 0;
        *self.target.lock() = Some(m.map_id);
        self.kills.lock().push(m.map_id);
        true
    }
}

fn arg(args: &[FlashArg], i: usize) -> String {
    match args.get(i) {
        Some(FlashArg::String(s)) => s.clone(),
        Some(FlashArg::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let a0 = arg(&args, 0);
        Ok(match name {
            "isLoggedIn" => "true".into(),
            "isNull" => (!self.values.lock().contains_key(&a0)).to_string(),
            "getGameObject" | "getGameObjectS" => self
                .values
                .lock()
                .get(&a0)
                .cloned()
                .unwrap_or_else(|| "null".into()),
            "jumpCorrectRoom" => {
                self.jumps.lock().push(a0.clone());
                let mut values = self.values.lock();
                values.insert("world.strFrame".into(), format!("\"{a0}\""));
                values.insert("world.strPad".into(), format!("\"{}\"", arg(&args, 1)));
                "null".into()
            }
            "getMonsters" => {
                let all: Vec<String> = self.monsters.lock().iter().map(Mon::json).collect();
                format!("[{}]", all.join(","))
            }
            "availableMonsters" => {
                let cell = self.cell();
                let here: Vec<String> = self
                    .monsters
                    .lock()
                    .iter()
                    .filter(|m| m.cell == cell && m.hp > 0)
                    .map(Mon::json)
                    .collect();
                format!("[{}]", here.join(","))
            }
            "getTargetMonster" => {
                let target = *self.target.lock();
                self.monsters
                    .lock()
                    .iter()
                    .find(|m| Some(m.map_id) == target && m.hp > 0)
                    .map_or_else(|| "{}".into(), Mon::json)
            }
            "attackMonsterID" => {
                let id: i64 = a0.parse::<f64>().unwrap_or(-1.0) as i64;
                self.hit(|m| m.map_id == id || m.id == id).to_string()
            }
            "attackMonsterName" => self
                .hit(|m| m.name.eq_ignore_ascii_case(a0.trim()))
                .to_string(),
            _ => "null".into(),
        })
    }
}

fn mon(id: i64, map_id: i64, name: &'static str, cell: &'static str) -> Mon {
    Mon {
        id,
        map_id,
        name,
        cell,
        hp: 100,
    }
}

fn new_bot(game: &Arc<FakeGame>) -> Arc<Bot> {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let bot = Bot::new();
    bot.set_host(Some(game.clone() as Arc<dyn FlashHost>));
    *bot.options.write() = Default::default();
    bot.options.write().action_delay = 10;
    bot.options.write().hunt_delay = 10;
    bot
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Stops it and waits for its thread to finish.
fn stop(bot: &Arc<Bot>) {
    auto::stop(bot);
    wait_for("stopped", || !bot.auto.is_running());
}

#[test]
fn hunt_follows_the_target_across_cells() {
    let game = FakeGame::new(vec![
        mon(5, 1, "Frogzard", "Enter"),
        mon(6, 2, "Slime", "r2"),
        mon(5, 3, "Frogzard", "r2"),
        mon(5, 4, "Frogzard", "r3"),
    ]);
    *game.target.lock() = Some(1);
    let bot = new_bot(&game);

    assert!(auto::start(
        &bot,
        AutoMode::Hunt,
        None,
        "Base".into(),
        vec![]
    ));
    assert_eq!(bot.auto.mode(), Some(AutoMode::Hunt));
    assert!(!auto::start(
        &bot,
        AutoMode::Attack,
        None,
        "Base".into(),
        vec![]
    ));
    wait_for("every Frogzard dead", || {
        !game.alive(1) && !game.alive(3) && !game.alive(4)
    });
    stop(&bot);

    assert!(game.alive(2), "the Slime isn't hunted");
    let jumps = game.jumps.lock().clone();
    assert!(
        jumps.contains(&"r2".into()) && jumps.contains(&"r3".into()),
        "{jumps:?}"
    );
    assert!(
        !bot.options.read().infinite_range,
        "turned off again when stopped"
    );
    let logs = bot.logs.script.text();
    assert!(
        logs.contains("[Auto Hunt] Hunting for Frogzard"),
        "{logs:?}"
    );
    assert!(logs.contains("[Auto Hunt] Stopped"), "{logs:?}");
}

#[test]
fn hunt_takes_the_manual_map_ids_in_priority_order() {
    let game = FakeGame::new(vec![
        mon(5, 1, "Frogzard", "Enter"),
        mon(6, 2, "Slime", "r2"),
        mon(7, 3, "Bat", "r3"),
    ]);
    let bot = new_bot(&game);

    assert!(auto::start(
        &bot,
        AutoMode::Hunt,
        None,
        "Base".into(),
        vec![3, 1]
    ));
    wait_for("both dead", || !game.alive(3) && !game.alive(1));
    stop(&bot);

    assert_eq!(*game.kills.lock(), [3, 1]);
    assert!(game.alive(2));
    assert_eq!(game.jumps.lock().first().map(String::as_str), Some("r3"));
}

#[test]
fn hunt_without_a_target_hunts_every_monster_of_the_cell() {
    let game = FakeGame::new(vec![
        mon(5, 1, "Frogzard", "Enter"),
        mon(6, 2, "Slime", "Enter"),
        mon(5, 3, "Frogzard", "r2"),
        mon(6, 4, "Slime", "r3"),
        mon(7, 5, "Bat", "r4"),
    ]);
    let bot = new_bot(&game);

    assert!(auto::start(
        &bot,
        AutoMode::Hunt,
        None,
        "Base".into(),
        vec![]
    ));
    wait_for("every Frogzard and Slime dead", || {
        (1..=4).all(|id| !game.alive(id))
    });
    stop(&bot);

    assert!(game.alive(5), "the Bat wasn't in your cell");
    let logs = bot.logs.script.text();
    assert!(logs.contains("Hunting for Frogzard|Slime"), "{logs:?}");
}

#[test]
fn stop_ends_a_hunt_with_nothing_to_hunt() {
    let game = FakeGame::new(vec![mon(5, 1, "Frogzard", "r2")]);
    let bot = new_bot(&game);

    // Nothing in your cell and no target: it waits for monsters to appear.
    assert!(auto::start(
        &bot,
        AutoMode::Hunt,
        None,
        "Base".into(),
        vec![]
    ));
    std::thread::sleep(Duration::from_millis(300));
    assert!(bot.auto.is_running());
    stop(&bot);
    assert_eq!(bot.auto.mode(), None);
    assert!(game.alive(1));

    // It can start again after a stop.
    assert!(auto::start(
        &bot,
        AutoMode::Hunt,
        None,
        "Base".into(),
        vec![1]
    ));
    wait_for("dead", || !game.alive(1));
    stop(&bot);
}

#[test]
fn attack_stays_in_the_cell_and_follows_the_priority() {
    let game = FakeGame::new(vec![
        mon(5, 1, "Frogzard", "Enter"),
        mon(6, 2, "Slime", "Enter"),
        mon(7, 3, "Bat", "r2"),
    ]);
    let bot = new_bot(&game);

    assert!(auto::start(
        &bot,
        AutoMode::Attack,
        None,
        "Base".into(),
        vec![2, 1]
    ));
    assert_eq!(bot.auto.mode(), Some(AutoMode::Attack));
    wait_for("both in the cell dead", || !game.alive(2) && !game.alive(1));
    stop(&bot);

    assert_eq!(*game.kills.lock(), [2, 1]);
    assert!(game.alive(3), "Auto Attack doesn't jump");
    assert!(game.jumps.lock().is_empty());
}

#[test]
fn parses_map_ids() {
    assert_eq!(auto::parse_map_ids("1, 2 3;x;4"), [1, 2, 3, 4]);
    assert!(auto::parse_map_ids("").is_empty());
}
