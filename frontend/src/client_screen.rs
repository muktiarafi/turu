//! Turu: a client's screen (docs/TAB_PROCESSES_PROPOSAL.md, M2). The client mode runs one tab
//! without a window: this draws the game and Turu's UI over it into a texture the size the shell
//! asked for ([`ToClient::Show`]), reads it back without making the engine wait, and hands each
//! picture to a sink (the client sends it to the shell). Input comes from the shell's messages
//! and goes, as on the desktop, to Turu's UI when a press starts on it, else to the game.
//!
//! With [`SharedSurfaces`] (macOS, M5) the picture is drawn straight into one of a few GPU surfaces
//! the shell can open too, and only its id goes out once the GPU has finished it: no readback,
//! no copy through the CPU. The shell hands each back ([`ToClient::Release`]) when it has a newer
//! one.
//!
//! With frame interpolation on (Options > Application), the tab asks the shell to present on every
//! refresh of the display ([`Pace::Refresh`]) and draws a new in-between picture on the refreshes
//! [`Pacing`] picks, as the window does, when the shell says it presented ([`ToClient::Refreshed`]):
//! its pictures keep in step with the display, which a timer of the client's own would drift
//! against. Uncapped ([`Pace::Uncapped`], Mugetsu 2.0.0), each new picture goes out as soon as
//! the last is done, or at the FPS limit. Only with shared surfaces: pixels read back through the
//! CPU stay at one picture per game frame.
//!
//! Android's screen (`android/src/screen.rs`) draws the same way into a window; the two share
//! [`crate::movie_texture`]. Everything here runs on the engine thread.

use crate::engine::Step;
use crate::gui::turu::TuruUi;
use crate::movie_texture::{Blit, MovieTexture};
use crate::pacing::Pacing;
use crate::shell::{Button, Cursor, GridShow, Mods, Pace, PointerKind, SHARED_POOL, ToClient};
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
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// The format of the pictures: what macOS and Windows windows take as they are.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// The shortest time between two pictures of the UI alone (30 a second).
const UI_FRAME: Duration = Duration::from_millis(33);
/// How often a readback is looked at while one is under way; while interpolating, every ms (4 ms
/// is half a refresh at 120 Hz).
const READBACK_POLL: Duration = Duration::from_millis(4);
const READBACK_POLL_PACED: Duration = Duration::from_millis(1);
/// Without the shell's refreshes for this long (another tab shown, the window covered, an older
/// shell), the tab draws once per game frame again.
const REFRESHES_LOST: Duration = Duration::from_millis(100);

