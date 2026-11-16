//! Turu: a client's screen (docs/TAB_PROCESSES_PROPOSAL.md, M2). The client mode runs one tab
//! without a window: this draws the game and Turu's UI over it into a texture the size the shell
//! asked for ([`ToClient::Show`]), reads it back without making the engine wait, and hands each
//! picture to a sink (the client sends it to the shell). Input comes from the shell's messages
//! and goes, as on the desktop, to Turu's UI when a press starts on it, else to the game.
//!
//! Android's screen (`android/src/screen.rs`) draws the same way into a window; the two share
//! [`crate::movie_texture`]. Everything here runs on the engine thread.

use crate::engine::Step;
use crate::gui::turu::TuruUi;
use crate::movie_texture::{Blit, MovieTexture};
use crate::shell::{Button, GridShow, Mods, PointerKind, ToClient};
use crate::turu;
use ruffle_core::ViewportDimensions;
use ruffle_core::events::{
    KeyDescriptor, KeyLocation, LogicalKey, MouseButton, MouseWheelDelta, NamedKey, PhysicalKey,
    TextControlCode,
};
use ruffle_core::{Player, PlayerEvent};
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::wgpu;
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The format of the pictures: what macOS and Windows windows take as they are.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// The shortest time between two pictures of the UI alone (30 a second).
const UI_FRAME: Duration = Duration::from_millis(33);
/// How often a readback is looked at while one is under way.
const READBACK_POLL: Duration = Duration::from_millis(4);

/// Where each picture goes: its width, height and BGRA rows.
pub type FrameSink = Box<dyn FnMut(u32, u32, Vec<u8>)>;

/// The texture drawn into and the buffer it's read back through, at one size.
struct Target {
    texture: wgpu::Texture,
    buffer: Arc<wgpu::Buffer>,
    size: (u32, u32),
    /// A row in the buffer, padded to wgpu's 256 bytes.
    padded_row: u32,
}

/// A readback under way: done when the buffer is mapped.
struct Readback {
    buffer: Arc<wgpu::Buffer>,
    size: (u32, u32),
    padded_row: u32,
    mapped: Arc<AtomicBool>,
}

pub struct ClientScreen {
    descriptors: Arc<Descriptors>,
    blit: Arc<Blit>,
    pipeline: wgpu::RenderPipeline,
    egui: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
    ui: TuruUi,
    sink: FrameSink,
    /// The picture's size in pixels and its pixels per point, while the shell shows the tab.
    shown: Option<((u32, u32), f32)>,
    /// How it shows in Grid View: the whole window with its game in its tile (the active tab), or
    /// only its game (another tab's tile).
    grid: Option<GridShow>,
    target: Option<Target>,
    readback: Option<Readback>,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    /// The button held, and whether its press went to the UI (else the game).
    pressed: Option<(Button, bool)>,
    /// The game's viewport, last set.
    movie_size: (u32, u32),
    started: Instant,
    last_draw: Instant,
    /// When the UI wants drawing again, if it does.
    ui_due: Option<Instant>,
    /// A new picture of the game is wanted (a resize, a frame not drawn because a readback was
    /// under way).
    picture_due: bool,
}

impl ClientScreen {
    pub fn new(descriptors: Arc<Descriptors>, blit: Arc<Blit>, sink: FrameSink) -> Self {
        let pipeline = blit.pipeline(&descriptors.device, FORMAT);
        let egui = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        crate::gui::turu::theme::add_symbol_font(&mut fonts);
        egui.set_fonts(fonts);
        let egui_renderer = egui_wgpu::Renderer::new(
            &descriptors.device,
            FORMAT,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );
        Self {
            descriptors,
            blit,
            pipeline,
            egui,
            egui_renderer,
            ui: TuruUi::new(),
            sink,
            shown: None,
            grid: None,
            target: None,
            readback: None,
            events: Vec::new(),
            modifiers: egui::Modifiers::NONE,
            pressed: None,
            movie_size: (0, 0),
            started: Instant::now(),
            last_draw: Instant::now(),
            ui_due: None,
            picture_due: false,
        }
    }

