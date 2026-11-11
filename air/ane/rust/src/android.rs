//! Kotlin's `Bridge` natives, and the engine's start.

use crate::host::{AIR_THREAD, AirFlashHost, BRIDGE};
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
            start_engine(root);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeFlashCall(name, args)`: `skua.swf`'s `ExternalInterface.call(name, ...)`, its
/// arguments as a JSON array. On AIR's thread: hands the call to the bot, which never waits here.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeFlashCall<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
    args: JString<'local>,
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
            turu::bot().on_flash_call(&name, args);
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
/// have its AIR mode; 1: skua.swf; 2: any other SWF the game or skua loads (a background, an
/// injected script), patched as the client's but required to have nothing.
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
            let strict = target != 2;
            let target = match target {
                1 => turu_air_patch::Target::Skua,
                _ => turu_air_patch::Target::Game,
            };
            let started = std::time::Instant::now();
            match turu_air_patch::patch_swf(&input, target) {
                Ok((output, report)) => {
                    tracing::info!(
                        "Patched {target:?} for AIR in {:?}: {} swapped, {} dropped",
                        started.elapsed(),
                        report.swapped.len(),
                        report.dropped.len()
                    );
                    if let Some(problem) = report.problem().filter(|_| strict) {
                        tracing::error!("{target:?} can't run under AIR: {problem}");
                        return Ok(JByteArray::default());
                    }
                    env.byte_array_from_slice(&output)
                }
                Err(e) => {
                    tracing::error!("Couldn't patch {target:?} for AIR: {e:#}");
                    Ok(JByteArray::default())
                }
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Starts the engine thread once per process, with `root` as the data folder.
fn start_engine(root: PathBuf) {
    static ENGINE: Once = Once::new();
    ENGINE.call_once(|| {
        turu_bot::paths::set_default_root(root.clone());
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
    turu::take_startup_args(&mut args);
    if !args.is_empty() {
        tracing::warn!(
            "{ARGS_FILE}: {} arguments aren't Turu's flags; ignored",
            args.len()
        );
    }
    // Before skua.swf's first call (`requestLoadGame`), which asks the bot to load the client.
    turu::bot().set_host(Some(Arc::new(AirFlashHost)));
    turu::configure_startup();
    turu::start_api();
    turu::start_monitor();
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
