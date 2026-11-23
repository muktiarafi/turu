//! Turu: the desktop shell's logic without a window (docs/NATIVE_UI_PROPOSAL.md, L1's shell part;
//! docs/TAB_PROCESSES_PROPOSAL.md, M2 and M5). The shell runs no game: each tab is a client process
//! (`turu --client=<address>`) that runs one game and bot and draws the whole tab, Turu's UI
//! included. The shell shows the pictures of the tab on screen, sends it the window's mouse, keys
//! and size, and tells every client the list of tabs, which their tab strips show; a strip's
//! click, close or "+" (Army > Accounts' "In a tab" too) comes back here.
//!
//! [`Shell`] is that logic, with no window: a front (`app/src/shell.rs`, winit; later a native
//! macOS app) owns the window and its event loop, hands the shell what happens there (its size,
//! focus, the pointer, keys as the link's messages, the [`Event`]s the shell's threads send through
//! its [`Sink`]), and carries out the [`Action`]s the shell asks of the window ([`Shell::actions`]):
//! the title, a redraw, the input method, the cursor, full screen, a file dialog, a graphics
//! choice, a link, quitting. The shell never calls the front from inside one of its methods.
//!
//! The first tab gets the shell's own startup flags (`--user`, `--run-script`, ...); a new one
//! those of its login. A client that dies is started again with the same flags after 1, 4, 16 and
//! then 60 s; after 5 deaths in 10 minutes its tab stays closed until selected, unless no other tab
//! is running (then it goes on trying every 60 s). When the tab on screen dies, a running one is
//! shown instead. A client quits when the shell does (its connection and stdin close).
//!
//! Pictures: a client draws into GPU surfaces it hands to the shell (IOSurfaces on macOS; Vulkan
//! memory or D3D12 resources on Linux and Windows: the front's [`SharedSurface`]s), and says which
//! one has its newest picture (`ToShell::Surface`); the shell draws from it on its own GPU
//! ([`Gpu`], chosen as the clients choose theirs, by the front), and gives each back
//! (`ToClient::Release`) once a newer one came and its own GPU work with the older one is done. A
//! client on another GPU (`ToShell::Gpu`), a surface the shell can't open, or
//! `TURU_NO_SHARED_SURFACES`: the client sends its pixels (`ToClient::NoSurfaces`), uploaded into a
//! texture here. Either way the window is drawn on the GPU, Grid View's tiles included.
//!
//! Testing without anyone at the window: `TURU_SHELL_TEST=<file>` runs its lines in order
//! ([`spawn_test`]; with `TURU_SHELL_TEST_FOLLOW=1`, lines appended to it later): `wait <seconds>`,
//! `open` (a tab at the login screen), `select <id>`, `close <id>`, `kill <id>` (its client, as a
//! crash would), `click <x> <y>` (in the window's pixels, as the mouse would; `rclick` with the
//! right button), `drop <path>` (a file dropped on the window), `gamepad <button>` (Ruffle's name,
//! pressed and released), `ime preedit <text>` / `ime commit <text>` (an input method's, as winit
//! gives it), `ime disabled` and `ime state` (the window's input method as applied, logged), `text
//! <string>` (typed), `key <key>` (a press and release, egui's key name after `cmd+`, `ctrl+`,
//! `shift+`, `alt+`; `keydown` / `keyup` the press or the release alone), `fullscreen on|off`,
//! `cover off|on` (the window counts as seen though covered, or not), `nosurfaces` (the next shared
//! surface fails to open), `rate` (each tab's pictures a second since the last `rate`) and `dump
//! <path>` (the window's picture as drawn, read back from the GPU, as a PNG). The front adds what
//! only a window has: `resize <width> <height>`, `minimize on|off`, `title` and `quit`.
//!
//! Frame interpolation in a tab (`ToShell::Pace`, `client_screen.rs`): while the tab on screen
//! asks for `Pace::Refresh`, the window is presented on every refresh of the display (presenting
//! waits for it, as the window's own interpolation does) and the tab is told after each
//! (`ToClient::Refreshed`); it draws its in-between pictures on those, so they keep in step with
//! the display, and each shows from the next refresh. `Pace::Uncapped` presents each picture when
//! it comes without waiting for the refresh. The display's rate goes to every tab
//! (`ToClient::RefreshRate`). `rate` also says, of the presents since the last, how many showed
//! no new picture, one, or more (one picture skipped).
//!
//! Grid View (the bar's Grid toggle, `ToShell::Grid`): the tab on screen draws the window with its
//! game in its tile, the others only their game at their tile's size; the shell puts the tiles
//! under the tab on screen's picture. A click on another tile goes to the tab on screen, which
//! asks for that tab (`SelectTab`) unless one of its windows covers the tile there.

use anyhow::{Context as _, Error, anyhow};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::monitor::hub::PeerSource;
use turu_frontend::blit::Blit;
use turu_frontend::gpu_waiter::GpuWaiter;
use turu_frontend::shell::{
    self, Answered, Button, Cursor, GridShow, GridTile, Mods, Pace, PointerKind, SHARED_POOL, TabInfo, ToClient,
    ToShell,
};
use turu_frontend::turu;

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
const RELEASE_POLL_PACED: Duration = Duration::from_millis(1);
/// How long a closing client has to write the game's saved data and end (`close_clients`).
const CLOSE_GRACE: Duration = Duration::from_secs(3);

/// A client's surface as the front got it: an IOSurface (macOS), Vulkan memory or a D3D12
/// resource (Windows, Linux), opened on the shell's GPU when it's drawn.
pub trait SharedSurface: Send + 'static {
    /// Its size in pixels.
    fn size(&self) -> (u32, u32);
    /// It as a texture of `device`, or `None` if it can't be opened there.
    fn texture(&mut self, device: &wgpu::Device) -> Option<wgpu::Texture>;
}

/// No shared surfaces (a system without them): the clients send pixels.
pub enum NoSurface {}

impl SharedSurface for NoSurface {
    fn size(&self) -> (u32, u32) {
        match *self {}
    }

    fn texture(&mut self, _device: &wgpu::Device) -> Option<wgpu::Texture> {
        match *self {}
    }
}

