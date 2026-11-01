//! Tools > Loader's quest cache and Tools > Grabber's readers and actions against a fake game.

use parking_lot::Mutex;
use serde_json::{Map, Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use turu_bot::grabber::{self, GrabAction, GrabKind, Grabbed, Prepared};
use turu_bot::models::{ItemBase, ShopItem};
use turu_bot::quest_cache::{self, QuestData, QuestSummary, UpdateMode};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// Quests the fake server knows: 1 to this ID.
const LAST_QUEST: i64 = 70;

fn raw_quest(id: i64) -> Value {
    json!({
        "QuestID": id.to_string(), "sName": format!(" Quest {id} "), "iSlot": 3, "iValue": id,
        "bOnce": "1", "bUpg": "0", "iLvl": 10, "sField": null, "iIndex": 0, "iGold": 100, "iExp": 50,
        "iClass": 0, "iReqCP": 0, "FactionID": 1, "iReqRep": 0,
        "oItems": { "35": { "ItemID": "35", "sName": "Chieftain's Head", "sDesc": "x", "iQty": 1, "iStk": 1,
                            "bUpg": 0, "bTemp": "1", "sType": "Item", "sES": "None" } },
        "turnin": [ { "ItemID": 35, "iQty": 4 } ],
        "oReqd": [],
        "oRewards": { "itemsS": { "559": { "ItemID": 559, "sName": "Sword", "iQty": 1, "sType": "Sword" } } },
        "reward": [ { "ItemID": 559, "iQty": 1, "iRate": 100, "iType": 0, "sType": "Sword" } ]
    })
}

struct FakeGame {
    logged_in: AtomicBool,
    /// `world.questTree`.
    tree: Mutex<Map<String, Value>>,
    /// The last `showQuests` found nothing (the fake then reports you dead, which ends
    /// VibeSkua's wait at once instead of after 10 s).
    nothing_found: AtomicBool,
    loads: Mutex<Vec<String>>,
    client_packets: Mutex<Vec<String>>,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let arg = |i: usize| match args.get(i) {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        let answer = match (name, arg(0).as_str()) {
            ("isLoggedIn", _) => self.logged_in.load(Ordering::SeqCst).to_string(),
            ("isNull", _) => "false".into(),
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => {
                if self.nothing_found.load(Ordering::SeqCst) { "0" } else { "1" }.into()
            }
            ("getGameObject", "world.questTree") => Value::Object(self.tree.lock().clone()).to_string(),
            ("getGameObject", "world.myAvatar.objData.intGold") => "1000".into(),
            ("getGameObject", "world.myAvatar.items") => json!([
                { "ItemID": 1, "CharItemID": 100, "sName": "Dragon Scale", "iQty": 5, "iStk": 99, "sType": "Item", "bCoins": "1" },
                { "ItemID": 2, "CharItemID": 200, "sName": "Void Highlord", "iQty": 1, "sType": "Class", "bEquip": "1", "bUpg": "1" }
            ])
            .to_string(),
            ("getGameObject", "world.myAvatar.tempitems") => {
                json!([{ "ItemID": 9, "sName": "Bone", "iQty": 3, "sType": "Item" }]).to_string()
            }
            ("getGameObject", "world.monsters") | ("getGameObject", "world.getMonstersByCell") => "null".into(),
            ("setGameObject", "world.questTree") => {
                self.tree.lock().clear();
                "null".into()
            }
            ("callGameFunction", "world.showQuests") => {
                let ids: Vec<i64> = arg(1).split(',').filter_map(|s| s.parse().ok()).collect();
                self.loads.lock().push(arg(1));
                let found: Vec<i64> = ids.into_iter().filter(|id| *id <= LAST_QUEST).collect();
                self.nothing_found.store(found.is_empty(), Ordering::SeqCst);
                let mut tree = self.tree.lock();
                for id in found {
                    tree.insert(id.to_string(), raw_quest(id));
                }
                "null".into()
            }
            ("sendClientPacket", _) => {
                self.client_packets.lock().push(arg(0));
                "null".into()
            }
            _ => "null".into(),
        };
        Ok(answer)
    }
}

fn setup(dir: &str) -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test in this file sets the same value.
    unsafe {
        std::env::set_var(
            "TURU_DATA_DIR",
            std::env::temp_dir().join("turu-test-loader-grabber"),
        )
    };
    let bot = Bot::new();
    let game = Arc::new(FakeGame {
        logged_in: AtomicBool::new(true),
        tree: Mutex::new(Map::new()),
        nothing_found: AtomicBool::new(false),
        loads: Mutex::new(Vec::new()),
        client_packets: Mutex::new(Vec::new()),
    });
    bot.set_host(Some(game.clone()));
    // Each test gets its own cache file.
    let _ = std::fs::remove_dir_all(bot.paths.root.join(dir));
    (bot, game)
}

