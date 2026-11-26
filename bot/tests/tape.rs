//! Recording tapes (docs/REPLAY_TESTS_PROPOSAL.md, phase 1): what is recorded and when, what
//! never is (passwords, tokens, chat), and tapes cut short by a killed process.

use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::player_packets::Direction;
use turu_bot::script::{self, ScriptStatus};
use turu_bot::tape::{self, Line, redact};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A game that answers a few reads as the real one would, the password among them.
struct FakeGame {
    player_packets: bool,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        if let Some(answer) =
            turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default())
        {
            return Ok(answer);
        }
        let path = match args.first() {
            Some(FlashArg::String(s)) => s.as_str(),
            _ => "",
        };
        Ok(match (name, path) {
            ("getGameObject", "world.strMapName") => r#""yulgar""#.into(),
            ("getGameObjectS", "loginInfo.strPassword") => r#""hunter2""#.into(),
            ("getGameObjectS", "loginInfo.strUsername") => r#""Questwalker""#.into(),
            ("getGameObject", "slow.answer") => {
                std::thread::sleep(Duration::from_millis(200));
                "1".into()
            }
            ("getGameObject", "never.answers") => {
                std::thread::sleep(Duration::from_millis(1500));
                "1".into()
            }
            ("getGameObjectS", "objLogin") => {
                json!({"sToken": "tok-123", "unm": "Questwalker"}).to_string()
            }
            _ => "null".into(),
        })
    }

    fn player_packets(&self) -> bool {
        self.player_packets
    }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("turu-test-tape-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn bot(player_packets: bool) -> Arc<Bot> {
    crate::common::data_dir();
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(FakeGame { player_packets })));
    bot
}

fn pext(cmd: &str) -> Vec<Value> {
    let packet =
        json!({"params": {"type": "json", "dataObj": {"cmd": cmd, "strMapName": "yulgar"}}});
    vec![Value::String(packet.to_string())]
}

/// The tape's lines that are calls named `name` with `path` first.
fn calls<'a>(lines: &'a [Line], name: &str, path: &str) -> Vec<&'a Line> {
    lines
        .iter()
        .filter(|l| matches!(l, Line::Call { name: n, args, .. } if n == name && args.first() == Some(&json!(path))))
        .collect()
}

/// The whole tape, decompressed, as text.
fn raw(path: &Path) -> String {
    let mut text = String::new();
    flate2::read::MultiGzDecoder::new(std::fs::File::open(path).unwrap())
        .read_to_string(&mut text)
        .unwrap();
    text
}

