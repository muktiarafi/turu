//! Turu on Android (docs/ANDROID_PROPOSAL.md, phase 4): the game on screen with Turu's nav bar
//! and bot windows over it, touch, keys and sound, and kept running in the background by a
//! foreground service (`notify.rs`). The engine thread (`turu_frontend::engine`'s loop) owns the
//! player and the screen (`screen.rs`) for as long as the process lives; the activity or the
//! service starts it ([`start_engine`]), whichever comes first, so it also runs with no window
//! (after Android killed the process, the service comes back alone). `android_main` forwards
//! the activity's events to it: a window to draw in (the first gets a screen, later ones a new
//! surface), the window going away (drawing stops, the game keeps running at the hidden-tab
//! rate), touch and keys.
//!
//! Setup comes from `turu-args.txt` in the app's files folder (the data folder, reachable with
//! `adb`): the desktop's flags (`--user`, `--password`, `--server`, `--run-script`, `--api`, ...),
//! one argument per line, `#` for comments, plus `--graphics auto|vulkan|gl`. The log goes to
//! logcat (tag `turu`).

#![cfg(target_os = "android")]

mod audio;
mod input;
mod memory;
mod notify;
mod platform;
mod screen;
mod ui;

use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use jni::JavaVM;
use jni::objects::JObject;
use jni::sys::jobject;
use ndk::native_window::NativeWindow;
use ruffle_core::backend::navigator::SocketMode;
use ruffle_core::config::Letterbox;
use ruffle_core::{LoadBehavior, Player, PlayerBuilder};
use ruffle_frontend_utils::backends::navigator::ExternalNavigatorBackend;
use ruffle_frontend_utils::backends::storage::DiskStorageBackend;
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_render::quality::StageQuality;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render::backend::ViewportDimensions;
use ruffle_render_wgpu::descriptors::Descriptors;
use screen::{Blit, Graphics, MovieTexture, SCREEN, Screen};
use std::any::Any;
use std::collections::HashSet;
use std::ffi::OsString;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Sender, channel, sync_channel};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::Duration;
use turu_frontend::engine::{self, EngineEvent, Executor, NoDialogs};
use turu_frontend::turu;
use url::Url;
use wgpu::rwh::{AndroidDisplayHandle, HasWindowHandle, RawDisplayHandle};

/// The startup flags, in the data folder.
const ARGS_FILE: &str = "turu-args.txt";

static ENGINE: Once = Once::new();
/// The activity's app, once it started (the service has none): the soft keyboard, the key
/// maps, the browser. Android makes a new activity for some configuration changes (a new
/// assets path, after boot), and its `android_main` puts its app here.
static APP: Mutex<Option<AndroidApp>> = Mutex::new(None);

thread_local! {
    /// The engine thread's GPU, for the screen the first window gets.
    static GPU: RefCell<Option<(Arc<Descriptors>, Arc<Blit>)>> = const { RefCell::new(None) };
}

/// The latest activity's app, when one has started.
pub(crate) fn app() -> Option<AndroidApp> {
    APP.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
/// Work for the engine thread's loop.
static ENGINE_TX: OnceLock<Sender<EngineEvent>> = OnceLock::new();
/// The window to draw in, while the activity has one.
static WINDOW: Mutex<Option<NativeWindow>> = Mutex::new(None);

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    init_logging();
    *APP.lock().unwrap_or_else(|e| e.into_inner()) = Some(app.clone());
    if let Err(e) = init_certificates(&app) {
        log::error!("Couldn't set up certificate checks; HTTPS will fail: {e}");
    }
    let root = app
        .external_data_path()
        .or_else(|| app.internal_data_path())
        .expect("Android gives every app a files folder");
    start_engine(root);
    let mut destroyed = false;
    while !destroyed {
        app.poll_events(None, |event| match event {
            PollEvent::Main(MainEvent::InitWindow { .. }) => {
                *lock_window() = app.native_window();
                run_on_engine(|_| attach_surface());
            }
            PollEvent::Main(MainEvent::TerminateWindow { .. }) => detach_surface(),
            PollEvent::Main(
                MainEvent::WindowResized { .. } | MainEvent::ConfigChanged { .. } | MainEvent::ContentRectChanged { .. },
            ) => {
                run_on_engine(|_| resize());
            }
            PollEvent::Main(MainEvent::InputAvailable) => forward_input(&app),
            PollEvent::Main(MainEvent::Destroy) => destroyed = true,
            _ => {}
        });
    }
}

