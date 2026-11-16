//! Turu: the client mode (`--client=<address>`, docs/TAB_PROCESSES_PROPOSAL.md, M2). One tab's
//! game and bot without a window, as `--headless` runs them, but drawn: the player renders on a
//! GPU set up without a window, and `turu_frontend::client_screen` draws it with Turu's UI into
//! pictures for the shell (`shell.rs`), which shows them and sends back input. The process quits
//! when the shell closes the connection.
//!
//! The shell's messages are read on a thread of their own and run on the engine thread
//! (`EngineEvent::Run`); pictures go out on another (`FrameSender`), newest first, so the engine
//! never waits on the shell. `Dump` (debugging) writes the next picture to a PNG.

use crate::headless::build_player;
use crate::preferences::GlobalPreferences;
use crate::turu;
use anyhow::{Context as _, Error, anyhow};
use ruffle_render::quality::StageQuality;
use ruffle_render_wgpu::backend::{WgpuRenderBackend, create_wgpu_instance, request_adapter_and_device};
use ruffle_render_wgpu::descriptors::Descriptors;
use std::cell::RefCell;
use std::path::Path;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use turu_frontend::client_screen::ClientScreen;
use turu_frontend::engine::{self, EngineEvent, STAGE};
use turu_frontend::movie_texture::{Blit, MovieTexture};
use turu_frontend::platform::{Answer, FileDialog, Platform};
use turu_frontend::players::{Players, Slot};
use turu_frontend::shell::{self, ClientEnd, FrameSender, ToClient, ToShell};
use url::Url;

thread_local! {
    /// The engine thread's screen.
    static SCREEN: RefCell<Option<ClientScreen>> = const { RefCell::new(None) };
}

