//! Turu: the shell mode (`--shell`, docs/TAB_PROCESSES_PROPOSAL.md, M2 and M5). The window runs no
//! game: each tab is a client process (`turu --client=<address>`, `client.rs`) that runs one game
//! and bot and draws the whole tab, Turu's UI included. The shell shows the pictures of the tab on
//! screen, sends it the window's mouse, keys and size, and tells every client the list of tabs,
//! which their tab strips show; a strip's click, close or "+" (Army > Accounts' "In a tab" too)
//! comes back here.
//!
//! The first tab gets the shell's own startup flags (`--user`, `--run-script`, ...); a new one
//! those of its login. A client that dies is started again with the same flags after 1, 4, 16 and
//! then 60 s; after 5 deaths in 10 minutes its tab stays closed until selected, unless no other tab
//! is running (then it goes on trying every 60 s). When the tab on screen dies, a running one is
//! shown instead. A client quits when the shell does (its connection and stdin close).
//!
//! Pictures: a client draws into GPU surfaces it hands to the shell (IOSurfaces on macOS,
//! `shared_surface.rs`; Vulkan memory or D3D12 resources on Linux and Windows, `shared_gpu.rs`),
//! and says which one has its newest picture (`ToShell::Surface`); the shell draws from it on its
//! own GPU, chosen as the clients choose theirs, and gives each back (`ToClient::Release`) once a
//! newer one came and its own GPU work with the older one is done. A client on another GPU
//! (`ToShell::Gpu`), a surface the shell can't open, or `TURU_NO_SHARED_SURFACES`: the client
//! sends its pixels (`ToClient::NoSurfaces`), uploaded into a texture here. Either way the window is
//! drawn on the GPU, Grid View's tiles included.
//!
//! Testing without anyone at the window: `TURU_SHELL_TEST=<file>` runs its lines in order (and,
//! with `TURU_SHELL_TEST_FOLLOW=1`, lines appended to it later):
//! `wait <seconds>`, `open` (a tab at the login screen), `select <id>`, `close <id>`, `kill <id>`
//! (its client, as a crash would), `click <x> <y>` (in the window's pixels, as the mouse would),
//! `text <string>` (typed), `key <key>` (a press and release, egui's key name after `cmd+`, `ctrl+`, `shift+`, `alt+`),
//! `resize <width> <height>` (the window's size in points), `minimize on|off`, `cover off|on` (the
//! window counts as seen though covered, or not), `nosurfaces` (the
//! next shared surface fails to open), `rate` (each tab's pictures a second
//! since the last `rate`)
//! and `dump <path>` (the window's picture as drawn, read back from the GPU, as a PNG).
//!
//! Grid View (the bar's Grid toggle, `ToShell::Grid`): the tab on screen draws the window with its
//! game in its tile, the others only their game at their tile's size; the shell puts the tiles
//! under the tab on screen's picture. A click on another tile goes to the tab on screen, which
//! asks for that tab (`SelectTab`) unless one of its windows covers the tile there.

use crate::preferences::GlobalPreferences;
use crate::turu;
use anyhow::{Context as _, Error, anyhow};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::monitor::hub::PeerSource;
use turu_frontend::engine::STAGE;
use turu_frontend::movie_texture::Blit;
use turu_frontend::shell::{
    self, Button, GridShow, GridTile, Mods, PointerKind, SHARED_POOL, TabInfo, ToClient, ToShell,
};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

/// Waits before starting a dead client again, by how many times it died lately.
const RESTART_DELAYS: [Duration; 4] = [
    Duration::from_secs(1),
    Duration::from_secs(4),
    Duration::from_secs(16),
    Duration::from_secs(60),
];
/// Deaths within [`DEATH_WINDOW`] after which a tab stays closed.
const MAX_DEATHS: usize = 5;
const DEATH_WINDOW: Duration = Duration::from_secs(600);
/// How long a client has to connect after it starts.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
/// How often the GPU is asked whether the work before a surface's release is done.
const RELEASE_POLL: Duration = Duration::from_millis(4);

/// A client's surface as the shell got it: an IOSurface (macOS), Vulkan memory or a D3D12
/// resource (Windows, Linux: `shared_gpu.rs`); elsewhere there are none.
#[cfg(target_os = "macos")]
type SharedSurface = crate::shared_surface::Surface;
#[cfg(any(windows, target_os = "linux"))]
type SharedSurface = crate::shared_gpu::Surface;
#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
enum SharedSurface {}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
impl SharedSurface {
    fn size(&self) -> (u32, u32) {
        match *self {}
    }

    fn texture(&mut self, _device: &wgpu::Device) -> Option<wgpu::Texture> {
        match *self {}
    }
}

/// A client's surface, kept by the shell: opened on its GPU once it's drawn.
struct Kept {
    id: u64,
    surface: SharedSurface,
    opened: Option<(wgpu::Texture, wgpu::BindGroup)>,
}

/// What the shell draws of a tab: its newest picture as a texture of the shell's GPU.
struct Shown {
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
    /// The client's shared surface it is, given back when a newer one comes.
    surface: Option<u64>,
}

/// The newest picture from a client.
#[derive(Clone, Default)]
struct Picture {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

enum ShellEvent {
    /// A client said hello: its process and connection.
    Connected { pid: u32, stream: TcpStream },
    /// A tab's client sent a picture.
    Picture(u32),
    /// A tab's client asked for something.
    Message(u32, ToShell),
    /// A tab's client went away (its connection `generation`, to tell an old one's end).
    Gone(u32, u64),
    /// A line of `TURU_SHELL_TEST`'s file.
    Test(String),
    /// A client (its process id) handed over a shared surface.
    SurfaceArrived { pid: u32, id: u64, surface: SharedSurface },
    /// Turu's update state changed (a check, an install's progress): to every tab.
    Updates(turu_bot::updates::State),
    /// The shell's Army clients changed (launched, ended, closed): to every tab.
    ArmyClients(Vec<shell::ArmyClientInfo>),
    /// A file dialog a tab's client asked for was answered (tab, process id, its id, the path).
    Answered(u32, u32, u32, Option<String>),
    /// The shell's GPU is done with these surfaces (tab, process id, surface): back to their clients.
    Released(Vec<(u32, u32, u64)>),
    /// The answer to a tab's graphics choice (`choose_graphics`): the choice ("" for Automatic),
    /// or None when cancelled.
    #[cfg(any(windows, target_os = "linux"))]
    GraphicsChosen(Option<String>),
}

/// A tab: its client process, and how to start it again.
struct Tab {
    id: u32,
    /// The client's flags (besides `--client` and `--exit-with-parent`).
    args: Vec<String>,
    /// Given in `TURU_PASSWORD`.
    password: Option<String>,
    child: Option<Child>,
    out: Option<TcpStream>,
    generation: u64,
    picture: Arc<Mutex<Picture>>,
    label: String,
    busy: bool,
    deaths: Vec<Instant>,
    /// When to start the client again.
    restart_at: Option<Instant>,
    /// Pictures received since `TURU_SHELL_TEST`'s last `rate`.
    pictures: u32,
    /// Gave up on restarting (until the tab is selected).
    closed: bool,
    started: Instant,
    /// Its client's control API (a new port each start) and token: the shell's monitor and the
    /// other tabs reach it there.
    api: Option<std::net::SocketAddr>,
    api_token: String,
    /// Its newest picture, on the shell's GPU.
    shown: Option<Shown>,
    /// Where pixel pictures are uploaded.
    upload: Option<(wgpu::Texture, wgpu::BindGroup)>,
    /// The newest picture is in this surface, whose port hasn't come yet.
    waiting: Option<u64>,
    /// Its client's GPU (`ToShell::Gpu`), for shared surfaces.
    adapter: Option<String>,
    /// Its client was told to send pixels (`ToClient::NoSurfaces`), until it's started again.
    pixels_only: bool,
}

impl Tab {
    fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    fn down(&self) -> Option<String> {
        if self.closed {
            Some("closed".into())
        } else if self.out.is_none() {
            Some(
                if self.deaths.is_empty() {
                    "starting"
                } else {
                    "restarting"
                }
                .into(),
            )
        } else {
            None
        }
    }
}

/// Whether this start runs as the shell: every start of Turu's own window (AQW, not a movie
/// given), Army > Accounts' "In its own window" clients included, but not `--headless` or a
/// shell's client. Each tab is a process of its own (docs/TAB_PROCESSES_PROPOSAL.md, M6).
pub fn runs_as_shell() -> bool {
    crate::mugetsu::plays_aqw() && !turu::headless() && turu::client_of().is_none()
}

/// The flags a tab's client gets from the shell's: all but `--shell`, and the control API, web
/// monitor and Report To ones (the shell runs those; each client gets a control API of its own).
pub fn client_args(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut kept = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let name = arg.split_once('=').map_or(arg.as_str(), |(name, _)| name);
        match name {
            "--shell" | "--api" | "--monitor" => {}
            "--report-to" => {
                if !arg.contains('=') {
                    args.next();
                }
            }
            _ => kept.push(arg),
        }
    }
    kept
}

