//! Tools & Helpers > Junk Items: the `JunkItems.json` list and selling it against a fake game.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, Weak};
use turu_bot::junk::{self, AcFilter, Entry, Filter, Highlight, JunkItem};
use turu_bot::{Bot, FlashArg, FlashHost, HostError, api};

fn junk_item(id: i64, name: &str, category: &str) -> JunkItem {
    JunkItem {
        id,
        name: name.into(),
        category: category.into(),
        meta: String::new(),
    }
}

fn temp_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("turu-test-junk-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join(junk::FILE_NAME)
}

#[test]
fn reads_and_writes_vibeskua_files() {
    let path = temp_file("format");
    // Missing, blank and broken files are an empty list.
    assert!(junk::load(&path).is_empty());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "  \n").unwrap();
    assert!(junk::load(&path).is_empty());
    std::fs::write(&path, "{ not json").unwrap();
    assert!(junk::load(&path).is_empty());

    // VibeSkua's file (auqw/Scripts `JunkItems.json`).
    let vibeskua = r#"[
  {
    "ID": 76322,
    "Name": "A Fish",
    "Category": "Item",
    "Meta": ""
  },
  {
    "ID": 102776,
    "Name": "A Warning",
    "Category": "Note",
    "Meta": ""
  }
]"#;
    std::fs::write(&path, vibeskua).unwrap();
    let items = junk::load(&path);
    assert_eq!(
        items,
        [
            junk_item(76322, "A Fish", "Item"),
            junk_item(102776, "A Warning", "Note")
        ]
    );
    // Written back byte for byte (System.Text.Json's indented output).
    junk::save(&path, &items).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), vibeskua);

    // `SetJunk` keeps the first of each ID.
    junk::set(
        &path,
        [
            junk_item(1, "A", "Item"),
            junk_item(2, "B", "Resource"),
            junk_item(1, "A again", "Item"),
        ],
    )
    .unwrap();
    assert_eq!(
        junk::load(&path),
        [junk_item(1, "A", "Item"), junk_item(2, "B", "Resource")]
    );
}

/// A logged-in player with an inventory and a bank; a `sellItem` packet sells the item.
#[derive(Default)]
struct FakeGame {
    bot: OnceLock<Weak<Bot>>,
    sent: Mutex<Vec<String>>,
    inventory: Mutex<Vec<Value>>,
    bank: Mutex<Vec<Value>>,
    favourites: Vec<i64>,
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
        let answer = match (name, arg(0).as_str()) {
            ("isLoggedIn", _) => "true".into(),
            ("isFavoriteItem", id) => self
                .favourites
                .iter()
                .any(|f| f.to_string() == id)
                .to_string(),
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => "1".into(),
            ("getGameObject", "world.curRoom") => "42".into(),
            ("getGameObject", "world.lock.sellItem") => json!({ "ts": 0, "cd": 0 }).to_string(),
            ("getGameObject", "world.myAvatar.items") => {
                Value::Array(self.inventory.lock().clone()).to_string()
            }
            ("getGameObject", "world.bankinfo.items") => {
                Value::Array(self.bank.lock().clone()).to_string()
            }
            ("callGameFunction", "sfc.sendString") => {
                let packet = arg(1);
                // %xt%zm%sellItem%room%id%qty%charItemId%
                if let Some(id) = packet.strip_prefix("%xt%zm%sellItem%42%") {
                    let id: i64 = id.split('%').next().unwrap().parse().unwrap();
                    self.inventory.lock().retain(|i| i["ItemID"] != id);
                    if let Some(bot) = self.bot.get().and_then(Weak::upgrade) {
                        bot.state.lock().signals.item_sold = true;
                    }
                }
                self.sent.lock().push(packet);
                "null".into()
            }
            _ => "null".into(),
        };
        Ok(answer)
    }
}

fn setup(favourites: Vec<i64>) -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test in this file sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-junk")) };
    let bot = Bot::new();
    let game = Arc::new(FakeGame {
        favourites,
        ..Default::default()
    });
    *game.inventory.lock() = vec![
        json!({ "ItemID": 1, "CharItemID": 100, "sName": "Dragon Scale", "iQty": 5, "sType": "Item" }),
        json!({ "ItemID": 2, "CharItemID": 200, "sName": "Junk Sword", "iQty": 1, "sType": "Sword", "bEquip": "1" }),
        json!({ "ItemID": 3, "CharItemID": 300, "sName": "Old Note", "iQty": 3, "sType": "Quest Item" }),
        json!({ "ItemID": 5, "CharItemID": 500, "sName": "Keeper", "iQty": 1, "sType": "Item" }),
    ];
    *game.bank.lock() = vec![
        json!({ "ItemID": 4, "CharItemID": 400, "sName": "Banked Junk", "iQty": 1, "sType": "Item" }),
        json!({ "ItemID": 1, "CharItemID": 100, "sName": "Dragon Scale", "iQty": 5, "sType": "Item" }),
    ];
    game.bot.set(Arc::downgrade(&bot)).unwrap();
    // Goals another test saved don't count.
    bot.settings.write().inventory_goals.clear();
    bot.set_host(Some(game.clone()));
    (bot, game)
}

