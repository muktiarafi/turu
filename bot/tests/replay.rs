//! Replaying tapes (docs/REPLAY_TESTS_PROPOSAL.md, phase 2): a script recorded against a fake
//! game, scrubbed, then played back to the same script with no game, and the ways a replay
//! fails: a different send, a read the tape never answered, a script that stops sending, and a
//! tape that isn't scrubbed.
//!
//! The tapes here are recorded by each test, not committed: the first committed tape is recorded
//! in game (phase 2's exit).

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};
use turu_bot::player_packets::Direction;
use turu_bot::script::{self, ScriptStatus};
use turu_bot::tape;
use turu_bot::testing::{ReplayHost, ReplayOptions};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

const QUEST: i64 = 4777;

/// A game with one quest: accepting it and turning it in answer after a while, as the server
/// does, on a thread of their own. With `socket`, the player reports the socket and the replies
/// come as its messages; else as turu.swf's `pext`s.
#[derive(Default)]
struct QuestGame {
    bot: OnceLock<Weak<Bot>>,
    in_progress: Arc<Mutex<bool>>,
    socket: bool,
}

impl QuestGame {
    /// In `ms`, sets the quest's state and delivers `reply` as the game would.
    fn later(&self, ms: u64, in_progress: bool, reply: Value) {
        let bot = self.bot.get().cloned().expect("bot");
        let state = self.in_progress.clone();
        let socket = self.socket;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(ms));
            *state.lock() = in_progress;
            let Some(bot) = bot.upgrade() else { return };
            if socket {
                let message = json!({"t": "xt", "b": {"r": -1, "o": reply}});
                bot.on_socket(1, Direction::Received, format!("{message}\0").as_bytes());
            } else {
                let pext = json!({"params": {"type": "json", "dataObj": reply}});
                bot.on_flash_call("pext", vec![Value::String(pext.to_string())]);
            }
        });
    }
}

impl FlashHost for QuestGame {
    fn player_packets(&self) -> bool {
        self.socket
    }

    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        if let Some(answer) =
            turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default())
        {
            return Ok(answer);
        }
        let arg = |i: usize| match args.get(i) {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        Ok(match (name, arg(0).as_str()) {
            ("isLoggedIn", _) => "true".into(),
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => "1".into(),
            ("getGameObject", "world.strMapName") => r#""yulgar""#.into(),
            ("getGameObject", path) if path.starts_with("world.lock.") => {
                r#"{"ts":0,"cd":0}"#.into()
            }
            ("getGameObjectS", "loginInfo.strUsername") => r#""Quietfern""#.into(),
            ("callGameFunction", "world.isQuestInProgress") => self.in_progress.lock().to_string(),
            ("callGameFunction", "world.acceptQuest") => {
                self.later(
                    80,
                    true,
                    json!({"cmd": "acceptQuest", "QuestID": QUEST, "bSuccess": 1}),
                );
                "null".into()
            }
            ("callGameFunction", "world.tryQuestComplete") => {
                self.later(
                    150,
                    false,
                    json!({"cmd": "ccqr", "QuestID": QUEST, "bSuccess": 1}),
                );
                "null".into()
            }
            _ => "null".into(),
        })
    }
}

/// A folder of its own for `name`.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("turu-test-replay-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The test script: accepts and turns in the quest three times, counting the game's replies,
/// then goes to its own account by name (a send carrying the name the scrubber replaces).
/// `quest` is the ID it uses.
fn script_source(quest: i64, extra: &str) -> String {
    format!(
        r#"import {{ bot }} from "turu";

export default function main(): void {{
    let replies = 0;
    bot.events.on("extensionPacket", (packet) => {{
        if (packet?.params?.dataObj?.cmd === "ccqr") replies++;
    }});
    for (let i = 0; i < 3; i++) {{
        bot.quests.ensureAccept({quest});
        bot.quests.ensureComplete({quest});
    }}
    bot.flash.callGameFunction("world.goto", bot.player.username);
    bot.sleep(300);
    {extra}
    bot.log(`Turned in 3 times, ${{replies}} replies`);
}}
"#
    )
}