    /// Draws the UI again soon (the shell's tab list changed, say).
    pub fn request_repaint(&mut self) {
        if self.shown.is_some() {
            self.ui_due = Some(Instant::now());
        }
    }

    /// Whether the shell shows this tab.
    pub fn shown(&self) -> bool {
        self.shown.is_some()
    }

    /// One of the shell's messages.
    pub fn handle(&mut self, player: &mut Player, message: ToClient) {
        match message {
            ToClient::Show {
                width,
                height,
                scale,
                grid,
            } => {
                let size = (width.clamp(1, 8192), height.clamp(1, 8192));
                let scale = if scale.is_finite() && scale > 0.0 {
                    scale
                } else {
                    1.0
                };
                self.shown = Some((size, scale));
                if self
                    .target
                    .as_ref()
                    .is_none_or(|target| target.size != size)
                {
                    self.target = Some(self.make_target(size));
                }
                self.picture_due = true;
                self.ui_due = Some(Instant::now());
                // Another tab's tile takes no input and runs at the tiles' rate.
                let tile = grid.as_ref().is_some_and(|grid| !grid.active);
                set_mouse(player, !tile);
                turu::bot().state.lock().hidden_tab = tile;
                set_grid(grid.as_ref());
                self.grid = grid;
            }
            ToClient::Hide => {
                set_grid(None);
                self.grid = None;
                self.shown = None;
                self.target = None;
                self.pressed = None;
                set_mouse(player, false);
                turu::bot().state.lock().hidden_tab = true;
            }
            ToClient::Pointer { kind, button, x, y } => self.pointer(player, kind, button, x, y),
            ToClient::Wheel { dx, dy, x, y } => {
                let pos = self.point(x, y);
                self.events.push(egui::Event::PointerMoved(pos));
                self.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(-dx, -dy),
                    modifiers: self.modifiers,
                    phase: egui::TouchPhase::Move,
                });
                if !self.on_ui(x, y) {
                    player.handle_event(PlayerEvent::MouseWheel {
                        delta: MouseWheelDelta::Lines(-f64::from(dy)),
                    });
                }
            }
            ToClient::Key { key, pressed, mods } => self.key(player, &key, pressed, mods),
            ToClient::Text { text } => {
                if self.egui.egui_wants_keyboard_input() {
                    self.events.push(egui::Event::Text(text));
                } else {
                    for codepoint in text.chars().filter(|c| !c.is_control()) {
                        player.handle_event(PlayerEvent::TextInput { codepoint });
                    }
                }
            }
            ToClient::Focus { focused } => {
                self.events.push(egui::Event::WindowFocused(focused));
                player.handle_event(if focused {
                    PlayerEvent::FocusGained
                } else {
                    PlayerEvent::FocusLost
                });
            }
            // The client's own (`app/src/client.rs`): not the screen's.
            ToClient::Tabs { .. }
            | ToClient::Answer { .. }
            | ToClient::Dump { .. }
            | ToClient::Unknown => {}
        }
        if self.shown.is_some() && !self.events.is_empty() {
            self.ui_due = Some(Instant::now());
        }
    }

    /// After a tick or at the time asked: collects a finished readback, and draws when the game
    /// has a new frame or the UI wants it. When to be called again, if before the next tick.
    pub fn step(&mut self, player: &mut Player, step: Step) -> Option<Instant> {
        self.collect();
        if self.shown.is_some() {
            let game_frame = matches!(step, Step::Ticked { .. }) && player.needs_render();
            let ui_wanted = self.ui_due.is_some_and(|due| due <= Instant::now());
            if game_frame || ui_wanted || self.picture_due {
                if self.readback.is_none() {
                    let picture = game_frame || self.picture_due;
                    self.picture_due = false;
                    self.draw(player, picture);
                } else if game_frame {
                    self.picture_due = true;
                }
            }
        }
        let readback = self
            .readback
            .as_ref()
            .map(|_| Instant::now() + READBACK_POLL);
        let ui = self.ui_due.map(|due| due.max(self.last_draw + UI_FRAME));
        [readback, ui].into_iter().flatten().min()
    }

    fn make_target(&self, size: (u32, u32)) -> Target {
        let device = &self.descriptors.device;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Turu client picture"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let padded_row = (size.0 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Turu client readback"),
            size: u64::from(padded_row) * u64::from(size.1),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Target {
            texture,
            buffer: Arc::new(buffer),
            size,
            padded_row,
        }
    }

    /// Hands a finished readback to the sink.
    fn collect(&mut self) {
        let Some(readback) = &self.readback else {
            return;
        };
        let _ = self.descriptors.device.poll(wgpu::PollType::Poll);
        if !readback.mapped.load(Ordering::Acquire) {
            return;
        }
        let readback = self.readback.take().expect("checked above");
        let (width, height) = readback.size;
        let row = width as usize * 4;
        let mut pixels = Vec::with_capacity(row * height as usize);
        match readback.buffer.slice(..).get_mapped_range() {
            Ok(data) => {
                for y in 0..height as usize {
                    let start = y * readback.padded_row as usize;
                    pixels.extend_from_slice(&data[start..start + row]);
                }
            }
            Err(e) => tracing::warn!("Couldn't read the picture back: {e}"),
        }
        readback.buffer.unmap();
        (self.sink)(width, height, pixels);
    }

    /// The game's part of the picture's left and top, in pixels: its tile's in Grid View.
    fn offsets(&self) -> (f32, f32) {
        match &self.grid {
            Some(grid) if !grid.active => return (0.0, 0.0),
            Some(grid) => {
                if let Some(tile) = grid.tile() {
                    return (tile.x as f32, tile.y as f32);
                }
            }
            None => {}
        }
        let ppp = self.shown.map_or(1.0, |(_, ppp)| ppp);
        (
            crate::gui::left_width() as f32 * ppp,
            crate::gui::top_height() as f32 * ppp,
        )
    }

    /// The game's size in the picture, in pixels.
    fn movie_size(&self, size: (u32, u32)) -> (u32, u32) {
        if let Some(tile) = self
            .grid
            .as_ref()
            .filter(|grid| grid.active)
            .and_then(GridShow::tile)
        {
            return (tile.width.max(1), tile.height.max(1));
        }
        let (left, top) = self.offsets();
        (
            size.0.saturating_sub(left.round() as u32).max(1),
            size.1.saturating_sub(top.round() as u32).max(1),
        )
    }

    /// Whether (`x`, `y`) is on the game, not the rest of the picture.
    fn on_game(&self, x: f32, y: f32) -> bool {
        let (left, top) = self.offsets();
        let size = self.shown.map_or((0, 0), |(size, _)| self.movie_size(size));
        x >= left && y >= top && x < left + size.0 as f32 && y < top + size.1 as f32
    }

    fn point(&self, x: f32, y: f32) -> egui::Pos2 {
        let ppp = self.shown.map_or(1.0, |(_, ppp)| ppp);
        egui::pos2(x / ppp, y / ppp)
    }

    /// Whether (`x`, `y`) is on Turu's UI: the rail, the bar above the game, or a window, menu or
    /// popup (the root covers the picture on egui's background layer, which doesn't count).
    fn on_ui(&self, x: f32, y: f32) -> bool {
        !self.on_game(x, y)
            || self
                .egui
                .layer_id_at(self.point(x, y))
                .is_some_and(|layer| layer.order != egui::Order::Background)
    }

    fn pointer(&mut self, player: &mut Player, kind: PointerKind, button: Button, x: f32, y: f32) {
        let pos = self.point(x, y);
        let (left, top) = self.offsets();
        let (game_x, game_y) = (f64::from(x - left), f64::from(y - top));
        let egui_button = match button {
            Button::Left => egui::PointerButton::Primary,
            Button::Right => egui::PointerButton::Secondary,
            Button::Middle => egui::PointerButton::Middle,
        };
        let game_button = match button {
            Button::Left => MouseButton::Left,
            Button::Right => MouseButton::Right,
            Button::Middle => MouseButton::Middle,
        };
        match kind {
            PointerKind::Move => {
                self.events.push(egui::Event::PointerMoved(pos));
                let to_game = match self.pressed {
                    Some((_, on_ui)) => !on_ui,
                    None => !self.on_ui(x, y),
                };
                if to_game {
                    player.handle_event(PlayerEvent::MouseMove {
                        x: game_x,
                        y: game_y,
                    });
                }
            }
            PointerKind::Down => {
                // egui sees every press: one on the game closes an open menu, as on the desktop.
                let on_ui = self.on_ui(x, y) || self.egui.egui_wants_pointer_input();
                self.pressed = Some((button, on_ui));
                self.events.push(egui::Event::PointerMoved(pos));
                self.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui_button,
                    pressed: true,
                    modifiers: self.modifiers,
                });
                if !on_ui {
                    player.handle_event(PlayerEvent::MouseDown {
                        x: game_x,
                        y: game_y,
                        button: game_button,
                        index: None,
                    });
                }
            }
            PointerKind::Up => {
                let to_game = self.pressed.take().is_some_and(|(_, on_ui)| !on_ui);
                self.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui_button,
                    pressed: false,
                    modifiers: self.modifiers,
                });
                if to_game {
                    player.handle_event(PlayerEvent::MouseUp {
                        x: game_x,
                        y: game_y,
                        button: game_button,
                    });
                }
            }
            PointerKind::Leave => {
                self.events.push(egui::Event::PointerGone);
                player.handle_event(PlayerEvent::MouseLeave);
            }
        }
    }

    /// A key: Turu's UI's while one of its text boxes has the focus, else the game's (and
    /// HotKeys', which see the press too). Typed text comes as [`ToClient::Text`].
    fn key(&mut self, player: &mut Player, name: &str, pressed: bool, mods: Mods) {
        self.modifiers = egui::Modifiers {
            alt: mods.alt,
            ctrl: mods.ctrl,
            shift: mods.shift,
            mac_cmd: cfg!(target_os = "macos") && mods.command,
            command: mods.command,
        };
        if let Some(key) = egui::Key::from_name(name) {
            self.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: self.modifiers,
            });
        }
        if self.egui.egui_wants_keyboard_input() {
            return;
        }
        let Some((logical_key, key_location)) = logical_key(name) else {
            return;
        };
        let key = KeyDescriptor {
            physical_key: PhysicalKey::Unknown,
            logical_key,
            key_location,
        };
        if !pressed {
            player.handle_event(PlayerEvent::KeyUp { key });
            return;
        }
        player.handle_event(PlayerEvent::KeyDown { key });
        if let Some(code) = text_control(logical_key, mods) {
            player.handle_event(PlayerEvent::TextControl { code });
        }
    }

    /// Draws the picture (the game rendered again with `new_picture`) and starts reading it back.
    fn draw(&mut self, player: &mut Player, mut new_picture: bool) {
        let Some((size, ppp)) = self.shown else {
            return;
        };
        if ruffle_render_wgpu::device_lost().is_some() {
            return;
        }
        if self
            .target
            .as_ref()
            .is_none_or(|target| target.size != size)
        {
            self.target = Some(self.make_target(size));
        }
        self.last_draw = Instant::now();
        let (left, top) = self.offsets();
        let movie_size = self.movie_size(size);
        if movie_size != self.movie_size {
            self.movie_size = movie_size;
            player.set_viewport_dimensions(ViewportDimensions {
                width: movie_size.0,
                height: movie_size.1,
                scale_factor: f64::from(ppp),
            });
            new_picture = true;
        }
        if new_picture && !turu::headless_mode() {
            player.render();
        }

        // Another tab's tile: its game alone.
        let tile = self.grid.as_ref().is_some_and(|grid| !grid.active);
        let mut raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size.0 as f32 / ppp, size.1 as f32 / ppp),
            )),
            time: Some(self.started.elapsed().as_secs_f64()),
            events: std::mem::take(&mut self.events),
            max_texture_side: Some(self.descriptors.limits.max_texture_dimension_2d as usize),
            ..Default::default()
        };
        if let Some(viewport) = raw_input.viewports.get_mut(&egui::ViewportId::ROOT) {
            viewport.native_pixels_per_point = Some(ppp);
        }
        let top_points = top / ppp;
        let ui = &mut self.ui;
        let output = self.egui.run_ui(raw_input, |root| {
            if tile {
                return;
            }
            ui.show_bar(root);
            if turu::headless_mode() {
                crate::gui::turu::show_headless_screen(root.ctx(), top_points);
            }
            ui.show_windows(root.ctx());
        });
        self.ui_due = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .and_then(|viewport| Instant::now().checked_add(viewport.repaint_delay));
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::OpenUrl(open) = command {
                crate::platform::platform().open_url(&open.url);
            }
        }

        let primitives = self.egui.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.0, size.1],
            pixels_per_point: output.pixels_per_point,
        };
        let device = &self.descriptors.device;
        let queue = &self.descriptors.queue;
        let mut textures = output.textures_delta;
        for (id, deltas) in &textures.set {
            for delta in deltas {
                self.egui_renderer.update_texture(device, queue, *id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Turu client picture"),
        });
        let mut buffers =
            self.egui_renderer
                .update_buffers(device, queue, &mut encoder, &primitives, &screen);
        let target = self.target.as_ref().expect("made above");
        {
            let view = target.texture.create_view(&Default::default());
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // The active tab in Grid View: the other tiles show through.
                            load: wgpu::LoadOp::Clear(
                                if self.grid.as_ref().is_some_and(|grid| grid.active) {
                                    wgpu::Color::TRANSPARENT
                                } else {
                                    wgpu::Color::BLACK
                                },
                            ),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    label: Some("Turu client picture"),
                    ..Default::default()
                })
                .forget_lifetime();
            if let Some(movie) =
                <dyn Any>::downcast_ref::<WgpuRenderBackend<MovieTexture>>(player.renderer_mut())
                    .map(|r| r.target())
            {
                pass.set_viewport(
                    left.round(),
                    top,
                    movie_size.0 as f32,
                    movie_size.1 as f32,
                    0.0,
                    1.0,
                );
                self.blit.draw(&self.pipeline, &movie.bind_group, &mut pass);
                pass.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32, 0.0, 1.0);
            }
            self.egui_renderer.render(&mut pass, &primitives, &screen);
        }
        encoder.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &target.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(target.padded_row),
                    rows_per_image: Some(size.1),
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        buffers.push(encoder.finish());
        queue.submit(buffers);
        for id in &textures.free {
            self.egui_renderer.free_texture(id);
        }
        // Applied: epaint checks (in debug builds) that none is dropped unhandled.
        textures.clear();
        let mapped = Arc::new(AtomicBool::new(false));
        let done = mapped.clone();
        target
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_ok() {
                    done.store(true, Ordering::Release);
                }
            });
        self.readback = Some(Readback {
            buffer: target.buffer.clone(),
            size,
            padded_row: target.padded_row,
            mapped,
        });
    }
}

