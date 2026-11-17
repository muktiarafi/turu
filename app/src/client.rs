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
    let (sender, events) = channel();
    let _ = ENGINE.set(sender.clone());
    let tab = turu::active_tab();
    let mut fonts = fontdb::Database::default();
    fonts.load_system_fonts();
    let mut players = Players::new();
    let player = build_player(
        &preferences,
        movie_url,
        sender.clone(),
        tab,
        renderer,
        // The window's device fonts, the clipboard and sound, for the game.
        Some(ClientUiBackend::new(Rc::new(fonts), preferences.clone())),
        StageQuality::High,
    );
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
            std::process::exit(0);
        }
    });
    spawned.context("Couldn't start reading the shell's messages")?;
    tracing::info!("Client of the shell at {address}");
    drop(sender);
    engine::run_loop_paced(
        &mut players,
        &events,
        |player, step| SCREEN.with_borrow_mut(|screen| screen.as_mut().and_then(|screen| screen.step(player, step))),
        |_, _| tracing::warn!("A client has one tab: the request is ignored"),
    );
    turu::bot().set_host(None);
    Ok(())
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
    // A driver crash from here until shortly after the first picture counts as the Vulkan driver
    // crashing at startup: the crash handler saves it, and the client the shell starts next takes
    // DirectX 12 (`main.rs`'s `mugetsu_args`), as the window would.
    crate::mugetsu::vulkan_startup_begin(wanted.contains(wgpu::Backends::VULKAN));
    if wanted.contains(wgpu::Backends::VULKAN) {
        // A hidden or covered tab draws no pictures (`ClientScreen`), which would leave that window
        // open as long as it stays hidden: it ends 30 s after this at the latest.
        let _ = std::thread::Builder::new()
            .name("vulkan startup".to_string())
            .spawn(|| {
                std::thread::sleep(Duration::from_secs(20));
                crate::mugetsu::frame_presented();
                std::thread::sleep(Duration::from_secs(11));
                crate::mugetsu::frame_presented();
            });
    }
    // No window, so no display handle: OpenGL on Linux tries EGL without one, and the other
    // backends follow if it finds no GPU.
    let (instance, backend) = crate::gui::select_wgpu_backend(wanted, None)?;
    #[cfg(windows)]
    crate::windows::test_native_crash();
    let power: wgpu::PowerPreference = preferences.graphics_power_preference().into();
    let (instance, adapter, device, queue) =
        match futures::executor::block_on(request_adapter_and_device(backend, &instance, None, power)) {
            Ok((adapter, device, queue)) => (instance, adapter, device, queue),
            // A backend can list a GPU and still give no device (EGL without a display): the others.
            Err(e) => {
                tracing::warn!(
                    "Client: no GPU on {} ({e}); trying the other backends",
                    backend_names(backend)
                );
                let others = wgpu::Backends::all() - backend;
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
    tracing::info!(
        "Client graphics: {} on {} (type: {:?})",
        info.backend.to_str(),
        info.name,
        info.device_type
    );
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
    // Options lives in the clients: turned off here, the next start is the window again.
    fn process_tabs(&self) -> Option<bool> {
        Some(crate::shell::switched_on())
    }

    fn set_process_tabs(&self, on: bool) {
        crate::shell::save_switch(on);
    }

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

    fn set_show_fps(&self, on: bool) {
        crate::mugetsu::set_show_fps(on);
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
