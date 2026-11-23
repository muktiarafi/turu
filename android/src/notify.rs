//! A slot's notification (`SlotService.kt`): what it shows of the slot's tab (its script, with
//! Pause and Stop while it runs, Start while one is loaded and stopped; `status`) and its buttons
//! (`nativeAction`). And the calls into `TuruService`'s class, which holds the statics every
//! process uses (crash reports, alerts, the GPU's warnings).

use crate::java::TuruService;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned, JavaVM};
use std::sync::{Arc, OnceLock};
use turu_bot::Bot;
use turu_frontend::turu;

/// The slot process's JVM (`TuruService`'s methods are bound in `java.rs`).
static SERVICE: OnceLock<JavaVM> = OnceLock::new();

/// `SlotService.nativeAction(action)`: the slot's notification's buttons, for its tab.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeAction<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    action: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let action = action.try_to_string(env)?;
            if action == "start_again" {
                crate::memory_kills::start_again();
                return Ok(());
            }
            // Quit Turu: on the calling thread, done before the slots are stopped.
            if action == "quit" {
                crate::restart::quit();
                return Ok(());
            }
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

/// The notification's Start: the loaded script, under Script Loader's rules (not while it runs,
/// nor while Auto or a Console snippet does). Silent (VibeSkua `SilentConfig`): nobody may be
/// looking at the app to answer the Script Options window, so the saved options are used.
fn start(bot: &Arc<Bot>) {
    if !can_start(bot) {
        return;
    }
    if let Err(e) = turu_bot::script::start_silent(bot) {
        bot.log(format!("Couldn't start the script: {e}"));
    }
}

/// Whether Start can start the tab's script: one is loaded, it doesn't run, and neither Auto nor a
/// Console snippet does.
fn can_start(bot: &Bot) -> bool {
    bot.scripts.loaded().is_some()
        && !bot.scripts.status().is_active()
        && !bot.auto.is_running()
        && !bot.scripts.is_snippet_running()
}

/// What the notification shows.
#[derive(Clone, PartialEq)]
pub(crate) struct Status {
    pub(crate) title: String,
    pub(crate) text: String,
    /// The active tab's script runs (Pause and Stop act on it).
    pub(crate) running: bool,
    pub(crate) paused: bool,
    /// The active tab's script is loaded and can start (Start).
    pub(crate) startable: bool,
    /// Some tab's script runs: the wake lock is held.
    pub(crate) awake: bool,
    /// Held after kills for memory (`memory_kills`): Start again, not Start.
    pub(crate) held: bool,
}

