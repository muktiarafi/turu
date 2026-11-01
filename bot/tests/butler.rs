//! Butler (VibeSkua ButlerV5): the sync file, and a master summoning a butler, both on fake
//! games that share one data folder; the follower's quest, level and AntiLag fakes, the
//! suspend question and the offline write on close.

use parking_lot::{Condvar, Mutex};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};
use turu_bot::butler::sync::{SyncData, SyncDir, master_ordering, now};
use turu_bot::butler::{self, ButlerSettings, FollowSource, antilag, quest_bypass, room_of};
use turu_bot::{Bot, FlashArg, FlashHost, HostError, script};

/// Held by the tests that follow: they share `butler.json`, and some change it.
static FOLLOWS: Mutex<()> = Mutex::new(());

#[test]
fn sync_data_round_trips_and_goes_stale() {
    let data = SyncData {
        username: "Hero".into(),
        pid: 42,
        server: "Twilly".into(),
        class_name: "Void Highlord".into(),
        map: "ultradage".into(),
        room: "100000".into(),
        cell: "Boss".into(),
        pad: "Left".into(),
        logged_in: true,
        attacking: true,
        followers: vec!["Alt1".into(), "Alt2".into()],
        script_on: true,
        script_name: "Farm.ts".into(),
        beat: now(),
        ..Default::default()
    };
    let back = SyncData::parse(&data.serialize()).unwrap();
    assert_eq!(back, data);
    assert_eq!(back.map_with_room(), "ultradage-100000");
    assert!(back.lists_follower("alt2"));
    assert!(back.is_online(now()));
    assert!(!back.is_online(now() + 60), "an old beat is offline");
    assert!(
        !SyncData {
            logged_in: false,
            ..back.clone()
        }
        .is_online(now())
    );
    assert!(back.same_state(&SyncData {
        beat: 1,
        ..back.clone()
    }));
    assert_eq!(SyncData::parse("pid=1\nmap=x\n"), None, "no username");
    let online = [back];
    assert_eq!(
        master_ordering("ALT1", &online).map(|m| m.username.as_str()),
        Some("Hero")
    );
    assert!(master_ordering("Other", &online).is_none());
}

#[test]
fn room_comes_from_the_area_title() {
    assert_eq!(room_of("ultradage-100000").as_deref(), Some("100000"));
    assert_eq!(room_of("battleon").as_deref(), None);
    assert_eq!(room_of("house-abc").as_deref(), None);
}

