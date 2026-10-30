//! Runs `scripts/core/CoreDailies.ts` against a fake game.

use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::script;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A player with Mine Crafting (2091) loaded and a few metals in the inventory.
struct FakeGame {
    daily_done: bool,
}

const QUEST_TREE: &str = r#"{"2091": {"QuestID": 2091, "sName": "Mine Crafting", "sField": "ia1", "iIndex": 5,
    "iSlot": -1, "iValue": 0, "oItems": {"900": {"ItemID": 900, "sName": "Raw Ore", "bTemp": 1}},
    "turnin": [{"ItemID": 900, "iQty": 30}]}}"#;

const INVENTORY: &str = r#"[
    {"ItemID": 11932, "sName": "Barium", "iQty": 1000, "iStk": 1000},
    {"ItemID": 12297, "sName": "Copper", "iQty": 1000, "iStk": 1000},
    {"ItemID": 12308, "sName": "Silver", "iQty": 3, "iStk": 1000}
]"#;

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let first = match args.first() {
            Some(FlashArg::String(s)) => s.as_str(),
            _ => "",
        };
        Ok(match (name, first) {
            ("getGameObject", "world.questTree") => QUEST_TREE.into(),
            ("getGameObject", "world.myAvatar.items") => INVENTORY.into(),
            ("getGameObject", "world.bankinfo.items") => "[]".into(),
            ("callGameFunction", "world.getAchievement") => {
                if self.daily_done { "1" } else { "0" }.into()
            }
            ("isLoggedIn", _) => "true".into(),
            _ => "null".into(),
        })
    }
}

fn run(name: &str, daily_done: bool, body: &str) -> (Arc<Bot>, String) {
    let dir = std::env::temp_dir().join(format!("turu-test-dailies-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/core")
        .canonicalize()
        .unwrap();
    let source = format!(
        r#"import {{ bot }} from "turu";
import {{ CoreDailies }} from "{core}/CoreDailies";
const Daily = new CoreDailies();
export default function main(): void {{
    {body}
}}
"#,
        core = core.to_string_lossy()
    );
    std::fs::write(dir.join("main.ts"), source).unwrap();

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame { daily_done })));
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
    let log = bot.logs.script.text();
    assert!(!log.contains("failed"), "{log}");
    (bot, log)
}

#[test]
fn a_completed_daily_is_skipped() {
    let (_, log) = run(
        "done",
        true,
        r#"bot.log(`can=${Daily.checkDailyv2(2091)}`);"#,
    );
    assert!(
        log.contains(r#"Daily/Weekly/Monthly "Mine Crafting [2091]" is not available right now"#),
        "{log}"
    );
    assert!(log.contains("can=false"), "{log}");
}

#[test]
fn max_stacks_stop_the_daily_as_csharp_does() {
    // `any` (the default): one maxed item is enough to skip.
    let (_, log) = run(
        "any",
        false,
        r#"bot.log(`any=${Daily.checkDailyv2(2091, ["Barium", "Silver"])}`);"#,
    );
    assert!(
        log.contains("You already own the maximum amount of: Barium"),
        "{log}"
    );
    assert!(log.contains("any=false"), "{log}");

    // `any: false`: only when every item is maxed.
    let (_, log) = run(
        "all",
        false,
        r#"bot.log(`some=${Daily.checkDailyv2(2091, ["Barium", "Silver"], { any: false })}`);
    bot.log(`every=${Daily.checkDailyv2(2091, ["Barium", "Copper"], { any: false })}`);"#,
    );
    assert!(log.contains("some=true"), "{log}");
    assert!(
        log.contains("You already own the maximum amount of: Barium,Copper"),
        "{log}"
    );
    assert!(log.contains("every=false"), "{log}");
}

#[test]
fn an_available_daily_adds_its_items_and_requirements_to_the_drops() {
    let (bot, log) = run(
        "drops",
        false,
        r#"bot.log(`can=${Daily.checkDailyv2(2091, ["Silver"])}`);"#,
    );
    assert!(log.contains("can=true"), "{log}");
    let to_pickup = turu_bot::api::dispatch(&bot, "drops.toPickup", &[]).unwrap();
    let names: Vec<&str> = to_pickup
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(names.contains(&"Silver"), "{names:?}");
    assert!(names.contains(&"Raw Ore"), "{names:?}");
}