/// A finished picture, for the shell.
pub enum Picture {
    /// Its BGRA rows, read back.
    Pixels {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    /// In shared surface `id`, finished on the GPU.
    Shared { id: u64, width: u32, height: u32 },
}

/// Where each picture goes.
pub type FrameSink = Box<dyn FnMut(Picture)>;

/// Makes GPU surfaces the shell can open too (the client's, by platform: `app/src/client.rs`).
pub trait SharedSurfaces {
    /// A new `FORMAT` texture of `size` that can be drawn into, already handed to the shell, with
    /// its id; `None` if that failed (the client goes back to sending pixels).
    fn make(&mut self, device: &wgpu::Device, size: (u32, u32)) -> Option<(u64, wgpu::Texture)>;
}

/// The system clipboard, for Turu's text boxes (the client's, by platform).
pub trait Clipboard {
    fn get(&mut self) -> Option<String>;
    fn set(&mut self, text: String);
}

/// One of the shared surfaces drawn into in turn.
struct SharedSurface {
    id: u64,
    texture: wgpu::Texture,
    size: (u32, u32),
    /// Not the shell's: free to draw into.
    free: bool,
}

/// The texture drawn into and the buffer it's read back through, at one size.
struct Target {
    texture: wgpu::Texture,
    buffer: Arc<wgpu::Buffer>,
    size: (u32, u32),
    /// A row in the buffer, padded to wgpu's 256 bytes.
    padded_row: u32,
}

/// A picture under way: a readback (done when the buffer is mapped), or a shared surface (done
/// when the GPU finished the work that drew it).
enum Readback {
    Pixels {
        buffer: Arc<wgpu::Buffer>,
        size: (u32, u32),
        padded_row: u32,
        mapped: Arc<AtomicBool>,
    },
    Shared {
        id: u64,
        size: (u32, u32),
        done: Arc<AtomicBool>,
    },
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
    /// Draws into surfaces shared with the shell, when it offered them.
    shared: Option<Box<dyn SharedSurfaces>>,
    pool: Vec<SharedSurface>,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    /// The button held, and whether its press went to the UI (else the game).
    pressed: Option<(Button, bool)>,
    /// The game's viewport, last set, and Performance mode's share of the game's area it was
    /// drawn at (`Platform::render_scale`).
    movie_size: (u32, u32),
    render_scale: f64,
    started: Instant,
    last_draw: Instant,
    /// When the UI wants drawing again, if it does.
    ui_due: Option<Instant>,
    /// The game's volume while it's the tab on screen (else muted).
    volume: f32,
    /// The shell's window is covered or minimized: nothing is drawn.
    covered: bool,
    /// The pointer is over the window (moved there, not left since).
    pointer_in: bool,
    /// Where the pointer is, in the picture's pixels, while it's in.
    pointer_at: Option<(f32, f32)>,
    /// Hands the shell the cursor's shape when it changes (`set_cursor_sink`), and the last one.
    cursor_sink: Option<Box<dyn Fn(Cursor)>>,
    /// None until sent since the tab was last shown (the shell may show another's).
    cursor: Option<Cursor>,
    clipboard: Option<Box<dyn Clipboard>>,
    /// A new picture of the game is wanted (a resize, a frame not drawn because a readback was
    /// under way).
    picture_due: bool,
    /// Pictures of the game drawn since `fps_since`, and the last second's rate, for Show FPS.
    fps_frames: u32,
    fps_since: Instant,
    fps: f32,
    /// Frame interpolation's pacing, with the display's refresh rate from the shell.
    pacing: Pacing,
    /// How the shell presents this tab's pictures, as last asked (`ToShell::Pace`), and where
    /// that goes (`set_pace_sink`).
    pace: Pace,
    pace_sink: Option<Box<dyn Fn(Pace)>>,
    /// The shell's last refresh not drawn on yet (`ToClient::Refreshed`), and the last one.
    refreshed: Option<Instant>,
    last_refreshed: Option<Instant>,
    /// Uncapped: at most a new picture every this long (the FPS limit), if there is one.
    uncapped_interval: Option<Duration>,
    /// New pictures due on the shell's refreshes but skipped (the last still under way, or no
    /// surface free), for the perf log: the pacer doesn't see them.
    skipped: u32,
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
            shared: None,
            pool: Vec::new(),
            events: Vec::new(),
            modifiers: egui::Modifiers::NONE,
            pressed: None,
            movie_size: (0, 0),
            render_scale: 1.0,
            started: Instant::now(),
            last_draw: Instant::now(),
            ui_due: None,
            picture_due: false,
            volume: 1.0,
            covered: false,
            pointer_in: false,
            pointer_at: None,
            cursor_sink: None,
            cursor: None,
            clipboard: None,
            fps_frames: 0,
            fps_since: Instant::now(),
            fps: 0.0,
            pacing: Pacing::new(),
            pace: Pace::Frames,
            pace_sink: None,
            refreshed: None,
            last_refreshed: None,
            uncapped_interval: None,
            skipped: 0,
        }
    }

    /// The game's volume while the tab is on screen (Mugetsu's volume and mute); muted otherwise.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    /// Where Turu's text boxes copy to and paste from.
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.clipboard = Some(clipboard);
    }

    /// The game has the mouse on its stage only while the pointer is over the tab on screen (the
    /// active one in Grid View) and the window is seen, as the window's tabs do
    /// (`PlayerController::give_mouse`): otherwise every update would hit-test the whole stage for
    /// a mouse nobody moves (`run_mouse_pick`, most of a covered client's tick).
    fn follow_mouse(&self, player: &mut Player) {
        let tile = self.grid.as_ref().is_some_and(|grid| !grid.active);
        set_mouse(
            player,
            self.shown.is_some() && !tile && !self.covered && self.pointer_in,
        );
    }

    /// Where the cursor's shape goes when it changes (the shell, `ToShell::Cursor`).
    pub fn set_cursor_sink(&mut self, sink: Box<dyn Fn(Cursor)>) {
        self.cursor_sink = Some(sink);
    }

    /// The cursor over the game or Turu's UI, sent when it changed: Turu's where the pointer is on
    /// its UI (or egui asks for one), else the game's (`set_game_cursor`).
    fn update_cursor(&mut self, ui_cursor: egui::CursorIcon) {
        let on_ui = self.pointer_at.is_some_and(|(x, y)| self.on_ui(x, y));
        let cursor = if on_ui || ui_cursor != egui::CursorIcon::Default {
            match ui_cursor {
                egui::CursorIcon::PointingHand => Cursor::Hand,
                egui::CursorIcon::Text | egui::CursorIcon::VerticalText => Cursor::Text,
                egui::CursorIcon::Grab => Cursor::Grab,
                egui::CursorIcon::Grabbing => Cursor::Grabbing,
                egui::CursorIcon::Move | egui::CursorIcon::AllScroll => Cursor::Move,
                egui::CursorIcon::NotAllowed | egui::CursorIcon::NoDrop => Cursor::NotAllowed,
                egui::CursorIcon::Wait | egui::CursorIcon::Progress => Cursor::Wait,
                egui::CursorIcon::ResizeHorizontal
                | egui::CursorIcon::ResizeColumn
                | egui::CursorIcon::ResizeEast
                | egui::CursorIcon::ResizeWest => Cursor::ResizeHorizontal,
                egui::CursorIcon::ResizeVertical
                | egui::CursorIcon::ResizeRow
                | egui::CursorIcon::ResizeNorth
                | egui::CursorIcon::ResizeSouth => Cursor::ResizeVertical,
                egui::CursorIcon::ResizeNwSe
                | egui::CursorIcon::ResizeNorthWest
                | egui::CursorIcon::ResizeSouthEast => Cursor::ResizeNwSe,
                egui::CursorIcon::ResizeNeSw
                | egui::CursorIcon::ResizeNorthEast
                | egui::CursorIcon::ResizeSouthWest => Cursor::ResizeNeSw,
                egui::CursorIcon::None => Cursor::None,
                _ => Cursor::Default,
            }
        } else {
            game_cursor()
        };
        if Some(cursor) != self.cursor {
            self.cursor = Some(cursor);
            if let Some(sink) = &self.cursor_sink {
                sink(cursor);
            }
        }
    }

    /// Tells the shell how to present the tab's pictures when that changes (`ToShell::Pace`).
    pub fn set_pace_sink(&mut self, sink: Box<dyn Fn(Pace)>) {
        self.pace_sink = Some(sink);
    }

    /// Draws into surfaces shared with the shell instead of reading pictures back.
    pub fn share_surfaces(&mut self, shared: Box<dyn SharedSurfaces>) {
        self.shared = Some(shared);
    }

    /// Draws the UI again soon (the shell's tab list changed, say).
    pub fn request_repaint(&mut self) {
        if self.shown.is_some() {
            self.ui_due = Some(Instant::now());
        }
    }

    /// The game's pictures drawn a second, over the last second (Show FPS's figure).
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// New pictures skipped since the last call (see `skipped`).
    pub fn take_skipped(&mut self) -> u32 {
        std::mem::take(&mut self.skipped)
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
                // Its cursor goes to the shell again, as the last tab shown may have left another.
                self.cursor = None;
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
                // Only the tab on screen is heard (in Grid View, the active one).
                player.set_volume(if tile { 0.0 } else { self.volume });
                turu::bot().state.lock().hidden_tab = tile;
                set_grid(grid.as_ref());
                self.grid = grid;
                self.follow_mouse(player);
            }
            ToClient::Hide => {
                set_grid(None);
                self.grid = None;
                self.shown = None;
                self.target = None;
                self.ui_due = None;
                self.picture_due = false;
                // A hidden tab keeps no surfaces (the shell lets go of its own).
                self.pool.clear();
                if matches!(self.readback, Some(Readback::Shared { .. })) {
                    self.readback = None;
                }
                self.pressed = None;
                self.pointer_in = false;
                set_mouse(player, false);
                player.set_volume(0.0);
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
            ToClient::NoSurfaces { reason } => {
                if self.shared.is_some() {
                    tracing::warn!(
                        "The shell can't use shared surfaces ({reason}): sending pixels"
                    );
                }
                self.shared = None;
                self.pool.clear();
                if matches!(self.readback, Some(Readback::Shared { .. })) {
                    self.readback = None;
                }
                if self.shown.is_some() {
                    self.picture_due = true;
                    self.ui_due = Some(Instant::now());
                }
            }
            ToClient::Covered { covered } => {
                self.covered = covered;
                self.follow_mouse(player);
                // Seen again: a picture at once.
                if !covered && self.shown.is_some() {
                    self.picture_due = true;
                    self.ui_due = Some(Instant::now());
                }
            }
            ToClient::Release { id } => {
                if let Some(surface) = self.pool.iter_mut().find(|surface| surface.id == id) {
                    surface.free = true;
                }
                // A picture waiting for a free surface.
                if self.picture_due {
                    self.ui_due = Some(Instant::now());
                }
            }
            ToClient::RefreshRate { millihertz } => {
                if self.pacing.set_refresh_rate(millihertz) {
                    tracing::info!(
                        target: "ruffle_aqw_perf",
                        "display refreshes at {:.1} Hz",
                        f64::from(millihertz) / 1000.0
                    );
                }
            }
            ToClient::Refreshed => {
                let now = Instant::now();
                self.refreshed = Some(now);
                self.last_refreshed = Some(now);
            }
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
            // The client's own (`app/src/client.rs`), or Android's (its slots): not the screen's.
            ToClient::Tabs { .. }
            | ToClient::Answer { .. }
            | ToClient::Dump { .. }
            | ToClient::FieldEdited { .. }
            | ToClient::FieldEnter { .. }
            | ToClient::FieldClosed { .. }
            | ToClient::Insets { .. }
            | ToClient::DocumentPicked { .. }
            | ToClient::DocumentCreated { .. }
            | ToClient::Installed { .. }
            | ToClient::AndroidKey { .. }
            | ToClient::HeadlessMode { .. }
            | ToClient::Updates { .. }
            | ToClient::ArmyClients { .. }
            | ToClient::FullScreen { .. }
            | ToClient::Unknown => {}
        }
        if self.shown.is_some() && !self.events.is_empty() {
            self.ui_due = Some(Instant::now());
        }
    }

    /// After a tick or at the time asked: collects a finished readback, and draws when the game
    /// has a new frame or the UI wants it, or with frame interpolation when the pace says (see the
    /// module's comment). When to be called again, if before the next tick.
    pub fn step(&mut self, player: &mut Player, step: Step) -> Option<Instant> {
        if let Step::Ticked { frame_ms } = step {
            if let Some(frame_ms) = frame_ms {
                self.pacing.record_frame(frame_ms);
            }
            self.plan_pace(player);
        }
        self.collect();
        let mut next = None;
        if self.shown.is_some() && !self.covered {
            let now = Instant::now();
            let game_frame = matches!(step, Step::Ticked { .. }) && player.needs_render();
            // The UI alone is drawn 30 times a second at most, however often egui asks.
            let ui_wanted = self
                .ui_due
                .is_some_and(|due| due.max(self.last_draw + UI_FRAME) <= now);
            let in_step = self
                .last_refreshed
                .is_some_and(|at| now.duration_since(at) < REFRESHES_LOST);
            match self.pace {
                // A picture still under way when a new one is due: that one is skipped, not
                // caught up on (which would break the rhythm twice).
                // Drawn only on the shell's refreshes, the UI too, so every picture keeps the
                // rhythm. A picture still under way, or no surface free, when a new one is due:
                // that one is skipped, not caught up on (which would break the rhythm twice).
                Pace::Refresh if in_step => {
                    if let Some(at) = self.refreshed.take() {
                        if self.readback.is_none() {
                            let new_picture = self.pacing.new_picture_due(at);
                            let wanted = self.picture_due;
                            if new_picture || wanted || ui_wanted {
                                self.picture_due = false;
                                let drawn_before = self.last_draw;
                                self.draw(player, new_picture || wanted);
                                if new_picture && self.last_draw == drawn_before {
                                    self.skipped += 1;
                                }
                                self.picture_due &= wanted;
                            }
                        } else if self.pacing.new_picture_due(at) {
                            self.skipped += 1;
                        }
                    }
                }
                Pace::Uncapped => {
                    if self.readback.is_none() {
                        let new_picture = self
                            .pacing
                            .new_uncapped_picture_due(now, self.uncapped_interval);
                        if new_picture || self.picture_due || ui_wanted {
                            let picture = new_picture || self.picture_due;
                            self.picture_due = false;
                            self.draw(player, picture);
                        }
                    }
                    // Without a limit, or while a picture is under way (its time may be past: the
                    // loop would spin on it), the readback's poll below calls again.
                    next = self
                        .uncapped_interval
                        .filter(|_| self.readback.is_none())
                        .map(|interval| self.pacing.next_uncapped_picture(interval));
                }
                _ => {
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
            }
        }
        let poll = match self.pace {
            Pace::Refresh | Pace::Uncapped => READBACK_POLL_PACED,
            Pace::Frames | Pace::Unknown => READBACK_POLL,
        };
        let readback = self.readback.as_ref().map(|_| Instant::now() + poll);
        // Nothing is drawn while hidden: a repaint due from before must not wake the loop (it was
        // in the past, and the engine spun on it).
        let seen = self.shown.is_some() && !self.covered;
        let ui = self
            .ui_due
            .filter(|_| seen)
            .map(|due| due.max(self.last_draw + UI_FRAME));
        [readback, ui, next.filter(|_| seen)]
            .into_iter()
            .flatten()
            .min()
    }

    /// After a tick: how the shell should present this tab's pictures (see the module's comment),
    /// told to it when that changes. Interpolating needs the tab seen whole (not another tab's
    /// tile, not Headless Mode's screen), the game playing, and shared surfaces.
    fn plan_pace(&mut self, player: &Player) {
        let tile = self.grid.as_ref().is_some_and(|grid| !grid.active);
        let interpolates = self.shown.is_some()
            && !self.covered
            && !tile
            && self.shared.is_some()
            && !turu::headless_mode()
            && ruffle_core::frame_interpolation::enabled()
            && player.is_playing();
        let frame_rate = player.frame_rate();
        let pace = if !interpolates {
            Pace::Frames
        } else if let Some((true, setting)) = crate::platform::platform().frame_pacing() {
            let limit = crate::pacing::fps_limit(setting, self.pacing.refresh_interval());
            self.uncapped_interval = limit.map(|fps| Duration::from_secs_f64(1.0 / fps));
            // Not even 1.5 pictures per game frame: one per frame, as without interpolation.
            if limit.is_some_and(|fps| fps < frame_rate * 1.5) {
                Pace::Frames
            } else {
                Pace::Uncapped
            }
        } else if self.pacing.plan(Instant::now(), frame_rate) {
            Pace::Refresh
        } else {
            Pace::Frames
        };
        if pace == self.pace {
            return;
        }
        tracing::info!(
            target: "ruffle_aqw_perf",
            "frame pacing: {pace:?} (new pictures a second: {})",
            match pace {
                Pace::Refresh => format!("{:.0}", self.pacing.rate()),
                Pace::Uncapped => self
                    .uncapped_interval
                    .map_or("as many as get through".into(), |interval| {
                        format!("{:.0}", 1.0 / interval.as_secs_f64())
                    }),
                Pace::Frames | Pace::Unknown => format!("{frame_rate:.0}, one per game frame"),
            }
        );
        self.pace = pace;
        self.refreshed = None;
        self.last_refreshed = None;
        if let Some(sink) = &self.pace_sink {
            sink(pace);
        }
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

    /// Hands a finished picture to the sink.
    fn collect(&mut self) {
        let Some(readback) = &self.readback else {
            return;
        };
        let _ = self.descriptors.device.poll(wgpu::PollType::Poll);
        let done = match readback {
            Readback::Pixels { mapped, .. } => mapped,
            Readback::Shared { done, .. } => done,
        };
        if !done.load(Ordering::Acquire) {
            return;
        }
        match self.readback.take().expect("checked above") {
            Readback::Pixels {
                buffer,
                size: (width, height),
                padded_row,
                ..
            } => {
                let row = width as usize * 4;
                let mut pixels = Vec::with_capacity(row * height as usize);
                match buffer.slice(..).get_mapped_range() {
                    Ok(data) => {
                        for y in 0..height as usize {
                            let start = y * padded_row as usize;
                            pixels.extend_from_slice(&data[start..start + row]);
                        }
                    }
                    Err(e) => tracing::warn!("Couldn't read the picture back: {e}"),
                }
                buffer.unmap();
                (self.sink)(Picture::Pixels {
                    width,
                    height,
                    pixels,
                });
            }
            Readback::Shared {
                id,
                size: (width, height),
                ..
            } => (self.sink)(Picture::Shared { id, width, height }),
        }
    }

    /// A shared surface of `size` to draw into, taken from the shell's: `Ok(None)` without shared
    /// surfaces, `Err` while the shell has every one.
    fn take_surface(&mut self, size: (u32, u32)) -> Result<Option<(u64, wgpu::Texture)>, ()> {
        if self.shared.is_none() {
            return Ok(None);
        }
        self.pool.retain(|surface| surface.size == size);
        if let Some(surface) = self.pool.iter_mut().find(|surface| surface.free) {
            surface.free = false;
            return Ok(Some((surface.id, surface.texture.clone())));
        }
        if self.pool.len() >= SHARED_POOL {
            return Err(());
        }
        let made = self
            .shared
            .as_mut()
            .and_then(|shared| shared.make(&self.descriptors.device, size));
        let Some((id, texture)) = made else {
            tracing::warn!("No shared surface for the shell: sending pixels instead");
            self.shared = None;
            self.pool.clear();
            return Ok(None);
        };
        self.pool.push(SharedSurface {
            id,
            texture: texture.clone(),
            size,
            free: false,
        });
        tracing::info!(
            target: "ruffle_aqw_perf",
            "shared surface {} of at most {SHARED_POOL} made ({}x{})",
            self.pool.len(),
            size.0,
            size.1
        );
        Ok(Some((id, texture)))
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

    /// In Grid View, as the active tab: the other tab whose tile is at (`x`, `y`) with nothing of
    /// Turu's over it (its label is the tile's).
    fn other_tile_at(&self, x: f32, y: f32) -> Option<u32> {
        let grid = self.grid.as_ref().filter(|grid| grid.active)?;
        let tile = grid.tiles.iter().find(|tile| {
            let (x0, y0) = (tile.x as f32, tile.y as f32);
            x >= x0 && y >= y0 && x < x0 + tile.width as f32 && y < y0 + tile.height as f32
        })?;
        let covered = self
            .egui
            .layer_id_at(self.point(x, y))
            .is_some_and(|layer| layer.order != egui::Order::Background);
        (tile.id != grid.mine && !covered).then_some(tile.id)
    }

    fn pointer(&mut self, player: &mut Player, kind: PointerKind, button: Button, x: f32, y: f32) {
        let pos = self.point(x, y);
        self.pointer_at = (!matches!(kind, PointerKind::Leave)).then_some((x, y));
        let (left, top) = self.offsets();
        // (In the game's own pixels: fewer in Performance mode, see `draw`.)
        let (game_x, game_y) = (
            f64::from(x - left) * self.render_scale,
            f64::from(y - top) * self.render_scale,
        );
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
                if !self.pointer_in {
                    self.pointer_in = true;
                    self.follow_mouse(player);
                }
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
                // Grid View: a press on another tab's tile, where none of Turu's windows covers it,
                // makes that tab the active one (VibeSkua's tabbed host's click); its release
                // goes nowhere.
                if let Some(id) = self.other_tile_at(x, y) {
                    self.pressed = Some((button, true));
                    turu::shell_tab_action(turu::ShellTabAction::Select(id));
                    return;
                }
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
                self.pointer_in = false;
                self.follow_mouse(player);
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
        // Cut, copy and paste in Turu's text boxes, as egui-winit makes them in a window.
        if pressed && mods.command && self.egui.egui_wants_keyboard_input() {
            let event = match name {
                "X" => Some(egui::Event::Cut),
                "C" => Some(egui::Event::Copy),
                "V" => self
                    .clipboard
                    .as_mut()
                    .and_then(|clipboard| clipboard.get())
                    .map(egui::Event::Paste),
                _ => None,
            };
            if let Some(event) = event {
                self.events.push(event);
                return;
            }
        }
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
        let Ok(surface) = self.take_surface(size) else {
            // The shell has every surface: drawn once it lets one go (`Release` wakes it), not
            // asked again meanwhile.
            self.picture_due = true;
            self.ui_due = None;
            return;
        };
        if surface.is_none()
            && self
                .target
                .as_ref()
                .is_none_or(|target| target.size != size)
        {
            self.target = Some(self.make_target(size));
        }
        self.last_draw = Instant::now();
        let (left, top) = self.offsets();
        let movie_size = self.movie_size(size);
        // Mugetsu 2.0.0's Performance mode: the game drawn with fewer pixels, stretched over its
        // area by the blit below.
        let scale = crate::platform::platform().render_scale();
        if movie_size != self.movie_size || scale != self.render_scale {
            self.movie_size = movie_size;
            self.render_scale = scale;
            let drawn = |len: u32| {
                if scale == 1.0 {
                    len
                } else {
                    ((f64::from(len) * scale).round() as u32).max(1)
                }
            };
            player.set_viewport_dimensions(ViewportDimensions {
                width: drawn(movie_size.0),
                height: drawn(movie_size.1),
                scale_factor: f64::from(ppp) * scale,
            });
            new_picture = true;
        }
        if new_picture && !turu::headless_mode() {
            let render_started = Instant::now();
            player.render();
            self.pacing
                .record_draw(render_started.elapsed().as_secs_f64() * 1000.0);
            // Pictures of the game a second, for Show FPS (the window counts its frames shown).
            self.fps_frames += 1;
            let elapsed = self.fps_since.elapsed().as_secs_f32();
            if elapsed >= 1.0 {
                self.fps = self.fps_frames as f32 / elapsed;
                self.fps_frames = 0;
                self.fps_since = Instant::now();
            }
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
        let fps = (crate::platform::platform().show_fps() == Some(true)).then_some(self.fps);
        let output = self.egui.run_ui(raw_input, |root| {
            if tile {
                return;
            }
            ui.show_bar(root);
            if let Some(fps) = fps {
                crate::gui::turu::fps_badge(root.ctx(), top_points, fps);
            }
            if turu::headless_mode() {
                crate::gui::turu::show_headless_screen(root.ctx(), top_points);
            }
            ui.show_windows(root.ctx());
        });
        self.update_cursor(output.platform_output.cursor_icon);
        self.ui_due = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .and_then(|viewport| Instant::now().checked_add(viewport.repaint_delay));
        for command in output.platform_output.commands {
            match command {
                egui::OutputCommand::OpenUrl(open) => {
                    crate::platform::platform().open_url(&open.url)
                }
                egui::OutputCommand::CopyText(text) => {
                    if let Some(clipboard) = &mut self.clipboard {
                        clipboard.set(text);
                    }
                }
                egui::OutputCommand::CopyImage(_) => {}
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
        let target = match &surface {
            Some(_) => None,
            None => Some(self.target.as_ref().expect("made above")),
        };
        {
            let view = match (&surface, target) {
                (Some((_, texture)), _) => texture.create_view(&Default::default()),
                (None, Some(target)) => target.texture.create_view(&Default::default()),
                (None, None) => unreachable!("a target is made without a surface"),
            };
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
        if let Some(target) = target {
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
        }
        buffers.push(encoder.finish());
        queue.submit(buffers);
        for id in &textures.free {
            self.egui_renderer.free_texture(id);
        }
        // Applied: epaint checks (in debug builds) that none is dropped unhandled.
        textures.clear();
        let Some(target) = target else {
            // The shell gets the surface once the GPU has finished drawing it.
            let (id, _) = surface.expect("no target without a surface");
            let done = Arc::new(AtomicBool::new(false));
            let finished = done.clone();
            queue.on_submitted_work_done(move || finished.store(true, Ordering::Release));
            self.readback = Some(Readback::Shared { id, size, done });
            return;
        };
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
        self.readback = Some(Readback::Pixels {
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
        None => 2,
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

/// The game's mouse cursor (Flash's `Mouse.cursor`, buttons' hand), set by the client's UI backend
/// (`set_game_cursor`) from the player, which runs outside the screen's calls.
static GAME_CURSOR: AtomicU8 = AtomicU8::new(0);

/// The game asks for `cursor` (its UI backend's `set_mouse_cursor`).
pub fn set_game_cursor(cursor: Cursor) {
    let index = match cursor {
        Cursor::Hand => 1,
        Cursor::Text => 2,
        Cursor::Grab => 3,
        _ => 0,
    };
    GAME_CURSOR.store(index, Ordering::Relaxed);
}

fn game_cursor() -> Cursor {
    match GAME_CURSOR.load(Ordering::Relaxed) {
        1 => Cursor::Hand,
        2 => Cursor::Text,
        3 => Cursor::Grab,
        _ => Cursor::Default,
    }
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