/// Grid View for the rest of the client: whether it's on (the bar's toggle), the tiles' labels
/// (the active tab draws them), and the frame rate of a tile that isn't the active tab's.
fn set_grid(grid: Option<&GridShow>) {
    crate::grid::set_on(grid.is_some());
    let tiles = grid
        .filter(|grid| grid.active)
        .map(|grid| {
            grid.tiles
                .iter()
                .map(|tile| {
                    let area = crate::grid::Tile {
                        x: tile.x,
                        y: tile.y,
                        width: tile.width,
                        height: tile.height,
                    };
                    (tile.id, area)
                })
                .collect()
        })
        .unwrap_or_default();
    crate::grid::set_shell_tiles(tiles);
    turu_bot::api::options::set_hidden_tab_fps(match grid {
        Some(_) => crate::grid::TILE_FPS,
        None => turu::grid_test_fps().unwrap_or(2),
    });
}

/// Puts the game's mouse on the stage while the tab is shown, and takes it off while it isn't
/// (a hidden player otherwise hit-tests the whole stage on every update: `give_mouse`).
fn set_mouse(player: &mut Player, on: bool) {
    if player.mouse_in_stage() != on {
        player.set_mouse_in_stage(on);
        if !on {
            player.handle_event(PlayerEvent::MouseLeave);
        }
    }
}