/// Tests share the data folder, so the ones that use the list take turns.
static LIST: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn sells_unequipped_inventory_junk() {
    let _turn = LIST.lock().unwrap_or_else(|e| e.into_inner());
    let (bot, game) = setup(vec![5]);
    let path = bot.paths.junk_items_file();
    // Nothing on the list: nothing sold.
    junk::save(&path, &[]).unwrap();
    assert_eq!(junk::sell_all(&bot).unwrap(), 0);

    junk::save(
        &path,
        &[
            junk_item(2, "Junk Sword", "Sword"),
            junk_item(3, "Old Note", "QuestItem"),
            junk_item(4, "Banked Junk", "Item"),
            junk_item(5, "Keeper", "Item"),
        ],
    )
    .unwrap();
    // Through the script API (`bot.sellJunk()`).
    api::dispatch(&bot, "sellJunk", &[]).unwrap();
    // The equipped sword and the banked item aren't sold; the favourite is skipped.
    assert_eq!(*game.sent.lock(), ["%xt%zm%sellItem%42%3%3%300%"]);
    let left: Vec<i64> = game
        .inventory
        .lock()
        .iter()
        .map(|i| i["ItemID"].as_i64().unwrap())
        .collect();
    assert_eq!(left, [1, 2, 5]);
}

#[test]
fn lists_inventory_and_bank_items() {
    let _turn = LIST.lock().unwrap_or_else(|e| e.into_inner());
    let (bot, game) = setup(Vec::new());
    junk::save(
        &bot.paths.junk_items_file(),
        &[junk_item(4, "Banked Junk", "Item")],
    )
    .unwrap();
    bot.state.lock().bank_loaded = true;
    let entries = junk::entries(&bot).unwrap();
    let entry =
        |id: i64, name: &str, category: &str, junk: bool, inventory: i64, bank: i64| Entry {
            id,
            name: name.into(),
            category: category.into(),
            junk,
            inventory_quantity: inventory,
            bank_quantity: bank,
            ..Default::default()
        };
    // By name, one per ID, `sType` as VibeSkua's `ItemCategory` name, quantities from both.
    assert_eq!(
        entries,
        [
            entry(4, "Banked Junk", "Item", true, 0, 1),
            entry(1, "Dragon Scale", "Item", false, 5, 5),
            Entry {
                equipped: true,
                ..entry(2, "Junk Sword", "Sword", false, 1, 0)
            },
            entry(5, "Keeper", "Item", false, 1, 0),
            // C#'s `Enum.TryParse("Quest Item")` fails.
            entry(3, "Old Note", "Unknown", false, 3, 0),
        ]
    );
    // The bank was loaded already: no `loadBank`.
    assert!(game.sent.lock().is_empty());
}

#[test]
fn reads_equipped_favourites_and_stacks() {
    let _turn = LIST.lock().unwrap_or_else(|e| e.into_inner());
    let (bot, game) = setup(vec![1, 6]);
    junk::save(&bot.paths.junk_items_file(), &[]).unwrap();
    bot.state.lock().bank_loaded = true;
    game.inventory.lock().extend([
        json!({ "ItemID": 6, "CharItemID": 600, "sName": "Arrows", "iQty": 7, "iStk": 10, "sType": "Item", "bCoins": 1 }),
        json!({ "ItemID": 6, "CharItemID": 601, "sName": "Arrows", "iQty": 9, "iStk": 10, "sType": "Item", "bEquip": 1 }),
        json!({ "ItemID": 7, "CharItemID": 700, "sName": "Gems", "iQty": 7, "iStk": 10, "sType": "Item" }),
        json!({ "ItemID": 7, "CharItemID": 701, "sName": "Gems", "iQty": 7, "iStk": 10, "sType": "Item" }),
        json!({ "ItemID": 8, "CharItemID": 800, "sName": "Ore", "iQty": 8, "iStk": 10, "sType": "Item" }),
        json!({ "ItemID": 9, "CharItemID": 900, "sName": "Stone", "iQty": 8, "iStk": 0, "sType": "Item" }),
    ]);
    game.bank.lock().push(
        json!({ "ItemID": 6, "CharItemID": 602, "sName": "Arrows", "iQty": 2, "iStk": 20, "sType": "Item" }),
    );
    let entries = junk::entries(&bot).unwrap();
    let find = |id: i64| entries.iter().find(|e| e.id == id).unwrap().clone();

    // Two stacks, 7/10 and 9/10, the second one equipped: near cap and equipped come from
    // either stack, the quantities add up, the bank's bigger stack size is the max.
    let arrows = find(6);
    assert!(arrows.equipped && arrows.near_stack_limit && arrows.favorite && arrows.ac);
    assert_eq!(
        (
            arrows.inventory_quantity,
            arrows.bank_quantity,
            arrows.max_stack
        ),
        (16, 2, 20)
    );
    // 7/10 and 7/10: 14 of 20 together, but neither stack is 80% full.
    let gems = find(7);
    assert!(!gems.near_stack_limit && !gems.equipped && !gems.favorite && !gems.ac);
    assert_eq!((gems.inventory_quantity, gems.max_stack), (14, 10));
    // 8 of 10 is exactly 80%; a stack size of 0 is never near it.
    assert!(find(8).near_stack_limit);
    assert!(!find(9).near_stack_limit);
    // Favourites are asked from the game (`isFavoriteItem`).
    assert!(find(1).favorite);
    assert!(!find(5).favorite);
}

