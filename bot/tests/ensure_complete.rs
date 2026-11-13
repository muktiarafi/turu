//! `quests.ensureComplete` against a fake game whose server lost the quest's accept: the client
//! shows it in progress, but every turn-in goes unanswered until it is accepted again.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::{Arc, OnceLock, Weak};
use turu_bot::{Bot, FlashArg, FlashHost, HostError, api};

#[derive(Default)]
struct Quest {
    /// What `world.isQuestInProgress` says.
    client_in_progress: bool,
    /// Whether the server has the quest accepted (it answers turn-ins only then).
    server_accepted: bool,
}

#[derive(Default)]
struct FakeGame {
    bot: OnceLock<Weak<Bot>>,
    quest: Mutex<Quest>,
    calls: Mutex<Vec<String>>,
    /// When each `world.tryQuestComplete` was called.
    turn_ins_at: Mutex<Vec<std::time::Instant>>,
}

impl FakeGame {
    fn bot(&self) -> Arc<Bot> {
        self.bot.get().and_then(Weak::upgrade).expect("bot")
    }
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        if let Some(answer) =
            turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default())
        {
            return Ok(answer);
        }
        let arg = |i: usize| match args.get(i) {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        match (name, arg(0).as_str()) {
            ("isLoggedIn", _) => return Ok("true".into()),
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => return Ok("1".into()),
            // No cooldown to wait for.
            ("getGameObject", path) if path.starts_with("world.lock.") => {
                return Ok(r#"{"ts":0,"cd":0}"#.into());
            }
            ("callGameFunction", _) => {}
            _ => return Ok("null".into()),
        }
        let function = arg(0);
        if function.starts_with("world.") && function != "world.isQuestInProgress" {
            self.calls.lock().push(function.clone());
        }
        let answer = match function.as_str() {
            "world.isQuestInProgress" => self.quest.lock().client_in_progress.to_string(),
            "world.tryQuestComplete" => {
                self.turn_ins_at.lock().push(std::time::Instant::now());
                let bot = self.bot();
                bot.on_flash_call(
                    "packet",
                    vec![Value::String(format!(
                        "%xt%zm%tryQuestComplete%5%{}%-1%false%1%wvz%",
                        arg(1)
                    ))],
                );
                let answered = {
                    let mut quest = self.quest.lock();
                    if quest.server_accepted {
                        quest.client_in_progress = false;
                        quest.server_accepted = false;
                    }
                    !quest.client_in_progress
                };
                if answered {
                    let reply = json!({ "params": { "type": "json", "dataObj":
                        { "cmd": "ccqr", "bSuccess": 1, "QuestID": 4777 } } });
                    bot.on_flash_call("pext", vec![Value::String(reply.to_string())]);
                }
                "null".into()
            }
            "world.abandonQuest" => {
                let mut quest = self.quest.lock();
                quest.client_in_progress = false;
                quest.server_accepted = false;
                "null".into()
            }
            "world.acceptQuest" => {
                let mut quest = self.quest.lock();
                quest.client_in_progress = true;
                quest.server_accepted = true;
                "null".into()
            }
            _ => "null".into(),
        };
        Ok(answer)
    }
}

fn setup(server_accepted: bool) -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test in this file sets the same value.
    unsafe {
        std::env::set_var(
            "TURU_DATA_DIR",
            std::env::temp_dir().join("turu-test-ensure-complete"),
        )
    };
    let bot = Bot::new();
    {
        let mut options = bot.options.write();
        options.action_delay = 0;
        options.quest_accept_and_complete_tries = 3;
    }
    let game = Arc::new(FakeGame::default());
    *game.quest.lock() = Quest {
        client_in_progress: true,
        server_accepted,
    };
    game.bot.set(Arc::downgrade(&bot)).ok();
    bot.set_host(Some(game.clone()));
    (bot, game)
}

fn ensure_complete(bot: &Arc<Bot>) -> Value {
    api::dispatch(
        bot,
        "quests.ensureComplete",
        &[json!(4777), json!(-1), json!(false)],
    )
    .expect("quests.ensureComplete")
}

#[test]
fn unanswered_turn_ins_abandon_and_accept_the_quest_again() {
    let (bot, game) = setup(false);
    assert_eq!(ensure_complete(&bot), json!(true));

    let calls = game.calls.lock().clone();
    let turn_ins = calls
        .iter()
        .filter(|c| *c == "world.tryQuestComplete")
        .count();
    assert_eq!(
        turn_ins, 4,
        "3 unanswered, then 1 after the accept: {calls:?}"
    );
    let abandon = calls.iter().position(|c| c == "world.abandonQuest");
    let accept = calls.iter().position(|c| c == "world.acceptQuest");
    assert!(
        abandon.is_some() && abandon < accept,
        "abandon, then accept: {calls:?}"
    );
    let log = bot.logs.debug.text();
    assert!(
        log.contains("Quest 4777: no reply to 3 turn-ins while it shows in progress"),
        "{log}"
    );
}

#[test]
fn an_answered_turn_in_needs_no_recovery() {
    let (bot, game) = setup(true);
    assert_eq!(ensure_complete(&bot), json!(true));
    assert_eq!(*game.calls.lock(), ["world.tryQuestComplete"]);
}

/// Turn-ins of quests one after another (CoreBots' `ensureCompleteQuests`) keep `ActionDelay`
/// apart: the server drops one sent ~500 ms after the last ("Please slow down").
#[test]
fn turn_ins_keep_action_delay_apart() {
    let (bot, game) = setup(true);
    bot.options.write().action_delay = 300;
    for _ in 0..3 {
        *game.quest.lock() = Quest {
            client_in_progress: true,
            server_accepted: true,
        };
        assert_eq!(ensure_complete(&bot), json!(true));
    }
    let times = game.turn_ins_at.lock().clone();
    assert_eq!(times.len(), 3);
    for pair in times.windows(2) {
        let gap = pair[1].duration_since(pair[0]);
        assert!(gap >= std::time::Duration::from_millis(300), "{gap:?}");
    }
}
