//! The Java side's foreground service (`TuruService.kt`): its native methods, and the status it
//! shows. The service starts the engine (also after Android killed the process, when there's no
//! activity), its notification shows the active tab's script with Pause and Stop while it runs,
//! Start while one is loaded and stopped (and a line per tab with several), and it holds a wake lock while any tab's script runs. A thread here sends
//! it the bots' state every 2 s, when it changed, and keeps `android-tabs.json` up to date.

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{Global, JClass, JObject, JString, JValue};
use jni::{Env, EnvUnowned, JavaVM, jni_sig, jni_str};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use turu_bot::Bot;
use turu_frontend::turu;

/// The JVM and `TuruService`'s class: a native thread can't look the app's classes up itself.
static SERVICE: OnceLock<(JavaVM, Global<JClass<'static>>)> = OnceLock::new();

/// `TuruService.nativeStart(context, filesDir)`: starts the engine, if the activity hasn't.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruService_nativeStart<'local>(
    mut unowned_env: EnvUnowned<'local>,
    class: JClass<'local>,
    context: JObject<'local>,
    files_dir: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            crate::init_logging();
            let context = env.new_local_ref(&context)?;
            if let Err(e) = rustls_platform_verifier::android::init_with_env(env, context) {
                log::error!("Couldn't set up certificate checks; HTTPS will fail: {e}");
            }
            let root = PathBuf::from(files_dir.try_to_string(env)?);
            crate::start_engine(root);
            // After the engine set the data folder: the first status read makes the bot.
            if SERVICE.get().is_none() {
                let _ = SERVICE.set((env.get_java_vm()?, env.new_global_ref(&class)?));
                start_updates();
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `TuruService.nativeAction(action)`: the notification's buttons.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruService_nativeAction<'local>(
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
struct Status {
    title: String,
    text: String,
    /// The active tab's script runs (Pause and Stop act on it).
    running: bool,
    paused: bool,
    /// The active tab's script is loaded and can start (Start).
    startable: bool,
    /// Some tab's script runs: the wake lock is held.
    awake: bool,
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

fn status() -> Status {
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
    Status {
        title,
        text,
        running,
        paused,
        startable: can_start(&bot),
        awake: tabs.iter().any(|(_, tab)| tab.scripts.is_running()),
    }
}

/// Sends the bot's state to the service every 2 s, when it changed.
fn start_updates() {
    let spawned = std::thread::Builder::new()
        .name("Turu Notify".into())
        .spawn(|| {
            let mut shown: Option<Status> = None;
            loop {
                turu_frontend::saved_tabs::save();
                let now = status();
                if shown.as_ref() != Some(&now) {
                    match send(&now) {
                        Ok(()) => shown = Some(now),
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

/// Runs `work` with `TuruService`'s class on this thread (attached to the JVM); `None` before the
/// service started the engine or when the call failed.
pub(crate) fn with_service<T>(
    work: impl FnOnce(&mut Env, &JClass) -> jni::errors::Result<T>,
) -> Option<T> {
    let (vm, class) = SERVICE.get()?;
    crate::attach(vm, |env: &mut Env| work(env, class))
        .inspect_err(|e| tracing::warn!("A call to TuruService failed: {e}"))
        .ok()
}

/// `TuruService.panicked(message)`: the panic's message goes with the crash report Sentry sends.
pub(crate) fn panicked(message: &str) {
    with_service(|env, class| {
        let message = env.new_string(message)?;
        env.call_static_method(
            class,
            jni_str!("panicked"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&message)],
        )?;
        Ok(())
    });
}

/// `TuruService.breadcrumb(category, message)`: a warning that goes with the next crash report.
pub(crate) fn breadcrumb(category: &str, message: &str) {
    with_service(|env, class| {
        let category = env.new_string(category)?;
        let message = env.new_string(message)?;
        env.call_static_method(
            class,
            jni_str!("breadcrumb"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&category), JValue::Object(&message)],
        )?;
        Ok(())
    });
}

/// `TuruService.gpuLost(why)`: the GPU dropped the device ([`crate::memory`]), sent to Sentry as a
/// warning of its own.
pub(crate) fn gpu_lost(why: &str) {
    with_service(|env, class| {
        let why = env.new_string(why)?;
        env.call_static_method(
            class,
            jni_str!("gpuLost"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&why)],
        )?;
        Ok(())
    });
}

/// `TuruService.status(title, text, running, paused, startable, awake)`.
fn send(status: &Status) -> jni::errors::Result<()> {
    let Some((vm, class)) = SERVICE.get() else {
        return Ok(());
    };
    crate::attach(vm, |env: &mut Env| {
        let title = env.new_string(&status.title)?;
        let text = env.new_string(&status.text)?;
        env.call_static_method(
            class,
            jni_str!("status"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;ZZZZ)V"),
            &[
                JValue::Object(&title),
                JValue::Object(&text),
                JValue::Bool(status.running),
                JValue::Bool(status.paused),
                JValue::Bool(status.startable),
                JValue::Bool(status.awake),
            ],
        )?;
        Ok(())
    })
}
