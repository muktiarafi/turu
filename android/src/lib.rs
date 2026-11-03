//! Turu on Android (docs/ANDROID_PROPOSAL.md, phase 2): the game on screen, with touch, keys and
//! sound; no Turu UI yet. The engine thread (`turu_frontend::engine`'s loop) owns the player for
//! as long as the process lives. `android_main` only forwards the activity's events to it: a
//! window to draw in (the player is built with the first one; later ones get a new surface),
//! the window going away (drawing stops, the game keeps running at the hidden-tab rate), touch
//! and keys.
//!
//! Setup comes from `turu-args.txt` in the app's files folder (the data folder, reachable with
//! `adb`): the desktop's flags (`--user`, `--password`, `--server`, `--run-script`, `--api`, ...),
//! one argument per line, `#` for comments, plus `--graphics auto|vulkan|gl`. The log goes to
//! logcat (tag `turu`).

#![cfg(target_os = "android")]

mod audio;
mod input;
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
use ruffle_render::backend::ViewportDimensions;
use ruffle_render::quality::StageQuality;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render_wgpu::target::SwapChainTarget;
use std::any::Any;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel, sync_channel};
use std::sync::{Arc, Condvar, Mutex, Once, OnceLock};
use std::time::Duration;
use turu_frontend::engine::{self, EngineEvent, Executor, NoDialogs};
use turu_frontend::turu;
use url::Url;
use wgpu::rwh::{AndroidDisplayHandle, HasWindowHandle, RawDisplayHandle};

/// The startup flags, in the data folder.
const ARGS_FILE: &str = "turu-args.txt";

static ENGINE: Once = Once::new();
/// Work for the engine thread's loop.
static ENGINE_TX: OnceLock<Sender<EngineEvent>> = OnceLock::new();
/// The window to draw in, while the activity has one.
static WINDOW: Mutex<Option<NativeWindow>> = Mutex::new(None);
static WINDOW_READY: Condvar = Condvar::new();
/// The window the renderer's surface was made for (its pointer), 0 for none.
static SURFACE_FOR: AtomicUsize = AtomicUsize::new(0);
/// Whether the player draws after each tick: there is a surface.
static DRAWING: AtomicBool = AtomicBool::new(false);

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
        let (sender, events) = channel();
        let _ = ENGINE_TX.set(sender);
        let engine_app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Engine".into())
            .spawn(move || engine(engine_app, &root, &events));
        if let Err(e) = spawned {
            log::error!("Couldn't start the engine thread: {e}");
        }
    });
    let mut destroyed = false;
    while !destroyed {
        app.poll_events(None, |event| match event {
            PollEvent::Main(MainEvent::InitWindow { .. }) => {
                *lock_window() = app.native_window();
                WINDOW_READY.notify_all();
                run_on_engine(attach_surface);
            }
            PollEvent::Main(MainEvent::TerminateWindow { .. }) => detach_surface(),
            PollEvent::Main(
                MainEvent::WindowResized { .. } | MainEvent::ConfigChanged { .. } | MainEvent::ContentRectChanged { .. },
            ) => {
                let density = density(&app);
                run_on_engine(move |player| resize(player, density));
            }
            PollEvent::Main(MainEvent::InputAvailable) => forward_input(&app),
            PollEvent::Main(MainEvent::Destroy) => destroyed = true,
            _ => {}
        });
    }
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
    run_on_engine(move |_| {
        DRAWING.store(false, Ordering::Release);
        SURFACE_FOR.store(0, Ordering::Release);
        // The game slows down like a hidden tab's (`api::options`).
        turu::bot().state.lock().hidden_tab = true;
        let _ = done.send(());
    });
    let _ = wait.recv_timeout(Duration::from_secs(2));
    *lock_window() = None;
}

/// Gives the renderer a surface on the current window, unless it has one there already.
fn attach_surface(player: &mut Player) {
    let Some(window) = lock_window().clone() else {
        return;
    };
    let ptr = window.ptr().as_ptr() as usize;
    if SURFACE_FOR.load(Ordering::Acquire) != ptr {
        let size = window_size(&window);
        let Some(renderer) = <dyn Any>::downcast_mut::<WgpuRenderBackend<SwapChainTarget>>(player.renderer_mut())
        else {
            return;
        };
        // SAFETY: the window stays valid until `detach_surface`, which stops drawing first.
        let made = unsafe { renderer.recreate_surface_unsafe(surface_target(&window), size) };
        if let Err(e) = made {
            tracing::error!("Couldn't draw in the new window: {e}");
            return;
        }
        SURFACE_FOR.store(ptr, Ordering::Release);
    }
    DRAWING.store(true, Ordering::Release);
    turu::bot().state.lock().hidden_tab = false;
}

