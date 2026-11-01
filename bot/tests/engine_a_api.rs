//! map / combat / kill / monsters / options against a fake skua.swf.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use turu_bot::api::{NameOrId, kill, map, monsters, options, quests};
use turu_bot::models::Monster;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A logged-in, alive player in `battleon`. `getGameObject` reads `values` (missing paths
/// are null), and a few callbacks change them like the game would.
#[derive(Default)]
struct FakeGame {
    values: Mutex<HashMap<String, String>>,
    calls: Mutex<Vec<(String, Vec<FlashArg>)>>,
}

impl FakeGame {
    fn new() -> Arc<Self> {
        let game = Self::default();
        for (path, value) in [
            ("world", "{}"),
            ("world.myAvatar", "{}"),
            ("world.myAvatar.dataLeaf.intState", "1"),
            ("world.myAvatar.items", "[{\"ItemID\":1}]"),
            ("world.mapLoadInProgress", "false"),
            ("world.strMapName", "\"battleon\""),
            ("world.strFrame", "\"Enter\""),
            ("world.strPad", "\"Spawn\""),
            ("world.curRoom", "5"),
            ("world.lock.tfer", "{\"ts\":0,\"cd\":1000}"),
            ("loginInfo.strUsername", "\"Hero\""),
            ("getTargetMonster", "{}"),
            (
                "availableMonsters",
                r#"[{"MonID":5,"MonMapID":3,"strMonName":"Frogzard","strFrame":"Enter","intHP":100,"intHPMax":100,"intState":1}]"#,
            ),
        ] {
            game.set(path, value);
        }
        Arc::new(game)
    }

    fn set(&self, path: &str, raw: &str) {
        self.values.lock().insert(path.to_owned(), raw.to_owned());
    }

    fn get(&self, path: &str) -> String {
        self.values
            .lock()
            .get(path)
            .cloned()
            .unwrap_or_else(|| "null".into())
    }

    fn calls_to(&self, name: &str) -> Vec<Vec<FlashArg>> {
        self.calls
            .lock()
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, a)| a.clone())
            .collect()
    }

    fn packets(&self) -> Vec<String> {
        self.calls_to("callGameFunction")
            .into_iter()
            .filter(|a| a.first() == Some(&FlashArg::String("sfc.sendString".into())))
            .filter_map(|a| match a.get(1) {
                Some(FlashArg::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }
}

fn arg(args: &[FlashArg], i: usize) -> String {
    match args.get(i) {
        Some(FlashArg::String(s)) => s.clone(),
        _ => String::new(),
    }
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        self.calls.lock().push((name.to_owned(), args.clone()));
        let a0 = arg(&args, 0);
        Ok(match name {
            "isLoggedIn" => "true".into(),
            "isNull" => (!self.values.lock().contains_key(&a0)).to_string(),
            "getGameObject" | "getGameObjectS" => self.get(&a0),
            "callGameFunction0" if a0 == "world.myAvatar.pMC.artLoaded" => "true".into(),
            "callGameFunction" if a0 == "sfc.sendString" => {
                // %xt%zm%cmd%room%tfer%user%map%cell%pad%
                let packet = arg(&args, 1);
                let parts: Vec<&str> = packet.split('%').collect();
                if parts.get(5) == Some(&"tfer") {
                    let map = parts[7].split('-').next().unwrap_or_default();
                    self.set("world.strMapName", &format!("\"{map}\""));
                }
                "null".into()
            }
            "jumpCorrectRoom" => {
                self.set("world.strFrame", &format!("\"{a0}\""));
                self.set("world.strPad", &format!("\"{}\"", arg(&args, 1)));
                "null".into()
            }
            "attackMonsterName" | "attackMonsterID" => "true".into(),
            "getTargetMonster" | "availableMonsters" | "getMonsters" => self.get(name),
            _ => "null".into(),
        })
    }
}

fn new_bot(game: &Arc<FakeGame>) -> Arc<Bot> {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let bot = Bot::new();
    bot.set_host(Some(game.clone() as Arc<dyn FlashHost>));
    *bot.options.write() = Default::default();
    bot.options.write().action_delay = 10;
    bot
}

#[test]
fn private_room_names() {
    assert_eq!(map::private_room_name("battleon", false, 4242), "battleon");
    assert_eq!(
        map::private_room_name("battleon", true, 4242),
        "battleon-4242"
    );
    assert_eq!(
        map::private_room_name("battleon", true, 0),
        "battleon-100000"
    );
    assert_eq!(
        map::private_room_name("battleon-7", true, 4242),
        "battleon-7"
    );
    assert_eq!(
        map::private_room_name("battleon-1e9", false, -1),
        "battleon-100000"
    );
}

#[test]
fn join_uses_a_private_room_and_jumps() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    {
        let mut o = bot.options.write();
        o.private_rooms = true;
        o.private_number = 4242;
    }
    map::join(&bot, "Yulgar", "Room", "Left", false, true).unwrap();

    assert_eq!(
        game.packets(),
        ["%xt%zm%cmd%5%tfer%Hero%Yulgar-4242%Room%Left%"]
    );
    let jumps = game.calls_to("jumpCorrectRoom");
    assert_eq!(jumps.len(), 1);
    assert_eq!(arg(&jumps[0], 0), "Room");
    assert_eq!(bot.state.lock().last_map.as_deref(), Some("Yulgar"));
}

