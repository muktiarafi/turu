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
use crate::shell::{
    Button, Cursor, GameKey, GridShow, ImeInput, Mods, Pace, PointerKind, SHARED_POOL, ToClient,
};
use crate::turu;
use ruffle_core::ViewportDimensions;
use ruffle_core::events::{
    GamepadButton, ImeCursorArea, ImeEvent, ImeNotification, ImePurpose, KeyDescriptor,
    KeyLocation, LogicalKey, MouseButton, MouseWheelDelta, NamedKey, PhysicalKey, TextControlCode,
};
use ruffle_core::{Player, PlayerEvent};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::render_thread;
use ruffle_render_wgpu::wgpu;
use std::collections::VecDeque;
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
/// Turu: shared pictures under way at once. One at a time, a picture's whole way (its draw, the
/// GPU's work, the shell's turn: ~30 ms in a crowded room on built-in graphics) was the time
/// between two, and frame interpolation gave up there; three keep the CPU and the GPU busy at
/// once, as the window's render and present threads do, and leave the shell two of the
/// [`SHARED_POOL`].
const SHARED_IN_FLIGHT: usize = 3;
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

/// Tells the shell what input method the tab wants: allowed, a password's, the text's area in the
/// picture's pixels (`ToShell::Ime`).
pub type ImeSink = Box<dyn Fn(bool, bool, Option<[f32; 4]>)>;

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
        /// When it was drawn (for the pacer: its way to finished).
        drawn: Instant,
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
    /// Pictures under way, oldest first: one readback of pixels at a time, or up to
    /// [`SHARED_IN_FLIGHT`] shared surfaces (the GPU finishes them in order).
    readbacks: VecDeque<Readback>,
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
    /// The game's pictures over 5-second windows, for the graphics tips (`crate::graphics_tips`).
    pictures: crate::graphics_tips::PictureCount,
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
    /// The game's texture as the blit draws it, and the renderer's target state it was read at
    /// (`render_thread::target_state`; None without a render thread, read each time then):
    /// reading it behind a render thread waits for the picture being recorded.
    movie: Option<(Option<u64>, wgpu::BindGroup)>,
    /// An input method over the shell's window (`set_ime_sink`): whether the game's text fields
    /// may have one (Ruffle's IME preference), the game's focused field's (a password's or not,
    /// its area in the picture's pixels), Turu's focused text box's area, what the shell was told
    /// last, and whether a composition is under way (its keys are the input method's).
    ime_sink: Option<ImeSink>,
    game_ime_allowed: bool,
    game_ime: Option<(bool, ImeCursorArea)>,
    ui_ime: Option<([f32; 4], bool)>,
    ime_sent: Option<(bool, bool, Option<[f32; 4]>)>,
    composing: bool,
    /// The keys the game has down (pressed here, not yet let go), and whether it has the focus (as
    /// the shell last said): let go of and lost when the tab stops taking input (`stop_input`).
    held_keys: Vec<KeyDescriptor>,
    game_focused: bool,
    /// Flash's right-click menu, while it's open (see `open_menu`).
    menu: Option<Menu>,
    /// Mugetsu's present thread, when the game's pictures are finished and submitted there (see
    /// `set_present_thread`): this screen's own picture goes after each, through it.
    present: Option<Arc<ruffle_render_wgpu::present::PresentThread>>,
}

/// Flash's right-click menu (`Player::prepare_context_menu`), drawn by Turu's UI as the window's
/// right-click menu draws it: AQW turns Flash's own items off and, at its login screen, adds none,
/// so it shows only for items a game adds.
struct Menu {
    items: Vec<ruffle_core::ContextMenuItem>,
    /// Where it opens, in points, and the right button's release it holds back for the game,
    /// sent when it closes (in the game's pixels), as the window does.
    at: egui::Pos2,
    release: (f64, f64),
    /// Drawn once already: a new egui area is laid out unseen first, and the press that opened
    /// the menu must not close it.
    shown: bool,
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
            readbacks: VecDeque::new(),
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
            pictures: Default::default(),
            fps_frames: 0,
            fps_since: Instant::now(),
            fps: 0.0,
            pacing: {
                let mut pacing = Pacing::new();
                pacing.set_in_flight(SHARED_IN_FLIGHT as u32);
                pacing
            },
            pace: Pace::Frames,
            pace_sink: None,
            refreshed: None,
            last_refreshed: None,
            uncapped_interval: None,
            skipped: 0,
            movie: None,
            ime_sink: None,
            game_ime_allowed: false,
            game_ime: None,
            ui_ime: None,
            ime_sent: None,
            composing: false,
            held_keys: Vec::new(),
            game_focused: false,
            menu: None,
            present: None,
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

