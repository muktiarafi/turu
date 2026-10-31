//! Equipping from outside a script (`turu_bot::gear`) against a fake game: the inventory, the
//! bank (loaded on demand), the script-running rule, and the gear file.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};
use turu_bot::api::NameOrId;
use turu_bot::gear::{self, GearError, Place, Slot};
use turu_bot::{Bot, FlashArg, FlashHost, HostError, script};

struct FakeGame {
    inventory: Mutex<Vec<Value>>,
    bank: Mutex<Vec<Value>>,
    bag_full: bool,
    packets: Mutex<Vec<String>>,
    bot: OnceLock<Weak<Bot>>,
}

fn item(id: i64, name: &str, slot: &str, equipped: bool) -> Value {
    json!({ "ItemID": id, "CharItemID": id * 10, "sName": name, "sES": slot, "bEquip": equipped, "iQty": 1 })
}

impl FakeGame {
    fn new(inventory: Vec<Value>, bank: Vec<Value>) -> Self {
        Self {
            inventory: Mutex::new(inventory),
            bank: Mutex::new(bank),
            bag_full: false,
            packets: Mutex::default(),
            bot: OnceLock::new(),
        }
    }

    fn packet(&self, packet: &str) {
        self.packets.lock().push(packet.to_owned());
        let parts: Vec<&str> = packet.split('%').collect();
        match parts.get(3).copied() {
            Some("equipItem") => {
                let id: i64 = parts[5].parse().unwrap();
                let mut inventory = self.inventory.lock();
                let slot = inventory
                    .iter()
                    .find(|i| i["ItemID"] == id)
                    .map(|i| i["sES"].clone());
                for i in inventory.iter_mut() {
                    if i["ItemID"] == id {
                        i["bEquip"] = json!(true);
                    } else if Some(&i["sES"]) == slot.as_ref() {
                        i["bEquip"] = json!(false);
                    }
                }
            }
            Some("bankToInv") => {
                let id: i64 = parts[5].parse().unwrap();
                let mut bank = self.bank.lock();
                if let Some(at) = bank.iter().position(|i| i["ItemID"] == id) {
                    self.inventory.lock().push(bank.remove(at));
                }
            }
            // The game's answer sets this (`packets.rs`).
            Some("loadBank") => {
                if let Some(bot) = self.bot.get().and_then(Weak::upgrade) {
                    bot.state.lock().bank_loaded = true;
                }
            }
            _ => {}
        }
    }
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let first = match args.first() {
            Some(FlashArg::String(s)) => s.as_str(),
            _ => "",
        };
        Ok(match (name, first) {
            ("callGameFunction", "sfc.sendString") => {
                if let Some(FlashArg::String(packet)) = args.get(1) {
                    self.packet(packet);
                }
                "true".into()
            }
            ("callGameFunction", _) => "true".into(),
            ("inventoryHasSpaceFor", _) => (!self.bag_full).to_string(),
            ("isLoggedIn", _) => "true".into(),
            ("getGameObjectS", "loginInfo.strUsername") => "\"Alt\"".into(),
            ("getGameObject", "world.myAvatar.items") => {
                Value::from(self.inventory.lock().clone()).to_string()
            }
            ("getGameObject", "world.bankinfo.items") => {
                Value::from(self.bank.lock().clone()).to_string()
            }
            ("getGameObject", "world.curRoom") => "7".into(),
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => "1".into(),
            _ => "null".into(),
        })
    }
}

fn start(game: FakeGame) -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let game = Arc::new(game);
    let bot = Bot::new();
    game.bot.set(Arc::downgrade(&bot)).ok();
    bot.set_host(Some(game.clone()));
    (bot, game)
}

fn names(list: &[String]) -> Vec<&str> {
    list.iter().map(String::as_str).collect()
}

