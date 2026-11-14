//! Kotlin's `Bridge` natives, and the engine's start.

use crate::host::{AIR_THREAD, BRIDGE};
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::jint;
use jni::{EnvUnowned, JavaVM};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use std::time::Duration;
use turu_frontend::turu;

/// Setup lines (Turu's flags, one per line), as the Ruffle build's (docs/ANDROID.md).
const ARGS_FILE: &str = "turu-args.txt";

/// Where a panic's report goes (the data folder's `crash/`), once the engine has its folder.
static CRASH_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// `Bridge.nativeStart(context, filesDir)`: starts the engine, once per process. Called on AIR's
/// thread, which this records (a Flash call from it would wait for itself).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeStart<'local>(
    mut unowned_env: EnvUnowned<'local>,
    class: JClass<'local>,
    context: JObject<'local>,
    files_dir: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            init_logging();
            let _ = AIR_THREAD.set(std::thread::current().id());
            if BRIDGE.get().is_none() {
                let _ = BRIDGE.set((env.get_java_vm()?, env.new_global_ref(&class)?));
            }
            // Before any reqwest client is built.
            let _ = rustls::crypto::ring::default_provider().install_default();
            let context = env.new_local_ref(&context)?;
            if let Err(e) = rustls_platform_verifier::android::init_with_env(env, context) {
                log::error!("Couldn't set up certificate checks; HTTPS will fail: {e}");
            }
            let root = PathBuf::from(files_dir.try_to_string(env)?);
            // Here, on a Java thread: the engine's threads can't find the extension's classes.
            if let Err(e) = crate::passwords::install(env) {
                // A missing class leaves its exception pending: not ours to throw to AIR.
                env.exception_clear();
                log::error!(
                    "Couldn't set up the password store; Accounts won't keep passwords: {e}"
                );
            }
            turu_frontend::platform::set(Arc::new(crate::platform::AirPlatform));
            start_engine(root);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeFlashCall(name, args, client)`: client `client`'s `turu.swf` called
/// `ExternalInterface.call(name, ...)`, its arguments as a JSON array (-1: the host SWF itself).
/// On AIR's thread: hands the call to that tab's bot, which never waits here.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeFlashCall<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
    args: JString<'local>,
    client: jint,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let name = name.try_to_string(env)?;
            let args = args.try_to_string(env)?;
            let args = match serde_json::from_str(&args) {
                Ok(serde_json::Value::Array(args)) => args,
                Ok(other) => vec![other],
                Err(e) => {
                    tracing::warn!("Flash call {name}: arguments aren't JSON ({e})");
                    Vec::new()
                }
            };
            let bot = u8::try_from(client)
                .ok()
                .and_then(crate::clients::bot)
                .unwrap_or_else(turu::bot);
            bot.on_flash_call(&name, args);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeReply(id, value, runMicros)`: AS3's answer to a call ([`crate::host`]).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeReply<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jni::sys::jlong,
    value: JString<'local>,
    run_micros: jint,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let value = value.try_to_string(env)?;
            let run = Duration::from_micros(run_micros.max(0) as u64);
            crate::host::answer(id as u64, value, run);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativePatch(swf, target)`: the SWF rewritten for AIR (`turu_air_patch`), or null when
/// it can't run there (then the host refuses to load it). Target 0: the game's client, which must
/// have its AIR mode; 2: any other SWF the game loads (a map, an asset, the game menu), patched
/// as the client's but required to have nothing. 1 is unused (it was VibeSkua's SWF until R6b).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativePatch<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    swf: JByteArray<'local>,
    target: jint,
) -> JByteArray<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JByteArray<'local>> {
            let input = env.convert_byte_array(&swf)?;
            match patched(&input, target) {
                Some(output) => env.byte_array_from_slice(&output),
                None => Ok(JByteArray::default()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// What [`Java_com_muktiarafi_turu_air_Bridge_nativePatch`] answers: the patched SWF, from the
/// data folder's `cache/airpatched` when this input was patched before, or `None` when it
/// can't run under AIR. Only SWFs that can are kept.
fn patched(input: &[u8], target: jint) -> Option<Vec<u8>> {
    static CACHE: std::sync::OnceLock<turu_air_patch::cache::PatchCache> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        turu_air_patch::cache::PatchCache::new(turu_bot::paths::Paths::new().air_patched_dir())
    });
    // Bit 0x10: the clients (tabs) share the stage.
    let shared_stage = target & TARGET_SHARED_STAGE != 0;
    if shared_stage {
        SHARED_STAGE.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let options = turu_air_patch::Options { shared_stage };
    let kind = match (target & 0xf, shared_stage) {
        (0, false) => "game",
        (0, true) => "gameshared",
        (_, false) => "asset",
        (_, true) => "assetshared",
    };
    let strict = target & 0xf == 0;
    let started = std::time::Instant::now();
    if let Some(output) = cache.get(input, kind) {
        tracing::info!(
            "Patched {kind} for AIR: kept from before, read in {:?}",
            started.elapsed()
        );
        return Some(output);
    }
    match turu_air_patch::patch_swf_with(input, options) {
        Ok((output, report)) => {
            tracing::info!(
                "Patched {kind} for AIR in {:?}: {} swapped, {} dropped, {} stage roots, {} hooks, {} missed",
                started.elapsed(),
                report.swapped.len(),
                report.dropped.len(),
                report.stage_roots.len(),
                report.stage_hooks.len(),
                report.stage_missed.len()
            );
            if let Some(problem) = report.problem().filter(|_| strict) {
                tracing::error!("{kind} can't run under AIR: {problem}");
                return None;
            }
            if let Err(e) = cache.put(input, kind, &output) {
                tracing::warn!(
                    "Couldn't keep the patched {kind} in {}: {e}",
                    cache.dir().display()
                );
            }
            Some(output)
        }
        Err(e) => {
            tracing::error!("Couldn't patch {kind} for AIR: {e:#}");
            None
        }
    }
}

/// [`patched`]'s target bit: the tabs' clients share AIR's one stage.
const TARGET_SHARED_STAGE: jint = 0x10;
/// Whether the host shares the stage among clients (it asked for a shared-stage patch): then
/// every SWF downloaded for the game is rewritten too, on the download's thread.
static SHARED_STAGE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A downloaded SWF with its `stage.getChildAt(0)`s rewritten for a shared stage.
fn shared_stage_swf(url: &str, body: Vec<u8>) -> Vec<u8> {
    if !SHARED_STAGE.load(std::sync::atomic::Ordering::Relaxed)
        || !matches!(body.get(..3), Some(b"FWS" | b"CWS" | b"ZWS"))
    {
        return body;
    }
    let started = std::time::Instant::now();
    let options = turu_air_patch::Options { shared_stage: true };
    match turu_air_patch::patch_swf_with(&body, options) {
        Ok((output, report)) => {
            if report.changed() || !report.stage_missed.is_empty() {
                tracing::info!(
                    "Shared stage: {url}: {} roots, {} hooks, {} missed, in {:?}",
                    report.stage_roots.len(),
                    report.stage_hooks.len(),
                    report.stage_missed.len(),
                    started.elapsed()
                );
            }
            output
        }
        Err(e) => {
            tracing::warn!("Shared stage: couldn't patch {url}: {e:#}");
            body
        }
    }
}

/// Downloads [`Java_com_muktiarafi_turu_air_Bridge_nativeFetch`] finished, by id, until AS3
/// takes them ([`Java_com_muktiarafi_turu_air_Bridge_nativeFetchResult`]).
static FETCHED: std::sync::Mutex<Option<std::collections::HashMap<i64, Vec<u8>>>> =
    std::sync::Mutex::new(None);

fn fetched() -> std::sync::MutexGuard<'static, Option<std::collections::HashMap<i64, Vec<u8>>>> {
    FETCHED.lock().unwrap_or_else(|e| e.into_inner())
}

/// `Bridge.nativeFetch(id, url)`: downloads a game file through Turu's disk cache
/// (`turu_frontend::game_files::fetch`; AIR's own downloads keep nothing), for the host's
/// `TuruURLLoader`. Returns at once; when done, a status event "fetched" with level `"<id>"`, or
/// `"<id> <why>"` when it failed, and the bytes wait for `nativeFetchResult(id)`.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeFetch<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jni::sys::jlong,
    url: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let url = url.try_to_string(env)?;
            // Downloads under way: when the last of a batch (a map's files) ends, the log says
            // what the cache has done so far.
            static IN_FLIGHT: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            IN_FLIGHT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let spawned = std::thread::Builder::new()
                .name("Turu Fetch".into())
                .spawn(move || {
                    let result = turu_frontend::game_files::fetch(&url);
                    if IN_FLIGHT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) == 1
                        && let Some(summary) = turu_frontend::game_files::summary()
                    {
                        tracing::info!("Game files: {summary}");
                    }
                    let level = match result {
                        Ok(body) => {
                            let body = shared_stage_swf(&url, body);
                            fetched().get_or_insert_default().insert(id, body);
                            id.to_string()
                        }
                        Err(why) => format!("{id} {why}"),
                    };
                    let Some((vm, class)) = BRIDGE.get() else {
                        return;
                    };
                    let sent = attach(vm, |env: &mut jni::Env| {
                        crate::host::dispatch(env, class, "fetched", &level)
                    });
                    if let Err(e) = sent {
                        fetched().get_or_insert_default().remove(&id);
                        tracing::warn!("Couldn't tell AIR {url} was fetched: {e}");
                    }
                });
            if let Err(e) = spawned {
                IN_FLIGHT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                tracing::warn!("Couldn't start a download: {e}");
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeFetchResult(id)`: the bytes of download `id`, once (null if there are none).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeFetchResult<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jni::sys::jlong,
) -> JByteArray<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JByteArray<'local>> {
            match fetched().get_or_insert_default().remove(&id) {
                Some(body) => env.byte_array_from_slice(&body),
                None => Ok(JByteArray::default()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeSurface(surface, density)`: the overlay's surface came or changed (null: it
/// went), at `density` pixels per point.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeSurface<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    surface: JObject<'local>,
    density: jni::sys::jfloat,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let window = if surface.is_null() {
                None
            } else {
                // SAFETY: `env` and `surface` are this call's, an android.view.Surface; the
                // native window holds its own reference.
                unsafe {
                    ndk::native_window::NativeWindow::from_surface(
                        env.get_raw().cast(),
                        surface.as_raw().cast(),
                    )
                }
            };
            crate::overlay::set_window(window, density);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeTouch(phase, x, y)`: a finger on the overlay's UI (0 down, 1 move, 2 up, 3
/// cancelled), in pixels.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeTouch<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    phase: jint,
    x: jni::sys::jfloat,
    y: jni::sys::jfloat,
) {
    let phase = match phase {
        0 => egui::TouchPhase::Start,
        1 => egui::TouchPhase::Move,
        2 => egui::TouchPhase::End,
        _ => egui::TouchPhase::Cancel,
    };
    crate::overlay::touch(phase, x, y);
}

/// `Bridge.nativeOnUi(x, y)`: whether a finger coming down there is Turu's UI's (else AIR's).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeOnUi<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    x: jni::sys::jfloat,
    y: jni::sys::jfloat,
) -> jni::sys::jboolean {
    crate::overlay::on_ui(x, y)
}

