//! Turu: the shell mode (`--shell`, docs/TAB_PROCESSES_PROPOSAL.md, M2 and M5): the window over
//! `turu_shell` (docs/NATIVE_UI_PROPOSAL.md, L1's shell part), which holds the shell's logic. This
//! front owns what only a window has: winit's event loop and the window, its GPU chosen as the
//! clients choose theirs (`make_gpu`), the mouse, keys, input method and cursor in the link's
//! forms, the gamepads, the file and graphics dialogs (rfd), where the clients hand over their
//! surfaces (`shared_surface.rs`, `shared_gpu.rs`), and Mugetsu's startup and shutdown steps.
//!
//! `TURU_SHELL_TEST=<file>` drives a shell without anyone at the window (`turu_shell`'s docs list
//! its lines); the window's own are taken here: `resize <width> <height>` (the window's size in
//! points), `minimize on|off`, `title` (the window's title and size, logged) and `quit` (the window
//! closed, as by its close button).

use crate::preferences::GlobalPreferences;
use crate::turu;
use anyhow::{Error, anyhow};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_frontend::engine::STAGE;
use turu_frontend::shell::{self, Button, Mods, PointerKind, ToClient};
use turu_shell::{Action, Event, Gpu, Shell};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

pub use turu_shell::client_args;

/// How often the display's refresh rate is looked up (the window may move to another monitor).
const REFRESH_CHECK: Duration = Duration::from_secs(2);

/// A client's surface as the shell got it: an IOSurface (macOS), Vulkan memory or a D3D12
/// resource (Windows, Linux: `shared_gpu.rs`); elsewhere there are none.
#[cfg(target_os = "macos")]
type SharedSurface = crate::shared_surface::Surface;
#[cfg(any(windows, target_os = "linux"))]
type SharedSurface = crate::shared_gpu::Surface;
#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
type SharedSurface = turu_shell::NoSurface;

#[cfg(target_os = "macos")]
impl turu_shell::SharedSurface for crate::shared_surface::Surface {
    fn size(&self) -> (u32, u32) {
        crate::shared_surface::Surface::size(self)
    }

    fn texture(&mut self, device: &wgpu::Device) -> Option<wgpu::Texture> {
        crate::shared_surface::Surface::texture(self, device)
    }
}

#[cfg(any(windows, target_os = "linux"))]
impl turu_shell::SharedSurface for crate::shared_gpu::Surface {
    fn size(&self) -> (u32, u32) {
        crate::shared_gpu::Surface::size(self)
    }

