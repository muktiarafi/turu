//! What's on screen, as the desktop's `gui/controller.rs` does it: the game draws into a texture
//! of its own ([`MovieTexture`]), and each frame that texture is drawn below Turu's nav bar with
//! the bot windows (`turu_frontend::gui::turu`, egui) over it. Everything here lives on the
//! engine thread, which runs the player ([`SCREEN`]).
//!
//! The GPU is set up without a window ([`gpu`]), so the game can run before one exists (started
//! by the service after Android killed the app). A window gets a screen and a surface; a new
//! window only a new surface; the player's renderer and its texture stay. Each finger goes
//! to the UI when it comes down on it (the nav bar, a window), else to the game; keys go to the UI
//! while one of its text boxes has the focus, which also brings up the soft keyboard.
//!
//! The screen is drawn after each tick, unless the game is behind ([`Screen::behind`]). With frame
//! interpolation on (Options > Application) it's presented on every refresh of the display
//! instead, with a new in-between picture on every n-th refresh, paced as on the desktop
//! (`turu_frontend::pacing`, [`Screen::step`]).

use crate::input::{self, Key, Touch};
use ndk::native_window::NativeWindow;
use ruffle_core::Player;
use ruffle_core::events::TouchPhase;
use ruffle_render::backend::ViewportDimensions;
use ruffle_render_wgpu::backend::{WgpuRenderBackend, create_wgpu_instance, request_adapter_and_device};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::target::{RenderTarget, RenderTargetFrame};
use std::any::Any;
use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_frontend::engine::Step;
use turu_frontend::gui::turu::TuruUi;
use turu_frontend::pacing::Pacing;
use turu_frontend::touch::GameTouches;
use turu_frontend::turu;
use wgpu::util::DeviceExt;

/// The longest the screen goes undrawn while the game is behind (see [`Screen::behind`]).
const MAX_DRAW_GAP: Duration = Duration::from_millis(250);

thread_local! {
    /// The engine thread's screen, once the first window came.
    pub static SCREEN: RefCell<Option<Screen>> = const { RefCell::new(None) };
}

/// The graphics API: `--graphics` in `turu-args.txt`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Graphics {
    /// Vulkan, else OpenGL ES.
    Auto,
    Vulkan,
    Gl,
}

pub struct Screen {
    descriptors: Arc<Descriptors>,
    /// The window's surface, while there is a window.
    surface: Option<wgpu::Surface<'static>>,
    /// The window `surface` is on (its pointer).
    surface_for: usize,
    format: wgpu::TextureFormat,
    /// The window, in pixels.
    size: (u32, u32),
    pixels_per_point: f32,
    blit: Arc<Blit>,
    pipeline: wgpu::RenderPipeline,
    egui: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
    ui: TuruUi,
    events: Vec<egui::Event>,
    started: Instant,
    /// The fingers that came down on the UI (the others are the game's).
    ui_fingers: Vec<i32>,
    /// The fingers on the game, as its one mouse.
    game_touches: GameTouches,
    /// The fingers on the game that are Flash touch points instead (`Player::handle_touch`):
    /// ones that came while another held the mouse, onto something listening for `touchBegin`.
    touch_points: Vec<(i32, (f64, f64))>,
    /// The finger egui follows.
    egui_finger: Option<EguiFinger>,
    keyboard_shown: bool,
    /// Whether a text box of the UI had the focus in the last frame (egui asked for IME then).
    /// Not `egui_wants_keyboard_input`: that's true for any focused widget, such as a modal's
    /// button, which would bring the keyboard up over the modal.
    ui_typing: bool,
    /// The game's viewport, last set.
    movie_size: (u32, u32),
    /// Frame interpolation's pacing, with the display's refresh rate.
    pacing: Pacing,
    /// The display's refresh rate the pacing has (mHz), 0 before the first.
    refresh_millihertz: u32,
    /// When the last draw started.
    draw_started: Instant,
    /// Whether the surface's last `configure` worked, and when to try again if it didn't
    /// ([`Screen::configure`]).
    configured: bool,
    retry_configure: Instant,
    /// The refresh rate last asked of the activity for the game's frame rate (Hz, 0: the system's
    /// choice); `None` until asked in this window.
    asked_refresh: Option<u32>,
    /// The game's texture as egui knows it (for the touch controls' skill icons), and the size it
    /// was registered at: the texture is made again when the game's area changes size.
    game_picture: Option<(egui::TextureId, (u32, u32))>,
}