/// This process is the shell (`run` was called).
static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether this process is the shell, for what the window says otherwise (Windows' title-bar
/// menu: "right-click the game", which a client lacks).
#[cfg(windows)]
pub fn running() -> bool {
    RUNNING.load(std::sync::atomic::Ordering::Relaxed)
}

/// Starts the first tab's client and runs the window until it's closed.
pub fn run(preferences: GlobalPreferences, client_args: Vec<String>) -> Result<(), Error> {
    RUNNING.store(true, std::sync::atomic::Ordering::Relaxed);
    let listener = TcpListener::bind("127.0.0.1:0").context("Couldn't listen for the clients")?;
    let address = listener.local_addr()?.to_string();
    let token = turu_bot::control_api::new_token().map_err(|e| anyhow!(e))?;
    let event_loop = EventLoop::<ShellEvent>::with_user_event().build()?;
    spawn_acceptor(listener, token.clone(), event_loop.create_proxy());
    if let Ok(file) = std::env::var("TURU_SHELL_TEST") {
        spawn_test(file, event_loop.create_proxy());
    }
    // Options > Application lives in the clients, which leave the web monitor and Report To here.
    turu::follow_settings_file(true);
    // The clients leave the launch updates (scripts, skill sets) to the shell, and Turu's own
    // update checks (`main.rs`); what the checks find goes to them to show.
    turu::start_app_updates();
    watch_updates(event_loop.create_proxy());
    // The tabs' processes of their own (Army > Accounts) are launched here: the list to them.
    watch_army(event_loop.create_proxy());
    // Clients hand their pictures over as GPU surfaces where the shell can take them, unless
    // `TURU_NO_SHARED_SURFACES` says pixels.
    let surfaces_proxy = event_loop.create_proxy();
    let surfaces_name = if std::env::var_os("TURU_NO_SHARED_SURFACES").is_some() {
        tracing::info!("Shell: TURU_NO_SHARED_SURFACES: the clients send pixels");
        None
    } else {
        listen_for_surfaces(surfaces_proxy)
    };
    let peers: Arc<Mutex<Vec<PeerSource>>> = Arc::default();
    let listed = peers.clone();
    turu::monitor_hub().set_peers(Arc::new(move || {
        listed.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }));
    // `--api`: the same routes over the tabs, passed on to each tab's client's control API.
    let listed = peers.clone();
    turu::start_shell_api(Arc::new(move || {
        listed.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }));
    let mut app = ShellApp {
        peers,
        address,
        token,
        proxy: event_loop.create_proxy(),
        tabs: Vec::new(),
        shown: 0,
        next_id: 1,
        gpu: None,
        window: None,
        modifiers: ModifiersState::empty(),
        cursor: PhysicalPosition::new(0.0, 0.0),
        size: None,
        title: String::new(),
        grid: None,
        headless_mode: None,
        full_screen: false,
        surfaces_name,
        surfaces: HashMap::new(),
        retiring: Vec::new(),
        releases_pending: 0,
        dialog_open: false,
        #[cfg(any(windows, target_os = "linux"))]
        start_again: false,
        rate_since: Instant::now(),
        covered: false,
        occluded: false,
        minimized: false,
        preferences,
        adapter: String::new(),
        fail_surfaces: false,
        never_covered: false,
    };
    // The first tab logs in as `--headless` would: with the shell's own `TURU_PASSWORD`.
    app.open_tab(client_args, std::env::var("TURU_PASSWORD").ok());
    event_loop.run_app(&mut app)?;
    for tab in &mut app.tabs {
        stop(tab);
    }
    // A tab's graphics choice: the shell starts again with it, and its tabs with it.
    #[cfg(any(windows, target_os = "linux"))]
    if app.start_again {
        turu::close_butlers();
        #[cfg(windows)]
        crate::windows::start_again();
        #[cfg(target_os = "linux")]
        crate::linux::start_again();
    }
    // The Unix socket for shared surfaces (Linux): removed, as nothing else would.
    #[cfg(target_os = "linux")]
    if let Some(path) = &app.surfaces_name {
        let _ = std::fs::remove_file(path);
    }
    // Update when I quit, asked from a tab: put in place now.
    turu::apply_pending_update();
    Ok(())
}

/// The shell's Army clients, as the tabs get them.
fn army_clients() -> Vec<shell::ArmyClientInfo> {
    turu::army()
        .clients()
        .iter()
        .map(|client| shell::ArmyClientInfo {
            username: client.username.clone(),
            label: client.label.clone(),
            headless: client.headless,
            api: client.api().to_string(),
            token: client.token().to_owned(),
            exited: client.state().exited,
        })
        .collect()
}

/// Sends the shell's Army clients to the event loop whenever they change (checked every second).
fn watch_army(proxy: EventLoopProxy<ShellEvent>) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Army".into())
        .spawn(move || {
            let mut seen = Vec::new();
            loop {
                let now = army_clients();
                if now != seen {
                    seen = now.clone();
                    if proxy.send_event(ShellEvent::ArmyClients(now)).is_err() {
                        return;
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Shell: couldn't watch the Army's clients: {e}");
    }
}

/// Sends Turu's update state to the event loop whenever it changes (checked every second).
fn watch_updates(proxy: EventLoopProxy<ShellEvent>) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Updates".into())
        .spawn(move || {
            let mut seen = turu_bot::updates::State::default();
            loop {
                let now = turu_bot::updates::state();
                if now != seen {
                    seen = now.clone();
                    if proxy.send_event(ShellEvent::Updates(now)).is_err() {
                        return;
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Shell: couldn't watch for updates: {e}");
    }
}

/// Where the clients hand over their surfaces, set up by platform: a Mach service (macOS), a Unix
/// socket (Linux), the clients' own messages (Windows: "handles"). `None`: they send pixels.
fn listen_for_surfaces(proxy: EventLoopProxy<ShellEvent>) -> Option<String> {
    #[cfg(target_os = "macos")]
    return crate::shared_surface::listen(move |pid, id, surface| {
        let _ = proxy.send_event(ShellEvent::SurfaceArrived { pid, id, surface });
    });
    #[cfg(target_os = "linux")]
    return crate::shared_gpu::listen(move |pid, id, surface| {
        let _ = proxy.send_event(ShellEvent::SurfaceArrived { pid, id, surface });
    });
    #[cfg(windows)]
    {
        drop(proxy);
        return Some("handles".to_owned());
    }
    #[allow(unreachable_code)]
    {
        drop(proxy);
        None
    }
}

/// Feeds `TURU_SHELL_TEST`'s lines to the event loop, waiting where it says. With
/// `TURU_SHELL_TEST_FOLLOW` set, lines added to the file later are run too (driving a running shell
/// step by step).
fn spawn_test(file: String, proxy: EventLoopProxy<ShellEvent>) {
    let follow = std::env::var_os("TURU_SHELL_TEST_FOLLOW").is_some();
    let _ = std::thread::Builder::new()
        .name("Turu Shell Test".into())
        .spawn(move || {
            let mut done = 0;
            loop {
                let text = match std::fs::read_to_string(&file) {
                    Ok(text) => text,
                    Err(e) => {
                        tracing::warn!("Shell: TURU_SHELL_TEST {file}: {e}");
                        return;
                    }
                };
                // Whole lines only: one being written is read next time.
                let complete = text.rfind('\n').map_or(0, |at| at + 1).max(done);
                for line in text[done..complete]
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                {
                    if let Some(seconds) = line.strip_prefix("wait ").and_then(|s| s.trim().parse::<f64>().ok()) {
                        std::thread::sleep(Duration::from_secs_f64(seconds));
                    } else if proxy.send_event(ShellEvent::Test(line.to_owned())).is_err() {
                        return;
                    }
                }
                done = complete;
                if !follow {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        });
}

/// Accepts the clients' connections, each with the token, and hands them to the event loop.
fn spawn_acceptor(listener: TcpListener, token: String, proxy: EventLoopProxy<ShellEvent>) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Accept".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let hello = shell::read::<ToShell>(&mut stream);
                let _ = stream.set_read_timeout(None);
                match hello {
                    Ok(Some(ToShell::Hello {
                        token: given,
                        pid,
                        version,
                        ..
                    })) if given == token => {
                        tracing::info!("Shell: client {pid} (Turu {version}) connected");
                        if proxy.send_event(ShellEvent::Connected { pid, stream }).is_err() {
                            return;
                        }
                    }
                    _ => tracing::warn!("Shell: a connection without the token; ignored"),
                }
            }
        });
    if let Err(e) = spawned {
        tracing::error!("Shell: couldn't start accepting clients: {e}");
    }
}

/// Reads a tab's client: its pictures into `picture`, the rest to the event loop.
fn spawn_reader(
    id: u32,
    generation: u64,
    mut stream: TcpStream,
    picture: Arc<Mutex<Picture>>,
    proxy: EventLoopProxy<ShellEvent>,
) {
    let spawned = std::thread::Builder::new()
        .name(format!("Turu Shell Tab {id}"))
        .spawn(move || {
            loop {
                let message = match shell::read::<ToShell>(&mut stream) {
                    Ok(Some(message)) => message,
                    Ok(None) => break,
                    Err(e) => {
                        tracing::info!("Shell: tab {id}'s client went away: {e}");
                        break;
                    }
                };
                match message {
                    ToShell::Frame { width, height, len, .. } => match shell::read_payload(&mut stream, len) {
                        Ok(pixels) => {
                            *picture.lock().unwrap_or_else(|e| e.into_inner()) = Picture { width, height, pixels };
                            let _ = proxy.send_event(ShellEvent::Picture(id));
                        }
                        Err(e) => {
                            tracing::warn!("Shell: a broken picture from tab {id}: {e}");
                            break;
                        }
                    },
                    ToShell::Unknown | ToShell::Hello { .. } => {}
                    message => {
                        let _ = proxy.send_event(ShellEvent::Message(id, message));
                    }
                }
            }
            let _ = proxy.send_event(ShellEvent::Gone(id, generation));
        });
    if let Err(e) = spawned {
        tracing::error!("Shell: couldn't read tab {id}'s client: {e}");
    }
}

/// Stops a tab's client: its connection, stdin (`--exit-with-parent`) and process.
fn stop(tab: &mut Tab) {
    if let Some(out) = tab.out.take() {
        let _ = out.shutdown(std::net::Shutdown::Both);
    }
    if let Some(mut child) = tab.child.take() {
        drop(child.stdin.take());
        let _ = child.kill();
        let _ = child.wait();
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    blit: Blit,
    /// Draws a picture as it is, and over what's there with its (premultiplied) alpha.
    replace: wgpu::RenderPipeline,
    over: wgpu::RenderPipeline,
}

struct ShellApp {
    /// The tabs' control APIs, for the monitor hub (`turu::monitor_hub`).
    peers: Arc<Mutex<Vec<PeerSource>>>,
    address: String,
    token: String,
    proxy: EventLoopProxy<ShellEvent>,
    tabs: Vec<Tab>,
    /// The tab on screen.
    shown: u32,
    next_id: u32,
    gpu: Option<Gpu>,
    window: Option<Arc<Window>>,
    modifiers: ModifiersState,
    cursor: PhysicalPosition<f64>,
    /// The window's size and scale.
    size: Option<(PhysicalSize<u32>, f64)>,
    title: String,
    /// Grid View, on from a client's bar: what Turu's UI takes of the window (left, top, in points).
    grid: Option<(f32, f32)>,
    /// Headless Mode as a client last turned it on or off (`None`: none has yet): every tab's.
    headless_mode: Option<bool>,
    /// The window is full screen, as the tabs were last told (`ToClient::FullScreen`).
    full_screen: bool,
    /// Where clients hand over their surfaces (`shared_surface::listen`), if they can.
    surfaces_name: Option<String>,
    /// The clients' surfaces, by process id: the newest [`SHARED_POOL`] of each.
    surfaces: HashMap<u32, Vec<Kept>>,
    /// Surfaces the shell is done with once its GPU has done what's submitted (tab, pid, id).
    retiring: Vec<(u32, u32, u64)>,
    /// Releases waiting on the GPU.
    releases_pending: usize,
    /// A file dialog is open (one at a time, as the window's own).
    dialog_open: bool,
    /// A graphics choice was saved: the shell starts again once its loop ends (`run`).
    #[cfg(any(windows, target_os = "linux"))]
    start_again: bool,
    /// When `TURU_SHELL_TEST`'s last `rate` was.
    rate_since: Instant,
    /// The window is covered or minimized: the clients draw nothing (`ToClient::Covered`).
    covered: bool,
    occluded: bool,
    minimized: bool,
    /// The backends and power preference the clients' GPUs are chosen by: the window's is chosen
    /// the same way, so they can share surfaces.
    preferences: GlobalPreferences,
    /// The window's GPU (`shell::adapter_key`), set when it's made.
    adapter: String,
    /// `TURU_SHELL_TEST`'s `nosurfaces`: the next surface is taken as one that couldn't be opened.
    fail_surfaces: bool,
    /// `TURU_SHELL_TEST`'s `cover off`: the window counts as seen even when it's covered (a test
    /// behind other windows).
    never_covered: bool,
}

impl ShellApp {
    fn tab(&mut self, id: u32) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    /// Opens a tab with these flags and shows it.
    fn open_tab(&mut self, args: Vec<String>, password: Option<String>) {
        let id = self.next_id;
        self.next_id += 1;
        let mut tab = Tab {
            id,
            args,
            password,
            child: None,
            out: None,
            generation: 0,
            picture: Arc::default(),
            label: "Login".into(),
            busy: false,
            deaths: Vec::new(),
            restart_at: None,
            pictures: 0,
            closed: false,
            started: Instant::now(),
            api: None,
            api_token: turu_bot::control_api::new_token().unwrap_or_default(),
            shown: None,
            upload: None,
            waiting: None,
            adapter: None,
            pixels_only: false,
        };
        self.start(&mut tab);
        self.tabs.push(tab);
        self.select(id);
    }

    /// Starts the tab's client process; it says hello by itself.
    fn start(&self, tab: &mut Tab) {
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(e) => {
                tracing::error!("Shell: can't start a client: {e}");
                return;
            }
        };
        // Its control API on a port nobody listens on (it binds it a moment later), as Army >
        // Accounts' clients get.
        tab.api = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).ok();
        let mut command = Command::new(exe);
        command
            .arg(format!("--client={}", self.address))
            .arg("--exit-with-parent")
            .args(&tab.args)
            .env(shell::TOKEN_ENV, &self.token)
            .env_remove("TURU_PASSWORD")
            .env_remove(shell::SURFACES_ENV)
            .stdin(Stdio::piped());
        if let Some(name) = &self.surfaces_name {
            command.env(shell::SURFACES_ENV, name);
        }
        if let Some(api) = tab.api {
            command
                .arg(format!("--api={api}"))
                .env("TURU_API_TOKEN", &tab.api_token);
        }
        if let Some(password) = tab.password.as_deref().filter(|p| !p.is_empty()) {
            command.env("TURU_PASSWORD", password);
        }
        tab.adapter = None;
        tab.pixels_only = false;
        match command.spawn() {
            Ok(child) => {
                tracing::info!("Shell: tab {} started its client (process {})", tab.id, child.id());
                tab.child = Some(child);
                tab.started = Instant::now();
                tab.restart_at = None;
            }
            Err(e) => {
                tracing::error!("Shell: tab {}'s client didn't start: {e}", tab.id);
                self.schedule_restart(tab);
            }
        }
    }

    /// After a client died: when to start it again, or give up.
    fn schedule_restart(&self, tab: &mut Tab) {
        let now = Instant::now();
        tab.deaths.retain(|at| now.duration_since(*at) < DEATH_WINDOW);
        tab.deaths.push(now);
        // Never all tabs closed: the window would show nothing that takes input.
        let others_running = self.tabs.iter().any(|other| other.id != tab.id && other.out.is_some());
        if tab.deaths.len() >= MAX_DEATHS && others_running {
            tab.closed = true;
            tab.restart_at = None;
            tracing::warn!(
                "Shell: tab {} died {MAX_DEATHS} times in 10 minutes; left closed",
                tab.id
            );
        } else {
            let delay = RESTART_DELAYS[(tab.deaths.len() - 1).min(RESTART_DELAYS.len() - 1)];
            tab.restart_at = Some(now + delay);
            tracing::info!("Shell: starting tab {} again in {} s", tab.id, delay.as_secs());
        }
    }

    /// Shows tab `id`: the one before is hidden, the new one shown at the window's size. A closed
    /// tab starts again.
    fn select(&mut self, id: u32) {
        if !self.tabs.iter().any(|tab| tab.id == id) {
            return;
        }
        let old = self.shown;
        self.shown = id;
        // In Grid View the one before stays, as a tile (`show_current`).
        if old != id && self.grid_tiles().is_none() {
            self.hide(old);
        }
        let reopen = self.tab(id).is_some_and(|tab| tab.closed);
        if reopen && let Some(index) = self.tabs.iter().position(|tab| tab.id == id) {
            let mut tab = self.tabs.remove(index);
            tab.closed = false;
            tab.deaths.clear();
            self.start(&mut tab);
            self.tabs.insert(index, tab);
        }
        self.show_current();
        self.broadcast_tabs();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// A strip's click on tab `id`: shown if its client runs; if it's down, a closed one starts
    /// again, and the tab on screen stays until it connects (a tab without a client takes no input).
    fn select_requested(&mut self, id: u32) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        if self.tabs[index].out.is_some() {
            self.select(id);
            return;
        }
        if self.tabs[index].closed {
            let mut tab = self.tabs.remove(index);
            tab.closed = false;
            tab.deaths.clear();
            self.start(&mut tab);
            self.tabs.insert(index, tab);
            self.broadcast_tabs();
        }
    }

    /// Closes tab `id`'s client and removes it (never the last tab).
    fn close(&mut self, id: u32) {
        if self.tabs.len() < 2 {
            return;
        }
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let mut tab = self.tabs.remove(index);
        if let Some(pid) = tab.pid() {
            self.surfaces.remove(&pid);
        }
        stop(&mut tab);
        tracing::info!("Shell: tab {id} closed");
        if self.shown == id {
            let next = self.tabs[index.min(self.tabs.len() - 1)].id;
            self.shown = 0;
            self.select(next);
        } else {
            self.show_current();
            self.broadcast_tabs();
        }
    }

    /// Grid View's tiles, when it's on with two tabs or more: the window less Turu's UI, shared out
    /// in tab order (`turu_frontend::grid::layout`).
    fn grid_tiles(&self) -> Option<Vec<GridTile>> {
        let (left, top) = self.grid?;
        let (size, scale) = self.size?;
        if self.tabs.len() < 2 {
            return None;
        }
        let left = (f64::from(left) * scale).round() as u32;
        let top = (f64::from(top) * scale).round() as u32;
        let area = (size.width.saturating_sub(left), size.height.saturating_sub(top));
        let tiles = turu_frontend::grid::layout(self.tabs.len(), area.0, area.1);
        Some(
            self.tabs
                .iter()
                .zip(tiles)
                .map(|(tab, tile)| GridTile {
                    id: tab.id,
                    x: tile.x + left,
                    y: tile.y + top,
                    width: tile.width,
                    height: tile.height,
                })
                .collect(),
        )
    }

    /// Tells the tab on screen the window's size; in Grid View, every tab its part.
    fn show_current(&mut self) {
        let Some((size, scale)) = self.size else {
            return;
        };
        let id = self.shown;
        let tiles = self.grid_tiles();
        for tab in &mut self.tabs {
            let active = tab.id == id;
            let grid = tiles.as_ref().map(|tiles| GridShow {
                tiles: tiles.clone(),
                mine: tab.id,
                active,
            });
            let tile = grid.as_ref().filter(|_| !active).and_then(GridShow::tile);
            match (active, tile) {
                (true, _) => send(
                    tab,
                    &ToClient::Show {
                        width: size.width,
                        height: size.height,
                        scale: scale as f32,
                        grid,
                    },
                ),
                (false, Some(tile)) => send(
                    tab,
                    &ToClient::Show {
                        width: tile.width,
                        height: tile.height,
                        scale: scale as f32,
                        grid,
                    },
                ),
                (false, None) => {}
            }
        }
        if let Some(tab) = self.tab(id) {
            send(tab, &ToClient::Focus { focused: true });
        }
        self.update_title();
    }

    /// The bar's Grid toggle, from a client.
    fn set_grid(&mut self, grid: Option<(f32, f32)>) {
        let was = self.grid_tiles().is_some();
        self.grid = grid;
        let on = self.grid_tiles().is_some();
        if was && !on {
            // Back to one tab at a time: the others hide.
            let hidden: Vec<u32> = self
                .tabs
                .iter()
                .map(|tab| tab.id)
                .filter(|id| *id != self.shown)
                .collect();
            for id in hidden {
                self.hide(id);
            }
        }
        self.show_current();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Every client gets the list of tabs, for its tab strip and its monitor hub; the shell's own
    /// hub gets their control APIs.
    fn broadcast_tabs(&mut self) {
        let tabs: Vec<TabInfo> = self
            .tabs
            .iter()
            .map(|tab| TabInfo {
                id: tab.id,
                label: tab.label.clone(),
                busy: tab.busy,
                down: tab.down(),
                api: tab.api.map(|api| api.to_string()),
                token: Some(tab.api_token.clone()),
            })
            .collect();
        *self.peers.lock().unwrap_or_else(|e| e.into_inner()) = self
            .tabs
            .iter()
            .filter_map(|tab| {
                Some(PeerSource {
                    id: format!("tab:{}", tab.id),
                    label: tab.label.clone(),
                    api: tab.api?,
                    token: tab.api_token.clone(),
                })
            })
            .collect();
        let shown = self.shown;
        for tab in &mut self.tabs {
            let message = ToClient::Tabs {
                tabs: tabs.clone(),
                shown,
                you: Some(tab.id),
            };
            send(tab, &message);
        }
        self.update_title();
    }

    /// "Turu 0.3.1 – <Mugetsu's notes> – <the shown tab> (<n> tabs)".
    fn title_for(&self, label: Option<String>) -> String {
        let name = crate::mugetsu::window_title();
        match label {
            Some(label) if self.tabs.len() > 1 => format!("{name} – {label} ({} tabs)", self.tabs.len()),
            Some(label) => format!("{name} – {label}"),
            None => name,
        }
    }

    fn update_title(&mut self) {
        let label = self
            .tabs
            .iter()
            .find(|tab| tab.id == self.shown)
            .map(|tab| tab.label.clone());
        let title = self.title_for(label);
        if title != self.title
            && let Some(window) = &self.window
        {
            window.set_title(&title);
            self.title = title;
        }
    }

    fn connected(&mut self, pid: u32, stream: TcpStream) {
        let proxy = self.proxy.clone();
        let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.child.as_ref().is_some_and(|c| c.id() == pid))
        else {
            tracing::warn!("Shell: client {pid} isn't one of the tabs'; closing it");
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return;
        };
        tab.generation += 1;
        let reader = match stream.try_clone() {
            Ok(reader) => reader,
            Err(e) => {
                tracing::warn!("Shell: tab {}'s connection: {e}", tab.id);
                return;
            }
        };
        spawn_reader(tab.id, tab.generation, reader, tab.picture.clone(), proxy);
        tab.out = Some(stream);
        // As the other tabs have it, once one changed it.
        if let Some(on) = self.headless_mode {
            send(tab, &ToClient::HeadlessMode { on });
        }
        if self.covered {
            send(tab, &ToClient::Covered { covered: true });
        }
        if self.full_screen {
            send(tab, &ToClient::FullScreen { on: true });
        }
        let clients = army_clients();
        if !clients.is_empty() {
            send(tab, &ToClient::ArmyClients { clients });
        }
        let updates = turu_bot::updates::state();
        if updates != turu_bot::updates::State::default() {
            send(
                tab,
                &ToClient::Updates {
                    updates: Box::new(shell::UpdateInfo::from_state(&updates)),
                },
            );
        }
        let id = tab.id;
        if id == self.shown || self.grid_tiles().is_some() {
            self.show_current();
        } else {
            self.hide(id);
        }
        self.broadcast_tabs();
    }

    fn gone(&mut self, id: u32, generation: u64) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        if self.tabs[index].generation != generation {
            return;
        }
        let mut tab = self.tabs.remove(index);
        let status = tab.child.as_mut().and_then(|child| child.try_wait().ok().flatten());
        tracing::warn!("Shell: tab {id}'s client is gone ({status:?})");
        if let Some(pid) = tab.pid() {
            self.surfaces.remove(&pid);
        }
        tab.shown = None;
        tab.waiting = None;
        stop(&mut tab);
        self.schedule_restart(&mut tab);
        self.tabs.insert(index, tab);
        // The tab on screen is gone: show a running one, so the window still takes input (the dead
        // one is selected again from the strip once it's back).
        if id == self.shown
            && let Some(running) = self.tabs.iter().find(|tab| tab.out.is_some()).map(|tab| tab.id)
        {
            tracing::info!("Shell: tab {id} was on screen; showing tab {running}");
            self.select(running);
        } else {
            self.broadcast_tabs();
        }
    }

    fn message(&mut self, id: u32, message: ToShell) {
        match message {
            ToShell::SelectTab { id: target } => self.select_requested(target),
            ToShell::Grid { on, left, top } => self.set_grid(on.then_some((left, top))),
            ToShell::CloseTab { id: target } => self.close(target),
            ToShell::OpenTab {
                username,
                password,
                server,
                script,
                run_script,
                playlist,
                run_playlist,
                options_storage,
            } => {
                let mut args = turu_frontend::slots::open_args(username, server, script, run_script);
                args.extend(turu_frontend::slots::playlist_args(
                    playlist,
                    run_playlist,
                    options_storage,
                ));
                self.open_tab(args, password);
            }
            ToShell::Status { label, busy } => {
                if let Some(tab) = self.tab(id)
                    && (tab.label != label || tab.busy != busy)
                {
                    tab.label = label;
                    tab.busy = busy;
                    self.broadcast_tabs();
                }
            }
            // Every tab's: to the other clients.
            ToShell::HeadlessMode { on } => {
                if self.headless_mode != Some(on) {
                    self.headless_mode = Some(on);
                    for tab in self.tabs.iter_mut().filter(|tab| tab.id != id) {
                        send(tab, &ToClient::HeadlessMode { on });
                    }
                }
            }
            ToShell::Surface { id: surface, .. } => self.surface_picture(id, surface),
            // A tab's Download & restart: installed here; the shell starts again on it, and so
            // does every tab (`install_update` restarts this process with its arguments).
            ToShell::InstallUpdate { on_quit } => match turu::install_update(on_quit) {
                Ok(started) => tracing::info!("Shell: tab {id} asked for the update: {started}"),
                Err(e) => {
                    tracing::warn!("Shell: tab {id} asked for the update: {e}");
                    turu_bot::updates::update_state(|s| s.install = turu_bot::updates::Install::Failed(e));
                }
            },
            ToShell::CheckUpdates => turu::check_client_updates_now(),
            ToShell::FullScreen { on } => self.set_full_screen(on),
            // The cursor of the tab on screen (another's would be stale by the time it shows).
            ToShell::Cursor { cursor } => {
                if id == self.shown
                    && let Some(window) = &self.window
                {
                    set_cursor(window, cursor);
                }
            }
            #[cfg(any(windows, target_os = "linux"))]
            ToShell::ChooseGraphics { graphics } => self.choose_graphics(id, graphics),
            #[cfg(not(any(windows, target_os = "linux")))]
            ToShell::ChooseGraphics { graphics } => {
                tracing::warn!("Shell: tab {id} asked for graphics {graphics:?}, which this system doesn't choose");
            }
            // A tab's Army > Accounts launches: the shell's Army starts them, so they outlive
            // the tab's client, and the shell's web monitor and Report To list them.
            ToShell::Launch { launches } => {
                tracing::info!("Shell: tab {id} launches {} account(s)", launches.len());
                turu::army().launch_all(launches, |username, result| match result {
                    Ok(()) => tracing::info!("Shell: started {username}"),
                    Err(e) => tracing::warn!("Shell: couldn't start {username}: {e}"),
                });
            }
            ToShell::ArmyClient { username, restart } => {
                if restart {
                    // The keychain may be asked for its password: not on the event loop.
                    let _ = std::thread::Builder::new()
                        .name("Turu Shell Restart".into())
                        .spawn(move || {
                            if let Err(e) = turu::army().restart(&username) {
                                tracing::warn!("Shell: couldn't restart {username}: {e}");
                            }
                        });
                } else {
                    turu::army().close(&username);
                }
            }
            ToShell::Gpu { adapter } => {
                tracing::info!("Shell: tab {id}'s client draws on {adapter}");
                if let Some(tab) = self.tab(id) {
                    tab.adapter = Some(adapter);
                }
            }
            #[cfg(windows)]
            ToShell::SharedHandle {
                id: surface,
                route,
                handle,
                width,
                height,
                size,
                memory_type,
            } => {
                let taken = self.tab(id).and_then(|tab| {
                    let child = tab.child.as_ref()?;
                    let taken =
                        crate::shared_gpu::take_handle(child, &route, handle, width, height, size, memory_type)?;
                    Some((child.id(), taken))
                });
                match taken {
                    Some((pid, taken)) => self.surface_arrived(pid, surface, taken),
                    None => {
                        if let Some(index) = self.tabs.iter().position(|tab| tab.id == id)
                            && let Some(pid) = self.tabs[index].pid()
                        {
                            self.no_surfaces(index, pid, "its handle couldn't be copied");
                        }
                    }
                }
            }
            ToShell::FileDialog {
                id: request,
                save,
                title,
                directory,
                file_name,
                filters,
            } => {
                let mut dialog = rfd::AsyncFileDialog::new();
                for (name, extensions) in &filters {
                    dialog = dialog.add_filter(name, extensions);
                }
                if let Some(title) = title {
                    dialog = dialog.set_title(title);
                }
                if let Some(directory) = directory {
                    dialog = dialog.set_directory(directory);
                }
                if let Some(file_name) = file_name {
                    dialog = dialog.set_file_name(file_name);
                }
                self.file_dialog(id, request, save, dialog);
            }
            ToShell::OpenUrl { url } => {
                if let Err(e) = webbrowser::open(&url) {
                    tracing::warn!("Couldn't open {url}: {e}");
                }
            }
            // The text-field messages are Android's (its slots); a desktop client sends keys.
            #[cfg(not(windows))]
            ToShell::SharedHandle { .. } => {}
            ToShell::Frame { .. }
            | ToShell::Hello { .. }
            | ToShell::FieldShow { .. }
            | ToShell::FieldUpdate { .. }
            | ToShell::FieldHide { .. }
            | ToShell::PickFile { .. }
            | ToShell::SaveFile { .. }
            | ToShell::Share { .. }
            | ToShell::Install { .. }
            | ToShell::SoftKeyboard { .. }
            | ToShell::Unknown => {}
        }
    }

    /// Shows a tab's file dialog over the window (made here, on the event loop's thread, as macOS
    /// needs), and answers the tab once it's closed; one at a time.
    fn file_dialog(&mut self, id: u32, request: u32, save: bool, mut dialog: rfd::AsyncFileDialog) {
        let Some(pid) = self.tab(id).and_then(|tab| tab.pid()) else {
            return;
        };
        if self.dialog_open {
            if let Some(tab) = self.tab(id) {
                send(
                    tab,
                    &ToClient::Answer {
                        id: request,
                        value: None,
                    },
                );
            }
            return;
        }
        if let Some(window) = &self.window {
            dialog = dialog.set_parent(&**window);
        }
        self.dialog_open = true;
        let proxy = self.proxy.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Shell Dialog".into())
            .spawn(move || {
                let chosen = if save {
                    futures::executor::block_on(dialog.save_file())
                } else {
                    futures::executor::block_on(dialog.pick_file())
                };
                let path = chosen.map(|file| file.path().to_string_lossy().into_owned());
                let _ = proxy.send_event(ShellEvent::Answered(id, pid, request, path));
            });
        if let Err(e) = spawned {
            tracing::warn!("Shell: couldn't wait for a file dialog: {e}");
            self.dialog_open = false;
        }
    }

    /// A tab's Options > Application > Graphics (the window has it in the game's right-click menu,
    /// which a client lacks): asked over the window as a file dialog is (one at a time), then
    /// saved, and the shell starts again (`GraphicsChosen`).
    #[cfg(any(windows, target_os = "linux"))]
    fn choose_graphics(&mut self, id: u32, value: String) {
        use crate::mugetsu::{self, Graphics};
        let choice = if value.is_empty() {
            None
        } else if let Some(graphics) = Graphics::from_arg(&value) {
            Some(graphics)
        } else {
            tracing::warn!("Shell: tab {id} asked for unknown graphics {value:?}");
            return;
        };
        if choice == mugetsu::saved_graphics() || self.dialog_open {
            return;
        }
        let name = choice.map_or("Automatic", Graphics::name);
        let mut dialog = rfd::AsyncMessageDialog::new()
            .set_level(rfd::MessageLevel::Info)
            .set_title("Turu")
            .set_description(format!(
                "Turu will restart to use {name} graphics, every tab with it.\n\n\
                 You can switch again any time: Options > Application > Graphics."
            ))
            .set_buttons(rfd::MessageButtons::OkCancel);
        if let Some(window) = &self.window {
            dialog = dialog.set_parent(&**window);
        }
        self.dialog_open = true;
        let proxy = self.proxy.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Shell Dialog".into())
            .spawn(move || {
                let ok = futures::executor::block_on(dialog.show()) == rfd::MessageDialogResult::Ok;
                let _ = proxy.send_event(ShellEvent::GraphicsChosen(ok.then_some(value)));
            });
        if let Err(e) = spawned {
            tracing::warn!("Shell: couldn't ask about the graphics: {e}");
            self.dialog_open = false;
        }
    }

    /// Starts the clients whose restart is due; when to look again.
    fn restart_due(&mut self) -> Option<Instant> {
        let now = Instant::now();
        let mut changed = false;
        for index in 0..self.tabs.len() {
            let due = self.tabs[index].restart_at.is_some_and(|at| at <= now);
            if due {
                let mut tab = self.tabs.remove(index);
                self.start(&mut tab);
                self.tabs.insert(index, tab);
                changed = true;
            }
            // A client that never connected is given up on, and restarted.
            let tab = &mut self.tabs[index];
            if tab.out.is_none() && tab.child.is_some() && now.duration_since(tab.started) > CONNECT_TIMEOUT {
                tracing::warn!("Shell: tab {}'s client didn't connect in time", tab.id);
                let mut tab = self.tabs.remove(index);
                stop(&mut tab);
                self.schedule_restart(&mut tab);
                self.tabs.insert(index, tab);
                changed = true;
            }
        }
        if changed {
            self.broadcast_tabs();
        }
        let connect_checks = self
            .tabs
            .iter()
            .filter(|tab| tab.out.is_none() && tab.child.is_some())
            .map(|tab| tab.started + CONNECT_TIMEOUT);
        self.tabs
            .iter()
            .filter_map(|tab| tab.restart_at)
            .chain(connect_checks)
            .min()
    }

    /// The window was covered or minimized, or is seen again: the clients stop or start drawing,
    /// as the window's own tabs do (`app.rs`, `occluded`).
    fn set_covered(&mut self, covered: bool) {
        let covered = covered && !self.never_covered;
        if self.covered == covered {
            return;
        }
        self.covered = covered;
        tracing::info!(
            "Shell: the window is {}",
            if covered { "covered" } else { "seen again" }
        );
        for tab in &mut self.tabs {
            send(tab, &ToClient::Covered { covered });
        }
    }

    /// The window's size changed: the GPU surface and the tab on screen follow.
    /// A tab's Options > Application > Full Screen, or Esc: borderless on the window's screen, as
    /// the window's right-click menu does. The tabs hear of it once the window has changed
    /// (`resized`).
    fn set_full_screen(&self, on: bool) {
        if let Some(window) = &self.window {
            window.set_fullscreen(on.then_some(winit::window::Fullscreen::Borderless(None)));
        }
    }

    /// Tells the tabs when the window went full screen or left it (also by the system's own
    /// button), for Options.
    fn full_screen_changed(&mut self) {
        let on = self.window.as_ref().is_some_and(|window| window.fullscreen().is_some());
        if on != self.full_screen {
            tracing::info!("Shell: the window {} full screen", if on { "went" } else { "left" });
            self.full_screen = on;
            for tab in &mut self.tabs {
                send(tab, &ToClient::FullScreen { on });
            }
        }
    }

    fn resized(&mut self) {
        self.full_screen_changed();
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        let scale = window.scale_factor();
        // How minimizing shows on Windows (macOS reports it as occluded).
        let minimized = size.width == 0 || size.height == 0;
        if minimized != self.minimized {
            self.minimized = minimized;
            let covered = minimized || self.occluded;
            self.set_covered(covered);
        }
        if size.width == 0 || size.height == 0 || self.size == Some((size, scale)) {
            return;
        }
        self.size = Some((size, scale));
        if let Some(gpu) = &mut self.gpu {
            gpu.config.width = size.width;
            gpu.config.height = size.height;
            gpu.surface.configure(&gpu.device, &gpu.config);
        }
        self.show_current();
    }

    fn mods(&self) -> Mods {
        let m = self.modifiers;
        Mods {
            shift: m.shift_key(),
            ctrl: m.control_key(),
            alt: m.alt_key(),
            command: if cfg!(target_os = "macos") {
                m.super_key()
            } else {
                m.control_key()
            },
        }
    }

    /// Sends to the tab on screen.
    fn send_shown(&mut self, message: &ToClient) {
        let id = self.shown;
        if let Some(tab) = self.tab(id) {
            send(tab, message);
        }
    }

    /// Whether tab `id`'s pictures are drawn: it's on screen, or a tile of Grid View.
    fn visible(&self, id: u32) -> bool {
        id == self.shown || self.grid_tiles().is_some()
    }

    /// Hides tab `id`: no more pictures, and the shell lets go of its surfaces.
    fn hide(&mut self, id: u32) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let tab = &mut self.tabs[index];
        send(tab, &ToClient::Hide);
        tab.waiting = None;
        // A pixel picture stays, to show at once if the tab comes back.
        if tab.shown.as_ref().is_some_and(|shown| shown.surface.is_some()) {
            tab.shown = None;
        }
        // The client drops its own; the newest of them may still be on its way.
        if let Some(pid) = tab.pid() {
            self.surfaces.remove(&pid);
        }
    }

    /// Tab `tab`'s client handed over `surface`.
    fn surface_arrived(&mut self, pid: u32, id: u64, surface: SharedSurface) {
        let kept = self.surfaces.entry(pid).or_default();
        kept.push(Kept {
            id,
            surface,
            opened: None,
        });
        // A client draws into its newest few in turn: older ones are of another size, or gone.
        kept.sort_by_key(|kept| kept.id);
        let extra = kept.len().saturating_sub(SHARED_POOL);
        kept.drain(..extra);
        let waiting = self
            .tabs
            .iter()
            .find(|tab| tab.pid() == Some(pid) && tab.waiting == Some(id))
            .map(|tab| tab.id);
        if let Some(tab) = waiting {
            self.surface_picture(tab, id);
        }
    }

    /// Tab `id`'s newest picture is in its client's surface `surface`.
    fn surface_picture(&mut self, id: u32, surface: u64) {
        if let Some(tab) = self.tab(id) {
            tab.pictures += 1;
        }
        let visible = self.visible(id);
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let Some(pid) = self.tabs[index].pid() else {
            return;
        };
        if !visible {
            self.retiring.push((id, pid, surface));
            return;
        }
        // A client on another GPU or backend can't share its surfaces with this one.
        if let Some(theirs) = self.tabs[index]
            .adapter
            .as_ref()
            .filter(|theirs| **theirs != self.adapter)
            && !self.adapter.is_empty()
        {
            let reason = format!("its GPU is {theirs}, the shell's {}", self.adapter);
            self.retiring.push((id, pid, surface));
            self.no_surfaces(index, pid, &reason);
            return;
        }
        let mut failed = std::mem::take(&mut self.fail_surfaces);
        let opened = match (&self.gpu, self.surfaces.get_mut(&pid)) {
            (Some(gpu), Some(kept)) => kept.iter_mut().find(|kept| kept.id == surface).and_then(|kept| {
                if kept.opened.is_none() {
                    kept.opened = kept.surface.texture(&gpu.device).map(|texture| {
                        let view = texture.create_view(&Default::default());
                        let bind_group = gpu.blit.bind_group(&gpu.device, &view);
                        (texture, bind_group)
                    });
                    failed = kept.opened.is_none();
                }
                let size = kept.surface.size();
                kept.opened.as_ref().map(|(_, bind_group)| (bind_group.clone(), size))
            }),
            _ => None,
        };
        if failed {
            self.retiring.push((id, pid, surface));
            self.no_surfaces(index, pid, "the shell couldn't open one");
            return;
        }
        let tab = &mut self.tabs[index];
        let Some((bind_group, size)) = opened else {
            // Its port is on its way (another channel): drawn when it comes.
            if let Some(older) = tab.waiting.replace(surface) {
                self.retiring.push((id, pid, older));
            }
            return;
        };
        if let Some(older) = tab.waiting.take().filter(|older| *older != surface) {
            self.retiring.push((id, pid, older));
        }
        let new = Shown {
            bind_group,
            size,
            surface: Some(surface),
        };
        if let Some(older) = tab.shown.replace(new).and_then(|shown| shown.surface) {
            self.retiring.push((id, pid, older));
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// The tab at `index`'s client goes back to pixels: its surfaces are dropped here.
    fn no_surfaces(&mut self, index: usize, pid: u32, reason: &str) {
        let tab = &mut self.tabs[index];
        if tab.pixels_only {
            return;
        }
        tracing::warn!(
            "Shell: tab {}'s surfaces can't be used ({reason}): it sends pixels",
            tab.id
        );
        tab.pixels_only = true;
        tab.waiting = None;
        send(
            tab,
            &ToClient::NoSurfaces {
                reason: reason.to_owned(),
            },
        );
        self.surfaces.remove(&pid);
    }

    /// Tab `id`'s client sent its pixels: uploaded into its texture.
    fn pixel_picture(&mut self, id: u32) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return;
        };
        tab.pictures += 1;
        let picture = std::mem::take(&mut *tab.picture.lock().unwrap_or_else(|e| e.into_inner()));
        let size = (picture.width, picture.height);
        if picture.pixels.len() < size.0 as usize * size.1 as usize * 4 || size.0 == 0 || size.1 == 0 {
            return;
        }
        if tab
            .upload
            .as_ref()
            .is_none_or(|(texture, _)| (texture.width(), texture.height()) != size)
        {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Turu tab picture"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let bind_group = gpu
                .blit
                .bind_group(&gpu.device, &texture.create_view(&Default::default()));
            tab.upload = Some((texture, bind_group));
        }
        let (texture, bind_group) = tab.upload.as_ref().expect("made above");
        gpu.queue.write_texture(
            texture.as_image_copy(),
            &picture.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.0 * 4),
                rows_per_image: Some(size.1),
            },
            texture.size(),
        );
        let new = Shown {
            bind_group: bind_group.clone(),
            size,
            surface: None,
        };
        let older = tab.shown.replace(new).and_then(|shown| shown.surface);
        let pid = tab.pid();
        if let (Some(older), Some(pid)) = (older, pid) {
            self.retiring.push((id, pid, older));
        }
        if self.visible(id)
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
    }

    /// Asks the GPU to say when what's submitted so far is done, and gives the retiring surfaces
    /// back then.
    fn release_retiring(&mut self) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        if self.retiring.is_empty() {
            return;
        }
        let retired = std::mem::take(&mut self.retiring);
        let proxy = self.proxy.clone();
        gpu.queue.submit([]);
        gpu.queue
            .on_submitted_work_done(move || drop(proxy.send_event(ShellEvent::Released(retired))));
        self.releases_pending += 1;
    }

    /// The surfaces' clients may draw into them again (unless the client has since been
    /// restarted: its process id differs).
    fn released(&mut self, retired: Vec<(u32, u32, u64)>) {
        self.releases_pending = self.releases_pending.saturating_sub(1);
        for (id, pid, surface) in retired {
            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id && tab.pid() == Some(pid)) {
                send(tab, &ToClient::Release { id: surface });
            }
        }
    }

    /// Draws the window: the shown tab's newest picture, or in Grid View each tile's under the
    /// active tab's (transparent where its own tile isn't). Nothing until the tab on screen has a
    /// picture (the window keeps what it showed).
    fn render(&self, gpu: &Gpu, view: &wgpu::TextureView, size: (u32, u32)) -> Option<wgpu::CommandBuffer> {
        let active = self.tabs.iter().find(|tab| tab.id == self.shown)?.shown.as_ref()?;
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Turu shell"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            let place = |pass: &mut wgpu::RenderPass<'static>, x: u32, y: u32, w: u32, h: u32| {
                let (x, y) = (x.min(size.0), y.min(size.1));
                let (w, h) = (w.min(size.0 - x), h.min(size.1 - y));
                pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                w > 0 && h > 0
            };
            match self.grid_tiles() {
                Some(tiles) => {
                    // Just after a tile was clicked, its client may still send its tile's picture:
                    // in its tile until the window's comes.
                    let whole = active.size == size;
                    for tile in tiles.iter().filter(|tile| tile.id != self.shown || !whole) {
                        let Some(picture) = self
                            .tabs
                            .iter()
                            .find(|tab| tab.id == tile.id)
                            .and_then(|tab| tab.shown.as_ref())
                        else {
                            continue;
                        };
                        // The window's picture of a tab that was active a moment ago: its tile's
                        // is on the way.
                        if picture.size == size {
                            continue;
                        }
                        if place(&mut pass, tile.x, tile.y, tile.width, tile.height) {
                            gpu.blit.draw(&gpu.replace, &picture.bind_group, &mut pass);
                        }
                    }
                    if whole && place(&mut pass, 0, 0, size.0, size.1) {
                        gpu.blit.draw(&gpu.over, &active.bind_group, &mut pass);
                    }
                }
                None => {
                    if place(&mut pass, 0, 0, size.0, size.1) {
                        gpu.blit.draw(&gpu.replace, &active.bind_group, &mut pass);
                    }
                }
            }
        }
        Some(encoder.finish())
    }

    fn draw(&self) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        if self
            .tabs
            .iter()
            .find(|tab| tab.id == self.shown)
            .is_none_or(|tab| tab.shown.is_none())
        {
            return;
        }
        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let size = (gpu.config.width, gpu.config.height);
        if let Some(commands) = self.render(gpu, &view, size) {
            gpu.queue.submit([commands]);
        }
        gpu.queue.present(frame);
        crate::mugetsu::frame_presented();
    }

    /// `TURU_SHELL_TEST`'s `dump`: the window drawn into a texture and read back, as a PNG.
    fn dump(&self, path: &str) {
        let (Some(gpu), Some((size, _))) = (&self.gpu, self.size) else {
            return;
        };
        let (width, height) = (size.width, size.height);
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Turu shell dump"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let Some(commands) = self.render(gpu, &texture.create_view(&Default::default()), (width, height)) else {
            tracing::info!("Shell test: nothing to dump yet");
            return;
        };
        let padded = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        gpu.queue.submit([commands, encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        if let Err(e) = gpu.device.poll(wgpu::PollType::wait_indefinitely()) {
            tracing::warn!("Shell test: dump: {e}");
            return;
        }
        let Ok(data) = buffer.slice(..).get_mapped_range() else {
            tracing::warn!("Shell test: dump: couldn't read the picture back");
            return;
        };
        let bgra = matches!(
            gpu.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for row in 0..height as usize {
            let start = row * padded as usize;
            rgba.extend_from_slice(&data[start..start + width as usize * 4]);
        }
        if bgra {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        let saved = image::RgbaImage::from_raw(width, height, rgba).map(|image| image.save(path));
        tracing::info!("Shell test: dump of tab {} to {path}: {saved:?}", self.shown);
    }

    /// A press, release or move of the mouse: to the tab on screen (in Grid View, the active tab
    /// asks for another tab whose tile was clicked).
    fn pointer_event(&mut self, kind: PointerKind, button: Button, at: PhysicalPosition<f64>) {
        self.send_shown(&pointer(kind, button, at));
    }

    /// A line of `TURU_SHELL_TEST`'s file.
    fn test(&mut self, line: &str) {
        tracing::info!("Shell test: {line}");
        let (command, arg) = line.split_once(' ').unwrap_or((line, ""));
        let id = arg.trim().parse::<u32>().ok();
        // `text <string>`: typed text, as a keyboard's.
        if command == "text" {
            self.send_shown(&ToClient::Text { text: arg.to_owned() });
            return;
        }
        // `key cmd+V`: a press and release, with `cmd`, `ctrl`, `shift` or `alt` before the key.
        if command == "key" {
            let mut mods = Mods::default();
            let mut parts: Vec<&str> = arg.trim().split('+').collect();
            let key = parts.pop().unwrap_or_default().to_owned();
            for part in parts {
                match part {
                    "cmd" => mods.command = true,
                    "ctrl" => mods.ctrl = true,
                    "shift" => mods.shift = true,
                    "alt" => mods.alt = true,
                    _ => {}
                }
            }
            if mods.command && cfg!(not(target_os = "macos")) {
                mods.ctrl = true;
            }
            for pressed in [true, false] {
                self.send_shown(&ToClient::Key {
                    key: key.clone(),
                    pressed,
                    mods,
                });
            }
            return;
        }
        // `resize 1600 900`: the window's inner size in points, as dragging its edge would.
        // `nosurfaces`: the next shared surface counts as one the shell couldn't open (the fallback
        // to pixels, tried where every surface opens).
        // `cover off`: the window counts as seen even when covered; `cover on`: as reported.
        if command == "cover" {
            self.never_covered = arg.trim() == "off";
            let covered = self.occluded || self.minimized;
            self.set_covered(covered);
            return;
        }
        if command == "nosurfaces" {
            self.fail_surfaces = true;
            return;
        }
        // `rate`: each tab's pictures a second since the last `rate`.
        if command == "rate" {
            let elapsed = self.rate_since.elapsed().as_secs_f64();
            self.rate_since = Instant::now();
            for tab in &mut self.tabs {
                let rate = f64::from(std::mem::take(&mut tab.pictures)) / elapsed.max(0.001);
                tracing::info!(
                    "Shell test: tab {} ({}): {rate:.1} pictures a second",
                    tab.id,
                    tab.label
                );
            }
            return;
        }
        // `minimize on` / `minimize off`: as the window's yellow button would (macOS reports it as
        // covered).
        if command == "minimize" {
            if let Some(window) = &self.window {
                window.set_minimized(arg.trim() == "on");
            }
            return;
        }
        // `title`: the window's title and size, for the log.
        if command == "title" {
            if let Some(window) = &self.window {
                let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                tracing::info!(
                    "Shell test: title \"{}\", {:.0}x{:.0} points",
                    window.title(),
                    size.width,
                    size.height
                );
            }
            return;
        }
        // `fullscreen on` / `fullscreen off`: as a tab's Options > Application > Full Screen.
        if command == "fullscreen" {
            self.set_full_screen(arg.trim() == "on");
            return;
        }
        if command == "resize" {
            let size: Vec<f64> = arg.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            if let (&[width, height], Some(window)) = (size.as_slice(), &self.window) {
                let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width, height));
            }
            return;
        }
        if command == "click" {
            let at: Vec<f64> = arg.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            if let [x, y] = at[..] {
                let at = PhysicalPosition::new(x, y);
                for kind in [PointerKind::Move, PointerKind::Down, PointerKind::Up] {
                    self.pointer_event(kind, Button::Left, at);
                }
            }
            return;
        }
        match (command, id) {
            ("open", _) => self.open_tab(Vec::new(), None),
            ("select", Some(id)) => self.select_requested(id),
            ("close", Some(id)) => self.close(id),
            ("kill", Some(id)) => {
                if let Some(child) = self.tab(id).and_then(|tab| tab.child.as_mut()) {
                    let _ = child.kill();
                }
            }
            ("dump", _) => self.dump(arg.trim()),
            _ => tracing::warn!("Shell test: what's \"{line}\"?"),
        }
    }

    fn key(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        // Esc leaves full screen, as in the window (the tab gets it too).
        if pressed && self.full_screen && event.logical_key == Key::Named(NamedKey::Escape) {
            self.set_full_screen(false);
        }
        if let Some(key) = egui_key_name(&event.logical_key) {
            let mods = self.mods();
            self.send_shown(&ToClient::Key {
                key: key.to_owned(),
                pressed,
                mods,
            });
        }
        let command = self.modifiers.control_key() || self.modifiers.super_key();
        if pressed
            && !command
            && let Some(text) = event.text.as_ref().filter(|text| text.chars().all(|c| !c.is_control()))
        {
            self.send_shown(&ToClient::Text { text: text.to_string() });
        }
    }
}