/// Starts the engine thread once per process, with `root` as the data folder. The caller set up
/// certificate checks first (they need a Java context).
pub(crate) fn start_engine(root: PathBuf) {
    ENGINE.call_once(|| {
        // Before any reqwest client is built (Search Scripts, the navigator).
        let _ = rustls::crypto::ring::default_provider().install_default();
        turu_bot::paths::set_default_root(root.clone());
        let _ = CRASH_DIR.set(turu_bot::paths::Paths::new().crash_dir());
        let (sender, events) = channel();
        let _ = ENGINE_TX.set(sender);
        let spawned = std::thread::Builder::new()
            .name("Turu Engine".into())
            .spawn(move || engine(&root, &events));
        if let Err(e) = spawned {
            log::error!("Couldn't start the engine thread: {e}");
        }
    });
}

fn lock_window() -> std::sync::MutexGuard<'static, Option<NativeWindow>> {
    WINDOW.lock().unwrap_or_else(|e| e.into_inner())
}

fn run_on_engine(work: impl FnOnce(&mut Player) + Send + 'static) {
    if let Some(sender) = ENGINE_TX.get() {
        let _ = sender.send(EngineEvent::Run(Box::new(work)));
    }
}

/// Stops drawing before Android takes the window away: the engine acknowledges between ticks
/// (or not at all while it has no player yet, which draws nothing anyway).
fn detach_surface() {
    let (done, wait) = sync_channel(1);
    run_on_engine(move |player| {
        with_screen(Screen::detach);
        // The game slows down like a hidden tab's (`api::options`).
        turu::bot().state.lock().hidden_tab = true;
        let _ = done.send(());
        // Nothing is drawn until a window comes back. After the answer: it waits for the GPU.
        memory::trim(player, "the window went away");
    });
    let _ = wait.recv_timeout(Duration::from_secs(2));
    *lock_window() = None;
}

fn with_screen(work: impl FnOnce(&mut Screen)) {
    SCREEN.with_borrow_mut(|screen| {
        if let Some(screen) = screen {
            work(screen);
        }
    });
}

/// Draws in the current window from now on: the first one gets the screen.
fn attach_surface() {
    let Some(window) = lock_window().clone() else {
        return;
    };
    let made = SCREEN.with_borrow_mut(|screen| match screen {
        Some(screen) => {
            screen.attach(&window);
            true
        }
        None => {
            let gpu = GPU.with_borrow(Clone::clone);
            let density = app().as_ref().map_or(1.0, density) as f32;
            *screen = gpu.and_then(|(descriptors, blit)| Screen::new(descriptors, blit, &window, density));
            screen.is_some()
        }
    });
    if made {
        turu::bot().state.lock().hidden_tab = false;
    }
}

fn resize() {
    if let Some(window) = lock_window().as_ref() {
        with_screen(|screen| screen.resize(window));
    }
}

fn forward_input(app: &AndroidApp) {
    let Ok(mut inputs) = app.input_events_iter() else {
        return;
    };
    while inputs.next(|event| match event {
        android_activity::input::InputEvent::MotionEvent(motion) => {
            let touches = input::motion(motion);
            if touches.is_empty() {
                return InputStatus::Unhandled;
            }
            run_on_engine(move |player| {
                with_screen(|screen| touches.into_iter().for_each(|touch| screen.touch(player, touch)))
            });
            InputStatus::Handled
        }
        android_activity::input::InputEvent::KeyEvent(key) => match input::key(app, key) {
            Some(key) => {
                run_on_engine(move |player| with_screen(|screen| screen.key(player, &key)));
                InputStatus::Handled
            }
            None => InputStatus::Unhandled,
        },
        _ => InputStatus::Unhandled,
    }) {}
}

