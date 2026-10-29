//! `bot.handlers` in real TypeScript scripts, against a fake game.

use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::script::{self, ScriptStatus};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// Answers the paths the scripts read, like skua.swf would.
struct FakeGame;

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let path = match args.first() {
            Some(FlashArg::String(s)) => s.as_str(),
            _ => "",
        };
        Ok(match (name, path) {
            ("isLoggedIn", _) => "true".into(),
            _ => "null".into(),
        })
    }
}

/// Writes `source` as the script's main file, runs it to its end and returns the bot.
fn run(name: &str, source: &str) -> Arc<Bot> {
    let dir = std::env::temp_dir().join(format!("turu-handlers-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    std::fs::write(dir.join("main.ts"), source).unwrap();

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
    bot.scripts.load(&dir.join("main.ts"));
    script::start(&bot).unwrap();

    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "script didn't finish"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    bot
}

/// The number the script logged after `label` (`bot.log(\`{label} {n}\`)`).
fn logged_number(log: &str, label: &str) -> i64 {
    let line = log
        .lines()
        .find(|l| l.contains(&format!("{label} ")))
        .unwrap_or_else(|| panic!("no `{label}` line in:\n{log}"));
    let rest = &line[line.find(&format!("{label} ")).unwrap() + label.len() + 1..];
    rest.split_whitespace().next().unwrap().parse().unwrap()
}

#[test]
fn a_handler_runs_during_a_sleep() {
    let bot = run(
        "sleep",
        r#"import { bot } from "turu";
export default function main(): void {
    let ticks = 0;
    bot.handlers.register("count", 20, () => { ticks++; });
    bot.sleep(500);
    bot.handlers.clear();
    bot.log(`ticks ${ticks}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert!(logged_number(&log, "ticks") >= 2, "{log}");
    assert_eq!(bot.scripts.status(), ScriptStatus::Loaded, "{log}");
}

#[test]
fn a_handler_runs_after_a_bot_call() {
    let bot = run(
        "after-call",
        r#"import { bot } from "turu";
export default function main(): void {
    let runs = 0;
    bot.handlers.once("first", 20, () => { runs++; });
    bot.sleep(100);
    bot.log(`runs ${runs}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert_eq!(logged_number(&log, "runs"), 1, "{log}");
}

#[test]
fn a_once_handler_runs_one_time() {
    let bot = run(
        "once",
        r#"import { bot } from "turu";
export default function main(): void {
    let runs = 0;
    bot.handlers.once("once", 20, () => { runs++; });
    bot.sleep(400);
    bot.log(`runs ${runs}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert_eq!(logged_number(&log, "runs"), 1, "{log}");
}

#[test]
fn returning_false_removes_the_handler() {
    let bot = run(
        "false",
        r#"import { bot } from "turu";
export default function main(): void {
    let runs = 0;
    bot.handlers.register("stop-me", 20, () => {
        runs++;
        return false;
    });
    bot.sleep(400);
    bot.log(`runs ${runs}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert_eq!(logged_number(&log, "runs"), 1, "{log}");
}

#[test]
fn remove_stops_a_handler() {
    let bot = run(
        "remove",
        r#"import { bot } from "turu";
export default function main(): void {
    let runs = 0;
    bot.handlers.register("gone", 20, () => { runs++; });
    const removed = bot.handlers.remove("gone");
    bot.sleep(300);
    bot.log(`runs ${runs} removed ${removed}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert_eq!(logged_number(&log, "runs"), 0, "{log}");
    assert!(log.contains("removed true"), "{log}");
}

#[test]
fn a_failing_handler_is_logged_and_the_others_still_run() {
    let bot = run(
        "failing",
        r#"import { bot } from "turu";
export default function main(): void {
    let runs = 0;
    bot.handlers.register("broken", 20, () => { throw new Error("boom"); });
    bot.handlers.register("fine", 20, () => { runs++; });
    bot.sleep(300);
    bot.log(`runs ${runs}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert!(log.contains(r#"Handler "broken" failed: boom"#), "{log}");
    assert!(logged_number(&log, "runs") >= 1, "{log}");
    assert_eq!(bot.scripts.status(), ScriptStatus::Loaded, "{log}");
}

#[test]
fn stopping_from_a_handler_stops_the_script() {
    let bot = run(
        "stop",
        r#"import { bot } from "turu";
export default function main(): void {
    bot.handlers.register("stopper", 20, () => { bot.stop(); });
    bot.sleep(5000);
    bot.log("not reached");
}
"#,
    );

    let log = bot.logs.script.text();
    assert!(!log.contains("not reached"), "{log}");
    assert!(log.contains("stopped"), "{log}");
    assert_eq!(bot.scripts.status(), ScriptStatus::Loaded, "{log}");
}
