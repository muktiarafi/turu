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
//! Pictures are copied into the window for now (sharing the GPU texture is the next step).
//!
//! Testing without anyone at the window: `TURU_SHELL_TEST=<file>` runs its lines in order:
//! `wait <seconds>`, `open` (a tab at the login screen), `select <id>`, `close <id>`, `kill <id>`
//! (its client, as a crash would), `click <x> <y>` (in the window's pixels, to the tab on screen)
//! and `dump <path>` (the window's picture, as a PNG).

use crate::preferences::GlobalPreferences;
use anyhow::{Context as _, Error, anyhow};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_frontend::shell::{self, Button, Mods, PointerKind, TabInfo, ToClient, ToShell};
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

/// The newest picture from a client.
#[derive(Default)]
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
    /// Gave up on restarting (until the tab is selected).
    closed: bool,
    started: Instant,
}

impl Tab {
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

/// Starts the first tab's client and runs the window until it's closed.
pub fn run(_preferences: GlobalPreferences, client_args: Vec<String>) -> Result<(), Error> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Couldn't listen for the clients")?;
    let address = listener.local_addr()?.to_string();
    let token = turu_bot::control_api::new_token().map_err(|e| anyhow!(e))?;
    let event_loop = EventLoop::<ShellEvent>::with_user_event().build()?;
    spawn_acceptor(listener, token.clone(), event_loop.create_proxy());
    if let Ok(file) = std::env::var("TURU_SHELL_TEST") {
        spawn_test(file, event_loop.create_proxy());
    }
    let mut app = ShellApp {
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
    };
    // The first tab logs in as `--headless` would: with the shell's own `TURU_PASSWORD`.
    app.open_tab(client_args, std::env::var("TURU_PASSWORD").ok());
    event_loop.run_app(&mut app)?;
    for tab in &mut app.tabs {
        stop(tab);
    }
    Ok(())
}