/// What the shell's threads (and the front's) hand to the event loop, for [`Shell::handle`].
pub enum Event<S> {
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
    SurfaceArrived { pid: u32, id: u64, surface: S },
    /// Turu's update state changed (a check, an install's progress): to every tab.
    Updates(turu_bot::updates::State),
    /// The shell's Army clients changed (launched, ended, closed): to every tab.
    ArmyClients(Vec<shell::ArmyClientInfo>),
    /// A file dialog a tab's client asked for was answered (tab, process id, its id, the path).
    Answered(u32, u32, u32, Option<String>),
    /// The shell's GPU is done with these surfaces (tab, process id, surface): back to their clients.
    Released(Vec<(u32, u32, u64)>),
}

/// Hands an [`Event`] to the front's event loop; false once the loop has gone (the threads end).
pub type Sink<S> = Arc<dyn Fn(Event<S>) -> bool + Send + Sync>;

/// A file dialog a tab asked for ([`Action::FileDialog`]): answered with [`Event::Answered`].
#[derive(Debug, Clone, PartialEq)]
pub struct FileDialog {
    pub tab: u32,
    pub pid: u32,
    pub request: u32,
    pub save: bool,
    pub title: Option<String>,
    pub directory: Option<String>,
    pub file_name: Option<String>,
    pub filters: Vec<(String, Vec<String>)>,
}

/// What the shell asks of the window, carried out by the front ([`Shell::actions`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// The window's title.
    Title(String),
    /// Draw again ([`Shell::draw`]) when the window can.
    Redraw,
    /// The window's input method: allowed or not, a password's or not, where the candidates go
    /// (in the window's pixels: x, y, width, height).
    Ime {
        allowed: bool,
        password: bool,
        area: Option<[f32; 4]>,
    },
    /// The cursor over the window (the tab on screen's).
    Cursor(Cursor),
    /// Full screen on the window's screen, or not. The shell hears that it changed from
    /// [`Shell::resized`].
    FullScreen(bool),
    /// A file dialog over the window; one at a time. The front answers with [`Event::Answered`],
    /// or [`Shell::dialog_closed`] if it couldn't show it.
    FileDialog(FileDialog),
    /// A tab's Options > Application > Graphics: the front asks, saves and starts the shell
    /// again (`value` is "" for Automatic), or calls [`Shell::dialog_closed`].
    ChooseGraphics { tab: u32, value: String },
    /// The only tab closed itself (`--exit-on-script-stop`): the window ends, as VibeSkua's
    /// process quits.
    Quit,
}

/// How the shell starts its clients.
pub struct Config {
    /// The client's program (`turu`), started with `--client=<address>` and the tab's flags.
    pub exe: PathBuf,
    /// Where clients hand over their surfaces (`shared_surface::listen` and its kin), or `None`:
    /// they send pixels.
    pub surfaces_name: Option<String>,
    /// The window's name, before the tab's label ("Turu 0.4.0 – Mugetsu's notes").
    pub name: Box<dyn Fn() -> String + Send>,
}

/// The newest picture from a client.
#[derive(Clone, Default)]
struct Picture {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// A client's surface, kept by the shell: opened on its GPU once it's drawn.
struct Kept<S> {
    id: u64,
    surface: S,
    opened: Option<(wgpu::Texture, wgpu::BindGroup)>,
}

/// What the shell draws of a tab: its newest picture as a texture of the shell's GPU.
struct Shown {
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
    /// The client's shared surface it is, given back when a newer one comes.
    surface: Option<u64>,
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
    /// The same connection, for the GPU waiter's thread (surfaces given back), and the lock every
    /// message to the client is written under (from either thread).
    release_out: Option<Arc<TcpStream>>,
    write_lock: Arc<Mutex<()>>,
    generation: u64,
    picture: Arc<Mutex<Picture>>,
    label: String,
    busy: bool,
    deaths: Vec<Instant>,
    /// When to start the client again.
    restart_at: Option<Instant>,
    /// Pictures received since `TURU_SHELL_TEST`'s last `rate`, and since the client started.
    pictures: u32,
    received: u64,
    /// How its client wants its pictures presented (frame interpolation), until it starts again.
    pace: Pace,
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
    /// The input method its client last asked for (`ToShell::Ime`: allowed, a password's, the
    /// text's area), applied to the window while it's the tab on screen.
    ime: (bool, bool, Option<[f32; 4]>),
    /// The last picture size too large for the shell's GPU, said once.
    too_large: Option<(u32, u32)>,
}

impl Tab {
    fn new(id: u32, args: Vec<String>, password: Option<String>) -> Self {
        Self {
            id,
            args,
            password,
            child: None,
            out: None,
            release_out: None,
            write_lock: Arc::default(),
            generation: 0,
            picture: Arc::default(),
            label: "Login".into(),
            busy: false,
            deaths: Vec::new(),
            restart_at: None,
            pictures: 0,
            received: 0,
            pace: Pace::Frames,
            closed: false,
            started: Instant::now(),
            api: None,
            api_token: turu_bot::control_api::new_token().unwrap_or_default(),
            shown: None,
            upload: None,
            waiting: None,
            adapter: None,
            pixels_only: false,
            ime: (false, false, None),
            too_large: None,
        }
    }

    /// Whether a picture of `size` fits a texture of `gpu`'s. A client on a stronger GPU can draw
    /// one this GPU can't hold (the shell falls back to any GPU that shows the window), and wgpu
    /// panics on a texture over its limit: such a picture isn't shown, and said once per size.
    fn fits(&mut self, gpu: &Gpu, size: (u32, u32)) -> bool {
        let max = gpu.device.limits().max_texture_dimension_2d;
        if size.0 <= max && size.1 <= max {
            return true;
        }
        if self.too_large != Some(size) {
            self.too_large = Some(size);
            tracing::warn!(
                "Shell: tab {}'s {}x{} picture is larger than this GPU's {max}: not shown",
                self.id,
                size.0,
                size.1
            );
        }
        false
    }

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

/// A tab as the front and the tests see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabState {
    pub id: u32,
    pub label: String,
    pub busy: bool,
    /// Why its client isn't connected ("starting", "restarting", "closed"), or `None`.
    pub down: Option<String>,
    /// Its client's process id, while one runs.
    pub pid: Option<u32>,
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

/// The shell's GPU: the window's surface on it, and what draws the clients' pictures into it. The
/// front chooses the adapter and opens the device as the clients choose theirs (so both sides are
/// on the same GPU and backend and can share surfaces).
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Gives the clients' surfaces back once the GPU is done with them (`release_retiring`).
    waiter: Option<GpuWaiter>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    blit: Blit,
    /// Draws a picture as it is, and over what's there with its (premultiplied) alpha.
    replace: wgpu::RenderPipeline,
    over: wgpu::RenderPipeline,
}

impl Gpu {
    /// Configures the window's `surface` (`size` in pixels) on `device` and makes what draws into
    /// it.
    pub fn new(
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        size: (u32, u32),
    ) -> Result<Self, Error> {
        let caps = surface.get_capabilities(adapter);
        // The pictures are BGRA: drawn as they are into a window that takes BGRA, else converted.
        let format = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm]
            .into_iter()
            .find(|format| caps.formats.contains(format))
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| anyhow!("the window takes no picture format"))?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.0.max(1),
            height: size.1.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: Vec::new(),
        };
        surface.configure(&device, &config);
        let blit = Blit::new(&device);
        let replace = blit.pipeline(&device, format);
        let over = blit.pipeline_blending(&device, format, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let waiter = GpuWaiter::new(device.clone(), "Turu shell GPU waiter")
            .inspect_err(|e| tracing::warn!("Shell: couldn't start the GPU waiter ({e}): surfaces go back when polled"))
            .ok();
        Ok(Self {
            device,
            queue,
            waiter,
            surface,
            config,
            blit,
            replace,
            over,
        })
    }
}