/// Sends to a tab's client, if it's connected.
fn send(tab: &mut Tab, message: &ToClient) {
    if let Some(out) = tab.out.as_mut()
        && let Err(e) = shell::write(out, message, None)
    {
        tracing::info!("Shell: couldn't reach tab {}'s client: {e}", tab.id);
    }
}

impl ApplicationHandler<ShellEvent> for ShellApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // As the window's: icon, title with Mugetsu's notes, Linux's app id and startup token,
        // smallest size; shown once sized.
        let attributes = crate::app::window_attributes(event_loop)
            .with_title(self.title_for(None))
            .with_visible(false)
            .with_min_inner_size(winit::dpi::LogicalSize::<u32>::from(crate::app::MIN_WINDOW_SIZE));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                tracing::error!("Shell: couldn't open the window: {e}");
                event_loop.exit();
                return;
            }
        };
        // As the window's: turu.swf's stage under the bar, or `--width` / `--height` (Mugetsu's
        // 958x584 off macOS), no bigger than the screens; then shown, and `--fullscreen`.
        let cli = &self.preferences.cli;
        let max = crate::util::get_screen_size(&window);
        window.set_max_inner_size(Some(max));
        let scale = window.scale_factor();
        let size = crate::app::window_size(
            (f64::from(STAGE.0), f64::from(STAGE.1)),
            (cli.width, cli.height),
            f64::from(crate::gui::top_height()),
            scale,
        );
        let min = winit::dpi::LogicalSize::<u32>::from(crate::app::MIN_WINDOW_SIZE);
        let _ = window.request_inner_size(winit::dpi::Size::clamp(size, min.into(), max.into(), scale));
        window.set_visible(true);
        // Once shown: macOS doesn't take a hidden window full screen.
        if cli.fullscreen {
            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }
        match make_gpu(window.clone(), &self.preferences) {
            Ok((gpu, adapter)) => {
                self.gpu = Some(gpu);
                self.adapter = adapter;
            }
            Err(e) => {
                tracing::error!("Shell: {e}");
                event_loop.exit();
                return;
            }
        }
        // As the window's: Windows' title-bar menu chooses the graphics (and starts Turu again).
        #[cfg(windows)]
        if crate::mugetsu::plays_aqw() {
            crate::windows::add_graphics_menu(&window);
        }
        self.window = Some(window);
        self.resized();
    }

    #[cfg_attr(not(any(windows, target_os = "linux")), allow(unused_variables))]
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: ShellEvent) {
        match event {
            ShellEvent::Connected { pid, stream } => self.connected(pid, stream),
            ShellEvent::Picture(id) => self.pixel_picture(id),
            ShellEvent::SurfaceArrived { pid, id, surface } => self.surface_arrived(pid, id, surface),
            ShellEvent::Released(retired) => self.released(retired),
            ShellEvent::ArmyClients(clients) => {
                let message = ToClient::ArmyClients { clients };
                for tab in &mut self.tabs {
                    send(tab, &message);
                }
            }
            ShellEvent::Updates(state) => {
                let message = ToClient::Updates {
                    updates: Box::new(shell::UpdateInfo::from_state(&state)),
                };
                for tab in &mut self.tabs {
                    send(tab, &message);
                }
            }
            ShellEvent::Answered(id, pid, request, value) => {
                self.dialog_open = false;
                if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id && tab.pid() == Some(pid)) {
                    send(tab, &ToClient::Answer { id: request, value });
                }
            }
            ShellEvent::Message(id, message) => self.message(id, message),
            #[cfg(any(windows, target_os = "linux"))]
            ShellEvent::GraphicsChosen(value) => {
                self.dialog_open = false;
                if let Some(value) = value {
                    let choice = crate::mugetsu::Graphics::from_arg(&value);
                    crate::mugetsu::save_graphics(choice);
                    tracing::info!(
                        "Shell: graphics changed to {}; starting again",
                        choice.map_or("Automatic", crate::mugetsu::Graphics::name)
                    );
                    self.start_again = true;
                    event_loop.exit();
                }
            }
            ShellEvent::Gone(id, generation) => self.gone(id, generation),
            ShellEvent::Test(line) => self.test(&line),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.release_retiring();
        let mut wake = self.restart_due();
        if self.releases_pending > 0
            && let Some(gpu) = &self.gpu
        {
            // Runs the GPU's callbacks: the releases come as events.
            let _ = gpu.device.poll(wgpu::PollType::Poll);
            let poll = Instant::now() + RELEASE_POLL;
            wake = Some(wake.map_or(poll, |at| at.min(poll)));
        }
        match wake {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.resized(),
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::Focused(focused) => self.send_shown(&ToClient::Focus { focused }),
            // winit reports it on macOS and X11 only.
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                let covered = occluded || self.minimized;
                self.set_covered(covered);
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                self.send_shown(&pointer(PointerKind::Move, Button::Left, position));
            }
            WindowEvent::CursorLeft { .. } => {
                let at = self.cursor;
                self.send_shown(&pointer(PointerKind::Leave, Button::Left, at));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    MouseButton::Middle => Button::Middle,
                    _ => return,
                };
                let kind = if state == ElementState::Pressed {
                    PointerKind::Down
                } else {
                    PointerKind::Up
                };
                let at = self.cursor;
                self.pointer_event(kind, button, at);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (-x, -y),
                    MouseScrollDelta::PixelDelta(p) => (-(p.x / 40.0) as f32, -(p.y / 40.0) as f32),
                };
                let (x, y) = (self.cursor.x as f32, self.cursor.y as f32);
                self.send_shown(&ToClient::Wheel { dx, dy, x, y });
            }
            WindowEvent::KeyboardInput { event, .. } => self.key(&event),
            _ => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        for tab in &mut self.tabs {
            stop(tab);
        }
    }
}

