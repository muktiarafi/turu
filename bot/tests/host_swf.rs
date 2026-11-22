//! Turu's host SWF (docs/HOST_SWF_PROPOSAL.md, R1): `hostInfo` asked when the client loads, names
//! it doesn't list refused, and its errors logged once a minute.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::{Bot, BotError, FlashArg, FlashHost, HostError};

/// A host SWF that records the calls it gets; `info` is what its `hostInfo` answers ("null":
/// none, as the tests' fake games). `hostInfo` waits while a test holds `gate`.
struct FakeSwf {
    info: String,
    calls: Mutex<Vec<(String, Vec<FlashArg>)>>,
    gate: Mutex<()>,
}

impl FakeSwf {
    fn new(info: Value) -> Arc<Self> {
        Arc::new(Self {
            info: info.to_string(),
            calls: Mutex::new(Vec::new()),
            gate: Mutex::new(()),
        })
    }

    fn calls(&self) -> Vec<(String, Vec<FlashArg>)> {
        self.calls.lock().clone()
    }

    fn count(&self, name: &str) -> usize {
        self.calls().iter().filter(|(n, _)| n == name).count()
    }
}

impl FlashHost for FakeSwf {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        self.calls.lock().push((name.to_owned(), args));
        if name == "hostInfo" {
            drop(self.gate.lock());
        }
        Ok(match name {
            "hostInfo" => self.info.clone(),
            _ => "null".into(),
        })
    }
}

fn turu_info() -> Value {
    json!({
        "name": "turu",
        "version": 1,
        "features": ["errors", "loadClientFrameRate"],
        "callbacks": ["loadClient", "hostInfo", "setBackgroundValues"]
    })
}

fn bot_with(swf: &Arc<FakeSwf>) -> Arc<Bot> {
    crate::common::data_dir();
    let bot = Bot::new();
    bot.set_host(Some(swf.clone()));
    bot
}

/// Waits until the fake got a call named `name`.
fn wait_for_call(swf: &FakeSwf, name: &str) -> Vec<FlashArg> {
    let start = Instant::now();
    loop {
        if let Some((_, args)) = swf.calls().into_iter().find(|(n, _)| n == name) {
            return args;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "no {name} call");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn turus_swf_gets_the_frame_rate_with_load_client_and_refuses_unlisted_names() {
    let swf = FakeSwf::new(turu_info());
    let bot = bot_with(&swf);
    bot.settings.write().animation_frame_rate = 60;
    bot.on_flash_call("requestLoadGame", vec![]);

    let args = wait_for_call(&swf, "loadClient");
    assert!(matches!(args.as_slice(), [FlashArg::Number(fps)] if *fps == 60.0));
    let info = bot.flash.host_info().expect("asked");
    assert_eq!(info.name, "turu");
    assert!(
        !swf.calls().iter().any(|(n, _)| n == "setGameObject"),
        "the frame rate went with loadClient"
    );

    let before = swf.calls().len();
    match bot
        .flash
        .call("getGameObject", vec!["world.strMapName".into()])
    {
        Err(BotError::NotInHostSwf(name)) => assert_eq!(name, "getGameObject"),
        other => panic!("expected NotInHostSwf, got {other:?}"),
    }
    assert_eq!(swf.calls().len(), before, "never reached Flash");
    assert!(bot.flash.call("setBackgroundValues", vec![]).is_ok());
}

/// The default starting rate (24, which every older settings file holds) leaves turu.swf at its
/// own 30: `loadClient` gets no rate.
#[test]
fn the_default_starting_rate_leaves_turus_swf_at_its_own() {
    let swf = FakeSwf::new(turu_info());
    let bot = bot_with(&swf);
    bot.on_flash_call("requestLoadGame", vec![]);

    let args = wait_for_call(&swf, "loadClient");
    assert!(args.is_empty(), "{args:?}");
}

/// A host without `hostInfo` (the tests' fake games): the client still loads, and no call is
/// refused.
#[test]
fn a_host_without_host_info_lets_every_call_through() {
    let swf = FakeSwf::new(Value::Null);
    let bot = bot_with(&swf);
    bot.on_flash_call("requestLoadGame", vec![]);

    wait_for_call(&swf, "loadClient");
    assert!(bot.flash.host_info().is_none());
    assert!(bot.flash.call("getGameObject", vec!["x".into()]).is_ok());
}

#[test]
fn a_new_start_asks_the_host_swf_again() {
    let swf = FakeSwf::new(turu_info());
    let bot = bot_with(&swf);
    bot.on_flash_call("requestLoadGame", vec![]);
    wait_for_call(&swf, "loadClient");
    assert!(bot.flash.host_info().is_some());

    // The new start's `hostInfo` waits at the gate: the old answer must be gone by then (the
    // client loading asks again at once, which raced this check on CI).
    let gate = swf.gate.lock();
    bot.on_flash_call("requestLoadGame", vec![]);
    assert!(
        bot.flash.host_info().is_none(),
        "forgotten until asked again"
    );
    drop(gate);

    let start = Instant::now();
    while swf.count("hostInfo") < 2 || bot.flash.host_info().is_none() {
        assert!(start.elapsed() < Duration::from_secs(5), "not asked again");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_same_error_is_logged_once_with_its_stack() {
    let swf = FakeSwf::new(turu_info());
    let bot = bot_with(&swf);
    let error = vec![json!("TypeError #1009: null"), json!("at Game/frame()")];
    for _ in 0..3 {
        bot.on_flash_call("error", error.clone());
    }
    bot.on_flash_call("error", vec![json!("Error #2: other"), json!("")]);

    let log = bot.logs.debug.text();
    assert_eq!(log.matches("TypeError #1009: null").count(), 1, "{log}");
    assert!(log.contains("at Game/frame()"), "{log}");
    assert!(log.contains("Error #2: other"), "{log}");
}