/// Runs turu.swf as a client of the shell at `address` until the shell goes away.
pub fn run(preferences: GlobalPreferences, movie_url: Url, address: &str) -> Result<(), Error> {
    let token = std::env::var(shell::TOKEN_ENV).with_context(|| format!("--client needs {}", shell::TOKEN_ENV))?;
    let (reader, writer) =
        shell::connect(address, &token).with_context(|| format!("Couldn't reach the shell at {address}"))?;
    let out = FrameSender::start(writer);
    if !turu_frontend::platform::set(Arc::new(ClientPlatform { out: out.clone() })) {
        tracing::warn!("Another platform was installed first: the client's requests won't reach the shell");
    }

    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();
    let (descriptors, blit, renderer) = gpu(&preferences)?;
    let (sender, events) = channel();
    let tab = turu::active_tab();
    let mut players = Players::new();
    let player = build_player(
        &preferences,
        movie_url,
        sender.clone(),
        tab,
        renderer,
        StageQuality::High,
    );
    players.add(Slot::new(Some(tab), player, ()));
    turu::bot().set_host(Some(Arc::new(engine::flash_host(sender.clone(), tab))));
    // Hidden until the shell shows it.
    turu::bot().state.lock().hidden_tab = true;
    let dump: Arc<Mutex<Option<String>>> = Arc::default();
    let sink_dump = dump.clone();
    let sink_out = out.clone();
    SCREEN.set(Some(ClientScreen::new(
        descriptors,
        blit,
        Box::new(move |width, height, pixels| {
            if let Some(path) = sink_dump.lock().unwrap_or_else(|e| e.into_inner()).take() {
                write_png(Path::new(&path), width, height, &pixels);
            }
            sink_out.frame(width, height, pixels);
        }),
    )));
    turu::start_updates();
    // The tab strip's tabs are the shell's: its "+" and Army > Accounts' "In a tab" ask it for a
    // new client; selecting and closing go to it too, and the bar's Grid toggle (M5).
    let tab_out = out.clone();
    turu::set_tab_requests(Box::new(move |request| {
        match request {
            turu::TabRequest::Open(startup) => tab_out.send(ToShell::OpenTab {
                username: startup.username,
                password: startup.password,
                server: startup.server,
                script: startup.script.map(|path| path.to_string_lossy().into_owned()),
                run_script: startup.run_script,
            }),
            // The shell lays the tiles out in what Turu's UI leaves of the window.
            turu::TabRequest::Grid(on) => tab_out.send(ToShell::Grid {
                on,
                left: turu_frontend::gui::left_width() as f32,
                top: turu_frontend::gui::top_height() as f32,
            }),
            _ => {}
        }
        true
    }));
    let action_out = out.clone();
    turu::set_shell_actions(Box::new(move |action| {
        action_out.send(match action {
            turu::ShellTabAction::Select(id) => ToShell::SelectTab { id },
            turu::ShellTabAction::Close(id) => ToShell::CloseTab { id },
        });
    }));
    report_status(out.clone());
    let spawned = std::thread::Builder::new().name("Turu Shell In".into()).spawn({
        let sender = sender.clone();
        move || {
            let mut forward = Forward { sender, dump };
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

/// Tells the shell the tab's name and whether its script or Auto runs, when either changes (checked
/// every second).
fn report_status(out: FrameSender) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Status".into())
        .spawn(move || {
            let mut sent = None;
            while !out.is_closed() {
                let bot = turu::bot();
                let now = (turu::tab_label(&bot), turu_frontend::gui::turu::busy(&bot));
                if sent.as_ref() != Some(&now) {
                    out.send(ToShell::Status {
                        label: now.0.clone(),
                        busy: now.1,
                    });
                    sent = Some(now);
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Couldn't start reporting the tab's name to the shell: {e}");
    }
}

/// Hands the shell's messages to the engine thread.
struct Forward {
    sender: Sender<EngineEvent>,
    dump: Arc<Mutex<Option<String>>>,
}

impl ClientEnd for Forward {
    fn handle(&mut self, message: ToClient) {
        match message {
            ToClient::Dump { path } => *self.dump.lock().unwrap_or_else(|e| e.into_inner()) = Some(path),
            ToClient::Tabs { tabs, shown, you } => {
                let tabs = tabs
                    .into_iter()
                    .map(|tab| turu::ShellTab {
                        me: you == Some(tab.id),
                        id: tab.id,
                        label: tab.label,
                        busy: tab.busy,
                        down: tab.down,
                        api: tab.api.and_then(|api| api.parse().ok()),
                        token: tab.token.unwrap_or_default(),
                    })
                    .collect();
                turu::set_shell_tabs(tabs, shown);
                let _ = self.sender.send(EngineEvent::Run(Box::new(|_| {
                    SCREEN.with_borrow_mut(|screen| {
                        if let Some(screen) = screen {
                            screen.request_repaint();
                        }
                    })
                })));
            }
            // Defined for file dialogs; nothing uses it yet.
            ToClient::Answer { .. } => {}
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

/// The GPU without a window (the backends the preferences allow), the blit, and the game's
/// renderer at turu.swf's stage.
fn gpu(preferences: &GlobalPreferences) -> Result<Gpu, Error> {
    let backends: wgpu::Backends = preferences.graphics_backends().into();
    let instance = create_wgpu_instance(backends, Default::default(), None);
    let (adapter, device, queue) = futures::executor::block_on(request_adapter_and_device(
        backends,
        &instance,
        None,
        wgpu::PowerPreference::HighPerformance,
    ))
    .map_err(|e| anyhow!("No GPU for the client: {e}"))?;
    let info = adapter.get_info();
    tracing::info!("Client graphics: {} on {}", info.backend.to_str(), info.name);
    let descriptors = Arc::new(Descriptors::new(instance, adapter, device, queue));
    let blit = Arc::new(Blit::new(&descriptors.device));
    let target = MovieTexture::new(blit.clone(), &descriptors.device, STAGE.0, STAGE.1);
    let renderer = WgpuRenderBackend::new(descriptors.clone(), target)
        .map_err(|e| anyhow!("Couldn't make the game's renderer: {e}"))?;
    Ok((descriptors, blit, renderer))
}

/// What the client asks of the shell. File dialogs and the rest wait for the shell to answer them
/// (defined in the messages, not used yet).
struct ClientPlatform {
    out: FrameSender,
}

impl Platform for ClientPlatform {
    fn pick_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        false
    }

    fn save_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        false
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

    fn has_file_dialogs(&self) -> bool {
        false
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