fn new_bot() -> Arc<Bot> {
    crate::common::data_dir();
    let bot = Bot::new();
    {
        let mut options = bot.options.write();
        options.action_delay = 100;
        options.quest_accept_and_complete_tries = 3;
    }
    bot
}

/// Runs `source` from `dir` to its end, failing the test after `limit`.
fn run_script(bot: &Arc<Bot>, dir: &Path, source: &str, limit: Duration) {
    let path = dir.join("main.ts");
    std::fs::write(&path, source).unwrap();
    bot.scripts.load(&path);
    script::start_silent(bot).unwrap();
    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < limit,
            "script didn't finish: {}",
            bot.logs.script.text()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Records the test script against [`QuestGame`] and scrubs the tape: its path.
fn record(dir: &Path) -> PathBuf {
    record_with(dir, false)
}

fn record_with(dir: &Path, socket: bool) -> PathBuf {
    let bot = new_bot();
    let game = Arc::new(QuestGame {
        socket,
        ..QuestGame::default()
    });
    game.bot.set(Arc::downgrade(&bot)).ok();
    bot.set_host(Some(game));
    let raw = bot
        .start_tape(&dir.join("raw.tape.jsonl.gz"), true)
        .unwrap();
    run_script(
        &bot,
        dir,
        &script_source(QUEST, ""),
        Duration::from_secs(20),
    );
    assert_eq!(
        bot.scripts.status(),
        ScriptStatus::Loaded,
        "{}",
        bot.logs.script.text()
    );
    assert!(
        bot.logs.script.text().contains("3 replies"),
        "{}",
        bot.logs.script.text()
    );
    bot.stop_tape();
    let scrubbed = dir.join("quest.tape.jsonl.gz");
    tape::scrub_file(&raw, &scrubbed).unwrap();
    scrubbed
}

/// Replays `tape` to `source`: the bot and the host.
fn replay(
    tape: &Path,
    dir: &Path,
    source: &str,
    options: ReplayOptions,
) -> (Arc<Bot>, Arc<ReplayHost>) {
    let bot = new_bot();
    // A recorded run logged in with `--user`/`--password`, which relogins use: a made-up one
    // stands in (the tape's login arguments are redacted and match anything).
    turu_bot::api::servers::set_login_info(&bot, "Questwalker", "not-a-password");
    let host = ReplayHost::open(tape, options).unwrap();
    host.attach(&bot);
    let path = dir.join("replayed.ts");
    std::fs::write(&path, source).unwrap();
    bot.scripts.load(&path);
    script::start_silent(&bot).unwrap();
    (bot, host)
}

fn fast() -> ReplayOptions {
    ReplayOptions {
        delay_scale: 0.5,
        stall: Duration::from_secs(3),
        ..ReplayOptions::default()
    }
}

#[test]
fn a_recorded_quest_loop_replays_with_no_game() {
    let dir = temp("same");
    let tape = record(&dir);
    let (bot, host) = replay(&tape, &dir, &script_source(QUEST, ""), fast());
    let report = host
        .wait(&bot, Duration::from_secs(30))
        .unwrap_or_else(|e| panic!("{e}"));
    // Three accepts and three turn-ins, at least.
    assert!(report.sends >= 6, "{report:?}");
    assert!(report.missing_reads.is_empty(), "{report:?}");
    assert_eq!(report.status, ScriptStatus::Loaded);
    // The game's replies were delivered from the tape: the script counted them.
    let log = bot.logs.script.text();
    assert!(log.contains("Turned in 3 times, 3 replies"), "{log}");
}

/// The account's name, read and sent, is the scrubbed one in both: the scrubber renamed it in
/// the read's answer and in the send alike.
#[test]
fn the_scrubbed_name_is_read_and_sent_alike() {
    let dir = temp("names");
    let tape = record(&dir);
    let text = format!("{:?}", tape::read(&tape).unwrap().lines);
    assert!(!text.contains("Quietfern"), "{text}");
    assert!(
        text.contains(r#"String("world.goto"), String("Questwalker")"#),
        "{text}"
    );
    let (bot, host) = replay(&tape, &dir, &script_source(QUEST, ""), fast());
    host.wait(&bot, Duration::from_secs(30))
        .unwrap_or_else(|e| panic!("{e}"));
}

/// A tape where the player reported the socket: the replies are its messages, delivered back
/// through `on_socket`.
#[test]
fn a_tape_of_the_socket_replays() {
    let dir = temp("socket");
    let tape = record_with(&dir, true);
    let read = tape::read(&tape).unwrap();
    let received = read
        .lines
        .iter()
        .filter(|l| matches!(l, tape::Line::Received { text, .. } if text.contains("ccqr")))
        .count();
    assert_eq!(received, 3);
    let (bot, host) = replay(&tape, &dir, &script_source(QUEST, ""), fast());
    host.wait(&bot, Duration::from_secs(30))
        .unwrap_or_else(|e| panic!("{e}"));
    let log = bot.logs.script.text();
    assert!(log.contains("Turned in 3 times, 3 replies"), "{log}");
}

#[test]
fn a_different_quest_id_fails_naming_both() {
    let dir = temp("other-id");
    let tape = record(&dir);
    let (bot, host) = replay(&tape, &dir, &script_source(4778, ""), fast());
    let failure = host
        .wait(&bot, Duration::from_secs(30))
        .expect_err("the replay should fail");
    let message = failure.to_string();
    assert!(message.contains("expected: "), "{message}");
    assert!(message.contains("4777"), "{message}");
    assert!(message.contains("4778"), "{message}");
    assert!(message.contains("world.acceptQuest"), "{message}");
    assert!(message.contains("the tape before it"), "{message}");
    assert!(!bot.scripts.is_running());
}

#[test]
fn a_read_the_tape_never_answered_fails_in_strict_mode() {
    let dir = temp("missing-read");
    let tape = record(&dir);
    let source = script_source(
        QUEST,
        r#"bot.log(String(bot.flash.get("world.notOnTheTape")));"#,
    );
    let (bot, host) = replay(&tape, &dir, &source, fast());
    let failure = host
        .wait(&bot, Duration::from_secs(30))
        .expect_err("the replay should fail");
    let message = failure.to_string();
    assert!(
        message.contains("a read the tape never answered"),
        "{message}"
    );
    assert!(message.contains("world.notOnTheTape"), "{message}");
}

#[test]
fn a_missing_read_answers_null_when_not_strict() {
    let dir = temp("lenient");
    let tape = record(&dir);
    let source = script_source(
        QUEST,
        r#"bot.log(String(bot.flash.get("world.notOnTheTape")));"#,
    );
    let options = ReplayOptions {
        strict: false,
        ..fast()
    };
    let (bot, host) = replay(&tape, &dir, &source, options);
    let report = host
        .wait(&bot, Duration::from_secs(30))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        report
            .missing_reads
            .iter()
            .any(|k| k.contains("world.notOnTheTape")),
        "{report:?}"
    );
}

#[test]
fn a_script_that_stops_sending_is_stuck() {
    let dir = temp("stuck");
    let tape = record(&dir);
    // Waits where the tape turned the quest in.
    let source = r#"import { bot } from "turu";
export default function main(): void {
    bot.quests.ensureAccept(4777);
    while (!bot.shouldExit) { bot.sleep(100); }
}
"#;
    let options = ReplayOptions {
        stall: Duration::from_millis(500),
        ..fast()
    };
    let (bot, host) = replay(&tape, &dir, source, options);
    let failure = host
        .wait(&bot, Duration::from_secs(30))
        .expect_err("the replay should fail");
    let message = failure.to_string();
    assert!(message.contains("the script is stuck"), "{message}");
    assert!(message.contains("world.tryQuestComplete"), "{message}");
    assert!(!bot.scripts.is_running());
}

#[test]
fn a_script_that_ends_early_didnt_reach_the_end() {
    let dir = temp("early");
    let tape = record(&dir);
    let source = r#"import { bot } from "turu";
export default function main(): void {
    bot.quests.ensureAccept(4777);
}
"#;
    let (bot, host) = replay(&tape, &dir, source, fast());
    let failure = host
        .wait(&bot, Duration::from_secs(30))
        .expect_err("the replay should fail");
    assert!(failure.to_string().contains("before the tape"), "{failure}");
}

#[test]
fn an_unscrubbed_tape_is_refused() {
    let dir = temp("unscrubbed");
    record(&dir);
    let refused = ReplayHost::open(&dir.join("raw.tape.jsonl.gz"), ReplayOptions::default())
        .err()
        .expect("refused");
    assert!(refused.contains("scrub"), "{refused}");
    // A test's own recording may say so.
    let options = ReplayOptions {
        allow_unscrubbed: true,
        ..ReplayOptions::default()
    };
    assert!(ReplayHost::open(&dir.join("raw.tape.jsonl.gz"), options).is_ok());
}

// ---- Tapes recorded in game (bot/tests/tapes) ----------------------------------------------

/// The committed tapes and their scripts.
fn tapes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tapes")
}

