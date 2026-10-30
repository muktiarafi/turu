//! Runs the enhancement part of `scripts/core/CoreAdvanced.ts` (SmartEnhance, EnhanceItem)
//! against a fake game with the enhancement shops, and checks the `enhanceItemShop` packets.

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::script;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A level 100 free player in /forge, with Void Highlord and its gear equipped.
struct FakeGame {
    /// The enhancement unlock quests done (CoreAdvanced's `uVainglory()` and co.).
    unlocked: &'static [i64],
    member: bool,
    inventory: &'static str,
    shop: Mutex<i64>,
    packets: Mutex<Vec<String>>,
}

/// Every unlock quest of CoreAdvanced; slot 1 is set (done), slot 2 isn't.
const UNLOCK_QUESTS: [i64; 23] = [
    2937, 8738, 8739, 8740, 8741, 8742, 8743, 8744, 8745, 8758, 8821, 8820, 8822, 8823, 8824, 8825,
    8828, 8827, 8826, 9172, 9171, 9560, 9466,
];

const INVENTORY: &str = r#"[
    {"ItemID": 100, "sName": "Void Highlord", "sType": "Class", "sES": "ar", "bEquip": 1, "iQty": 302500,
     "EnhLvl": 100, "EnhPatternID": 2},
    {"ItemID": 101, "sName": "Some Cape", "sType": "Cape", "sES": "ba", "bEquip": 1, "iQty": 1, "EnhLvl": 0},
    {"ItemID": 102, "sName": "Some Helm", "sType": "Helm", "sES": "he", "bEquip": 1, "iQty": 1, "EnhLvl": 0},
    {"ItemID": 103, "sName": "Some Sword", "sType": "Sword", "sES": "Weapon", "bEquip": 1, "iQty": 1, "EnhLvl": 0},
    {"ItemID": 104, "sName": "Some Pet", "sType": "Pet", "sES": "pe", "bEquip": 1, "iQty": 1}
]"#;

/// The helm already has Anima at the player's level.
const INVENTORY_ANIMA_HELM: &str = r#"[
    {"ItemID": 100, "sName": "Void Highlord", "sType": "Class", "sES": "ar", "bEquip": 1, "iQty": 302500,
     "EnhLvl": 100, "EnhPatternID": 9},
    {"ItemID": 102, "sName": "Some Helm", "sType": "Helm", "sES": "he", "bEquip": 1, "iQty": 1,
     "EnhLvl": 100, "EnhPatternID": 28, "sMeta": "dmgAll:1.25,gold:1.1"}
]"#;

