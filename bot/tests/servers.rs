//! `bot.servers` and auto-relogin against a fake skua.swf whose login screen and servers work.

use parking_lot::Mutex;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use turu_bot::api::servers::{self, Preferences};
use turu_bot::events::BotEvent;
use turu_bot::models::Server;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

fn server(name: &str, ip: &str) -> Server {
    Server {
        name: name.into(),
        ip: ip.into(),
        port: 5588,
        online: true,
        player_count: 10,
        max_players: 1000,
        ..Default::default()
    }
}

fn names(servers: &[Server]) -> Vec<&str> {
    servers.iter().map(|s| s.name.as_str()).collect()
}

const PREFS: Preferences = Preferences {
    any: false,
    relogin_server: "Twilly",
    last_ip: "1.1.1.1",
};

#[test]
fn candidates_skip_unusable_servers_once_per_name() {
    let mut offline = server("Artix", "1.1.1.1");
    offline.online = false;
    let mut full = server("Galanoth", "2.2.2.2");
    full.player_count = 1000;
    let mut member = server("Gravelyn", "3.3.3.3");
    member.upgrade = true;
    let cached = [
        offline,
        full,
        member.clone(),
        server("Class Test Realm", "4.4.4.4"),
        server("Twilly", "5.5.5.5"),
    ];
    let listed = [server("twilly", "5.5.5.5"), server("Safiria", "6.6.6.6")];

    let free = servers::relogin_candidates(&cached, &listed, true);
    assert_eq!(names(&free), ["Twilly", "Safiria"]);
    let members = servers::relogin_candidates(&cached, &listed, false);
    assert_eq!(names(&members), ["Gravelyn", "Twilly", "Safiria"]);
    assert_eq!(
        servers::blacklist_reason(&member, true),
        Some("member-only")
    );
}

#[test]
fn selection_prefers_the_requested_server_then_relogin_server() {
    let candidates = [
        server("Artix", "1.1.1.1"),
        server("Twilly", "2.2.2.2"),
        server("Safiria", "3.3.3.3"),
    ];
    let pick = |preferred| {
        servers::select_server(&candidates, preferred, None, &PREFS)
            .unwrap()
            .name
    };
    assert_eq!(pick(Some("safiria")), "Safiria");
    assert_eq!(pick(None), "Twilly");
    assert_eq!(pick(Some("Nowhere")), "Twilly");

    let any = Preferences { any: true, ..PREFS };
    let other = servers::select_server(&candidates, None, None, &any).unwrap();
    assert_eq!(other.name, "Twilly", "AutoReloginAny skips the last IP");
    assert_eq!(servers::select_server(&[], None, None, &PREFS), None);
}

#[test]
fn selection_tries_each_server_then_starts_over() {
    let candidates = [server("Artix", "1.1.1.1"), server("Twilly", "2.2.2.2")];
    let mut attempted = HashSet::new();
    let mut picks = Vec::new();
    for _ in 0..3 {
        let s = servers::select_server(&candidates, Some("Artix"), Some(&mut attempted), &PREFS)
            .unwrap();
        attempted.insert(s.name.to_lowercase());
        picks.push(s.name);
    }
    assert_eq!(picks, ["Artix", "Twilly", "Artix"]);
}

#[test]
fn server_reads_game_and_script_names() {
    let game: Server = serde_json::from_value(json!({
        "sName": "Twilly", "sIP": "2.2.2.2", "iPort": "5588", "bOnline": "1",
        "iCount": 12, "iMax": 1000, "bUpg": 0, "sLang": "en", "iChat": 2, "iLevel": 0
    }))
    .unwrap();
    assert!(game.online && !game.upgrade);
    assert_eq!((game.port, game.player_count), (5588, 12));
    let script: Server = serde_json::from_value(serde_json::to_value(&game).unwrap()).unwrap();
    assert_eq!(script, game);
    let back: serde_json::Value = serde_json::from_str(&game.to_game_json()).unwrap();
    assert_eq!(back["sIP"], "2.2.2.2");
    assert_eq!(back["bOnline"], 1);
}

