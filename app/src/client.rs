//! Turu: the client mode (`--client=<address>`, docs/TAB_PROCESSES_PROPOSAL.md, M2). One tab's
//! game and bot without a window, as `--headless` runs them, but drawn: the player renders on a
//! GPU set up without a window, and `turu_frontend::client_screen` draws it with Turu's UI into
//! pictures for the shell (`shell.rs`), which shows them and sends back input. The process quits
//! when the shell closes the connection.
//!
//! The shell's messages are read on a thread of their own and run on the engine thread
//! (`EngineEvent::Run`); pictures go out on another (`FrameSender`), newest first, so the engine
//! never waits on the shell. `Dump` (debugging) writes the next picture to a PNG.

use crate::backends::ClientUiBackend;
use crate::headless::build_player;
use crate::preferences::GlobalPreferences;
use crate::turu;
use anyhow::{Context as _, Error, anyhow};
use ruffle_render::quality::StageQuality;
use ruffle_render_wgpu::backend::{WgpuRenderBackend, create_wgpu_instance, request_adapter_and_device};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::utils::{format_list, get_backend_names};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use turu_frontend::client_screen::{ClientScreen, Picture};
use turu_frontend::engine::{self, EngineEvent, STAGE};
use turu_frontend::movie_texture::{Blit, MovieTexture};
use turu_frontend::platform::{Answer, FileDialog, Platform};
use turu_frontend::players::{Players, Slot};
use turu_frontend::shell::{self, ClientEnd, FrameSender, ToClient, ToShell};
use url::Url;

/// The engine thread's events, for what Options changes in the game.
static ENGINE: std::sync::OnceLock<Sender<EngineEvent>> = std::sync::OnceLock::new();

/// The client's GPU device, for `TURU_TEST_GPU_LOSS` (see `gpu_check`).
static GPU_DEVICE: std::sync::OnceLock<wgpu::Device> = std::sync::OnceLock::new();

/// The shell's window is full screen (`ToClient::FullScreen`), for Options.
static FULL_SCREEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    /// The engine thread's screen.
    static SCREEN: RefCell<Option<ClientScreen>> = const { RefCell::new(None) };
}