#[test]
fn saves_quantity_goals() {
    let _turn = LIST.lock().unwrap_or_else(|e| e.into_inner());
    let (bot, _game) = setup(Vec::new());
    junk::save(&bot.paths.junk_items_file(), &[]).unwrap();
    bot.state.lock().bank_loaded = true;

    // Set goal: a whole number above 0.
    assert_eq!(junk::parse_goal(" 2000 "), Some(2000));
    assert_eq!(junk::parse_goal("0"), None);
    assert_eq!(junk::parse_goal("-3"), None);
    assert_eq!(junk::parse_goal("1.5"), None);
    assert_eq!(junk::parse_goal(""), None);

    junk::set_goals(&bot, &[1, 3], Some(2000));
    // Saved as `inventoryGoals` in settings.json.
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(bot.paths.settings_file()).unwrap()).unwrap();
    assert_eq!(saved["inventoryGoals"], json!({ "1": 2000, "3": 2000 }));
    let entries = junk::entries(&bot).unwrap();
    let scale = entries.iter().find(|e| e.id == 1).unwrap();
    assert_eq!(scale.goal, 2000);
    // Inventory and bank together.
    assert_eq!(scale.goal_progress(), "10 / 2,000");
    assert_eq!(scale.quantity_display(), "Inv: 5  |  Bank: 5");

    junk::set_goals(&bot, &[1], Some(4));
    junk::set_goals(&bot, &[3], None);
    let entries = junk::entries(&bot).unwrap();
    let scale = entries.iter().find(|e| e.id == 1).unwrap();
    // At most the goal.
    assert_eq!(scale.goal_progress(), "4 / 4");
    assert!(!entries.iter().find(|e| e.id == 3).unwrap().goal_tracked());
    junk::set_goals(&bot, &[1], None);
    assert!(bot.settings.read().inventory_goals.is_empty());
}

#[test]
fn filters_by_search_category_ac_and_highlight() {
    let item = |id: i64, name: &str, category: &str| Entry {
        id,
        name: name.into(),
        category: category.into(),
        ..Default::default()
    };
    let entries = [
        Entry {
            ac: true,
            equipped: true,
            ..item(10, "Blade of Awe", "Sword")
        },
        Entry {
            favorite: true,
            junk: true,
            ..item(21, "Rusty Axe", "axe")
        },
        Entry {
            near_stack_limit: true,
            goal: 5,
            ..item(32, "Tokens", "Item")
        },
        item(43, "Pebble", "Item"),
    ];
    // "All Categories", then each once, ignoring case.
    assert_eq!(
        junk::categories(&entries),
        ["All Categories", "axe", "Item", "Sword"]
    );

    let shown = |filter: &Filter| -> Vec<i64> {
        entries
            .iter()
            .filter(|e| filter.matches(e))
            .map(|e| e.id)
            .collect()
    };
    assert_eq!(shown(&Filter::default()), [10, 21, 32, 43]);
    // Name, ID or category, ignoring case.
    let search = |text: &str| Filter {
        search: text.into(),
        ..Filter::default()
    };
    assert_eq!(shown(&search(" AWE ")), [10]);
    assert_eq!(shown(&search("3")), [32, 43]);
    assert_eq!(shown(&search("sWoRd")), [10]);
    let category = Filter {
        category: "ITEM".into(),
        ..Filter::default()
    };
    assert_eq!(shown(&category), [32, 43]);
    let ac = |ac| Filter {
        ac,
        ..Filter::default()
    };
    assert_eq!(shown(&ac(AcFilter::Ac)), [10]);
    assert_eq!(shown(&ac(AcFilter::NonAc)), [21, 32, 43]);
    let highlight = |highlight| Filter {
        highlight,
        ..Filter::default()
    };
    assert_eq!(shown(&highlight(Highlight::Goal)), [32]);
    assert_eq!(shown(&highlight(Highlight::NearStackLimit)), [32]);
    assert_eq!(shown(&highlight(Highlight::Favorites)), [21]);
    assert_eq!(shown(&highlight(Highlight::Equipped)), [10]);
    assert_eq!(shown(&highlight(Highlight::Junk)), [21]);
    // The filters add up.
    let both = Filter {
        search: "peb".into(),
        category: "Item".into(),
        highlight: Highlight::Goal,
        ..Filter::default()
    };
    assert!(shown(&both).is_empty());
    // VibeSkua's option labels.
    assert_eq!(
        AcFilter::ALL.map(AcFilter::label),
        ["All Items", "AC Items", "Non-AC Items"]
    );
    assert_eq!(
        Highlight::ALL.map(Highlight::label),
        [
            "All Highlights",
            "Goal Items",
            "Near Stack Limit",
            "Favorites",
            "Equipped",
            "Junk"
        ]
    );
}
