//! `bot.boosts` (VibeSkua `ScriptBoost`) against a fake game.

use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use turu_bot::api::boosts::{self, BoostType};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

#[derive(Default)]
struct FakeGame {
    sent: Mutex<Vec<String>>,
    gold_boost_active: AtomicBool,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let arg = |i: usize| match args.get(i) {
            Some(FlashArg::String(s)) => s.clone(),
            _ => String::new(),
        };
        Ok(match (name, arg(0).as_str()) {
            ("isLoggedIn", _) => "true".into(),
            ("getGameObject", "world.curRoom") => "42".into(),
            ("getGameObject", "world.myAvatar.items") => json!([
                { "ItemID": 1, "sName": "Dragon Scale", "iQty": 5, "sType": "Item" },
                { "ItemID": 5000, "sName": "Gold Boost! (1 hr)", "iQty": 2, "sType": "ServerUse" }
            ])
            .to_string(),
            ("getGameObject", "world.myAvatar.objData.iBoostG") => {
                if self.gold_boost_active.load(Ordering::Relaxed) {
                    "60"
                } else {
                    "0"
                }
                .into()
            }
            ("callGameFunction", "sfc.sendString") => {
                self.sent.lock().push(arg(1));
                "null".into()
            }
            _ => "null".into(),
        })
    }
}

fn bot_with(game: &Arc<FakeGame>) -> Arc<Bot> {
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    bot
}

#[test]
fn finds_the_boost_item_ignoring_case() {
    let game = Arc::new(FakeGame::default());
    let bot = bot_with(&game);
    assert_eq!(
        boosts::get_boost_id(&bot, BoostType::Gold, false).unwrap(),
        5000
    );
    assert_eq!(
        boosts::get_boost_id(&bot, BoostType::Class, false).unwrap(),
        0
    );
}

#[test]
fn the_timer_uses_a_wanted_boost_only_while_it_is_inactive() {
    let game = Arc::new(FakeGame::default());
    let bot = bot_with(&game);
    bot.boosts.set_id(BoostType::Gold, 5000);
    bot.boosts.set_uses(BoostType::Gold, true);
    bot.boosts.start(&bot);
    assert!(bot.boosts.enabled());

    let start = Instant::now();
    while game.sent.lock().is_empty() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(*game.sent.lock(), ["%xt%zm%serverUseItem%42%+%5000%"]);

    // Turning the boost off and on while it's active uses nothing.
    game.gold_boost_active.store(true, Ordering::Relaxed);
    bot.boosts.reset();
    assert!(!bot.boosts.enabled() && !bot.boosts.uses(BoostType::Gold));
    bot.boosts.set_uses(BoostType::Gold, true);
    bot.boosts.start(&bot);
    std::thread::sleep(Duration::from_millis(300));
    bot.boosts.stop();
    assert_eq!(game.sent.lock().len(), 1);
}