/// The GPU, set up without a window (the API `graphics` asks for), the blit the screen draws the
/// game with, and the renderer the player draws with, at `size`. None when no API works.
pub fn gpu(graphics: Graphics, size: (u32, u32)) -> Option<(Arc<Descriptors>, Arc<Blit>, WgpuRenderBackend<MovieTexture>)> {
    let tries: &[wgpu::Backends] = match graphics {
        Graphics::Auto => &[wgpu::Backends::VULKAN, wgpu::Backends::GL],
        Graphics::Vulkan => &[wgpu::Backends::VULKAN],
        Graphics::Gl => &[wgpu::Backends::GL],
    };
    let descriptors = tries.iter().find_map(|&backend| {
        let instance = create_wgpu_instance(backend, Default::default(), None);
        let made = futures::executor::block_on(request_adapter_and_device(
            backend,
            &instance,
            None,
            wgpu::PowerPreference::HighPerformance,
        ));
        match made {
            Ok((adapter, device, queue)) => {
                // In the debug log too (Diagnostics > Logs, and its Share), not only logcat: a
                // driver that misdraws says nothing, so a report needs to say which one it was.
                let info = adapter.get_info();
                turu::bot().logs.debug.push(format!(
                    "Graphics: {} on {} ({:?}, vendor {:#06x}, device {:#06x}); driver {} {}",
                    info.backend.to_str(),
                    info.name,
                    info.device_type,
                    info.vendor,
                    info.device,
                    info.driver,
                    info.driver_info,
                ));
                Some(Arc::new(Descriptors::new(instance, adapter, device, queue)))
            }
            Err(e) => {
                turu::bot().logs.debug.push(format!("Graphics: {backend:?} failed: {e}"));
                None
            }
        }
    })?;
    let blit = Arc::new(Blit::new(&descriptors.device));
    let target = MovieTexture::new(blit.clone(), &descriptors.device, size.0, size.1);
    match WgpuRenderBackend::new(descriptors.clone(), target) {
        Ok(renderer) => Some((descriptors, blit, renderer)),
        Err(e) => {
            tracing::error!("Couldn't make the game's renderer: {e}");
            None
        }
    }
}

/// How far a finger on the game moves, in points, before it's dragging ([`GameTouches`]).
const TAP_SLOP: f32 = 6.0;

/// The finger egui follows, in window pixels.
#[derive(Clone, Copy)]
struct EguiFinger {
    id: i32,
    /// Whether it came down on the UI.
    on_ui: bool,
    at: (f64, f64),
}

impl Screen {
    /// The screen in `window`, on the GPU [`gpu`] set up. None when the window can't be drawn in.
    pub fn new(descriptors: Arc<Descriptors>, blit: Arc<Blit>, window: &NativeWindow, pixels_per_point: f32) -> Option<Self> {
        // SAFETY: the window stays valid until `detach`, which drops the surface first.
        let surface = match unsafe { descriptors.wgpu_instance.create_surface_unsafe(crate::surface_target(window)) } {
            Ok(surface) => surface,
            Err(e) => {
                tracing::error!("Couldn't draw in the window: {e}");
                return None;
            }
        };
        let formats = surface.get_capabilities(&descriptors.adapter).formats;
        let format = [wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Bgra8Unorm]
            .into_iter()
            .find(|f| formats.contains(f))
            .or_else(|| formats.first().copied())?;
        let pipeline = blit.pipeline(&descriptors.device, format);
        let egui = egui::Context::default();
        egui.set_fonts(system_fonts());
        let egui_renderer = egui_wgpu::Renderer::new(
            &descriptors.device,
            format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );
        let screen = Self {
            descriptors,
            surface: Some(surface),
            surface_for: window.ptr().as_ptr() as usize,
            format,
            size: crate::window_size(window),
            pixels_per_point,
            blit,
            pipeline,
            egui,
            egui_renderer,
            ui: TuruUi::new(),
            events: Vec::new(),
            started: Instant::now(),
            ui_fingers: Vec::new(),
            game_touches: GameTouches::new(f64::from(TAP_SLOP * pixels_per_point)),
            touch_points: Vec::new(),
            egui_finger: None,
            keyboard_shown: false,
            ui_typing: false,
            movie_size: (0, 0),
            pacing: Pacing::new(),
            refresh_millihertz: 0,
            draw_started: Instant::now(),
            configured: false,
            retry_configure: Instant::now(),
            asked_refresh: None,
            game_picture: None,
        };
        let mut screen = screen;
        screen.configure();
        Some(screen)
    }

