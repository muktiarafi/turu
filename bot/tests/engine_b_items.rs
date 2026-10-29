//! Item matching, quest/shop JSON parsing and the item APIs against a fake game.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use turu_bot::api::inventory::{contains_in, contains_many_in, find_item, quantity_in};
use turu_bot::api::{self, NameOrId};
use turu_bot::models::{ItemBase, Quest, ShopItem, Skill};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

fn item(id: i64, name: &str, qty: i64, category: &str) -> ItemBase {
    serde_json::from_value(
        json!({ "ItemID": id, "sName": name, "iQty": qty, "iStk": 10, "sType": category }),
    )
    .unwrap()
}

#[test]
fn contains_follows_vibeskua() {
    let items = vec![
        item(1, " Dragon Scale ", 5, "Item"),
        item(2, "Void Highlord", 1, "Class"),
    ];
    assert!(contains_in(&items, NameOrId::Name("dragon scale"), 5));
    assert!(!contains_in(&items, NameOrId::Name("Dragon Scale"), 6));
    assert!(contains_in(&items, NameOrId::Id(1), 1));
    // A class counts at any quantity; 0 is always true.
    assert!(contains_in(&items, NameOrId::Name("void highlord"), 99));
    assert!(contains_in(&items, NameOrId::Name("missing"), 0));
    assert!(!contains_in(&items, NameOrId::Id(3), 1));
    assert_eq!(quantity_in(&items, NameOrId::Name("DRAGON SCALE")), 5);
    assert_eq!(quantity_in(&items, NameOrId::Id(3)), 0);
    assert_eq!(
        find_item(&items, NameOrId::Id(2)).unwrap().name,
        "Void Highlord"
    );
}

#[test]
fn contains_many_needs_all_or_any() {
    let items = vec![item(1, "A", 5, "Item"), item(2, "B", 1, "Item")];
    let ab = [NameOrId::Name("A"), NameOrId::Name("B")];
    let ac = [NameOrId::Name("A"), NameOrId::Name("C")];
    assert!(contains_many_in(&items, &ab, 1, true));
    assert!(!contains_many_in(&items, &ab, 2, true));
    assert!(contains_many_in(&items, &ac, 1, false));
    assert!(!contains_many_in(&items, &ac, 1, true));
    assert!(contains_many_in(&items, &[], 1, true));
    assert!(!contains_many_in(&items, &[], 1, false));
}

const QUEST_TREE: &str = r#"{
  "1234": {
    "QuestID": "1234", "sName": "Dragon Hunt ", "iSlot": 12, "iValue": "3", "bOnce": "0", "bUpg": "1",
    "iLvl": 30, "sField": "", "iIndex": 0, "status": "c", "FactionID": 5, "iReqRep": 100,
    "oItems": { "11": { "ItemID": "11", "sName": "Dragon Scale", "iQty": 1, "bTemp": "0" },
                "12": { "sName": "Temp Thing", "iQty": 1, "bTemp": "1" } },
    "turnin": [ { "ItemID": 11, "iQty": 5 }, { "ItemID": 12, "iQty": 2 } ],
    "oReqd": { "20": { "ItemID": 20, "sName": "Key", "iQty": 1 } },
    "oRewards": { "itemsS": { "30": { "ItemID": 30, "sName": "Sword", "iQty": 1 } },
                  "itemsR": { "31": { "ItemID": 31, "sName": "Rare", "iQty": 1 } } },
    "reward": [ { "ItemID": 30, "iQty": 1, "iRate": 100, "iType": 1 } ]
  },
  "5": { "QuestID": 5, "sName": "Not Started", "oItems": [], "oReqd": [] },
  "length": 2
}"#;

#[test]
fn parses_the_quest_tree() {
    let tree = Quest::tree_from(serde_json::from_str(QUEST_TREE).unwrap());
    assert_eq!(tree.len(), 2);
    let q = tree.iter().find(|q| q.id == 1234).unwrap();
    assert_eq!(q.name, "Dragon Hunt");
    assert!(q.completable() && q.active() && q.upgrade && !q.once);
    assert_eq!((q.slot, q.value, q.level), (12, 3, 30));
    let mut reqs: Vec<_> = q
        .requirements
        .iter()
        .map(|r| (r.id, r.quantity, r.temp))
        .collect();
    reqs.sort();
    assert_eq!(reqs, [(11, 5, false), (12, 2, true)]);
    assert_eq!(q.accept_requirements[0].name, "Key");
    assert_eq!(q.rewards.len(), 2);
    assert_eq!(q.simple_rewards[0].id, 30);
    assert_eq!((q.required_faction_id, q.required_faction_rep), (5, 100));

    let json = serde_json::to_value(q).unwrap();
    for key in [
        "id",
        "name",
        "status",
        "once",
        "upgrade",
        "level",
        "requirements",
        "acceptRequirements",
        "rewards",
        "simpleRewards",
        "slot",
        "value",
        "field",
        "index",
    ] {
        assert!(json.get(key).is_some(), "missing {key}: {json}");
    }
    assert!(json.get("requiredFactionId").is_none());
    let other = tree.iter().find(|q| q.id == 5).unwrap();
    assert!(!other.active());
    assert_eq!(serde_json::to_value(other).unwrap()["status"], Value::Null);
}

