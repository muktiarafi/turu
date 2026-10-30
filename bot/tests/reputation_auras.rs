//! `bot.reputation`, `bot.self` and `bot.target` against a fake skua.swf.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// Answers the faction list and skua.swf's aura callbacks; records their arguments.
#[derive(Default)]
struct FakeGame {
    calls: Mutex<Vec<(String, Vec<FlashArg>)>>,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        self.calls.lock().push((name.to_owned(), args.clone()));
        let first = match args.first() {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        Ok(match (name, first.as_str()) {
            ("getGameObject", "world.myAvatar.factions") => r#"[
                {"FactionID": "3", "sName": "Good", "iRank": "4", "iRep": 3600, "iSpillRep": 600, "iRepToRank": 2700},
                {"FactionID": 7, "sName": "Alchemy", "iRank": 10, "iRep": 302500, "iSpillRep": 0, "iRepToRank": 0}
            ]"#
            .into(),
            ("getGameObjectS", "loginInfo.strUsername") => "\"Hero\"".into(),
            ("getTargetMonster", _) => r#"{"MonID": 1, "MonMapID": 9, "strMonName": "Boss", "intHP": 10, "intState": 2}"#.into(),
            ("GetPlayerAura", "hero") => r#"[{"nam": "Focus", "val": "3", "dur": 10, "ts": 1700000000000}]"#.into(),
            ("GetMonsterAuraByID", "9") => r#"[{"nam": "Counter Attack", "passive": true}]"#.into(),
            ("GetAuraSnapshots", "Self") => r#"[
                {"nam": "Focus", "n": 3, "dur": 10, "remaining": 4.5},
                {"nam": "Rage", "n": 5, "dur": 8, "remaining": 0},
                {"nam": "Elixir", "n": 1, "dur": 0, "remaining": 0, "persist": true}
            ]"#
            .into(),
            ("GetAuraSnapshots", "Target") => "not json".into(),
            ("GetAurasValue", "Self") => "\"3\"".into(),
            ("HasAnyActiveAura", "Target") => "true".into(),
            _ => "null".into(),
        })
    }
}

fn call(bot: &Arc<Bot>, method: &str, args: Value) -> Value {
    let args = args.as_array().cloned().unwrap_or_default();
    turu_bot::api::dispatch(bot, method, &args).unwrap()
}

fn bot() -> (Arc<Bot>, Arc<FakeGame>) {
    let game = Arc::new(FakeGame::default());
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    (bot, game)
}

#[test]
fn reputation_reads_the_faction_list() {
    let (bot, _) = bot();
    let list = call(&bot, "reputation.factionList", json!([]));
    assert_eq!(
        list[0],
        json!({"id": 3, "name": "Good", "rank": 4, "totalRep": 3600, "rep": 600, "requiredRep": 2700, "remainingRep": 2100})
    );
    assert_eq!(
        call(&bot, "reputation.getRank", json!(["Alchemy"])),
        json!(10)
    );
    assert_eq!(call(&bot, "reputation.getRank", json!([3])), json!(4));
    // Names compare exactly, as VibeSkua does.
    assert_eq!(call(&bot, "reputation.getRank", json!(["good"])), json!(0));
    assert_eq!(
        call(&bot, "reputation.hasRank", json!(["Good", 4])),
        json!(true)
    );
    assert_eq!(
        call(&bot, "reputation.hasRank", json!(["Good", 5])),
        json!(false)
    );
    assert_eq!(
        call(&bot, "reputation.hasRank", json!(["Chaos"])),
        json!(false)
    );
}

#[test]
fn self_auras_use_your_lowercase_name() {
    let (bot, game) = bot();
    let auras = call(&bot, "self.auras", json!([]));
    assert_eq!(auras[0]["name"], "Focus");
    assert_eq!(auras[0]["value"], 3.0);
    assert_eq!(auras[0]["timeStamp"], 1_700_000_000_000_i64);
    assert_eq!(
        call(&bot, "self.hasActiveAura", json!(["focus"])),
        json!(true)
    );
    assert_eq!(
        call(&bot, "self.hasActiveAura", json!(["Rage"])),
        json!(false)
    );
    assert_eq!(call(&bot, "self.getAuraStacks", json!(["FOCUS"])), json!(3));
    assert_eq!(
        call(&bot, "self.getAuraSnapshot", json!(["Focus"]))["remainingTime"],
        4.5
    );
    // A timed aura with no time left is gone; one without a duration stays.
    assert_eq!(call(&bot, "self.getAuraStacks", json!(["Rage"])), json!(0));
    assert_eq!(
        call(&bot, "self.getAuraSnapshot", json!(["Rage"])),
        Value::Null
    );
    let names: Vec<Value> = call(&bot, "self.snapshots", json!([]))
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].clone())
        .collect();
    assert_eq!(names, [json!("Focus"), json!("Elixir")]);
    assert_eq!(
        call(&bot, "self.getAuraValue", json!(["Focus"])),
        json!(3.0)
    );
    assert!(
        game.calls
            .lock()
            .iter()
            .any(|(n, a)| n == "GetPlayerAura" && a == &vec![FlashArg::String("hero".into())])
    );
}

#[test]
fn target_auras_use_the_targets_map_id() {
    let (bot, game) = bot();
    let auras = call(&bot, "target.auras", json!([]));
    assert_eq!(auras[0]["name"], "Counter Attack");
    // A missing value is 1, as in VibeSkua.
    assert_eq!(auras[0]["value"], 1.0);
    assert_eq!(auras[0]["passive"], true);
    // Bad JSON is an empty list.
    assert_eq!(call(&bot, "target.snapshots", json!([])), json!([]));
    assert_eq!(
        call(&bot, "target.getAuraStacks", json!(["Counter Attack"])),
        json!(0)
    );
    assert_eq!(
        call(
            &bot,
            "target.hasAnyActiveAura",
            json!([["Counter Attack", "Rage"]])
        ),
        json!(true)
    );
    let any = game
        .calls
        .lock()
        .iter()
        .find(|(n, _)| n == "HasAnyActiveAura")
        .map(|(_, a)| a.clone())
        .unwrap();
    assert_eq!(any[1], FlashArg::String("Counter Attack,Rage".into()));
    assert_eq!(
        call(&bot, "target.getMonsterAura", json!([9]))[0]["name"],
        "Counter Attack"
    );
    assert!(turu_bot::api::dispatch(&bot, "self.getMonsterAura", &[json!(9)]).is_err());
}