fn cache_path(bot: &Bot, dir: &str) -> std::path::PathBuf {
    bot.paths.root.join(dir).join(quest_cache::FILE_NAME)
}

fn quest(id: i64, name: &str) -> QuestData {
    QuestData {
        id,
        name: name.into(),
        ..Default::default()
    }
}

#[test]
fn parses_the_id_and_range_boxes() {
    assert_eq!(quest_cache::parse_ids("2566"), Some(vec![2566]));
    assert_eq!(
        quest_cache::parse_ids("101, 102  103,104"),
        Some(vec![101, 102, 103, 104])
    );
    assert_eq!(quest_cache::parse_ids(" , "), None);
    assert_eq!(quest_cache::parse_ids("12a"), None);
    assert_eq!(quest_cache::parse_ids(""), None);
    assert_eq!(quest_cache::parse_shop_id(" 2566 "), Some(2566));
    assert_eq!(quest_cache::parse_shop_id("1, 2"), None);
    assert_eq!(quest_cache::parse_range("2566, 2599"), Ok((2566, 2599)));
    assert_eq!(quest_cache::parse_range("5 5"), Ok((5, 5)));
    assert_eq!(
        quest_cache::parse_range("2599, 2566"),
        Err("Start ID must be less than or equal to End ID")
    );
    assert_eq!(
        quest_cache::parse_range("1, 2, 3"),
        Err("Invalid format. Use: startID, endID")
    );
    assert_eq!(
        quest_cache::parse_range("a, 2"),
        Err("Invalid format. Use: startID, endID")
    );
}

#[test]
fn splits_updates_into_chunks_of_29() {
    assert_eq!(
        quest_cache::chunks(1, 60, false),
        [(1, 29), (30, 58), (59, 87)]
    );
    assert_eq!(
        quest_cache::chunks(2566, 2599, true),
        [(2566, 2594), (2595, 2599)]
    );
    assert_eq!(quest_cache::chunks(10, 10, true), [(10, 10)]);
    assert!(quest_cache::chunks(13000, 12999, false).is_empty());
}

/// An entry of VibeSkua's shipped QuestData.json.
const VIBESKUA_ENTRY: &str = r#"{
    "ID": 2, "Slot": -1, "Value": 0, "Name": "Chieftain's Head", "Once": false, "Field": null,
    "Index": 0, "Upgrade": false, "Level": 1, "RequiredClassID": 0, "RequiredClassPoints": 0,
    "RequiredFactionId": 1, "RequiredFactionRep": 0, "Gold": 100, "XP": 100, "AcceptRequirements": [],
    "Requirements": [ { "ItemID": 35, "sName": "Chieftain's Head", "sDesc": "x", "iQty": 1, "iStk": 1,
        "bUpg": "0", "bWear": "0", "bCoins": "0", "sType": "Item", "EnhPatternID": 0, "ProcID": 0,
        "Category": "Item", "bTemp": "1", "sES": "None", "sLink": null, "sFile": null, "sMeta": null } ],
    "Rewards": [],
    "SimpleRewards": [ { "iRate": 100.0, "iType": 0, "ItemID": 559, "sName": null, "sDesc": null, "iQty": 1,
        "iStk": 0, "bUpg": "0", "bWear": "0", "bCoins": "0", "sType": "Sword", "EnhPatternID": 0,
        "ProcID": 0, "Category": "Sword", "bTemp": "0", "sES": null, "sLink": null, "sFile": null, "sMeta": null } ]
}"#;