/// The game, logged in on Artix. `login` shows the server list; `clickServer`,
/// `connectToServer` and `connectTo` log in; logging out and disconnecting leave the world.
#[derive(Default)]
struct FakeGame {
    values: Mutex<HashMap<String, String>>,
    calls: Mutex<Vec<(String, Vec<FlashArg>)>>,
    logged_in: AtomicBool,
}

const LIST: &str = r#"[{"sName":"Artix","sIP":"1.1.1.1","iPort":5588,"bOnline":1,"iCount":5,"iMax":1000},
    {"sName":"Twilly","sIP":"2.2.2.2","iPort":5588,"bOnline":1,"iCount":5,"iMax":1000}]"#;

impl FakeGame {
    fn new() -> Arc<Self> {
        let game = Self::default();
        for (path, value) in [
            ("loginInfo.strUsername", "\"Hero\""),
            ("loginInfo.strPassword", "\"secret\""),
            ("serialCmd.servers", LIST),
            ("objLogin.iUpgDays", "-1"),
        ] {
            game.set(path, value);
        }
        game.enter_world();
        Arc::new(game)
    }

    fn set(&self, path: &str, raw: &str) {
        self.values.lock().insert(path.to_owned(), raw.to_owned());
    }

    fn enter_world(&self) {
        self.logged_in.store(true, Ordering::SeqCst);
        self.set("world", "{}");
        self.set("world.myAvatar", "{}");
        self.set("world.myAvatar.dataLeaf.intState", "1");
        self.set("mcLogin.visible", "false");
    }

    fn leave_world(&self) {
        self.logged_in.store(false, Ordering::SeqCst);
        let mut values = self.values.lock();
        values.remove("world");
        values.remove("world.myAvatar");
    }

    fn calls_to(&self, name: &str) -> Vec<Vec<FlashArg>> {
        self.calls
            .lock()
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, a)| a.clone())
            .collect()
    }

    fn game_functions(&self) -> Vec<String> {
        self.calls_to("callGameFunction")
            .into_iter()
            .map(|a| arg(&a, 0))
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
        let get = |path: &str| {
            self.values
                .lock()
                .get(path)
                .cloned()
                .unwrap_or_else(|| "null".into())
        };
        Ok(match name {
            "isLoggedIn" => self.logged_in.load(Ordering::SeqCst).to_string(),
            "isNull" => (!self.values.lock().contains_key(&a0)).to_string(),
            "getGameObject" | "getGameObjectS" => get(&a0),
            "getActiveModalMessage" => "null".into(),
            "isKicked" => "false".into(),
            "resetConnectionState" => {
                self.leave_world();
                "null".into()
            }
            "clickServer" | "connectToServer" => {
                self.enter_world();
                "true".into()
            }
            "callGameFunction" => {
                match a0.as_str() {
                    "login" => {
                        self.set("mcLogin.visible", "true");
                        self.set("mcLogin.sl.iList", "{}");
                        self.set("mcLogin.sl.iList.numChildren", "2");
                    }
                    "logout" | "sfc.disconnect" => self.leave_world(),
                    "connectTo" => self.enter_world(),
                    _ => {}
                }
                "null".into()
            }
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
    bot.servers
        .set_fetcher(|| Ok(serde_json::from_str(LIST).unwrap()));
    bot
}

fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    f()
}

/// The game's client sends `%xt%zm%cmd%1%logout%` when the server logs you out.
fn server_logout(bot: &Arc<Bot>, game: &FakeGame) {
    game.leave_world();
    bot.on_flash_call("packet", vec![json!("%xt%zm%cmd%1%logout%")]);
}

#[test]
fn auto_relogin_logs_back_in_after_a_server_logout() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    bot.options.write().auto_relogin = true;
    let triggered = Arc::new(Mutex::new(Vec::new()));
    let seen = triggered.clone();
    bot.events.subscribe(move |e| {
        if let BotEvent::ReloginTriggered { was_kicked } = e {
            seen.lock().push(*was_kicked);
        }
    });

    server_logout(&bot, &game);
    assert!(bot.servers.is_relogging());
    assert!(
        wait_until(Duration::from_secs(10), || !bot.servers.is_relogging()),
        "the relogin didn't finish"
    );

    assert!(game.logged_in.load(Ordering::SeqCst));
    let login = game
        .calls_to("callGameFunction")
        .into_iter()
        .find(|a| arg(a, 0) == "login")
        .expect("logged in");
    assert_eq!(
        (arg(&login, 1), arg(&login, 2)),
        ("Hero".into(), "secret".into())
    );
    // No server asked for: the Relogin Server option (Twilly).
    assert_eq!(arg(&game.calls_to("clickServer")[0], 0), "Twilly");
    assert_eq!(*triggered.lock(), [false]);
    assert_eq!(bot.state.lock().stats.relogins, 1);
}