/// Replays a committed tape to `source` (its script, or a changed copy) at a quarter of its
/// events' delays.
fn replay_committed(name: &str, source: &str) -> (Arc<Bot>, Arc<ReplayHost>) {
    replay_committed_with(name, source, ReplayOptions::default())
}

/// [`replay_committed`] with `options` (a tape's noise), at a quarter of its events' delays.
fn replay_committed_with(
    name: &str,
    source: &str,
    options: ReplayOptions,
) -> (Arc<Bot>, Arc<ReplayHost>) {
    let dir = temp(name);
    let options = ReplayOptions {
        delay_scale: 0.25,
        ..options
    };
    replay(
        &tapes_dir().join(format!("{name}.tape.jsonl.gz")),
        &dir,
        source,
        options,
    )
}

/// A committed tape's script, its imports of the repo's scripts made absolute (the replay loads
/// it from a folder of its own).
fn committed_script(name: &str) -> String {
    let scripts = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts")
        .canonicalize()
        .unwrap();
    std::fs::read_to_string(tapes_dir().join(format!("{name}.ts")))
        .unwrap()
        .replace("\"../../../scripts/", &format!("\"{}/", scripts.display()))
}

/// Quest 8543 in /chute, accepted, its map item picked up and turned in three times, recorded
/// on a spare account on a quiet server (twig).
#[test]
fn the_chute_quest_tape_replays() {
    let (bot, host) = replay_committed("chute_quest", &committed_script("chute_quest"));
    let report = host
        .wait(&bot, Duration::from_secs(90))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.status, ScriptStatus::Loaded, "{report:?}");
    let log = bot.logs.script.text();
    assert!(log.contains("Turn-in 3: done"), "{log}");
}