/// `Bridge.nativeText(text)`: the soft keyboard typed `text` into the UI's text box.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeText<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    text: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let text = text.try_to_string(env)?;
            crate::overlay::input(vec![egui::Event::Text(text)]);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeKey(key)`: a key the UI's text box handles (0 Backspace, 1 Enter, 2 Delete,
/// 3 left, 4 right).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeKey<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    key: jint,
) {
    let key = match key {
        0 => egui::Key::Backspace,
        1 => egui::Key::Enter,
        2 => egui::Key::Delete,
        3 => egui::Key::ArrowLeft,
        4 => egui::Key::ArrowRight,
        _ => return,
    };
    let press = |pressed| egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    crate::overlay::input(vec![press(true), press(false)]);
}

/// `Bridge.nativeInsets(keyboard, cutoutLeft, cutoutRight)`: the window's insets changed: the soft
/// keyboard's height and the display cutout's sides, in pixels.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeInsets<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    keyboard: jint,
    cutout_left: jint,
    cutout_right: jint,
) {
    crate::overlay::set_insets(keyboard as f32, (cutout_left as f32, cutout_right as f32));
}

/// Starts the engine thread once per process, with `root` as the data folder.
fn start_engine(root: PathBuf) {
    static ENGINE: Once = Once::new();
    ENGINE.call_once(|| {
        turu_bot::paths::set_default_root(root.clone());
        // Client updates look for Turu Mas's APK, not Turu's, and name it.
        turu_bot::updates::set_turu_mas();
        let _ = CRASH_DIR.set(turu_bot::paths::Paths::new().crash_dir());
        let spawned = std::thread::Builder::new()
            .name("Turu Engine".into())
            .spawn(move || engine(&root));
        if let Err(e) = spawned {
            log::error!("Couldn't start the engine thread: {e}");
        }
    });
}

/// Sets the bot up from `turu-args.txt` and connects it to the game in AIR. Calls into the game
/// wait for AIR's thread, so this runs on a thread of its own.
fn engine(root: &Path) {
    tracing::info!(
        "Turu {} in Adobe AIR; data folder {}",
        turu::VERSION,
        root.display()
    );
    let mut args = read_args(&root.join(ARGS_FILE));
    let perf_log = take_value(&mut args, "--perf-log");
    turu::take_startup_args(&mut args);
    if !args.is_empty() {
        tracing::warn!(
            "{ARGS_FILE}: {} arguments aren't Turu's flags; ignored",
            args.len()
        );
    }
    // Before turu.swf's first call (`requestLoadGame`), which asks the bot to load the client:
    // the first tab is client 0, which the host SWF loads at start. The others open and close
    // through tab requests (`clients`).
    crate::clients::start();
    turu::configure_startup();
    turu::start_api();
    turu::start_monitor();
    // `--perf-log <seconds>`: `TURU_PERF_LOG`, which an app can't be started with: each tab's
    // calls to turu.swf (each a round trip through the extension) and the busiest of them.
    if let Some(seconds) = perf_log {
        // SAFETY: before the perf log's thread exists (started just below).
        unsafe { std::env::set_var("TURU_PERF_LOG", seconds) };
    }
    turu::start_perf_log(None);
    // At start and every 6 hours (Options > Application): a newer Turu Mas shows Update in the
    // bar, whose button downloads its APK for Android to install.
    turu::start_client_updates();
    // The notification's state and buttons (`TuruAirService`).
    crate::notify::start_updates();
}

/// Takes `flag <value>` or `flag=<value>` out of `args`.
fn take_value(args: &mut Vec<OsString>, flag: &str) -> Option<String> {
    let mut value = None;
    let mut rest = Vec::with_capacity(args.len());
    let mut iter = std::mem::take(args).into_iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some(text) if text == flag => value = iter.next().and_then(|v| v.into_string().ok()),
            Some(text) if text.strip_prefix(flag).is_some_and(|v| v.starts_with('=')) => {
                value = Some(text[flag.len() + 1..].to_owned());
            }
            _ => rest.push(arg),
        }
    }
    *args = rest;
    value
}

fn read_args(file: &Path) -> Vec<OsString> {
    match std::fs::read_to_string(file) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(OsString::from)
            .collect(),
        Err(e) => {
            tracing::info!("No {} ({e}): log in on the game's screen", file.display());
            Vec::new()
        }
    }
}

fn init_logging() {
    static LOGGING: Once = Once::new();
    LOGGING.call_once(|| {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("turu")
                .with_filter(
                    android_logger::FilterBuilder::new()
                        .parse("warn,turu=info,turu_bot=info,turu_frontend=info,turu_air=info")
                        .build(),
                ),
        );
        log::info!("Turu {} starting (AIR)", turu::VERSION);
        std::panic::set_hook(Box::new(|info| {
            let thread = std::thread::current();
            let thread = thread.name().unwrap_or("<unnamed>");
            let backtrace = std::backtrace::Backtrace::force_capture();
            log::error!("Panic on {thread}: {info}\n{backtrace}");
            // As the Ruffle build's `panic-<time>.txt`: logcat's buffer is gone by the time
            // someone says "it crashed".
            if let Some(dir) = CRASH_DIR.get() {
                let seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let _ = std::fs::create_dir_all(dir);
                let _ = std::fs::write(
                    dir.join(format!("panic-{seconds}.txt")),
                    format!(
                        "Turu {} (AIR)\nPanic on {thread}: {info}\n{backtrace}\n",
                        turu::VERSION
                    ),
                );
            }
            // A release build aborts after this; Sentry's native handler (the extension's) reports
            // the abort, with this message.
            crate::crash_uploads::panicked(&format!("Panic on {thread}: {info}"));
        }));
    });
}

/// Runs `work` on this thread attached to the JavaVM, under the thread's own name (Android
/// otherwise calls it "Thread-N"), as the Ruffle build's `attach`.
pub(crate) fn attach<T, E>(
    vm: &JavaVM,
    work: impl FnOnce(&mut jni::Env) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<jni::errors::Error>,
{
    let name = std::thread::current()
        .name()
        .map(jni::strings::JNIString::from);
    vm.attach_current_thread_with_config(
        || match &name {
            Some(name) => jni::AttachConfig::new().thread_name(name),
            None => jni::AttachConfig::new(),
        },
        Some(jni::DEFAULT_LOCAL_FRAME_CAPACITY),
        work,
    )
}