#[test]
fn intentional_logout_doesnt_relogin() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    bot.options.write().auto_relogin = true;

    servers::logout(&bot).unwrap();
    bot.on_flash_call("packet", vec![json!("%xt%zm%cmd%1%logout%")]);
    assert!(!bot.servers.is_relogging());
    std::thread::sleep(Duration::from_millis(1200));
    assert!(!game.logged_in.load(Ordering::SeqCst));
    assert!(!game.game_functions().contains(&"login".to_owned()));
    assert!(bot.state.lock().intentional_logout);
}

/// Clicking Logout in the game: the click arrives while the world is still up, the connection
/// closes a moment later.
#[test]
fn logout_clicked_in_game_isnt_relogged_by_the_watchdog() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    {
        let mut o = bot.options.write();
        o.auto_relogin = true;
        o.relogin_watchdog = true;
    }
    bot.on_flash_call("loaded", vec![]);
    assert!(wait_until(Duration::from_secs(2), || bot
        .state
        .lock()
        .in_game));

    bot.on_flash_call("packet", vec![json!("%xt%zm%cmd%1%logout%")]);
    bot.on_flash_call("userIntentionalAction", vec![json!("logout")]);
    // The watchdog ticks while the world is still up.
    std::thread::sleep(Duration::from_millis(300));
    assert!(bot.state.lock().intentional_logout);

    game.leave_world();
    bot.on_flash_call("connectionLost", vec![]);
    std::thread::sleep(Duration::from_millis(300));
    assert!(!bot.servers.is_relogging());
    assert!(!game.game_functions().contains(&"login".to_owned()));

    // Logging in again re-arms it.
    game.enter_world();
    assert!(wait_until(Duration::from_secs(2), || !bot
        .state
        .lock()
        .intentional_logout));
}

#[test]
fn lost_connection_needs_the_watchdog() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    {
        let mut o = bot.options.write();
        o.auto_relogin = true;
        o.relogin_watchdog = false;
    }
    game.leave_world();
    bot.on_flash_call("connectionLost", vec![]);
    assert!(!bot.servers.is_relogging());

    bot.options.write().relogin_watchdog = true;
    bot.on_flash_call("connectionLost", vec![]);
    assert!(bot.servers.is_relogging());
    // Clicking Logout in the game cancels it.
    bot.on_flash_call("userIntentionalAction", vec![json!("logout")]);
    assert!(!bot.servers.is_relogging());
}

