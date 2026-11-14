//! Turu's host SWF (docs/HOST_SWF_PROPOSAL.md, R1): `hostInfo` asked when the client loads, names
//! it doesn't list refused, and its errors logged once a minute.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::{Bot, BotError, FlashArg, FlashHost, HostError};

/// A host SWF that records the calls it gets; `info` is what its `hostInfo` answers ("null":
/// none, as the tests' fake games).
struct FakeSwf {
    info: String,
    calls: Mutex<Vec<(String, Vec<FlashArg>)>>,
}

impl FakeSwf {
    fn new(info: Value) -> Arc<Self> {
        Arc::new(Self {
            info: info.to_string(),
            calls: Mutex::new(Vec::new()),
        })
    }

    fn calls(&self) -> Vec<(String, Vec<FlashArg>)> {
        self.calls.lock().clone()
    }
}

impl FlashHost for FakeSwf {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        self.calls.lock().push((name.to_owned(), args));
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
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
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
    bot.on_flash_call("requestLoadGame", vec![]);

    let args = wait_for_call(&swf, "loadClient");
    assert!(matches!(args.as_slice(), [FlashArg::Number(fps)] if *fps > 0.0));
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

/// A host without `hostInfo` (the tests' fake games): the client still loads at the frame rate,
/// and no call is refused.
#[test]
fn a_host_without_host_info_lets_every_call_through() {
    let swf = FakeSwf::new(Value::Null);
    let bot = bot_with(&swf);
    bot.on_flash_call("requestLoadGame", vec![]);

    let args = wait_for_call(&swf, "loadClient");
    assert!(matches!(args.as_slice(), [FlashArg::Number(fps)] if *fps > 0.0));
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

    bot.on_flash_call("requestLoadGame", vec![]);
    assert!(
        bot.flash.host_info().is_none(),
        "forgotten until asked again"
    );
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