    /// Tells the shell whether its window takes an input method now (`ToShell::Ime`: allowed, a
    /// password's, the text's area), when that changes; `game` says whether the game's text fields
    /// may have one (Ruffle's IME preference, as the window has it). Turu's text boxes always may,
    /// as egui-winit lets them in a window.
    pub fn set_ime_sink(&mut self, game: bool, sink: ImeSink) {
        self.game_ime_allowed = game;
        self.ime_sink = Some(sink);
    }

    /// The game's focused text field wants an input method, or no longer does (the player's
    /// notification).
    pub fn game_ime(&mut self, player: &mut Player, notification: ImeNotification) {
        // Ruffle gives the field's area in the stage's pixels (AQW's 958x550); the game is drawn
        // scaled to its viewport, whose pixels the rest goes by. (The window's `app.rs` takes them
        // as the viewport's.)
        let view = player.mutate_with_update_context(|context| context.stage.view_matrix());
        let to_view = |area: ImeCursorArea| ImeCursorArea {
            x: f64::from(view.a) * area.x + view.tx.to_pixels(),
            y: f64::from(view.d) * area.y + view.ty.to_pixels(),
            width: f64::from(view.a) * area.width,
            height: f64::from(view.d) * area.height,
        };
        match notification {
            ImeNotification::ImeReady {
                purpose,
                cursor_area,
            } => {
                self.game_ime = Some((
                    matches!(purpose, ImePurpose::Password),
                    to_view(cursor_area),
                ))
            }
            ImeNotification::ImePurposeUpdated(purpose) => {
                if let Some((password, _)) = &mut self.game_ime {
                    *password = matches!(purpose, ImePurpose::Password);
                }
            }
            ImeNotification::ImeCursorAreaUpdated(area) => {
                if let Some((_, cursor_area)) = &mut self.game_ime {
                    *cursor_area = to_view(area);
                }
            }
            ImeNotification::ImeNotReady => self.game_ime = None,
        }
        self.sync_ime();
    }

    /// Tells the shell what input method the tab wants now, if that changed: Turu's focused text
    /// box's, else the game's focused field's (with the preference), and none while the tab
    /// isn't seen whole.
    fn sync_ime(&mut self) {
        let tile = self.grid.as_ref().is_some_and(|grid| !grid.active);
        let seen = self.shown.is_some() && !self.covered && !tile;
        let wanted = if !seen {
            (false, false, None)
        } else if let Some((area, password)) = self.ui_ime {
            (true, password, Some(area))
        } else if let Some((password, area)) =
            self.game_ime.as_ref().filter(|_| self.game_ime_allowed)
        {
            // The field's area is in the game's pixels: fewer in Performance mode, and offset by
            // where the game is in the picture.
            let (left, top) = self.offsets();
            let scale = self.render_scale as f32;
            (
                true,
                *password,
                Some([
                    left + area.x as f32 / scale,
                    top + area.y as f32 / scale,
                    area.width as f32 / scale,
                    area.height as f32 / scale,
                ]),
            )
        } else {
            (false, false, None)
        };
        if !wanted.0 {
            self.composing = false;
        }
        if self.ime_sent == Some(wanted) {
            return;
        }
        self.ime_sent = Some(wanted);
        if let Some(sink) = &self.ime_sink {
            sink(wanted.0, wanted.1, wanted.2);
        }
    }

    /// An input method's composition from the shell: Turu's focused text box's, as egui-winit
    /// hands it over in a window, else the game's, as the window's `app.rs` does.
    fn ime(&mut self, player: &mut Player, ime: ImeInput) {
        let to_ui = self.ui_ime.is_some() || self.egui.egui_wants_keyboard_input();
        match ime {
            ImeInput::Preedit { text, cursor } => {
                self.composing = !text.is_empty();
                if to_ui {
                    // egui wants the cursor's range in characters, winit gives it in bytes.
                    let active_range_chars = cursor.and_then(|(start, end)| {
                        let start_chars = text.get(..start)?.chars().count();
                        let middle_chars = text.get(start..end)?.chars().count();
                        Some(start_chars..start_chars + middle_chars)
                    });
                    self.events.push(egui::Event::Ime(egui::ImeEvent::Preedit {
                        text,
                        active_range_chars,
                    }));
                } else {
                    player.handle_event(PlayerEvent::Ime(ImeEvent::Preedit(text, cursor)));
                }
            }
            ImeInput::Commit { text } => {
                self.composing = false;
                if to_ui {
                    self.events
                        .push(egui::Event::Ime(egui::ImeEvent::Commit(text)));
                } else {
                    player.handle_event(PlayerEvent::Ime(ImeEvent::Commit(text)));
                }
            }
            // (As egui-winit, otherwise ignored: they mean different things on X11 and Wayland.)
            // A composition that ends without a commit (the window lost the focus) leaves the
            // keys to the tab again.
            ImeInput::Enabled | ImeInput::Disabled => self.composing = false,
            ImeInput::Unknown => {}
        }
    }