    /// Configures the surface for the window's size. A failed `configure` isn't fatal: wgpu
    /// reports the error but leaves the surface without its error sink, and the next
    /// `get_current_texture` then aborted (Sentry TURU-ANDROID-9: an Infinix back in front after
    /// running out of GPU memory; TURU-ANDROID-8: an OPPO whose window "does not support the
    /// adapter's queue family", one going away). So it runs in error scopes; until it works, the
    /// screen isn't drawn and `draw` tries again once a second.
    fn configure(&mut self) {
        let Some(surface) = &self.surface else {
            self.configured = false;
            return;
        };
        let device = &self.descriptors.device;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        surface.configure(
            device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: self.size.0,
                height: self.size.1,
                present_mode: Default::default(),
                desired_maximum_frame_latency: 2,
                alpha_mode: Default::default(),
                view_formats: Vec::new(),
            },
        );
        let error = pop_error_scope(memory).or(pop_error_scope(validation));
        self.configured = error.is_none();
        self.retry_configure = Instant::now() + Duration::from_secs(1);
        if let Some(error) = error {
            tracing::warn!(
                target: "ruffle_aqw_perf",
                "Couldn't set up the window for drawing ({}x{}); trying again: {error}",
                self.size.0,
                self.size.1
            );
        }
    }

    /// Draws in `window` from now on (unless it already does: Vulkan allows one surface a window).
    pub fn attach(&mut self, window: &NativeWindow) {
        let ptr = window.ptr().as_ptr() as usize;
        if self.surface.is_some() && self.surface_for == ptr {
            return;
        }
        self.surface = None;
        // SAFETY: the window stays valid until `detach`, which drops the surface first.
        match unsafe {
            self.descriptors
                .wgpu_instance
                .create_surface_unsafe(crate::surface_target(window))
        } {
            Ok(surface) => {
                self.surface = Some(surface);
                self.surface_for = ptr;
                // A new window (the activity came back): its display mode is asked for again.
                self.asked_refresh = None;
                self.size = crate::window_size(window);
                self.configure();
            }
            Err(e) => tracing::error!("Couldn't draw in the new window: {e}"),
        }
    }

    /// Whether there's a window to draw in.
    pub fn shown(&self) -> bool {
        self.surface.is_some()
    }

    /// Another tab's player takes the screen: `player`'s fingers are let go (the game's and
    /// egui's), and the next player drawn gets the game's size.
    pub fn leave(&mut self, player: &mut Player) {
        self.touch(player, Touch::Cancel);
        self.movie_size = (0, 0);
    }

    /// The window is going away: no more drawing in it.
    pub fn detach(&mut self) {
        self.surface = None;
        self.keyboard_shown = false;
    }

    /// Android said the window may have changed (resized, content rect or configuration changed).
    /// Only a new size reconfigures the surface: that drops its pictures, and some drivers then
    /// show black until the next draw (up to a SWF frame, ~40 ms, later), so the game flickered
    /// on every inset change (the keyboard, the system bars). A new size is drawn right away.
    pub fn resize(&mut self, window: &NativeWindow, player: &mut Player) {
        let size = crate::window_size(window);
        if size == self.size {
            return;
        }
        tracing::info!("window now {}x{} (was {}x{})", size.0, size.1, self.size.0, self.size.1);
        self.size = size;
        self.configure();
        if self.surface.is_some() {
            self.draw(player, true);
        }
    }

    /// What of Turu is above the game (the tab strip, with tabs), in pixels.
    fn top(&self) -> f32 {
        turu_frontend::gui::top_height() as f32 * self.pixels_per_point
    }

    /// Turu's nav rail left of the game, in pixels: the game keeps the screen's height.
    fn left(&self) -> f32 {
        turu_frontend::gui::left_width() as f32 * self.pixels_per_point
    }

    /// The game's part of the window, in pixels.
    fn movie_area(&self) -> (u32, u32) {
        let top = self.top().round() as u32;
        let left = self.left().round() as u32;
        (
            self.size.0.saturating_sub(left).max(1),
            self.size.1.saturating_sub(top).max(1),
        )
    }

    /// A finger: the UI's when it comes down on the nav rail or on a window, menu or popup, else
    /// the game's. The root UI covers the whole screen on egui's background layer, so that layer
    /// doesn't count. The game's fingers share its one mouse ([`GameTouches`]: a finger tapping a
    /// skill button while another drags the joystick); a finger that comes while another holds
    /// the mouse is first a Flash touch point, and only a tap when nothing under it listens for
    /// touches (AQW's dash button and keys do, and dash in the joystick's direction). egui follows one finger, the first one
    /// down, or one on the UI over one on the game (a tap on the nav bar while the joystick is
    /// held); it sees the game's too: one on the game closes an open menu, as a click elsewhere
    /// does on the desktop.
    pub fn touch(&mut self, player: &mut Player, touch: Touch) {
        let top = f64::from(self.top());
        let left = f64::from(self.left());
        let mouse = match touch {
            Touch::Down(id, x, y) => {
                let pos = egui::pos2(x as f32 / self.pixels_per_point, y as f32 / self.pixels_per_point);
                let on_layer = self
                    .egui
                    .layer_id_at(pos)
                    .is_some_and(|layer| layer.order != egui::Order::Background);
                let on_ui = y < top || x < left || on_layer;
                // Turu's joystick and skill buttons take their fingers before egui and the game,
                // so one can be held while another taps (`touch_controls`).
                if !on_ui && turu_frontend::touch_controls::press(id, pos) {
                    return;
                }
                match self.egui_finger {
                    Some(finger) if finger.on_ui || !on_ui => {}
                    other => {
                        if let Some(finger) = other {
                            self.egui_touch(egui::TouchPhase::End, finger.at);
                        }
                        self.egui_finger = Some(EguiFinger { id, on_ui, at: (x, y) });
                        self.egui_touch(egui::TouchPhase::Start, (x, y));
                    }
                }
                if on_ui {
                    self.ui_fingers.push(id);
                    Vec::new()
                } else if self.game_touches.holding() && player.handle_touch(TouchPhase::Begin, id, x - left, y - top) {
                    self.touch_points.push((id, (x, y)));
                    Vec::new()
                } else {
                    self.game_touches.down(id, x, y)
                }
            }
            Touch::Move(id, x, y) => {
                if turu_frontend::touch_controls::holds(id) {
                    let pos = egui::pos2(x as f32 / self.pixels_per_point, y as f32 / self.pixels_per_point);
                    turu_frontend::touch_controls::moved(id, pos);
                    return;
                }
                if let Some(finger) = self.egui_finger.as_mut().filter(|finger| finger.id == id) {
                    finger.at = (x, y);
                    self.egui_touch(egui::TouchPhase::Move, (x, y));
                }
                if let Some((_, at)) = self.touch_points.iter_mut().find(|(finger, _)| *finger == id) {
                    *at = (x, y);
                    player.handle_touch(TouchPhase::Move, id, x - left, y - top);
                    Vec::new()
                } else if self.ui_fingers.contains(&id) {
                    Vec::new()
                } else {
                    self.game_touches.moved(id, x, y)
                }
            }
            Touch::Up(id, x, y) => {
                if turu_frontend::touch_controls::holds(id) {
                    turu_frontend::touch_controls::release(id);
                    return;
                }
                if self.egui_finger.is_some_and(|finger| finger.id == id) {
                    self.egui_finger = None;
                    self.egui_touch(egui::TouchPhase::End, (x, y));
                }
                if let Some(i) = self.touch_points.iter().position(|&(finger, _)| finger == id) {
                    self.touch_points.swap_remove(i);
                    player.handle_touch(TouchPhase::End, id, x - left, y - top);
                    Vec::new()
                } else if let Some(i) = self.ui_fingers.iter().position(|&finger| finger == id) {
                    self.ui_fingers.swap_remove(i);
                    Vec::new()
                } else {
                    self.game_touches.up(id, x, y)
                }
            }
            Touch::Cancel => {
                turu_frontend::touch_controls::cancel();
                if let Some(finger) = self.egui_finger.take() {
                    self.egui_touch(egui::TouchPhase::Cancel, finger.at);
                }
                self.ui_fingers.clear();
                for (id, (x, y)) in std::mem::take(&mut self.touch_points) {
                    player.handle_touch(TouchPhase::End, id, x - left, y - top);
                }
                self.game_touches.cancel()
            }
        };
        for mouse in mouse {
            player.handle_event(input::player_event(mouse, left, top));
        }
    }

    fn egui_touch(&mut self, phase: egui::TouchPhase, (x, y): (f64, f64)) {
        self.events.extend(input::egui_events(phase, x, y, self.pixels_per_point));
    }

    /// A key: the UI's while a text box of it has the focus, else the game's (and HotKeys'). A game text field
    /// takes no more characters once it's full ([`Typing::MAX_CHARS`]).
    pub fn key(&mut self, player: &mut Player, key: &Key) {
        if self.ui_typing {
            self.events.extend(key.egui_events());
            return;
        }
        // Turu's HotKeys see the press too (no text: nothing types into a window).
        self.events.extend(key.egui_key_events());
        let full = key.typed.is_some() && key.control.is_none() && Typing::read(player).is_some_and(|t| t.is_full());
        for event in key.player_events() {
            if !(full && matches!(event, ruffle_core::PlayerEvent::TextInput { .. })) {
                player.handle_event(event);
            }
        }
    }

    /// After a tick, or at the time it asked for: draws, and with frame interpolation on says
    /// when to draw next. Without interpolation the screen is drawn after each tick (the game
    /// rendered again when it changed). With it, it's presented on every refresh of the display
    /// (presenting waits for vsync), with a new in-between picture when [`Pacing`] says one is
    /// due; ticks only plan (as the desktop's `pace_interpolation`).
    pub fn step(&mut self, player: &mut Player, step: Step) -> Option<Instant> {
        if let Step::Ticked { frame_ms: Some(frame_ms) } = step {
            self.pacing.record_frame(frame_ms);
        }
        if self.surface.is_none() {
            return None;
        }
        self.prefer_refresh_rate(player.frame_rate());
        // Headless Mode's screen hides the game: nothing to interpolate.
        let interpolating = ruffle_core::frame_interpolation::enabled()
            && player.is_playing()
            && !turu::headless_mode()
            && {
            self.update_refresh_rate();
            self.pacing.plan(Instant::now(), player.frame_rate())
        };
        match (step, interpolating) {
            (Step::Ticked { .. }, false) => {
                // Faster than the display (Set FPS above its refresh rate): presenting each tick
                // would wait for vsync and hold the game back to the display's rate. The game
                // ticks at its rate and the screen shows the newest picture once a refresh.
                self.update_refresh_rate();
                let refresh = self.pacing.refresh_interval();
                if self.draw_started.elapsed() < refresh.mul_f32(0.75) {
                    return None;
                }
                if self.behind(player) {
                    crate::frame_perf::skipped();
                    return None;
                }
                let new_picture = player.needs_render();
                self.draw(player, new_picture);
                None
            }
            // Turned off, or the movie stopped, since the last draw was asked for.
            (Step::Due, false) => None,
            (Step::Ticked { .. }, true) => Some(self.draw_started + self.pacing.refresh_interval() / 2),
            (Step::Due, true) => {
                let started = Instant::now();
                let new_picture = self.pacing.new_picture_due(started);
                self.draw(player, new_picture);
                // Asked for right away, keeping the display's queue of pictures full so a busy
                // moment doesn't miss a refresh: the next present waits for its refresh.
                Some(self.draw_started + self.pacing.refresh_interval() / 2)
            }
        }
    }

    /// Whether to skip this tick's draw because the game is behind: more than half a frame of
    /// game time is already waiting to run. A phone that renders a crowded room in more than a
    /// frame (50 ms on the emulator in battleon, 280 render passes) otherwise ran one game
    /// frame per render (`Player::max_frames_per_tick` leaves no time to catch up), so the game
    /// itself played at 15-18 FPS instead of 30, in slow motion. Skipping the draws that would
    /// make the game later keeps it at full speed and draws as often as the rest of the time
    /// allows (30 game frames and ~15 pictures a second there, not 18 and 18). A render that fits
    /// in half a frame never skips; the screen is drawn at least every [`MAX_DRAW_GAP`].
    fn behind(&self, player: &Player) -> bool {
        player.is_playing()
            && self.draw_started.elapsed() < MAX_DRAW_GAP
            && player.time_til_next_frame() * 2 < Duration::from_secs_f64(1.0 / player.frame_rate().max(1.0))
    }

    /// A game running faster than 60 FPS (Options > Game > Set FPS) asks the activity for a display
    /// mode that keeps up with it: many 90 and 120 Hz phones hold an app at 60 Hz unless it asks.
    /// At 60 or less the system chooses again (saving battery).
    fn prefer_refresh_rate(&mut self, frame_rate: f64) {
        let want = if frame_rate > 61.0 { frame_rate.round() as u32 } else { 0 };
        if self.asked_refresh != Some(want) {
            self.asked_refresh = Some(want);
            crate::ui::prefer_refresh_rate(want);
        }
    }

    /// Lends the game's texture to egui, with the stage's place in it, so the touch controls' skill
    /// buttons can show the action bar's icons (`touch_controls::set_game_picture`).
    fn share_game_picture(&mut self, player: &mut Player) {
        let Some(texture) = <dyn Any>::downcast_ref::<WgpuRenderBackend<MovieTexture>>(player.renderer_mut())
            .map(|r| r.target().texture.clone())
        else {
            turu_frontend::touch_controls::set_game_picture(None);
            return;
        };
        let size = (texture.width(), texture.height());
        let device = &self.descriptors.device;
        let id = match self.game_picture {
            Some((id, at)) if at == size => id,
            Some((id, _)) => {
                let view = texture.create_view(&Default::default());
                self.egui_renderer
                    .update_egui_texture_from_wgpu_texture(device, &view, wgpu::FilterMode::Linear, id);
                id
            }
            None => {
                let view = texture.create_view(&Default::default());
                self.egui_renderer
                    .register_native_texture(device, &view, wgpu::FilterMode::Linear)
            }
        };
        self.game_picture = Some((id, size));
        let m = player.mutate_with_update_context(|context| context.stage.view_matrix());
        turu_frontend::touch_controls::set_game_picture(Some(turu_frontend::touch_controls::GamePicture {
            texture: id,
            size: [size.0 as f32, size.1 as f32],
            stage_to_pixels: [m.a, m.b, m.c, m.d, m.tx.to_pixels() as f32, m.ty.to_pixels() as f32],
        }));
    }

    /// The display's refresh rate, from the activity (`ui::refresh_millihertz`).
    fn update_refresh_rate(&mut self) {
        let millihertz = crate::ui::refresh_millihertz();
        if millihertz != self.refresh_millihertz {
            self.refresh_millihertz = millihertz;
            self.pacing.set_refresh_rate(millihertz);
            tracing::info!(target: "ruffle_aqw_perf", "display refreshes at {:.1} Hz", millihertz as f64 / 1000.0);
        }
    }

    /// Draws a frame, when there's a window: the game (rendered again with `new_picture`) below
    /// the bar, the UI over it.
    fn draw(&mut self, player: &mut Player, mut new_picture: bool) {
        let started = Instant::now();
        self.draw_started = started;
        // A lost GPU device draws nothing (`memory::gpu_lost`): configuring the window again
        // every second would only make warnings.
        if self.surface.is_none() || ruffle_render_wgpu::device_lost().is_some() {
            return;
        }
        // The rail grows by a cutout on its side (a punch-hole camera, held sideways).
        turu_frontend::gui::set_rail_inset(crate::ui::cutout().0 as f32 / self.pixels_per_point);
        let movie_size = self.movie_area();
        if movie_size != self.movie_size {
            self.movie_size = movie_size;
            player.set_viewport_dimensions(ViewportDimensions {
                width: movie_size.0,
                height: movie_size.1,
                scale_factor: f64::from(self.pixels_per_point),
            });
            // The resize made the game's texture anew, empty: drawn without a new picture, it
            // flashed black (or stretched) until the next frame rendered.
            new_picture = true;
            tracing::info!("game area now {}x{}", movie_size.0, movie_size.1);
        }
        if new_picture {
            player.render();
            let took = started.elapsed();
            crate::frame_perf::add(crate::frame_perf::Phase::Render, took);
            self.pacing.record_draw(took.as_secs_f64() * 1000.0);
        }
        if !self.configured {
            if Instant::now() < self.retry_configure {
                return;
            }
            self.configure();
            if !self.configured {
                return;
            }
        }
        let Some(surface) = &self.surface else {
            return;
        };
        let acquire_started = Instant::now();
        let acquired = surface.get_current_texture();
        crate::frame_perf::add(crate::frame_perf::Phase::Acquire, acquire_started.elapsed());
        let frame = match acquired {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure();
                return;
            }
            _ => return,
        };

        let ui_started = Instant::now();
        let ppp = self.pixels_per_point;
        let mut raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.size.0 as f32 / ppp, self.size.1 as f32 / ppp),
            )),
            time: Some(self.started.elapsed().as_secs_f64()),
            events: std::mem::take(&mut self.events),
            max_texture_side: Some(self.descriptors.limits.max_texture_dimension_2d as usize),
            ..Default::default()
        };
        if let Some(viewport) = raw_input.viewports.get_mut(&egui::ViewportId::ROOT) {
            viewport.native_pixels_per_point = Some(ppp);
        }
        let top = self.top() / ppp;
        // The game's text field under the keyboard, while the keyboard is up for it.
        let keyboard = crate::ui::keyboard_height() as f32 / ppp;
        let (cutout_left, cutout_right) = crate::ui::cutout();
        let cutout = (cutout_left as f32 / ppp, cutout_right as f32 / ppp);
        let typing = (keyboard > 0.0 && !self.ui_typing)
            .then(|| Typing::read(player))
            .flatten();
        self.share_game_picture(player);
        let ui = &mut self.ui;
        let output = self.egui.run_ui(raw_input, |root| {
            ui.show_bar(root);
            if turu::headless_mode() {
                turu_frontend::gui::turu::show_headless_screen(root.ctx(), top);
            }
            ui.show_windows(root.ctx());
            if let Some(typing) = &typing {
                typing.show(root.ctx(), keyboard, cutout);
            }
        });
        self.ui_typing = output.platform_output.ime.is_some();
        let ime = output.platform_output.ime.as_ref();
        crate::pan::follow_text_field(&self.egui, ime, self.size.1 as f32 / ppp, keyboard);
        self.soft_keyboard();
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                crate::platform::copy_text(text);
            }
        }

        let primitives = self.egui.tessellate(output.shapes, output.pixels_per_point);
        crate::frame_perf::add(crate::frame_perf::Phase::Ui, ui_started.elapsed());
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.0, self.size.1],
            pixels_per_point: output.pixels_per_point,
        };
        let device = &self.descriptors.device;
        let queue = &self.descriptors.queue;
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                self.egui_renderer.update_texture(device, queue, *id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Turu screen"),
        });
        let mut buffers = self
            .egui_renderer
            .update_buffers(device, queue, &mut encoder, &primitives, &screen);
        {
            let view = frame.texture.create_view(&Default::default());
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    label: Some("Turu screen"),
                    ..Default::default()
                })
                .forget_lifetime();
            if let Some(movie) =
                <dyn Any>::downcast_ref::<WgpuRenderBackend<MovieTexture>>(player.renderer_mut()).map(|r| r.target())
            {
                let top = self.top();
                pass.set_viewport(
                    self.left().round(),
                    top,
                    movie_size.0 as f32,
                    movie_size.1 as f32,
                    0.0,
                    1.0,
                );
                self.blit.draw(&self.pipeline, &movie.bind_group, &mut pass);
                pass.set_viewport(0.0, 0.0, self.size.0 as f32, self.size.1 as f32, 0.0, 1.0);
            }
            self.egui_renderer.render(&mut pass, &primitives, &screen);
        }
        buffers.push(encoder.finish());
        let present_started = Instant::now();
        queue.submit(buffers);
        for id in &output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        queue.present(frame);
        crate::frame_perf::add(crate::frame_perf::Phase::Present, present_started.elapsed());
        crate::frame_perf::add(crate::frame_perf::Phase::Draw, started.elapsed());
    }

    /// Brings the soft keyboard up while a text box of the UI has the focus.
    fn soft_keyboard(&mut self) {
        let wanted = self.ui_typing;
        if wanted != self.keyboard_shown {
            self.keyboard_shown = wanted;
            crate::ui::soft_keyboard(wanted);
        }
    }
}

