//! Turu on Android (docs/ANDROID_PROPOSAL.md, phase 4): the game on screen with Turu's nav bar
//! and bot windows over it, touch, keys and sound, and kept running in the background by a
//! foreground service (`notify.rs`). The engine thread (`turu_frontend::engine`'s loop) owns the
//! tabs' players (one game client each: [`tab_request`], `tabs.rs`) and the screen (`screen.rs`) for as long as the process lives; the activity or the
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
mod frame_perf;
mod game_field;
mod input;
mod memory;
mod notify;
mod pan;
mod platform;
mod restart;
mod saf;
mod screen;
mod storage;
use turu_frontend::saved_tabs as tabs;
mod ui;
mod ui_field;

use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use jni::JavaVM;
use jni::objects::JObject;
use jni::sys::jobject;
use ndk::native_window::NativeWindow;
use ruffle_core::backend::navigator::SocketMode;
use ruffle_core::config::Letterbox;
use ruffle_core::{LoadBehavior, Player, PlayerBuilder, PlayerEvent};
use ruffle_frontend_utils::backends::navigator::ExternalNavigatorBackend;
use ruffle_frontend_utils::backends::storage::DiskStorageBackend;
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_render::backend::ViewportDimensions;
use ruffle_render::quality::StageQuality;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render_wgpu::descriptors::Descriptors;
use screen::{Blit, Graphics, MovieTexture, SCREEN, Screen};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Sender, channel, sync_channel};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_frontend::engine::{self, EngineEvent, Executor, NoDialogs};
use turu_frontend::players::{Players, Slot};
use turu_frontend::turu::{self, TabId, TabRequest};
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
    /// The game's volume while it's muted in the background, to give back on return.
    static BACKGROUND_MUTED: Cell<Option<f32>> = const { Cell::new(None) };
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
                run_on_engine(attach_surface);
            }
            PollEvent::Main(MainEvent::TerminateWindow { .. }) => detach_surface(),
            PollEvent::Main(
                MainEvent::WindowResized { .. }
                | MainEvent::ConfigChanged { .. }
                | MainEvent::ContentRectChanged { .. },
            ) => {
                run_on_engine(resize);
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
        storage::install();
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
        set_heard(player, false);
        set_touchable(player, false);
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

/// Mutes the game while no window shows it, as a hidden desktop tab (`set_tab_volumes`), and gives
/// its volume back when one does. Notify Drop's beep has a stream of its own and still sounds.
/// Not a paused stream: Ruffle's mixer only lets a sound finish by playing it, so sounds started
/// while paused would pile up (32 at most, then no new ones) and all play at once on return.
fn set_heard(player: &mut Player, heard: bool) {
    match (heard, BACKGROUND_MUTED.get()) {
        (true, Some(volume)) => {
            player.set_volume(volume);
            BACKGROUND_MUTED.set(None);
            tracing::info!("Sound back on");
        }
        (false, None) => {
            BACKGROUND_MUTED.set(Some(player.volume()));
            player.set_volume(0.0);
            tracing::info!("Sound muted while no window shows the game");
        }
        _ => {}
    }
}

/// Takes the pointer off the stage while no window shows the game, as `give_mouse` does for a hidden
/// desktop tab, and puts it back when one does. Ruffle starts with it on the stage, and then every
/// update hit-tests the whole display list for it (`run_mouse_pick`): in the background, at 2 FPS,
/// that was 75% of the player's time (measured with `--headless`, which has the same loop).
fn set_touchable(player: &mut Player, on: bool) {
    if player.mouse_in_stage() != on {
        player.set_mouse_in_stage(on);
        if !on {
            player.handle_event(PlayerEvent::MouseLeave);
        }
    }
}

/// Draws in the current window from now on: the first one gets the screen. Without a window (the
/// service started the engine alone), the game stays muted.
fn attach_surface(player: &mut Player) {
    let Some(window) = lock_window().clone() else {
        set_heard(player, false);
        set_touchable(player, false);
        return;
    };
    set_heard(player, true);
    set_touchable(player, true);
    let made = SCREEN.with_borrow_mut(|screen| match screen {
        Some(screen) => {
            screen.attach(&window);
            true
        }
        None => {
            let gpu = GPU.with_borrow(Clone::clone);
            let density = app().as_ref().map_or(1.0, density) as f32;
            *screen = gpu
                .and_then(|(descriptors, blit)| Screen::new(descriptors, blit, &window, density));
            screen.is_some()
        }
    });
    if made {
        turu::bot().state.lock().hidden_tab = false;
    }
}

fn resize(player: &mut Player) {
    // A copy: the lock isn't held while drawing.
    let Some(window) = lock_window().clone() else {
        return;
    };
    with_screen(|screen| screen.resize(&window, player));
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
                with_screen(|screen| {
                    touches
                        .into_iter()
                        .for_each(|touch| screen.touch(player, touch))
                })
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
    app.config()
        .density()
        .map_or(1.0, |dpi| f64::from(dpi) / 160.0)
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
    static LOGGER: OnceLock<GpuCrumbs> = OnceLock::new();
    let logger = LOGGER.get_or_init(|| {
        GpuCrumbs(android_logger::AndroidLogger::new(
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
        ))
    });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
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

/// Logcat, plus ruffle-aqw's drawing warnings (`ruffle_aqw_perf`: the GPU out of memory, an error
/// tolerated) as breadcrumbs of the next crash report: the panic itself only shows the last step,
/// not the out-of-memory error that led to it. At most `MAX` per process.
struct GpuCrumbs(android_logger::AndroidLogger);

impl log::Log for GpuCrumbs {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.0.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        use std::sync::atomic::{AtomicU32, Ordering};
        const MAX: u32 = 100;
        static SENT: AtomicU32 = AtomicU32::new(0);
        if record.level() <= log::Level::Warn
            && record.target().starts_with("ruffle_aqw_perf")
            && self.0.enabled(record.metadata())
            && SENT.fetch_add(1, Ordering::Relaxed) < MAX
        {
            notify::breadcrumb("gpu", &record.args().to_string());
        }
        self.0.log(record);
    }

    fn flush(&self) {
        self.0.flush();
    }
}

/// The data folder's `crash/`, once the engine set the data folder (a panic before has only
/// logcat).
static CRASH_DIR: OnceLock<PathBuf> = OnceLock::new();

/// `crash/panic-<time>.txt`, written now: a release build aborts right after the hook, and
/// logcat's buffer is gone by the time anyone looks (`turu_frontend::crash_reports`).
fn write_panic_report(
    thread: &str,
    info: &std::panic::PanicHookInfo,
    backtrace: &std::backtrace::Backtrace,
) {
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
    let _ = std::fs::write(
        dir.join(format!("panic-{}.txt", now.format("%F_%H-%M-%S"))),
        report,
    );
}

/// Runs `work` on this thread attached to the JavaVM, under the thread's own name. A thread
/// attached without one gets "Thread-N" from Android, as its Java name and in `/proc` too: the
/// engine's calls into Java (breadcrumbs, the notification, the GPU-lost notice) renamed "Turu
/// Engine", and the memory log's busiest threads said "Thread-3" (Sentry TURU-ANDROID-B). The
/// name counts on the first attach, which stays for the thread's life.
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

/// Lets reqwest check server certificates against Android's trust store.
fn init_certificates(app: &AndroidApp) -> jni::errors::Result<()> {
    // SAFETY: android-activity hands out the process's JavaVM.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    // A global reference: it can only be borrowed, not wrapped as a local one.
    let activity_raw = app.activity_as_ptr() as jobject;
    attach(&vm, |env| {
        // SAFETY: the activity outlives this call.
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw) }?;
        let context = env.new_local_ref(&*activity)?;
        rustls_platform_verifier::android::init_with_env(env, context)
    })
}

