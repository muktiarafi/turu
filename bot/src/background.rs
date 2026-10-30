//! The bot's own threads (VibeSkua: the 20 ms "ScriptInterface" timer and the startup
//! handler): answers `skua.swf`'s startup calls, finishes packet work that needs Flash, keeps
//! the options applied in game and watches for disconnects (auto-relogin).

use crate::Bot;
use crate::api::inventory::{self, Container};
use crate::api::{map, servers};
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

fn do_work(bot: &Arc<Bot>, work: Work) {
    let result = match &work {
        Work::LoadClient => {
            // VibeSkua's "Client Animation Frame-rate": skua.swf runs at 30 until Set FPS
            // applies in game.
            let fps = bot.settings.read().animation_frame_rate.max(1);
            if let Err(e) = bot.flash.set("stage.frameRate", fps) {
                bot.logs
                    .debug
                    .push(format!("Couldn't set the frame rate: {e}"));
            }
            bot.flash.call("loadClient", vec![]).map(|_| ())
        }
        Work::AutoLogin => {
            // Logging in takes a while; items added meanwhile still need this thread.
            let bot = bot.clone();
            std::thread::Builder::new()
                .name("Turu Login".into())
                .spawn(move || crate::startup::auto_login(&bot))
                .map(|_| ())
                .map_err(|e| crate::BotError::Other(e.to_string()))
        }
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
/// every 250 ms while playing. Its watchdog starts auto-relogin on an unexpected disconnect,
/// a kick (checked every 1 s) or a loading screen that's stuck (checked every 100 ms).
fn timer(weak: Weak<Bot>) {
    let mut last_options = Instant::now();
    let mut last_rest = Instant::now();
    let mut last_kick_check = Instant::now();
    let mut last_load_check = Instant::now();
    let mut load = LoadWatch::default();
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let Some(bot) = weak.upgrade() else { break };
        if !bot.state.lock().game_loaded || !bot.flash.has_host() {
            continue;
        }
        let in_game = crate::api::player::is_in_game(&bot).unwrap_or(false);
        let (was_in_game, catching, intentional) = {
            let mut state = bot.state.lock();
            let was = state.in_game;
            state.in_game = in_game;
            if !in_game && was {
                // The session ended: its drops are gone.
                state.drops.clear();
            }
            if in_game && !was {
                // A new login re-arms auto-relogin. VibeSkua clears it on every tick in game,
                // but the world stays up for a moment after you click Logout: the lost
                // connection that follows would then relog you.
                state.intentional_logout = false;
            } else if !in_game {
                state.catching_packets = false;
            }
            (was, state.catching_packets, state.intentional_logout)
        };
        if !in_game
            && (was_in_game || bot.scripts.is_running())
            && !intentional
            && !bot.servers.is_relogging()
        {
            let (auto, watchdog) = {
                let o = bot.opts();
                (o.auto_relogin, o.relogin_watchdog)
            };
            if auto && watchdog {
                bot.log("Unexpected disconnection detected by watchdog. Triggering auto re-login.");
                servers::begin_logout(&bot, false);
            }
        }
        if last_kick_check.elapsed() >= Duration::from_secs(1) {
            last_kick_check = Instant::now();
            if !bot.servers.is_relogging()
                && bot.opts().auto_relogin
                && !bot.state.lock().intentional_logout
                && servers::kicked(&bot).unwrap_or(false)
            {
                bot.log("Player kick detected. Triggering auto re-login.");
                servers::begin_logout(&bot, false);
            }
        }
        if last_load_check.elapsed() >= Duration::from_millis(100) {
            last_load_check = Instant::now();
            check_stuck_loading(&bot, &mut load);
        }
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

/// Once per login: `catchPackets` and remember who we are and where.
fn on_login(bot: &Bot) {
    if let Err(e) = bot.flash.call("catchPackets", vec![]) {
        tracing::debug!("catchPackets failed: {e}");
        return;
    }
    let username = crate::api::player::username(bot)
        .ok()
        .filter(|u| !u.is_empty());
    bot.servers.set_last(
        crate::api::player::server_ip(bot).unwrap_or_default(),
        bot.flash
            .get_or("objServerInfo.sName", String::new())
            .unwrap_or_default(),
    );
    let mut state = bot.state.lock();
    state.catching_packets = true;
    if username.is_some() {
        state.username = username;
    }
}

/// The loading screen's message and since when it shows.
struct LoadWatch {
    detail: String,
    since: Instant,
}

impl Default for LoadWatch {
    fn default() -> Self {
        Self {
            detail: String::new(),
            since: Instant::now(),
        }
    }
}

/// VibeSkua `CheckStuckonLoading`: "Connection has been lost" starts auto-relogin; a message
/// that doesn't change for `LoadTimeout` gets the map reloaded ("loading map") or a relogin
/// (a logout without auto-relogin).
fn check_stuck_loading(bot: &Arc<Bot>, load: &mut LoadWatch) {
    let detail = servers::conn_detail(bot);
    if detail == "null" {
        *load = LoadWatch {
            detail,
            since: Instant::now(),
        };
        return;
    }
    let relogging = bot.servers.is_relogging();
    let timeout = Duration::from_millis(bot.opts().load_timeout.max(0) as u64);
    if detail.contains("has been lost") && !relogging {
        // Checked 10 times a second: without auto-relogin there's nothing to start (or log).
        if bot.opts().auto_relogin && !bot.state.lock().intentional_logout {
            bot.log("Connection lost detected. Triggering auto re-login.");
            servers::begin_logout(bot, false);
        }
    } else if load.since.elapsed() >= timeout && detail == load.detail && !relogging {
        bot.log(format!(
            "Stuck on the loading screen (\"{detail}\"). Recovering."
        ));
        recover_stuck_loading(bot, detail.contains("loading map"));
        // Turu: wait another LoadTimeout before trying again (VibeSkua's timer is blocked by
        // the recovery instead).
        load.since = Instant::now();
    }
    if detail != load.detail {
        *load = LoadWatch {
            detail,
            since: Instant::now(),
        };
    }
}

/// Off the timer thread, as joining a map waits.
fn recover_stuck_loading(bot: &Arc<Bot>, loading_map: bool) {
    let bot = bot.clone();
    let _ = std::thread::Builder::new()
        .name("Turu Load Watchdog".into())
        .spawn(move || {
            if loading_map {
                let last_map = bot.state.lock().last_map.clone();
                let _ = map::join(&bot, "battleon", "Enter", "Spawn", false, true);
                let _ = map::reload(&bot);
                // VibeSkua checks again in a 500-tick handler (10 s).
                std::thread::sleep(Duration::from_secs(10));
                if servers::conn_detail(&bot) != "loading map" {
                    if let Some(map) = last_map {
                        let _ = map::join(&bot, &map, "Enter", "Spawn", false, true);
                    }
                    return;
                }
            }
            if bot.opts().auto_relogin {
                servers::begin_logout(&bot, false);
            } else if let Err(e) = servers::logout(&bot) {
                bot.logs
                    .debug
                    .push(format!("Logout after a stuck load failed: {e}"));
            }
        });
}
