//! Turu Mas's notification (`TuruAirService.kt`, through Kotlin's `Bridge`), as Turu's Android
//! app has it (`android/src/notify.rs`): the loaded script's state, Pause and Stop while it runs,
//! Start while it's loaded and stopped, and the wake lock held only while it runs. A thread here
//! sends the state every 2 s, when it changed. Turu Mas has one client, so one line.

use crate::host::BRIDGE;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JString, JValue};
use jni::{Env, EnvUnowned, jni_sig, jni_str};
use std::sync::Arc;
use std::time::Duration;
use turu_bot::Bot;
use turu_frontend::turu;

/// `Bridge.nativeAction(action)`: the notification's buttons ("start", "pause", "stop").
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeAction<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    action: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let action = action.try_to_string(env)?;
            // Off the calling thread: stopping waits for the script's stop handlers.
            turu::spawn_action("Notification", move |bot| {
                match action.as_str() {
                    "start" => start(bot),
                    "stop" => turu_bot::script::stop(bot),
                    "pause" => {
                        let paused = bot.control.is_paused();
                        turu_bot::script::set_paused(bot, !paused);
                    }
                    other => tracing::warn!("Unknown notification action {other}"),
                }
                Ok(())
            });
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The notification's Start: the loaded script, under Script Loader's rules, silently (nobody may
/// be looking at the app to answer the Script Options window: the saved options are used).
fn start(bot: &Arc<Bot>) {
    if !can_start(bot) {
        return;
    }
    if let Err(e) = turu_bot::script::start_silent(bot) {
        bot.log(format!("Couldn't start the script: {e}"));
    }
}

/// A script is loaded, it doesn't run, and neither Auto nor a Console snippet does.
fn can_start(bot: &Bot) -> bool {
    bot.scripts.loaded().is_some()
        && !bot.scripts.status().is_active()
        && !bot.auto.is_running()
        && !bot.scripts.is_snippet_running()
}

/// What the notification shows.
#[derive(Clone, PartialEq)]
struct Status {
    title: String,
    text: String,
    running: bool,
    paused: bool,
    startable: bool,
}

fn status() -> Status {
    let bot = turu::bot();
    let running = bot.scripts.is_running();
    let paused = running && bot.control.is_paused();
    let name = bot
        .scripts
        .loaded()
        .and_then(|path| path.file_stem().map(|s| s.to_string_lossy().into_owned()));
    let title = match (name, running) {
        (Some(name), true) => format!("{name} {}", if paused { "paused" } else { "running" }),
        (Some(name), false) => format!("{name} loaded"),
        _ => "Turu Mas".to_owned(),
    };
    let who = turu::tab_label(&bot);
    let text = match (bot.state.lock().in_game, running) {
        (true, true) => format!("{who} is farming"),
        (true, false) => format!("{who} is in game, no script running"),
        (false, _) if bot.servers.is_relogging() => "Logging back in".to_owned(),
        (false, _) => "Not logged in".to_owned(),
    };
    Status {
        title,
        text,
        running,
        paused,
        startable: can_start(&bot),
    }
}

/// Sends the state every 2 s, when it changed (from when the engine has started).
pub(crate) fn start_updates() {
    let spawned = std::thread::Builder::new()
        .name("Turu Notify".into())
        .spawn(|| {
            let mut shown: Option<Status> = None;
            loop {
                let now = status();
                if shown.as_ref() != Some(&now) {
                    match send(&now) {
                        Ok(true) => shown = Some(now),
                        Ok(false) => {}
                        Err(e) => tracing::warn!("Couldn't update the notification: {e}"),
                    }
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start the notification updates: {e}");
    }
}

/// `Bridge.status(title, text, running, paused, startable)`; false before Kotlin's `Bridge` is
/// known.
fn send(status: &Status) -> jni::errors::Result<bool> {
    let Some((vm, class)) = BRIDGE.get() else {
        return Ok(false);
    };
    crate::android::attach(vm, |env: &mut Env| {
        let title = env.new_string(&status.title)?;
        let text = env.new_string(&status.text)?;
        env.call_static_method(
            class,
            jni_str!("status"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;ZZZ)V"),
            &[
                JValue::Object(&title),
                JValue::Object(&text),
                JValue::Bool(status.running),
                JValue::Bool(status.paused),
                JValue::Bool(status.startable),
            ],
        )?;
        Ok(true)
    })
}