fn resize(player: &mut Player, scale_factor: f64) {
    if let Some(window) = lock_window().as_ref() {
        let (width, height) = window_size(window);
        player.set_viewport_dimensions(ViewportDimensions {
            width,
            height,
            scale_factor,
        });
    }
}

fn forward_input(app: &AndroidApp) {
    let Ok(mut inputs) = app.input_events_iter() else {
        return;
    };
    while inputs.next(|event| {
        let events = match event {
            android_activity::input::InputEvent::MotionEvent(motion) => input::motion(motion).map(|e| vec![e]),
            android_activity::input::InputEvent::KeyEvent(key) => input::key(app, key),
            _ => None,
        };
        match events {
            Some(events) => {
                run_on_engine(move |player| {
                    for event in events {
                        player.handle_event(event);
                    }
                });
                InputStatus::Handled
            }
            None => InputStatus::Unhandled,
        }
    }) {}
}

fn density(app: &AndroidApp) -> f64 {
    app.config().density().map_or(1.0, |dpi| f64::from(dpi) / 160.0)
}

fn window_size(window: &NativeWindow) -> (u32, u32) {
    (window.width().max(1) as u32, window.height().max(1) as u32)
}

fn surface_target(window: &NativeWindow) -> wgpu::SurfaceTargetUnsafe {
    wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: window
            .window_handle()
            .expect("an Android window has a handle")
            .as_raw(),
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
                    .parse("warn,turu=info,turu_bot=info,turu_frontend=info,turu_android=info,ruffle_aqw_perf=info,ruffle_core::avm2=off,ruffle_core::stub=off,avm_trace=off,symphonia_bundle_mp3=error")
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

/// The graphics API: `--graphics` in `turu-args.txt`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Graphics {
    /// Vulkan, else OpenGL ES.
    Auto,
    Vulkan,
    Gl,
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

/// A renderer for `window`: the chosen API, and with Auto, OpenGL ES when Vulkan fails.
fn make_renderer(window: &NativeWindow, graphics: Graphics) -> Option<WgpuRenderBackend<SwapChainTarget>> {
    let tries: &[wgpu::Backends] = match graphics {
        Graphics::Auto => &[wgpu::Backends::VULKAN, wgpu::Backends::GL],
        Graphics::Vulkan => &[wgpu::Backends::VULKAN],
        Graphics::Gl => &[wgpu::Backends::GL],
    };
    for &backend in tries {
        // SAFETY: the window stays valid until `detach_surface`, which stops drawing first.
        let made = unsafe {
            WgpuRenderBackend::for_window_unsafe(
                surface_target(window),
                window_size(window),
                backend,
                wgpu::PowerPreference::HighPerformance,
                None,
            )
        };
        match made {
            Ok(renderer) => {
                tracing::info!("Graphics: {backend:?}");
                return Some(renderer);
            }
            Err(e) => tracing::warn!("Graphics: {backend:?} failed: {e}"),
        }
    }
    None
}

/// The engine thread: sets the bot up from `turu-args.txt`, waits for the first window, then runs
/// the player.
fn engine(app: AndroidApp, root: &Path, events: &std::sync::mpsc::Receiver<EngineEvent>) {
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
    let window = {
        let mut window = lock_window();
        while window.is_none() {
            window = WINDOW_READY.wait(window).unwrap_or_else(|e| e.into_inner());
        }
        window.clone().expect("waited for it")
    };
    let Some(renderer) = make_renderer(&window, graphics) else {
        tracing::error!("No graphics API worked; the game can't start");
        return;
    };
    SURFACE_FOR.store(window.ptr().as_ptr() as usize, Ordering::Release);
    DRAWING.store(true, Ordering::Release);
    let sender = ENGINE_TX.get().expect("set before the thread started").clone();
    let player = build_player(&app, root, movie_url, renderer, &window, sender.clone());
    turu::bot().set_host(Some(Arc::new(engine::flash_host(sender))));
    tracing::info!("Engine running");
    engine::run_loop_with(&player, events, |player| {
        if let Some(audio) = <dyn Any>::downcast_mut::<audio::AAudioAudioBackend>(player.audio_mut()) {
            audio.recreate_stream_if_needed();
        }
        if DRAWING.load(Ordering::Acquire) && player.needs_render() {
            player.render();
        }
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

/// skua.swf's player, drawing in `window`, with the desktop's defaults.
fn build_player(
    app: &AndroidApp,
    root: &Path,
    movie_url: Url,
    renderer: WgpuRenderBackend<SwapChainTarget>,
    window: &NativeWindow,
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
        .with_ui(ui::AndroidUiBackend::new(app.clone()))
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
        let (width, height) = window_size(window);
        player.set_viewport_dimensions(ViewportDimensions {
            width,
            height,
            scale_factor: density(app),
        });
        player.fetch_root_movie(movie_url.to_string(), Vec::new(), Box::new(|_| {}));
    }
    player
}