/// The desktop shell: the tabs' clients, their pictures and the window's state, without the
/// window.
pub struct Shell<S: SharedSurface> {
    config: Config,
    /// The tabs' control APIs, for the monitor hub (`turu::monitor_hub`).
    peers: Arc<Mutex<Vec<PeerSource>>>,
    address: String,
    token: String,
    sink: Sink<S>,
    tabs: Vec<Tab>,
    /// The tab on screen.
    shown: u32,
    next_id: u32,
    gpu: Option<Gpu>,
    /// The front has a window (titles, the input method and redraws reach it).
    window: bool,
    actions: Vec<Action>,
    /// The window's size in pixels and its scale.
    size: Option<(u32, u32, f64)>,
    title: String,
    /// Grid View, on from a client's bar: what Turu's UI takes of the window (left, top, in points).
    grid: Option<(f32, f32)>,
    /// Headless Mode as a client last turned it on or off (`None`: none has yet): every tab's.
    headless_mode: Option<bool>,
    /// The window is full screen, as the tabs were last told (`ToClient::FullScreen`).
    full_screen: bool,
    /// The clients' surfaces, by process id: the newest [`SHARED_POOL`] of each.
    surfaces: HashMap<u32, Vec<Kept<S>>>,
    /// Surfaces the shell is done with once its GPU has done what's submitted (tab, pid, id).
    retiring: Vec<(u32, u32, u64)>,
    /// Releases waiting on the GPU.
    releases_pending: usize,
    /// A file dialog is open (one at a time, as the window's own).
    dialog_open: bool,
    /// When `TURU_SHELL_TEST`'s last `rate` was.
    rate_since: Instant,
    /// How the window is presented: the tab on screen's pace while it's seen (`apply_pace`).
    pace: Pace,
    /// The display's refresh rate (mHz, 0 before the first look).
    refresh_millihertz: u32,
    /// When the last draw started (pacing the draws on every refresh), and the tab on screen's
    /// pictures received by then.
    draw_started: Instant,
    drawn_received: u64,
    /// Presents since `rate` with no new picture of the tab on screen, one, or more; and draws
    /// that got no picture of the window to draw into (timed out, lost).
    presents: [u32; 3],
    no_frame: u32,
    /// The last draw got no picture of the window (`no_frame`): the next waits longer.
    draw_failed: bool,
    /// The window is covered or minimized: the clients draw nothing (`ToClient::Covered`).
    covered: bool,
    occluded: bool,
    minimized: bool,
    /// The window's GPU (`shell::adapter_key`), set with it.
    adapter: String,
    /// `TURU_SHELL_TEST`'s `nosurfaces`: the next surface is taken as one that couldn't be opened.
    fail_surfaces: bool,
    /// `TURU_SHELL_TEST`'s `cover off`: the window counts as seen even when it's covered (a test
    /// behind other windows).
    never_covered: bool,
    /// The input method last applied to the window (`apply_ime`), and whether it has the focus.
    ime_applied: Option<(bool, bool, Option<[f32; 4]>)>,
    focused: bool,
}

impl<S: SharedSurface> Shell<S> {
    /// Listens for the clients on loopback; their connections come to `sink`. Opens no tab.
    pub fn new(config: Config, sink: Sink<S>) -> Result<Self, Error> {
        let listener = TcpListener::bind("127.0.0.1:0").context("Couldn't listen for the clients")?;
        let address = listener.local_addr()?.to_string();
        let token = turu_bot::control_api::new_token().map_err(|e| anyhow!(e))?;
        spawn_acceptor(listener, token.clone(), sink.clone());
        Ok(Self {
            config,
            peers: Arc::default(),
            address,
            token,
            sink,
            tabs: Vec::new(),
            shown: 0,
            next_id: 1,
            gpu: None,
            window: false,
            actions: Vec::new(),
            size: None,
            title: String::new(),
            grid: None,
            headless_mode: None,
            full_screen: false,
            surfaces: HashMap::new(),
            retiring: Vec::new(),
            releases_pending: 0,
            dialog_open: false,
            rate_since: Instant::now(),
            pace: Pace::Frames,
            refresh_millihertz: 0,
            draw_started: Instant::now(),
            drawn_received: 0,
            presents: [0; 3],
            no_frame: 0,
            draw_failed: false,
            covered: false,
            occluded: false,
            minimized: false,
            adapter: String::new(),
            fail_surfaces: false,
            never_covered: false,
            ime_applied: None,
            focused: true,
        })
    }

    /// What the shell runs for every tab besides their clients: the settings file followed, the
    /// launch and Turu's updates (to the tabs), the Army's launched clients (to the tabs), and
    /// the monitor hub's and `--api`'s peers (the tabs' control APIs). Not in the tests.
    pub fn start_services(&self) {
        // Options > Application lives in the clients, which leave the web monitor and Report To here.
        turu::follow_settings_file(true);
        // The clients leave the launch updates (scripts, skill sets) to the shell, and Turu's own
        // update checks (`main.rs`); what the checks find goes to them to show.
        turu::start_app_updates();
        watch_updates(self.sink.clone());
        // The tabs' processes of their own (Army > Accounts) are launched here: the list to them.
        watch_army(self.sink.clone());
        let listed = self.peers.clone();
        turu::monitor_hub().set_peers(Arc::new(move || {
            listed.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }));
        // `--api`: the same routes over the tabs, passed on to each tab's client's control API.
        let listed = self.peers.clone();
        turu::start_shell_api(Arc::new(move || {
            listed.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }));
    }

