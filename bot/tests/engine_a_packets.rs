//! Packet handling (`pext` / `packet` from skua.swf) -> state, stats and events.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::api::{NameOrId, wait};
use turu_bot::events::BotEvent;

fn new_bot() -> Arc<Bot> {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    Bot::new()
}

/// Records every event's name and payload.
fn record(bot: &Bot) -> Arc<Mutex<Vec<(String, Value)>>> {
    let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let sink = seen.clone();
    bot.events.subscribe(move |e: &BotEvent| {
        if !matches!(e, BotEvent::ExtensionPacket(_) | BotEvent::PacketSent(_)) {
            sink.lock().push((e.name().to_owned(), e.payload()));
        }
    });
    seen
}

fn pext_json(bot: &Arc<Bot>, data: Value) {
    let packet = json!({ "params": { "type": "json", "dataObj": data } });
    bot.on_flash_call("pext", vec![Value::String(packet.to_string())]);
}

fn pext_str(bot: &Arc<Bot>, data: &[&str]) {
    let packet = json!({ "params": { "type": "str", "dataObj": data } });
    bot.on_flash_call("pext", vec![Value::String(packet.to_string())]);
}

fn sent(bot: &Arc<Bot>, packet: &str) {
    bot.on_flash_call("packet", vec![Value::String(packet.into())]);
}

#[test]
fn move_to_area_sets_the_map() {
    let bot = new_bot();
    let seen = record(&bot);
    pext_json(
        &bot,
        json!({ "cmd": "moveToArea", "strMapName": "battleon", "strMapFileName": "Battleon/town-Battleon-7Nov23.swf" }),
    );
    let state = bot.state.lock();
    assert_eq!(state.map_name.as_deref(), Some("battleon"));
    assert_eq!(state.last_map.as_deref(), Some("battleon"));
    assert_eq!(
        state.map_file_path.as_deref(),
        Some("Battleon/town-Battleon-7Nov23.swf")
    );
    assert_eq!(
        seen.lock()[0],
        ("mapChanged".to_owned(), json!({ "map": "battleon" }))
    );
}

#[test]
fn drops_stack_up_and_leave_on_pickup() {
    let bot = new_bot();
    let seen = record(&bot);
    let drop = json!({ "cmd": "dropItem", "items": { "1234": { "ItemID": 1234, "sName": "Fang", "iQty": 1, "iStk": 10 } } });
    pext_json(&bot, drop.clone());
    pext_json(&bot, drop);
    {
        let state = bot.state.lock();
        assert_eq!(state.drops[&1234].quantity, 2);
        assert!(state.drop_exists(NameOrId::Name("fang")));
        assert!(state.drop_exists(NameOrId::Name("*")));
        assert!(state.drop_exists(NameOrId::Id(1234)));
    }
    let names: Vec<String> = seen.lock().iter().map(|(n, _)| n.clone()).collect();
    assert_eq!(names, ["itemDropped", "itemDropped"]);
    assert_eq!(seen.lock()[0].1["name"], "Fang");

    pext_json(
        &bot,
        json!({ "cmd": "getDrop", "bSuccess": 1, "ItemID": 1234, "iQty": 2, "bBank": false }),
    );
    let state = bot.state.lock();
    assert!(state.drops.is_empty());
    assert_eq!(state.stats.drops, 2);
}

#[test]
fn kills_deaths_and_quests_are_counted() {
    let bot = new_bot();
    let seen = record(&bot);
    bot.state.lock().username = Some("Hero".into());

    pext_json(
        &bot,
        json!({ "cmd": "addGoldExp", "typ": "m", "id": 7, "intExp": 10 }),
    );
    pext_json(&bot, json!({ "cmd": "addGoldExp", "typ": "p", "id": 1 }));
    pext_json(
        &bot,
        json!({ "cmd": "ct", "p": { "hero": { "intHP": 0 } } }),
    );
    pext_json(
        &bot,
        json!({ "cmd": "ct", "p": { "hero": { "intHP": 500 } } }),
    );
    pext_json(
        &bot,
        json!({ "cmd": "ccqr", "bSuccess": 1, "QuestID": 236 }),
    );
    pext_json(
        &bot,
        json!({ "cmd": "ccqr", "bSuccess": 0, "QuestID": 236 }),
    );
    sent(&bot, "%xt%zm%acceptQuest%5%236%");

    let stats = bot.state.lock().stats.clone();
    assert_eq!(
        (
            stats.kills,
            stats.deaths,
            stats.quests_completed,
            stats.quests_accepted
        ),
        (1, 1, 1, 1)
    );
    let seen = seen.lock();
    assert_eq!(seen[0], ("monsterKilled".to_owned(), json!({ "mapId": 7 })));
    assert_eq!(seen[1].0, "playerDeath");
    assert_eq!(
        seen[2],
        ("questTurnedIn".to_owned(), json!({ "questId": 236 }))
    );
    assert_eq!(
        seen[3],
        ("questAccepted".to_owned(), json!({ "questId": 236 }))
    );
    assert_eq!(seen.len(), 4);
}

