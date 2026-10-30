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
fn choice_options_and_groups_read_and_save_like_vibeskua() {
    let dir = temp_dir("option-groups");
    let storage = "RuntimeOptionGroupsTest";
    let file = turu_bot::paths::Paths::new().script_options_file(storage);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "Options:Pet=Dragon Pet\nGeneric:mode=mergeMats\n").unwrap();
    std::fs::write(
        dir.join("main.ts"),
        format!(
            r#"import {{ bot, choices, defineScriptOptions, option }} from "turu";
enum Pet {{ None = "None", Dragon_Pet = "Dragon_Pet" }}
enum Mode {{ all, acOnly, mergeMats, select }}
export const scriptOptions = defineScriptOptions("{storage}", [
    option.choice("Pet", "Pet", "", choices(Pet), Pet.None),
], true, {{
    Generic: [option.choice("mode", "Mode", "", choices(Mode))],
    Select: [option.bool("1234", "Item", "")],
}});
export default function main(): void {{
    const mode = bot.config.get<string>("Generic", "mode");
    bot.log(`pet=${{bot.config.get<Pet>("Pet")}} mode=${{mode}} index=${{Mode[mode as keyof typeof Mode]}} item=${{bot.config.get("Select", "1234")}}`);
    bot.log(`has=${{bot.config.has("Select", "1234")}} main=${{bot.config.has("1234")}}`);
    bot.config.set("Select", "1234", true);
    try {{
        bot.config.set("Pet", "Cat");
    }} catch (error) {{
        bot.log(`bad: ${{(error as Error).message}}`);
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
    assert!(
        log.contains("pet=Dragon_Pet mode=mergeMats index=2 item=false"),
        "{log}"
    );
    assert!(log.contains("has=true main=false"), "{log}");
    assert!(
        log.contains("bad: config.set: \"Pet\" must be one of None, Dragon_Pet"),
        "{log}"
    );
    let saved = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        saved,
        "Options:Pet=Dragon_Pet\nGeneric:mode=mergeMats\nSelect:1234=True\n"
    );
}

/// A script with one option that logs "in main" and calls `body`.
fn options_script(name: &str, dont_preconfigure: bool, body: &str) -> std::path::PathBuf {
    let dir = temp_dir(name);
    let path = dir.join("main.ts");
    std::fs::write(
        &path,
        format!(
            r#"import {{ bot, defineScriptOptions, option }} from "turu";
export const scriptOptions = defineScriptOptions("Runtime{name}", [option.bool("On", "On", "")], {dont_preconfigure});
export default function main(): void {{
    bot.log("in main");
    {body}
    bot.log("done");
}}
"#
        ),
    )
    .unwrap();
    path
}

fn wait_for_request(bot: &Bot) -> u64 {
    let start = Instant::now();
    loop {
        if let Some(request) = bot.scripts.configure_request() {
            return request;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no configure request"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn start_waits_for_the_options_window_unless_dont_preconfigure() {
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.set_interactive(true);
    bot.scripts.load(&options_script("Preconfigure", false, ""));
    script::start(&bot).unwrap();
    let request = wait_for_request(&bot);
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        !bot.logs.script.text().contains("in main"),
        "main ran before the window closed"
    );
    bot.scripts.finish_configure(request);
    wait_until_done(&bot);
    assert!(bot.logs.script.text().contains("done"));

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.set_interactive(true);
    bot.scripts
        .load(&options_script("DontPreconfigure", true, ""));
    script::start(&bot).unwrap();
    wait_until_done(&bot);
    assert!(bot.logs.script.text().contains("done"));
}

#[test]
fn config_configure_waits_for_the_window_and_stop_ends_the_wait() {
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.set_interactive(true);
    bot.scripts.load(&options_script(
        "Configure",
        true,
        r#"bot.log(`has=${bot.config.has("On")} missing=${bot.config.has("Off")}`);
    bot.config.configure();"#,
    ));
    script::start(&bot).unwrap();
    let request = wait_for_request(&bot);
    bot.scripts.finish_configure(request);
    wait_until_done(&bot);
    let log = bot.logs.script.text();
    assert!(log.contains("has=true missing=false"), "{log}");
    assert!(log.contains("done"), "{log}");

    script::start(&bot).unwrap();
    wait_for_request(&bot);
    script::stop(&bot);
    wait_until_done(&bot);
    assert_eq!(bot.scripts.configure_request(), None);
    let log = bot.logs.script.text();
    assert_eq!(log.matches("done").count(), 1, "{log}");
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn silent_runs_and_runs_without_a_ui_dont_show_the_window() {
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts
        .load(&options_script("NoUi", false, "bot.config.configure();"));
    script::start(&bot).unwrap();
    wait_until_done(&bot);
    assert!(bot.logs.script.text().contains("done"));

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.set_interactive(true);
    bot.scripts
        .load(&options_script("Silent", false, "bot.config.configure();"));
    script::start_silent(&bot).unwrap();
    wait_until_done(&bot);
    assert!(bot.logs.script.text().contains("done"));
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
    assert_eq!(declaration.options.len(), 21);
    assert_eq!(declaration.options[0].name, "BuyReagents");
    assert_eq!(declaration.options[1].name, "PotionQuant");
    assert_eq!(declaration.options[3].name, "SkipOption");
    assert!(declaration.dont_preconfigure);
    assert!(declaration.options.iter().any(|o| o.name == "FarmSoul"));
}

#[test]
fn merge_scripts_read_vibeskuas_merge_options_file() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/other/mergeshops/SynderesMerge.ts");
    let declaration = turu_bot::script::options::declaration(&script)
        .unwrap()
        .expect("SynderesMerge declares options");
    assert_eq!(declaration.storage, "MergeOptionStorage");
    assert!(declaration.options.is_empty());
    assert!(declaration.dont_preconfigure);
    let names: Vec<&str> = declaration.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Generic", "Select"]);
    assert_eq!(declaration.groups[0].options[0].name, "SkipOption");
    assert_eq!(declaration.groups[0].options[1].name, "mode");
    assert_eq!(
        declaration.groups[0].options[1].choices,
        ["all", "acOnly", "mergeMats", "select"]
    );

    // A file VibeSkua wrote: its four explanation lines are all named " ".
    let dir = temp_dir("merge-options");
    let file = dir.join("MergeOptionStorage.cfg");
    std::fs::write(
        &file,
        "Generic:SkipOption=True\nGeneric:mode=select\nGeneric: =click here\nGeneric: =click here\n\
         Select:29576=True\nSelect:29711=False\n",
    )
    .unwrap();
    let options = turu_bot::script::options::ScriptOptions::load(declaration, file).unwrap();
    assert_eq!(
        options.get_in("Generic", "SkipOption"),
        Some(&serde_json::json!(true))
    );
    assert_eq!(
        options.get_in("Generic", "mode"),
        Some(&serde_json::json!("select"))
    );
    assert_eq!(
        options.get_in("Select", "29576"),
        Some(&serde_json::json!(true))
    );
    assert_eq!(
        options.get_in("Select", "29574"),
        Some(&serde_json::json!(false))
    );
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

#[test]
fn farmer_joe_do_all_loads_and_stops() {
    // The whole FarmerJoe tree (~125 modules) has to import and construct without error.
    temp_dir("farmerjoe");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/other/farmjoekits/0FarmerJoeDoAll.ts");
    let declaration = turu_bot::script::options::declaration(&script)
        .unwrap()
        .expect("DoAll declares CoreFarmerJoe's options");
    assert_eq!(declaration.storage, "FarmerJoePet");
    assert!(declaration.dont_preconfigure);
    assert!(declaration.options.iter().any(|o| o.name == "PetChoice"));

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