/// Replays a committed tape to its own script: it must reach the tape's end with `logged` in the
/// script log.
fn replays_to_its_end(name: &str, logged: &[&str]) {
    replays_to_its_end_with(name, logged, ReplayOptions::default());
}

fn replays_to_its_end_with(
    name: &str,
    logged: &[&str],
    options: ReplayOptions,
) -> turu_bot::testing::ReplayReport {
    let (bot, host) = replay_committed_with(name, &committed_script(name), options);
    // Replays run at the recorded pace but for the events' delays: a combat tape takes about as
    // long as it was recorded (decision 2, a virtual clock, would make it instant).
    let report = host
        .wait(&bot, Duration::from_secs(200))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.status, ScriptStatus::Loaded, "{report:?}");
    let log = bot.logs.script.text();
    for line in logged {
        assert!(log.contains(line), "{line} isn't in the log:\n{log}");
    }
    report
}

/// CoreAdvanced's `enhanceEquipped`, Lucky, on a level-16 character's class gear: the real
/// enhancement shops' items and the `enhanceItemShop` packets.
#[test]
fn the_enhancement_tape_replays() {
    replays_to_its_end(
        "enhance_equipped",
        &[
            "Before: Healer 1, Battle Oracle Hood 12, Battle Oracle Battlestaff 9, Battle Oracle Wings 11",
            "After: Healer 14, Battle Oracle Hood 16, Battle Oracle Battlestaff 13, Battle Oracle Wings 15",
        ],
    );
}

