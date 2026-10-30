//! Incoming extension packets (`pext`) and outgoing packets (`packet`) -> state and events
//! (VibeSkua `ScriptInterface.HandleFlashCall`, cases `pext` and `packet`).
//!
//! Both run on the player's thread while it is locked: they only update [`GameState`],
//! emit events and post [`Work`]. They never call Flash or block.
//!
//! [`GameState`]: crate::state::GameState

use crate::Bot;
use crate::events::{BotEvent, Work};
use crate::models::ItemBase;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

/// A server packet: `{ params: { type: "json" | "str", dataObj } }`.
pub fn on_extension_packet(bot: &Arc<Bot>, packet: Value) {
    let params = packet.get("params");
    let kind = params.and_then(|p| p.get("type")).and_then(Value::as_str);
    let data = params
        .and_then(|p| p.get("dataObj"))
        .unwrap_or(&Value::Null);
    match kind {
        Some("json") => on_json(bot, data),
        Some("str") => on_str(bot, data),
        _ => {}
    }
    bot.events.emit(BotEvent::ExtensionPacket(packet));
}

fn on_json(bot: &Arc<Bot>, data: &Value) {
    let Some(cmd) = data.get("cmd").and_then(Value::as_str) else {
        return;
    };
    match cmd {
        "event" => {
            if let Some(zone) = data
                .get("args")
                .and_then(|a| a.get("zoneSet"))
                .and_then(Value::as_str)
            {
                bot.events.emit(BotEvent::RunToArea {
                    zone: zone.to_owned(),
                });
            }
        }
        "moveToArea" => {
            let map = text(data.get("strMapName"));
            {
                let mut state = bot.state.lock();
                state.map_file_path = Some(text(data.get("strMapFileName")));
                state.map_name = Some(map.clone());
                state.last_map = Some(map.clone());
            }
            bot.events.emit(BotEvent::MapChanged { map });
        }
        "ct" => on_combat_tick(bot, data),
        "sellItem" => {
            bot.state.lock().signals.item_sold = true;
            bot.events.emit(BotEvent::ItemSold);
        }
        "buyItem" => {
            if num(data.get("bitSuccess")) == Some(1) {
                bot.state.lock().signals.item_bought = true;
                bot.events.emit(BotEvent::ItemBought);
            }
        }
        "dropItem" => {
            let Some(item) = first_item(data.get("items")) else {
                return;
            };
            bot.state.lock().add_drop(item.clone());
            bot.events.emit(BotEvent::ItemDropped { item });
        }
        "addItems" => {
            // The packet only carries the quantities; the item itself is looked up in the
            // inventory / temp inventory / bank on the bot's thread (that calls Flash).
            let Some((key, value)) = data
                .get("items")
                .and_then(Value::as_object)
                .and_then(|o| o.iter().next())
            else {
                return;
            };
            let Ok(item_id) = key.trim().parse::<i64>() else {
                return;
            };
            let mut fallback: ItemBase = serde_json::from_value(value.clone()).unwrap_or_default();
            if fallback.id == 0 {
                fallback.id = item_id;
            }
            let quantity_now = num(value.get("iQtyNow")).unwrap_or(fallback.quantity);
            bot.events.post(Work::ItemAdded {
                item_id,
                quantity_now,
                fallback: Box::new(fallback),
            });
        }
        "getDrop" => {
            let item_id = num(data.get("ItemID")).unwrap_or_default();
            if num(data.get("bSuccess")) == Some(1) {
                let mut state = bot.state.lock();
                state.stats.drops += num(data.get("iQty")).unwrap_or_default().max(0) as u64;
                // The item left the drop stack (VibeSkua removes it when it sends getDrop).
                state.drops.remove(&item_id);
            }
            if truthy(data.get("bBank")) {
                bot.events.post(Work::BankItemAdded {
                    item_id,
                    quantity_now: num(data.get("iQtyNow")).unwrap_or_default(),
                });
            }
        }
        "addGoldExp" => {
            if data.get("typ").and_then(Value::as_str) == Some("m") {
                let map_id = num(data.get("id")).unwrap_or_default();
                bot.state.lock().stats.kills += 1;
                bot.events.emit(BotEvent::MonsterKilled { map_id });
            }
        }
        "ccqr" => {
            if num(data.get("bSuccess")) == Some(1) {
                let quest_id = num(data.get("QuestID")).unwrap_or_default();
                bot.state.lock().stats.quests_completed += 1;
                bot.events.emit(BotEvent::QuestTurnedIn { quest_id });
            }
        }
        "loadBank" => {
            {
                let mut state = bot.state.lock();
                state.bank_loaded = true;
                state.signals.bank_loaded = true;
            }
            bot.events.emit(BotEvent::BankLoaded);
        }
        "loadShop" => {
            bot.state.lock().shop_loaded_at = Some(Instant::now());
            bot.events.emit(BotEvent::ShopLoaded);
        }
        _ => {}
    }
}