#[test]
fn update_quest_with_a_slot_sends_the_client_packet() {
    // VibeSkua `UpdateQuest(value, slot)`, as DragonOfTime calls it.
    let game = FakeGame::new();
    let bot = new_bot(&game);
    quests::update_quest_slot(&bot, 9, 159).unwrap();

    let sent = game.calls_to("sendClientPacket");
    assert_eq!(sent.len(), 1);
    assert_eq!(
        arg(&sent[0], 0),
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"updateQuest","iValue":9,"iIndex":159}}}"#
    );
    assert_eq!(arg(&sent[0], 1), "json");
}

#[test]
fn join_skips_the_current_map() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    map::join(&bot, "battleon", "Enter", "Spawn", false, true).unwrap();
    assert!(game.packets().is_empty());
}

#[test]
fn kill_attacks_by_name_or_map_id_until_no_target() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    kill::monster(&bot, NameOrId::Name("frogzard")).unwrap();
    kill::monster(&bot, NameOrId::Id(3)).unwrap();
    assert_eq!(arg(&game.calls_to("attackMonsterName")[0], 0), "frogzard");
    assert_eq!(game.calls_to("attackMonsterID")[0], [FlashArg::Number(3.0)]);
    assert!(!game.calls_to("getTargetMonster").is_empty());
    assert!(monsters::exists(&bot, NameOrId::Name("*")).unwrap());
    assert!(monsters::exists(&bot, NameOrId::Id(5)).unwrap());
    assert!(!monsters::exists(&bot, NameOrId::Name("Slime")).unwrap());
}

#[test]
fn living_monster_cells() {
    let mon = |id, map_id, name: &str, cell: &str, hp| Monster {
        id,
        map_id,
        name: name.into(),
        cell: cell.into(),
        hp,
        max_hp: 100,
        state: if hp > 0 { 1 } else { 0 },
        ..Default::default()
    };
    let all = [
        mon(1, 1, "Frogzard", "r2", 100),
        mon(1, 2, "Frogzard", "r3", 0),
        mon(1, 3, "Big Frogzard", "r4", 100),
        mon(2, 4, "Slime", "r2", 100),
        mon(2, 5, "Slime", "r5", 100),
    ];
    assert_eq!(
        monsters::living_cells_in(&all, NameOrId::Name("frogzard")),
        ["r2"]
    );
    // Resolved to MonID 1: every monster of that type.
    assert_eq!(
        monsters::living_data_leaf_cells_in(&all, NameOrId::Name("Frogzard")),
        ["r2", "r4"]
    );
    assert_eq!(
        monsters::living_data_leaf_cells_in(&all, NameOrId::Name("*")),
        ["r2", "r4", "r5"]
    );
    assert_eq!(
        monsters::living_cells_in(&all, NameOrId::Id(2)),
        ["r2", "r5"]
    );
}

#[test]
fn options_send_toggles_once_and_repeat_ticks() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    {
        let mut o = bot.options.write();
        o.hide_players = true;
        o.lag_killer = true;
        o.walk_speed = 12;
    }
    options::apply(&bot).unwrap();
    options::apply(&bot).unwrap();
    assert_eq!(
        game.calls_to("modEnable"),
        [[FlashArg::String("HidePlayers".into())]]
    );
    assert!(game.calls_to("modDisable").is_empty());
    assert_eq!(game.calls_to("killLag").len(), 2);
    let speed_sets = game
        .calls_to("setGameObject")
        .into_iter()
        .filter(|a| arg(a, 0) == "world.WALKSPEED")
        .count();
    assert_eq!(speed_sets, 2);
    // Lite's untarget options are forced on.
    assert!(
        game.calls_to("setGameObject")
            .iter()
            .any(|a| arg(a, 0) == "litePreference.data.bUntargetSelf")
    );

    {
        let mut o = bot.options.write();
        o.hide_players = false;
        o.lag_killer = false;
    }
    options::apply(&bot).unwrap();
    options::apply(&bot).unwrap();
    assert_eq!(
        game.calls_to("modDisable"),
        [[FlashArg::String("HidePlayers".into())]]
    );
    assert_eq!(
        game.calls_to("killLag").last().unwrap(),
        &[FlashArg::Bool(false)]
    );
    assert_eq!(game.calls_to("killLag").len(), 3);
}