/// What the game's focused text field holds, shown in a strip above the soft keyboard: the
/// keyboard covers the field itself, and NativeActivity has no view to scroll it into sight.
struct Typing {
    /// The text on one line, a password's characters hidden.
    text: String,
    /// The caret, in characters of `text`.
    caret: usize,
    /// Its length in characters.
    length: usize,
    /// The selected characters, when some are (typing replaces them).
    selected: Option<(usize, usize)>,
}

impl Typing {
    const HEIGHT: f32 = 44.0;
    /// The most the game takes in a message: typing stops there.
    const MAX_CHARS: usize = 150;

    /// Typing a character would go past [`Self::MAX_CHARS`].
    fn is_full(&self) -> bool {
        self.length >= Self::MAX_CHARS && self.selected.is_none()
    }

    /// The focused field, when it's one you type in.
    fn read(player: &mut Player) -> Option<Self> {
        player.mutate_with_update_context(|context| {
            let field = context.focus_tracker.get_as_edit_text()?;
            if !field.is_editable() {
                return None;
            }
            let text = field.text();
            // Positions in the field are in its code units; the strip counts characters.
            let chars_to = |at: usize| text.slice(..at.min(text.len())).map_or(0, |before| before.chars().count());
            let selection = field.selection();
            let caret = chars_to(selection.map_or(text.len(), |s| s.to()));
            let selected = selection
                .filter(|s| s.start() != s.end())
                .map(|s| (chars_to(s.start()), chars_to(s.end())));
            let length = text.chars().count();
            let text = if field.is_password() {
                "•".repeat(length)
            } else {
                text.to_utf8_lossy().replace(['\r', '\n'], " ")
            };
            Some(Self { text, caret, length, selected })
        })
    }

