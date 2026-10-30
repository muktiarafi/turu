//! Butler (VibeSkua ButlerV5): the sync file, and a master summoning a butler, both on fake
//! games that share one data folder.

use parking_lot::{Condvar, Mutex};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};
use turu_bot::butler::sync::{SyncData, SyncDir, master_ordering, now};
use turu_bot::butler::{self, FollowSource, room_of};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

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