#[test]
fn reads_and_writes_vibeskuas_format() {
    let entry: QuestData = serde_json::from_str(VIBESKUA_ENTRY).unwrap();
    assert_eq!(
        (entry.id, entry.name.as_str(), entry.gold, entry.xp),
        (2, "Chieftain's Head", 100, 100)
    );
    assert!(entry.requirements[0].temp);
    assert_eq!(entry.simple_rewards[0].rate, Some(100.0));
    // Written back, it's the same JSON, plus the `bHouse` VibeSkua's current ItemBase writes.
    let written = serde_json::to_value(&entry).unwrap();
    let mut expected: Value = serde_json::from_str(VIBESKUA_ENTRY).unwrap();
    for list in ["Requirements", "SimpleRewards"] {
        for item in expected[list].as_array_mut().unwrap() {
            item["bHouse"] = json!("0");
        }
    }
    assert_eq!(written, expected);
    let text = serde_json::to_string(&entry).unwrap();
    assert!(
        text.starts_with(r#"{"ID":2,"Slot":-1,"Value":0,"Name":"Chieftain's Head","Once":false"#),
        "{text}"
    );
    assert!(
        text.contains(r#""SimpleRewards":[{"iRate":100.0,"iType":0,"ItemID":559"#),
        "{text}"
    );
    assert!(
        text.contains(r#""ProcID":0,"bHouse":"0","Category":"Item","bTemp":"1""#),
        "{text}"
    );
}

#[test]
fn converts_the_quest_tree_like_vibeskua() {
    let q = QuestData::from_raw(&raw_quest(7)).unwrap();
    assert_eq!(
        (q.id, q.name.as_str(), q.slot, q.value, q.level),
        (7, "Quest 7", 3, 7, 10)
    );
    assert!(q.once && !q.upgrade);
    assert_eq!((q.gold, q.xp, q.required_faction_id), (100, 50, 1));
    // `oItems` with the `turnin` quantity.
    assert_eq!((q.requirements[0].id, q.requirements[0].quantity), (35, 4));
    assert_eq!(q.requirements[0].category, "Item");
    assert_eq!(q.requirements[0].rate, None);
    assert_eq!(q.rewards[0].name.as_deref(), Some("Sword"));
    assert_eq!(
        (q.simple_rewards[0].rate, q.simple_rewards[0].kind),
        (Some(100.0), Some(0))
    );
    assert_eq!(quest_cache::category_of(Some("quest item")), "Unknown");
    assert_eq!(quest_cache::category_of(Some("questitem")), "Quest Item");
    assert_eq!(quest_cache::category_of(None), "Unknown");
    assert!(QuestData::from_raw(&json!({ "sName": "no id" })).is_none());
}

#[test]
fn merges_fetched_over_cached() {
    let merged = quest_cache::merge(
        vec![quest(5, "new five"), quest(2, "new two")],
        vec![quest(1, "one"), quest(2, "old two"), quest(9, "nine")],
    );
    let names: Vec<&str> = merged.iter().map(|q| q.name.as_str()).collect();
    assert_eq!(names, ["one", "new two", "new five", "nine"]);
}

#[test]
fn searches_and_copies_cached_quests() {
    let quests = vec![
        QuestSummary {
            id: 2566,
            name: "Dragon Hunt".into(),
        },
        QuestSummary {
            id: 101,
            name: "Bones".into(),
        },
    ];
    assert_eq!(quest_cache::search(&quests, ""), [0, 1]);
    assert_eq!(quest_cache::search(&quests, "DRAGON"), [0]);
    assert_eq!(quest_cache::search(&quests, "10"), [1]);
    let all: Vec<&QuestSummary> = quests.iter().collect();
    assert_eq!(quest_cache::copy_ids(&all), "2566,101");
    assert_eq!(quest_cache::copy_names(&all), "Dragon Hunt,Bones");
    assert_eq!(
        quest_cache::copy_comments(&all),
        "// Dragon Hunt | 2566\n// Bones | 101"
    );
}

#[test]
fn reads_a_missing_or_empty_cache_as_none() {
    let (bot, _) = setup("empty");
    let path = cache_path(&bot, "empty");
    assert!(quest_cache::read(&path).unwrap().is_empty());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "").unwrap();
    assert!(quest_cache::read_summaries(&path).unwrap().is_empty());
    quest_cache::write(&path, &[quest(3, "three")]).unwrap();
    assert_eq!(
        quest_cache::read_summaries(&path).unwrap(),
        [QuestSummary {
            id: 3,
            name: "three".into()
        }]
    );
}

#[test]
fn add_missing_loads_after_the_last_cached_quest() {
    let (bot, game) = setup("add");
    let path = cache_path(&bot, "add");
    quest_cache::write(
        &path,
        &(1..=29).map(|id| quest(id, "cached")).collect::<Vec<_>>(),
    )
    .unwrap();
    let messages = Mutex::new(Vec::new());
    let progress = |m: String| messages.lock().push(m);
    let quests = quest_cache::update(
        &bot,
        &path,
        UpdateMode::AddMissing,
        &progress,
        &AtomicBool::new(false),
    )
    .unwrap();
    // 30-58 and 59-87 (only up to 70 exist), then 88-116 finds nothing.
    assert_eq!(game.loads.lock().len(), 3);
    assert!(game.loads.lock()[0].starts_with("30,31,"));
    assert_eq!(quests.len(), 70);
    assert_eq!(quests[0].name, "cached");
    assert_eq!(quests[69].name, "Quest 70");
    assert_eq!(
        *messages.lock(),
        [
            "Loading Quests 30-59...",
            "Loading Quests 59-88...",
            "Loading Quests 88-117...",
            "No more quests found.",
            "Getting quests from file QuestData.json",
        ]
    );
    assert_eq!(quest_cache::read(&path).unwrap(), quests);
}

#[test]
fn rebuild_all_cancelled_keeps_what_it_read_and_the_later_cache() {
    let (bot, _) = setup("rebuild");
    let path = cache_path(&bot, "rebuild");
    quest_cache::write(&path, &[quest(1, "old"), quest(500, "far")]).unwrap();
    let cancel = AtomicBool::new(false);
    let messages = Mutex::new(Vec::new());
    let progress = |m: String| {
        // Cancel while the first chunk loads: it's kept, nothing after it is loaded.
        cancel.store(true, Ordering::SeqCst);
        messages.lock().push(m);
    };
    let quests =
        quest_cache::update(&bot, &path, UpdateMode::RebuildAll, &progress, &cancel).unwrap();
    assert_eq!(quests.len(), 30);
    assert_eq!(quests[0].name, "Quest 1");
    assert_eq!(quests[29].name, "far");
    assert!(
        messages
            .lock()
            .contains(&"Update cancelled - saved 30 quests (partial data + existing)".to_owned())
    );
}

#[test]
fn rebuild_all_replaces_the_cache() {
    let (bot, _) = setup("replace");
    let path = cache_path(&bot, "replace");
    quest_cache::write(&path, &[quest(1, "old"), quest(500, "far")]).unwrap();
    let quests = quest_cache::update(
        &bot,
        &path,
        UpdateMode::RebuildAll,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(quests.len(), LAST_QUEST as usize);
    assert!(quests.iter().all(|q| q.name.starts_with("Quest ")));
}

#[test]
fn update_range_merges_into_the_cache() {
    let (bot, game) = setup("range");
    let path = cache_path(&bot, "range");
    quest_cache::write(&path, &[quest(1, "one"), quest(65, "old 65")]).unwrap();
    let messages = Mutex::new(Vec::new());
    let progress = |m: String| messages.lock().push(m);
    let quests = quest_cache::update(
        &bot,
        &path,
        UpdateMode::Range(60, 64),
        &progress,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(*game.loads.lock(), ["60,61,62,63,64"]);
    let ids: Vec<i64> = quests.iter().map(|q| q.id).collect();
    assert_eq!(ids, [1, 60, 61, 62, 63, 64, 65]);
    assert_eq!(quests[6].name, "old 65");
    assert_eq!(messages.lock()[0], "Loading Quests 60-64...");
}

#[test]
fn update_while_logged_out_only_reads_the_file() {
    let (bot, game) = setup("offline");
    let path = cache_path(&bot, "offline");
    quest_cache::write(&path, &[quest(1, "one")]).unwrap();
    game.logged_in.store(false, Ordering::SeqCst);
    let quests = quest_cache::update(
        &bot,
        &path,
        UpdateMode::AddMissing,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(quests, [quest(1, "one")]);
    assert!(game.loads.lock().is_empty());
}

fn item(json: Value) -> ItemBase {
    serde_json::from_value(json).unwrap()
}

#[test]
fn formats_grabbed_rows_like_vibeskua() {
    let shop: ShopItem = serde_json::from_value(json!({
        "ItemID": 7, "ShopItemID": 99, "sName": "Blade", "iCost": 25000, "bUpg": "1", "iStk": 1,
        "turnin": [ { "ItemID": 11, "sName": "Dragon Scale", "iQty": 3 } ]
    }))
    .unwrap();
    let row = Grabbed::Shop(shop);
    assert_eq!(
        row.summary(),
        "[7] Blade | Cost: 25,000 Gold [Member] Requirements: [11] Dragon Scale x3"
    );
    assert_eq!(
        row.row(),
        "Blade | Cost: 25,000 Gold [Member] Requirements: [11] Dragon Scale x3"
    );
    let inv = Grabbed::Item(item(
        json!({ "ItemID": 1, "sName": "Scale", "iQty": 5, "sType": "quest item", "bCoins": 1 }),
    ));
    assert_eq!(inv.summary(), "[1] Scale x5 | Category: Unknown [AC]");
    let class = Grabbed::Item(item(
        json!({ "ItemID": 2, "sName": "Rogue", "iQty": 1, "sType": "class" }),
    ));
    assert_eq!(class.summary(), "[2] Rogue x1 | Category: Class");
    let temp = Grabbed::TempItem(item(json!({ "ItemID": 9, "sName": "Bone", "iQty": 3 })));
    assert_eq!(temp.summary(), "[9] Bone x3");
    let details = inv.details();
    assert!(
        details.contains(&("name".to_owned(), "Scale".to_owned())),
        "{details:?}"
    );
    let all = [&inv, &temp];
    assert_eq!(
        grabber::export_text("Inventory", &all),
        "=== Inventory (2 items) ===\n[1] Scale x5 | Category: Unknown [AC]\n[9] Bone x3\n"
    );
    let detailed = grabber::detailed_export_text("Inventory", &all);
    assert!(
        detailed.starts_with("=== Inventory (2 items) ===\n\n[1] Scale x5"),
        "{detailed}"
    );
    assert!(detailed.contains("\n  quantity: 5\n"), "{detailed}");
    assert!(grabber::export_text("Inventory", &[]).is_empty());
}

#[test]
fn grabs_from_the_game() {
    let (bot, _) = setup("grab");
    let inventory = grabber::grab(&bot, GrabKind::Inventory, &no_scan).unwrap();
    assert_eq!(inventory.len(), 2);
    assert_eq!(
        inventory[1].summary(),
        "[2] Void Highlord x1 | Category: Class [Member]"
    );
    let temp = grabber::grab(&bot, GrabKind::TempInventory, &no_scan).unwrap();
    assert_eq!(temp[0].summary(), "[9] Bone x3");
    assert!(
        grabber::grab(&bot, GrabKind::ShopIds, &no_scan)
            .unwrap()
            .is_empty()
    );
    // A loadShop packet records the shop for Shop IDs; loading it again replaces it.
    for name in ["Old Name", "Blacksmith"] {
        let packet = json!({ "params": { "type": "json", "dataObj": { "cmd": "loadShop", "shopinfo": {
            "ShopID": "250", "sName": name, "items": [ { "ItemID": 7, "sName": "Blade", "iCost": 5 } ] } } } });
        bot.on_flash_call("pext", vec![Value::String(packet.to_string())]);
    }
    let shops = grabber::grab(&bot, GrabKind::ShopIds, &no_scan).unwrap();
    assert_eq!(shops.len(), 1);
    assert_eq!(shops[0].summary(), "[250] Blacksmith");
    assert!(
        shops[0]
            .details()
            .iter()
            .any(|(k, v)| k == "items" && v.contains("[7] Blade"))
    );
    assert!(!GrabKind::ShopIds.select_multiple());
    assert_eq!(GrabKind::Quests.actions().len(), 5);
}

#[test]
fn prepares_actions_like_vibeskua() {
    let blade = |coins: bool, cost: i64| {
        Grabbed::Shop(
            serde_json::from_value(
                json!({ "ItemID": 7, "sName": "Blade", "iCost": cost, "bCoins": coins, "iStk": 5 }),
            )
            .unwrap(),
        )
    };
    assert_eq!(
        grabber::prepare(GrabAction::Buy, &[]),
        Prepared::Report("No items found/selected.".into())
    );
    assert_eq!(
        grabber::prepare(GrabAction::Kill, &[]),
        Prepared::Report("No monsters found/selected.".into())
    );
    assert_eq!(
        grabber::prepare(GrabAction::UnregisterAll, &[]),
        Prepared::Run
    );
    assert!(
        matches!(grabber::prepare(GrabAction::Buy, &[&blade(true, 10)]), Prepared::Warn { title, .. } if title == "AC Item")
    );
    assert_eq!(
        grabber::prepare(GrabAction::Buy, &[&blade(true, 0), &blade(false, 5)]),
        Prepared::Run
    );
    assert_eq!(
        grabber::prepare(GrabAction::Buy, &[&blade(false, 5)]),
        Prepared::AskQuantity {
            title: "Buying Blade".into(),
            prompt: "Buy quantity (Cost: 5 Gold)".into()
        }
    );
    let scale = Grabbed::Item(item(json!({ "ItemID": 1, "sName": "Scale", "iQty": 5 })));
    let class = Grabbed::Item(item(
        json!({ "ItemID": 2, "sName": "Rogue", "iQty": 1, "sType": "Class", "bEquip": 1 }),
    ));
    assert_eq!(
        grabber::prepare(GrabAction::Sell, &[&scale]),
        Prepared::AskQuantity {
            title: "Selling Scale".into(),
            prompt: "Sell quantity (Currently has: 5)".into()
        }
    );
    assert_eq!(
        grabber::prepare(GrabAction::SellAll, &[&scale]),
        Prepared::Run
    );
    assert!(
        matches!(grabber::prepare(GrabAction::Sell, &[&class]), Prepared::Warn { message, .. } if message == "Cannot sell equipped item.")
    );
    assert!(
        matches!(grabber::prepare(GrabAction::SellAll, &[&scale, &class]), Prepared::Warn { progress, .. } if progress == "Warning")
    );
    let q = Grabbed::Quest(Default::default());
    assert_eq!(
        grabber::prepare(GrabAction::FakeComplete, &[&q, &q]),
        Prepared::Report("Please select exactly one quest to complete.".into())
    );
    assert_eq!(
        grabber::prepare(GrabAction::FakeComplete, &[&q]),
        Prepared::Run
    );
}

#[test]
fn buy_without_enough_gold_shows_vibeskuas_message() {
    let (bot, _) = setup("buy");
    let blade = Grabbed::Shop(
        serde_json::from_value(json!({ "ItemID": 7, "sName": "Blade", "iCost": 600, "iStk": 5 }))
            .unwrap(),
    );
    let messages = Mutex::new(Vec::new());
    let progress = |m: String| messages.lock().push(m);
    let notice = grabber::run(
        &bot,
        GrabAction::Buy,
        &[&blade],
        Some(3),
        &progress,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        notice,
        Some((
            "Not enough gold".into(),
            "Not enough gold to buy 3 Blade.\nTotal: 1,800\nNeeded: 800".into()
        ))
    );
    assert_eq!(*messages.lock(), ["Not enough gold. Total: 1,800"]);
}

#[test]
fn fake_complete_sends_the_update_quest_packet() {
    let (bot, game) = setup("fake-complete");
    game.tree.lock().insert("12".into(), raw_quest(12));
    let grabbed = grabber::grab(&bot, GrabKind::Quests, &no_scan).unwrap();
    assert_eq!(grabbed[0].summary(), "[12] Quest 12 (Slot: 3)");
    let messages = Mutex::new(Vec::new());
    let progress = |m: String| messages.lock().push(m);
    grabber::run(
        &bot,
        GrabAction::FakeComplete,
        &[&grabbed[0]],
        None,
        &progress,
        &AtomicBool::new(false),
    )
    .unwrap();
    let packets = game.client_packets.lock();
    assert_eq!(packets.len(), 1);
    assert!(
        packets[0].contains(r#""cmd":"updateQuest","iValue":12,"iIndex":3"#),
        "{}",
        packets[0]
    );
    assert_eq!(
        *messages.lock(),
        ["Fake completing Quest 12...", "Fake completed Quest 12."]
    );
}

#[test]
fn register_and_unregister_all_change_the_registered_quests() {
    let (bot, game) = setup("register");
    game.tree.lock().insert("12".into(), raw_quest(12));
    let grabbed = grabber::grab(&bot, GrabKind::Quests, &no_scan).unwrap();
    let cancel = AtomicBool::new(false);
    grabber::run(
        &bot,
        GrabAction::Register,
        &[&grabbed[0]],
        None,
        &|_| {},
        &cancel,
    )
    .unwrap();
    assert_eq!(bot.quests.registered(), [12]);
    grabber::run(&bot, GrabAction::UnregisterAll, &[], None, &|_| {}, &cancel).unwrap();
    assert!(bot.quests.registered().is_empty());
}

/// The community cache as a fake source: answers `Unchanged` for `etag`.
struct FakeSource {
    body: String,
    etag: &'static str,
    asked_with: Mutex<Vec<Option<String>>>,
}

impl quest_cache::QuestSource for FakeSource {
    fn fetch(&self, etag: Option<&str>) -> Result<quest_cache::Fetched, String> {
        self.asked_with.lock().push(etag.map(str::to_owned));
        if etag == Some(self.etag) {
            return Ok(quest_cache::Fetched::Unchanged);
        }
        Ok(quest_cache::Fetched::File {
            body: self.body.clone().into_bytes(),
            etag: Some(self.etag.to_owned()),
        })
    }
}

#[test]
fn download_merges_the_community_cache_and_remembers_its_etag() {
    let (bot, _) = setup("download");
    let path = cache_path(&bot, "download");
    quest_cache::write(&path, &[quest(1, "local one"), quest(9000, "only local")]).unwrap();
    let source = FakeSource {
        body: serde_json::to_string(&vec![quest(1, "remote one"), quest(2, "remote two")]).unwrap(),
        etag: "\"v1\"",
        asked_with: Mutex::new(Vec::new()),
    };
    // No ETag yet: a full download; the remote copy wins, the local-only quest stays.
    let quests = match quest_cache::download(&path, &source).unwrap() {
        quest_cache::Downloaded::Updated {
            quests,
            downloaded,
            kept,
        } => {
            assert_eq!((downloaded, kept), (2, 1));
            quests
        }
        other => panic!("{other:?}"),
    };
    let names: Vec<&str> = quests.iter().map(|q| q.name.as_str()).collect();
    assert_eq!(names, ["remote one", "remote two", "only local"]);
    assert_eq!(quest_cache::read(&path).unwrap(), quests);
    // Again: the ETag is sent and nothing changes.
    assert_eq!(
        quest_cache::download(&path, &source).unwrap(),
        quest_cache::Downloaded::Unchanged
    );
    assert_eq!(*source.asked_with.lock(), [None, Some("\"v1\"".to_owned())]);
    // Without the cache file, the ETag isn't sent: the file is downloaded again.
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        quest_cache::download(&path, &source).unwrap(),
        quest_cache::Downloaded::Updated { kept: 0, .. }
    ));
    assert_eq!(source.asked_with.lock()[2], None);
}

#[test]
fn download_rejects_a_broken_file_and_keeps_the_cache() {
    let (bot, _) = setup("download-broken");
    let path = cache_path(&bot, "download-broken");
    quest_cache::write(&path, &[quest(1, "local")]).unwrap();
    let source = FakeSource {
        body: "<html>rate limited</html>".into(),
        etag: "\"x\"",
        asked_with: Mutex::new(Vec::new()),
    };
    let error = quest_cache::download(&path, &source).unwrap_err();
    assert!(
        error.starts_with("The downloaded quest cache isn't valid"),
        "{error}"
    );
    assert_eq!(quest_cache::read(&path).unwrap(), [quest(1, "local")]);
}

/// Serves `body` gzipped with an ETag, and 304 to a request that sends it back.
fn serve_once_per_request(
    body: &str,
    requests: usize,
) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/QuestData.json", listener.local_addr().unwrap());
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(body.as_bytes()).unwrap();
    let gzipped = gz.finish().unwrap();
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_lowercase();
            if request.contains("if-none-match: \"abc\"") {
                stream.write_all(b"HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            } else {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nETag: \"abc\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    gzipped.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(&gzipped).unwrap();
            }
            seen.push(request);
        }
        seen
    });
    (url, handle)
}

#[test]
fn http_source_gets_gzip_and_honours_the_etag() {
    let (bot, _) = setup("download-http");
    let path = cache_path(&bot, "download-http");
    let body = serde_json::to_string(&vec![quest(5, "five")]).unwrap();
    let (url, server) = serve_once_per_request(&body, 2);
    let source = quest_cache::HttpSource::new(&url);
    assert!(matches!(
        quest_cache::download(&path, &source).unwrap(),
        quest_cache::Downloaded::Updated { downloaded: 1, .. }
    ));
    assert_eq!(
        quest_cache::download(&path, &source).unwrap(),
        quest_cache::Downloaded::Unchanged
    );
    let requests = server.join().unwrap();
    assert!(
        requests[0].contains("accept-encoding: gzip"),
        "{}",
        requests[0]
    );
    assert!(
        requests[1].contains("if-none-match: \"abc\""),
        "{}",
        requests[1]
    );
    assert_eq!(quest_cache::read(&path).unwrap(), [quest(5, "five")]);
}

/// A scanner for tests that don't grab map items.
fn no_scan(_: &[u8]) -> Result<Vec<(i64, i64)>, String> {
    Err("no scanner".into())
}

/// Serves `body` for `requests` requests; returns the base URL and the paths asked for.
fn serve_bytes(
    body: &'static [u8],
    requests: usize,
) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/maps/", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut paths = Vec::new();
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).into_owned();
            paths.push(
                request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned(),
            );
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
        }
        paths
    });
    (url, handle)
}