#[test]
fn equips_from_the_inventory_and_the_bank() {
    let (bot, game) = start(FakeGame::new(
        vec![
            item(1, "Legion Revenant", "ar", true),
            item(2, "Necrotic Sword", "Weapon", false),
            item(3, "Bone Dust", "", false),
        ],
        vec![item(4, "Hollowborn Reaper", "ba", false)],
    ));
    let report = gear::equip(
        &bot,
        &[
            NameOrId::Name("legion revenant"),
            NameOrId::Name("Necrotic Sword"),
            NameOrId::Id(4),
            NameOrId::Name("Nothing Like It"),
            NameOrId::Name("Bone Dust"),
        ],
        false,
    )
    .unwrap();
    assert_eq!(names(&report.already_worn), ["Legion Revenant"]);
    assert_eq!(
        names(&report.equipped),
        ["Necrotic Sword", "Hollowborn Reaper"]
    );
    assert_eq!(names(&report.from_bank), ["Hollowborn Reaper"]);
    assert_eq!(names(&report.missing), ["Nothing Like It"]);
    assert_eq!(report.failed.len(), 1, "{report:?}");
    assert_eq!(report.failed[0].item, "Bone Dust");
    assert_eq!(report.stopped_script, None);

    // The bank was loaded once (for item 4), not again for the missing item.
    let packets = game.packets.lock().clone();
    assert_eq!(
        packets.iter().filter(|p| p.contains("%loadBank%")).count(),
        1,
        "{packets:?}"
    );
    assert!(
        packets.contains(&"%xt%zm%bankToInv%7%4%40%".to_owned()),
        "{packets:?}"
    );

    // The gear file shows it.
    let file = gear::refresh(&bot).unwrap().unwrap();
    assert_eq!(file.username, "Alt");
    assert!(file.bank_as_of.is_some());
    assert_eq!(
        file.worn(Slot::Cape).map(|i| i.name.as_str()),
        Some("Hollowborn Reaper")
    );
    assert_eq!(
        file.worn(Slot::Weapon).map(|i| i.name.as_str()),
        Some("Necrotic Sword")
    );
    assert_eq!(gear::read(&bot.paths.gear_dir(), "alt"), Some(file));
}

#[test]
fn a_full_bag_leaves_the_item_in_the_bank() {
    let mut game = FakeGame::new(vec![], vec![item(4, "Hollowborn Reaper", "ba", false)]);
    game.bag_full = true;
    let (bot, game) = start(game);
    bot.state.lock().bank_loaded = true;
    let report = gear::equip(&bot, &[NameOrId::Name("Hollowborn Reaper")], false).unwrap();
    assert!(report.equipped.is_empty());
    assert_eq!(report.failed[0].reason, "in the bank, and the bag is full");
    // Loaded already: not again.
    assert!(!game.packets.lock().iter().any(|p| p.contains("%loadBank%")));
    let file = gear::refresh(&bot).unwrap().unwrap();
    assert_eq!(file.for_slot(Slot::Cape)[0].place, Place::Bank);
}

#[test]
fn refuses_while_a_script_runs_unless_asked_to_stop_it() {
    let (bot, _game) = start(FakeGame::new(
        vec![item(2, "Necrotic Sword", "Weapon", false)],
        vec![],
    ));
    let dir = std::env::temp_dir().join(format!("turu-test-gear-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Farm.ts");
    std::fs::write(
        &path,
        "import { bot } from \"turu\";\nexport default function main(): void { while (true) bot.sleep(50); }\n",
    )
    .unwrap();
    bot.scripts.load(&path);
    script::start_silent(&bot).unwrap();
    let started = Instant::now();
    while bot.scripts.status() != script::ScriptStatus::Running {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the script didn't start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    let sword = [NameOrId::Name("Necrotic Sword")];
    assert_eq!(
        gear::equip(&bot, &sword, false),
        Err(GearError::ScriptRunning("Farm.ts".into()))
    );
    assert_eq!(
        gear::load_bank(&bot, false).unwrap_err().to_string(),
        "a script is running: Farm.ts"
    );
    assert!(bot.scripts.is_running());

    let report = gear::equip(&bot, &sword, true).unwrap();
    assert_eq!(report.stopped_script.as_deref(), Some("Farm.ts"));
    assert_eq!(names(&report.equipped), ["Necrotic Sword"]);
    assert!(!bot.scripts.is_running(), "the script stays stopped");
}