#[test]
fn records_once_the_player_joins_a_map() {
    let dir = temp("world");
    let bot = bot(false);
    let path = bot
        .start_tape(&dir.join("run.tape.jsonl.gz"), false)
        .unwrap();

    bot.flash.get::<String>("world.strMapName").unwrap();
    bot.on_flash_call("pext", pext("moveToArea"));
    bot.flash.get::<String>("world.strMapName").unwrap();
    bot.on_flash_call("packet", vec![json!("%xt%zm%moveToCell%1%Enter%Spawn%")]);
    assert_eq!(bot.stop_tape().as_deref(), Some(path.as_path()));

    let tape = tape::read(&path).unwrap();
    assert!(tape.header.from_world);
    assert!(tape.is_complete(), "{tape:?}");
    assert!(!tape.truncated);
    // The read before the map isn't there; the one after is, with its answer.
    let reads = calls(&tape.lines, "getGameObject", "world.strMapName");
    assert_eq!(reads.len(), 1, "{:?}", tape.lines);
    assert!(matches!(reads[0], Line::Call { answer: Some(a), .. } if a == r#""yulgar""#));
    // The `pext` that joined the map is the first line; the call and the packet follow in order.
    let first = &tape.lines[0];
    assert!(
        matches!(first, Line::Event { name, .. } if name == "pext"),
        "{first:?}"
    );
    let sent = tape
        .lines
        .iter()
        .position(|l| matches!(l, Line::Event { name, .. } if name == "packet"))
        .expect("the packet");
    let read = tape.lines.iter().position(|l| l == reads[0]).unwrap();
    assert!(read < sent);
}

#[test]
fn records_the_socket_from_the_start() {
    let dir = temp("socket");
    let bot = bot(true);
    let path = bot
        .start_tape(&dir.join("run.tape.jsonl.gz"), true)
        .unwrap();

    bot.on_socket(1, Direction::Sent, b"%xt%zm%moveToCell%1%Enter%Spawn%\0");
    bot.on_socket(
        1,
        Direction::Received,
        br#"{"t":"xt","b":{"r":-1,"o":{"cmd":"ct"}}}"#,
    );
    bot.on_socket(1, Direction::Received, b"\0");
    bot.stop_tape();

    let tape = tape::read(&path).unwrap();
    assert!(!tape.header.from_world);
    let socket: Vec<_> = tape
        .lines
        .iter()
        .filter_map(|l| match l {
            Line::Sent { text, .. } => Some(format!("out {text}")),
            Line::Received { text, .. } => Some(format!("in {text}")),
            _ => None,
        })
        .collect();
    assert_eq!(
        socket,
        [
            "out %xt%zm%moveToCell%1%Enter%Spawn%",
            r#"in {"t":"xt","b":{"r":-1,"o":{"cmd":"ct"}}}"#
        ]
    );
}

#[test]
fn passwords_tokens_logins_and_chat_are_never_written() {
    let dir = temp("secrets");
    let bot = bot(true);
    let path = bot
        .start_tape(&dir.join("run.tape.jsonl.gz"), true)
        .unwrap();

    // A relogin: the game's login call, the password read, the socket's login with its token.
    bot.flash
        .call_game_function("login", vec!["Questwalker".into(), "hunter2".into()])
        .unwrap();
    bot.flash
        .get_static::<String>("loginInfo.strPassword")
        .unwrap();
    bot.flash
        .get_static::<String>("loginInfo.strUsername")
        .unwrap();
    bot.flash.get_static::<Value>("objLogin").unwrap();
    bot.flash
        .set("loginInfo.strPassword", FlashArg::from("hunter2"))
        .unwrap();
    bot.on_socket(
        1,
        Direction::Sent,
        b"<msg t='sys'><body action='login' r='0'><login z='zone_master'><nick><![CDATA[Questwalker~tok-123]]></nick><pword><![CDATA[tok-123]]></pword></login></body></msg>\0",
    );
    // Chat both ways, as packets and as a `pext`.
    bot.on_socket(
        1,
        Direction::Sent,
        b"%xt%zm%message%1%meet at yulgar%zone%\0",
    );
    bot.on_socket(
        1,
        Direction::Sent,
        b"%xt%zm%whisper%1%secret plans%Player Two%\0",
    );
    bot.on_socket(
        1,
        Direction::Received,
        b"%xt%chatm%-1%zone~meet at yulgar%Player Two%\0",
    );
    let whisper = json!({"params": {"type": "str", "dataObj": ["whisper", "-1", "secret plans", "Player Two"]}});
    bot.on_flash_call("pext", vec![Value::String(whisper.to_string())]);
    bot.stop_tape();

    let text = raw(&path);
    for secret in ["hunter2", "tok-123", "meet at yulgar", "secret plans"] {
        assert!(!text.contains(secret), "{secret} is on the tape:\n{text}");
    }
    let tape = tape::read(&path).unwrap();
    let login = calls(&tape.lines, "callGameFunction", "login");
    assert!(
        matches!(login[0], Line::Call { args, .. } if args[1] == redact::REDACTED && args[2] == redact::REDACTED),
        "{login:?}"
    );
    // The username isn't a secret (the scrubber renames players later).
    let username = calls(&tape.lines, "getGameObjectS", "loginInfo.strUsername");
    assert!(matches!(username[0], Line::Call { answer: Some(a), .. } if a == r#""Questwalker""#));
    let login_info = calls(&tape.lines, "getGameObjectS", "objLogin");
    assert!(
        matches!(login_info[0], Line::Call { answer: Some(a), .. } if a.contains("Questwalker") && a.contains(redact::REDACTED)),
        "{login_info:?}"
    );
    assert!(
        tape.lines
            .iter()
            .any(|l| matches!(l, Line::Sent { text, .. } if text == redact::REDACTED_LOGIN))
    );
    let chat = tape
        .lines
        .iter()
        .filter(|l| match l {
            Line::Sent { text, .. } | Line::Received { text, .. } => text == redact::REDACTED_CHAT,
            Line::Event { args, .. } => args[0] == redact::REDACTED_CHAT,
            _ => false,
        })
        .count();
    assert_eq!(chat, 4);
}

/// Every `loginInfo` field the engine reads, but the username, counts as a secret: a new read
/// of one can't reach a tape by mistake.
#[test]
fn every_login_info_read_in_the_engine_is_redacted() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for (i, _) in text.match_indices("\"loginInfo.") {
                    let rest = &text[i + 1..];
                    let end = rest.find('"').unwrap();
                    found.push(rest[..end].to_owned());
                }
            }
        }
    }
    assert!(
        found.iter().any(|p| p == "loginInfo.strPassword"),
        "{found:?}"
    );
    for path in found.iter().filter(|p| *p != "loginInfo.strUsername") {
        assert!(
            redact::is_secret_path(path),
            "{path} would be written to tapes"
        );
    }
}

