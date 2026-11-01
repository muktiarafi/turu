//! Scripts > Scheduler: the playlist runs real scripts one after another against a fake game.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use turu_bot::scheduler::{Delays, EntryStatus};
use turu_bot::script;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// Logged in or not, as the test says.
struct FakeGame {
    logged_in: Arc<AtomicBool>,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, _args: Vec<FlashArg>) -> Result<String, HostError> {
        Ok(match name {
            "isLoggedIn" => self.logged_in.load(Ordering::SeqCst).to_string(),
            _ => "null".into(),
        })
    }
}

fn data_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("turu-test-scheduler-data-{}", std::process::id()));
    // SAFETY: every test in this file sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", &dir) };
    dir
}

fn scripts_dir(name: &str) -> PathBuf {
    data_dir();
    let dir =
        std::env::temp_dir().join(format!("turu-test-scheduler-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, file: &str, body: &str) -> PathBuf {
    let path = dir.join(file);
    std::fs::write(&path, body).unwrap();
    path
}

/// A script whose `main` runs `body`.
fn script(dir: &Path, file: &str, body: &str) -> PathBuf {
    write(
        dir,
        file,
        &format!(
            "import {{ bot }} from \"turu\";\nexport default function main(): void {{\n    {body}\n}}\n"
        ),
    )
}

fn fast() -> Delays {
    let ms = Duration::from_millis;
    Delays {
        after_stop: ms(20),
        after_failure: ms(20),
        after_missing: ms(20),
        login_poll: ms(20),
        after_login: ms(20),
        after_lingering: ms(20),
    }
}

fn new_bot() -> (Arc<Bot>, Arc<AtomicBool>) {
    data_dir();
    let logged_in = Arc::new(AtomicBool::new(true));
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame {
        logged_in: logged_in.clone(),
    })));
    bot.scheduler.set_delays(fast());
    (bot, logged_in)
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn statuses(bot: &Bot) -> Vec<EntryStatus> {
    bot.scheduler.entries().iter().map(|e| e.status).collect()
}

/// The script log's lines that start with `prefix` (after the time).
fn lines(bot: &Bot, prefix: &str) -> Vec<String> {
    bot.logs
        .script
        .text()
        .lines()
        .filter_map(|l| l.find(prefix).map(|i| l[i..].to_owned()))
        .filter(|l| !l.starts_with(&format!("{prefix}\"")))
        .collect()
}

#[test]
fn runs_the_playlist_in_order() {
    let dir = scripts_dir("order");
    let (bot, _) = new_bot();
    for name in ["a", "b", "c"] {
        let path = script(
            &dir,
            &format!("{name}.ts"),
            &format!("bot.log(\"ran {name}\");"),
        );
        bot.scheduler.add(&path).unwrap();
    }
    assert_eq!(bot.scheduler.summary(), "3 scripts");
    bot.scheduler.start(&bot).unwrap();
    assert!(
        bot.scheduler.summary().contains("running 1 of 3"),
        "{}",
        bot.scheduler.summary()
    );
    assert!(
        bot.scheduler.add(&dir.join("a.ts")).is_err(),
        "added while running"
    );
    wait_for("the playlist", || !bot.scheduler.is_running());

    assert_eq!(lines(&bot, "ran "), ["ran a", "ran b", "ran c"]);
    assert_eq!(statuses(&bot), [EntryStatus::Completed; 3]);
    assert!(bot.scheduler.entries().iter().all(|e| e.duration.is_some()));
    assert_eq!(bot.scheduler.summary(), "3 scripts");
    assert_eq!(bot.scripts.loaded().unwrap().file_name().unwrap(), "c.ts");
}

#[test]
fn moves_on_after_errors_and_missing_files() {
    let dir = scripts_dir("errors");
    let (bot, _) = new_bot();
    let throws = script(&dir, "throws.ts", "throw new Error(\"boom\");");
    let broken = write(&dir, "broken.ts", "export default function main( {\n");
    let gone = script(&dir, "gone.ts", "");
    let last = script(&dir, "last.ts", "bot.log(\"ran last\");");
    for path in [&throws, &broken, &gone, &last] {
        bot.scheduler.add(path).unwrap();
    }
    std::fs::remove_file(&gone).unwrap();
    bot.scheduler.start(&bot).unwrap();
    wait_for("the playlist", || !bot.scheduler.is_running());

    // VibeSkua: an error while running ends the script like any stop; one that keeps it from
    // starting is "Failed".
    assert_eq!(
        statuses(&bot),
        [
            EntryStatus::Completed,
            EntryStatus::Failed,
            EntryStatus::FileNotFound,
            EntryStatus::Completed
        ]
    );
    assert_eq!(lines(&bot, "ran "), ["ran last"]);
}