    /// What the shell asks of the window since the last call, in order.
    pub fn actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    /// Asks for a redraw, once per batch of actions.
    fn redraw(&mut self) {
        if self.window && !self.actions.contains(&Action::Redraw) {
            self.actions.push(Action::Redraw);
        }
    }

    /// The tabs, in order.
    pub fn tabs(&self) -> Vec<TabState> {
        self.tabs
            .iter()
            .map(|tab| TabState {
                id: tab.id,
                label: tab.label.clone(),
                busy: tab.busy,
                down: tab.down(),
                pid: tab.pid(),
            })
            .collect()
    }

    /// The tab on screen.
    pub fn shown(&self) -> u32 {
        self.shown
    }

    /// Tab `id`'s client process, while it runs (Windows' shared handles are copied from it).
    pub fn child(&self, id: u32) -> Option<&Child> {
        self.tabs.iter().find(|tab| tab.id == id)?.child.as_ref()
    }

    fn tab(&mut self, id: u32) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    /// The front has made its window: the title, the input method and redraws reach it now.
    pub fn window_opened(&mut self) {
        self.window = true;
        self.update_title();
    }

    /// The shell's GPU, made by the front for its window, with its key (`shell::adapter_key`).
    pub fn set_gpu(&mut self, gpu: Gpu, adapter: String) {
        self.gpu = Some(gpu);
        self.adapter = adapter;
    }

    /// The window's title as it would be with `label` shown: "Turu 0.3.1 – <Mugetsu's notes> – <the
    /// shown tab> (<n> tabs)".
    pub fn title_for(&self, label: Option<String>) -> String {
        let name = (self.config.name)();
        match label {
            Some(label) if self.tabs.len() > 1 => format!("{name} – {label} ({} tabs)", self.tabs.len()),
            Some(label) => format!("{name} – {label}"),
            None => name,
        }
    }

    /// The window's title is looked at again (its name may have changed: "DirectX 12 not
    /// available").
    pub fn update_title(&mut self) {
        let label = self
            .tabs
            .iter()
            .find(|tab| tab.id == self.shown)
            .map(|tab| tab.label.clone());
        let title = self.title_for(label);
        if title != self.title && self.window {
            self.actions.push(Action::Title(title.clone()));
            self.title = title;
        }
    }

