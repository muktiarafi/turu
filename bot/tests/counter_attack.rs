//! The counter-attack pause (VibeSkua `Combat.StopAttacking`, `EnableCounterHandler`,
//! `UltraBossHelper`) against a fake skua.swf.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// Answers the target; records every call by name and first argument.
#[derive(Default)]
struct FakeGame {
    calls: Mutex<Vec<String>>,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let first = match args.first() {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        self.calls.lock().push(format!("{name} {first}"));
        Ok(match name {
            "getTargetMonster" => {
                r#"{"MonID": 1, "MonMapID": 9, "strMonName": "Boss", "intHP": 10, "intState": 2}"#
                    .into()
            }
            "attackMonsterID" | "attackMonsterName" => "true".into(),
            _ => "null".into(),
        })
    }
}

fn bot() -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let game = Arc::new(FakeGame::default());
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    (bot, game)
}

fn call(bot: &Arc<Bot>, method: &str, args: Value) -> Value {
    let args = args.as_array().cloned().unwrap_or_default();
    turu_bot::api::dispatch(bot, method, &args).unwrap()
}

fn pext_json(bot: &Arc<Bot>, data: Value) {
    let packet = json!({ "params": { "type": "json", "dataObj": data } });
    bot.on_flash_call("pext", vec![Value::String(packet.to_string())]);
}

fn prepares(bot: &Arc<Bot>) {
    pext_json(
        bot,
        json!({ "cmd": "ct", "anims": [{ "msg": "Ultra Boss prepares a counter attack!" }] }),
    );
}

fn fades(bot: &Arc<Bot>) {
    pext_json(
        bot,
        json!({ "cmd": "ct", "a": [{ "cmd": "aura-", "aura": { "nam": "Counter Attack" } }] }),
    );
}

/// Waits (up to 2 s) until the game saw `call`.
fn wait_for_call(game: &FakeGame, call: &str) {
    let start = Instant::now();
    while !game.calls.lock().iter().any(|c| c == call) {
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "no {call}: {:?}",
            game.calls.lock()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn counter_handler_stops_attacks_and_resumes_on_the_target() {
    let (bot, game) = bot();
    assert_eq!(
        call(&bot, "combat.isCounterAttackEnabled", json!([])),
        false
    );
    call(&bot, "combat.enableCounterAttack", json!([]));
    assert_eq!(call(&bot, "combat.isCounterAttackEnabled", json!([])), true);
    assert_eq!(call(&bot, "combat.enableCounterHandler", json!([])), true);

    prepares(&bot);
    assert_eq!(call(&bot, "combat.stopAttacking", json!([])), true);
    // The work thread saves the target and drops it.
    wait_for_call(&game, "callGameFunction0 world.cancelTarget");
    assert!(
        game.calls
            .lock()
            .iter()
            .any(|c| c == "callGameFunction0 world.cancelAutoAttack")
    );

    // Attacks are refused (and cancel the target) while it's up.
    game.calls.lock().clear();
    assert_eq!(call(&bot, "combat.attack", json!(["Boss"])), false);
    assert_eq!(*game.calls.lock(), ["callGameFunction0 world.cancelTarget"]);

    fades(&bot);
    assert_eq!(call(&bot, "combat.stopAttacking", json!([])), false);
    wait_for_call(&game, "attackMonsterID 9");
    assert_eq!(call(&bot, "combat.attack", json!(["Boss"])), true);
}

#[test]
fn without_the_handler_counter_attacks_change_nothing() {
    let (bot, game) = bot();
    call(&bot, "combat.disableCounterAttack", json!([]));
    prepares(&bot);
    assert_eq!(call(&bot, "combat.stopAttacking", json!([])), false);
    std::thread::sleep(Duration::from_millis(100));
    assert!(game.calls.lock().is_empty(), "{:?}", game.calls.lock());
}

#[test]
fn stop_attacking_is_cleared_by_a_death_and_set_by_hand() {
    let (bot, _) = bot();
    bot.state.lock().username = Some("Hero".into());
    call(&bot, "combat.setAttacksStopped", json!([true]));
    assert_eq!(call(&bot, "combat.stopAttacking", json!([])), true);
    pext_json(
        &bot,
        json!({ "cmd": "ct", "p": { "hero": { "intHP": 0 } } }),
    );
    assert_eq!(call(&bot, "combat.stopAttacking", json!([])), false);

    call(&bot, "combat.setStopAttacking", json!([true]));
    assert_eq!(call(&bot, "combat.stopAttacking", json!([])), true);
    call(&bot, "combat.setAttacksStopped", json!([false]));
    assert_eq!(call(&bot, "combat.stopAttacking", json!([])), false);
    assert_eq!(call(&bot, "combat.isCounterAttackActive", json!([])), false);
}