/// Ruffle's logical key for egui's name of it: named keys, letters and digits.
fn logical_key(name: &str) -> Option<(LogicalKey, KeyLocation)> {
    let named = match name {
        "Enter" => NamedKey::Enter,
        "Tab" => NamedKey::Tab,
        "Backspace" => NamedKey::Backspace,
        "Delete" => NamedKey::Delete,
        "Escape" => NamedKey::Escape,
        "Space" => return Some((LogicalKey::Character(' '), KeyLocation::Standard)),
        "ArrowUp" => NamedKey::ArrowUp,
        "ArrowDown" => NamedKey::ArrowDown,
        "ArrowLeft" => NamedKey::ArrowLeft,
        "ArrowRight" => NamedKey::ArrowRight,
        "Home" => NamedKey::Home,
        "End" => NamedKey::End,
        "PageUp" => NamedKey::PageUp,
        "PageDown" => NamedKey::PageDown,
        "Insert" => NamedKey::Insert,
        "F1" => NamedKey::F1,
        "F2" => NamedKey::F2,
        "F3" => NamedKey::F3,
        "F4" => NamedKey::F4,
        "F5" => NamedKey::F5,
        "F6" => NamedKey::F6,
        "F7" => NamedKey::F7,
        "F8" => NamedKey::F8,
        "F9" => NamedKey::F9,
        "F10" => NamedKey::F10,
        "F11" => NamedKey::F11,
        "F12" => NamedKey::F12,
        other => {
            let mut chars = other.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => Some((
                    LogicalKey::Character(c.to_ascii_lowercase()),
                    KeyLocation::Standard,
                )),
                _ => None,
            };
        }
    };
    Some((LogicalKey::Named(named), KeyLocation::Standard))
}

