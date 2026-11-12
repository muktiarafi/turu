//! Parity mode (`TURU_HOST_PARITY=1`, docs/HOST_SWF_PROPOSAL.md R2) against a fake player with
//! both host SWFs: reads asked of both, VibeSkua's answer used, writes sent once, Turu's events
//! only compared.

use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// VibeSkua's SWF answers `world.strMapName` with `"battleon"`, Turu's (`turu.` names) with
/// `"yulgar"`; every other read is the same.
#[derive(Default)]
struct BothSwfs {
    calls: Mutex<Vec<String>>,
    pairs: Mutex<usize>,
}

impl BothSwfs {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().clone()
    }
}

impl FlashHost for BothSwfs {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        self.calls.lock().push(name.to_owned());
        let path = match args.first() {
            Some(FlashArg::String(s)) => s.as_str(),
            _ => "",
        };
        Ok(match (name, path) {
            ("hostInfo", _) => "null".into(),
            ("turu.hostInfo", _) => json!({
                "name": "turu",
                "version": 1,
                "features": ["loadClientFrameRate"],
                "callbacks": ["getGameObject", "setGameObject", "isNull"]
            })
            .to_string(),
            ("getGameObject", "world.strMapName") => "\"battleon\"".into(),
            ("turu.getGameObject", "world.strMapName") => "\"yulgar\"".into(),
            ("getGameObject" | "turu.getGameObject", _) => "1".into(),
            _ => "null".into(),
        })
    }

    fn call_pair(
        &self,
        first: (&str, Vec<FlashArg>),
        second: (&str, Vec<FlashArg>),
    ) -> Result<(String, String), HostError> {
        *self.pairs.lock() += 1;
        Ok((self.call(first.0, first.1)?, self.call(second.0, second.1)?))
    }
}

fn parity_bot() -> (Arc<Bot>, Arc<BothSwfs>) {
    // SAFETY: tests in this file set the same values.
    unsafe {
        std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data"));
        std::env::set_var("TURU_HOST_PARITY", "1");
    }
    let swfs = Arc::new(BothSwfs::default());
    let bot = Bot::new();
    bot.set_host(Some(swfs.clone()));
    bot.on_flash_call("requestLoadGame", vec![json!([])]);
    let start = Instant::now();
    while !swfs.calls().iter().any(|c| c == "loadClient") {
        assert!(start.elapsed() < Duration::from_secs(5), "no loadClient");
        std::thread::sleep(Duration::from_millis(10));
    }
    (bot, swfs)
}

#[test]
fn reads_go_to_both_and_vibeskuas_answer_is_used() {
    let (bot, swfs) = parity_bot();
    assert!(
        bot.flash.host_info().is_none(),
        "VibeSkua's is authoritative"
    );

    let map: String = bot.flash.get("world.strMapName").unwrap();
    assert_eq!(map, "battleon");
    let gold: i64 = bot.flash.get("world.myAvatar.objData.intGold").unwrap();
    assert_eq!(gold, 1);
    assert_eq!(*swfs.pairs.lock(), 2);

    let log = bot.logs.debug.text();
    assert!(
        log.contains(
            r#"parity: getGameObject world.strMapName: VibeSkua "battleon" / Turu "yulgar""#
        ),
        "{log}"
    );
    assert_eq!(log.matches("parity:").count(), 1, "{log}");
    let report = bot.flash.parity().unwrap().report().unwrap();
    assert!(report.contains("getGameObject 1/2 mismatched"), "{report}");
}

#[test]
fn writes_and_reads_turu_lacks_go_to_vibeskua_only() {
    let (bot, swfs) = parity_bot();
    bot.flash.set("world.myAvatar.x", 1).unwrap();
    bot.flash.get_static::<String>("serverIP").unwrap();
    let calls = swfs.calls();
    assert!(calls.contains(&"setGameObject".to_owned()));
    assert!(calls.contains(&"getGameObjectS".to_owned()));
    assert!(
        !calls
            .iter()
            .any(|c| c == "turu.setGameObject" || c == "turu.getGameObjectS"),
        "{calls:?}"
    );
}

#[test]
fn turus_events_are_compared_not_acted_on() {
    let (bot, _swfs) = parity_bot();
    bot.on_flash_call("packet", vec![json!(["%xt%zm%moveToCell%1%Enter%Spawn%"])]);
    bot.on_flash_call(
        "turu.packet",
        vec![json!(["%xt%zm%moveToCell%1%Enter%Spawn%"])],
    );
    bot.on_flash_call(
        "turu.packet",
        vec![json!(["%xt%zm%moveToCell%1%Wrong%Spawn%"])],
    );
    bot.on_flash_call("packet", vec![json!(["%xt%zm%moveToCell%1%Other%Spawn%"])]);

    let log = bot.logs.debug.text();
    assert_eq!(log.matches("parity: packet").count(), 1, "{log}");
    assert!(log.contains("Wrong"), "{log}");
    let summary = bot.flash.parity().unwrap().summary().unwrap();
    assert!(
        summary.contains("1 unlisted mismatches (packet 1)"),
        "{summary}"
    );
}
