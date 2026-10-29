//! The bot's own threads (VibeSkua: the 20 ms "ScriptInterface" timer and the startup
//! handler): answers `skua.swf`'s startup calls, finishes packet work that needs Flash, and
//! keeps the options applied in game.

use crate::Bot;
use crate::api::inventory::{self, Container};
use crate::events::{BotEvent, Work};
use crate::models::ItemBase;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

pub(crate) fn start(bot: &Arc<Bot>) {
    let work_rx = bot.events.work_rx.clone();
    let weak = Arc::downgrade(bot);
    std::thread::Builder::new()
        .name("Turu Startup".into())
        .spawn(move || {
            while let Ok(work) = work_rx.recv() {
                let Some(bot) = weak.upgrade() else { break };
                do_work(&bot, work);
            }
        })
        .expect("couldn't start the bot's startup thread");

    let weak = Arc::downgrade(bot);
    std::thread::Builder::new()
        .name("ScriptInterface".into())
        .spawn(move || timer(weak))
        .expect("couldn't start the bot's timer thread");
}

fn do_work(bot: &Bot, work: Work) {
    let result = match &work {
        Work::LoadClient => bot.flash.call("loadClient", vec![]).map(|_| ()),
        Work::SetBackground => {
            std::thread::sleep(Duration::from_secs(1));
            bot.flash
                .call(
                    "setBackgroundValues",
                    vec!["Generic2.swf".into(), "".into()],
                )
                .map(|_| ())
        }
        Work::ItemAdded {
            item_id,
            quantity_now,
            fallback,
        } => {
            item_added(bot, *item_id, *quantity_now, fallback);
            Ok(())
        }
        Work::BankItemAdded {
            item_id,
            quantity_now,
        } => {
            let item = find(bot, Container::Bank, *item_id).unwrap_or_else(|| ItemBase {
                id: *item_id,
                ..Default::default()
            });
            bot.events.emit(BotEvent::ItemAddedToBank {
                item,
                quantity_now: *quantity_now,
            });
            Ok(())
        }
    };
    if let Err(e) = result {
        bot.logs
            .debug
            .push(format!("Startup call {work:?} failed: {e}"));
    }
}

fn find(bot: &Bot, container: Container, id: i64) -> Option<ItemBase> {
    inventory::items(bot, container)
        .ok()?
        .into_iter()
        .find(|i| i.id == id)
}

/// VibeSkua `addItems`: the item is in the inventory or temp inventory (`ItemDropped` with
/// `addedToInv`; a drop unless temp), else in the bank (`ItemAddedToBank`).
fn item_added(bot: &Bot, item_id: i64, quantity_now: i64, fallback: &ItemBase) {
    let found =
        find(bot, Container::Inventory, item_id).or_else(|| find(bot, Container::TempInv, item_id));
    match found {
        Some(item) => {
            if !item.temp {
                bot.state.lock().stats.drops += 1;
            }
            bot.events.emit(BotEvent::ItemAdded { item, quantity_now });
        }
        None => match find(bot, Container::Bank, item_id) {
            Some(item) => bot.events.emit(BotEvent::ItemAddedToBank {
                quantity_now: item.quantity,
                item,
            }),
            // Couldn't look it up: pass on what the packet said.
            None => bot.events.emit(BotEvent::ItemAdded {
                item: fallback.clone(),
                quantity_now,
            }),
        },
    }
}

/// 20 ms tick (VibeSkua `ScriptInterface.ScriptTimer`): tracks login state, calls
/// `catchPackets` once per login, sends rest packets every 1200 ms and applies the options
/// every 250 ms while playing.
///
/// TODO(turu): VibeSkua's watchdog (auto-relogin on an unexpected disconnect, kick
/// detection every 1 s, stuck-on-loading detection through `mcConnDetail.txtDetail.text`)
/// and script handlers (`Handlers.RegisterHandler`) need the servers/handlers APIs.
fn timer(weak: Weak<Bot>) {
    let mut last_options = Instant::now();
    let mut last_rest = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let Some(bot) = weak.upgrade() else { break };
        if !bot.state.lock().game_loaded || !bot.flash.has_host() {
            continue;
        }
        let in_game = crate::api::player::is_in_game(&bot).unwrap_or(false);
        let catching = {
            let mut state = bot.state.lock();
            let was = state.in_game;
            state.in_game = in_game;
            if !in_game && was {
                // The session ended: its drops are gone.
                state.drops.clear();
            }
            if !in_game {
                state.catching_packets = false;
            }
            state.catching_packets
        };
        if !in_game {
            continue;
        }
        if !catching {
            on_login(&bot);
        }

        let rest_due =
            bot.opts().rest_packets && last_rest.elapsed() >= Duration::from_millis(1200);
        let options_due = last_options.elapsed() >= Duration::from_millis(250);
        if !rest_due && !options_due {
            continue;
        }
        if !crate::api::player::alive(&bot).unwrap_or(false) {
            continue;
        }
        if rest_due && !crate::api::player::in_combat(&bot).unwrap_or(true) {
            last_rest = Instant::now();
            let _ = crate::api::send::packet(&bot, "%xt%zm%restRequest%1%%", "String");
        }
        if options_due {
            last_options = Instant::now();
            if let Err(e) = crate::api::options::apply(&bot) {
                tracing::debug!("Applying options failed: {e}");
            }
        }
    }
}

/// Once per login: `catchPackets` and remember who we are.
fn on_login(bot: &Bot) {
    if let Err(e) = bot.flash.call("catchPackets", vec![]) {
        tracing::debug!("catchPackets failed: {e}");
        return;
    }
    let username = crate::api::player::username(bot)
        .ok()
        .filter(|u| !u.is_empty());
    let mut state = bot.state.lock();
    state.catching_packets = true;
    if username.is_some() {
        state.username = username;
    }
}