/// What a key does in a game text field instead of typing (Backspace, arrows, Ctrl+V).
fn text_control(key: LogicalKey, mods: Mods) -> Option<TextControlCode> {
    use TextControlCode as C;
    let named = match key {
        LogicalKey::Character(c) if mods.command => {
            return match c {
                'a' => Some(C::SelectAll),
                'c' => Some(C::Copy),
                'v' => Some(C::Paste),
                'x' => Some(C::Cut),
                _ => None,
            };
        }
        LogicalKey::Named(named) => named,
        _ => return None,
    };
    let word = mods.alt || (mods.ctrl && !cfg!(target_os = "macos"));
    Some(match (named, mods.shift, word) {
        (NamedKey::Enter, ..) => C::Enter,
        (NamedKey::Backspace, _, false) => C::Backspace,
        (NamedKey::Backspace, _, true) => C::BackspaceWord,
        (NamedKey::Delete, _, false) => C::Delete,
        (NamedKey::Delete, _, true) => C::DeleteWord,
        (NamedKey::ArrowLeft, false, false) => C::MoveLeft,
        (NamedKey::ArrowLeft, true, false) => C::SelectLeft,
        (NamedKey::ArrowLeft, false, true) => C::MoveLeftWord,
        (NamedKey::ArrowLeft, true, true) => C::SelectLeftWord,
        (NamedKey::ArrowRight, false, false) => C::MoveRight,
        (NamedKey::ArrowRight, true, false) => C::SelectRight,
        (NamedKey::ArrowRight, false, true) => C::MoveRightWord,
        (NamedKey::ArrowRight, true, true) => C::SelectRightWord,
        (NamedKey::Home, false, _) => C::MoveLeftLine,
        (NamedKey::Home, true, _) => C::SelectLeftLine,
        (NamedKey::End, false, _) => C::MoveRightLine,
        (NamedKey::End, true, _) => C::SelectRightLine,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_ruffle() {
        assert_eq!(
            logical_key("A"),
            Some((LogicalKey::Character('a'), KeyLocation::Standard))
        );
        assert_eq!(
            logical_key("Enter").map(|(key, _)| key),
            Some(LogicalKey::Named(NamedKey::Enter))
        );
        assert_eq!(logical_key("Minus"), None);
        let shift = Mods {
            shift: true,
            ..Mods::default()
        };
        assert_eq!(
            text_control(LogicalKey::Named(NamedKey::ArrowLeft), shift),
            Some(TextControlCode::SelectLeft)
        );
        let command = Mods {
            command: true,
            ..Mods::default()
        };
        assert_eq!(
            text_control(LogicalKey::Character('v'), command),
            Some(TextControlCode::Paste)
        );
    }
}