/// Feeds `TURU_SHELL_TEST`'s lines to the event loop, waiting where it says.
fn spawn_test(file: String, proxy: EventLoopProxy<ShellEvent>) {
    let lines = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) => {
            tracing::warn!("Shell: TURU_SHELL_TEST {file}: {e}");
            return;
        }
    };
    let _ = std::thread::Builder::new()
        .name("Turu Shell Test".into())
        .spawn(move || {
            for line in lines.lines().map(str::trim).filter(|line| !line.is_empty()) {
                if let Some(seconds) = line.strip_prefix("wait ").and_then(|s| s.trim().parse::<f64>().ok()) {
                    std::thread::sleep(Duration::from_secs_f64(seconds));
                } else if proxy.send_event(ShellEvent::Test(line.to_owned())).is_err() {
                    return;
                }
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
}

struct ShellApp {
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
            closed: false,
            started: Instant::now(),
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
        let mut command = Command::new(exe);
        command
            .arg(format!("--client={}", self.address))
            .arg("--exit-with-parent")
            .args(&tab.args)
            .env(shell::TOKEN_ENV, &self.token)
            .env_remove("TURU_PASSWORD")
            .stdin(Stdio::piped());
        if let Some(password) = tab.password.as_deref().filter(|p| !p.is_empty()) {
            command.env("TURU_PASSWORD", password);
        }
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
        if old != id
            && let Some(tab) = self.tab(old)
        {
            send(tab, &ToClient::Hide);
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
        stop(&mut tab);
        tracing::info!("Shell: tab {id} closed");
        if self.shown == id {
            let next = self.tabs[index.min(self.tabs.len() - 1)].id;
            self.shown = 0;
            self.select(next);
        } else {
            self.broadcast_tabs();
        }
    }

    /// Tells the tab on screen the window's size.
    fn show_current(&mut self) {
        let Some((size, scale)) = self.size else {
            return;
        };
        let id = self.shown;
        if let Some(tab) = self.tab(id) {
            send(
                tab,
                &ToClient::Show {
                    width: size.width,
                    height: size.height,
                    scale: scale as f32,
                },
            );
            send(tab, &ToClient::Focus { focused: true });
        }
        self.update_title();
    }

    /// Every client gets the list of tabs, for its tab strip.
    fn broadcast_tabs(&mut self) {
        let tabs: Vec<TabInfo> = self
            .tabs
            .iter()
            .map(|tab| TabInfo {
                id: tab.id,
                label: tab.label.clone(),
                busy: tab.busy,
                down: tab.down(),
            })
            .collect();
        let message = ToClient::Tabs {
            tabs,
            shown: self.shown,
        };
        for tab in &mut self.tabs {
            send(tab, &message);
        }
        self.update_title();
    }

    fn update_title(&mut self) {
        let label = self
            .tabs
            .iter()
            .find(|tab| tab.id == self.shown)
            .map(|tab| tab.label.clone());
        let title = match label {
            Some(label) if self.tabs.len() > 1 => format!("Turu – {label} ({} tabs)", self.tabs.len()),
            Some(label) => format!("Turu – {label}"),
            None => "Turu".into(),
        };
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
        let id = tab.id;
        if id == self.shown {
            self.show_current();
        } else if let Some(tab) = self.tab(id) {
            send(tab, &ToClient::Hide);
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
            ToShell::CloseTab { id: target } => self.close(target),
            ToShell::OpenTab {
                username,
                password,
                server,
                script,
                run_script,
            } => {
                let mut args = Vec::new();
                if let Some(user) = username.filter(|u| !u.trim().is_empty()) {
                    args.extend(["--user".into(), user]);
                }
                if let Some(server) = server.filter(|s| !s.trim().is_empty()) {
                    args.extend(["--server".into(), server]);
                }
                if let Some(script) = script.filter(|s| !s.trim().is_empty()) {
                    args.extend([if run_script { "--run-script" } else { "--load-script" }.into(), script]);
                }
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
            ToShell::OpenUrl { url } => {
                if let Err(e) = webbrowser::open(&url) {
                    tracing::warn!("Couldn't open {url}: {e}");
                }
            }
            ToShell::Frame { .. } | ToShell::Hello { .. } | ToShell::Unknown => {}
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

    /// The window's size changed: the GPU surface and the tab on screen follow.
    fn resized(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        let scale = window.scale_factor();
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

    /// Copies the shown tab's newest picture into the window (the part that fits, while a resize
    /// is under way).
    fn draw(&self) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == self.shown) else {
            return;
        };
        let picture = tab.picture.lock().unwrap_or_else(|e| e.into_inner());
        if picture.pixels.is_empty() {
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
        gpu.queue.write_texture(
            frame.texture.as_image_copy(),
            &picture.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(picture.width * 4),
                rows_per_image: Some(picture.height),
            },
            wgpu::Extent3d {
                width: picture.width.min(gpu.config.width),
                height: picture.height.min(gpu.config.height),
                depth_or_array_layers: 1,
            },
        );
        drop(picture);
        gpu.queue.submit([]);
        gpu.queue.present(frame);
    }

    /// A line of `TURU_SHELL_TEST`'s file.
    fn test(&mut self, line: &str) {
        tracing::info!("Shell test: {line}");
        let (command, arg) = line.split_once(' ').unwrap_or((line, ""));
        let id = arg.trim().parse::<u32>().ok();
        if command == "click" {
            let at: Vec<f64> = arg.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            if let [x, y] = at[..] {
                let at = PhysicalPosition::new(x, y);
                for kind in [PointerKind::Move, PointerKind::Down, PointerKind::Up] {
                    self.send_shown(&pointer(kind, Button::Left, at));
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
            ("dump", _) => {
                let Some(tab) = self.tabs.iter().find(|tab| tab.id == self.shown) else {
                    return;
                };
                let picture = tab.picture.lock().unwrap_or_else(|e| e.into_inner());
                let mut rgba = picture.pixels.clone();
                for pixel in rgba.as_chunks_mut::<4>().0 {
                    pixel.swap(0, 2);
                }
                let saved =
                    image::RgbaImage::from_raw(picture.width, picture.height, rgba).map(|image| image.save(arg));
                tracing::info!("Shell test: dump of tab {} to {arg}: {saved:?}", tab.id);
            }
            _ => tracing::warn!("Shell test: what's \"{line}\"?"),
        }
    }

    fn key(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
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
        let attributes = Window::default_attributes()
            .with_title("Turu")
            .with_inner_size(winit::dpi::LogicalSize::new(1180.0, 640.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                tracing::error!("Shell: couldn't open the window: {e}");
                event_loop.exit();
                return;
            }
        };
        match make_gpu(window.clone()) {
            Ok(gpu) => self.gpu = Some(gpu),
            Err(e) => {
                tracing::error!("Shell: {e}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.resized();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ShellEvent) {
        match event {
            ShellEvent::Connected { pid, stream } => self.connected(pid, stream),
            ShellEvent::Picture(id) => {
                if id == self.shown
                    && let Some(window) = &self.window
                {
                    window.request_redraw();
                }
            }
            ShellEvent::Message(id, message) => self.message(id, message),
            ShellEvent::Gone(id, generation) => self.gone(id, generation),
            ShellEvent::Test(line) => self.test(&line),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.restart_due() {
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
                self.send_shown(&pointer(kind, button, at));
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

/// The window's surface on the GPU, with the clients' picture format copied straight in.
fn make_gpu(window: Arc<Window>) -> Result<Gpu, Error> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = instance.create_surface(window.clone())?;
    let adapter = futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        ..Default::default()
    }))?;
    let (device, queue) = futures::executor::block_on(adapter.request_device(&Default::default()))?;
    let caps = surface.get_capabilities(&adapter);
    let format = wgpu::TextureFormat::Bgra8Unorm;
    if !caps.formats.contains(&format) || !caps.usages.contains(wgpu::TextureUsages::COPY_DST) {
        return Err(anyhow!(
            "this window can't take the clients' pictures as they are ({:?})",
            caps.formats
        ));
    }
    let size = window.inner_size();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::COPY_DST,
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
    Ok(Gpu {
        device,
        queue,
        surface,
        config,
    })
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