    /// The strip, its bottom `keyboard` points above the screen's, as wide as the screen less
    /// `cutout`, the display cutout's (left, right) insets in points: held sideways, a punch-hole
    /// camera sits at the side, about where the strip is, and the keyboard below keeps clear of it too.
    fn show(&self, ctx: &egui::Context, keyboard: f32, cutout: (f32, f32)) {
        use turu_frontend::gui::turu::theme::{ACCENT, CARD, DANGER, EDGE, MUTED, TEXT};
        let screen = ctx.content_rect();
        let bottom = (screen.bottom() - keyboard).max(screen.top() + Self::HEIGHT);
        let rect = egui::Rect::from_min_max(
            egui::pos2(screen.left() + cutout.0, bottom - Self::HEIGHT),
            egui::pos2((screen.right() - cutout.1).max(screen.left() + cutout.0), bottom),
        );
        // An area of its own, above the windows: a touch on it stays off the game, so the field keeps the focus.
        egui::Area::new(egui::Id::new("turu_typing"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(rect.size(), egui::Sense::click());
                let painter = ui.painter();
                painter.rect_filled(rect, 0.0, CARD);
                painter.hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, EDGE));
                let mut inner = rect.shrink2(egui::vec2(16.0, 8.0));
                // The count on the right, red at the limit.
                let count = format!("{}/{}", self.length, Self::MAX_CHARS);
                let color = if self.length >= Self::MAX_CHARS { DANGER } else { MUTED };
                let count = painter.layout_no_wrap(count, egui::FontId::proportional(13.0), color);
                let count_at = egui::pos2(inner.right() - count.size().x, inner.center().y - count.size().y / 2.0);
                inner.max.x -= count.size().x + 16.0;
                painter.galley(count_at, count, color);
                let font = egui::FontId::proportional(18.0);
                if self.text.is_empty() {
                    let galley = painter.layout_no_wrap("Typing in the game".into(), font, MUTED);
                    let at = egui::pos2(inner.left() + 4.0, inner.center().y - galley.size().y / 2.0);
                    painter.galley(at, galley, MUTED);
                    painter.vline(inner.left(), inner.y_range(), egui::Stroke::new(2.0, ACCENT));
                    return;
                }
                let galley = painter.layout_no_wrap(self.text.clone(), font, TEXT);
                let caret = galley.pos_from_cursor(egui::text::CCursor::new(self.caret)).min.x;
                // Scrolled so the caret is in sight, as the field itself scrolls.
                let shift = (caret - inner.width() + 4.0).max(0.0);
                let left = inner.left() - shift;
                let clipped = painter.with_clip_rect(inner.expand2(egui::vec2(2.0, 0.0)));
                if let Some((start, end)) = self.selected {
                    let x = |at| left + galley.pos_from_cursor(egui::text::CCursor::new(at)).min.x;
                    let selection = egui::Rect::from_x_y_ranges(x(start)..=x(end), inner.y_range());
                    clipped.rect_filled(selection, 2.0, ACCENT.gamma_multiply(0.35));
                }
                clipped.galley(egui::pos2(left, inner.center().y - galley.size().y / 2.0), galley, TEXT);
                clipped.vline(left + caret, inner.y_range(), egui::Stroke::new(2.0, ACCENT));
            });
    }
}