fn pointer(kind: PointerKind, button: Button, at: PhysicalPosition<f64>) -> ToClient {
    ToClient::Pointer {
        kind,
        button,
        x: at.x as f32,
        y: at.y as f32,
    }
}

/// The window's surface on the GPU, and what draws the clients' pictures into it.
///
/// The GPU is chosen as the window and the clients choose theirs (`select_wgpu_backend`: Options'
/// backends in turn, then the others; Options' power), so that both sides are on the same GPU and
/// backend and can share surfaces; with it, its key (`shell::adapter_key`).
fn make_gpu(window: Arc<Window>, preferences: &GlobalPreferences) -> Result<(Gpu, String), Error> {
    let wanted: wgpu::Backends = preferences.graphics_backends().into();
    // As the window: a driver crash from here until shortly after the first frame counts as the
    // Vulkan driver crashing at startup (`mugetsu`), and the next start takes DirectX 12.
    crate::mugetsu::vulkan_startup_begin(wanted.contains(wgpu::Backends::VULKAN));
    // With the window as the display: OpenGL needs it (Linux).
    let (instance, backend) = crate::gui::select_wgpu_backend(wanted, Some(&window))?;
    let surface = instance.create_surface(window.clone())?;
    let (adapter, device, queue) =
        match futures::executor::block_on(ruffle_render_wgpu::backend::request_adapter_and_device(
            backend,
            &instance,
            Some(&surface),
            preferences.graphics_power_preference().into(),
        )) {
            Ok(found) => found,
            // Any GPU that can show the window, if the clients' choice can't.
            Err(e) => {
                tracing::warn!("Shell: no GPU as the clients choose theirs ({e}); taking any");
                let adapter = futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                }))?;
                let (device, queue) = futures::executor::block_on(adapter.request_device(&Default::default()))?;
                (adapter, device, queue)
            }
        };
    let info = adapter.get_info();
    crate::mugetsu::set_adapter_ids(info.vendor, info.device);
    tracing::info!(
        "Shell graphics: {} on {} (type: {:?})",
        info.backend.to_str(),
        info.name,
        info.device_type
    );
    let key = shell::adapter_key(info.backend.to_str(), info.vendor, info.device, &info.name);
    let caps = surface.get_capabilities(&adapter);
    // The pictures are BGRA: drawn as they are into a window that takes BGRA, else converted.
    let format = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm]
        .into_iter()
        .find(|format| caps.formats.contains(format))
        .or_else(|| caps.formats.first().copied())
        .ok_or_else(|| anyhow!("the window takes no picture format"))?;
    let size = window.inner_size();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        color_space: wgpu::SurfaceColorSpace::Auto,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes[0],
        view_formats: Vec::new(),
    };
    surface.configure(&device, &config);
    let blit = Blit::new(&device);
    let replace = blit.pipeline(&device, format);
    let over = blit.pipeline_blending(&device, format, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
    let gpu = Gpu {
        device,
        queue,
        surface,
        config,
        blit,
        replace,
        over,
    };
    Ok((gpu, key))
}