#[test]
fn parses_shop_items() {
    let item: ShopItem = serde_json::from_value(json!({
        "ItemID": 7, "ShopItemID": "99", "sName": "Blade", "iCost": 500, "iQty": 1, "iStk": 1,
        "bCoins": 1, "sType": "Sword", "sFaction": "Good", "iReqRep": 302500,
        "turnin": [ { "ItemID": 11, "sName": "Dragon Scale", "iQty": 3 } ]
    }))
    .unwrap();
    assert_eq!((item.item.id, item.shop_item_id, item.cost), (7, 99, 500));
    assert!(item.item.coins);
    assert_eq!(item.requirements, "Dragon Scale x3, Good reputation 302500");
    let json = serde_json::to_value(&item).unwrap();
    assert_eq!(json["id"], 7);
    assert_eq!(json["shopItemId"], 99);
    assert_eq!(json["name"], "Blade");
    assert!(json.get("turnin").is_none());
}

#[test]
fn parses_the_action_bar() {
    let skills = Skill::list_from(json!([
        { "nam": "Auto Attack", "mp": 0, "cd": 2000, "isOK": true, "auto": true, "ts": 5 },
        null,
        { "nam": "Fireball", "mp": "15", "cd": 3000.0, "isOK": false }
    ]));
    assert_eq!(skills.len(), 3);
    assert_eq!(skills[2].index, 2);
    assert_eq!(skills[2].mana_cost, 15);
    let json = serde_json::to_value(&skills[0]).unwrap();
    assert_eq!(
        json,
        json!({ "index": 0, "name": "Auto Attack", "manaCost": 0, "cooldown": 2000, "isOk": true })
    );
}

/// A logged-in player with an inventory, and a packet log.
struct FakeGame {
    sent: Mutex<Vec<String>>,
    calls: Mutex<Vec<String>>,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let arg = |i: usize| match args.get(i) {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        self.calls.lock().push(format!("{name}({})", arg(0)));
        let answer = match (name, arg(0).as_str()) {
            ("isLoggedIn", _) | ("inventoryHasSpaceFor", _) | ("rejectExcept", _) => "true".into(),
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => "1".into(),
            ("getGameObject", "world.curRoom") => "42".into(),
            ("getGameObject", "world.myAvatar.items") => json!([
                { "ItemID": 1, "CharItemID": 100, "sName": "Dragon Scale", "iQty": 5, "iStk": 5, "sType": "Item" },
                { "ItemID": 2, "CharItemID": 200, "sName": "Void Highlord", "iQty": 1, "sType": "Class", "bEquip": "1" }
            ])
            .to_string(),
            ("getGameObject", "world.myAvatar.objData.iBagSlots") => "30".into(),
            ("inventoryBagUsedSlots", _) => "12".into(),
            ("getGameObject", "world.questTree") => QUEST_TREE.into(),
            ("callGameFunction", "sfc.sendString") => {
                self.sent.lock().push(arg(1));
                "null".into()
            }
            ("callGameFunction", "world.isQuestInProgress") => (arg(1) == "1234").to_string(),
            _ => "null".into(),
        };
        Ok(answer)
    }
}

fn setup() -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test in this file sets the same value.
    unsafe {
        std::env::set_var(
            "TURU_DATA_DIR",
            std::env::temp_dir().join("turu-test-engine-b"),
        )
    };
    let bot = Bot::new();
    let game = Arc::new(FakeGame {
        sent: Mutex::new(Vec::new()),
        calls: Mutex::new(Vec::new()),
    });
    bot.set_host(Some(game.clone()));
    (bot, game)
}

fn call(bot: &Arc<Bot>, method: &str, args: Value) -> Value {
    let Value::Array(args) = args else { panic!() };
    api::dispatch(bot, method, &args).unwrap_or_else(|e| panic!("{method}: {e}"))
}

