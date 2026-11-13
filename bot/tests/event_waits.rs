//! Waits that wake on turu.swf's events and the game's packets (docs/HOST_SWF_PROPOSAL.md M5),
//! against a fake game: they wake early, still answer from the game, keep their timeouts, and
//! the perf log says how they went.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::api::wait;
use turu_bot::control::{ScriptControl, Topics, mark_script_thread};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A logged-in player whose values a test changes.
#[derive(Default)]
struct FakeGame {
    values: Mutex<HashMap<String, String>>,
}

impl FakeGame {
    fn set(&self, path: &str, value: &str) {
        self.values.lock().insert(path.into(), value.into());
    }
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        if let Some(answer) =
            turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default())
        {
            return Ok(answer);
        }
        let path = match args.first() {
            Some(FlashArg::String(s)) => s.clone(),
            _ => String::new(),
        };
        let get = |path: &str| self.values.lock().get(path).cloned();
        Ok(match name {
            "isLoggedIn" => "true".into(),
            "isNull" => "false".into(),
            "getGameObject" => get(&path).unwrap_or_else(|| match path.as_str() {
                "world.myAvatar.dataLeaf.intState" => "1".into(),
                _ => "null".into(),
            }),
            _ => "null".into(),
        })
    }
}

fn bot() -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let game = Arc::new(FakeGame::default());
    game.set("world.strFrame", "\"Enter\"");
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    (bot, game)
}

/// After `delay`, on another thread, runs `then`.
fn later(delay: Duration, then: impl FnOnce() + Send + 'static) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        then();
    })
}

fn event(bot: &Arc<Bot>, name: &str, data: Value) {
    bot.on_flash_call("event", vec![json!(name), json!(data.to_string())]);
}

#[test]
fn a_fire_between_the_look_and_the_sleep_isnt_lost() {
    let control = ScriptControl::default();
    let mark = control.mark();
    control.fire(Topics::CELL);
    let started = Instant::now();
    let woken = control
        .sleep_until(
            started + Duration::from_secs(5),
            Topics::CELL | Topics::MAP,
            &mark,
        )
        .unwrap();
    assert!(woken);
    assert!(started.elapsed() < Duration::from_millis(50));
    // Another topic doesn't wake it.
    let mark = control.mark();
    control.fire(Topics::SIGNALS);
    let woken = control
        .sleep_until(
            Instant::now() + Duration::from_millis(100),
            Topics::CELL,
            &mark,
        )
        .unwrap();
    assert!(!woken);
    assert!(control.fired_since(&mark, Topics::SIGNALS));
}

#[test]
fn stop_still_wakes_a_script_waiting_on_events() {
    let control = Arc::new(ScriptControl::default());
    let stopper = control.clone();
    let stop = later(Duration::from_millis(100), move || stopper.request_stop());
    mark_script_thread(true);
    let mark = control.mark();
    let started = Instant::now();
    let result = control.sleep_until(started + Duration::from_secs(5), Topics::MAP, &mark);
    mark_script_thread(false);
    stop.join().unwrap();
    assert!(
        matches!(result, Err(turu_bot::BotError::Stopped)),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn a_buy_wait_returns_with_its_packet() {
    let (bot, _game) = bot();
    let packets = bot.clone();
    let sent = Arc::new(Mutex::new(None));
    let at = sent.clone();
    let buy = later(Duration::from_millis(150), move || {
        *at.lock() = Some(Instant::now());
        let packet = json!({ "params": { "type": "json", "dataObj": { "cmd": "buyItem", "bitSuccess": 1 } } });
        packets.on_flash_call("pext", vec![Value::String(packet.to_string())]);
    });
    assert!(wait::for_item_buy(&bot, 20).unwrap());
    let returned = Instant::now();
    buy.join().unwrap();
    let late = returned - sent.lock().unwrap();
    assert!(
        late < Duration::from_millis(10),
        "{late:?} after the packet"
    );
}

#[test]
fn a_cell_wait_wakes_on_the_event_and_answers_from_the_game() {
    let (bot, game) = bot();
    // The event alone isn't the answer: the cell hasn't changed yet.
    let early = bot.clone();
    let spurious = later(Duration::from_millis(20), move || {
        event(&early, "cellChanged", json!({ "cell": "Enter" }))
    });
    let (moved_bot, moved_game) = (bot.clone(), game.clone());
    let moved = Arc::new(Mutex::new(None));
    let at = moved.clone();
    // Between two of the wait's 100 ms looks.
    let walk = later(Duration::from_millis(150), move || {
        moved_game.set("world.strFrame", "\"r2\"");
        *at.lock() = Some(Instant::now());
        event(&moved_bot, "cellChanged", json!({ "cell": "r2" }));
    });
    assert!(wait::for_cell_change(&bot, "r2").unwrap());
    let returned = Instant::now();
    spurious.join().unwrap();
    walk.join().unwrap();
    let late = returned - moved.lock().unwrap();
    assert!(late < Duration::from_millis(40), "{late:?} after the event");
}

#[test]
fn an_event_wake_doesnt_count_as_a_try() {
    let (bot, _game) = bot();
    // Events every 20 ms for a cell it never reaches: the 25 tries still take ~2.5 s.
    let firing = bot.clone();
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop = done.clone();
    let noise = std::thread::spawn(move || {
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            event(&firing, "cellChanged", json!({ "cell": "Enter" }));
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    let started = Instant::now();
    assert!(!wait::for_cell_change(&bot, "nowhere").unwrap());
    let took = started.elapsed();
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    noise.join().unwrap();
    assert!(took >= Duration::from_millis(2400), "{took:?}");
}

#[test]
fn the_perf_log_says_how_waits_went() {
    let (bot, game) = bot();
    game.set("world.strFrame", "\"r2\"");
    assert!(wait::for_cell_change(&bot, "r2").unwrap());
    assert!(!wait::for_item_buy(&bot, 1).unwrap());
    let snapshot = bot.flash.perf().snapshot();
    let row = |name: &str| {
        snapshot
            .waits
            .iter()
            .find(|w| w.wait == name)
            .unwrap_or_else(|| panic!("no {name}: {:?}", snapshot.waits))
            .clone()
    };
    assert_eq!(
        (row("forCellChange").done, row("forCellChange").timed_out),
        (1, 0)
    );
    assert_eq!(
        (row("forItemBuy").done, row("forItemBuy").timed_out),
        (0, 1)
    );
    let report = snapshot.report("tab", 3);
    assert!(report.contains("waits: forCellChange 1x"), "{report}");
    assert!(
        report.contains("forItemBuy 1x") && report.contains("(1 timed out)"),
        "{report}"
    );
}
