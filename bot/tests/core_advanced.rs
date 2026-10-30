//! Runs the merge template of `scripts/core/CoreAdvanced.ts` against a fake shop.

use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::script;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A player standing in enemyforest with shop 332 loaded; everything else is "null".
struct FakeShop {
    items: &'static str,
}

impl FlashHost for FakeShop {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let path = match args.first() {
            Some(FlashArg::String(s)) => s.as_str(),
            _ => "",
        };
        Ok(match (name, path) {
            ("getGameObject", "world.strMapName") => "\"enemyforest\"".into(),
            ("getGameObject", "world.shopinfo.ShopID") => "332".into(),
            ("getGameObject", "world.shopinfo.items") => self.items.into(),
            ("isNull", "world.shopinfo") => "false".into(),
            _ => "null".into(),
        })
    }
}

fn run_merge(name: &str, items: &'static str, call: &str) -> String {
    let dir = std::env::temp_dir().join(format!("turu-test-merge-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/core")
        .canonicalize()
        .unwrap();
    let source = format!(
        r#"import {{ CoreAdvanced, mergeOptionsEnum }} from "{core}/CoreAdvanced";
const Adv = new CoreAdvanced();
export default function main(): void {{
    {call}
}}
"#,
        core = core.to_string_lossy()
    );
    std::fs::write(dir.join("main.ts"), source).unwrap();

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeShop { items })));
    bot.scripts.load(&dir.join("main.ts"));
    script::start(&bot).unwrap();
    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "script didn't finish: {}",
            bot.logs.script.text()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    bot.logs.script.text()
}

const SHOP: &str = r#"[
    {"ItemID": 1, "ShopItemID": 10, "sName": "Some Armor", "sType": "Armor", "bCoins": 0, "bUpg": 0, "iQty": 1, "iStk": 1,
     "turnin": [{"ItemID": 5, "sName": "Synderes' Souvenir", "iQty": 3}]},
    {"ItemID": 2, "ShopItemID": 11, "sName": "A Note", "sType": "Note", "bCoins": 0, "bUpg": 0, "iQty": 1, "iStk": 1},
    {"ItemID": 3, "ShopItemID": 12, "sName": "Guild Insignia", "sType": "Item", "bCoins": 0, "bUpg": 0, "iQty": 1, "iStk": 1}
]"#;

#[test]
fn ac_only_mode_skips_a_shop_without_ac_items() {
    let log = run_merge(
        "aconly",
        SHOP,
        r#"Adv.startBuyAllMerge("enemyforest", 332, () => {}, null, { buyMode: mergeOptionsEnum.acOnly });"#,
    );
    assert!(
        log.contains("none of the items in this shop are AC tagged"),
        "{log}"
    );
    assert!(!log.contains("failed"), "{log}");
}

#[test]
fn a_requirement_the_shop_doesnt_sell_goes_to_find_ingredients() {
    let log = run_merge(
        "external",
        SHOP,
        r#"Adv.startBuyAllMerge("enemyforest", 332, () => {
        console.log(`find ${Adv.externalItem.name} x${Adv.externalQuant}`);
    }, null, { buyMode: mergeOptionsEnum.all });"#,
    );
    // The note and the insignia are skipped; the armor's souvenirs aren't sold, so the script gets them.
    assert!(log.contains("Farming to buy Some Armor (#0/1)"), "{log}");
    assert!(log.contains("find Synderes' Souvenir x3"), "{log}");
    assert!(!log.contains("A Note"), "{log}");
    assert!(!log.contains("Guild Insignia"), "{log}");
}
