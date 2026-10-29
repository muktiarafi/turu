//! The "Auto" popup's Auto Attack: a stoppable loop on a bot thread (VibeSkua's
//! `AutoViewModel`): start the class's skill rotation, then kill any monster, over and over.

use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use turu_bot::api::dispatch;
use turu_bot::control::mark_script_thread;
use turu_bot::{Bot, BotError, BotResult};

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn running() -> bool {
    RUNNING.load(Ordering::Acquire)
}

/// Starts Auto Attack. `class` is the saved skill set to use (`None`: the equipped class) in
/// `mode` ("Base", "Farm", ...).
pub fn start(class: Option<String>, mode: String) {
    if RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::turu::spawn_action("Auto Attack", move |bot| {
        // Like the script thread: Stop (`request_stop`) makes the next bot call fail.
        mark_script_thread(true);
        bot.control.reset();
        let result = run(bot, class, &mode);
        mark_script_thread(false);
        if !bot.scripts.is_running() {
            bot.control.reset();
        }
        let _ = dispatch(bot, "skills.stop", &[]);
        bot.log("Auto Attack stopped");
        RUNNING.store(false, Ordering::Release);
        match result {
            Err(BotError::Stopped) => Ok(()),
            other => other,
        }
    });
}

/// Stops Auto Attack (no-op when it isn't running).
pub fn stop() {
    if running() {
        crate::turu::bot().control.request_stop();
    }
}

fn run(bot: &std::sync::Arc<Bot>, class: Option<String>, mode: &str) -> BotResult<()> {
    bot.log("Auto Attack started");
    dispatch(
        bot,
        "skills.startAdvanced",
        &[
            class.map_or(serde_json::Value::Null, Into::into),
            json!(true),
            json!(mode),
        ],
    )?;
    loop {
        if !bot.state.lock().in_game {
            bot.sleep(1000)?;
            continue;
        }
        if let Err(e) = dispatch(bot, "kill.monster", &[json!("*")]) {
            if matches!(e, BotError::Stopped) {
                return Err(e);
            }
            bot.logs.debug.push(format!("Auto Attack: {e}"));
            bot.sleep(1000)?;
        }
        bot.sleep(100)?;
    }
}
