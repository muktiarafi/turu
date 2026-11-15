//! Turu: the shell mode (`--shell`, docs/TAB_PROCESSES_PROPOSAL.md, M2). The window runs no game:
//! it starts a client process (`turu --client=<address>`, `client.rs`) for the tab, shows the
//! pictures the client sends, and sends it the window's mouse, keys and size. The client gets the
//! shell's other startup flags (`--user`, `--run-script`, ...), and quits with it.
//!
//! M2's shell has one tab and copies each picture into the window (fine for a start; sharing the
//! GPU texture instead is M2's last check and M5's work).

use crate::preferences::GlobalPreferences;
use anyhow::{Context as _, Error, anyhow};
use std::io::Write as _;
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_frontend::shell::{self, Button, Mods, PointerKind, ToClient, ToShell};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

/// The newest picture from the client.
#[derive(Default)]
struct Picture {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    frames: u64,
}

enum ShellEvent {
    /// A new picture came.
    Picture,
    /// The client went away.
    Gone,
}

/// Starts the client and runs the window until it's closed or the client goes away.
pub fn run(_preferences: GlobalPreferences, client_args: Vec<String>) -> Result<(), Error> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Couldn't listen for the client")?;
    let address = listener.local_addr()?.to_string();
    let token = turu_bot::control_api::new_token().map_err(|e| anyhow!(e))?;
    let exe = std::env::current_exe()?;
    let mut child = Command::new(exe)
        .arg(format!("--client={address}"))
        .arg("--exit-with-parent")
        .args(&client_args)
        .env(shell::TOKEN_ENV, &token)
        .stdin(Stdio::piped())
        .spawn()
        .context("Couldn't start the client")?;
    tracing::info!("Shell: started the client (process {}) for {address}", child.id());
    let stream = accept(&listener, &token, &mut child)?;
    let event_loop = EventLoop::<ShellEvent>::with_user_event().build()?;
    let picture: Arc<Mutex<Picture>> = Arc::default();
    spawn_reader(stream.try_clone()?, picture.clone(), event_loop.create_proxy());
    let mut app = ShellApp {
        out: stream,
        picture,
        gpu: None,
        window: None,
        modifiers: ModifiersState::empty(),
        cursor: PhysicalPosition::new(0.0, 0.0),
        shown: None,
        drawn: 0,
        child,
        started: Instant::now(),
    };
    event_loop.run_app(&mut app)?;
    let _ = app.child.kill();
    Ok(())
}