/// egui's fonts, then Android's for what they lack (arrows, symbols), as the desktop's
/// `load_system_fonts` adds the system's. Each group is the first of its files that's there.
fn system_fonts() -> egui::FontDefinitions {
    const GROUPS: [&[&str]; 2] = [
        &["RobotoStatic-Regular.ttf", "Roboto-Regular.ttf", "DroidSans.ttf"],
        &["NotoSansSymbols-Regular-Subsetted.ttf"],
    ];
    let mut fonts = egui::FontDefinitions::default();
    for files in GROUPS {
        let found = files.iter().find_map(|&file| {
            let data = std::fs::read(std::path::Path::new("/system/fonts").join(file)).ok()?;
            Some((file, data))
        });
        let Some((file, data)) = found else {
            tracing::warn!("No system font of {files:?}");
            continue;
        };
        fonts.font_data.insert(file.to_owned(), Arc::new(egui::FontData::from_owned(data)));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().push(file.to_owned());
    }
    turu_frontend::gui::turu::theme::add_symbol_font(&mut fonts);
    fonts
}

/// Draws a texture over the render pass's viewport: the desktop's `blit.wgsl`. The pipeline,
/// which depends on the surface's format, is the screen's ([`Blit::pipeline`]).
pub struct Blit {
    module: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertices: wgpu::Buffer,
}