fn shop_items(shop: i64) -> &'static str {
    match shop {
        // Lucky, level 50+: the best one the player may buy is the free level 95 one.
        763 => {
            r#"[
            {"ItemID": 1001, "ShopItemID": 1, "sName": "Lucky Armor", "sType": "Enhancement", "iLvl": 90, "bUpg": 0},
            {"ItemID": 1002, "ShopItemID": 2, "sName": "Lucky Armor", "sType": "Enhancement", "iLvl": 95, "bUpg": 0},
            {"ItemID": 1003, "ShopItemID": 3, "sName": "Lucky Armor", "sType": "Enhancement", "iLvl": 100, "bUpg": 1},
            {"ItemID": 1004, "ShopItemID": 4, "sName": "Lucky Armor", "sType": "Enhancement", "iLvl": 105, "bUpg": 0},
            {"ItemID": 1011, "ShopItemID": 5, "sName": "Lucky Helm", "sType": "Enhancement", "iLvl": 100, "bUpg": 0},
            {"ItemID": 1021, "ShopItemID": 6, "sName": "Lucky Cape", "sType": "Enhancement", "iLvl": 100, "bUpg": 0},
            {"ItemID": 1031, "ShopItemID": 7, "sName": "Lucky Weapon", "sType": "Enhancement", "iLvl": 100, "bUpg": 0}
        ]"#
        }
        // Awe (Lucky).
        639 => {
            r#"[
            {"ItemID": 2001, "ShopItemID": 1, "sName": "Spiral Carve Weapon", "sType": "Enhancement", "iLvl": 100, "bUpg": 0},
            {"ItemID": 2002, "ShopItemID": 2, "sName": "Awe Blast Weapon", "sType": "Enhancement", "iLvl": 100, "bUpg": 0}
        ]"#
        }
        // Forge weapons. Named by their special only: an item whose name holds "weapon"
        // ("cape", "helm") also passes C#'s filter and, being first, would win the tie.
        // Unverified against the real shops.
        2142 => {
            r#"[
            {"ItemID": 3001, "ShopItemID": 1, "sName": "Forge", "sType": "Enhancement", "iLvl": 100, "bUpg": 0},
            {"ItemID": 3002, "ShopItemID": 2, "sName": "Valiance", "sType": "Enhancement", "iLvl": 100, "bUpg": 0},
            {"ItemID": 3003, "ShopItemID": 3, "sName": "Dauntless", "sType": "Enhancement", "iLvl": 100, "bUpg": 0}
        ]"#
        }
        // Forge capes.
        2143 => {
            r#"[
            {"ItemID": 4001, "ShopItemID": 1, "sName": "Forge", "sType": "Enhancement", "iLvl": 100, "bUpg": 0},
            {"ItemID": 4002, "ShopItemID": 2, "sName": "Vainglory", "sType": "Enhancement", "iLvl": 100, "bUpg": 0}
        ]"#
        }
        // Forge helms.
        2164 => {
            r#"[
            {"ItemID": 5001, "ShopItemID": 1, "sName": "Forge", "sType": "Enhancement", "iLvl": 100, "bUpg": 0},
            {"ItemID": 5002, "ShopItemID": 2, "sName": "Anima", "sType": "Enhancement", "iLvl": 100, "bUpg": 0}
        ]"#
        }
        _ => "[]",
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
                    self.packets.lock().push(packet.clone());
                }
                "true".into()
            }
            ("callGameFunction", "world.sendLoadShopRequest") => {
                if let Some(FlashArg::Number(id)) = args.get(1) {
                    *self.shop.lock() = *id as i64;
                }
                "true".into()
            }
            ("callGameFunction", "world.getQuestValue") => match args.get(1) {
                Some(FlashArg::Number(slot)) if *slot == 1.0 => "1".into(),
                _ => "0".into(),
            },
            ("isNull", "world.myAvatar") => "false".into(),
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => "1".into(),
            ("getGameObject", "world.lock.loadShop") => r#"{"ts": 0, "cd": 0}"#.into(),
            ("isNull", "world.shopinfo") => (*self.shop.lock() == 0).to_string(),
            ("getGameObject", "world.shopinfo.ShopID") => self.shop.lock().to_string(),
            ("getGameObject", "world.shopinfo.items") => shop_items(*self.shop.lock()).into(),
            ("getGameObject", "world.questTree") => {
                let quests: Vec<String> = UNLOCK_QUESTS
                    .iter()
                    .map(|id| {
                        let slot = if self.unlocked.contains(id) { 1 } else { 2 };
                        format!(
                            r#""{id}": {{"QuestID": {id}, "sName": "Unlock {id}", "iSlot": {slot}, "iValue": 1}}"#
                        )
                    })
                    .collect();
                format!("{{{}}}", quests.join(","))
            }
            ("getGameObject", "world.myAvatar.items") => self.inventory.into(),
            ("getGameObject", "world.bankinfo.items") => "[]".into(),
            ("getGameObject", "world.strMapName") => "\"forge\"".into(),
            ("getGameObject", "world.curRoom") => "7".into(),
            ("getGameObject", "world.myAvatar.dataLeaf.intLevel") => "100".into(),
            ("getGameObject", "world.myAvatar.objData.iUpgDays") => {
                if self.member { "30" } else { "-1" }.into()
            }
            ("getGameObject", path) if path.starts_with("world.invTree.") => "0".into(),
            ("isLoggedIn", _) => "true".into(),
            _ => "null".into(),
        })
    }
}

