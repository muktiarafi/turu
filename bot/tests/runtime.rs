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

#[test]
fn script_options_are_read_from_the_saved_file_and_saved_back() {
    let dir = temp_dir("options");
    let storage = "RuntimeOptionsTest";
    let file = turu_bot::paths::Paths::new().script_options_file(storage);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "Options:Count=7\nOptions:Enabled=True\n").unwrap();
    std::fs::write(
        dir.join("main.ts"),
        format!(
            r#"import {{ bot, defineScriptOptions, option }} from "turu";
export const scriptOptions = defineScriptOptions("{storage}", [
    option.int("Count", "Count", "How many", 1),
    option.bool("Enabled", "Enabled", "", false),
    option.text("Label", "Label", "", "start"),
]);
export default function main(): void {{
    bot.log(`count=${{bot.config.get<number>("Count")}} enabled=${{bot.config.get<boolean>("Enabled")}} label=${{bot.config.get<string>("Label")}}`);
    bot.config.set("Label", "changed");
    try {{
        bot.config.get("Missing");
    }} catch (error) {{
        bot.log(`missing: ${{(error as Error).message}}`);
    }}
}}
"#
        ),
    )
    .unwrap();
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&dir.join("main.ts"));
    script::start(&bot).unwrap();
    wait_until_done(&bot);
    let log = bot.logs.script.text();
    assert!(log.contains("count=7 enabled=true label=start"), "{log}");
    assert!(log.contains("no option named \"Missing\""), "{log}");
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("Options:Label=changed"), "{saved}");
    assert!(saved.contains("Options:Count=7"), "{saved}");
}

#[test]
fn a_bot_call_outside_main_makes_the_options_fail() {
    let dir = temp_dir("options-toplevel");
    std::fs::write(
        dir.join("main.ts"),
        r#"import { bot, defineScriptOptions, option } from "turu";
bot.log("top level");
export const scriptOptions = defineScriptOptions("RuntimeTopLevelTest", [option.bool("On", "On", "")]);
export default function main(): void {}
"#,
    )
    .unwrap();
    let bot = Bot::new();
    bot.scripts.load(&dir.join("main.ts"));
    let message = match bot.scripts.options() {
        turu_bot::script::ScriptOptionsState::Failed(message) => message,
        other => panic!("expected the options to fail, got {other:?}"),
    };
    assert!(message.contains("options are read"), "{message}");
}

#[test]
fn a_script_without_options_declares_none() {
    temp_dir("options-none");
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/farm/GoldFarm.ts");
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&script);
    assert!(matches!(
        bot.scripts.options(),
        turu_bot::script::ScriptOptionsState::NoOptions
    ));
    assert!(!bot.scripts.has_options());
}

fn repo_typescript_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            repo_typescript_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "ts") {
            out.push(path);
        }
    }
}

#[test]
fn every_repo_script_declares_its_options_without_error() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts");
    let mut files = Vec::new();
    repo_typescript_files(&root, &mut files);
    assert!(files.len() > 5, "found {} scripts", files.len());
    for file in files {
        if let Err(e) = turu_bot::script::options::declaration(&file) {
            panic!("{}: {e}", file.display());
        }
    }
}

#[test]
fn potions_declares_its_csharp_options() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/other/various/Potions.ts");
    let declaration = turu_bot::script::options::declaration(&script)
        .unwrap()
        .expect("Potions declares options");
    assert_eq!(declaration.storage, "Potionv2");
    assert_eq!(declaration.options.len(), 20);
    assert_eq!(declaration.options[0].name, "BuyReagents");
    assert_eq!(declaration.options[1].name, "PotionQuant");
    assert!(declaration.options.iter().any(|o| o.name == "FarmSoul"));
}

#[test]
fn potions_loads_and_stops() {
    temp_dir("potions");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/other/various/Potions.ts");
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&script);
    script::start(&bot).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    script::stop(&bot);
    wait_until_done(&bot);
    let log = bot.logs.script.text();
    assert!(!log.contains("failed"), "{log}");
    assert!(log.contains("stopped") || log.contains("finished"), "{log}");
}

/// Runs `body` as `main` with a `scriptStopping` handler that uses the bot; `stop` presses Stop.
fn run_with_stopping_handler(name: &str, body: &str, stop: bool) -> String {
    let dir = temp_dir(name);
    std::fs::write(
        dir.join("main.ts"),
        format!(
            r#"import {{ bot }} from "turu";
export default function main(): void {{
    bot.events.on("scriptStopping", (error) => {{
        bot.sleep(10);
        bot.log(`stopping handler: ${{error}}; gold ${{bot.player.gold}}`);
    }});
    {body}
}}
"#
        ),
    )
    .unwrap();
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&dir.join("main.ts"));
    script::start(&bot).unwrap();
    if stop {
        std::thread::sleep(Duration::from_millis(300));
        script::stop(&bot);
    }
    wait_until_done(&bot);
    bot.logs.script.text()
}

#[test]
fn stopping_handlers_run_on_stop_and_can_use_the_bot() {
    let log = run_with_stopping_handler("stopping-stop", "while (true) bot.sleep(50);", true);
    assert!(log.contains("stopping handler: null; gold 1234"), "{log}");
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn stopping_handlers_get_the_error_of_a_failed_script() {
    let log = run_with_stopping_handler("stopping-fail", r#"throw new Error("boom");"#, false);
    assert!(log.contains("stopping handler: boom"), "{log}");
}

#[test]
fn stopping_handlers_dont_run_when_main_returns() {
    let log = run_with_stopping_handler("stopping-done", r#"bot.log("done");"#, false);
    assert!(
        log.contains("done") && !log.contains("stopping handler"),
        "{log}"
    );
}