#[test]
fn a_killed_recording_reads_up_to_its_last_flush() {
    let dir = temp("killed");
    let bot = bot(false);
    let path = bot
        .start_tape(&dir.join("run.tape.jsonl.gz"), true)
        .unwrap();
    bot.on_flash_call("pext", pext("moveToArea"));
    bot.flash.get::<String>("world.strMapName").unwrap();
    // The writer flushes every second; copy the file as a killed process would leave it.
    let copy = dir.join("copy.tape.jsonl.gz");
    let start = Instant::now();
    let tape = loop {
        std::thread::sleep(Duration::from_millis(200));
        std::fs::copy(&path, &copy).unwrap();
        if let Ok(tape) = tape::read(&copy)
            && !calls(&tape.lines, "getGameObject", "world.strMapName").is_empty()
        {
            break tape;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "nothing flushed");
    };
    bot.stop_tape();
    assert!(tape.end.is_none());
    assert!(tape.truncated);
    assert!(!tape.is_complete());
    // The real one, stopped cleanly, has its end.
    let whole = tape::read(&path).unwrap();
    assert!(whole.is_complete() && !whole.truncated, "{whole:?}");
}

#[test]
fn files_are_never_written_over_and_folders_get_one_per_tape() {
    let dir = temp("names");
    let target = dir.join("run.tape.jsonl.gz");
    std::fs::write(&target, "keep me").unwrap();
    let bot = bot(false);
    let first = bot.start_tape(&target, true).unwrap();
    assert_eq!(first, dir.join("run-2.tape.jsonl.gz"));
    // A second recording replaces the first, which is finished.
    let folder = dir.join("tapes");
    let second = bot.start_tape(&folder.join(""), true).unwrap();
    assert_eq!(second.parent(), Some(folder.as_path()));
    assert!(second.to_string_lossy().ends_with(tape::EXTENSION));
    bot.stop_tape();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
    assert!(tape::read(&first).unwrap().is_complete());
    assert!(tape::read(&second).unwrap().is_complete());
}