    fn texture(&mut self, device: &wgpu::Device) -> Option<wgpu::Texture> {
        crate::shared_gpu::Surface::texture(self, device)
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "almost every event is the shell's, passed by value as before the split"
)]
enum ShellEvent {
    /// The shell's threads: clients, pictures, surfaces, tests, updates, the Army, dialogs.
    Shell(Event<SharedSurface>),
    /// A gamepad's button (Ruffle's name for it) was pressed or released: to the tab on screen.
    Gamepad { button: &'static str, pressed: bool },
    /// The answer to a tab's graphics choice (`choose_graphics`): the choice ("" for Automatic),
    /// or None when cancelled.
    #[cfg(any(windows, target_os = "linux"))]
    GraphicsChosen(Option<String>),
}

/// Whether this start runs as the shell: every start of Turu's own window (AQW, not a movie
/// given), Army > Accounts' "In its own window" clients included, but not `--headless` or a
/// shell's client. Each tab is a process of its own (docs/TAB_PROCESSES_PROPOSAL.md, M6).
pub fn runs_as_shell() -> bool {
    crate::mugetsu::plays_aqw() && !turu::headless() && turu::client_of().is_none()
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
    let event_loop = EventLoop::<ShellEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let sink: turu_shell::Sink<SharedSurface> =
        Arc::new(move |event| proxy.send_event(ShellEvent::Shell(event)).is_ok());
    // Clients hand their pictures over as GPU surfaces where the shell can take them, unless
    // `TURU_NO_SHARED_SURFACES` says pixels.
    let surfaces_name = if std::env::var_os("TURU_NO_SHARED_SURFACES").is_some() {
        tracing::info!("Shell: TURU_NO_SHARED_SURFACES: the clients send pixels");
        None
    } else {
        listen_for_surfaces(sink.clone())
    };
    let config = turu_shell::Config {
        exe: std::env::current_exe().map_err(|e| anyhow!("can't start the clients: {e}"))?,
        surfaces_name: surfaces_name.clone(),
        name: Box::new(crate::mugetsu::window_title),
    };
    let mut shell = Shell::new(config, sink.clone())?;
    if let Ok(file) = std::env::var("TURU_SHELL_TEST") {
        turu_shell::spawn_test(file, sink);
    }
    shell.start_services();
    // Gamepads, for the tab on screen (`--gamepad-button` maps their buttons to keys).
    watch_gamepads(event_loop.create_proxy());
    // The first tab logs in as `--headless` would: with the shell's own `TURU_PASSWORD`.
    shell.open_tab(client_args, std::env::var("TURU_PASSWORD").ok());
    let mut app = ShellApp {
        shell,
        proxy: event_loop.create_proxy(),
        window: None,
        modifiers: ModifiersState::empty(),
        cursor: PhysicalPosition::new(0.0, 0.0),
        refresh_checked: None,
        preferences,
        #[cfg(any(windows, target_os = "linux"))]
        start_again: false,
    };
    event_loop.run_app(&mut app)?;
    app.shell.stop_all();
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
    if let Some(path) = &surfaces_name {
        let _ = std::fs::remove_file(path);
    }
    #[cfg(not(target_os = "linux"))]
    drop(surfaces_name);
    // Update when I quit, asked from a tab: put in place now.
    turu::apply_pending_update();
    Ok(())
}

/// Reads the gamepads on a thread of its own (gilrs waits there for their events: the event loop
/// sleeps until something comes) and hands each button to the loop for the tab on screen. Ends
/// when the loop has gone (the shell starts again after a graphics choice).
fn watch_gamepads(proxy: EventLoopProxy<ShellEvent>) {
    let spawned = std::thread::Builder::new().name("Turu Gamepads".into()).spawn(move || {
        let mut gilrs = match gilrs::Gilrs::new() {
            Ok(gilrs) => gilrs,
            Err(e) => {
                tracing::warn!("Shell: no gamepads ({e})");
                return;
            }
        };
        while let Some(event) = gilrs.next_event_blocking(None) {
            let (button, pressed) = match event.event {
                gilrs::EventType::ButtonPressed(button, _) => (button, true),
                gilrs::EventType::ButtonReleased(button, _) => (button, false),
                _ => continue,
            };
            let Some(button) = crate::util::gilrs_button_to_gamepad_button(button).map(gamepad_button_name) else {
                continue;
            };
            if proxy.send_event(ShellEvent::Gamepad { button, pressed }).is_err() {
                return;
            }
        }
    });
    if let Err(e) = spawned {
        tracing::warn!("Shell: no gamepads ({e})");
    }
}

/// Ruffle's name for a gamepad's button, as `GamepadButton::from_str` reads it (`ToClient::Gamepad`).
fn gamepad_button_name(button: ruffle_core::events::GamepadButton) -> &'static str {
    use ruffle_core::events::GamepadButton;
    match button {
        GamepadButton::South => "south",
        GamepadButton::East => "east",
        GamepadButton::North => "north",
        GamepadButton::West => "west",
        GamepadButton::LeftTrigger => "left-trigger",
        GamepadButton::LeftTrigger2 => "left-trigger-2",
        GamepadButton::RightTrigger => "right-trigger",
        GamepadButton::RightTrigger2 => "right-trigger-2",
        GamepadButton::Select => "select",
        GamepadButton::Start => "start",
        GamepadButton::DPadUp => "dpad-up",
        GamepadButton::DPadDown => "dpad-down",
        GamepadButton::DPadLeft => "dpad-left",
        GamepadButton::DPadRight => "dpad-right",
    }
}

/// Where the clients hand over their surfaces, set up by platform: a Mach service (macOS), a Unix
/// socket (Linux), the clients' own messages (Windows: "handles"). `None`: they send pixels.
fn listen_for_surfaces(sink: turu_shell::Sink<SharedSurface>) -> Option<String> {
    #[cfg(target_os = "macos")]
    return crate::shared_surface::listen(move |pid, id, surface| {
        let _ = sink(Event::SurfaceArrived { pid, id, surface });
    });
    #[cfg(target_os = "linux")]
    return crate::shared_gpu::listen(move |pid, id, surface| {
        let _ = sink(Event::SurfaceArrived { pid, id, surface });
    });
    #[cfg(windows)]
    {
        drop(sink);
        return Some("handles".to_owned());
    }
    #[allow(unreachable_code)]
    {
        drop(sink);
        None
    }
}