#[test]
fn stop_script_moves_on_and_stop_queue_stops() {
    let dir = scripts_dir("stop");
    let (bot, _) = new_bot();
    let forever = "bot.log(\"looping\"); while (true) bot.sleep(20);";
    for name in ["one", "two", "three"] {
        let path = script(&dir, &format!("{name}.ts"), forever);
        bot.scheduler.add(&path).unwrap();
    }
    bot.scheduler.start(&bot).unwrap();
    wait_for("the first script", || lines(&bot, "looping").len() == 1);

    // Stop Script (the Script Loader's) completes the entry; the playlist carries on.
    script::stop(&bot);
    wait_for("the second script", || lines(&bot, "looping").len() == 2);
    assert_eq!(statuses(&bot)[0], EntryStatus::Completed);
    assert_eq!(statuses(&bot)[1], EntryStatus::Running);

    // Stop queue stops the script and the playlist.
    bot.scheduler.stop(&bot);
    wait_for("the script to stop", || !bot.scripts.is_running());
    std::thread::sleep(Duration::from_millis(200));
    assert!(!bot.scheduler.is_running());
    assert_eq!(
        statuses(&bot),
        [
            EntryStatus::Completed,
            EntryStatus::Stopped,
            EntryStatus::Queued
        ]
    );
    assert_eq!(lines(&bot, "looping").len(), 2, "a third script started");

    // Starting again runs from the top.
    bot.scheduler.start(&bot).unwrap();
    wait_for("the first script again", || {
        lines(&bot, "looping").len() == 3
    });
    assert_eq!(bot.scheduler.current(), Some(0));
    assert_eq!(statuses(&bot)[1], EntryStatus::Queued);
    bot.scheduler.stop(&bot);
    wait_for("the script to stop", || !bot.scripts.is_running());
}

#[test]
fn a_disconnection_waits_for_the_login_and_resumes_the_script() {
    let dir = scripts_dir("relogin");
    let (bot, logged_in) = new_bot();
    // Auto-relogin restarts the script right after the login, before the playlist looks.
    bot.scheduler.set_delays(Delays {
        after_login: Duration::from_millis(500),
        ..fast()
    });
    let path = script(
        &dir,
        "farm.ts",
        "bot.log(\"farming\"); while (true) bot.sleep(20);",
    );
    bot.scheduler.add(&path).unwrap();
    let next = script(&dir, "next.ts", "bot.log(\"ran next\");");
    bot.scheduler.add(&next).unwrap();
    bot.scheduler.start(&bot).unwrap();
    wait_for("the script", || lines(&bot, "farming").len() == 1);

    // Auto-relogin stops the script while logged out.
    logged_in.store(false, Ordering::SeqCst);
    script::stop(&bot);
    wait_for("Disconnected", || {
        statuses(&bot)[0] == EntryStatus::Disconnected
    });
    std::thread::sleep(Duration::from_millis(200));
    assert!(!bot.scripts.is_running(), "started while logged out");
    assert_eq!(bot.scheduler.current(), Some(0));

    // Auto-relogin logs in and restarts the script: the playlist takes it over.
    logged_in.store(true, Ordering::SeqCst);
    script::start_silent(&bot).unwrap();
    wait_for("Running again", || {
        statuses(&bot)[0] == EntryStatus::Running
    });
    wait_for("the restarted script", || lines(&bot, "farming").len() == 2);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        lines(&bot, "farming").len(),
        2,
        "started twice after the login"
    );

    // Logged in, a stop completes it and the next one runs.
    script::stop(&bot);
    wait_for("the playlist", || !bot.scheduler.is_running());
    assert_eq!(
        statuses(&bot),
        [EntryStatus::Completed, EntryStatus::Completed]
    );
    assert_eq!(lines(&bot, "ran next"), ["ran next"]);
}

#[test]
fn a_disconnection_without_auto_relogin_restarts_the_script_after_the_login() {
    let dir = scripts_dir("login");
    let (bot, logged_in) = new_bot();
    let path = script(
        &dir,
        "farm.ts",
        "bot.log(\"farming\"); while (true) bot.sleep(20);",
    );
    bot.scheduler.add(&path).unwrap();
    bot.scheduler.start(&bot).unwrap();
    wait_for("the script", || lines(&bot, "farming").len() == 1);
    logged_in.store(false, Ordering::SeqCst);
    script::stop(&bot);
    wait_for("Disconnected", || {
        statuses(&bot)[0] == EntryStatus::Disconnected
    });
    logged_in.store(true, Ordering::SeqCst);
    wait_for("the restart", || lines(&bot, "farming").len() == 2);
    assert_eq!(statuses(&bot)[0], EntryStatus::Running);
    bot.scheduler.stop(&bot);
    wait_for("the script to stop", || !bot.scripts.is_running());
}