    /// A file dropped on the shell's window: a script (`.ts`, `.js`) is loaded, as Script Loader's
    /// Load Script loads it (not started), unless a script runs.
    fn dropped(&mut self, path: String) {
        let path = std::path::PathBuf::from(path);
        let bot = turu::bot();
        let script = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ts") || ext.eq_ignore_ascii_case("js"));
        if !script {
            bot.log(format!(
                "Dropped {}: only scripts (.ts, .js) are taken",
                path.display()
            ));
        } else if bot.scripts.status().is_active() {
            bot.log(format!("Dropped {}: stop the script first", path.display()));
        } else {
            // Turu: VibeSkua takes no dropped files; a script dropped on a tab is loaded.
            bot.scripts.load(&path);
            bot.log(format!("Loaded script {}", path.display()));
        }
        self.request_repaint();
    }

    /// A right press the game didn't take (on the game, not Turu's UI): Flash's menu opens there,
    /// if it has any items. The press's release is held back until it closes.
    fn open_menu(&mut self, player: &mut Player, at: egui::Pos2, release: (f64, f64)) -> bool {
        let items = player.prepare_context_menu();
        if items.is_empty() {
            return false;
        }
        self.menu = Some(Menu {
            items,
            at,
            release,
            shown: false,
        });
        self.ui_due = Some(Instant::now());
        true
    }