#[test]
fn inventory_api_reads_the_game() {
    let (bot, _) = setup();
    assert_eq!(
        call(&bot, "inventory.contains", json!(["dragon scale", 5])),
        json!(true)
    );
    assert_eq!(
        call(&bot, "inventory.contains", json!([1, 6])),
        json!(false)
    );
    assert_eq!(
        call(&bot, "inventory.contains", json!(["void highlord", 100])),
        json!(true)
    );
    assert_eq!(
        call(
            &bot,
            "inventory.containsMany",
            json!([["Dragon Scale", 9], 1, false])
        ),
        json!(true)
    );
    assert_eq!(
        call(&bot, "inventory.getQuantity", json!(["Dragon Scale"])),
        json!(5)
    );
    assert_eq!(call(&bot, "inventory.isMaxStack", json!([1])), json!(true));
    assert_eq!(
        call(&bot, "inventory.isEquipped", json!(["Void Highlord"])),
        json!(true)
    );
    assert_eq!(
        call(&bot, "inventory.getItem", json!([2]))["category"],
        json!("Class")
    );
    assert_eq!(call(&bot, "inventory.getItem", json!([3])), Value::Null);
    assert_eq!(call(&bot, "inventory.freeSlots", json!([])), json!(18));
    assert_eq!(call(&bot, "tempInv.items", json!([])), json!([]));
    assert_eq!(
        call(&bot, "player.currentClass", json!([])),
        json!("Void Highlord")
    );
}

#[test]
fn drops_pickup_sends_get_drop() {
    let (bot, game) = setup();
    bot.state
        .lock()
        .add_drop(item(11, "Dragon Scale", 1, "Item"));
    bot.state.lock().add_drop(item(12, "Junk", 1, "Item"));
    assert_eq!(call(&bot, "drops.exists", json!(["*"])), json!(true));
    call(&bot, "drops.pickup", json!([["dragon scale", 999]]));
    assert_eq!(*game.sent.lock(), ["%xt%zm%getDrop%42%11%"]);
    assert_eq!(call(&bot, "drops.exists", json!([11])), json!(false));
    call(&bot, "drops.rejectExcept", json!([["Something"]]));
    assert!(
        game.calls
            .lock()
            .iter()
            .any(|c| c == "rejectExcept(something)")
    );
    assert_eq!(call(&bot, "drops.current", json!([])), json!([]));

    call(&bot, "drops.add", json!([["A", 5, "A"]]));
    assert_eq!(call(&bot, "drops.toPickup", json!([])), json!(["A"]));
    assert_eq!(call(&bot, "drops.toPickupIds", json!([])), json!([5]));
    call(&bot, "drops.remove", json!([[5]]));
    assert_eq!(call(&bot, "drops.toPickupIds", json!([])), json!([]));
}

#[test]
fn quests_read_the_tree() {
    let (bot, _) = setup();
    assert_eq!(call(&bot, "quests.canComplete", json!([1234])), json!(true));
    assert_eq!(call(&bot, "quests.canComplete", json!([5])), json!(false));
    assert_eq!(
        call(&bot, "quests.isInProgress", json!([1234])),
        json!(true)
    );
    assert_eq!(
        call(&bot, "quests.active", json!([]))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        call(&bot, "quests.get", json!([1234]))["name"],
        json!("Dragon Hunt")
    );
    assert_eq!(call(&bot, "quests.get", json!([9])), Value::Null);
}

#[test]
fn drop_loop_starts_and_stops() {
    let (bot, _) = setup();
    call(&bot, "drops.start", json!([]));
    assert_eq!(call(&bot, "drops.running", json!([])), json!(true));
    bot.drops.stop();
    assert_eq!(call(&bot, "drops.running", json!([])), json!(false));
}

#[test]
fn skill_timer_starts_pauses_and_stops() {
    let (bot, game) = setup();
    call(
        &bot,
        "skills.startSkills",
        json!(["1 | 2 H<50", 10, "UseIfAvailable"]),
    );
    assert_eq!(call(&bot, "skills.timerRunning", json!([])), json!(true));
    let commands = bot.skills.provider().unwrap().commands().len();
    assert_eq!(commands, 2);
    call(&bot, "skills.pause", json!([]));
    assert!(bot.skills.is_paused());
    call(&bot, "skills.resume", json!([]));
    call(&bot, "skills.stop", json!([]));
    assert_eq!(call(&bot, "skills.timerRunning", json!([])), json!(false));
    // The timer cancels the target on its way out.
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        game.calls
            .lock()
            .iter()
            .any(|c| c == "callGameFunction0(world.cancelTarget)")
    );
}