impl Blit {
    fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Turu blit"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../../app/src/gui/blit.wgsl"))),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // The whole viewport: x y u v.
        let quad: [[f32; 4]; 6] = [
            [-1.0, 1.0, 0.0, 0.0],
            [1.0, 1.0, 1.0, 0.0],
            [1.0, -1.0, 1.0, 1.0],
            [1.0, -1.0, 1.0, 1.0],
            [-1.0, -1.0, 0.0, 1.0],
            [-1.0, 1.0, 0.0, 0.0],
        ];
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&quad),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self {
            module,
            layout,
            sampler,
            vertices,
        }
    }

    fn pipeline(&self, device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&self.layout)],
            immediate_size: 0,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Turu blit"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &self.module,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 4 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                })],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.module,
                entry_point: Some(if format.is_srgb() {
                    "fs_main_srgb_framebuffer"
                } else {
                    "fs_main_linear_framebuffer"
                }),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    fn draw(&self, pipeline: &wgpu::RenderPipeline, bind_group: &wgpu::BindGroup, pass: &mut wgpu::RenderPass<'static>) {
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..6, 0..1);
    }
}

/// The texture the game draws into (the desktop's `MovieView`).
#[derive(Debug)]
pub struct MovieTexture {
    blit: Arc<Blit>,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl std::fmt::Debug for Blit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Blit")
    }
}