#[test]
fn notes_say_which_script_ran_and_how_it_ended() {
    let dir = temp("script");
    std::fs::write(
        dir.join("main.ts"),
        "import { bot } from \"turu\";\nexport default function main(): void { bot.log(\"hi\"); }\n",
    )
    .unwrap();
    let bot = bot(false);
    let path = bot
        .start_tape(&dir.join("run.tape.jsonl.gz"), true)
        .unwrap();
    bot.scripts.load(&dir.join("main.ts"));
    script::start_silent(&bot).unwrap();
    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "script didn't finish"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(bot.scripts.status(), ScriptStatus::Loaded);
    bot.stop_tape();

    let tape = tape::read(&path).unwrap();
    let notes: Vec<_> = tape
        .lines
        .iter()
        .filter_map(|l| match l {
            Line::Note { what, detail, .. } if what.starts_with("script") => {
                Some((what.as_str(), detail))
            }
            _ => None,
        })
        .collect();
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert_eq!(notes[0].0, "scriptStarted");
    // Outside a scripts folder only its name: never the whole path.
    assert_eq!(notes[0].1["script"], "main.ts");
    assert_eq!(notes[0].1["silent"], true);
    assert_eq!(notes[1].0, "scriptStopped");
    assert_eq!(notes[1].1["finished"], true);
}

#[test]
fn calls_answered_while_stopping_are_kept_and_later_ones_listed() {
    let dir = temp("in-flight");
    let bot = bot(false);
    let path = bot
        .start_tape(&dir.join("run.tape.jsonl.gz"), true)
        .unwrap();
    let call = |path: &'static str| {
        let bot = bot.clone();
        std::thread::spawn(move || bot.flash.get::<i64>(path).unwrap())
    };
    let slow = call("slow.answer");
    let never = call("never.answers");
    std::thread::sleep(Duration::from_millis(50));
    // An event while both wait: its number is after theirs.
    bot.on_flash_call("event", vec![json!("cellChanged")]);
    bot.stop_tape();
    slow.join().unwrap();
    never.join().unwrap();

    let tape = tape::read(&path).unwrap();
    // The stop waited for the slow answer; the other is listed, not a gap.
    assert_eq!(
        calls(&tape.lines, "getGameObject", "slow.answer").len(),
        1,
        "{tape:?}"
    );
    assert!(calls(&tape.lines, "getGameObject", "never.answers").is_empty());
    assert_eq!(tape.end.as_ref().unwrap().unanswered.len(), 1, "{tape:?}");
    assert!(tape.is_complete(), "{tape:?}");
}

#[test]
fn the_host_is_noted_when_recording_opens() {
    let dir = temp("host-info");
    let bot = bot(false);
    let path = bot
        .start_tape(&dir.join("run.tape.jsonl.gz"), false)
        .unwrap();
    bot.on_flash_call("pext", pext("moveToArea"));
    bot.stop_tape();
    let tape = tape::read(&path).unwrap();
    // A fake game has no `hostInfo`: the note says so (null) rather than being left out.
    assert!(
        tape.lines
            .iter()
            .any(|l| matches!(l, Line::Note { what, detail, .. } if what == "hostInfo" && detail.is_null())),
        "{tape:?}"
    );
}

#[test]
fn a_script_under_a_scripts_folder_is_named_from_there() {
    let dir = temp("scripts-folder");
    let script = dir.join("scripts/farm/Gold.ts");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::write(&script, "export default function main(): void {}\n").unwrap();
    let bot = bot(false);
    let path = bot
        .start_tape(&dir.join("run.tape.jsonl.gz"), true)
        .unwrap();
    bot.scripts.load(&script);
    script::start_silent(&bot).unwrap();
    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "script didn't finish"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    bot.stop_tape();
    let tape = tape::read(&path).unwrap();
    let started = tape
        .lines
        .iter()
        .find_map(|l| match l {
            Line::Note { what, detail, .. } if what == "scriptStarted" => Some(detail.clone()),
            _ => None,
        })
        .expect("scriptStarted");
    assert_eq!(started["script"], "farm/Gold.ts");
}