#[test]
fn skipping_a_cutscene_shows_the_world_unless_lag_killer_is_on() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    let world_shown = |game: &FakeGame| {
        game.calls_to("setGameObject")
            .iter()
            .filter(|a| arg(a, 0) == "world.visible")
            .count()
    };
    bot.options.write().skip_cutscenes = true;
    options::apply(&bot).unwrap();
    assert_eq!(game.calls_to("skipCutscenes").len(), 1);
    assert_eq!(world_shown(&game), 0, "no cutscene, nothing to show");

    game.set("mcExtSWF.numChildren", "1");
    bot.options.write().lag_killer = true;
    options::apply(&bot).unwrap();
    assert_eq!(world_shown(&game), 0, "Lag Killer keeps the world hidden");

    bot.options.write().lag_killer = false;
    options::apply(&bot).unwrap();
    assert_eq!(world_shown(&game), 1);

    // A stopping script shows the world even when Lag Killer was never on.
    let kills = game.calls_to("killLag").len();
    bot.state.lock().options_applied.restore_world();
    options::apply(&bot).unwrap();
    options::apply(&bot).unwrap();
    assert_eq!(game.calls_to("killLag").len(), kills + 1);
    assert_eq!(
        game.calls_to("killLag").last().unwrap(),
        &[FlashArg::Bool(false)]
    );
}

fn frame_rates(game: &FakeGame) -> Vec<FlashArg> {
    game.calls_to("setGameObject")
        .into_iter()
        .filter(|a| arg(a, 0) == "stage.frameRate")
        .map(|a| a[1].clone())
        .collect()
}

#[test]
fn set_fps_zero_leaves_the_game_frame_rate_alone() {
    let game = FakeGame::new();
    game.set("stage.frameRate", "40");
    let bot = new_bot(&game);
    options::apply(&bot).unwrap();
    assert!(frame_rates(&game).is_empty());

    // Headless Mode drops to 2 and gives back the rate the game had.
    bot.options.write().headless_mode = true;
    options::apply(&bot).unwrap();
    bot.options.write().headless_mode = false;
    options::apply(&bot).unwrap();
    options::apply(&bot).unwrap();
    assert_eq!(
        frame_rates(&game),
        [FlashArg::Number(2.0), FlashArg::Number(40.0)]
    );

    // A Set FPS above 0 is forced once.
    bot.options.write().set_fps = 30;
    options::apply(&bot).unwrap();
    options::apply(&bot).unwrap();
    assert_eq!(frame_rates(&game).last(), Some(&FlashArg::Number(30.0)));
    assert_eq!(frame_rates(&game).len(), 3);
}

#[test]
fn hidden_tabs_slow_down_and_come_back() {
    let game = FakeGame::new();
    game.set("stage.frameRate", "24");
    game.set("stage.quality", "\"MEDIUM\"");
    let bot = new_bot(&game);
    options::apply(&bot).unwrap();

    // VibeSkua's tabbed host: a hidden tab runs at 2 FPS. Its quality stays (it isn't drawn).
    bot.state.lock().hidden_tab = true;
    options::apply(&bot).unwrap();
    options::apply(&bot).unwrap();
    assert_eq!(frame_rates(&game), [FlashArg::Number(2.0)]);
    assert!(qualities(&game).is_empty());

    // Headless Mode while hidden only lowers the quality; shown again, the tab stays slowed by
    // it.
    bot.options.write().headless_mode = true;
    options::apply(&bot).unwrap();
    bot.state.lock().hidden_tab = false;
    options::apply(&bot).unwrap();
    assert_eq!(frame_rates(&game), [FlashArg::Number(2.0)]);
    assert_eq!(qualities(&game), ["LOW"]);

    // Neither: the rate and quality from before.
    bot.options.write().headless_mode = false;
    options::apply(&bot).unwrap();
    assert_eq!(
        frame_rates(&game),
        [FlashArg::Number(2.0), FlashArg::Number(24.0)]
    );
    assert_eq!(qualities(&game), ["LOW", "MEDIUM"]);
}

fn qualities(game: &FakeGame) -> Vec<String> {
    game.calls_to("setGameObject")
        .into_iter()
        .filter(|a| arg(a, 0) == "stage.quality")
        .map(|a| arg(&a, 1))
        .collect()
}
