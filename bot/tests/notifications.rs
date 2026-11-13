//! Diagnostics > Notification Center: the feed every bot collects from its events (VibeSkua
//! `NotificationCenterViewModel`), with no window open.

use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::events::BotEvent;
use turu_bot::models::ItemBase;
use turu_bot::notifications::{MAXIMUM_NOTIFICATIONS, NotificationKind, to_text};
use turu_bot::script;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

fn new_bot() -> Arc<Bot> {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    Bot::new()
}

fn pext_json(bot: &Arc<Bot>, data: Value) {
    let packet = json!({ "params": { "type": "json", "dataObj": data } });
    bot.on_flash_call("pext", vec![Value::String(packet.to_string())]);
}

/// The feed's messages and kinds, oldest first.
fn feed(bot: &Bot) -> Vec<(NotificationKind, String)> {
    let mut entries: Vec<_> = bot
        .notifications
        .entries()
        .into_iter()
        .map(|e| (e.kind, e.message))
        .collect();
    entries.reverse();
    entries
}

fn item(name: &str) -> ItemBase {
    ItemBase {
        id: 1,
        name: name.into(),
        ..Default::default()
    }
}

#[test]
fn game_events_fill_the_feed_from_the_start() {
    let bot = new_bot();
    assert!(bot.notifications.is_empty());

    pext_json(
        &bot,
        json!({ "cmd": "moveToArea", "strMapName": "battleon", "strMapFileName": "a.swf" }),
    );
    pext_json(
        &bot,
        json!({ "cmd": "dropItem", "items": { "1234": { "ItemID": 1234, "sName": "Fang", "iQty": 1 } } }),
    );
    pext_json(
        &bot,
        json!({ "cmd": "sellItem", "CharItemID": 9, "iQty": 3, "iQtyNow": 7, "intAmount": 12500, "bCoins": 0 }),
    );
    bot.events.emit(BotEvent::ItemAdded {
        item: item("Fang"),
        quantity_now: 1,
    });
    bot.events.emit(BotEvent::PlayerDeath);
    bot.events
        .emit(BotEvent::ReloginTriggered { was_kicked: true });
    bot.events
        .emit(BotEvent::ReloginTriggered { was_kicked: false });
    bot.events.emit(BotEvent::QuestAccepted { quest_id: 7551 });
    bot.events.emit(BotEvent::QuestTurnedIn { quest_id: 7551 });
    bot.events.emit(BotEvent::ItemSold {
        char_item_id: 1,
        quantity_sold: 1,
        quantity_now: 0,
        cost: 50,
        is_ac: true,
    });
    // Not in the feed (VibeSkua's curated list).
    bot.events.emit(BotEvent::ItemBought);
    bot.events.emit(BotEvent::MonsterKilled { map_id: 1 });

    use NotificationKind::*;
    assert_eq!(
        feed(&bot),
        [
            (System, "Entered battleon".to_owned()),
            (Item, "Fang (dropped)".to_owned()),
            (Item, "Sold 3 item(s) for 12,500 gold".to_owned()),
            (Item, "Fang (picked up)".to_owned()),
            (Warning, "Your character died.".to_owned()),
            (Warning, "Connection interrupted; reconnecting.".to_owned()),
            (Warning, "Reconnecting to the game.".to_owned()),
            (Activity, "Accepted quest #7551".to_owned()),
            (Activity, "Completed quest #7551".to_owned()),
            (Item, "Sold 1 item(s) for 50 AC".to_owned()),
        ]
    );
    assert_eq!(bot.notifications.count(None), 10);
    assert_eq!(bot.notifications.count(Some(Item)), 4);
    assert_eq!(bot.notifications.count(Some(Warning)), 3);
    assert_eq!(bot.notifications.count(Some(System)), 1);
    assert_eq!(bot.notifications.count(Some(Error)), 0);

    // Copy and Save: oldest first, "HH:mm:ss [Kind] message".
    let entries = bot.notifications.entries();
    let text = to_text(&entries);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 10);
    assert!(
        lines[0].ends_with(" [Map] Entered battleon"),
        "{}",
        lines[0]
    );
    assert_eq!(lines[0].as_bytes()[2], b':');
    assert!(lines[9].ends_with(" [Item] Sold 1 item(s) for 50 AC"));

    let revision = bot.notifications.revision();
    bot.notifications.clear();
    assert!(bot.notifications.is_empty());
    assert_ne!(bot.notifications.revision(), revision);
}

#[test]
fn the_feed_keeps_the_newest_500() {
    let bot = new_bot();
    for quest_id in 0..(MAXIMUM_NOTIFICATIONS as i64 + 20) {
        bot.events.emit(BotEvent::QuestAccepted { quest_id });
    }
    let entries = bot.notifications.entries();
    assert_eq!(entries.len(), MAXIMUM_NOTIFICATIONS);
    assert_eq!(entries[0].message, "Accepted quest #519");
    assert_eq!(entries.last().unwrap().message, "Accepted quest #20");
}

struct FakeGame;

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        if let Some(answer) =
            turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default())
        {
            return Ok(answer);
        }
        Ok(match name {
            "isLoggedIn" => "true".into(),
            _ => "null".into(),
        })
    }
}

fn run(bot: &Arc<Bot>, dir: &std::path::Path, file: &str, source: &str) {
    std::fs::write(dir.join(file), source).unwrap();
    bot.scripts.load(&dir.join(file));
    script::start(bot).unwrap();
    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "script didn't finish"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn script_starts_stops_and_errors_are_named() {
    let dir = std::env::temp_dir().join(format!("turu-notifications-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bot = new_bot();
    bot.set_host(Some(Arc::new(FakeGame)));

    run(
        &bot,
        &dir,
        "Farm.ts",
        "import { bot } from \"turu\";
export default function main(): void { bot.log(\"hi\"); }
",
    );
    run(
        &bot,
        &dir,
        "Broken.ts",
        "export default function main(): void { throw new Error(\"boom\"); }
",
    );

    use NotificationKind::*;
    let entries = feed(&bot);
    assert_eq!(entries[0], (Activity, "Script started: Farm".to_owned()));
    assert_eq!(entries[1], (Activity, "Script stopped: Farm".to_owned()));
    assert_eq!(entries[2], (Activity, "Script started: Broken".to_owned()));
    assert_eq!(entries[3].0, Error);
    assert!(
        entries[3].1.starts_with("Script error (Broken): ") && entries[3].1.contains("boom"),
        "{}",
        entries[3].1
    );
    assert_eq!(entries[4], (Activity, "Script stopped: Broken".to_owned()));
    assert_eq!(entries.len(), 5);
}