/// egui's name for a key (`egui::Key::name`), for the client.
fn egui_key_name(key: &Key) -> Option<&'static str> {
    const NAMED: [(NamedKey, &str); 27] = [
        (NamedKey::Enter, "Enter"),
        (NamedKey::Tab, "Tab"),
        (NamedKey::Backspace, "Backspace"),
        (NamedKey::Delete, "Delete"),
        (NamedKey::Escape, "Escape"),
        (NamedKey::Space, "Space"),
        (NamedKey::ArrowUp, "ArrowUp"),
        (NamedKey::ArrowDown, "ArrowDown"),
        (NamedKey::ArrowLeft, "ArrowLeft"),
        (NamedKey::ArrowRight, "ArrowRight"),
        (NamedKey::Home, "Home"),
        (NamedKey::End, "End"),
        (NamedKey::PageUp, "PageUp"),
        (NamedKey::PageDown, "PageDown"),
        (NamedKey::Insert, "Insert"),
        (NamedKey::F1, "F1"),
        (NamedKey::F2, "F2"),
        (NamedKey::F3, "F3"),
        (NamedKey::F4, "F4"),
        (NamedKey::F5, "F5"),
        (NamedKey::F6, "F6"),
        (NamedKey::F7, "F7"),
        (NamedKey::F8, "F8"),
        (NamedKey::F9, "F9"),
        (NamedKey::F10, "F10"),
        (NamedKey::F11, "F11"),
        (NamedKey::F12, "F12"),
    ];
    const LETTERS: [&str; 36] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V",
        "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
    ];
    match key {
        Key::Named(named) => NAMED.iter().find(|(key, _)| key == named).map(|(_, name)| *name),
        Key::Character(text) => {
            let upper = text.to_uppercase();
            LETTERS.iter().copied().find(|letter| *letter == upper)
        }
        _ => None,
    }
}

