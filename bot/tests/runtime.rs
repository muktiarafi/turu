//! Runs real TypeScript scripts against a fake game.

use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::script::{self, ScriptStatus};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// Answers `getGameObject` for a few paths, like skua.swf would.
struct FakeGame;

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let path = match args.first() {
            Some(FlashArg::String(s)) => s.as_str(),
            _ => "",
        };
        Ok(match (name, path) {
            ("getGameObject", "world.myAvatar.objData.intGold") => "1234".into(),
            ("isLoggedIn", _) => "true".into(),
            _ => "null".into(),
        })
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("turu-test-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    dir
}

fn wait_until_done(bot: &Bot) {
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
fn runs_a_typescript_script_with_imports() {
    let dir = temp_dir("imports");
    std::fs::write(
        dir.join("lib/helper.ts"),
        "export function double(n: number): number { return n * 2; }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.ts"),
        r#"import { bot } from "turu";
import { double } from "./lib/helper";
interface Shown { gold: number }
export default function main(): void {
    const shown: Shown = { gold: double(bot.player.gold) };
    bot.log(`gold ${shown.gold}`);
}
"#,
    )
    .unwrap();

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&dir.join("main.ts"));
    script::start(&bot).unwrap();
    wait_until_done(&bot);

    let log = bot.logs.script.text();
    assert!(log.contains("gold 2468"), "{log}");
    assert_eq!(bot.scripts.status(), ScriptStatus::Loaded, "{log}");
}

#[test]
fn stop_interrupts_a_busy_script() {
    let dir = temp_dir("stop");
    std::fs::write(
        dir.join("main.ts"),
        r#"import { bot } from "turu";
export default function main(): void {
    bot.log("looping");
    try { while (true) { bot.sleep(50); } } catch (e) { /* scripts that swallow errors */ }
    while (true) {}
}
"#,
    )
    .unwrap();

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&dir.join("main.ts"));
    script::start(&bot).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    script::stop(&bot);
    wait_until_done(&bot);
    let log = bot.logs.script.text();
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn reports_errors_with_location() {
    let dir = temp_dir("error");
    std::fs::write(
        dir.join("main.ts"),
        "import { bot } from \"turu\";\nexport default function main(): void {\n    if (bot.player.gold > 0) throw new Error(\"boom\");\n}\n",
    )
    .unwrap();

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&dir.join("main.ts"));
    script::start(&bot).unwrap();
    wait_until_done(&bot);
    let log = bot.logs.script.text();
    assert_eq!(bot.scripts.status(), ScriptStatus::Error, "{log}");
    assert!(log.contains("boom") && log.contains("main.ts"), "{log}");
}

#[test]
fn max_bags_loads_and_stops() {
    temp_dir("maxbags");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/nation/materials/0MaxBags.ts");
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&script);
    script::start(&bot).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    script::stop(&bot);
    wait_until_done(&bot);
    let log = bot.logs.script.text();
    // The fake game never logs in, so the script waits; it must have loaded and then stopped.
    assert!(!log.contains("failed"), "{log}");
    assert!(log.contains("stopped") || log.contains("finished"), "{log}");
}

#[test]
fn gold_farm_loads_and_stops() {
    temp_dir("goldfarm");
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/farm/GoldFarm.ts");
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&script);
    script::start(&bot).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    script::stop(&bot);
    wait_until_done(&bot);
    let log = bot.logs.script.text();
    // Like 0MaxBags: the fake game never logs in, so the script waits until it is stopped.
    assert!(!log.contains("failed"), "{log}");
    assert!(log.contains("stopped") || log.contains("finished"), "{log}");
}