#[test]
fn finds_map_items_in_the_downloaded_map_and_keeps_them() {
    let (bot, _) = setup("map-items");
    let _ = std::fs::remove_file(bot.paths.map_items_file());
    let scan = |swf: &[u8]| -> Result<Vec<(i64, i64)>, String> {
        assert_eq!(swf, b"FWS map");
        Ok(vec![(1465, 2365), (42, 0)])
    };
    // Without a map there is nothing to download.
    let error =
        turu_bot::map_items::find_at(&bot, "http://127.0.0.1:9/", &scan, false).unwrap_err();
    assert!(error.to_string().contains("Join a map first"), "{error}");

    {
        let mut state = bot.state.lock();
        state.map_file_path = Some("ChiralValley/town-Mobius-21Feb14.swf".into());
        state.map_name = Some("mobius".into());
    }
    // One request: the second find reads the saved result.
    let (url, server) = serve_bytes(b"FWS map", 1);
    let items = turu_bot::map_items::find_at(&bot, &url, &scan, false).unwrap();
    let again = turu_bot::map_items::find_at(&bot, &url, &scan, false).unwrap();
    assert_eq!(
        server.join().unwrap(),
        ["/maps/ChiralValley/town-Mobius-21Feb14.swf"]
    );
    assert_eq!(items, again);
    let found: Vec<(i64, i64)> = items.iter().map(|i| (i.id, i.quest_id)).collect();
    assert_eq!(found, [(42, 0), (1465, 2365)]);
    assert_eq!(items[0].map_name, "mobius");
    assert_eq!(
        items[0].map_file_path,
        "ChiralValley/town-Mobius-21Feb14.swf"
    );

    // The rows, and Open / Accept use the items' quests (none for an item without one).
    let rows: Vec<Grabbed> = items.into_iter().map(Grabbed::MapItem).collect();
    assert_eq!(rows[1].summary(), "[1465] Map Item (Quest: 2365)");
    assert_eq!(rows[1].row(), "Map Item (Quest: 2365)");
    let messages = Mutex::new(Vec::new());
    let progress = |m: String| messages.lock().push(m);
    let all: Vec<&Grabbed> = rows.iter().collect();
    grabber::run(
        &bot,
        GrabAction::Open,
        &all,
        None,
        &progress,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(messages.lock()[0], "Showing 1 quests...");
    assert_eq!(
        grabber::prepare(GrabAction::GetMapItem, &[&rows[1]]),
        Prepared::AskQuantity {
            title: "Getting 1465".into(),
            prompt: "Quantity:".into()
        }
    );
}