#[test]
fn counter_attacks_are_reported() {
    let bot = new_bot();
    let seen = record(&bot);
    pext_json(
        &bot,
        json!({ "cmd": "ct", "anims": [{ "msg": "Ultra Boss prepares a counter attack!" }] }),
    );
    pext_json(
        &bot,
        json!({ "cmd": "ct", "a": [{ "aura": { "nam": "Counter Attack" } }] }),
    );
    let seen = seen.lock();
    assert_eq!(
        seen[0],
        ("counterAttack".to_owned(), json!({ "faded": false }))
    );
    assert_eq!(
        seen[1],
        ("counterAttack".to_owned(), json!({ "faded": true }))
    );
}

#[test]
fn bank_shop_buy_and_sell_signal_their_waits() {
    let bot = new_bot();
    let seen = record(&bot);
    assert!(!wait::for_bank_load(&bot, 1).unwrap());
    pext_json(&bot, json!({ "cmd": "loadBank", "items": [] }));
    assert!(bot.state.lock().bank_loaded);
    assert!(wait::for_bank_load(&bot, 1).unwrap());
    // One-shot: consumed by the wait.
    assert!(!wait::for_bank_load(&bot, 1).unwrap());

    pext_json(&bot, json!({ "cmd": "buyItem", "bitSuccess": 0 }));
    assert!(!wait::for_item_buy(&bot, 1).unwrap());
    pext_json(
        &bot,
        json!({ "cmd": "buyItem", "bitSuccess": 1, "CharItemID": 5 }),
    );
    assert!(wait::for_item_buy(&bot, 1).unwrap());
    pext_json(
        &bot,
        json!({ "cmd": "sellItem", "CharItemID": 5, "iQty": 1 }),
    );
    assert!(wait::for_item_sell(&bot, 1).unwrap());
    pext_json(&bot, json!({ "cmd": "loadShop", "shopinfo": {} }));
    assert!(bot.state.lock().shop_loaded_at.is_some());

    let names: Vec<String> = seen.lock().iter().map(|(n, _)| n.clone()).collect();
    assert_eq!(
        names,
        ["bankLoaded", "itemBought", "itemSold", "shopLoaded"]
    );
}

#[test]
fn login_response_and_outgoing_packets() {
    let bot = new_bot();
    let seen = record(&bot);
    bot.options.write().custom_name = "Someone".into();
    bot.state.lock().drops.insert(1, Default::default());
    pext_str(
        &bot,
        &["loginResponse", "-1", "true", "12345", "Hero", "message"],
    );
    {
        let state = bot.state.lock();
        assert_eq!(state.username.as_deref(), Some("Hero"));
        assert!(state.drops.is_empty());
    }
    assert!(bot.options.read().custom_name.is_empty());

    pext_json(
        &bot,
        json!({ "cmd": "moveToArea", "strMapName": "yulgar", "strMapFileName": "yulgar.swf" }),
    );
    sent(&bot, "%xt%zm%moveToCell%12%Room%Left%");
    sent(&bot, "%xt%zm%buyItem%12%1234%56%789%");
    pext_str(&bot, &["uotls", "-1", "hero", "afk:true"]);
    sent(&bot, "%xt%zm%cmd%12%logout%");

    let seen = seen.lock();
    assert_eq!(seen[0], ("login".to_owned(), json!({ "username": "Hero" })));
    assert_eq!(
        seen[2],
        (
            "cellChanged".to_owned(),
            json!({ "map": "yulgar", "cell": "Room", "pad": "Left" })
        )
    );
    assert_eq!(
        seen[3],
        (
            "tryBuyItem".to_owned(),
            json!({ "shopId": 56, "itemId": 1234, "shopItemId": 789 })
        )
    );
    assert_eq!(seen[4].0, "playerAFK");
    assert_eq!(seen[5].0, "logout");
}

#[test]
fn every_packet_is_forwarded() {
    let bot = new_bot();
    let count: Arc<Mutex<(u32, u32)>> = Arc::default();
    let sink = count.clone();
    bot.events.subscribe(move |e: &BotEvent| match e {
        BotEvent::ExtensionPacket(_) => sink.lock().0 += 1,
        BotEvent::PacketSent(_) => sink.lock().1 += 1,
        _ => {}
    });
    pext_json(&bot, json!({ "cmd": "whatever" }));
    pext_str(&bot, &["server", "-1", "hello"]);
    sent(&bot, "%xt%zm%restRequest%1%%");
    assert_eq!(*count.lock(), (2, 1));
}