#[test]
fn sync_dir_lists_online_accounts_but_self() {
    let dir = std::env::temp_dir().join(format!("turu-butler-dir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sync = SyncDir::new(&dir);
    for (name, logged_in, beat) in [
        ("Hero", true, now()),
        ("Alt", true, now()),
        ("Gone", true, now() - 120),
        ("Out", false, now()),
    ] {
        sync.write(&SyncData {
            username: name.into(),
            logged_in,
            beat,
            ..Default::default()
        })
        .unwrap();
    }
    let names: Vec<String> = sync
        .online(Some("hero"))
        .into_iter()
        .map(|d| d.username)
        .collect();
    assert_eq!(names, ["Alt"]);
    assert_eq!(sync.read_user("HERO").unwrap().username, "Hero");
    let _ = std::fs::remove_dir_all(&dir);
}

/// One account's game: logged in on Artix, at `map-room`/`cell`. `world.goto` lands on the
/// other game's spot, a transfer packet changes the map, `jumpCorrectRoom` the cell.
struct FakeGame {
    values: Mutex<HashMap<String, String>>,
    calls: Mutex<Vec<(String, Vec<FlashArg>)>>,
    /// The other account's game, for goto and `world.uoTree`.
    other: Mutex<Option<(String, Arc<FakeGame>)>>,
    /// While set, reading the cell waits for `release` (a game call the UI thread must
    /// answer, as in the real client).
    hold_cell: AtomicBool,
    cell_held: AtomicBool,
    released: (Mutex<bool>, Condvar),
}

impl FakeGame {
    fn new(username: &str, map: &str, room: &str, cell: &str) -> Arc<Self> {
        let game = Self {
            values: Mutex::new(HashMap::new()),
            calls: Mutex::new(Vec::new()),
            other: Mutex::new(None),
            hold_cell: AtomicBool::new(false),
            cell_held: AtomicBool::new(false),
            released: (Mutex::new(false), Condvar::new()),
        };
        for (path, value) in [
            ("world", "{}".to_owned()),
            ("world.myAvatar", "{}".to_owned()),
            ("world.myAvatar.dataLeaf.intState", "1".to_owned()),
            ("world.myAvatar.items", r#"[{"ItemID":1}]"#.to_owned()),
            ("world.myAvatar.objData.CharID", "77".to_owned()),
            ("world.mapLoadInProgress", "false".to_owned()),
            ("world.curRoom", "3".to_owned()),
            ("loginInfo.strUsername", format!("\"{username}\"")),
            ("objServerInfo.sName", "\"Artix\"".to_owned()),
            ("uoPref.bGoto", "true".to_owned()),
        ] {
            game.set(path, &value);
        }
        game.place(map, room, cell, "Spawn");
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

    fn place(&self, map: &str, room: &str, cell: &str, pad: &str) {
        self.set("world.strMapName", &format!("\"{map}\""));
        self.set(
            "ui.mcInterface.areaList.title.t1.text",
            &format!("\"Area: {map}-{room}\""),
        );
        self.set("world.strFrame", &format!("\"{cell}\""));
        self.set("world.strPad", &format!("\"{pad}\""));
    }

    fn spot(&self) -> (String, String, String) {
        let unquote = |v: String| v.trim_matches('"').to_owned();
        let title = unquote(self.get("ui.mcInterface.areaList.title.t1.text"));
        let full = title.trim_start_matches("Area: ").to_owned();
        let (map, room) = full.rsplit_once('-').unwrap();
        (
            map.to_owned(),
            room.to_owned(),
            unquote(self.get("world.strFrame")),
        )
    }

    fn calls_to(&self, name: &str) -> Vec<Vec<FlashArg>> {
        self.calls
            .lock()
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, a)| a.clone())
            .collect()
    }

    fn game_function_calls(&self, function: &str) -> Vec<Vec<FlashArg>> {
        self.calls_to("callGameFunction")
            .into_iter()
            .filter(|a| arg(a, 0) == function)
            .collect()
    }

    fn packets(&self) -> Vec<String> {
        self.game_function_calls("sfc.sendString")
            .into_iter()
            .map(|a| arg(&a, 1))
            .collect()
    }

    /// The packets faked to this client (`sendClientPacket`).
    fn client_packets(&self) -> Vec<String> {
        self.calls_to("sendClientPacket")
            .into_iter()
            .map(|a| arg(&a, 0))
            .collect()
    }

    fn update_quest_packets(&self) -> Vec<String> {
        self.client_packets()
            .into_iter()
            .filter(|p| p.contains("\"updateQuest\""))
            .collect()
    }

    fn sets_of(&self, path: &str) -> Vec<String> {
        self.calls_to("setGameObject")
            .into_iter()
            .filter(|a| arg(a, 0) == path)
            .map(|a| to_json(&a[1]))
            .collect()
    }
}

fn to_json(value: &FlashArg) -> String {
    fn convert(value: &FlashArg) -> serde_json::Value {
        match value {
            FlashArg::Null => serde_json::Value::Null,
            FlashArg::Bool(b) => (*b).into(),
            FlashArg::Number(n) => (*n).into(),
            FlashArg::String(s) => s.clone().into(),
            FlashArg::List(l) => l.iter().map(convert).collect(),
            FlashArg::Object(o) => o.iter().map(|(k, v)| (k.clone(), convert(v))).collect(),
        }
    }
    convert(value).to_string()
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
        if name == "getGameObject"
            && a0 == "world.strFrame"
            && self.hold_cell.load(Ordering::SeqCst)
        {
            self.cell_held.store(true, Ordering::SeqCst);
            let (released, wake) = &self.released;
            let mut released = released.lock();
            while !*released {
                wake.wait(&mut released);
            }
        }
        Ok(match name {
            "isLoggedIn" => "true".into(),
            "isNull" => (!self.values.lock().contains_key(&a0)).to_string(),
            "getGameObject" | "getGameObjectS" => self.get(&a0),
            "getGameObjectKey" if a0 == "world.uoTree" => {
                let other = self.other.lock().clone();
                match other {
                    Some((name, game)) if name.eq_ignore_ascii_case(&arg(&args, 1)) => {
                        let (map, room, cell) = game.spot();
                        if self.spot().0 == map && self.spot().1 == room {
                            format!(r#"{{"strFrame":"{cell}","strPad":"Left"}}"#)
                        } else {
                            "null".into()
                        }
                    }
                    _ => "null".into(),
                }
            }
            "getTargetMonster" => "{}".into(),
            "callGameFunction0" if a0 == "world.myAvatar.pMC.artLoaded" => "true".into(),
            "callGameFunction" if a0 == "world.goto" => {
                if let Some((_, game)) = self.other.lock().clone() {
                    let (map, room, cell) = game.spot();
                    self.place(&map, &room, &cell, "Left");
                }
                "null".into()
            }
            "callGameFunction" if a0 == "sfc.sendString" => {
                // %xt%zm%cmd%room%tfer%user%map-room%cell%pad%
                let packet = arg(&args, 1);
                let parts: Vec<&str> = packet.split('%').collect();
                if parts.get(5) == Some(&"tfer") {
                    let (map, room) = parts[7].rsplit_once('-').unwrap_or((parts[7], "1"));
                    self.place(map, room, parts[8], parts[9]);
                }
                "null".into()
            }
            "jumpCorrectRoom" => {
                let (map, room, _) = self.spot();
                self.place(&map, &room, &a0, &arg(&args, 1));
                "null".into()
            }
            "attackMonsterName" | "attackMonsterID" => "true".into(),
            "setGameObject" => {
                self.set(&a0, &to_json(&args[1]));
                "null".into()
            }
            "callGameFunction0" if a0 == "world.toggleMonsters" => {
                let hidden = self.get("ui.monsterIcon.redX.visible") == "true";
                self.set("ui.monsterIcon.redX.visible", &(!hidden).to_string());
                "null".into()
            }
            _ => "null".into(),
        })
    }
}

fn until(what: &str, timeout: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// One data folder for every bot of this test binary, set before the first one reads it.
fn data_dir() -> std::path::PathBuf {
    static INIT: Once = Once::new();
    let data = std::env::temp_dir().join(format!("turu-butler-test-{}", std::process::id()));
    INIT.call_once(|| {
        let _ = std::fs::remove_dir_all(&data);
        // SAFETY: set once, before any bot of this test binary reads it.
        unsafe { std::env::set_var("TURU_DATA_DIR", &data) };
    });
    data
}

fn client(game: &Arc<FakeGame>) -> Arc<Bot> {
    data_dir();
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    bot.on_flash_call("loaded", vec![]);
    bot
}

#[test]
fn a_summoned_butler_goes_to_its_master_fights_and_parks_on_release() {
    let _follows = FOLLOWS.lock();
    let master_game = FakeGame::new("Master", "battleon", "5", "r2");
    let alt_game = FakeGame::new("Alt", "yulgar", "1", "Enter");
    *alt_game.other.lock() = Some(("Master".into(), master_game.clone()));
    let master = client(&master_game);
    let alt = client(&alt_game);

    until("both files", Duration::from_secs(5), || {
        master.butler.username() == "Master"
            && alt.butler.roster().iter().any(|d| d.username == "Master")
    });
    let message = butler::toggle_follower(&master, "Alt").unwrap();
    assert_eq!(message, "Summoned Alt to follow me.");

    until("the follow", Duration::from_secs(5), || {
        alt.butler.following() == Some(("Master".into(), FollowSource::Ordered))
    });
    until(
        "the goto and the cell jump",
        Duration::from_secs(10),
        || alt_game.spot() == ("battleon".into(), "5".into(), "r2".into()),
    );
    assert!(!alt_game.game_function_calls("world.goto").is_empty());
    until("the attack", Duration::from_secs(5), || {
        alt_game
            .calls_to("attackMonsterName")
            .iter()
            .any(|a| arg(a, 0) == "*")
    });
    assert!(alt.opts().infinite_range, "on while following");
    // AntiLag while summoned is on by default.
    assert!(alt.opts().lag_killer);
    assert_eq!(
        alt_game.get("litePreference.data.bDisMonAnim"),
        "true",
        "the Lite animation cuts"
    );
    // The master's roster shows the butler following it.
    until("the following state", Duration::from_secs(5), || {
        master
            .butler
            .roster()
            .iter()
            .any(|d| d.username == "Alt" && d.following == "Master")
    });

    // No house: a released butler parks in whitemap.
    butler::release_all(&master);
    until("the release", Duration::from_secs(5), || {
        alt.butler.following().is_none()
    });
    until("the park", Duration::from_secs(10), || {
        alt_game
            .packets()
            .iter()
            .any(|p| p.contains("%tfer%Alt%whitemap-100000%"))
    });
    until("the options back", Duration::from_secs(5), || {
        !alt.opts().infinite_range
    });
    assert!(!alt.opts().lag_killer, "AntiLag restored");
    assert_eq!(alt_game.get("litePreference.data.bDisMonAnim"), "false");

    // A summon on hold while a script runs is covered by `obey_orders`; a manual stop of
    // an ordered follow declines it, and the master releases the butler.
    butler::toggle_follower(&master, "Alt").unwrap();
    until("the second follow", Duration::from_secs(5), || {
        alt.butler.following().is_some()
    });
    butler::stop_follow(&alt);
    until("the decline handshake", Duration::from_secs(5), || {
        master.butler.followers().is_empty()
    });
    assert!(alt.butler.following().is_none());

    // A butler can't summon, and nobody summons twice.
    assert!(butler::start_follow(&alt, "Master").is_ok());
    assert!(butler::toggle_follower(&alt, "Master").is_err());
    butler::stop_follow(&alt);
}

/// The UI thread answers the game's calls and reads Butler's state every frame: no Butler lock
/// it reads may be held across a game call, or both wait for each other (the client froze and
/// the games disconnected).
#[test]
fn butler_state_reads_never_wait_for_a_game_call() {
    let game = FakeGame::new("Gated", "battleon", "7", "Enter");
    let bot = client(&game);
    until("a broadcast", Duration::from_secs(5), || {
        bot.butler.username() == "Gated"
    });
    game.hold_cell.store(true, Ordering::SeqCst);
    until("the held cell read", Duration::from_secs(5), || {
        game.cell_held.load(Ordering::SeqCst)
    });

    // What the Butler window reads, while the broadcast thread waits for the game.
    let reader = bot.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = (
            reader.butler.username(),
            reader.butler.server(),
            reader.butler.following(),
            reader.butler.followers(),
            reader.butler.is_parked(),
            reader.butler.is_parking(),
            reader.butler.settings(),
            reader.butler.roster(),
        );
        let _ = done_tx.send(());
    });
    let answered = done_rx.recv_timeout(Duration::from_secs(2)).is_ok();

    game.hold_cell.store(false, Ordering::SeqCst);
    let (released, wake) = &game.released;
    *released.lock() = true;
    wake.notify_all();
    assert!(answered, "reading Butler's state waited for a game call");
}

/// A quest tree with these `(id, slot, value)` quests loaded.
fn quest_tree(quests: &[(i64, i64, i64)]) -> String {
    let entries: Vec<String> = quests
        .iter()
        .map(|(id, slot, value)| {
            format!(r#""{id}":{{"QuestID":{id},"sName":"Q{id}","iSlot":{slot},"iValue":{value}}}"#)
        })
        .collect();
    format!("{{{}}}", entries.join(","))
}

fn update_quest(value: i64, slot: i64) -> String {
    format!(
        r#"{{"t":"xt","b":{{"r":-1,"o":{{"cmd":"updateQuest","iValue":{value},"iIndex":{slot}}}}}}}"#
    )
}

#[test]
fn the_bypass_table_fakes_a_maps_quests_on_this_client() {
    let game = FakeGame::new("Faker", "battleon", "2", "Enter");
    game.set(
        "world.questTree",
        &quest_tree(&[(3004, 10, 2), (3008, 11, 5), (793, 12, 1)]),
    );
    let bot = client(&game);
    let cancel = AtomicBool::new(false);

    quest_bypass::apply_for_map(&bot, "doomvaultb", &cancel);
    assert_eq!(
        game.client_packets(),
        [
            update_quest(2, 10),
            update_quest(5, 11),
            quest_bypass::ACHIEVEMENT_PACKET.to_owned()
        ],
        "both quests, then the achievement"
    );
    assert!(
        game.game_function_calls("world.showQuests")
            .iter()
            .any(|a| arg(a, 1) == "3004")
    );

    quest_bypass::apply_for_map(&bot, "battleon", &cancel);
    assert_eq!(
        game.client_packets().len(),
        3,
        "nothing for an unlisted map"
    );
    quest_bypass::apply_custom(&bot, &quest_bypass::parse_custom_ids("793, x"), &cancel);
    assert_eq!(game.client_packets().last(), Some(&update_quest(1, 12)));

    // A quest that never loads is skipped once the follow stops.
    cancel.store(true, Ordering::SeqCst);
    let start = Instant::now();
    quest_bypass::apply_custom(&bot, &[999_999], &cancel);
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(game.client_packets().len(), 4);
}

#[test]
fn unlock_all_and_the_level_fake() {
    let game = FakeGame::new("Unlocker", "battleon", "2", "Enter");
    game.set(
        "world.myAvatar.objData",
        r#"{"strQuests":"0a1B","strQuests2":"7","strQuests3":"","intGold":5}"#,
    );
    game.set("world.myAvatar.dataLeaf.intLevel", "35");
    let bot = client(&game);

    quest_bypass::unlock_all(&bot);
    assert_eq!(
        game.sets_of("world.myAvatar.objData.strQuests"),
        [r#""zzzz""#]
    );
    assert_eq!(
        game.sets_of("world.myAvatar.objData.strQuests2"),
        [r#""z""#]
    );
    assert!(
        game.sets_of("world.myAvatar.objData.strQuests3").is_empty(),
        "an empty string stays empty"
    );

    quest_bypass::apply_level_fake(&bot);
    assert_eq!(game.client_packets(), [quest_bypass::LEVEL_PACKET]);
    game.set("world.myAvatar.dataLeaf.intLevel", "100");
    quest_bypass::apply_level_fake(&bot);
    assert_eq!(game.client_packets().len(), 1, "already level 100");
}

#[test]
fn antilag_puts_the_users_settings_back_exactly() {
    let game = FakeGame::new("Lagger", "battleon", "2", "Enter");
    game.set("litePreference.data.bDisMonAnim", "true");
    game.set("litePreference.data.bDisAuraAnim", "true");
    game.set("ui.monsterIcon.redX.visible", "false");
    let bot = client(&game);
    bot.options.write().lag_killer = false;

    let before = antilag::capture(&bot).unwrap();
    assert_eq!(before.lite, [false, true, false, false, false, false, true]);
    assert!(!before.lag_killer && !before.monsters_hidden);

    antilag::set(&bot, true).unwrap();
    assert!(bot.opts().lag_killer);
    for key in antilag::LITE_KEYS {
        assert_eq!(
            game.get(&format!("litePreference.data.{key}")),
            "true",
            "{key}"
        );
    }
    assert_eq!(
        game.get("ui.monsterIcon.redX.visible"),
        "true",
        "monsters hidden"
    );

    antilag::restore(&bot, Some(&before));
    assert_eq!(antilag::capture(&bot).unwrap(), before);
    assert_eq!(
        game.game_function_calls("world.toggleMonsters").len(),
        0,
        "toggleMonsters takes no arguments"
    );
    assert_eq!(
        game.calls_to("callGameFunction0")
            .iter()
            .filter(|a| arg(a, 0) == "world.toggleMonsters")
            .count(),
        2
    );
}

/// Waits for the follower thread to end (the follow's options are back by then).
fn until_follow_ended(bot: &Bot) {
    until("the follow to end", Duration::from_secs(10), || {
        bot.butler.follow_ended()
    });
}

#[test]
fn a_butler_fakes_quests_and_its_level_and_unlock_all_wins() {
    let _follows = FOLLOWS.lock();
    let chief_game = FakeGame::new("Chief", "doomvaultb", "9", "r1");
    let page_game = FakeGame::new("Page", "battleon", "4", "Enter");
    *page_game.other.lock() = Some(("Chief".into(), chief_game.clone()));
    page_game.set(
        "world.questTree",
        &quest_tree(&[(3004, 10, 2), (3008, 11, 5), (4242, 12, 1)]),
    );
    page_game.set("world.myAvatar.dataLeaf.intLevel", "10");
    page_game.set("litePreference.data.bDisMonAnim", "true");
    let chief = client(&chief_game);
    let page = client(&page_game);
    until("both files", Duration::from_secs(5), || {
        chief.butler.username() == "Chief"
            && page.butler.roster().iter().any(|d| d.username == "Chief")
    });
    let defaults = page.butler.settings();
    page.butler
        .set_settings(ButlerSettings {
            quest_unlock_all: false,
            quest_bypass: true,
            quest_bypass_custom: "4242".into(),
            fake_level: true,
            anti_lag_following: true,
            ..defaults.clone()
        })
        .unwrap();

    // The bypasses: the custom ID at the start, the master's map before going there.
    butler::start_follow(&page, "Chief").unwrap();
    until("the goto", Duration::from_secs(10), || {
        page_game.spot().0 == "doomvaultb"
    });
    until("the fakes", Duration::from_secs(5), || {
        page_game.update_quest_packets().len() >= 3
    });
    let packets = page_game.client_packets();
    assert!(packets.contains(&quest_bypass::LEVEL_PACKET.to_owned()));
    assert!(packets.contains(&quest_bypass::ACHIEVEMENT_PACKET.to_owned()));
    assert_eq!(
        page_game.update_quest_packets(),
        [
            update_quest(1, 12),
            update_quest(2, 10),
            update_quest(5, 11)
        ],
        "the custom ID, then doomvaultb's, once: arriving doesn't fake them again"
    );
    // AntiLag while following by hand: on, then back as it was.
    assert!(page.opts().lag_killer);
    assert_eq!(page_game.get("litePreference.data.bFreezeMons"), "true");
    butler::stop_follow(&page);
    until_follow_ended(&page);
    assert!(!page.opts().lag_killer);
    assert_eq!(page_game.get("litePreference.data.bFreezeMons"), "false");
    assert_eq!(
        page_game.get("litePreference.data.bDisMonAnim"),
        "true",
        "the user's own setting stays on"
    );

    // Unlock All wins: every slot maxed, no bypass faked.
    page_game.set("world.myAvatar.objData", r#"{"strQuests":"012"}"#);
    page.butler
        .set_settings(ButlerSettings {
            quest_unlock_all: true,
            ..page.butler.settings()
        })
        .unwrap();
    let faked = page_game.update_quest_packets().len();
    butler::start_follow(&page, "Chief").unwrap();
    until("the unlock", Duration::from_secs(5), || {
        !page_game
            .sets_of("world.myAvatar.objData.strQuests")
            .is_empty()
    });
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(
        page_game.sets_of("world.myAvatar.objData.strQuests")[0],
        r#""zzz""#
    );
    assert_eq!(page_game.update_quest_packets().len(), faked);
    butler::stop_follow(&page);
    until_follow_ended(&page);
    page.butler.set_settings(defaults).unwrap();
}

#[test]
fn a_script_starting_while_following_asks_whether_to_keep_following() {
    let _follows = FOLLOWS.lock();
    let lead_game = FakeGame::new("Lead", "battleon", "6", "Enter");
    let tail_game = FakeGame::new("Tail", "battleon", "6", "Enter");
    *tail_game.other.lock() = Some(("Lead".into(), lead_game.clone()));
    let lead = client(&lead_game);
    let tail = client(&tail_game);
    until("both files", Duration::from_secs(5), || {
        lead.butler.username() == "Lead"
            && tail.butler.roster().iter().any(|d| d.username == "Lead")
    });
    let dir = data_dir().join("suspend-script");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Busy.ts");
    std::fs::write(
        &path,
        "import { bot } from \"turu\";\nexport default function main(): void {\n    while (true) bot.sleep(50);\n}\n",
    )
    .unwrap();
    tail.scripts.load(&path);

    butler::start_follow(&tail, "Lead").unwrap();
    assert!(tail.butler.suspend_notice().is_none());
    script::start(&tail).unwrap();
    until("the question", Duration::from_secs(5), || {
        tail.butler.suspend_notice().is_some()
    });
    let notice = tail.butler.suspend_notice().unwrap();
    assert_eq!(notice.master, "Lead");
    assert!(notice.message().contains("following Lead"));

    // Yes: stays suspended, follows again once the script stops.
    butler::answer_suspend(&tail, notice.id, true);
    assert!(tail.butler.suspend_notice().is_none());
    assert_eq!(
        tail.butler.following(),
        Some(("Lead".into(), FollowSource::Manual))
    );
    script::stop(&tail);
    until("the script to stop", Duration::from_secs(5), || {
        !tail.scripts.is_running()
    });
    std::thread::sleep(Duration::from_millis(1500));
    assert!(tail.butler.following().is_some(), "following again");

    // No: stops following.
    script::start(&tail).unwrap();
    until("the second question", Duration::from_secs(5), || {
        tail.butler.suspend_notice().is_some()
    });
    let second = tail.butler.suspend_notice().unwrap();
    assert_ne!(second.id, notice.id);
    butler::answer_suspend(&tail, notice.id, false);
    assert!(
        tail.butler.following().is_some(),
        "an old question's answer does nothing"
    );
    butler::answer_suspend(&tail, second.id, false);
    assert!(tail.butler.following().is_none());
    assert!(tail.butler.suspend_notice().is_none());
    script::stop(&tail);
    until_follow_ended(&tail);
}

#[test]
fn closing_marks_the_account_offline_at_once() {
    let game = FakeGame::new("Leaving", "battleon", "8", "Enter");
    let bot = client(&game);
    let dir = SyncDir::new(data_dir().join("butler"));
    until("a broadcast", Duration::from_secs(5), || {
        dir.read_user("Leaving").is_some_and(|d| d.logged_in)
    });
    butler::close(&bot);
    assert!(!dir.read_user("Leaving").unwrap().logged_in);
    assert!(dir.online(None).iter().all(|d| d.username != "Leaving"));
    // No broadcast after it.
    std::thread::sleep(Duration::from_millis(1200));
    assert!(!dir.read_user("Leaving").unwrap().logged_in);
}
