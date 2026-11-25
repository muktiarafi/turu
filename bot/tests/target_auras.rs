//! The Target Auras HUD through the packets as the player reports them, and its reads of a fake
//! game for what the packets don't say (names made up; the packets' shapes are an Ultra fight's,
//! captured 2026-11-25).

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::player_packets::Direction;
use turu_bot::target_auras::{CasterKind, PinState, TargetView};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A player that reports its sockets, and a game that names a late arrival, the monster and the
/// auras it had before the HUD saw it.
#[derive(Default)]
struct Game {
    calls: Mutex<Vec<String>>,
}

impl FlashHost for Game {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        if let Some(answer) =
            turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default())
        {
            return Ok(answer);
        }
        let first = match args.first() {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        self.calls.lock().push(format!("{name} {first}"));
        let now = turu_bot::gear::now_ms();
        Ok(match (name, first.as_str()) {
            ("getGameObject", "world.areaUsers") => r#"["Questwalker", "Nightjar"]"#.into(),
            ("getGameObject", "world.uoTree.nightjar.entID") => "99999".into(),
            ("getGameObject", "world.uoTree.nightjar.strUsername") => "\"Nightjar\"".into(),
            ("getMonsters", _) => r#"[{"MonID": 7, "MonMapID": 1, "strMonName": "Ultra Boss",
                "intHP": 15000000, "intHPMax": 22222222, "strFrame": "Boss", "intState": 2}]"#
                .into(),
            // Two by two pixels, opaque red (ARGB); a class not loaded yet draws nothing.
            ("GetAuraIcon", "LR2" | "isp2") => "\"2://8AAP//AAD//wAA//8AAA==\"".into(),
            ("GetAuraIcon", _) => "\"\"".into(),
            ("GetMonsterAuraByID", "1") => format!(
                r#"[{{"nam": "Decay", "dur": 30, "ts": {}}},
                    {{"nam": "Run Out", "dur": 5, "ts": {}}},
                    {{"nam": "Aria", "dur": 7200}}]"#,
                now - 10_000,
                now - 60_000
            ),
            _ => "null".into(),
        })
    }

    fn player_packets(&self) -> bool {
        true
    }
}

fn bot() -> (Arc<Bot>, Arc<Game>) {
    crate::common::data_dir();
    let game = Arc::new(Game::default());
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    bot.state.lock().username = Some("Questwalker".into());
    turu_bot::target_auras::set_shown(&bot, true);
    (bot, game)
}

fn receive(bot: &Arc<Bot>, json: &str) {
    bot.on_socket(7, Direction::Received, json.as_bytes());
    bot.on_socket(7, Direction::Received, b"\0");
}

fn send(bot: &Arc<Bot>, packet: &str) {
    bot.on_socket(7, Direction::Sent, packet.as_bytes());
    bot.on_socket(7, Direction::Sent, b"\0");
}

/// The HUD's view once `ready` holds (packets are handled off this thread), else the last one.
fn view_when(bot: &Bot, ready: impl Fn(&Option<TargetView>) -> bool) -> Option<TargetView> {
    let start = Instant::now();
    loop {
        let view = bot
            .target_auras
            .snapshot(Some("Questwalker"), Instant::now());
        if ready(&view) || start.elapsed() > Duration::from_secs(2) {
            return view;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn packets_name_the_casters_of_the_targets_auras() {
    let (bot, _) = bot();
    receive(
        &bot,
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"moveToArea","strMapName":"ultraboss","strMapFileName":"ultraboss.swf",
            "uoBranch":[{"uoName":"questwalker","strUsername":"Questwalker","entID":70171},
                        {"uoName":"moonfarer","strUsername":"Moonfarer","entID":70177}]}}}"#,
    );
    send(&bot, "%xt%zm%gar%1%177%aa>m:1%wvz%");
    receive(
        &bot,
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"ct","m":{"1":{"intHP":8596302}},"p":{"moonfarer":{}},
            "a":[{"cmd":"aura-","tInf":"m:1","aura":{"nam":"Focus"}},
                 {"cmd":"aura+","tInf":"m:1","cInf":"p:70177","auras":[{"nam":"Focus","dur":6,"t":"s","isNew":true,"icon":"iwd1,ied1"}]},
                 {"cmd":"aura+","tInf":"m:1","cInf":"p:70177","auras":[{"nam":"Reckless","dur":10,"t":"s","icon":"iwd1,ied1"}]},
                 {"cmd":"aura+","tInf":"m:1","cInf":"p:70171","auras":[{"nam":"Seared","dur":12,"t":"s","stk":10,"icon":"i,i,i,i,i,LR2"}]}],
            "anims":[{"strFrame":"r2","cInf":"p:70177","tInf":"m:1","animStr":"Point","strl":"sp_ei3","fx":"w"}]}}}"#,
    );
    let view = view_when(&bot, |v| v.as_ref().is_some_and(|v| !v.casters.is_empty())).unwrap();
    assert_eq!(view.pins[0].state, PinState::Up);
    assert_eq!(view.pins[0].caster.as_deref(), Some("Moonfarer"));
    assert_eq!(view.hp, Some(8596302));
    assert_eq!(view.casters[0].name, "Questwalker");
    assert_eq!(view.casters[0].kind, CasterKind::Me);
    assert_eq!(view.casters[0].auras[0].stacks, 10);

    // A cleanse, sent alone.
    receive(
        &bot,
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"aura*","tInf":"m:1"}}}"#,
    );
    let view = view_when(&bot, |v| v.as_ref().is_some_and(|v| v.casters.is_empty())).unwrap();
    assert!(matches!(
        view.pins[0].state,
        PinState::Down { cleansed: true, .. }
    ));

    // The boss dies: the HUD goes.
    receive(
        &bot,
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"ct","m":{"1":{"intHP":0,"intState":0}}}}}"#,
    );
    assert_eq!(view_when(&bot, Option::is_none), None);
}