fn density(app: &AndroidApp) -> f64 {
    app.config().density().map_or(1.0, |dpi| f64::from(dpi) / 160.0)
}

pub(crate) fn window_size(window: &NativeWindow) -> (u32, u32) {
    (window.width().max(1) as u32, window.height().max(1) as u32)
}

pub(crate) fn surface_target(window: &NativeWindow) -> wgpu::SurfaceTargetUnsafe {
    wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: window
            .window_handle()
            .expect("an Android window has a handle")
            .as_raw(),
    }
}

/// Logcat (tag `turu`) and panics there; once per process (the activity and the service both
/// call it).
pub(crate) fn init_logging() {
    static LOGGING: Once = Once::new();
    LOGGING.call_once(init_logging_now);
}

fn init_logging_now() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("turu")
            .with_filter(
                android_logger::FilterBuilder::new()
                    // The desktop's default (`main.rs`): AQW's trace output and script errors
                    // stay out.
                    .parse("warn,turu=info,turu_bot=info,turu_frontend=info,turu_android=info,ruffle_aqw_perf=info,ruffle_core::avm2=off,ruffle_core::stub=off,avm_trace=off,symphonia_bundle_mp3=error")
                    .build(),
            ),
    );
    log::info!("Turu {} starting", turu::VERSION);
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("<unnamed>");
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("Panic on {thread}: {info}\n{backtrace}");
        write_panic_report(thread, info, &backtrace);
        notify::panicked(&format!("Panic on {thread}: {info}"));
    }));
}

/// The data folder's `crash/`, once the engine set the data folder (a panic before has only
/// logcat).
static CRASH_DIR: OnceLock<PathBuf> = OnceLock::new();

/// `crash/panic-<time>.txt`, written now: a release build aborts right after the hook, and
/// logcat's buffer is gone by the time anyone looks (`turu_frontend::crash_reports`).
fn write_panic_report(thread: &str, info: &std::panic::PanicHookInfo, backtrace: &std::backtrace::Backtrace) {
    let Some(dir) = CRASH_DIR.get() else {
        return;
    };
    let now = chrono::Local::now();
    let message = info.payload_as_str().unwrap_or("panic");
    let first_line = message.lines().next().unwrap_or_default();
    let report = format!(
        "Turu crashed (a panic on {thread}): {first_line}\n\nTuru {}\n{}\nthread '{thread}': {info}\n\n{backtrace}\n",
        turu::VERSION,
        now.to_rfc3339(),
    );
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join(format!("panic-{}.txt", now.format("%F_%H-%M-%S"))), report);
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

/// Takes `--graphics <auto|vulkan|gl>` (or `--graphics=...`) out of `args`.
fn take_graphics(args: &mut Vec<OsString>) -> Graphics {
    let mut graphics = Graphics::Auto;
    let mut rest = Vec::with_capacity(args.len());
    let mut iter = std::mem::take(args).into_iter();
    while let Some(arg) = iter.next() {
        let value = match arg.to_str() {
            Some("--graphics") => iter.next().and_then(|v| v.into_string().ok()),
            Some(text) if text.starts_with("--graphics=") => Some(text["--graphics=".len()..].to_owned()),
            _ => {
                rest.push(arg);
                continue;
            }
        };
        graphics = match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
            Some("vulkan") => Graphics::Vulkan,
            Some("gl" | "gles" | "opengl") => Graphics::Gl,
            Some("auto") => Graphics::Auto,
            other => {
                tracing::warn!("--graphics {other:?}: expected auto, vulkan or gl");
                Graphics::Auto
            }
        };
    }
    *args = rest;
    graphics
}