#[test]
fn scripted_relogin_picks_the_named_server_and_restores_auto_relogin() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    bot.options.write().auto_relogin = true;

    let relogged = turu_bot::api::dispatch(&bot, "servers.relogin", &[json!("twil")]).unwrap();
    assert_eq!(relogged, json!(true));
    let connect = game.calls_to("connectToServer");
    let sent: serde_json::Value = serde_json::from_str(&arg(&connect[0], 0)).unwrap();
    assert_eq!(sent["sName"], "Twilly");
    assert!(bot.opts().auto_relogin);
    assert_eq!(bot.state.lock().stats.relogins, 1);
    assert_eq!(
        turu_bot::api::dispatch(&bot, "servers.cachedServers", &[]).unwrap()[1]["name"],
        "Twilly"
    );
}

#[test]
fn startup_logs_in_once_the_game_has_loaded() {
    let game = FakeGame::new();
    game.leave_world();
    let bot = new_bot(&game);
    turu_bot::startup::configure(
        &bot,
        turu_bot::startup::Startup {
            username: Some("Alt".into()),
            password: Some("pw".into()),
            server: Some("Artix".into()),
            ..Default::default()
        },
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        !game.game_functions().contains(&"login".to_owned()),
        "waits for the game"
    );

    bot.on_flash_call("loaded", vec![]);
    assert!(wait_until(Duration::from_secs(10), || game
        .logged_in
        .load(Ordering::SeqCst)));
    let login = game
        .calls_to("callGameFunction")
        .into_iter()
        .find(|a| arg(a, 0) == "login")
        .expect("logged in");
    assert_eq!(
        (arg(&login, 1), arg(&login, 2)),
        ("Alt".into(), "pw".into())
    );
    assert_eq!(arg(&game.calls_to("clickServer")[0], 0), "Artix");
    // Relogins use the same login.
    assert_eq!(
        turu_bot::api::servers::cached_username(&bot)
            .unwrap()
            .as_deref(),
        Some("Alt")
    );
}

#[test]
fn a_stuck_message_on_the_login_screen_logs_out_only_once() {
    let game = FakeGame::new();
    game.leave_world();
    // A login that never finished leaves its message up.
    game.set("mcConnDetail.stage", "{}");
    game.set(
        "mcConnDetail.txtDetail.text",
        "\"Authenticating Account Info...\"",
    );
    let bot = new_bot(&game);
    bot.options.write().load_timeout = 200;
    bot.state.lock().game_loaded = true;
    // `game.logout()` takes no arguments: `callGameFunction0`.
    let logouts = || {
        game.calls_to("callGameFunction0")
            .iter()
            .filter(|a| arg(a, 0) == "logout")
            .count()
    };
    assert!(wait_until(Duration::from_secs(5), || logouts() == 1));
    // Several more LoadTimeouts: the message stays, and so does the login screen.
    std::thread::sleep(Duration::from_millis(1000));
    assert_eq!(logouts(), 1);
}

#[test]
fn the_timer_checks_less_often_in_game_but_still_catches_logins_at_once() {
    let game = FakeGame::new();
    let bot = new_bot(&game);
    bot.state.lock().game_loaded = true;
    let catches = || game.calls_to("catchPackets").len();
    assert!(wait_until(Duration::from_secs(2), || catches() == 1));

    // In game: "still in game?" every 100 ms, not every 20 ms tick.
    let world_checks = || {
        game.calls_to("isNull")
            .iter()
            .filter(|a| arg(a, 0) == "world")
            .count()
    };
    let before = world_checks();
    std::thread::sleep(Duration::from_millis(1000));
    let checks = world_checks() - before;
    assert!((5..=15).contains(&checks), "{checks} checks in a second");

    // A logout is still noticed within a check or two.
    game.leave_world();
    assert!(wait_until(Duration::from_millis(500), || !bot
        .state
        .lock()
        .in_game));

    // Out of game it checks every tick: the next login gets `catchPackets` right away.
    game.enter_world();
    let started = Instant::now();
    assert!(wait_until(Duration::from_secs(1), || catches() == 2));
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "{:?}",
        started.elapsed()
    );
}
