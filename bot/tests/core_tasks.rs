//! `scripts/core/CoreTasks.ts` (C#'s `Task.Run` as generator tasks) in real scripts, against a fake game.

use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::script::{self, ScriptStatus};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

struct FakeGame;

impl FlashHost for FakeGame {
    fn call(&self, name: &str, _args: Vec<FlashArg>) -> Result<String, HostError> {
        Ok(match name {
            "isLoggedIn" => "true".into(),
            _ => "null".into(),
        })
    }
}

/// Writes `source` as the script's main file (`CORE` is the repo's scripts/core), runs it to its
/// end and returns the bot.
fn run(name: &str, source: &str) -> Arc<Bot> {
    let dir = std::env::temp_dir().join(format!("turu-tasks-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/core")
        .canonicalize()
        .unwrap();
    let source = source.replace("CORE", &core.to_string_lossy());
    std::fs::write(dir.join("main.ts"), source).unwrap();

    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame)));
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
    bot
}

/// The text the script logged after `label` (`bot.log(\`{label} {text}\`)`).
fn logged(log: &str, label: &str) -> String {
    let line = log
        .lines()
        .find(|l| l.contains(&format!("{label} ")))
        .unwrap_or_else(|| panic!("no `{label}` line in:\n{log}"));
    line[line.find(&format!("{label} ")).unwrap() + label.len() + 1..]
        .trim()
        .to_owned()
}

#[test]
fn a_task_runs_beside_the_script_and_waits_with_yield() {
    let bot = run(
        "steps",
        r#"import { bot } from "turu";
import { tasks } from "CORE/CoreTasks";
export default function main(): void {
    const steps: string[] = [];
    tasks.start("walk", function* () {
        steps.push("start");
        yield 200;
        steps.push("after-delay");
        yield 200;
        steps.push("end");
    });
    // Task.Run returns before its body starts.
    steps.push("main");
    bot.sleep(100);
    const midway = tasks.running("walk");
    bot.sleep(800);
    bot.log(`steps ${steps.join(",")}`);
    bot.log(`midway ${midway} after ${tasks.running("walk")}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert_eq!(logged(&log, "steps"), "main,start,after-delay,end", "{log}");
    assert_eq!(logged(&log, "midway"), "true after false", "{log}");
    assert_eq!(bot.scripts.status(), ScriptStatus::Loaded, "{log}");
}

#[test]
fn until_gives_back_whether_the_condition_came_true() {
    let bot = run(
        "until",
        r#"import { bot } from "turu";
import { tasks, until } from "CORE/CoreTasks";
export default function main(): void {
    let flag = false;
    const results: boolean[] = [];
    tasks.start("met", function* () {
        results.push(yield until(() => flag, 30));
    });
    tasks.start("timeout", function* () {
        results.push(yield until(() => false, 2));
    });
    bot.sleep(500);
    flag = true;
    bot.sleep(300);
    bot.log(`results ${results.join(",")}`);
}
"#,
    );

    let log = bot.logs.script.text();
    // The 2-try (~200 ms) timeout gives up first; the other sees the flag set at 500 ms.
    assert_eq!(logged(&log, "results"), "false,true", "{log}");
}

#[test]
fn cancel_ends_a_task_and_runs_its_finally() {
    let bot = run(
        "cancel",
        r#"import { bot } from "turu";
import { tasks } from "CORE/CoreTasks";
export default function main(): void {
    let loops = 0;
    let cleaned = false;
    tasks.start("loop", function* () {
        try {
            for (;;) {
                loops++;
                yield 50;
            }
        } finally {
            cleaned = true;
        }
    });
    bot.sleep(300);
    const cancelled = tasks.cancel("loop");
    const atCancel = loops;
    bot.sleep(200);
    bot.log(`cancel ${cancelled} ${cleaned} ${tasks.running("loop")} ${loops === atCancel} ${atCancel > 1}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert_eq!(logged(&log, "cancel"), "true true false true true", "{log}");
}

#[test]
fn a_task_can_cancel_or_replace_itself() {
    let bot = run(
        "self",
        r#"import { bot } from "turu";
import { tasks } from "CORE/CoreTasks";
export default function main(): void {
    const seen: string[] = [];
    tasks.start("self", function* () {
        try {
            seen.push("first");
            tasks.cancel("self");
            yield 0;
            seen.push("never");
        } finally {
            seen.push("closed");
        }
    });
    tasks.start("again", function* () {
        seen.push("old");
        tasks.start("again", function* () {
            seen.push("new");
        });
        yield 0;
        seen.push("never");
    });
    bot.sleep(300);
    bot.log(`seen ${seen.join(",")}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert_eq!(logged(&log, "seen"), "first,closed,old,new", "{log}");
}

#[test]
fn a_failing_task_is_logged_and_the_others_keep_running() {
    let bot = run(
        "fail",
        r#"import { bot } from "turu";
import { tasks } from "CORE/CoreTasks";
export default function main(): void {
    let runs = 0;
    tasks.start("broken", function* () {
        yield 50;
        throw new Error("boom");
    });
    tasks.start("fine", function* () {
        for (;;) {
            runs++;
            yield 50;
        }
    });
    bot.sleep(400);
    tasks.cancelAll();
    bot.log(`runs ${runs > 3} ${tasks.running("broken")}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert!(log.contains(r#"Task "broken" failed: boom"#), "{log}");
    assert_eq!(logged(&log, "runs"), "true false", "{log}");
}

#[test]
fn stop_inside_a_task_stops_the_script() {
    let bot = run(
        "stop",
        r#"import { bot } from "turu";
import { tasks } from "CORE/CoreTasks";
export default function main(): void {
    tasks.start("stopper", function* () {
        yield 50;
        bot.stop();
    });
    bot.sleep(5000);
    bot.log("not reached");
}
"#,
    );

    let log = bot.logs.script.text();
    assert!(!log.contains("not reached"), "{log}");
}

#[test]
fn the_nightmare_carnax_ultra_loads() {
    // Its module tree (CoreEnginev3, CoreUltrav3, UltraPotions, ...) evaluates without the game.
    let bot = run(
        "carnax",
        r#"import { bot } from "turu";
import { scriptOptions } from "CORE/../ultrasv3/challengebosses/NightmareCarnaxv3";
import { CoreEnginev3 } from "CORE/../ultrasv3/dependenciesultras/CoreEnginev3";
export default function main(): void {
    bot.log(`storage ${scriptOptions.storage} ${typeof CoreEnginev3.instance.enableSkills}`);
}
"#,
    );

    let log = bot.logs.script.text();
    assert_eq!(
        logged(&log, "storage"),
        "NightmareCarnax-v3 function",
        "{log}"
    );
}