    /// Closes Flash's menu, if it's open: the right button's release goes to the game (once), and
    /// the item picked, if one was, runs.
    fn close_menu(&mut self, player: &mut Player, picked: Option<usize>) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        player.handle_event(PlayerEvent::MouseUp {
            x: menu.release.0,
            y: menu.release.1,
            button: MouseButton::Right,
        });
        if let Some(index) = picked {
            player.run_context_menu_callback(index);
        }
    }

    /// Tells the shell how to present the tab's pictures when that changes (`ToShell::Pace`).
    pub fn set_pace_sink(&mut self, sink: Box<dyn Fn(Pace)>) {
        self.pace_sink = Some(sink);
    }

    /// The present thread the game's renderer submits its pictures from (Mugetsu 2.0.0's
    /// `ruffle_render_wgpu::present`; Windows): the screen's own picture of each (the blit, Turu's
    /// UI, the readback or the shared surface's "done") is sent there after it, as the window's
    /// frame is in Mugetsu, or it would be drawn from the picture before.
    pub fn set_present_thread(&mut self, present: Arc<ruffle_render_wgpu::present::PresentThread>) {
        self.present = Some(present);
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

    /// For the perf log, once a picture's way has been measured (with frame interpolation): what
    /// the pacer reckons with.
    pub fn pacing_report(&self) -> Option<String> {
        let pipeline_ms = self.pacing.pipeline_ms();
        if pipeline_ms <= 0.0 {
            return None;
        }
        Some(format!(
            "{pipeline_ms:.1} ms from draw to finished ({SHARED_IN_FLIGHT} under way at most); draw {:.1} ms, render thread {:.1} ms; {}",
            self.pacing.draw_cost_ms(),
            self.pacing.thread_cost_ms(),
            match self.pace {
                Pace::Refresh => format!("aiming for {:.0} a second", self.pacing.rate()),
                pace => format!("{pace:?}"),
            }
        ))
    }

    /// New pictures skipped since the last call (see `skipped`).
    pub fn take_skipped(&mut self) -> u32 {
        std::mem::take(&mut self.skipped)
    }

    /// Whether the shell shows this tab.
    pub fn shown(&self) -> bool {
        self.shown.is_some()
    }

    /// A 5-second window of the game's pictures that has ended, for the graphics tips
    /// (`graphics_tips::PictureCount::take`).
    pub fn take_picture_window(&mut self) -> Option<Option<f64>> {
        self.pictures.take()
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
                turu::set_tab_shown(true);
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
                if tile {
                    self.stop_input(player);
                }
                // Only the tab on screen is heard (in Grid View, the active one).
                player.set_volume(if tile { 0.0 } else { self.volume });
                turu::bot().state.lock().hidden_tab = tile;
                set_grid(grid.as_ref());
                self.grid = grid;
                self.follow_mouse(player);
            }
            ToClient::Hide => {
                self.stop_input(player);
                set_grid(None);
                self.grid = None;
                self.shown = None;
                turu::set_tab_shown(false);
                self.target = None;
                self.ui_due = None;
                self.picture_due = false;
                // A hidden tab keeps no surfaces (the shell lets go of its own).
                self.pool.clear();
                self.drop_shared_readbacks();
                self.pressed = None;
                self.pointer_in = false;
                set_mouse(player, false);
                player.set_volume(0.0);
                turu::bot().state.lock().hidden_tab = true;
            }
            ToClient::Pointer { kind, button, x, y } => self.pointer(player, kind, button, x, y),
            ToClient::Wheel {
                dx,
                dy,
                x,
                y,
                pixels,
            } => {
                let pos = self.point(x, y);
                self.events.push(egui::Event::PointerMoved(pos));
                self.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(-dx, -dy),
                    modifiers: self.modifiers,
                    phase: egui::TouchPhase::Move,
                });
                if !self.on_ui(x, y) {
                    // A trackpad's in pixels, as the window gives them (Ruffle makes 100 a line).
                    let delta = if pixels {
                        MouseWheelDelta::Pixels(-f64::from(
                            dy * crate::shell::WHEEL_PIXELS_PER_LINE,
                        ))
                    } else {
                        MouseWheelDelta::Lines(-f64::from(dy))
                    };
                    player.handle_event(PlayerEvent::MouseWheel { delta });
                }
            }
            ToClient::Key {
                key,
                pressed,
                mods,
                game,
            } => self.key(player, &key, game.as_ref(), pressed, mods),
            ToClient::NoSurfaces { reason } => {
                if self.shared.is_some() {
                    tracing::warn!(
                        "The shell can't use shared surfaces ({reason}): sending pixels"
                    );
                }
                self.shared = None;
                self.pool.clear();
                self.drop_shared_readbacks();
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
            ToClient::Ime { ime } => self.ime(player, ime),
            ToClient::Gamepad { button, pressed } => match button.parse::<GamepadButton>() {
                Ok(button) => {
                    player.handle_event(if pressed {
                        PlayerEvent::GamepadButtonDown { button }
                    } else {
                        PlayerEvent::GamepadButtonUp { button }
                    });
                }
                Err(_) => tracing::info!("A gamepad button I don't know: {button}"),
            },
            ToClient::Dropped { path } => self.dropped(path),
            ToClient::Text { text } => {
                let mods = self.modifiers;
                if self.egui.egui_wants_keyboard_input() {
                    // As egui-winit: no text with Ctrl or Cmd (its shortcuts').
                    if !(mods.ctrl || mods.command || mods.mac_cmd) {
                        self.events.push(egui::Event::Text(text));
                    }
                } else if (mods.ctrl && !mods.alt) || mods.mac_cmd {
                    // Turu: the window types the text a Ctrl shortcut gives (none on Windows,
                    // the letter on Linux); not here. With Alt too it's AltGr on Windows
                    // (`@` on a German keyboard), typed as the window types it.
                } else {
                    for codepoint in text.chars().filter(|c| !c.is_control()) {
                        player.handle_event(PlayerEvent::TextInput { codepoint });
                    }
                }
            }
            ToClient::Focus { focused } => {
                // The window's input method went with the focus (the shell turns it off).
                if !focused {
                    self.composing = false;
                }
                self.game_focused = focused;
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
            | ToClient::OpenWindow { .. }
            | ToClient::RailHidden { .. }
            | ToClient::Unknown => {}
        }
        if self.shown.is_some() && !self.events.is_empty() {
            self.ui_due = Some(Instant::now());
        }
        // Flash's menu goes with the tab's being seen whole (hidden, covered, another tab's tile).
        let tile = self.grid.as_ref().is_some_and(|grid| !grid.active);
        if self.shown.is_none() || self.covered || tile {
            self.close_menu(player, None);
        }
        self.sync_ime();
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
        // The graphics tips count only windows in which the game was drawn all along, in full, one
        // picture per game frame (not with frame interpolation's in-between pictures).
        let tile = self.grid.as_ref().is_some_and(|grid| !grid.active);
        if self.shown.is_none()
            || self.covered
            || tile
            || turu::headless_mode()
            || ruffle_core::frame_interpolation::enabled()
        {
            self.pictures.not_shown();
        }
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
                        if self.can_draw() {
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
                    if self.can_draw() {
                        let new_picture = self
                            .pacing
                            .new_uncapped_picture_due(now, self.uncapped_interval);
                        if new_picture || self.picture_due || ui_wanted {
                            let picture = new_picture || self.picture_due;
                            self.picture_due = false;
                            self.draw(player, picture);
                        }
                    }
                    // Without a limit, or while no more pictures can be under way (its time may be
                    // past: the loop would spin on it), the readback's poll below calls again.
                    next = self
                        .uncapped_interval
                        .filter(|_| self.can_draw())
                        .map(|interval| self.pacing.next_uncapped_picture(interval));
                }
                _ => {
                    if game_frame || ui_wanted || self.picture_due {
                        if self.can_draw() {
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
        // (Every ms also while interpolation is wanted: the pacer times the pictures' way.)
        let poll = match self.pace {
            Pace::Refresh | Pace::Uncapped => READBACK_POLL_PACED,
            _ if self.shared.is_some() && ruffle_core::frame_interpolation::enabled() => {
                READBACK_POLL_PACED
            }
            Pace::Frames | Pace::Unknown => READBACK_POLL,
        };
        let readback = (!self.readbacks.is_empty()).then(|| Instant::now() + poll);
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

    /// Hands the finished pictures to the sink, oldest first.
    fn collect(&mut self) {
        if self.readbacks.is_empty() {
            return;
        }
        let _ = self.descriptors.device.poll(wgpu::PollType::Poll);
        while let Some(readback) = self.readbacks.front() {
            let done = match readback {
                Readback::Pixels { mapped, .. } => mapped,
                Readback::Shared { done, .. } => done,
            };
            if !done.load(Ordering::Acquire) {
                return;
            }
            match self.readbacks.pop_front().expect("looked at above") {
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
                    drawn,
                    ..
                } => {
                    // A picture's whole way, for the pacer: of shared surfaces (pixels never
                    // interpolate), while interpolation is wanted, once per game frame too, or a
                    // tab its floor took down to that would never come back.
                    if ruffle_core::frame_interpolation::enabled() {
                        self.pacing
                            .record_pipeline(drawn.elapsed().as_secs_f64() * 1000.0);
                    }
                    (self.sink)(Picture::Shared { id, width, height });
                }
            }
        }
    }

    /// Whether another picture may be drawn now: none under way, or fewer shared ones than
    /// [`SHARED_IN_FLIGHT`] (a readback of pixels goes alone).
    fn can_draw(&self) -> bool {
        match self.readbacks.back() {
            None => true,
            Some(Readback::Pixels { .. }) => false,
            Some(Readback::Shared { .. }) => self.readbacks.len() < SHARED_IN_FLIGHT,
        }
    }

    /// Forgets the shared pictures under way (the tab hidden, or no shared surfaces any more).
    fn drop_shared_readbacks(&mut self) {
        self.readbacks
            .retain(|readback| matches!(readback, Readback::Pixels { .. }));
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
                // egui sees every press: one on the game closes an open menu, as on the desktop
                // (Flash's too, which takes every press while it's open).
                let on_ui =
                    self.menu.is_some() || self.on_ui(x, y) || self.egui.egui_wants_pointer_input();
                self.pressed = Some((button, on_ui));
                self.events.push(egui::Event::PointerMoved(pos));
                self.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui_button,
                    pressed: true,
                    modifiers: self.modifiers,
                });
                if !on_ui {
                    let handled = player.handle_event(PlayerEvent::MouseDown {
                        x: game_x,
                        y: game_y,
                        button: game_button,
                        index: None,
                    });
                    // A right press the game didn't take opens Flash's menu (when it has items),
                    // which then takes the release, as the window's does.
                    if !handled
                        && matches!(button, Button::Right)
                        && self.open_menu(player, pos, (game_x, game_y))
                    {
                        self.pressed = Some((button, true));
                    }
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
    /// HotKeys', which see the press too). Typed text comes as [`ToClient::Text`]. `game` is the
    /// key for the game as the window makes it, when the shell sent it (a test's `key` line
    /// doesn't: the key is then found from egui's name, as on a US keyboard).
    fn key(
        &mut self,
        player: &mut Player,
        name: &str,
        game: Option<&GameKey>,
        pressed: bool,
        mods: Mods,
    ) {
        self.modifiers = egui::Modifiers {
            alt: mods.alt,
            ctrl: mods.ctrl,
            shift: mods.shift,
            mac_cmd: cfg!(target_os = "macos") && mods.command,
            command: mods.command,
        };
        let (key, control) = match game {
            Some(game) => (game.descriptor(), game.control()),
            None => {
                let (logical_key, key_location) =
                    logical_key(name).unwrap_or((LogicalKey::Unknown, KeyLocation::Standard));
                let key = KeyDescriptor {
                    physical_key: physical_key_for(name).unwrap_or(PhysicalKey::Unknown),
                    logical_key,
                    key_location,
                };
                (key, text_control(logical_key, mods))
            }
        };
        // A key the game has down is let go there, wherever the focus went meanwhile (one of
        // Turu's text boxes, an input method): Ruffle would keep it down (WASD walking on). egui
        // hears the release too (below).
        if !pressed {
            self.release_key(player, key);
        }
        // While an input method composes, its keys are its own (winit doesn't send them on most
        // systems; Enter there must not send the game's chat).
        if self.composing {
            return;
        }
        // Esc closes Flash's menu, as a click elsewhere does.
        if pressed && name == "Escape" && self.menu.is_some() {
            self.close_menu(player, None);
            return;
        }
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
        if !pressed
            || self.egui.egui_wants_keyboard_input()
            || (key.physical_key == PhysicalKey::Unknown && key.logical_key == LogicalKey::Unknown)
        {
            return;
        }
        // Ruffle tells keys held apart by the key on the keyboard, and on macOS and Linux takes
        // the game's `keyCode` from it: the window's key, whole.
        if !self.held_keys.iter().any(|held| same_key(held, &key)) {
            self.held_keys.push(key);
        }
        player.handle_event(PlayerEvent::KeyDown { key });
        if let Some(code) = control {
            player.handle_event(PlayerEvent::TextControl { code });
        }
    }

    /// Lets go of `key` in the game if it has it down.
    fn release_key(&mut self, player: &mut Player, key: KeyDescriptor) {
        if let Some(index) = self.held_keys.iter().position(|held| same_key(held, &key)) {
            self.held_keys.swap_remove(index);
            player.handle_event(PlayerEvent::KeyUp { key });
        }
    }

    /// The tab stops taking input (hidden, or another tab's tile in Grid View): what the game has
    /// down is let go, as the release would go to the next tab (the character walked on in a
    /// tab left with W held), and the game loses the focus, as when the window does.
    fn stop_input(&mut self, player: &mut Player) {
        let held = std::mem::take(&mut self.held_keys);
        if !held.is_empty() {
            tracing::info!(
                "Stopped taking input: let go of {} key(s) in the game",
                held.len()
            );
        }
        for key in held {
            player.handle_event(PlayerEvent::KeyUp { key });
        }
        if let Some((button, false)) = self.pressed.take()
            && let Some((x, y)) = self.pointer_at
        {
            let (left, top) = self.offsets();
            player.handle_event(PlayerEvent::MouseUp {
                x: f64::from(x - left) * self.render_scale,
                y: f64::from(y - top) * self.render_scale,
                button: match button {
                    Button::Left => MouseButton::Left,
                    Button::Right => MouseButton::Right,
                    Button::Middle => MouseButton::Middle,
                },
            });
        }
        self.close_menu(player, None);
        self.composing = false;
        if std::mem::take(&mut self.game_focused) {
            player.handle_event(PlayerEvent::FocusLost);
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
        // Before the picture: behind a render thread nothing is under way now, so a resize's new
        // texture is read without waiting.
        let movie = self.movie_bind_group(player);
        if new_picture && !turu::headless_mode() {
            use ruffle_render_wgpu::perf_counters::timers;
            let render_started = Instant::now();
            let waited_before = timers::snapshot()[timers::RENDER_WAIT];
            player.render();
            let mut draw_ms = render_started.elapsed().as_secs_f64() * 1000.0;
            // Behind a render thread, `render` only walks the stage: the thread makes the GPU
            // commands meanwhile, on a budget of its own (its last picture's time). What this
            // thread waited for it isn't this thread's cost (as in Mugetsu's window).
            if let Some(micros) = render_thread::picture_micros::<MovieTexture>(player.renderer()) {
                let waited = timers::snapshot()[timers::RENDER_WAIT].saturating_sub(waited_before);
                draw_ms = (draw_ms - waited as f64 / 1000.0).max(0.0);
                self.pacing.record_thread(micros as f64 / 1000.0);
            }
            self.pacing.record_draw(draw_ms);
            self.pictures.drawn();
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
        let mut menu = self.menu.as_mut();
        let mut menu_done = None;
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
            if let Some(menu) = menu.as_deref_mut() {
                menu_done = show_menu(root.ctx(), menu);
            }
        });
        // Flash's menu: an item picked, or closed by a press elsewhere (after the UI's frame,
        // which has the player's turn now).
        if let Some(picked) = menu_done {
            self.close_menu(player, picked);
        }
        // An input method for Turu's focused text box, as egui-winit allows one in a window.
        self.ui_ime = output.platform_output.ime.as_ref().map(|ime| {
            let rect = ime.rect;
            (
                [
                    rect.min.x * ppp,
                    rect.min.y * ppp,
                    rect.width() * ppp,
                    rect.height() * ppp,
                ],
                ime.purpose == egui::IMEPurpose::Password,
            )
        });
        self.sync_ime();
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
            if let Some(movie) = &movie {
                pass.set_viewport(
                    left.round(),
                    top,
                    movie_size.0 as f32,
                    movie_size.1 as f32,
                    0.0,
                    1.0,
                );
                self.blit.draw(&self.pipeline, movie, &mut pass);
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
        for id in &textures.free {
            self.egui_renderer.free_texture(id);
        }
        // Applied: epaint checks (in debug builds) that none is dropped unhandled.
        textures.clear();
        // Submitted after the game's picture, which a render thread submits (at once without
        // one), and only then asked to say when it's done: asked here, it would say so for the
        // work before it, and the shell would get a surface not drawn yet.
        let queue = queue.clone();
        let (readback, finish): (Readback, ruffle_render_wgpu::present::Job) = match target {
            None => {
                let (id, _) = surface.expect("no target without a surface");
                let done = Arc::new(AtomicBool::new(false));
                let finished = done.clone();
                (
                    Readback::Shared {
                        id,
                        size,
                        done,
                        drawn: self.last_draw,
                    },
                    Box::new(move || {
                        queue.submit(buffers);
                        queue.on_submitted_work_done(move || {
                            finished.store(true, Ordering::Release)
                        });
                    }),
                )
            }
            Some(target) => {
                let mapped = Arc::new(AtomicBool::new(false));
                let done = mapped.clone();
                let buffer = target.buffer.clone();
                (
                    Readback::Pixels {
                        buffer: target.buffer.clone(),
                        size,
                        padded_row: target.padded_row,
                        mapped,
                    },
                    Box::new(move || {
                        queue.submit(buffers);
                        buffer
                            .slice(..)
                            .map_async(wgpu::MapMode::Read, move |result| {
                                if result.is_ok() {
                                    done.store(true, Ordering::Release);
                                }
                            });
                    }),
                )
            }
        };
        // With a present thread, the game's submit reaches it from the render thread (or this
        // one): this picture is sent after it, through the same thread.
        let finish = match &self.present {
            Some(present) => {
                let present = present.clone();
                Box::new(move || {
                    present.run(finish);
                }) as ruffle_render_wgpu::present::Job
            }
            None => finish,
        };
        render_thread::run_after_picture::<MovieTexture>(player.renderer_mut(), finish);
        self.readbacks.push_back(readback);
    }

    /// The game's texture's bind group for the blit (see `movie`).
    fn movie_bind_group(&mut self, player: &mut Player) -> Option<wgpu::BindGroup> {
        let renderer = player.renderer_mut();
        let state = render_thread::target_state::<MovieTexture>(renderer);
        if let (Some(state), Some((Some(cached), bind_group))) = (state, &self.movie)
            && state == *cached
        {
            return Some(bind_group.clone());
        }
        let bind_group =
            render_thread::with_wgpu_backend::<MovieTexture, _>(renderer, |renderer| {
                renderer.target().bind_group.clone()
            })?;
        // (Reading it counts as a change of the target: the state after it is the one to keep.)
        self.movie = Some((
            render_thread::target_state::<MovieTexture>(renderer),
            bind_group.clone(),
        ));
        Some(bind_group)
    }
}

/// Draws Flash's right-click menu (see [`Menu`]) at its place, as the window's draws Flash's items:
/// `Some(Some(i))` when item `i` was picked, `Some(None)` when it was closed by a press elsewhere
/// (as a system's menu closes).
fn show_menu(ctx: &egui::Context, menu: &mut Menu) -> Option<Option<usize>> {
    let mut picked = None;
    let response = egui::Area::new(egui::Id::new("turu_flash_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(menu.at)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(160.0);
                for (index, item) in menu.items.iter().enumerate() {
                    if item.separator_before && index > 0 {
                        ui.separator();
                    }
                    // A checked item shows selected (Turu's fonts have no check mark).
                    let button = egui::Button::new(item.caption.as_str())
                        .frame(false)
                        .selected(item.checked);
                    if ui.add_enabled(item.enabled, button).clicked() {
                        picked = Some(index);
                    }
                }
            });
        })
        .response;
    let first = !std::mem::replace(&mut menu.shown, true);
    let pressed_elsewhere = ctx.input(|input| {
        input.pointer.any_pressed()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|at| !response.rect.contains(at))
    });
    match picked {
        Some(index) => Some(Some(index)),
        None => (!first && pressed_elsewhere).then_some(None),
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

/// The same key for Ruffle (it tells keys held apart by their place on the keyboard and location).
fn same_key(a: &KeyDescriptor, b: &KeyDescriptor) -> bool {
    a.physical_key == b.physical_key && a.key_location == b.key_location
}

/// The key on a US keyboard egui's name is on, for a key that came without the shell's (a test's
/// `key` line).
fn physical_key_for(name: &str) -> Option<PhysicalKey> {
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => {
            crate::shell::keys::physical_key(&format!("Key{}", c.to_ascii_uppercase()))
        }
        (Some(c), None) if c.is_ascii_digit() => {
            crate::shell::keys::physical_key(&format!("Digit{c}"))
        }
        // The named keys egui and Ruffle name alike: Enter, Space, ArrowLeft, F1, ...
        _ => crate::shell::keys::physical_key(name),
    }
}

/// What a key does in a game text field instead of typing (Backspace, arrows, Ctrl+V), as the
/// window's `util::winit_to_ruffle_text_control` finds it, for a key that came without the
/// shell's.
fn text_control(key: LogicalKey, mods: Mods) -> Option<TextControlCode> {
    use TextControlCode as C;
    // (`command` is Ctrl off macOS.)
    let ctrl_cmd = mods.ctrl || mods.command;
    let shift = mods.shift;
    let named = match key {
        LogicalKey::Character(c) if ctrl_cmd => {
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
    Some(match (named, ctrl_cmd, shift) {
        (NamedKey::Enter, ..) => C::Enter,
        (NamedKey::Backspace, true, _) => C::BackspaceWord,
        (NamedKey::Backspace, false, _) => C::Backspace,
        (NamedKey::Delete, true, _) => C::DeleteWord,
        (NamedKey::Delete, false, _) => C::Delete,
        (NamedKey::ArrowLeft, true, true) => C::SelectLeftWord,
        (NamedKey::ArrowLeft, true, false) => C::MoveLeftWord,
        (NamedKey::ArrowLeft, false, true) => C::SelectLeft,
        (NamedKey::ArrowLeft, false, false) => C::MoveLeft,
        (NamedKey::ArrowRight, true, true) => C::SelectRightWord,
        (NamedKey::ArrowRight, true, false) => C::MoveRightWord,
        (NamedKey::ArrowRight, false, true) => C::SelectRight,
        (NamedKey::ArrowRight, false, false) => C::MoveRight,
        (NamedKey::Home, true, true) => C::SelectLeftDocument,
        (NamedKey::Home, true, false) => C::MoveLeftDocument,
        (NamedKey::Home, false, true) => C::SelectLeftLine,
        (NamedKey::Home, false, false) => C::MoveLeftLine,
        (NamedKey::End, true, true) => C::SelectRightDocument,
        (NamedKey::End, true, false) => C::MoveRightDocument,
        (NamedKey::End, false, true) => C::SelectRightLine,
        (NamedKey::End, false, false) => C::MoveRightLine,
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

/// Whether the game hid the mouse (Flash's `Mouse.hide`, its UI backend's `set_mouse_visible`).
static GAME_MOUSE_HIDDEN: AtomicBool = AtomicBool::new(false);

/// The game shows or hides the mouse over itself.
pub fn set_game_mouse_visible(visible: bool) {
    GAME_MOUSE_HIDDEN.store(!visible, Ordering::Relaxed);
}

/// Whether the game's mouse shows (Flash's `Mouse.hide` hasn't hidden it).
pub fn game_mouse_visible() -> bool {
    !GAME_MOUSE_HIDDEN.load(Ordering::Relaxed)
}

fn game_cursor() -> Cursor {
    if !game_mouse_visible() {
        return Cursor::None;
    }
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

    /// Ruffle takes `keyCode` from the key on the keyboard on macOS and Linux, and tells held keys
    /// apart by it everywhere: never `Unknown` for the keys the game reads.
    #[test]
    fn keys_have_their_place_on_the_keyboard() {
        for (name, physical) in [
            ("W", PhysicalKey::KeyW),
            ("A", PhysicalKey::KeyA),
            ("S", PhysicalKey::KeyS),
            ("D", PhysicalKey::KeyD),
            ("1", PhysicalKey::Digit1),
            ("Enter", PhysicalKey::Enter),
            ("Space", PhysicalKey::Space),
            ("ArrowLeft", PhysicalKey::ArrowLeft),
            ("F5", PhysicalKey::F5),
        ] {
            assert_eq!(physical_key_for(name), Some(physical), "{name}");
        }
        assert_eq!(physical_key_for("Minus"), Some(PhysicalKey::Minus));
        assert_eq!(physical_key_for("Nothing"), None);
    }
}
