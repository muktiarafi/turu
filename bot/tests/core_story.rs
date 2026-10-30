//! Runs `scripts/core/CoreStory.ts` against a fake game.

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::script;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// Answers "null" to everything, like a game with nothing loaded, and records the calls.
#[derive(Default)]
struct RecordingGame {
    calls: Mutex<Vec<(String, Vec<FlashArg>)>>,
}

impl FlashHost for RecordingGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        self.calls.lock().push((name.to_owned(), args));
        Ok("null".into())
    }
}

fn run(name: &str, source: &str) -> (Arc<Bot>, Arc<RecordingGame>) {
    let dir = std::env::temp_dir().join(format!("turu-test-story-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    // The repo's scripts/core, imported by absolute path.
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/core")
        .canonicalize()
        .unwrap();
    let source = source.replace("CORE", &core.to_string_lossy());
    std::fs::write(dir.join("main.ts"), source).unwrap();

    let game = Arc::new(RecordingGame::default());
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
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
    (bot, game)
}

#[test]
fn preload_loads_the_quests_a_story_function_uses() {
    let (bot, game) = run(
        "preload",
        r#"import { bot } from "turu";
import { CoreStory } from "CORE/CoreStory";
import { Core } from "CORE/CoreBots";
const Story = new CoreStory();

function storyLine(): void {
    Story.preLoad(storyLine);
    Story.mapItemQuest(3907, "frozentower", 3022);
    Story.killQuest(3908, "frozentower", "Polar Elemental");
    if (!Story.questProgression(3911)) {
        Core.ensureAccept(3911);
    }
    Story.killQuest(6593, "stalagbite", ["Balboa", "Kuro"], { getReward: false });
    bot.log("not a quest 1234");
}

export default function main(): void {
    Core.actionDelay = 0;
    Story.preLoad(storyLine);
}
"#,
    );
    let log = bot.logs.script.text();
    assert!(log.contains("Loading 4 Quests."), "{log}");
    let shown: Vec<String> = game
        .calls
        .lock()
        .iter()
        .filter(|(name, args)| {
            name == "callGameFunction"
                && args.first() == Some(&FlashArg::String("world.showQuests".into()))
        })
        .filter_map(|(_, args)| match args.get(1) {
            Some(FlashArg::String(ids)) => Some(ids.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(shown, vec!["3907,3908,3911,6593".to_owned()], "{log}");
}
