//! Turu on Android, phase 1 of docs/ANDROID_PROPOSAL.md: the game and the bot without a UI. The
//! activity only starts the engine thread, which runs the player with the null renderer
//! (`turu_frontend::engine`, as `--headless` does) for as long as the process lives; the activity
//! coming and going doesn't stop it.
//!
//! Setup comes from `turu-args.txt` in the app's files folder (the data folder, reachable with
//! `adb`): the desktop's flags (`--user`, `--password`, `--server`, `--run-script`, `--api`, ...),
//! one argument per line, `#` for comments. The log goes to logcat (tag `turu`).

#![cfg(target_os = "android")]

use android_activity::{AndroidApp, MainEvent, PollEvent};
use jni::JavaVM;
use jni::objects::JObject;
use jni::sys::jobject;
use ruffle_core::backend::navigator::SocketMode;
use ruffle_core::backend::ui::NullUiBackend;
use ruffle_core::{LoadBehavior, Player, PlayerBuilder};
use ruffle_frontend_utils::backends::navigator::ExternalNavigatorBackend;
use ruffle_frontend_utils::backends::storage::DiskStorageBackend;
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_render::backend::ViewportDimensions;
use ruffle_render::backend::null::NullRenderer;
use ruffle_render::quality::StageQuality;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;
use turu_frontend::engine::{self, EngineEvent, Executor, NoDialogs, STAGE};
use turu_frontend::turu;
use url::Url;

/// The startup flags, in the data folder.
const ARGS_FILE: &str = "turu-args.txt";

static ENGINE: Once = Once::new();

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    ENGINE.call_once(|| {
        init_logging();
        // Before any reqwest client is built (Search Scripts, the navigator).
        let _ = rustls::crypto::ring::default_provider().install_default();
        if let Err(e) = init_certificates(&app) {
            log::error!("Couldn't set up certificate checks; HTTPS will fail: {e}");
        }
        let root = app
            .external_data_path()
            .or_else(|| app.internal_data_path())
            .expect("Android gives every app a files folder");
        turu_bot::paths::set_default_root(root.clone());
        let spawned = std::thread::Builder::new()
            .name("Turu Engine".into())
            .spawn(move || engine(&root));
        if let Err(e) = spawned {
            log::error!("Couldn't start the engine thread: {e}");
        }
    });
    // The activity has no UI yet: wait for it to go. The engine thread stays.
    let mut destroyed = false;
    while !destroyed {
        app.poll_events(None, |event| {
            if let PollEvent::Main(MainEvent::Destroy) = event {
                destroyed = true;
            }
        });
    }
}

fn init_logging() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("turu")
            .with_filter(
                android_logger::FilterBuilder::new()
                    // The desktop's default (`main.rs`): AQW's trace output and script errors
                    // stay out.
                    .parse("warn,turu=info,turu_bot=info,turu_frontend=info,turu_android=info,ruffle_aqw_perf=info,ruffle_core::avm2=off,ruffle_core::stub=off,avm_trace=off")
                    .build(),
            ),
    );
    log::info!("Turu {} starting", turu::VERSION);
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        log::error!("Panic on {}: {info}", thread.name().unwrap_or("<unnamed>"));
    }));
}

/// Lets reqwest check server certificates against Android's trust store.
fn init_certificates(app: &AndroidApp) -> jni::errors::Result<()> {
    // SAFETY: android-activity hands out the process's JavaVM.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    // A global reference: it can only be borrowed, not wrapped as a local one.
    let activity_raw = app.activity_as_ptr() as jobject;
    vm.attach_current_thread(|env| {
        // SAFETY: the activity outlives this call.
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw) }?;
        let context = env.new_local_ref(&*activity)?;
        rustls_platform_verifier::android::init_with_env(env, context)
    })
}

/// The engine thread: sets the bot up from `turu-args.txt`, then runs the player.
fn engine(root: &Path) {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            log::error!("Couldn't start the async runtime: {e}");
            return;
        }
    };
    let _guard = runtime.enter();
    tracing::info!("Turu {} on Android; data folder {}", turu::VERSION, root.display());

    let args_file = root.join(ARGS_FILE);
    let mut args = read_args(&args_file);
    turu::take_startup_args(&mut args);
    if !args.is_empty() {
        tracing::warn!("{ARGS_FILE}: {} arguments aren't Turu's flags; ignored", args.len());
    }
    // AQW (since its 2026-10-10 update) uses Flash APIs newer than its loader's SWF version.
    ruffle_core::set_show_all_apis(true);
    turu::configure_startup();
    turu::start_api();
    turu::start_monitor();
    turu::start_perf_log(None);

    let movie_url = match turu::skua_swf_url().map(|url| Url::parse(&url)) {
        Ok(Ok(url)) => url,
        Ok(Err(e)) => {
            tracing::error!("Bad skua.swf URL: {e}");
            return;
        }
        Err(e) => {
            tracing::error!("Couldn't write skua.swf: {e}");
            return;
        }
    };
    let (sender, events) = channel();
    let player = build_player(root, movie_url, sender.clone());
    turu::bot().set_host(Some(Arc::new(engine::flash_host(sender))));
    tracing::info!("Engine running");
    engine::run_loop(&player, &events);
    turu::bot().set_host(None);
}

/// `turu-args.txt`'s arguments: one per line, without blank lines and `#` comments. None when
/// the file isn't there.
fn read_args(file: &Path) -> Vec<OsString> {
    match std::fs::read_to_string(file) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(OsString::from)
            .collect(),
        Err(e) => {
            tracing::info!("No {} ({e}): the game loads, nothing logs in", file.display());
            Vec::new()
        }
    }
}

/// skua.swf's player, as `--headless` builds it with the desktop's defaults.
fn build_player(root: &Path, movie_url: Url, sender: Sender<EngineEvent>) -> Arc<Mutex<Player>> {
    let content = PlayingContent::DirectFile(ContentDescriptor {
        url: movie_url.clone(),
        root_content_path: None,
    });
    let navigator = ExternalNavigatorBackend::new(
        movie_url.clone(),
        None,
        None,
        Executor::new(sender),
        None,
        false,
        HashSet::new(),
        // The game server is a TCP socket; there's nobody to ask.
        SocketMode::Allow,
        Rc::new(content),
        NoDialogs,
    );
    let navigator = turu_frontend::capture::CaptureNavigator::new(navigator, Some(turu::bot().capture.clone()));
    let shared_objects: PathBuf = root.join("SharedObjects");
    let player = PlayerBuilder::new()
        .with_navigator(navigator)
        .with_renderer(NullRenderer::new(ViewportDimensions {
            width: STAGE.0,
            height: STAGE.1,
            scale_factor: 1.0,
        }))
        .with_ui(NullUiBackend::new())
        .with_storage(Box::new(DiskStorageBackend::new(shared_objects)))
        .with_external_interface(Box::new(turu::TuruExternalInterface { bot: turu::bot() }))
        .with_autoplay(true)
        .with_quality(StageQuality::Low)
        .with_max_execution_duration(Duration::MAX)
        .with_load_behavior(LoadBehavior::Streaming)
        .build();
    player
        .lock()
        .expect("the player was just built")
        .fetch_root_movie(movie_url.to_string(), Vec::new(), Box::new(|_| {}));
    player
}