/// CoreBots' `toBank` and `unbank` on a resource, there and back.
#[test]
fn the_bank_tape_replays() {
    replays_to_its_end(
        "bank_moves",
        &[
            "Banked: in the bank true, in the inventory false",
            "Unbanked: in the bank false, in the inventory true",
        ],
    );
}

/// The engine's relogin back onto the quiet server it logged into: the login screen, the login
/// (its arguments redacted, matching anything), the server list and the connect.
#[test]
fn the_relogin_tape_replays() {
    replays_to_its_end(
        "relogin",
        &[
            "Before: logged in true, map battleon",
            "Relogin done: logged in true, map battleon",
        ],
    );
}

/// CoreStory's `mapItemQuest` then `killQuest` (the first two Dark Dungeon quests): the first
/// tape with combat.
#[test]
fn the_core_story_tape_replays() {
    let report = replays_to_its_end_with(
        "core_story",
        &[
            "Quest 3538: unlocked true, done false",
            "Quest 3539: unlocked true, done false",
            "After: 3538 true, 3539 true",
        ],
        ReplayOptions::default(),
    );
    // The skill timer cast (unchecked: which skill, and when, follows its cooldowns' timing).
    assert!(report.unchecked_sends > 0, "{report:?}");
}

/// A kill farm with drops on a high-level character: CoreBots' `huntMonster` for three more
/// Bone Dust, its drop handling picking them up.
#[test]
fn the_kill_farm_tape_replays() {
    let report = replays_to_its_end_with(
        "kill_farm",
        &[
            "Start: level 100, Bone Dust 1163",
            "Farmed: Bone Dust 1163 to 1171",
        ],
        ReplayOptions::default(),
    );
    assert!(report.unchecked_sends > 0, "{report:?}");
}

/// The merge template on SynderesMerge, buying only "NO BOTS Armor": the missing souvenir handed
/// to `findIngredients`, farmed through a registered quest, then the buy.
#[test]
fn the_merge_tape_replays() {
    replays_to_its_end(
        "merge_buy",
        &[
            "Before: NO BOTS Armor false, gold 93481788",
            "Bought 1 NO BOTS Armor",
            "After: NO BOTS Armor true, gold 93481663",
        ],
    );
}

#[test]
fn the_chute_quest_tape_fails_on_another_quest_id() {
    let source = committed_script("chute_quest").replace("QUEST = 8543", "QUEST = 8544");
    let (bot, host) = replay_committed("chute_quest", &source);
    let message = host
        .wait(&bot, Duration::from_secs(90))
        .expect_err("the replay should fail")
        .to_string();
    assert!(message.contains("8543"), "{message}");
    assert!(message.contains("8544"), "{message}");
}

/// Every committed tape is scrubbed and holds nothing redaction would still take out: no
/// password, token, login message, email, address or chat (docs/REPLAY_TESTS_PROPOSAL.md, Risks).
#[test]
fn committed_tapes_hold_no_secrets() {
    let mut tapes = 0;
    for entry in std::fs::read_dir(tapes_dir()).unwrap() {
        let path = entry.unwrap().path();
        if !path.to_string_lossy().ends_with(tape::EXTENSION) {
            continue;
        }
        tapes += 1;
        let read = tape::read(&path).unwrap();
        assert!(read.header.scrubbed, "{} isn't scrubbed", path.display());
        assert!(
            read.is_complete() && !read.truncated,
            "{} isn't whole",
            path.display()
        );
        for line in &read.lines {
            let mut again = line.clone();
            tape::redact::line(&mut again);
            assert_eq!(&again, line, "{} keeps a secret", path.display());
        }
    }
    assert!(tapes > 0, "no tapes in {}", tapes_dir().display());
}