#[test]
fn queued_runs_skip_the_options_window_and_use_their_own_options() {
    let dir = scripts_dir("options");
    let (bot, _) = new_bot();
    bot.scripts.set_interactive(true);
    let path = write(
        &dir,
        "opts.ts",
        r#"import { bot, defineScriptOptions, option } from "turu";
export const scriptOptions = defineScriptOptions("SchedulerOpts", [option.int("Count", "Count", "", 1)]);
export default function main(): void {
    bot.config.configure();
    bot.log(`count=${bot.config.get<number>("Count")}`);
}
"#,
    );
    bot.scheduler.add(&path).unwrap();
    bot.scheduler.add(&path).unwrap();
    let entries = bot.scheduler.entries();
    assert_eq!(entries[0].name, "opts.ts");
    assert_eq!(entries[1].name, "opts (2).ts");
    assert_ne!(entries[0].id, entries[1].id);

    // The first copy has its own saved options; the second has none yet (the defaults), and
    // the script's own storage is left alone.
    let options = data_dir().join("options");
    std::fs::create_dir_all(&options).unwrap();
    assert_eq!(
        entries[0].storage(),
        format!("Queue_opts_{}", entries[0].id)
    );
    std::fs::write(
        options.join(format!("{}.cfg", entries[0].storage())),
        "Options:Count=7\n",
    )
    .unwrap();
    std::fs::write(options.join("SchedulerOpts.cfg"), "Options:Count=3\n").unwrap();

    bot.scheduler.start(&bot).unwrap();
    let start = Instant::now();
    while bot.scheduler.is_running() {
        assert!(
            bot.scripts.configure_request().is_none(),
            "the options window was asked for"
        );
        assert!(start.elapsed() < Duration::from_secs(15), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(lines(&bot, "count="), ["count=7", "count=1"]);
    assert_eq!(statuses(&bot), [EntryStatus::Completed; 2]);
}

#[test]
fn playlists_round_trip_in_vibeskua_format() {
    let dir = scripts_dir("playlist");
    let (bot, _) = new_bot();
    let a = script(&dir, "a.ts", "");
    let b = script(&dir, "b.ts", "");
    bot.scheduler.add(&a).unwrap();
    bot.scheduler.add(&b).unwrap();
    bot.scheduler.add(&a).unwrap();
    let ids: Vec<String> = bot
        .scheduler
        .entries()
        .iter()
        .map(|e| e.id.clone())
        .collect();
    bot.scheduler.rename(&ids[1], "Second");
    bot.scheduler.rename(&ids[1], "  ");
    bot.scheduler.move_by(&ids[2], -1).unwrap();
    bot.scheduler.move_by(&ids[0], -1).unwrap();
    let saved = bot.scheduler.entries();
    assert_eq!(
        saved.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["a.ts", "a (2).ts", "Second"]
    );

    let file = dir.join("playlist.json");
    bot.scheduler.save(&file).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(json[1]["Name"], "a (2).ts");
    assert_eq!(json[2]["Path"], b.to_string_lossy().as_ref());
    assert_eq!(json[0]["Id"], ids[0].as_str());

    let (other, _) = new_bot();
    assert_eq!(other.scheduler.load(&file).unwrap(), 3);
    assert_eq!(
        other
            .scheduler
            .entries()
            .iter()
            .map(|e| (&e.id, &e.name, &e.path))
            .collect::<Vec<_>>(),
        saved
            .iter()
            .map(|e| (&e.id, &e.name, &e.path))
            .collect::<Vec<_>>()
    );
    assert!(
        other
            .scheduler
            .entries()
            .iter()
            .all(|e| e.status == EntryStatus::Queued)
    );

    // A VibeSkua file: scripts that are gone are skipped; an empty name keeps the file name.
    std::fs::write(
        &file,
        format!(
            r#"[
  {{ "Path": {a:?}, "Id": "3f2504e0-4f89-11d3-9a0c-0305e82c3301", "Name": "" }},
  {{ "Path": "/nowhere/Gone.cs", "Id": "3f2504e0-4f89-11d3-9a0c-0305e82c3302", "Name": "Gone" }}
]"#,
            a = a.to_string_lossy()
        ),
    )
    .unwrap();
    assert_eq!(other.scheduler.load(&file).unwrap(), 1);
    let entry = &other.scheduler.entries()[0];
    assert_eq!(entry.name, "a.ts");
    assert_eq!(
        entry.storage(),
        "Queue_a_3f2504e0-4f89-11d3-9a0c-0305e82c3301"
    );

    other.scheduler.remove(&entry.id.clone()).unwrap();
    assert!(other.scheduler.entries().is_empty());
    assert!(
        other.scheduler.save(&file).is_err(),
        "saved an empty playlist"
    );
    assert!(
        other.scheduler.start(&other).is_err(),
        "started an empty playlist"
    );
}