/// Takes every `flag` out of `args`; whether there was one.
fn take_flag(args: &mut Vec<OsString>, flag: &str) -> bool {
    let before = args.len();
    args.retain(|arg| arg.to_str() != Some(flag));
    args.len() != before
}

/// Takes `flag <value>` (or `flag=value`) out of `args`, returning the value.
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

/// Takes `--graphics <auto|vulkan|gl>` (or `--graphics=...`) out of `args`.
fn take_graphics(args: &mut Vec<OsString>) -> Graphics {
    let mut graphics = Graphics::Auto;
    let mut rest = Vec::with_capacity(args.len());
    let mut iter = std::mem::take(args).into_iter();
    while let Some(arg) = iter.next() {
        let value = match arg.to_str() {
            Some("--graphics") => iter.next().and_then(|v| v.into_string().ok()),
            Some(text) if text.starts_with("--graphics=") => {
                Some(text["--graphics=".len()..].to_owned())
            }
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
    // Rayon's pool (ruffle-aqw finishes command buffers and uploads textures on it) is made by the
    // first thread to use it, this one, and Linux gives new threads their maker's name: its
    // workers were "Turu Engine" too, and the memory log's busiest threads added their CPU to the
    // engine's. Made first, with names of their own.
    if let Err(e) = rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("Turu Rayon {i}"))
        .build_global()
    {
        log::warn!("Couldn't name rayon's threads: {e}");
    }
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
    tracing::info!(
        "Turu {} on Android; data folder {}",
        turu::VERSION,
        root.display()
    );

    let args_file = root.join(ARGS_FILE);
    let mut args = read_args(&args_file);
    let graphics = take_graphics(&mut args);
    let safe_drawing =
        take_flag(&mut args, "--safe-drawing") || root.join(SAFE_DRAWING_FILE).exists();
    // Destroys the GPU device 20 s after the start, to test what a lost device does.
    let mut test_gpu_loss =
        take_flag(&mut args, "--test-gpu-loss").then(|| Instant::now() + Duration::from_secs(20));
    let perf_log = take_value(&mut args, "--perf-log");
    let releases_url = take_value(&mut args, "--releases-url");
    restart::prepare(&mut args);
    turu::take_startup_args(&mut args);
    if !args.is_empty() {
        tracing::warn!(
            "{ARGS_FILE}: {} arguments aren't Turu's flags; ignored",
            args.len()
        );
    }
    // AQW (since its 2026-10-10 update) uses Flash APIs newer than its loader's SWF version.
    ruffle_core::set_show_all_apis(true);
    // Mugetsu's defaults (`main.rs`), per thread, so here, on the player's: how long a frame
    // waits for its new shapes, and Flash-style edge smoothing.
    ruffle_core::set_shape_wait(Some(Duration::from_millis(12)));
    ruffle_render::tessellator::set_flash_aa(true);
    if safe_drawing {
        for var in turu_frontend::platform::SAFE_DRAWING {
            // SAFETY: before the engine's threads exist and before Ruffle reads them; the
            // process's other threads (Java's) don't read Turu's environment.
            unsafe { std::env::set_var(var, "1") };
        }
    }
    // `--releases-url <url>`: `TURU_RELEASES_URL`, where client updates are looked for (a test
    // feed, say), which an app can't be started with.
    if let Some(url) = releases_url {
        // SAFETY: as Safe Drawing's, before the update checks' thread exists.
        unsafe { std::env::set_var("TURU_RELEASES_URL", url) };
    }
    ruffle_core::frame_interpolation::set_enabled(root.join(INTERPOLATION_FILE).exists());
    turu::configure_startup();
    restart::start();
    turu::start_api();
    turu::start_monitor();
    turu::start_client_updates();
    // `--perf-log <seconds>`: `TURU_PERF_LOG`, which an app can't be started with.
    if let Some(seconds) = perf_log {
        // SAFETY: before the engine's threads exist (the perf log's is started just below).
        unsafe { std::env::set_var("TURU_PERF_LOG", seconds) };
    }
    turu::start_perf_log(Some({
        // The engine thread as a whole while there's a window (`frame_perf`).
        let mut last = frame_perf::snapshot();
        Box::new(move |_| {
            let now = frame_perf::snapshot();
            let report = now.report(&last);
            last = now;
            report
        }) as turu::ThreadReport
    }));

    let movie_url = match turu::host_swf_url().map(|url| Url::parse(&url)) {
        Ok(Ok(url)) => url,
        Ok(Err(e)) => {
            tracing::error!("Bad host SWF URL: {e}");
            return;
        }
        Err(e) => {
            tracing::error!("Couldn't write the host SWF: {e}");
            return;
        }
    };
    // turu.swf's stage until a window sets the game's size.
    let Some((descriptors, blit, renderer)) = screen::gpu(graphics, engine::STAGE) else {
        tracing::error!("No graphics API worked; the game can't start");
        return;
    };
    if safe_drawing {
        turu::bot()
            .logs
            .debug
            .push("Safe Drawing: Mugetsu's drawing speed-ups are off");
    }
    GPU.set(Some((descriptors, blit)));
    let sender = ENGINE_TX
        .get()
        .expect("set before the thread started")
        .clone();
    // The first tab; the UI's "+", Army > Accounts and Army Control open and select more.
    let tab = turu::active_tab();
    let bot = turu::bot();
    let mut players = Players::new();
    let player = build_player(root, &movie_url, renderer, sender.clone(), tab, &bot);
    players.add(Slot::new(Some(tab), player, ()));
    let requests = Mutex::new(sender.clone());
    turu::set_tab_requests(Box::new(move |request| {
        requests
            .lock()
            .is_ok_and(|sender| sender.send(EngineEvent::Tab(request)).is_ok())
    }));
    // A window that came while the engine started up.
    if let Some(mut player) = players.get() {
        attach_surface(&mut player);
    }
    bot.set_host(Some(Arc::new(engine::flash_host(sender.clone(), tab))));
    tracing::info!("Engine running");
    memory::init();
    // Frames the active tab's player had run at its last tick, for `--perf-log`'s frame count;
    // taken again when a tab request may have made another player the active one.
    let frames_seen = Cell::new(players.get().map_or(0, |player| player.frames_run()));
    // The tabs open before Android stopped the app, after this one (`android-tabs.json`).
    tabs::reopen();
    engine::run_loop_paced(
        &mut players,
        events,
        |player, step| {
            if let engine::Step::Ticked { frame_ms } = step {
                if let Some(frame_ms) = frame_ms {
                    frame_perf::frames(
                        player.frames_run().saturating_sub(frames_seen.get()),
                        Duration::from_secs_f64(frame_ms / 1000.0),
                        Duration::from_secs_f64(1.0 / player.frame_rate().max(1.0)),
                    );
                }
                frames_seen.set(player.frames_run());
                if test_gpu_loss.is_some_and(|at| Instant::now() >= at) {
                    test_gpu_loss = None;
                    tracing::warn!("--test-gpu-loss: destroying the GPU device");
                    GPU.with_borrow(|gpu| {
                        gpu.as_ref()
                            .map(|(descriptors, _)| descriptors.device.destroy())
                    });
                }
                memory::tick(player);
                if let Some(audio) =
                    <dyn Any>::downcast_mut::<audio::AAudioAudioBackend>(player.audio_mut())
                {
                    audio.recreate_stream_if_needed();
                }
            }
            SCREEN.with_borrow_mut(|screen| {
                screen.as_mut().and_then(|screen| screen.step(player, step))
            })
        },
        |players, request| {
            tab_request(players, request, root, &movie_url, &sender);
            frames_seen.set(players.get().map_or(0, |player| player.frames_run()));
        },
    );
    for (_, bot) in turu::tabs() {
        bot.set_host(None);
    }
}

/// Opens, selects or closes a tab, as the desktop's window does (`app.rs`, `tab_request`): one
/// player per tab, the active one drawn on the screen and heard, the others muted and ticked at
/// the hidden tab's rate. A new tab over Options > Application > Tab Limit is refused.
fn tab_request(
    players: &mut Players<()>,
    request: TabRequest,
    root: &Path,
    movie_url: &Url,
    sender: &Sender<EngineEvent>,
) {
    match request {
        TabRequest::Open(startup) => {
            let limit = turu::bot().settings.read().tab_limit.clamp(1, 4) as usize;
            if players.len() >= limit {
                let message = format!(
                    "Not opening a tab: {limit} are open, the Tab Limit (Options > Application). Each one takes about 400 MB."
                );
                tracing::warn!("{message}");
                turu::bot().log(message);
                return;
            }
            let Some(renderer) = GPU.with_borrow(|gpu| {
                let (descriptors, blit) = gpu.as_ref()?;
                let target = MovieTexture::new(
                    blit.clone(),
                    &descriptors.device,
                    engine::STAGE.0,
                    engine::STAGE.1,
                );
                WgpuRenderBackend::new(descriptors.clone(), target)
                    .inspect_err(|e| tracing::error!("Couldn't make the new tab's renderer: {e}"))
                    .ok()
            }) else {
                return;
            };
            leave_active(players);
            let (tab, bot) = turu::add_tab(*startup);
            let player = build_player(root, movie_url, renderer, sender.clone(), tab, &bot);
            players.add(Slot::new(Some(tab), player, ()));
            bot.set_host(Some(Arc::new(engine::flash_host(sender.clone(), tab))));
            turu::set_active_tab(tab);
            tracing::info!("Opened a tab ({} open)", players.len());
        }
        TabRequest::Select(tab) => {
            if players.active().and_then(Slot::tab) == Some(tab) {
                return;
            }
            leave_active(players);
            if players.select_tab(tab) {
                turu::set_active_tab(tab);
            }
        }
        TabRequest::Close(tab) => {
            if players.len() < 2 {
                return;
            }
            if players.active().and_then(Slot::tab) == Some(tab) {
                leave_active(players);
            }
            // The player first: it detaches the tab's bot, which `remove_tab` stops.
            players.remove_tab(tab);
            turu::remove_tab(tab);
            let active = turu::active_tab();
            turu::set_active_tab(active);
            players.select_tab(active);
        }
        // No Grid View on a phone: the tiles would be too small.
        TabRequest::Grid(_) => return,
    }
    // Only the active tab is heard, and only while a window shows it (`set_heard`: its volume
    // comes back when one does).
    let shown = window_shown();
    for (active, mut player) in players.all() {
        player.set_volume(if active && shown { 1.0 } else { 0.0 });
    }
    if !shown {
        BACKGROUND_MUTED.set(Some(1.0));
    }
    // Without a window the active tab's game slows down too, as the one tab's did.
    if !window_shown() {
        turu::bot().state.lock().hidden_tab = true;
    }
    tabs::save();
}

/// The active tab's player stops being the one on screen: its fingers are let go, and the next
/// one gets the screen's size.
fn leave_active(players: &Players<()>) {
    if let Some(mut player) = players.get() {
        with_screen(|screen| screen.leave(&mut player));
    }
}

/// Whether the game is drawn in a window now.
fn window_shown() -> bool {
    SCREEN.with_borrow(|screen| screen.as_ref().is_some_and(Screen::shown))
}

/// Frame interpolation (Options > Application; off by default) is on while this file is in the
/// data folder, as Mugetsu's on a desktop.
const INTERPOLATION_FILE: &str = "mugetsu-interpolation.txt";
/// Options > Application > Safe Drawing, for the next start (as `--safe-drawing` does).
pub(crate) const SAFE_DRAWING_FILE: &str = "safe-drawing.txt";

/// Saves frame interpolation's switch (`platform.rs`).
pub(crate) fn save_interpolation(on: bool) {
    save_switch(INTERPOLATION_FILE, on);
}

/// Saves a switch read at start: the file in the data folder while it's on.
pub(crate) fn save_switch(file: &str, on: bool) {
    let path = turu_bot::paths::Paths::new().root.join(file);
    let result = if on {
        std::fs::write(&path, "on\n")
    } else {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    };
    if let Err(e) = result {
        tracing::warn!("Couldn't save {}: {e}", path.display());
    }
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

/// turu.swf's player, drawing with `renderer`, with the desktop's defaults. Its viewport is
/// turu.swf's stage until the screen's first frame sets it.
fn build_player(
    root: &Path,
    movie_url: &Url,
    renderer: WgpuRenderBackend<MovieTexture>,
    sender: Sender<EngineEvent>,
    tab: TabId,
    bot: &Arc<Bot>,
) -> Arc<Mutex<Player>> {
    let content = PlayingContent::DirectFile(ContentDescriptor {
        url: movie_url.clone(),
        root_content_path: None,
    });
    let navigator = ExternalNavigatorBackend::new(
        movie_url.clone(),
        None,
        None,
        Executor::new(sender, tab),
        None,
        false,
        HashSet::new(),
        // The game server is a TCP socket; there's nobody to ask.
        SocketMode::Allow,
        Rc::new(content),
        NoDialogs,
    )
    .with_http_cache(turu_frontend::game_files::cache());
    let navigator =
        turu_frontend::capture::CaptureNavigator::new(navigator, Some(bot.capture.clone()));
    let mut builder = PlayerBuilder::new()
        .with_navigator(navigator)
        .with_renderer(renderer)
        .with_ui(ui::AndroidUiBackend::new())
        .with_storage(Box::new(DiskStorageBackend::new(
            root.join("SharedObjects"),
        )))
        .with_external_interface(Box::new(turu::TuruExternalInterface { bot: bot.clone() }))
        // Turu: the game's packets, from its socket (R5).
        .with_socket_observer(Box::new(turu::TuruSocketObserver { bot: bot.clone() }))
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