/// `ct`: our HP at 0 is a death; the counter attack animation / aura is a counter attack.
fn on_combat_tick(bot: &Arc<Bot>, data: &Value) {
    let username = bot.state.lock().username.clone();
    let me = username.and_then(|name| {
        let players = data.get("p")?.as_object()?;
        players
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(&name))
            .map(|(_, v)| v)
    });
    if let Some(me) = me
        && me.get("intHP").is_some()
        && num(me.get("intHP")) == Some(0)
    {
        bot.state.lock().stats.deaths += 1;
        // VibeSkua `ScriptCombat.PlayerDead`.
        bot.combat.set_stop_attacking(false);
        bot.events.emit(BotEvent::PlayerDeath);
        return;
    }
    let anim_msg = data
        .get("anims")
        .and_then(|a| a.get(0))
        .and_then(|a| a.get("msg"))
        .and_then(Value::as_str);
    if anim_msg.is_some_and(|m| m.contains("prepares a counter attack!")) {
        super::combat::on_counter_attack(bot, false);
        bot.events.emit(BotEvent::CounterAttack { faded: false });
        return;
    }
    if let Some(auras) = data.get("a").and_then(Value::as_array) {
        let faded = auras.iter().any(|a| {
            a.get("aura")
                .and_then(|aura| aura.get("nam"))
                .and_then(Value::as_str)
                == Some("Counter Attack")
        });
        if faded {
            super::combat::on_counter_attack(bot, true);
            bot.events.emit(BotEvent::CounterAttack { faded: true });
        }
    }
}

fn on_str(bot: &Arc<Bot>, data: &Value) {
    let field = |i: usize| data.get(i).map(|v| text(Some(v))).unwrap_or_default();
    match field(0).as_str() {
        "uotls" => {
            let is_me = bot
                .state
                .lock()
                .username
                .as_deref()
                .is_some_and(|u| u.eq_ignore_ascii_case(&field(2)));
            if is_me {
                let afk = field(3) == "afk:true";
                bot.state.lock().afk = afk;
                if afk {
                    bot.events.emit(BotEvent::PlayerAfk);
                }
            }
        }
        "loginResponse" => {
            let username = field(4);
            {
                let mut state = bot.state.lock();
                state.reset_session();
                state.intentional_logout = false;
                if !username.is_empty() {
                    state.username = Some(username.clone());
                }
            }
            // VibeSkua clears the custom name and guild on every login. Never block here:
            // a script thread may hold the options lock while it waits on Flash.
            if let Some(mut options) = bot.options.try_write() {
                options.custom_name.clear();
                options.custom_guild.clear();
            }
            bot.events.emit(BotEvent::Login { username });
        }
        "popup" => tracing::debug!("popup packet: {data}"),
        _ => {}
    }
}

/// A client packet: `%xt%zm%cmd%room%...%`.
pub fn on_packet_sent(bot: &Arc<Bot>, packet: &str) {
    // VibeSkua: `Split('%', RemoveEmptyEntries)`.
    let parts: Vec<&str> = packet.split('%').filter(|p| !p.is_empty()).collect();
    let part = |i: usize| parts.get(i).copied().unwrap_or_default();
    match part(2) {
        "eLO" | "eLO2" | "eSL" | "eSL2" => {
            bot.logs.debug.push(format!(
                "In-game exit packet ({}) detected. Disarming auto re-login.",
                part(2)
            ));
            bot.state.lock().intentional_logout = true;
            crate::api::servers::cancel_relogin(bot);
            crate::script::stop(bot);
        }
        "moveToCell" => {
            let map = bot.state.lock().map_name.clone().unwrap_or_default();
            bot.events.emit(BotEvent::CellChanged {
                map,
                cell: part(4).to_owned(),
                pad: part(5).to_owned(),
            });
        }
        "buyItem" => bot.events.emit(BotEvent::TryBuyItem {
            shop_id: part(5).parse().unwrap_or_default(),
            item_id: part(4).parse().unwrap_or_default(),
            shop_item_id: part(6).parse().unwrap_or_default(),
        }),
        "acceptQuest" => {
            let quest_id = part(4).parse().unwrap_or_default();
            bot.state.lock().stats.quests_accepted += 1;
            bot.events.emit(BotEvent::QuestAccepted { quest_id });
        }
        // An auto-relogin's own logout.
        "cmd" if part(4) == "logout" && bot.servers.is_relogging() => {}
        "cmd" if part(4) == "logout" => {
            let intentional = {
                let mut state = bot.state.lock();
                state.drops.clear();
                state.intentional_logout
            };
            bot.events.emit(BotEvent::Logout);
            if intentional {
                bot.logs
                    .debug
                    .push("Intentional logout confirmed. Auto re-login skipped.");
                crate::api::servers::cancel_relogin(bot);
            } else {
                bot.logs
                    .debug
                    .push("Server logout command detected. Triggering auto re-login.");
                crate::api::servers::begin_logout(bot, true);
            }
        }
        _ => {}
    }
    bot.events.emit(BotEvent::PacketSent(packet.to_owned()));
}

/// The first item of an `items` object (`{ "<ItemID>": { ...item } }`).
fn first_item(items: Option<&Value>) -> Option<ItemBase> {
    let (key, value) = items?.as_object()?.iter().next()?;
    let mut item: ItemBase = serde_json::from_value(value.clone()).ok()?;
    if item.id == 0 {
        item.id = key.trim().parse().unwrap_or_default();
    }
    Some(item)
}

/// A number, numeric string or bool as an integer (the game mixes them).
pub(crate) fn num(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<f64>().ok().map(|f| f as i64),
        Value::Bool(b) => Some(*b as i64),
        _ => None,
    }
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.trim(), "1" | "true" | "True"),
        other => num(other).is_some_and(|n| n != 0),
    }
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}