fn run(name: &str, game: FakeGame, body: &str) -> (Vec<String>, String) {
    let dir = std::env::temp_dir().join(format!("turu-test-enh-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/core")
        .canonicalize()
        .unwrap();
    let source = format!(
        r#"import {{ bot }} from "turu";
import {{ CapeSpecial, CoreAdvanced, EnhancementType, GenericGearBoost, HelmSpecial, RacialGearBoost, WeaponSpecial }} from "{core}/CoreAdvanced";
const Adv = new CoreAdvanced();
export default function main(): void {{
    {body}
}}
"#,
        core = core.to_string_lossy()
    );
    std::fs::write(dir.join("main.ts"), source).unwrap();

    let game = Arc::new(game);
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    bot.scripts.load(&dir.join("main.ts"));
    script::start(&bot).unwrap();
    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < Duration::from_secs(90),
            "script didn't finish: {}",
            bot.logs.script.text()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let log = bot.logs.script.text();
    let packets = game.packets.lock().clone();
    (packets, log)
}

fn game(unlocked: &'static [i64], inventory: &'static str) -> FakeGame {
    FakeGame {
        unlocked,
        member: false,
        inventory,
        shop: Mutex::new(0),
        packets: Mutex::new(Vec::new()),
    }
}

fn enhance(item: i64, enhancement: i64, shop: i64) -> String {
    format!("%xt%zm%enhanceItemShop%7%{item}%{enhancement}%{shop}%")
}

#[test]
fn smart_enhance_uses_the_forge_library_when_its_specials_are_unlocked() {
    // Void Highlord: Lucky, Vainglory, Anima, and Dauntless over Valiance.
    let (packets, log) = run(
        "forge",
        game(&[2937, 8741, 8744, 8826, 9172], INVENTORY),
        r#"Adv.smartEnhance("void highlord");"#,
    );
    // The class: the best Lucky armor a free level 100 player may buy (not the member one, not
    // the level 105 one).
    assert!(
        packets.contains(&enhance(100, 1002, 763)),
        "{packets:?}\n{log}"
    );
    assert!(
        packets.contains(&enhance(101, 4002, 2143)),
        "{packets:?}\n{log}"
    );
    assert!(
        packets.contains(&enhance(102, 5002, 2164)),
        "{packets:?}\n{log}"
    );
    assert!(
        packets.contains(&enhance(103, 3003, 2142)),
        "{packets:?}\n{log}"
    );
    // The pet isn't enhanceable.
    assert!(!packets.iter().any(|p| p.contains("%104%")), "{packets:?}");
    assert!(!log.contains("Enhancement Failed"), "{log}");
}

#[test]
fn smart_enhance_falls_back_to_awe_enhancements() {
    // Only Awe is unlocked: Void Highlord gets Lucky with Spiral Carve, and plain Lucky gear.
    let (packets, log) = run(
        "awe",
        game(&[2937], INVENTORY),
        r#"Adv.smartEnhance("Void Highlord");"#,
    );
    assert!(
        packets.contains(&enhance(100, 1002, 763)),
        "{packets:?}\n{log}"
    );
    assert!(
        packets.contains(&enhance(101, 1021, 763)),
        "{packets:?}\n{log}"
    );
    assert!(
        packets.contains(&enhance(102, 1011, 763)),
        "{packets:?}\n{log}"
    );
    assert!(
        packets.contains(&enhance(103, 2001, 639)),
        "{packets:?}\n{log}"
    );
    assert!(!packets.iter().any(|p| p.contains("%2142%")), "{packets:?}");
}

#[test]
fn enhance_item_skips_what_is_done_and_refuses_locked_specials() {
    // The helm already has Anima at level 100; Vim isn't unlocked.
    let (packets, log) = run(
        "skip",
        game(&[8826], INVENTORY_ANIMA_HELM),
        r#"Adv.enhanceItem("Some Helm", EnhancementType.Lucky, { hSpecial: HelmSpecial.Anima });
    Adv.enhanceItem("Some Helm", EnhancementType.Lucky, { hSpecial: HelmSpecial.Vim });
    Adv.enhanceItem("Missing Thing", EnhancementType.Lucky);
    bot.log(`class=${Adv.currentClassEnh()} helm=${Adv.currentHelmSpecial()} cape=${Adv.currentCapeSpecial()}`);
    bot.log(`boost=${Adv.hasMinimalBoost(GenericGearBoost.dmgAll, 25)},${Adv.hasMinimalBoost(GenericGearBoost.dmgAll, 30)},${Adv.hasMinimalBoost(GenericGearBoost.gold, 10)},${Adv.hasMinimalBoost(RacialGearBoost.Undead, 1)}`);
    Adv.gearStore();"#,
    );
    assert!(packets.is_empty(), "{packets:?}\n{log}");
    assert!(log.contains("Enhancement Skipped:\t1 item"), "{log}");
    assert!(
        log.contains("Enhancement Failed:\tYou did not unlock the Vim Enhancement yet"),
        "{log}"
    );
    assert!(
        log.contains(r#"Enhancement Failed: Could not find "Missing Thing""#),
        "{log}"
    );
    assert!(log.contains("class=9 helm=28 cape=0"), "{log}");
    assert!(log.contains("boost=true,false,true,false"), "{log}");
    assert!(log.contains(" - Items: Void Highlord, Some Helm"), "{log}");
    assert!(log.contains(" - Class Enh: Lucky"), "{log}");
    assert!(log.contains(" - Helm Special: Anima"), "{log}");
}