#[test]
fn a_read_names_what_the_packets_dont() {
    let (bot, game) = bot();
    send(&bot, "%xt%zm%gar%1%177%aa>m:1%wvz%");
    receive(
        &bot,
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"ct","a":[
            {"cmd":"aura+","tInf":"m:1","cInf":"p:99999","auras":[{"nam":"Sinkhole","dur":8,"t":"s"}]}]}}}"#,
    );
    view_when(&bot, |v| v.as_ref().is_some_and(|v| !v.casters.is_empty()));
    assert!(bot.target_auras.refresh_due());
    bot.target_auras.refresh(&bot).unwrap();
    assert!(!bot.target_auras.refresh_due());

    let view = bot
        .target_auras
        .snapshot(Some("Questwalker"), Instant::now())
        .unwrap();
    assert_eq!(view.monster, "Ultra Boss");
    assert_eq!(view.max_hp, 22222222);
    let names: Vec<(&str, CasterKind)> = view
        .casters
        .iter()
        .map(|c| (c.name.as_str(), c.kind))
        .collect();
    assert_eq!(
        names,
        [
            ("Nightjar", CasterKind::Player),
            ("Unknown", CasterKind::Unknown)
        ]
    );
    // The earlier auras: Decay with the 20 s it has left, Aria (no time stamp) without a
    // countdown, and the one that ran out left out.
    let earlier: Vec<(&str, Option<i64>)> = view.casters[1]
        .auras
        .iter()
        .map(|a| (a.name.as_str(), a.left.map(|l| l.round() as i64)))
        .collect();
    assert_eq!(earlier, [("Decay", Some(20)), ("Aria", None)]);

    // A caster no read can name is asked for once, not every second.
    receive(
        &bot,
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"ct","a":[
            {"cmd":"aura+","tInf":"m:1","cInf":"p:55555","auras":[{"nam":"Bruised","dur":8,"t":"s"}]}]}}}"#,
    );
    view_when(&bot, |v| v.as_ref().is_some_and(|v| v.casters.len() == 3));
    bot.target_auras.refresh(&bot).unwrap();
    receive(
        &bot,
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"ct","a":[
            {"cmd":"aura+","tInf":"m:1","cInf":"p:55555","auras":[{"nam":"Bruised","dur":8,"t":"s","stk":2}]}]}}}"#,
    );
    view_when(&bot, |v| {
        v.as_ref().is_some_and(|v| {
            v.casters
                .iter()
                .any(|c| c.auras.iter().any(|a| a.stacks == 2))
        })
    });
    assert!(!bot.target_auras.refresh_due());
    let reads = game
        .calls
        .lock()
        .iter()
        .filter(|c| c.starts_with("GetMonsterAuraByID"))
        .count();
    assert_eq!(reads, 1, "the earlier auras are read once");
}

#[test]
fn icons_are_drawn_by_the_game_once_and_stand_in_when_missing() {
    let (bot, game) = bot();
    send(&bot, "%xt%zm%gar%1%177%aa>m:1%wvz%");
    receive(
        &bot,
        r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"ct","a":[
            {"cmd":"aura+","tInf":"m:1","cInf":"p:99999","auras":[{"nam":"Seared","dur":12,"t":"s","stk":10,"icon":"i,i,i,i,i,LR2"}]},
            {"cmd":"aura+","tInf":"m:1","cInf":"p:99999","auras":[{"nam":"Commandment","dur":10,"t":"s","icon":"apal1"}]}]}}}"#,
    );
    let view = view_when(&bot, |v| v.as_ref().is_some_and(|v| !v.casters.is_empty())).unwrap();
    let links: Vec<Option<&str>> = view.casters[0]
        .auras
        .iter()
        .map(|a| a.icon.as_deref())
        .collect();
    assert_eq!(links, [Some("LR2"), Some("apal1")]);

    // The panel asks; the timer has the game draw them (never the UI thread).
    assert_eq!(bot.target_auras.icon("LR2"), None);
    assert_eq!(bot.target_auras.icon("apal1"), None);
    bot.target_auras.refresh(&bot).unwrap();
    let drawn = bot.target_auras.icon("LR2").unwrap();
    assert_eq!((drawn.size, &drawn.rgba[..4]), (2, &[255u8, 0, 0, 255][..]));
    // apal1's class isn't loaded: the game's stand-in, once it is drawn too.
    assert_eq!(bot.target_auras.icon("apal1"), None);
    bot.target_auras.refresh(&bot).unwrap();
    assert!(bot.target_auras.icon("apal1").is_some());

    let draws = |name: &str| {
        game.calls
            .lock()
            .iter()
            .filter(|c| *c == &format!("GetAuraIcon {name}"))
            .count()
    };
    assert_eq!(draws("LR2"), 1, "drawn once");
    bot.target_auras.refresh(&bot).unwrap();
    assert_eq!(draws("LR2"), 1);
    assert_eq!(draws("apal1"), 1, "tried again only after a while");
}