/// Runs turu.swf as a client of the shell at `address` until the shell goes away.
pub fn run(preferences: GlobalPreferences, movie_url: Url, address: &str) -> Result<(), Error> {
    // A crash shows no message box here: the shell starts the client again.
    #[cfg(windows)]
    crate::windows::set_client();
    let token = std::env::var(shell::TOKEN_ENV).with_context(|| format!("--client needs {}", shell::TOKEN_ENV))?;
    let (reader, writer) =
        shell::connect(address, &token).with_context(|| format!("Couldn't reach the shell at {address}"))?;
    let out = FrameSender::start(writer);
    let answers: Answers = Arc::default();
    let platform = ClientPlatform {
        out: out.clone(),
        answers: answers.clone(),
        next_id: AtomicU32::new(1),
    };
    if !turu_frontend::platform::set(Arc::new(platform)) {
        tracing::warn!("Another platform was installed first: the client's requests won't reach the shell");
    }

    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();
    let (descriptors, blit, renderer) = gpu(&preferences)?;
    // The shell checks it's on the same GPU before using shared surfaces.
    let info = descriptors.adapter.get_info();
    out.send(ToShell::Gpu {
        adapter: shell::adapter_key(info.backend.to_str(), info.vendor, info.device, &info.name),
    });
    let device = descriptors.device.clone();
    let _ = GPU_DEVICE.set(device.clone());
    let (sender, events) = channel();
    let _ = ENGINE.set(sender.clone());
    let tab = turu::active_tab();
    let mut fonts = fontdb::Database::default();
    fonts.load_system_fonts();
    let mut players = Players::new();
    // The window's device fonts, the clipboard and sound, for the game; and its notifications (a
    // text field wanting an input method), for the screen.
    let (notify, notifications) = async_channel::unbounded();
    let ui = Some((ClientUiBackend::new(Rc::new(fonts), preferences.clone()), notify));
    // Mugetsu's render thread (`render-thread` next to the program; see
    // `mugetsu::apply_drawing_switches`): the picture's GPU commands are made there while this
    // thread goes on (`ClientScreen` hands the shell its picture after them).
    let player = if ruffle_render_wgpu::render_thread::enabled() {
        let renderer = ruffle_render_wgpu::render_thread::ThreadedBackend::new(renderer);
        build_player(
            &preferences,
            movie_url,
            sender.clone(),
            tab,
            renderer,
            ui,
            StageQuality::High,
        )
    } else {
        build_player(
            &preferences,
            movie_url,
            sender.clone(),
            tab,
            renderer,
            ui,
            StageQuality::High,
        )
    };
    // Heard only while it's the tab on screen (`ClientScreen`), as the window's tabs are.
    player.lock().expect("the player was just built").set_volume(0.0);
    players.add(Slot::new(Some(tab), player, ()));
    turu::bot().set_host(Some(Arc::new(engine::flash_host(sender.clone(), tab))));
    // Hidden until the shell shows it.
    turu::bot().state.lock().hidden_tab = true;
    let dump: Arc<Mutex<Option<String>>> = Arc::default();
    let sink_dump = dump.clone();
    let sink_out = out.clone();
    let mut screen = ClientScreen::new(
        descriptors,
        blit,
        Box::new(move |picture| {
            match picture {
                Picture::Pixels { width, height, pixels } => {
                    if let Some(path) = sink_dump.lock().unwrap_or_else(|e| e.into_inner()).take() {
                        write_png(Path::new(&path), width, height, &pixels);
                    }
                    sink_out.frame(width, height, pixels);
                }
                Picture::Shared { id, width, height } => sink_out.send(ToShell::Surface { id, width, height }),
            }
            // Ends the Vulkan startup window 10 s after the first picture (`gpu`).
            crate::mugetsu::frame_presented();
        }),
    );
    // A shell that takes GPU surfaces names where to hand them; `Dump` needs pixels.
    #[cfg(target_os = "macos")]
    if let Ok(name) = std::env::var(shell::SURFACES_ENV)
        && let Some(sender) = crate::shared_surface::Sender::connect(&name)
    {
        tracing::info!("Pictures go to the shell as shared surfaces (IOSurface)");
        screen.share_surfaces(Box::new(sender));
    }
    #[cfg(any(windows, target_os = "linux"))]
    if let Ok(surfaces) = std::env::var(shell::SURFACES_ENV)
        && let Some(sender) = crate::shared_gpu::Sender::connect(&device, info.backend, &surfaces, &out)
    {
        tracing::info!(
            "Pictures go to the shell as shared surfaces ({})",
            sender.route().name()
        );
        screen.share_surfaces(Box::new(sender));
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    drop(device);
    screen.set_volume(if preferences.mute() {
        0.0
    } else {
        preferences.preferred_volume()
    });
    screen.set_clipboard(Box::new(SystemClipboard(egui_winit::clipboard::Clipboard::new(None))));
    let cursor_out = out.clone();
    screen.set_cursor_sink(Box::new(move |cursor| cursor_out.send(ToShell::Cursor { cursor })));
    // Frame interpolation's pace: the shell presents in step with it.
    let pace_out = out.clone();
    screen.set_pace_sink(Box::new(move |pace| pace_out.send(ToShell::Pace { pace })));
    // An input method over the shell's window, for Turu's text boxes and (with Ruffle's IME
    // preference, as the window has it) the game's.
    let ime_out = out.clone();
    screen.set_ime_sink(
        preferences.ime_enabled().unwrap_or(false),
        Box::new(move |allowed, password, area| {
            ime_out.send(ToShell::Ime {
                allowed,
                password,
                area,
            })
        }),
    );
    SCREEN.set(Some(screen));
    turu::start_updates();
    // Turu's updates are the shell's: it checks and installs; Check Now and Download & restart go
    // there, and its state comes back (`ToClient::Updates`).
    let update_out = out.clone();
    turu::set_update_requests(Box::new(move |request| {
        update_out.send(match request {
            turu::UpdateRequest::Check => ToShell::CheckUpdates,
            turu::UpdateRequest::Install { on_quit } => ToShell::InstallUpdate { on_quit },
        })
    }));
    // The control API's `/update` (also through the shell's `--api`): asks the shell, as Download &
    // restart does, after this tab's own check that no script runs.
    turu_bot::updates::set_installer(Box::new(|| turu::install_update(false)));
    // Army > Accounts' processes of their own are the shell's: launched there, they outlive this
    // client, and its web monitor lists them. Its list comes back (`ToClient::ArmyClients`).
    let army_out = out.clone();
    turu::set_shell_army(Box::new(move |request| {
        army_out.send(match request {
            turu::ShellArmyRequest::Launch(launches) => ToShell::Launch { launches },
            turu::ShellArmyRequest::Close(username) => ToShell::ArmyClient {
                username,
                restart: false,
            },
            turu::ShellArmyRequest::Restart(username) => ToShell::ArmyClient {
                username,
                restart: true,
            },
        })
    }));
    // The tab strip's tabs are the shell's (M5): requests go to it, its list comes back.
    let link_out = out.clone();
    let open_out = out.clone();
    turu_frontend::client_link::install(Arc::new(move |message| link_out.send(message)), move || {
        !open_out.is_closed()
    });
    let spawned = std::thread::Builder::new().name("Turu Shell In".into()).spawn({
        let sender = sender.clone();
        move || {
            let mut forward = Forward { sender, dump, answers };
            match shell::serve(reader, &mut forward) {
                Ok(()) => tracing::info!("The shell closed the connection; quitting"),
                Err(e) => tracing::warn!("Lost the shell ({e}); quitting"),
            }
            // AQW build (Mugetsu 2.0.0): what the engine thread's storage holds of the graphics
            // device is let go of there, while the driver is alive (thread storage is freed last
            // of all at exit, when Windows has already unloaded the driver: DirectX 12 crashed
            // there), the video memory query's DXGI adapter too, and wgpu's last command buffers
            // are dropped. 3 s at most. A crash being handled on another thread meanwhile gets
            // its report written before the end (`windows::wait_for_crash_report`).
            crate::mugetsu::set_exit_phase(1);
            let _ = forward.sender.send(EngineEvent::Run(Box::new(|player| {
                // The render thread's storage holds its own of the device: let go of there too,
                // once its last picture is done (at once without one).
                let renderer = player.renderer_mut();
                ruffle_render_wgpu::render_thread::settle::<MovieTexture>(renderer);
                ruffle_render_wgpu::render_thread::run_after_picture::<MovieTexture>(
                    renderer,
                    Box::new(|| {
                        ruffle_render_wgpu::segments::clear();
                    }),
                );
                ruffle_render_wgpu::render_thread::settle::<MovieTexture>(renderer);
                let shapes = ruffle_core::shutdown_deferred_shapes();
                let segments = ruffle_render_wgpu::segments::clear();
                crate::mugetsu::release_video_memory_adapter();
                wgpu_core::aqw_flush_encoder_drops();
                crate::mugetsu::set_exit_phase(3);
                tracing::info!("closing: let go of {shapes} shapes still being drawn and {segments} recorded segments");
                exit_now(0);
            })));
            std::thread::sleep(Duration::from_secs(3));
            exit_now(0);
        }
    });
    spawned.context("Couldn't start reading the shell's messages")?;
    tracing::info!("Client of the shell at {address}");
    drop(sender);
    engine::run_loop_paced(
        &mut players,
        &events,
        |player, step| {
            gpu_check();
            memory_tick(player);
            let next = SCREEN.with_borrow_mut(|screen| {
                let screen = screen.as_mut()?;
                while let Ok(ruffle_core::events::PlayerNotification::ImeNotification(ime)) = notifications.try_recv() {
                    screen.game_ime(player, ime);
                }
                screen.step(player, step)
            });
            // Mugetsu 2.0.0's one-time tips for built-in graphics (`turu_frontend::graphics_tips`).
            if let Some(drawn_fps) =
                SCREEN.with_borrow_mut(|screen| screen.as_mut().and_then(|screen| screen.take_picture_window()))
            {
                turu_frontend::graphics_tips::window(drawn_fps, player.frame_rate(), tip_quality(player));
            }
            perf_tick(player);
            next
        },
        |_, _| tracing::warn!("A client has one tab: the request is ignored"),
    );
    turu::bot().set_host(None);
    Ok(())
}

/// The GPU dropped the device (a driver reset, the GPU out of memory): it can't come back, and
/// the game's shapes and bitmaps lived on it. Said once (the log, the tab's debug log, Sentry),
/// then this client ends: the shell starts it again, logged in from its record, while the other
/// tabs play on (docs/GPU_RECOVERY_PROPOSAL.md: "restart that client"). `TURU_TEST_GPU_LOSS=1`
/// destroys the device 20 s after the start, to test it.
fn gpu_check() {
    thread_local! {
        static STARTED: std::time::Instant = std::time::Instant::now();
        static TESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if std::env::var_os("TURU_TEST_GPU_LOSS").is_some_and(|v| v == "1")
        && !TESTED.get()
        && STARTED.with(|started| started.elapsed() >= Duration::from_secs(20))
        && let Some(device) = GPU_DEVICE.get()
    {
        TESTED.set(true);
        tracing::warn!("TURU_TEST_GPU_LOSS: destroying the GPU device");
        device.destroy();
    }
    let Some(why) = ruffle_render_wgpu::device_lost() else {
        return;
    };
    tracing::error!("The GPU stopped working ({why}); this tab's client ends, and the shell starts it again");
    turu::bot()
        .logs
        .debug
        .push(format!("Graphics: the GPU stopped working ({why}); the tab starts again"));
    crate::crash_uploads::gpu_lost(&why);
    // (Sent before the process ends, 2 s at most.)
    if let Some(client) = sentry::Hub::main().client() {
        client.flush(Some(Duration::from_secs(2)));
    }
    exit_now(EXIT_GPU_LOST);
}

/// AQW's Quality, as the graphics tips take it.
fn tip_quality(player: &mut ruffle_core::Player) -> turu_frontend::graphics_tips::Quality {
    use ruffle_render::quality::StageQuality;
    use turu_frontend::graphics_tips::Quality;
    match player.quality() {
        StageQuality::Low => Quality::Low,
        StageQuality::Medium => Quality::Medium,
        _ => Quality::High,
    }
}

/// Ends the client with `code`, once a native crash being handled on another thread has its
/// report written (Windows).
fn exit_now(code: i32) -> ! {
    #[cfg(windows)]
    crate::windows::wait_for_crash_report();
    std::process::exit(code);
}

/// The client's exit code after losing the GPU device (`gpu_check`).
const EXIT_GPU_LOST: i32 = 86;

/// Every 5 s: the memory watch (`memory::memory_watch`: shapes not drawn for a while, the
/// graphics memory nearly full, the memory limit, for this client's one player), and once a
/// minute what mimalloc keeps free goes back to the system, as the window and `--headless` do.
fn memory_tick(player: &mut ruffle_core::Player) {
    thread_local! {
        static LAST: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
    }
    let now = std::time::Instant::now();
    if LAST.get().is_some_and(|last| now.duration_since(last) < Duration::from_secs(5)) {
        return;
    }
    LAST.set(Some(now));
    if let Some(trim) = crate::memory::memory_watch(player) {
        let freed = crate::memory::free_memory(&mut |f| f(player), false);
        tracing::info!(
            target: "ruffle_aqw_perf",
            "memory limit {} MB: {freed}{}; the next freeing in {} s at the soonest",
            trim.limit_mb,
            if trim.held { "" } else { "; the last freeing did not hold" },
            trim.next_wait_secs
        );
    }
}

/// With `TURU_PERF_LOG`: every period, `--headless`'s memory lines (the process's memory, the
/// heap, the SWFs loaded, uncaught AS3 errors) and the game's pictures drawn a second.
fn perf_tick(player: &mut ruffle_core::Player) {
    thread_local! {
        static LAST: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
    }
    let Some(every) = turu::perf_log_every() else {
        return;
    };
    let now = std::time::Instant::now();
    match LAST.get() {
        None => LAST.set(Some(now)),
        Some(last) if now.duration_since(last) >= every => {
            LAST.set(Some(now));
            crate::headless::log_memory(player);
            let shown = SCREEN.with_borrow_mut(|screen| {
                screen
                    .as_mut()
                    .map(|screen| (screen.shown(), screen.fps(), screen.take_skipped(), screen.pipeline_ms()))
            });
            if let Some((shown, fps, skipped, pipeline_ms)) = shown {
                // With frame interpolation: new pictures due on the shell's refreshes but skipped,
                // and a picture's way from its draw to finished (the pacer's floor).
                tracing::info!(
                    target: "turu_perf",
                    "pictures: {}{}{}",
                    if shown { format!("{fps:.1} a second") } else { "none (hidden)".to_string() },
                    if skipped > 0 { format!(", {skipped} due skipped") } else { String::new() },
                    if pipeline_ms > 0.0 { format!(", {pipeline_ms:.1} ms from draw to finished") } else { String::new() }
                );
            }
            // With the render thread: its last picture's time, and what waited for a picture it
            // was making (a wait on every picture would mean the two threads take turns).
            let renderer = player.renderer_mut();
            if let Some(micros) = ruffle_render_wgpu::render_thread::picture_micros::<MovieTexture>(renderer) {
                tracing::info!(
                    target: "turu_perf",
                    "render thread: last picture {:.1} ms; waited for its pictures: {}",
                    micros as f64 / 1000.0,
                    ruffle_render_wgpu::render_thread::waits_report::<MovieTexture>(renderer)
                        .unwrap_or_else(|| "nothing".into())
                );
            }
        }
        Some(_) => {}
    }
}

/// The system clipboard for Turu's text boxes (egui-winit's, without a window).
struct SystemClipboard(egui_winit::clipboard::Clipboard);

impl turu_frontend::client_screen::Clipboard for SystemClipboard {
    fn get(&mut self) -> Option<String> {
        self.0.get()
    }

    fn set(&mut self, text: String) {
        self.0.set_text(text);
    }
}

/// Hands the shell's messages to the engine thread.
struct Forward {
    sender: Sender<EngineEvent>,
    dump: Arc<Mutex<Option<String>>>,
    answers: Answers,
}

impl ClientEnd for Forward {
    fn handle(&mut self, message: ToClient) {
        match message {
            ToClient::Dump { path } => *self.dump.lock().unwrap_or_else(|e| e.into_inner()) = Some(path),
            ToClient::Tabs { tabs, shown, you } => {
                turu_frontend::client_link::set_tabs(tabs, shown, you);
                let _ = self.sender.send(EngineEvent::Run(Box::new(|_| {
                    SCREEN.with_borrow_mut(|screen| {
                        if let Some(screen) = screen {
                            screen.request_repaint();
                        }
                    })
                })));
            }
            ToClient::HeadlessMode { on } => turu_frontend::client_link::set_headless_mode(on),
            ToClient::FullScreen { on } => FULL_SCREEN.store(on, Ordering::Relaxed),
            ToClient::Updates { updates } => (*updates).apply(),
            ToClient::ArmyClients { clients } => turu_frontend::client_link::set_army_clients(clients),
            // A file dialog the shell showed.
            ToClient::Answer { id, value } => {
                let answer = self.answers.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                if let Some(answer) = answer {
                    answer(value.map(Into::into));
                }
            }
            message => {
                let _ = self.sender.send(EngineEvent::Run(Box::new(move |player| {
                    SCREEN.with_borrow_mut(|screen| {
                        if let Some(screen) = screen {
                            screen.handle(player, message);
                        }
                    })
                })));
            }
        }
    }
}

/// The GPU set up for drawing without a window: its descriptors, the blit and the game's renderer.
type Gpu = (Arc<Descriptors>, Arc<Blit>, WgpuRenderBackend<MovieTexture>);

/// The GPU without a window, chosen as the window and the shell choose theirs (Options'
/// backends in turn, then the others; Options' power), the blit, and the game's renderer at
/// turu.swf's stage.
fn gpu(preferences: &GlobalPreferences) -> Result<Gpu, Error> {
    let wanted: wgpu::Backends = preferences.graphics_backends().into();
    // A driver crash from here until shortly after the first picture counts as the graphics API
    // crashing at startup: the crash handler saves it, and the client the shell starts next takes
    // the other one (`mugetsu::automatic_graphics`), as the window would.
    if wanted.intersects(wgpu::Backends::VULKAN | wgpu::Backends::DX12) {
        // A hidden or covered tab draws no pictures (`ClientScreen`), which would leave that window
        // open as long as it stays hidden: it ends 30 s after this at the latest.
        let _ = std::thread::Builder::new()
            .name("graphics startup".to_string())
            .spawn(|| {
                std::thread::sleep(Duration::from_secs(20));
                crate::mugetsu::frame_presented();
                std::thread::sleep(Duration::from_secs(11));
                crate::mugetsu::frame_presented();
            });
    }
    // No window, so no display handle: OpenGL on Linux tries EGL without one, and the other
    // backends follow if it finds no GPU. DirectX 12 that can't be started falls back to Vulkan
    // first (`mugetsu::open_gpu`), as the shell's does, so both end up on the same backend.
    let power: wgpu::PowerPreference = preferences.graphics_power_preference().into();
    let mut tried = wgpu::Backends::empty();
    let opened = crate::mugetsu::open_gpu(wanted, |wanted| {
        let (instance, backend) = crate::gui::select_wgpu_backend(wanted, None)?;
        tried |= backend;
        crate::mugetsu::graphics_startup_begin(
            backend.contains(wgpu::Backends::VULKAN),
            backend.contains(wgpu::Backends::DX12),
        );
        #[cfg(windows)]
        crate::windows::test_native_crash();
        let (adapter, device, queue) =
            futures::executor::block_on(request_adapter_and_device(backend, &instance, None, power))
                .map_err(|e| anyhow!(e.to_string()))?;
        Ok((instance, adapter, device, queue))
    });
    let (instance, adapter, device, queue) = match opened {
        Ok(opened) => opened,
        // A backend can list a GPU and still give no device (EGL without a display): the others.
        Err(e) => {
            tracing::warn!(
                "Client: no GPU on {} ({e}); trying the other backends",
                backend_names(tried)
            );
            let others = wgpu::Backends::all() - tried;
            let instance = create_wgpu_instance(others, Default::default(), None);
            let (adapter, device, queue) =
                futures::executor::block_on(request_adapter_and_device(others, &instance, None, power))
                    .map_err(|e| anyhow!("No GPU for the client: {e}"))?;
            (instance, adapter, device, queue)
        }
    };
    let info = adapter.get_info();
    crate::mugetsu::set_adapter_ids(info.vendor, info.device);
    #[cfg(not(windows))]
    crate::mugetsu::set_video_memory_adapter(adapter.clone());
    crate::mugetsu::note_graphics_chip(&info);
    tracing::info!(
        "Client graphics: {} on {} (type: {:?})",
        info.backend.to_str(),
        info.name,
        info.device_type
    );
    // As the window (`gui/controller.rs`): which speed-ups are off, transient textures, and the
    // size detailed shapes are tessellated for at least (8 px on a dedicated card, else 32).
    let off = crate::mugetsu::drawing_switches_off();
    if !off.is_empty() {
        tracing::info!("Drawing speed-ups switched off: {off:?}");
    }
    ruffle_render_wgpu::set_transient_textures_enabled(!crate::mugetsu::exe_flag("no-transient-memory"));
    let shape_min_px = ruffle_core::set_shape_min_px(if info.device_type == wgpu::DeviceType::DiscreteGpu {
        8.0
    } else {
        32.0
    });
    tracing::info!("Detailed shapes are tessellated for at least {shape_min_px} px");
    let descriptors = Arc::new(Descriptors::new(instance, adapter, device, queue));
    let blit = Arc::new(Blit::new(&descriptors.device));
    let target = MovieTexture::new(blit.clone(), &descriptors.device, STAGE.0, STAGE.1);
    let renderer = WgpuRenderBackend::new(descriptors.clone(), target)
        .map_err(|e| anyhow!("Couldn't make the game's renderer: {e}"))?;
    Ok((descriptors, blit, renderer))
}

/// The backends' names, for the log ("Vulkan and DirectX 12").
fn backend_names(backends: wgpu::Backends) -> String {
    format_list(&get_backend_names(backends), "and").into_owned()
}

/// File dialogs waiting for the shell's answer, by id.
type Answers = Arc<Mutex<HashMap<u32, Answer>>>;

/// What the client asks of the shell: file dialogs over its window (answered by id) and links.
struct ClientPlatform {
    out: FrameSender,
    answers: Answers,
    next_id: AtomicU32,
}

impl ClientPlatform {
    fn dialog(&self, dialog: FileDialog, save: bool, answer: Answer) -> bool {
        if self.out.is_closed() {
            return false;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.answers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, answer);
        self.out.send(ToShell::FileDialog {
            id,
            save,
            title: dialog.title,
            directory: dialog.directory.map(|d| d.to_string_lossy().into_owned()),
            file_name: dialog.file_name,
            filters: dialog.filters,
        });
        true
    }
}

impl Platform for ClientPlatform {
    // The window has the game's right-click menu for this; a client has none.
    fn graphics_choices(&self) -> Option<(Vec<(&'static str, &'static str)>, &'static str)> {
        use crate::mugetsu::Graphics;
        if !(cfg!(windows) || cfg!(target_os = "linux")) || !crate::mugetsu::plays_aqw() {
            return None;
        }
        let mut choices = vec![("", "Automatic")];
        choices.extend(Graphics::ALL.iter().map(|graphics| (graphics.arg(), graphics.name())));
        Some((choices, crate::mugetsu::saved_graphics().map_or("", Graphics::arg)))
    }

    fn set_graphics(&self, value: &str) {
        self.out.send(ToShell::ChooseGraphics {
            graphics: value.to_owned(),
        });
    }

    // Options' Frame Interpolation: applied here and saved, as the window's right-click menu does;
    // the other tabs take it at their next start.
    fn set_frame_interpolation(&self, on: bool) {
        ruffle_core::frame_interpolation::set_enabled(on);
        crate::mugetsu::save_interpolation(on);
        tracing::info!("Frame interpolation turned {}", if on { "on" } else { "off" });
    }

    // The rest of the window's right-click menu (Mugetsu's): a client has none.
    fn show_fps(&self) -> Option<bool> {
        Some(crate::mugetsu::show_fps())
    }

    // Mugetsu 2.0.0's options (the window had them in its right-click menu).
    fn performance(&self) -> Option<u8> {
        Some(crate::mugetsu::performance() as u8)
    }

    fn set_performance(&self, level: u8) {
        let level = crate::mugetsu::Performance::from_index(level);
        crate::mugetsu::set_performance(level);
        tracing::info!(
            "Performance mode: {level:?} (the game is drawn at {:.0} % of its width and height)",
            level.scale() * 100.0
        );
    }

    fn render_scale(&self) -> f64 {
        crate::mugetsu::render_scale()
    }

    fn memory_limit_mb(&self) -> Option<u32> {
        Some(crate::mugetsu::memory_limit_mb())
    }

    fn set_memory_limit_mb(&self, mb: u32) {
        crate::mugetsu::set_memory_limit_mb(mb);
        tracing::info!("Memory limit: {}", if mb == 0 { "off".to_string() } else { format!("{mb} MB") });
    }

    fn memory_now_mb(&self) -> Option<u64> {
        Some(crate::mugetsu::memory_now_mb())
    }

    fn free_memory(&self) {
        // On the engine thread, which has the player (Options runs there too, inside a draw).
        if let Some(engine) = ENGINE.get() {
            let _ = engine.send(EngineEvent::Run(Box::new(|player| {
                let freed = crate::memory::free_memory(&mut |f| f(player), true);
                tracing::info!(target: "ruffle_aqw_perf", "free memory now: {freed}");
            })));
        }
    }

    fn set_show_fps(&self, on: bool) {
        crate::mugetsu::set_show_fps(on);
    }

    // Mugetsu 2.0.0's frame pacing and FPS limit (the window's right-click menu has them): a tab
    // asks the shell to present its pictures uncapped (`ClientScreen`). Applied from the next
    // game frame; the other tabs take it at their next start, as Frame Interpolation.
    fn frame_pacing(&self) -> Option<(bool, u32)> {
        use crate::mugetsu::FpsLimit;
        let limit = match crate::mugetsu::fps_limit() {
            FpsLimit::Off => 0,
            FpsLimit::Screen => 1,
            FpsLimit::Fixed(fps) => fps,
        };
        Some((crate::mugetsu::uncapped(), limit))
    }

    fn set_uncapped(&self, on: bool) {
        crate::mugetsu::set_uncapped(on);
        tracing::info!("Frame pacing set to {}", if on { "uncapped" } else { "v-sync" });
    }

    fn set_fps_limit(&self, limit: u32) {
        use crate::mugetsu::FpsLimit;
        let limit = match limit {
            0 => FpsLimit::Off,
            1 => FpsLimit::Screen,
            fps => FpsLimit::Fixed(fps),
        };
        crate::mugetsu::set_fps_limit(limit);
        tracing::info!("FPS limit: {limit:?}");
    }

    fn full_screen(&self) -> Option<bool> {
        Some(FULL_SCREEN.load(Ordering::Relaxed))
    }

    fn set_full_screen(&self, on: bool) {
        self.out.send(ToShell::FullScreen { on });
    }

    fn opens_log_folder(&self) -> bool {
        true
    }

    fn open_log_folder(&self) {
        crate::mugetsu::open_log_folder();
    }

    fn pick_file(&self, dialog: FileDialog, answer: Answer) -> bool {
        self.dialog(dialog, false, answer)
    }

    fn save_file(&self, dialog: FileDialog, answer: Answer) -> bool {
        self.dialog(dialog, true, answer)
    }

    fn open_url(&self, url: &str) {
        self.out.send(ToShell::OpenUrl { url: url.to_owned() });
    }

    fn open_folder(&self, folder: &Path) {
        crate::mugetsu::open_folder(folder);
    }

    fn open_file(&self, path: &Path) {
        crate::mugetsu::open_folder(path);
    }
}

/// Writes BGRA `pixels` to `path` as a PNG.
fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) {
    let mut rgba = pixels.to_vec();
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    match image::RgbaImage::from_raw(width, height, rgba).map(|image| image.save(path)) {
        Some(Ok(())) => tracing::info!("Picture written to {}", path.display()),
        Some(Err(e)) => tracing::warn!("Couldn't write {}: {e}", path.display()),
        None => tracing::warn!("A picture of the wrong size ({width}x{height})"),
    }
}