/// Shows `cursor` over the window (`ToShell::Cursor`).
fn set_cursor(window: &winit::window::Window, cursor: turu_frontend::shell::Cursor) {
    use turu_frontend::shell::Cursor;
    use winit::window::CursorIcon;
    let icon = match cursor {
        Cursor::None => {
            window.set_cursor_visible(false);
            return;
        }
        Cursor::Default => CursorIcon::Default,
        Cursor::Hand => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::Move => CursorIcon::Move,
        Cursor::NotAllowed => CursorIcon::NotAllowed,
        Cursor::Wait => CursorIcon::Wait,
        Cursor::ResizeHorizontal => CursorIcon::EwResize,
        Cursor::ResizeVertical => CursorIcon::NsResize,
        Cursor::ResizeNwSe => CursorIcon::NwseResize,
        Cursor::ResizeNeSw => CursorIcon::NeswResize,
    };
    window.set_cursor_visible(true);
    window.set_cursor(icon);
}

#[cfg(test)]
mod tests {
    use super::client_args;

    #[test]
    fn clients_get_the_shells_flags_but_its_servers() {
        let args = [
            "--shell",
            "--user",
            "Hero",
            "--api=127.0.0.1:9091",
            "--monitor",
            "--report-to",
            "https://monitor.example",
            "--run-script",
            "farm/Gold.ts",
            "--report-to=https://other.example",
            "--monitor=0.0.0.0:9021",
            "--api",
        ];
        let kept = client_args(args.iter().map(|arg| (*arg).to_owned()));
        assert_eq!(kept, ["--user", "Hero", "--run-script", "farm/Gold.ts"]);
    }
}