struct ShellApp {
    shell: Shell<SharedSurface>,
    proxy: EventLoopProxy<ShellEvent>,
    window: Option<Arc<Window>>,
    modifiers: ModifiersState,
    cursor: PhysicalPosition<f64>,
    /// When the display's refresh rate was last looked up.
    refresh_checked: Option<Instant>,
    /// The backends and power preference the clients' GPUs are chosen by: the window's is chosen
    /// the same way, so they can share surfaces.
    preferences: GlobalPreferences,
    /// A graphics choice was saved: the shell starts again once its loop ends (`run`).
    #[cfg(any(windows, target_os = "linux"))]
    start_again: bool,
}

impl ShellApp {
    /// Carries out what the shell asked of the window.
    fn apply(&mut self, event_loop: &ActiveEventLoop) {
        for action in self.shell.actions() {
            match action {
                Action::Title(title) => {
                    if let Some(window) = &self.window {
                        window.set_title(&title);
                    }
                }
                Action::Redraw => {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                Action::Ime {
                    allowed,
                    password,
                    area,
                } => self.apply_ime(allowed, password, area),
                Action::Cursor(cursor) => {
                    if let Some(window) = &self.window {
                        set_cursor(window, cursor);
                    }
                }
                // A tab's Options > Application > Full Screen, or Esc: borderless on the window's
                // screen, as the window's right-click menu does. The tabs hear of it once the
                // window has changed (`resized`).
                Action::FullScreen(on) => {
                    if let Some(window) = &self.window {
                        window.set_fullscreen(on.then_some(winit::window::Fullscreen::Borderless(None)));
                    }
                }
                Action::FileDialog(dialog) => self.file_dialog(dialog),
                #[cfg(any(windows, target_os = "linux"))]
                Action::ChooseGraphics { tab, value } => self.choose_graphics(tab, value),
                #[cfg(not(any(windows, target_os = "linux")))]
                Action::ChooseGraphics { tab, value } => {
                    tracing::warn!("Shell: tab {tab} asked for graphics {value:?}, which this system doesn't choose");
                }
                Action::Quit => event_loop.exit(),
            }
        }
    }

    /// The window takes an input method as the tab on screen last asked: allowed or not, a
    /// password's or not, and where the candidates go.
    fn apply_ime(&self, allowed: bool, password: bool, area: Option<[f32; 4]>) {
        let Some(window) = &self.window else {
            return;
        };
        window.set_ime_allowed(allowed);
        if allowed {
            window.set_ime_purpose(if password {
                winit::window::ImePurpose::Password
            } else {
                winit::window::ImePurpose::Normal
            });
            if let Some([x, y, width, height]) = area {
                window.set_ime_cursor_area(PhysicalPosition::new(x, y), PhysicalSize::new(width, height));
            }
        }
    }

    /// Shows a tab's file dialog over the window (made here, on the event loop's thread, as macOS
    /// needs), and answers the tab once it's closed.
    fn file_dialog(&mut self, asked: turu_shell::FileDialog) {
        let mut dialog = rfd::AsyncFileDialog::new();
        for (name, extensions) in &asked.filters {
            dialog = dialog.add_filter(name, extensions);
        }
        if let Some(title) = &asked.title {
            dialog = dialog.set_title(title);
        }
        if let Some(directory) = &asked.directory {
            dialog = dialog.set_directory(directory);
        }
        if let Some(file_name) = &asked.file_name {
            dialog = dialog.set_file_name(file_name);
        }
        if let Some(window) = &self.window {
            dialog = dialog.set_parent(&**window);
        }
        let proxy = self.proxy.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Shell Dialog".into())
            .spawn(move || {
                let chosen = if asked.save {
                    futures::executor::block_on(dialog.save_file())
                } else {
                    futures::executor::block_on(dialog.pick_file())
                };
                let path = chosen.map(|file| file.path().to_string_lossy().into_owned());
                let answered = Event::Answered(asked.tab, asked.pid, asked.request, path);
                let _ = proxy.send_event(ShellEvent::Shell(answered));
            });
        if let Err(e) = spawned {
            tracing::warn!("Shell: couldn't wait for a file dialog: {e}");
            self.shell.dialog_closed();
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
        if choice == mugetsu::saved_graphics() || self.shell.dialog_open() {
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
        self.shell.dialog_opened();
        let proxy = self.proxy.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Shell Dialog".into())
            .spawn(move || {
                let ok = futures::executor::block_on(dialog.show()) == rfd::MessageDialogResult::Ok;
                let _ = proxy.send_event(ShellEvent::GraphicsChosen(ok.then_some(value)));
            });
        if let Err(e) = spawned {
            tracing::warn!("Shell: couldn't ask about the graphics: {e}");
            self.shell.dialog_closed();
        }
    }

    /// Windows: a client's shared handle, copied from its process here (the shell has no
    /// process handles of its own to give), then handed to the shell as a surface.
    #[cfg(windows)]
    #[allow(clippy::too_many_arguments, reason = "the message's fields, as they come")]
    fn shared_handle(
        &mut self,
        id: u32,
        surface: u64,
        route: &str,
        handle: u64,
        width: u32,
        height: u32,
        size: u64,
        memory_type: u32,
    ) {
        let taken = self.shell.child(id).and_then(|child| {
            let taken = crate::shared_gpu::take_handle(child, route, handle, width, height, size, memory_type)?;
            Some((child.id(), taken))
        });
        match taken {
            Some((pid, taken)) => self.shell.surface_arrived(pid, surface, taken),
            None => self.shell.no_surfaces_for(id, "its handle couldn't be copied"),
        }
    }

    /// The window's size, scale and full screen, to the shell.
    fn resized(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        let full_screen = window.fullscreen().is_some();
        self.shell
            .resized(size.width, size.height, window.scale_factor(), full_screen);
    }

    /// The display's refresh rate, looked up every [`REFRESH_CHECK`] (frame interpolation's
    /// pacing).
    fn check_refresh_rate(&mut self) {
        let now = Instant::now();
        if self
            .refresh_checked
            .is_some_and(|at| now.duration_since(at) < REFRESH_CHECK)
        {
            return;
        }
        self.refresh_checked = Some(now);
        let Some(window) = &self.window else {
            return;
        };
        let millihertz = window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .unwrap_or(60_000);
        self.shell.set_refresh_rate(millihertz);
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

    /// A line of `TURU_SHELL_TEST`'s file that only the window can carry out; false for the
    /// shell's.
    fn window_test(&self, line: &str) -> bool {
        let (command, arg) = line.split_once(' ').unwrap_or((line, ""));
        match command {
            // `minimize on` / `minimize off`: as the window's yellow button would (macOS reports it
            // as covered).
            "minimize" => {
                tracing::info!("Shell test: {line}");
                if let Some(window) = &self.window {
                    window.set_minimized(arg.trim() == "on");
                }
            }
            // `title`: the window's title and size, for the log.
            "title" => {
                tracing::info!("Shell test: {line}");
                if let Some(window) = &self.window {
                    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                    tracing::info!(
                        "Shell test: title \"{}\", {:.0}x{:.0} points",
                        window.title(),
                        size.width,
                        size.height
                    );
                }
            }
            // `resize 1600 900`: the window's inner size in points, as dragging its edge would.
            "resize" => {
                tracing::info!("Shell test: {line}");
                let size: Vec<f64> = arg.split_whitespace().filter_map(|v| v.parse().ok()).collect();
                if let (&[width, height], Some(window)) = (size.as_slice(), &self.window) {
                    let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width, height));
                }
            }
            _ => return false,
        }
        true
    }

    fn key(&mut self, event: &KeyEvent) {
        // Windows: winit still sends the keys an input method processed (egui-winit drops them
        // too); they're the input method's.
        if event.logical_key == Key::Named(NamedKey::Process) {
            return;
        }
        let pressed = event.state == ElementState::Pressed;
        // Esc leaves full screen, as in the window (the tab gets it too).
        if pressed
            && self.shell.full_screen()
            && event.logical_key == Key::Named(NamedKey::Escape)
            && let Some(window) = &self.window
        {
            window.set_fullscreen(None);
        }
        // The key by egui's name, for Turu's UI, and whole for the game, as the window's
        // `app.rs` hands it to Ruffle (with what it does in a text field); with either known.
        let name = egui_key_name(&event.logical_key);
        let key = crate::util::winit_input_to_ruffle_key_descriptor(event);
        let control = if pressed {
            crate::util::winit_to_ruffle_text_control(event, self.modifiers.into())
        } else {
            None
        };
        let known = key.physical_key != ruffle_core::events::PhysicalKey::Unknown
            || key.logical_key != ruffle_core::events::LogicalKey::Unknown;
        if name.is_some() || known {
            let mods = self.mods();
            self.shell.send_shown(&ToClient::Key {
                key: name.unwrap_or_default().to_owned(),
                pressed,
                mods,
                game: known.then(|| turu_frontend::shell::GameKey::new(key, control)),
            });
        }
        // Typed text, unless the key does something in a text field instead (as in the window);
        // the client leaves out what Ctrl or Cmd shortcuts give.
        if pressed
            && control.is_none()
            && let Some(text) = event.text.as_ref().filter(|text| text.chars().all(|c| !c.is_control()))
        {
            self.shell.send_shown(&ToClient::Text { text: text.to_string() });
        }
    }

    fn pointer(&mut self, kind: PointerKind, button: Button, at: PhysicalPosition<f64>) {
        self.shell.pointer(kind, button, at.x as f32, at.y as f32);
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
            .with_title(self.shell.title_for(None))
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
            Ok((gpu, adapter)) => self.shell.set_gpu(gpu, adapter),
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
        // (With "DirectX 12 not available" in it, if `make_gpu` noted that.)
        self.shell.window_opened();
        self.resized();
        self.apply(event_loop);
    }

    #[cfg_attr(not(any(windows, target_os = "linux")), allow(unused_variables))]
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: ShellEvent) {
        match event {
            ShellEvent::Gamepad { button, pressed } => self.shell.send_shown(&ToClient::Gamepad {
                button: button.to_owned(),
                pressed,
            }),
            #[cfg(any(windows, target_os = "linux"))]
            ShellEvent::GraphicsChosen(value) => {
                self.shell.dialog_closed();
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
            #[cfg(windows)]
            ShellEvent::Shell(Event::Message(
                id,
                shell::ToShell::SharedHandle {
                    id: surface,
                    route,
                    handle,
                    width,
                    height,
                    size,
                    memory_type,
                },
            )) => self.shared_handle(id, surface, &route, handle, width, height, size, memory_type),
            // `quit`: the window closed, as its close button does.
            ShellEvent::Shell(Event::Test(line)) if line.trim() == "quit" => event_loop.exit(),
            ShellEvent::Shell(Event::Test(line)) if self.window_test(line.trim()) => {}
            ShellEvent::Shell(event) => self.shell.handle(event),
        }
        self.apply(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.check_refresh_rate();
        let wake = self.shell.tick(Instant::now());
        self.apply(event_loop);
        match wake {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.resized(),
            WindowEvent::RedrawRequested => {
                if self.shell.draw() {
                    crate::mugetsu::frame_presented();
                }
            }
            WindowEvent::Focused(focused) => self.shell.focused(focused),
            // An input method's composition, for the tab on screen (which allowed it).
            WindowEvent::Ime(ime) => {
                let ime = match ime {
                    winit::event::Ime::Enabled => shell::ImeInput::Enabled,
                    winit::event::Ime::Preedit(text, cursor) => shell::ImeInput::Preedit { text, cursor },
                    winit::event::Ime::Commit(text) => shell::ImeInput::Commit { text },
                    winit::event::Ime::Disabled => shell::ImeInput::Disabled,
                };
                self.shell.send_shown(&ToClient::Ime { ime });
            }
            WindowEvent::DroppedFile(path) => self.shell.send_shown(&ToClient::Dropped {
                path: path.to_string_lossy().into_owned(),
            }),
            // winit reports it on macOS and X11 only.
            WindowEvent::Occluded(occluded) => self.shell.occluded(occluded),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                self.pointer(PointerKind::Move, Button::Left, position);
            }
            WindowEvent::CursorLeft { .. } => {
                let at = self.cursor;
                self.pointer(PointerKind::Leave, Button::Left, at);
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
                self.pointer(kind, button, at);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let per_line = f64::from(shell::WHEEL_PIXELS_PER_LINE);
                let (dx, dy, pixels) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (-x, -y, false),
                    MouseScrollDelta::PixelDelta(p) => (-(p.x / per_line) as f32, -(p.y / per_line) as f32, true),
                };
                let (x, y) = (self.cursor.x as f32, self.cursor.y as f32);
                self.shell.send_shown(&ToClient::Wheel { dx, dy, x, y, pixels });
            }
            WindowEvent::KeyboardInput { event, .. } => self.key(&event),
            _ => {}
        }
        self.apply(event_loop);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        crate::mugetsu::set_exit_phase(1);
        // Mugetsu 2.0.0, as the window (`app.rs`): the GPU, and the clients' pictures opened on
        // it, are let go of now, while the driver is alive, and wgpu's last command buffers are
        // dropped (2 s at most), so the device is gone before the rest of the shutdown, not
        // during it: on DirectX 12, Turu's default on Windows, that crashed.
        // The clients write the game's saved data and end together (`CLOSE_GRACE` at most).
        self.shell.exit();
        wgpu_core::aqw_flush_encoder_drops();
        crate::mugetsu::set_exit_phase(3);
        tracing::info!("Shell: closing: done");
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
    // graphics API crashing at startup (`mugetsu`), and the next start takes the other one; and
    // DirectX 12 that can't be started falls back to Vulkan (`mugetsu::open_gpu`), as the
    // clients' does.
    let power: wgpu::PowerPreference = preferences.graphics_power_preference().into();
    // With the window as the display: OpenGL needs it (Linux).
    let open = |wanted: wgpu::Backends| -> Result<_, Error> {
        let (instance, backend) = crate::gui::select_wgpu_backend(wanted, Some(&window))?;
        crate::mugetsu::graphics_startup_begin(
            backend.contains(wgpu::Backends::VULKAN),
            backend.contains(wgpu::Backends::DX12),
        );
        let surface = instance.create_surface(window.clone())?;
        let found = futures::executor::block_on(ruffle_render_wgpu::backend::request_adapter_and_device(
            backend,
            &instance,
            Some(&surface),
            power,
        ));
        Ok((instance, surface, found))
    };
    let (_instance, surface, adapter, device, queue) = match crate::mugetsu::open_gpu(wanted, |wanted| {
        let (instance, surface, found) = open(wanted)?;
        let (adapter, device, queue) = found.map_err(|e| anyhow!(e.to_string()))?;
        Ok((instance, surface, adapter, device, queue))
    }) {
        Ok(opened) => opened,
        // Any GPU that can show the window, if the clients' choice can't.
        Err(e) => {
            tracing::warn!("Shell: no GPU as the clients choose theirs ({e}); taking any");
            let (instance, surface, _) = open(wanted)?;
            let adapter = futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            }))?;
            let (device, queue) = futures::executor::block_on(adapter.request_device(&Default::default()))?;
            (instance, surface, adapter, device, queue)
        }
    };
    let info = adapter.get_info();
    crate::mugetsu::note_dx12_fallback(wanted, info.backend);
    crate::mugetsu::set_adapter_ids(info.vendor, info.device);
    tracing::info!(
        "Shell graphics: {} on {} (type: {:?})",
        info.backend.to_str(),
        info.name,
        info.device_type
    );
    let key = shell::adapter_key(info.backend.to_str(), info.vendor, info.device, &info.name);
    let size = window.inner_size();
    let gpu = Gpu::new(&adapter, device, queue, surface, (size.width, size.height))?;
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
    #[test]
    fn gamepad_buttons_are_named_as_ruffle_reads_them() {
        use ruffle_core::events::GamepadButton::*;
        let all = [
            South,
            East,
            North,
            West,
            LeftTrigger,
            LeftTrigger2,
            RightTrigger,
            RightTrigger2,
            Select,
            Start,
            DPadUp,
            DPadDown,
            DPadLeft,
            DPadRight,
        ];
        for button in all {
            let name = super::gamepad_button_name(button);
            let parsed: Result<ruffle_core::events::GamepadButton, _> = name.parse();
            assert!(parsed.is_ok_and(|parsed| parsed == button), "{name}");
        }
    }
}