    /// Hands an event from the shell's threads to the shell.
    pub fn handle(&mut self, event: Event<S>) {
        match event {
            Event::Connected { pid, stream } => self.connected(pid, stream),
            Event::Picture(id) => self.pixel_picture(id),
            Event::SurfaceArrived { pid, id, surface } => self.surface_arrived(pid, id, surface),
            Event::Released(retired) => self.released(retired),
            Event::ArmyClients(clients) => {
                let message = ToClient::ArmyClients { clients };
                for tab in &mut self.tabs {
                    send(tab, &message);
                }
            }
            Event::Updates(state) => {
                let message = ToClient::Updates {
                    updates: Box::new(shell::UpdateInfo::from_state(&state)),
                };
                for tab in &mut self.tabs {
                    send(tab, &message);
                }
            }
            Event::Answered(id, pid, request, value) => {
                self.dialog_open = false;
                if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id && tab.pid() == Some(pid)) {
                    send(
                        tab,
                        &ToClient::Answer {
                            id: request,
                            answer: Answered::Path { path: value },
                        },
                    );
                }
            }
            Event::Message(id, message) => self.message(id, message),
            Event::Gone(id, generation) => self.gone(id, generation),
            Event::Test(line) => self.test(&line),
        }
    }

    /// Opens a tab with these flags and shows it.
    pub fn open_tab(&mut self, args: Vec<String>, password: Option<String>) {
        let id = self.next_id;
        self.next_id += 1;
        let mut tab = Tab::new(id, args, password);
        self.start(&mut tab);
        self.tabs.push(tab);
        self.select(id);
    }

    /// Starts the tab's client process; it says hello by itself.
    fn start(&self, tab: &mut Tab) {
        // Its control API on a port nobody listens on (it binds it a moment later), as Army >
        // Accounts' clients get.
        tab.api = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).ok();
        let mut command = Command::new(&self.config.exe);
        command
            .arg(format!("--client={}", self.address))
            .arg("--exit-with-parent")
            .args(&tab.args)
            .env(shell::TOKEN_ENV, &self.token)
            .env_remove("TURU_PASSWORD")
            .env_remove(shell::SURFACES_ENV)
            .stdin(Stdio::piped());
        if let Some(name) = &self.config.surfaces_name {
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
                self.schedule_restart(tab, Instant::now());
            }
        }
    }

    /// After a client died at `now`: when to start it again, or give up.
    fn schedule_restart(&self, tab: &mut Tab, now: Instant) {
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
        self.redraw();
    }

    /// A strip's click on tab `id`: shown if its client runs; if it's down, a closed one starts
    /// again, and the tab on screen stays until it connects (a tab without a client takes no input).
    pub fn select_requested(&mut self, id: u32) {
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
    pub fn close(&mut self, id: u32) {
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
        // Its client writes the game's saved data and ends on a thread of its own: the window
        // doesn't wait.
        let children = close_clients([&mut tab]);
        let spawned = std::thread::Builder::new()
            .name("Turu Tab Close".into())
            .spawn(move || end_clients(children));
        if let Err(e) = spawned {
            tracing::warn!("Shell: couldn't wait for tab {id}'s client: {e}");
        }
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
    pub fn grid_tiles(&self) -> Option<Vec<GridTile>> {
        let (left, top) = self.grid?;
        let (width, height, scale) = self.size?;
        if self.tabs.len() < 2 {
            return None;
        }
        let left = (f64::from(left) * scale).round() as u32;
        let top = (f64::from(top) * scale).round() as u32;
        let area = (width.saturating_sub(left), height.saturating_sub(top));
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
        let Some((width, height, scale)) = self.size else {
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
                        width,
                        height,
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
        self.apply_ime();
    }

    /// The window takes an input method as the tab on screen last asked (`ToShell::Ime`), while it
    /// has the focus: allowed or not, a password's or not, and where the candidates go.
    fn apply_ime(&mut self) {
        let wanted = match self.tabs.iter().find(|tab| tab.id == self.shown) {
            Some(tab) if self.focused && tab.out.is_some() => tab.ime,
            _ => (false, false, None),
        };
        if self.ime_applied == Some(wanted) || !self.window {
            return;
        }
        self.ime_applied = Some(wanted);
        let (allowed, password, area) = wanted;
        self.actions.push(Action::Ime {
            allowed,
            password,
            area,
        });
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
        self.redraw();
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
                user: turu_frontend::slots::flag_value(&tab.args, "--user").map(str::to_owned),
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
                rail_top: None,
                native_rail: false,
            };
            send(tab, &message);
        }
        self.update_title();
    }

    fn connected(&mut self, pid: u32, stream: TcpStream) {
        let sink = self.sink.clone();
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
        spawn_reader(tab.id, tab.generation, reader, tab.picture.clone(), sink);
        // A client that stops reading (suspended, swapped out) would otherwise hold the window's
        // thread once its socket's buffer is full; a write that times out ends its connection.
        let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
        tab.release_out = stream.try_clone().ok().map(Arc::new);
        tab.out = Some(stream);
        tab.pace = Pace::Frames;
        if self.refresh_millihertz > 0 {
            send(
                tab,
                &ToClient::RefreshRate {
                    millihertz: self.refresh_millihertz,
                },
            );
        }
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
        tab.pace = Pace::Frames;
        tab.ime = (false, false, None);
        stop(&mut tab);
        self.schedule_restart(&mut tab, Instant::now());
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
            // Turu: a tab asking to close itself is `--exit-on-script-stop`'s quit (`exit_after_stop`):
            // as the only tab, the window ends with it (Army's "In its own window" launches, which
            // Army Queue waits on). Restarting it instead would run the script again.
            ToShell::CloseTab { id: target } if target == id && self.tabs.len() == 1 => {
                tracing::info!("Shell: the only tab closed itself; quitting");
                self.actions.push(Action::Quit);
            }
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
                exit_on_stop,
            } => {
                let mut args = turu_frontend::slots::open_args(username, server, script, run_script);
                args.extend(turu_frontend::slots::playlist_args(playlist, run_playlist, options_storage));
                args.extend(turu_frontend::slots::exit_args(exit_on_stop));
                self.open_tab(args, password);
            }
            ToShell::Status { label, busy, .. } => {
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
            ToShell::FullScreen { on } => self.actions.push(Action::FullScreen(on)),
            // The cursor of the tab on screen (another's would be stale by the time it shows).
            ToShell::Ime {
                allowed,
                password,
                area,
            } => {
                if let Some(tab) = self.tab(id) {
                    tab.ime = (allowed, password, area);
                }
                self.apply_ime();
            }
            ToShell::Pace { pace } => {
                if let Some(tab) = self.tab(id) {
                    tab.pace = pace;
                }
                self.apply_pace();
            }
            ToShell::Cursor { cursor } => {
                if id == self.shown && self.window {
                    self.actions.push(Action::Cursor(cursor));
                }
            }
            ToShell::ChooseGraphics { graphics } => {
                self.actions.push(Action::ChooseGraphics {
                    tab: id,
                    value: graphics,
                });
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
            ToShell::FileDialog {
                id: request,
                save,
                title,
                directory,
                file_name,
                filters,
            } => self.file_dialog(FileDialog {
                tab: id,
                pid: 0,
                request,
                save,
                title,
                directory,
                file_name,
                filters,
            }),
            ToShell::OpenUrl { url } => {
                if let Err(e) = webbrowser::open(&url) {
                    tracing::warn!("Couldn't open {url}: {e}");
                }
            }
            // Windows' shared handles are copied from the client's process by the front
            // (`shared_gpu::take_handle`), which hands the surface to `surface_arrived`.
            ToShell::SharedHandle { .. }
            // The text-field messages are Android's (its slots); a desktop client sends keys.
            | ToShell::Frame { .. }
            | ToShell::Hello { .. }
            | ToShell::FieldShow { .. }
            | ToShell::FieldUpdate { .. }
            | ToShell::FieldHide { .. }
            | ToShell::PickFile { .. }
            | ToShell::SaveFile { .. }
            | ToShell::Share { .. }
            | ToShell::Install { .. }
            | ToShell::SoftKeyboard { .. }
            | ToShell::PreferRefreshRate { .. }
            // Android's: the desktop's clients keep their own Accounts window.
            | ToShell::OpenScreen { .. }
            | ToShell::Unknown => {}
        }
    }

    /// A tab's file dialog, shown over the window by the front (made on the event loop's thread,
    /// as macOS needs) and answered once it's closed; one at a time.
    fn file_dialog(&mut self, mut dialog: FileDialog) {
        let Some(pid) = self.tab(dialog.tab).and_then(|tab| tab.pid()) else {
            return;
        };
        if self.dialog_open {
            if let Some(tab) = self.tab(dialog.tab) {
                send(
                    tab,
                    &ToClient::Answer {
                        id: dialog.request,
                        answer: Answered::Path { path: None },
                    },
                );
            }
            return;
        }
        dialog.pid = pid;
        self.dialog_open = true;
        self.actions.push(Action::FileDialog(dialog));
    }

    /// Whether a dialog is open over the window (one at a time).
    pub fn dialog_open(&self) -> bool {
        self.dialog_open
    }

    /// The front shows a dialog of its own over the window (the graphics choice): no other until
    /// [`Shell::dialog_closed`].
    pub fn dialog_opened(&mut self) {
        self.dialog_open = true;
    }

    /// A dialog the front showed (or couldn't show) is closed; file dialogs close by their
    /// [`Event::Answered`].
    pub fn dialog_closed(&mut self) {
        self.dialog_open = false;
    }

    /// Starts the clients whose restart is due at `now`; when to look again.
    fn restart_due(&mut self, now: Instant) -> Option<Instant> {
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
                self.schedule_restart(&mut tab, now);
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

    /// The window is covered by others, or not (winit reports it on macOS and X11 only).
    pub fn occluded(&mut self, occluded: bool) {
        self.occluded = occluded;
        self.set_covered(occluded || self.minimized);
    }

    /// The window has the focus, or lost it.
    pub fn focused(&mut self, focused: bool) {
        self.focused = focused;
        self.apply_ime();
        self.send_shown(&ToClient::Focus { focused });
    }

    /// Whether the window is full screen, as the tabs were last told.
    pub fn full_screen(&self) -> bool {
        self.full_screen
    }

    /// The window's size (in pixels, 0 by 0 while minimized), its scale, and whether it is full
    /// screen: the GPU surface, the tab on screen and the tabs' Options follow.
    pub fn resized(&mut self, width: u32, height: u32, scale: f64, full_screen: bool) {
        // Tells the tabs when the window went full screen or left it (also by the system's own
        // button), for Options.
        if full_screen != self.full_screen {
            tracing::info!(
                "Shell: the window {} full screen",
                if full_screen { "went" } else { "left" }
            );
            self.full_screen = full_screen;
            for tab in &mut self.tabs {
                send(tab, &ToClient::FullScreen { on: full_screen });
            }
        }
        // How minimizing shows on Windows (macOS reports it as occluded).
        let minimized = width == 0 || height == 0;
        if minimized != self.minimized {
            self.minimized = minimized;
            self.set_covered(minimized || self.occluded);
        }
        if minimized || self.size == Some((width, height, scale)) {
            return;
        }
        self.size = Some((width, height, scale));
        if let Some(gpu) = &mut self.gpu {
            gpu.config.width = width;
            gpu.config.height = height;
            gpu.surface.configure(&gpu.device, &gpu.config);
        }
        self.show_current();
    }

    /// Sends to the tab on screen.
    pub fn send_shown(&mut self, message: &ToClient) {
        let id = self.shown;
        if let Some(tab) = self.tab(id) {
            send(tab, message);
        }
    }

    /// A press, release or move of the mouse at (x, y) in the window's pixels: to the tab on screen
    /// (in Grid View, the active tab asks for another tab whose tile was clicked).
    pub fn pointer(&mut self, kind: PointerKind, button: Button, x: f32, y: f32) {
        self.send_shown(&ToClient::Pointer { kind, button, x, y });
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

    /// The client of process `pid` handed over `surface`.
    pub fn surface_arrived(&mut self, pid: u32, id: u64, surface: S) {
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
            tab.received += 1;
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
        let mut oversized = false;
        let tab = &mut self.tabs[index];
        let opened = match (&self.gpu, self.surfaces.get_mut(&pid)) {
            (Some(gpu), Some(kept)) => kept.iter_mut().find(|kept| kept.id == surface).and_then(|kept| {
                if kept.opened.is_none() && !tab.fits(gpu, kept.surface.size()) {
                    oversized = true;
                    return None;
                }
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
        // Over the GPU's limit: not opened, and given back (as a picture over it isn't shown).
        if oversized {
            self.retiring.push((id, pid, surface));
            return;
        }
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
        // Presented on every refresh anyway: drawn then, which keeps the rhythm.
        if self.pace != Pace::Refresh {
            self.redraw();
        }
    }

    /// Tab `id`'s client can't use shared surfaces (`reason`): it goes back to pixels.
    pub fn no_surfaces_for(&mut self, id: u32, reason: &str) {
        if let Some(index) = self.tabs.iter().position(|tab| tab.id == id)
            && let Some(pid) = self.tabs[index].pid()
        {
            self.no_surfaces(index, pid, reason);
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
        tab.received += 1;
        let picture = std::mem::take(&mut *tab.picture.lock().unwrap_or_else(|e| e.into_inner()));
        let size = (picture.width, picture.height);
        if picture.pixels.len() < size.0 as usize * size.1 as usize * 4 || size.0 == 0 || size.1 == 0 {
            return;
        }
        if !tab.fits(gpu, size) {
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
        if self.visible(id) && self.pace != Pace::Refresh {
            self.redraw();
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
        // Turu: from the GPU waiter's thread, written straight to the clients: the main thread
        // spends most of a refresh waiting for the display's next picture, and a surface it
        // polled for and sent back from there came back about 4 refreshes after it went, so the
        // clients ran out of surfaces at 120 Hz and more.
        if let Some(waiter) = &gpu.waiter {
            let releases: Vec<_> = retired
                .into_iter()
                .filter_map(|(id, pid, surface)| {
                    let tab = self.tabs.iter().find(|tab| tab.id == id && tab.pid() == Some(pid))?;
                    Some((tab.id, tab.release_out.clone()?, tab.write_lock.clone(), surface))
                })
                .collect();
            if releases.is_empty() {
                return;
            }
            let index = gpu.queue.submit([]);
            waiter.after(index, move || {
                for (id, out, lock, surface) in releases {
                    let _writing = lock.lock().unwrap_or_else(|e| e.into_inner());
                    write_or_drop(id, &out, &ToClient::Release { id: surface });
                }
            });
            return;
        }
        let sink = self.sink.clone();
        gpu.queue.submit([]);
        gpu.queue.on_submitted_work_done(move || {
            let _ = sink(Event::Released(retired));
        });
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

    /// Each tab's new pictures a second since the last call, and the window's presents a second
    /// (with no new picture, one, or more: a picture skipped): `TURU_SHELL_TEST`'s `rate`, and
    /// with `TURU_PERF_LOG` every period (as the window's "drawn, presented").
    fn log_rates(&mut self, prefix: &str) {
        let elapsed = self.rate_since.elapsed().as_secs_f64().max(0.001);
        self.rate_since = Instant::now();
        for tab in &mut self.tabs {
            let rate = f64::from(std::mem::take(&mut tab.pictures)) / elapsed;
            tracing::info!("{prefix}: tab {} ({}): {rate:.1} pictures a second", tab.id, tab.label);
        }
        let [none, one, more] = std::mem::take(&mut self.presents);
        tracing::info!(
            "{prefix}: {:.1} presents a second ({:?}): {none} with no new picture, {one} with one, {more} with more; {} draws without a picture of the window",
            f64::from(none + one + more) / elapsed,
            self.pace,
            std::mem::take(&mut self.no_frame)
        );
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

    /// Draws and presents the window (on the front's redraw); whether a picture was presented.
    pub fn draw(&mut self) -> bool {
        self.draw_started = Instant::now();
        let Some(gpu) = &self.gpu else {
            return false;
        };
        if self
            .tabs
            .iter()
            .find(|tab| tab.id == self.shown)
            .is_none_or(|tab| tab.shown.is_none())
        {
            return false;
        }
        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                self.no_frame += 1;
                self.draw_failed = true;
                return false;
            }
            _ => {
                self.no_frame += 1;
                self.draw_failed = true;
                return false;
            }
        };
        self.draw_failed = false;
        let view = frame.texture.create_view(&Default::default());
        let size = (gpu.config.width, gpu.config.height);
        if let Some(commands) = self.render(gpu, &view, size) {
            gpu.queue.submit([commands]);
        }
        gpu.queue.present(frame);
        let pace = self.pace;
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == self.shown) else {
            return true;
        };
        let new = tab.received - self.drawn_received.min(tab.received);
        self.drawn_received = tab.received;
        self.presents[(new as usize).min(2)] += 1;
        if pace == Pace::Refresh {
            send(tab, &ToClient::Refreshed);
        }
        true
    }

    /// How the window is presented, from the tab on screen's pace while the window is seen: on
    /// every refresh (`Refresh`, drawn from `tick`), or when a picture comes, waiting for
    /// the refresh or (`Uncapped`) not.
    fn apply_pace(&mut self) {
        let pace = match self.tabs.iter().find(|tab| tab.id == self.shown) {
            Some(tab) if !self.covered && tab.out.is_some() => tab.pace,
            _ => Pace::Frames,
        };
        let pace = if pace == Pace::Unknown { Pace::Frames } else { pace };
        if pace == self.pace {
            return;
        }
        let uncapped = pace == Pace::Uncapped;
        if uncapped != (self.pace == Pace::Uncapped)
            && let Some(gpu) = &mut self.gpu
        {
            gpu.config.present_mode = if uncapped {
                wgpu::PresentMode::AutoNoVsync
            } else {
                wgpu::PresentMode::AutoVsync
            };
            gpu.surface.configure(&gpu.device, &gpu.config);
        }
        tracing::info!(target: "ruffle_aqw_perf", "Shell: presenting {pace:?} (tab {})", self.shown);
        self.pace = pace;
        if let Some(tab) = self.tabs.iter().find(|tab| tab.id == self.shown) {
            self.drawn_received = tab.received;
        }
        self.redraw();
    }

    /// The display's refresh rate in mHz (the front looks it up every couple of seconds: the
    /// window may move to another monitor); told to every tab when it changes (frame
    /// interpolation's pacing).
    pub fn set_refresh_rate(&mut self, millihertz: u32) {
        let millihertz = millihertz.clamp(24_000, 360_000);
        if millihertz == self.refresh_millihertz {
            return;
        }
        self.refresh_millihertz = millihertz;
        tracing::info!(
            target: "ruffle_aqw_perf",
            "Shell: the display refreshes at {:.1} Hz",
            f64::from(millihertz) / 1000.0
        );
        for tab in &mut self.tabs {
            send(tab, &ToClient::RefreshRate { millihertz });
        }
    }

    /// Before the front's loop waits: surfaces given back, the perf log, the pace, clients due to
    /// start again, the next draw on every refresh. When to wake at the latest (`None`: when
    /// something comes).
    pub fn tick(&mut self, now: Instant) -> Option<Instant> {
        self.release_retiring();
        if let Some(every) = turu::perf_log_every()
            && self.rate_since.elapsed() >= every
        {
            self.log_rates("Shell");
        }
        self.apply_pace();
        let mut wake = self.restart_due(now);
        if self.pace == Pace::Refresh {
            // Asked for half a refresh after the last draw, as the window's interpolation does:
            // the next present waits for its refresh, and the draw is ready in time for it.
            // Without a picture of the window last time (macOS gives none while the window is
            // hidden from view), every 4th refresh only.
            let refresh = Duration::from_secs_f64(1000.0 / f64::from(self.refresh_millihertz.max(24_000)));
            let wait = if self.draw_failed { refresh * 4 } else { refresh / 2 };
            let mut due = self.draw_started + wait;
            if Instant::now() >= due {
                self.redraw();
                due = Instant::now() + wait;
            }
            wake = Some(wake.map_or(due, |at| at.min(due)));
        }
        if self.releases_pending > 0
            && let Some(gpu) = &self.gpu
        {
            // Runs the GPU's callbacks: the releases come as events.
            let _ = gpu.device.poll(wgpu::PollType::Poll);
            // On every refresh a picture comes and one goes back: sooner, or the client waits.
            let poll = Instant::now()
                + if self.pace == Pace::Refresh {
                    RELEASE_POLL_PACED
                } else {
                    RELEASE_POLL
                };
            wake = Some(wake.map_or(poll, |at| at.min(poll)));
        }
        wake
    }

    /// `TURU_SHELL_TEST`'s `dump`: the window drawn into a texture and read back, as a PNG.
    fn dump(&self, path: &str) {
        let (Some(gpu), Some((width, height, _))) = (&self.gpu, self.size) else {
            return;
        };
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

    /// A line of `TURU_SHELL_TEST`'s file (the front takes the window's own first).
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
        // `key`, and `keydown` / `keyup` (held between lines).
        if command == "key" || command == "keydown" || command == "keyup" {
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
            let presses: &[bool] = match command {
                "keydown" => &[true],
                "keyup" => &[false],
                _ => &[true, false],
            };
            for &pressed in presses {
                self.send_shown(&ToClient::Key {
                    key: key.clone(),
                    pressed,
                    mods,
                    // The client finds it from the name, as for a US keyboard.
                    game: None,
                });
            }
            return;
        }
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
            self.log_rates("Shell test");
            return;
        }
        // `fullscreen on` / `fullscreen off`: as a tab's Options > Application > Full Screen.
        if command == "fullscreen" {
            self.actions.push(Action::FullScreen(arg.trim() == "on"));
            return;
        }
        // `click` and `rclick` (the right button).
        if command == "click" || command == "rclick" {
            let at: Vec<f32> = arg.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            let button = if command == "rclick" {
                Button::Right
            } else {
                Button::Left
            };
            if let [x, y] = at[..] {
                for kind in [PointerKind::Move, PointerKind::Down, PointerKind::Up] {
                    self.pointer(kind, button, x, y);
                }
            }
            return;
        }
        // `drop <path>`: a file dropped on the window.
        if command == "drop" {
            self.send_shown(&ToClient::Dropped { path: arg.to_owned() });
            return;
        }
        // `gamepad <button>` (Ruffle's name: south, dpad-up, ...): pressed and released.
        if command == "gamepad" {
            for pressed in [true, false] {
                self.send_shown(&ToClient::Gamepad {
                    button: arg.to_owned(),
                    pressed,
                });
            }
            return;
        }
        // `ime preedit <text>`, `ime commit <text>`, `ime state` (the window's IME as applied): an
        // input method, as winit gives it.
        if command == "ime" {
            let (kind, text) = arg.split_once(' ').unwrap_or((arg, ""));
            let ime = match kind {
                "preedit" => shell::ImeInput::Preedit {
                    text: text.to_owned(),
                    cursor: Some((text.len(), text.len())),
                },
                "commit" => shell::ImeInput::Commit { text: text.to_owned() },
                "disabled" => shell::ImeInput::Disabled,
                _ => {
                    tracing::info!("Shell test: IME applied: {:?}", self.ime_applied);
                    return;
                }
            };
            self.send_shown(&ToClient::Ime { ime });
            return;
        }
        match (command, id) {
            ("open", _) => self.open_tab(Vec::new(), None),
            ("select", Some(id)) => self.select_requested(id),
            ("close", Some(id)) => self.close(id),
            ("kill", Some(id)) => self.kill(id),
            ("dump", _) => self.dump(arg.trim()),
            _ => tracing::warn!("Shell test: what's \"{line}\"?"),
        }
    }

    /// Kills tab `id`'s client, as a crash would (`TURU_SHELL_TEST`'s `kill`).
    pub fn kill(&mut self, id: u32) {
        if let Some(child) = self.tab(id).and_then(|tab| tab.child.as_mut()) {
            let _ = child.kill();
        }
    }

    /// The window is closing: the clients write the game's saved data and end together
    /// ([`CLOSE_GRACE`] at most), then the GPU, and the clients' pictures opened on it, are let go
    /// of, while the driver is alive (Mugetsu 2.0.0: on DirectX 12 a device dropped later in the
    /// shutdown crashed). The front flushes wgpu's last command buffers after.
    pub fn exit(&mut self) {
        end_clients(close_clients(self.tabs.iter_mut()));
        for tab in &mut self.tabs {
            tab.shown = None;
            tab.upload = None;
        }
        self.surfaces.clear();
        self.gpu = None;
    }

    /// Stops every client still running at once (after the window's loop has ended).
    pub fn stop_all(&mut self) {
        for tab in &mut self.tabs {
            stop(tab);
        }
    }
}

/// Sends to a tab's client, if it's connected.
fn send(tab: &Tab, message: &ToClient) {
    let _writing = tab.write_lock.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(out) = tab.out.as_ref() {
        write_or_drop(tab.id, out, message);
    }
}

/// How long a write to a client may wait for room in its socket.
const WRITE_TIMEOUT: Duration = Duration::from_secs(3);

/// Writes `message` to tab `id`'s client. A write that timed out may have left part of it in the
/// stream, so the connection ends there: its reader sees that, and the client starts again (`gone`).
fn write_or_drop(id: u32, mut out: &TcpStream, message: &ToClient) {
    let Err(e) = shell::write(&mut out, message, None) else {
        return;
    };
    if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) {
        tracing::warn!("Shell: tab {id}'s client stopped reading for {WRITE_TIMEOUT:?}: it starts again");
        let _ = out.shutdown(std::net::Shutdown::Both);
    } else {
        tracing::info!("Shell: couldn't reach tab {id}'s client: {e}");
    }
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
fn watch_army<S: SharedSurface>(sink: Sink<S>) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Army".into())
        .spawn(move || {
            let mut seen = Vec::new();
            loop {
                let now = army_clients();
                if now != seen {
                    seen = now.clone();
                    if !sink(Event::ArmyClients(now)) {
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
fn watch_updates<S: SharedSurface>(sink: Sink<S>) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Updates".into())
        .spawn(move || {
            let mut seen = turu_bot::updates::State::default();
            loop {
                let now = turu_bot::updates::state();
                if now != seen {
                    seen = now.clone();
                    if !sink(Event::Updates(now)) {
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

/// Feeds `TURU_SHELL_TEST`'s lines in `file` to the event loop, waiting where it says. With
/// `TURU_SHELL_TEST_FOLLOW` set, lines added to the file later are run too (driving a running shell
/// step by step).
pub fn spawn_test<S: SharedSurface>(file: String, sink: Sink<S>) {
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
                    } else if !sink(Event::Test(line.to_owned())) {
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
fn spawn_acceptor<S: SharedSurface>(listener: TcpListener, token: String, sink: Sink<S>) {
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
                        if !sink(Event::Connected { pid, stream }) {
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
fn spawn_reader<S: SharedSurface>(
    id: u32,
    generation: u64,
    mut stream: TcpStream,
    picture: Arc<Mutex<Picture>>,
    sink: Sink<S>,
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
                            let _ = sink(Event::Picture(id));
                        }
                        Err(e) => {
                            tracing::warn!("Shell: a broken picture from tab {id}: {e}");
                            break;
                        }
                    },
                    ToShell::Unknown | ToShell::Hello { .. } => {}
                    message => {
                        let _ = sink(Event::Message(id, message));
                    }
                }
            }
            let _ = sink(Event::Gone(id, generation));
        });
    if let Err(e) = spawned {
        tracing::error!("Shell: couldn't read tab {id}'s client: {e}");
    }
}

/// Closes tabs' clients as they close themselves: their connections first, at which a client
/// writes the game's saved data (AQW's saved login and preferences) and ends; whoever hasn't
/// ended after [`CLOSE_GRACE`] is stopped (`stop`). Turu: `stop` alone killed them at once, and
/// the game's unsaved data went with them (the window writes it when it closes).
fn close_clients<'a>(tabs: impl IntoIterator<Item = &'a mut Tab>) -> Vec<Child> {
    let mut children = Vec::new();
    for tab in tabs {
        if let Some(out) = tab.out.take() {
            let _ = out.shutdown(std::net::Shutdown::Both);
        }
        children.extend(tab.child.take());
    }
    children
}

/// Waits up to [`CLOSE_GRACE`] for closing clients (`close_clients`) to end, and kills the rest.
fn end_clients(mut children: Vec<Child>) {
    let deadline = Instant::now() + CLOSE_GRACE;
    loop {
        children.retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));
        if children.is_empty() || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    for mut child in children {
        tracing::warn!("Shell: client {} didn't close in time; stopped", child.id());
        drop(child.stdin.take());
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Stops a tab's client at once (gone, hung, never connected): its connection, stdin
/// (`--exit-with-parent`) and process. One that's working closes with `close_clients`.
fn stop(tab: &mut Tab) {
    tab.release_out = None;
    if let Some(out) = tab.out.take() {
        let _ = out.shutdown(std::net::Shutdown::Both);
    }
    if let Some(mut child) = tab.child.take() {
        drop(child.stdin.take());
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests;