/// Waits for the client's hello with the right token (30 s at most).
fn accept(listener: &TcpListener, token: &str, child: &mut Child) -> Result<TcpStream, Error> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_nodelay(true)?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                let hello = shell::read::<ToShell>(&mut stream)?;
                stream.set_read_timeout(None)?;
                match hello {
                    Some(ToShell::Hello {
                        token: given,
                        pid,
                        version,
                    }) if given == token => {
                        tracing::info!("Shell: client {pid} (Turu {version}) connected");
                        return Ok(stream);
                    }
                    _ => tracing::warn!("Shell: a connection without the token; ignored"),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if let Some(status) = child.try_wait()? {
                    return Err(anyhow!("The client quit before connecting ({status})"));
                }
                if Instant::now() > deadline {
                    return Err(anyhow!("The client didn't connect in 30 s"));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Reads the client's pictures and requests into `picture` and the event loop.
fn spawn_reader(mut stream: TcpStream, picture: Arc<Mutex<Picture>>, proxy: EventLoopProxy<ShellEvent>) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Reader".into())
        .spawn(move || {
            loop {
                let message = match shell::read::<ToShell>(&mut stream) {
                    Ok(Some(message)) => message,
                    Ok(None) => break,
                    Err(e) => {
                        tracing::warn!("Shell: lost the client: {e}");
                        break;
                    }
                };
                match message {
                    ToShell::Frame { width, height, len, .. } => match shell::read_payload(&mut stream, len) {
                        Ok(pixels) => {
                            let mut newest = picture.lock().unwrap_or_else(|e| e.into_inner());
                            newest.width = width;
                            newest.height = height;
                            newest.pixels = pixels;
                            newest.frames += 1;
                            drop(newest);
                            let _ = proxy.send_event(ShellEvent::Picture);
                        }
                        Err(e) => {
                            tracing::warn!("Shell: a broken picture: {e}");
                            break;
                        }
                    },
                    ToShell::OpenUrl { url } => {
                        if let Err(e) = webbrowser::open(&url) {
                            tracing::warn!("Couldn't open {url}: {e}");
                        }
                    }
                    ToShell::SelectTab { .. } | ToShell::Hello { .. } | ToShell::Unknown => {}
                }
            }
            let _ = proxy.send_event(ShellEvent::Gone);
        });
    if let Err(e) = spawned {
        tracing::error!("Shell: couldn't start reading the client: {e}");
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct ShellApp {
    out: TcpStream,
    picture: Arc<Mutex<Picture>>,
    gpu: Option<Gpu>,
    window: Option<Arc<Window>>,
    modifiers: ModifiersState,
    cursor: PhysicalPosition<f64>,
    /// The size and scale the client was last told.
    shown: Option<(PhysicalSize<u32>, f64)>,
    /// The pictures drawn, for the log.
    drawn: u64,
    child: Child,
    started: Instant,
}

impl ShellApp {
    fn send(&mut self, message: &ToClient) {
        if let Err(e) = shell::write(&mut self.out, message, None) {
            tracing::warn!("Shell: couldn't reach the client: {e}");
        }
    }

    /// Tells the client the window's size, when it changed.
    fn show(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        let scale = window.scale_factor();
        if size.width == 0 || size.height == 0 || self.shown == Some((size, scale)) {
            return;
        }
        self.shown = Some((size, scale));
        if let Some(gpu) = &mut self.gpu {
            gpu.config.width = size.width;
            gpu.config.height = size.height;
            gpu.surface.configure(&gpu.device, &gpu.config);
        }
        self.send(&ToClient::Show {
            width: size.width,
            height: size.height,
            scale: scale as f32,
        });
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

    /// Copies the newest picture into the window (the part that fits, while a resize is under way).
    fn draw(&mut self) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let picture = self.picture.lock().unwrap_or_else(|e| e.into_inner());
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
        let width = picture.width.min(gpu.config.width);
        let height = picture.height.min(gpu.config.height);
        gpu.queue.write_texture(
            frame.texture.as_image_copy(),
            &picture.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(picture.width * 4),
                rows_per_image: Some(picture.height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let frames = picture.frames;
        drop(picture);
        gpu.queue.submit([]);
        gpu.queue.present(frame);
        self.drawn += 1;
        if self.drawn == 1 || self.drawn.is_multiple_of(300) {
            tracing::info!(
                "Shell: {} pictures drawn ({frames} received) in {:.0} s",
                self.drawn,
                self.started.elapsed().as_secs_f64()
            );
        }
    }

    fn key(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        if let Some(key) = egui_key_name(&event.logical_key) {
            let mods = self.mods();
            self.send(&ToClient::Key {
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
            self.send(&ToClient::Text { text: text.to_string() });
        }
    }
}

impl ApplicationHandler<ShellEvent> for ShellApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Turu (shell)")
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
        self.show();
        self.send(&ToClient::Focus { focused: true });
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: ShellEvent) {
        match event {
            ShellEvent::Picture => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            ShellEvent::Gone => {
                tracing::info!("Shell: the client is gone; closing");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.show(),
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::Focused(focused) => self.send(&ToClient::Focus { focused }),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                self.send(&pointer(PointerKind::Move, Button::Left, position));
            }
            WindowEvent::CursorLeft { .. } => self.send(&pointer(PointerKind::Leave, Button::Left, self.cursor)),
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
                self.send(&pointer(kind, button, self.cursor));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (-x, -y),
                    MouseScrollDelta::PixelDelta(p) => (-(p.x / 40.0) as f32, -(p.y / 40.0) as f32),
                };
                let (x, y) = (self.cursor.x as f32, self.cursor.y as f32);
                self.send(&ToClient::Wheel { dx, dy, x, y });
            }
            WindowEvent::KeyboardInput { event, .. } => self.key(&event),
            _ => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // The client quits when the shell's connection closes (and its stdin, `--exit-with-parent`).
        let _ = self.out.shutdown(std::net::Shutdown::Both);
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.flush();
        }
        self.child.stdin = None;
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

/// The window's surface on the GPU, with the client's picture format copied straight in.
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
            "this window can't take the client's pictures as they are ({:?})",
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
    Some(match key {
        Key::Named(named) => match named {
            NamedKey::Enter => "Enter",
            NamedKey::Tab => "Tab",
            NamedKey::Backspace => "Backspace",
            NamedKey::Delete => "Delete",
            NamedKey::Escape => "Escape",
            NamedKey::Space => "Space",
            NamedKey::ArrowUp => "ArrowUp",
            NamedKey::ArrowDown => "ArrowDown",
            NamedKey::ArrowLeft => "ArrowLeft",
            NamedKey::ArrowRight => "ArrowRight",
            NamedKey::Home => "Home",
            NamedKey::End => "End",
            NamedKey::PageUp => "PageUp",
            NamedKey::PageDown => "PageDown",
            NamedKey::Insert => "Insert",
            NamedKey::F1 => "F1",
            NamedKey::F2 => "F2",
            NamedKey::F3 => "F3",
            NamedKey::F4 => "F4",
            NamedKey::F5 => "F5",
            NamedKey::F6 => "F6",
            NamedKey::F7 => "F7",
            NamedKey::F8 => "F8",
            NamedKey::F9 => "F9",
            NamedKey::F10 => "F10",
            NamedKey::F11 => "F11",
            NamedKey::F12 => "F12",
            _ => return None,
        },
        Key::Character(text) => {
            const LETTERS: [&str; 36] = [
                "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T",
                "U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
            ];
            let upper = text.to_uppercase();
            return LETTERS.iter().copied().find(|letter| *letter == upper);
        }
        _ => return None,
    })
}