/// The tab's script's name, while one is loaded.
fn script_name(bot: &Bot) -> Option<String> {
    bot.scripts
        .loaded()
        .and_then(|path| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
}

/// One tab's line: "Name is farming", "Name is in game, no script running", ... With one tab
/// the name is left out of the logged-out states ("Not logged in").
fn tab_line(bot: &Bot, named: bool) -> String {
    let who = turu::tab_label(bot);
    let running = bot.scripts.is_running();
    let logged_in = bot.state.lock().in_game;
    let (state, alone) = match (logged_in, running) {
        (true, true) => return format!("{who} is farming"),
        (true, false) => return format!("{who} is in game, no script running"),
        (false, _) if bot.servers.is_relogging() => ("logging back in", "Logging back in"),
        (false, _) => ("not logged in", "Not logged in"),
    };
    if named {
        format!("{who}: {state}")
    } else {
        alone.to_owned()
    }
}

/// The notification's title and text, and whether the CPU stays awake (a slot's too).
pub(crate) fn status() -> Status {
    let bot = turu::bot();
    let tabs = turu::tabs();
    let running = bot.scripts.is_running();
    let paused = running && bot.control.is_paused();
    let title = match (script_name(&bot), running) {
        (Some(name), true) => format!("{name} {}", if paused { "paused" } else { "running" }),
        // Loaded and stopped: what Start runs.
        (Some(name), false) => format!("{name} loaded"),
        _ => "Turu".to_owned(),
    };
    let mut text = if tabs.len() > 1 {
        // A line per tab, the active one marked: "▶ Name is farming (GoldFarm)".
        tabs.iter()
            .map(|(_, tab)| {
                let mark = if Arc::ptr_eq(tab, &bot) { "▶ " } else { "" };
                let script = script_name(tab)
                    .filter(|_| tab.scripts.is_running())
                    .map_or_else(String::new, |name| format!(" ({name})"));
                format!("{mark}{}{script}", tab_line(tab, true))
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        tab_line(&bot, false)
    };
    // The screen can't draw any more (`memory::gpu_lost`): the notification is what's left.
    if ruffle_render_wgpu::device_lost().is_some() {
        text.push_str(". Graphics stopped: restart Turu to see the game");
    }
    // Held after kills for memory: why comes first (the group's summary shows the first line).
    let held = crate::memory_kills::held_text();
    if let Some(why) = &held {
        text = format!("{why}\n{text}");
    }
    Status {
        title,
        text,
        running,
        paused,
        // Its Start would run the script at the login screen: Start again instead.
        startable: held.is_none() && can_start(&bot),
        awake: tabs.iter().any(|(_, tab)| tab.scripts.is_running()),
        held: held.is_some(),
    }
}

/// Runs `work` with `TuruService`'s class on this thread (attached to the JVM); `None` before the
/// service started the engine or when the call failed.
/// A process tab's slot (`slots.rs`): its calls into `TuruService`'s class (crash reports, the
/// panic's message, the GPU's warnings) reach this process's `CrashUploads`, and its alerts this
/// process's `Alerts` (`SlotService` sets it up). Not the app's notification: the slot's own is
/// `SlotService`'s.
pub(crate) fn set_slot_vm(vm: JavaVM) {
    let _ = SERVICE.set(vm);
}

pub(crate) fn with_service<T>(work: impl FnOnce(&mut Env) -> jni::errors::Result<T>) -> Option<T> {
    let vm = SERVICE.get()?;
    crate::attach(vm, work)
        .inspect_err(|e| tracing::warn!("A call to TuruService failed: {e}"))
        .ok()
}

/// `TuruService.panicked(message)`: the panic's message goes with the crash report Sentry sends.
pub(crate) fn panicked(message: &str) {
    with_service(|env| {
        let message = env.new_string(message)?;
        TuruService::panicked(env, &message)?;
        Ok(())
    });
}

/// `TuruService.breadcrumb(category, message)`: a warning that goes with the next crash report.
pub(crate) fn breadcrumb(category: &str, message: &str) {
    with_service(|env| {
        let category = env.new_string(category)?;
        let message = env.new_string(message)?;
        TuruService::breadcrumb(env, &category, &message)?;
        Ok(())
    });
}

/// `TuruService.gpuLost(why)`: the GPU dropped the device ([`crate::memory`]), sent to Sentry as a
/// warning of its own.
pub(crate) fn gpu_lost(why: &str) {
    with_service(|env| {
        let why = env.new_string(why)?;
        TuruService::gpu_lost(env, &why)?;
        Ok(())
    });
}

/// `TuruService.alert(kind, title, text)`: a notification while the app isn't on screen
/// (`Host::alert`). Through the service's class, as the activity may be gone.
pub(crate) fn alert(kind: turu_frontend::host::Alert, title: &str, text: &str) {
    // In a process tab's slot too: its `SlotService` set up `Alerts` there, and the shell tells it
    // whether the app is on screen.
    let Some(vm) = SERVICE.get() else {
        return;
    };
    let posted = crate::attach(vm, |env: &mut Env| -> jni::errors::Result<()> {
        let title = env.new_string(title)?;
        let text = env.new_string(text)?;
        TuruService::alert(env, kind as i32, &title, &text)?;
        Ok(())
    });
    if let Err(e) = posted {
        tracing::warn!("Couldn't post the alert: {e}");
    }
}