/// The engine thread: sets the bot up from `turu-args.txt`, then runs the player, drawing while
/// there is a window.
fn engine(root: &Path, events: &std::sync::mpsc::Receiver<EngineEvent>) {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            log::error!("Couldn't start the async runtime: {e}");
            return;
        }
    };
    let _guard = runtime.enter();
    // The UI runs here too: a Flash call from this thread would wait for itself.
    turu::init_main_thread();
    turu_frontend::platform::set(Arc::new(platform::AndroidPlatform));
    tracing::info!("Turu {} on Android; data folder {}", turu::VERSION, root.display());

    let args_file = root.join(ARGS_FILE);
    let mut args = read_args(&args_file);
    let graphics = take_graphics(&mut args);
    turu::take_startup_args(&mut args);
    if !args.is_empty() {
        tracing::warn!("{ARGS_FILE}: {} arguments aren't Turu's flags; ignored", args.len());
    }
    // AQW (since its 2026-10-10 update) uses Flash APIs newer than its loader's SWF version.
    ruffle_core::set_show_all_apis(true);
    // Mugetsu's defaults (`main.rs`), per thread, so here, on the player's: how long a frame
    // waits for its new shapes, and Flash-style edge smoothing.
    ruffle_core::set_shape_wait(Some(Duration::from_millis(12)));
    ruffle_render::tessellator::set_flash_aa(true);
    turu::configure_startup();
    turu::start_api();
    turu::start_monitor();
    turu::start_client_updates();
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
    // skua.swf's stage until a window sets the game's size.
    let Some((descriptors, blit, renderer)) = screen::gpu(graphics, engine::STAGE) else {
        tracing::error!("No graphics API worked; the game can't start");
        return;
    };
    GPU.set(Some((descriptors, blit)));
    let sender = ENGINE_TX.get().expect("set before the thread started").clone();
    let player = build_player(root, movie_url, renderer, sender.clone());
    // A window that came while the engine started up.
    attach_surface();
    turu::bot().set_host(Some(Arc::new(engine::flash_host(sender))));
    tracing::info!("Engine running");
    memory::init();
    engine::run_loop_with(&player, events, |player| {
        memory::tick(player);
        if let Some(audio) = <dyn Any>::downcast_mut::<audio::AAudioAudioBackend>(player.audio_mut()) {
            audio.recreate_stream_if_needed();
        }
        with_screen(|screen| screen.draw(player));
    });
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
            tracing::info!("No {} ({e}): log in on the game's screen", file.display());
            Vec::new()
        }
    }
}

/// skua.swf's player, drawing with `renderer`, with the desktop's defaults. Its viewport is
/// skua.swf's stage until the screen's first frame sets it.
fn build_player(
    root: &Path,
    movie_url: Url,
    renderer: WgpuRenderBackend<MovieTexture>,
    sender: Sender<EngineEvent>,
) -> Arc<Mutex<Player>> {
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
    let mut builder = PlayerBuilder::new()
        .with_navigator(navigator)
        .with_renderer(renderer)
        .with_ui(ui::AndroidUiBackend::new())
        .with_storage(Box::new(DiskStorageBackend::new(root.join("SharedObjects"))))
        .with_external_interface(Box::new(turu::TuruExternalInterface { bot: turu::bot() }))
        .with_autoplay(true)
        .with_letterbox(Letterbox::On)
        .with_quality(StageQuality::High)
        .with_max_execution_duration(Duration::MAX)
        .with_load_behavior(LoadBehavior::Streaming);
    match audio::AAudioAudioBackend::new() {
        Ok(audio) => builder = builder.with_audio(audio),
        Err(e) => tracing::warn!("No sound: {e}"),
    }
    let player = builder.build();
    {
        let mut player = player.lock().expect("the player was just built");
        player.set_viewport_dimensions(ViewportDimensions {
            width: engine::STAGE.0,
            height: engine::STAGE.1,
            scale_factor: 1.0,
        });
        player.fetch_root_movie(movie_url.to_string(), Vec::new(), Box::new(|_| {}));
    }
    player
}