impl MovieTexture {
    pub fn new(blit: Arc<Blit>, device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Turu game"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &blit.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&blit.sampler),
                },
            ],
        });
        Self {
            blit,
            texture,
            bind_group,
        }
    }
}

impl RenderTarget for MovieTexture {
    type Frame = MovieFrame;

    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = MovieTexture::new(self.blit.clone(), device, width, height);
    }

    fn format(&self) -> wgpu::TextureFormat {
        self.texture.format()
    }

    fn width(&self) -> u32 {
        self.texture.width()
    }

    fn height(&self) -> u32 {
        self.texture.height()
    }

    fn get_next_texture(&mut self) -> Option<Self::Frame> {
        Some(MovieFrame(self.texture.create_view(&Default::default())))
    }

    fn submit<I: IntoIterator<Item = wgpu::CommandBuffer>>(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        command_buffers: I,
        _frame: Self::Frame,
    ) -> wgpu::SubmissionIndex {
        queue.submit(command_buffers)
    }
}

#[derive(Debug)]
pub struct MovieFrame(wgpu::TextureView);

impl RenderTargetFrame for MovieFrame {
    fn into_view(self) -> wgpu::TextureView {
        self.0
    }

    fn view(&self) -> &wgpu::TextureView {
        &self.0
    }
}

/// The error a popped scope caught, if any. On native wgpu the future is ready at once, so it's
/// polled once here (an executor's `block_on` panics inside another executor).
fn pop_error_scope(scope: wgpu::ErrorScopeGuard) -> Option<wgpu::Error> {
    let future = std::pin::pin!(scope.pop());
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match std::future::Future::poll(future, &mut context) {
        std::task::Poll::Ready(error) => error,
        std::task::Poll::Pending => None,
    }
}
