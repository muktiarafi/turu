//! Tools & Helpers > Junk Items: the `JunkItems.json` list and selling it against a fake game.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, Weak};
use turu_bot::junk::{self, Entry, JunkItem};
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
    favourites: Vec<i64>,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let arg = |i: usize| match args.get(i) {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        let answer = match (name, arg(0).as_str()) {
            ("isLoggedIn", _) => "true".into(),
            ("isFavoriteItem", id) => self.favourites.iter().any(|f| f.to_string() == id).to_string(),
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => "1".into(),
            ("getGameObject", "world.curRoom") => "42".into(),
            ("getGameObject", "world.lock.sellItem") => json!({ "ts": 0, "cd": 0 }).to_string(),
            ("getGameObject", "world.myAvatar.items") => Value::Array(self.inventory.lock().clone()).to_string(),
            ("getGameObject", "world.bankinfo.items") => json!([
                { "ItemID": 4, "CharItemID": 400, "sName": "Banked Junk", "iQty": 1, "sType": "Item" },
                { "ItemID": 1, "CharItemID": 100, "sName": "Dragon Scale", "iQty": 5, "sType": "Item" }
            ])
            .to_string(),
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
    game.bot.set(Arc::downgrade(&bot)).unwrap();
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
    let entry = |id: i64, name: &str, category: &str, junk: bool| Entry {
        id,
        name: name.into(),
        category: category.into(),
        junk,
    };
    // By name, the first of each ID, `sType` as VibeSkua's `ItemCategory` name.
    assert_eq!(
        entries,
        [
            entry(4, "Banked Junk", "Item", true),
            entry(1, "Dragon Scale", "Item", false),
            entry(2, "Junk Sword", "Sword", false),
            entry(5, "Keeper", "Item", false),
            // C#'s `Enum.TryParse("Quest Item")` fails.
            entry(3, "Old Note", "Unknown", false),
        ]
    );
    // The bank was loaded already: no `loadBank`.
    assert!(game.sent.lock().is_empty());
}
