//! Turu's windows over the game in AIR (docs/AIR_PROPOSAL.md, "The UI over AIR"): the nav rail and
//! the bot windows (`turu_frontend::gui::turu::TuruUi`, egui), drawn with wgpu into a transparent
//! view over AIR's (Kotlin's `TuruOverlay`), as the Ruffle build draws them over its game
//! (`android/src/screen.rs`) minus the game's picture: AIR draws the game itself, its view moved
//! right of the rail ([`game_insets`]).
//!
//! A thread of its own runs egui and draws: at once after a touch, when egui asks, and at least
//! [`IDLE_FRAME`] apart (the logs change without input). Kotlin asks [`on_ui`] when a finger comes
//! down: the UI's touches come here, the others go to AIR's view below.
//!
//! As the Ruffle build's screen: the rail grows by the display cutout on its side; a window whose
//! text box has the focus moves up above the soft keyboard (`android/src/pan.rs`); and while the
//! keyboard is up for one of the game's text fields (AIR's), a strip above it shows what the field
//! holds ([`GameTyping`], which the host SWF reads), as the keyboard covers the field itself.

use crate::host::BRIDGE;
use jni::objects::JValue;
use jni::{Env, jni_sig, jni_str};
use ndk::native_window::NativeWindow;
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use turu_frontend::gui::turu::TuruUi;
use wgpu::rwh::{AndroidDisplayHandle, HasWindowHandle, RawDisplayHandle};

/// The longest the UI goes undrawn: its logs and status change without input.
const IDLE_FRAME: Duration = Duration::from_millis(200);

/// What Kotlin and the host hand the drawing thread.
struct Shared {
    /// The overlay's surface, while there is one, and a count of the ones it had.
    window: Option<NativeWindow>,
    window_count: u64,
    /// The count of surfaces the drawing thread has taken in (dropping the old one's).
    applied: u64,
    pixels_per_point: f32,
    events: Vec<egui::Event>,
    wake: bool,
    /// The soft keyboard's height, and the display cutout's left and right insets, in pixels.
    keyboard: f32,
    cutout: (f32, f32),
    /// The game's focused text field, while there's one you type in.
    game_typing: Option<GameTyping>,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    window: None,
    window_count: 0,
    applied: 0,
    pixels_per_point: 1.0,
    events: Vec::new(),
    wake: false,
    keyboard: 0.0,
    cutout: (0.0, 0.0),
    game_typing: None,
});
static WAKE: Condvar = Condvar::new();
/// The drawing thread took a surface change in.
static APPLIED: Condvar = Condvar::new();
/// How long Kotlin waits for the drawing thread to let a surface go.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(2);
/// egui's context, for [`on_ui`] (Kotlin's thread) and the drawing thread.
static EGUI: OnceLock<egui::Context> = OnceLock::new();
/// What's above and left of the game, in pixels, as last drawn.
static INSETS: Mutex<(f32, f32)> = Mutex::new((0.0, 0.0));

fn shared() -> MutexGuard<'static, Shared> {
    SHARED.lock().unwrap_or_else(|e| e.into_inner())
}

/// The overlay's surface came (or changed) or went (`window` None), at `pixels_per_point`.
/// Returns once the drawing thread has taken the change in: after `surfaceDestroyed` returns,
/// Android may free the surface, and a frame still drawing into it crashed the app on a phone
/// (Samsung Galaxy S24 FE) coming back from the background.
pub(crate) fn set_window(window: Option<NativeWindow>, pixels_per_point: f32) {
    start();
    let mut shared = shared();
    shared.window = window;
    shared.window_count += 1;
    shared.pixels_per_point = pixels_per_point.max(0.5);
    shared.wake = true;
    let count = shared.window_count;
    WAKE.notify_one();
    let deadline = Instant::now() + RELEASE_TIMEOUT;
    while shared.applied < count {
        let now = Instant::now();
        if now >= deadline {
            tracing::warn!("The UI's thread didn't let its surface go in time");
            break;
        }
        shared = APPLIED
            .wait_timeout(shared, deadline - now)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
}

/// The window's insets changed: the soft keyboard's height and the display cutout's sides, in
/// pixels.
pub(crate) fn set_insets(keyboard: f32, cutout: (f32, f32)) {
    {
        let mut shared = shared();
        shared.keyboard = keyboard.max(0.0);
        shared.cutout = (cutout.0.max(0.0), cutout.1.max(0.0));
        shared.wake = true;
    }
    WAKE.notify_one();
}

/// The game's focused text field (from the host SWF), or `None` when there's none to type in.
pub(crate) fn set_game_typing(typing: Option<GameTyping>) {
    {
        let mut shared = shared();
        if shared.game_typing == typing {
            return;
        }
        shared.game_typing = typing;
        shared.wake = true;
    }
    WAKE.notify_one();
}

/// A finger (`phase`), at (`x`, `y`) in the overlay's pixels.
pub(crate) fn touch(phase: egui::TouchPhase, x: f32, y: f32) {
    {
        let mut shared = shared();
        let ppp = shared.pixels_per_point;
        let pos = egui::pos2(x / ppp, y / ppp);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        // As touches too: egui's ScrollArea drags only on a touch screen.
        let touch = egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(0),
            phase,
            pos,
            force: None,
        };
        let events = match phase {
            egui::TouchPhase::Start => vec![touch, egui::Event::PointerMoved(pos), button(true)],
            egui::TouchPhase::Move => vec![touch, egui::Event::PointerMoved(pos)],
            egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                vec![touch, button(false), egui::Event::PointerGone]
            }
        };
        shared.events.extend(events);
        shared.wake = true;
    }
    WAKE.notify_one();
}

/// Events from the soft keyboard, for the UI's focused text box.
pub(crate) fn input(events: Vec<egui::Event>) {
    {
        let mut shared = shared();
        shared.events.extend(events);
        shared.wake = true;
    }
    WAKE.notify_one();
}

/// Whether a finger coming down at (`x`, `y`) (pixels) is on Turu's UI: the rail or the bar, or a
/// window, menu or popup (egui's root covers the screen on its background layer, which doesn't
/// count), as the Ruffle build decides.
pub(crate) fn on_ui(x: f32, y: f32) -> bool {
    let (left, top) = *INSETS.lock().unwrap_or_else(|e| e.into_inner());
    if x < left || y < top {
        return true;
    }
    let Some(egui) = EGUI.get() else {
        return false;
    };
    let ppp = shared().pixels_per_point;
    egui.layer_id_at(egui::pos2(x / ppp, y / ppp))
        .is_some_and(|layer| layer.order != egui::Order::Background)
}

fn start() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let spawned = std::thread::Builder::new()
            .name("Turu UI".into())
            .spawn(run);
        if let Err(e) = spawned {
            tracing::error!("Couldn't start the UI thread: {e}");
        }
    });
}

/// The GPU, made with the first surface (OpenGL ES needs one to pick an adapter).
struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// egui's renderer, for the surfaces' format: kept across surfaces, as egui uploads its
    /// textures (the fonts) once. A new one per surface drew nothing after the app came back from
    /// the background ("Missing texture: Managed(0)").
    renderer: Option<(wgpu::TextureFormat, egui_wgpu::Renderer)>,
}

/// The surface in the overlay's current window.
struct Target {
    surface: wgpu::Surface<'static>,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    /// Whether the last `configure` took.
    configured: bool,
    /// Keeps the window alive as long as the surface on it.
    _window: NativeWindow,
}

fn run() {
    let egui = EGUI.get_or_init(egui::Context::default).clone();
    egui.set_fonts(system_fonts());
    let mut ui = TuruUi::new();
    let started = Instant::now();
    let mut gpu: Option<Gpu> = None;
    let mut target: Option<Target> = None;
    let mut target_for = 0;
    let mut next = Instant::now();
    let mut keyboard_shown = false;
    let mut ui_typing = false;
    let mut sent_insets = (u32::MAX, u32::MAX);
    let mut panned: Option<(egui::LayerId, f32)> = None;
    loop {
        // Wait for input, a new window, or the next frame.
        let (events, ppp, window, keyboard, cutout, game_typing) = {
            let mut shared = shared();
            while !shared.wake {
                let now = Instant::now();
                if now >= next {
                    break;
                }
                shared = WAKE
                    .wait_timeout(shared, next - now)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
            shared.wake = false;
            let window = (shared.window_count != target_for).then(|| {
                target_for = shared.window_count;
                shared.window.clone()
            });
            (
                std::mem::take(&mut shared.events),
                shared.pixels_per_point,
                window,
                shared.keyboard,
                shared.cutout,
                shared.game_typing.clone(),
            )
        };
        if let Some(window) = window {
            target = None;
            if let Some(window) = window {
                target = make_target(&mut gpu, window);
            }
            let mut shared = shared();
            shared.applied = target_for;
            APPLIED.notify_all();
        }
        let (Some(gpu), Some(target)) = (gpu.as_mut(), target.as_mut()) else {
            next = Instant::now() + IDLE_FRAME;
            continue;
        };

        let mut raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(target.size.0 as f32 / ppp, target.size.1 as f32 / ppp),
            )),
            time: Some(started.elapsed().as_secs_f64()),
            events,
            max_texture_side: Some(gpu.device.limits().max_texture_dimension_2d as usize),
            ..Default::default()
        };
        if let Some(viewport) = raw_input.viewports.get_mut(&egui::ViewportId::ROOT) {
            viewport.native_pixels_per_point = Some(ppp);
        }
        // The rail grows by a cutout on its side (a punch-hole camera, held sideways).
        turu_frontend::gui::set_rail_inset(cutout.0 / ppp);
        let keyboard = keyboard / ppp;
        let cutout = (cutout.0 / ppp, cutout.1 / ppp);
        let game_typing = game_typing.filter(|_| keyboard > 0.0 && !ui_typing);
        let output = egui.run_ui(raw_input, |root| {
            ui.show_bar(root);
            ui.show_windows(root.ctx());
            if let Some(typing) = &game_typing {
                typing.show(root.ctx(), keyboard, cutout);
            }
        });
        ui_typing = output.platform_output.ime.is_some();
        panned = follow_text_field(
            &egui,
            output.platform_output.ime.as_ref(),
            target.size.1 as f32 / ppp,
            keyboard,
            panned,
        );

        // The game's place: right of the rail, below the bar.
        let insets = (
            (turu_frontend::gui::left_width() as f32 * ppp).round(),
            (turu_frontend::gui::top_height() as f32 * ppp).round(),
        );
        *INSETS.lock().unwrap_or_else(|e| e.into_inner()) = insets;
        let insets = (insets.0 as u32, insets.1 as u32);
        if insets != sent_insets && game_insets(insets) {
            sent_insets = insets;
        }
        if ui_typing != keyboard_shown && soft_keyboard(ui_typing) {
            keyboard_shown = ui_typing;
        }
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                copy_text(text);
            }
        }

        let delay = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(IDLE_FRAME, |v| v.repaint_delay)
            .min(IDLE_FRAME);
        draw(gpu, target, &egui, output);
        next = Instant::now() + delay;
    }
}

/// The space kept between a text box and the keyboard, in points.
const PAN_MARGIN: f32 = 8.0;

/// Keeps Turu's own text box above the soft keyboard (the Ruffle build's `pan.rs`): while an egui
/// text field has the focus and the keyboard is up, the window (layer) holding it moves up just
/// enough; it moves back when the keyboard goes down or the field loses the focus. `ime` is the
/// frame's IME output, `height` the screen's and `keyboard` the keyboard's height in points;
/// `before` what was moved, and the result what is now.
fn follow_text_field(
    ctx: &egui::Context,
    ime: Option<&egui::output::IMEOutput>,
    height: f32,
    keyboard: f32,
    before: Option<(egui::LayerId, f32)>,
) -> Option<(egui::LayerId, f32)> {
    use egui::emath::TSTransform;
    let wanted = ime.filter(|_| keyboard > 0.0).and_then(|ime| {
        let layer = ctx.layer_id_at(ime.rect.center())?;
        // Where the field is with the layer where it belongs.
        let moved = match before {
            Some((panned, by)) if panned == layer => by,
            _ => 0.0,
        };
        let bottom = ime.rect.bottom() + moved;
        let top = ime.rect.top() + moved;
        let visible_bottom = height - keyboard - PAN_MARGIN;
        // Not above the screen's top either.
        let by = (bottom - visible_bottom).min(top - PAN_MARGIN).max(0.0);
        (by > 0.0).then_some((layer, by))
    });
    if wanted == before {
        return before;
    }
    if let Some((layer, _)) = before
        && wanted.is_none_or(|(now, _)| now != layer)
    {
        ctx.set_transform_layer(layer, TSTransform::IDENTITY);
    }
    if let Some((layer, by)) = wanted {
        ctx.set_transform_layer(layer, TSTransform::from_translation(egui::vec2(0.0, -by)));
    }
    ctx.request_repaint();
    wanted
}

/// What the game's focused text field holds, shown in a strip above the soft keyboard (the Ruffle
/// build's `Typing`): the keyboard covers the field itself. The host SWF reads it from AIR's
/// `stage.focus`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GameTyping {
    /// The text on one line, a password's characters hidden.
    pub text: String,
    /// The caret, in characters of `text`.
    pub caret: usize,
    /// The selected characters, when some are.
    pub selected: Option<(usize, usize)>,
    /// The field's `maxChars` (0: no limit).
    pub max: usize,
}

impl GameTyping {
    const HEIGHT: f32 = 44.0;

    /// From the host's JSON (`{"text", "caret", "selStart", "selEnd", "password", "max"}`), or
    /// `None` for anything else (no field).
    pub(crate) fn from_json(json: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        let text = value.get("text")?.as_str()?;
        let length = text.chars().count();
        let number = |key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .map(|n| n as usize)
        };
        let caret = number("caret").unwrap_or(length).min(length);
        let selected = match (number("selStart"), number("selEnd")) {
            (Some(start), Some(end)) if start != end => {
                Some((start.min(end).min(length), start.max(end).min(length)))
            }
            _ => None,
        };
        let text = if value.get("password").and_then(serde_json::Value::as_bool) == Some(true) {
            "•".repeat(length)
        } else {
            text.replace(['\r', '\n'], " ")
        };
        Some(Self {
            text,
            caret,
            selected,
            max: number("max").unwrap_or(0),
        })
    }

    /// The strip, its bottom `keyboard` points above the screen's, as wide as the screen less
    /// `cutout`'s (left, right) insets, in points.
    fn show(&self, ctx: &egui::Context, keyboard: f32, cutout: (f32, f32)) {
        use turu_frontend::gui::turu::theme::{ACCENT, CARD, DANGER, EDGE, MUTED, TEXT};
        let screen = ctx.content_rect();
        let bottom = (screen.bottom() - keyboard).max(screen.top() + Self::HEIGHT);
        let rect = egui::Rect::from_min_max(
            egui::pos2(screen.left() + cutout.0, bottom - Self::HEIGHT),
            egui::pos2(
                (screen.right() - cutout.1).max(screen.left() + cutout.0),
                bottom,
            ),
        );
        // An area of its own, above the windows; a touch on it stays off the game, so the field
        // keeps the focus.
        egui::Area::new(egui::Id::new("turu_typing"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(rect.size(), egui::Sense::click());
                let painter = ui.painter();
                painter.rect_filled(rect, 0.0, CARD);
                painter.hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, EDGE));
                let mut inner = rect.shrink2(egui::vec2(16.0, 8.0));
                let length = self.text.chars().count();
                if self.max > 0 {
                    // The count on the right, red at the limit.
                    let count = format!("{length}/{}", self.max);
                    let color = if length >= self.max { DANGER } else { MUTED };
                    let count =
                        painter.layout_no_wrap(count, egui::FontId::proportional(13.0), color);
                    let count_at = egui::pos2(
                        inner.right() - count.size().x,
                        inner.center().y - count.size().y / 2.0,
                    );
                    inner.max.x -= count.size().x + 16.0;
                    painter.galley(count_at, count, color);
                }
                let font = egui::FontId::proportional(18.0);
                if self.text.is_empty() {
                    let galley = painter.layout_no_wrap("Typing in the game".into(), font, MUTED);
                    let at =
                        egui::pos2(inner.left() + 4.0, inner.center().y - galley.size().y / 2.0);
                    painter.galley(at, galley, MUTED);
                    painter.vline(
                        inner.left(),
                        inner.y_range(),
                        egui::Stroke::new(2.0, ACCENT),
                    );
                    return;
                }
                let galley = painter.layout_no_wrap(self.text.clone(), font, TEXT);
                let caret = galley
                    .pos_from_cursor(egui::text::CCursor::new(self.caret))
                    .min
                    .x;
                // Scrolled so the caret is in sight, as the field itself scrolls.
                let shift = (caret - inner.width() + 4.0).max(0.0);
                let left = inner.left() - shift;
                let clipped = painter.with_clip_rect(inner.expand2(egui::vec2(2.0, 0.0)));
                if let Some((start, end)) = self.selected {
                    let x = |at| left + galley.pos_from_cursor(egui::text::CCursor::new(at)).min.x;
                    let selection = egui::Rect::from_x_y_ranges(x(start)..=x(end), inner.y_range());
                    clipped.rect_filled(selection, 2.0, ACCENT.gamma_multiply(0.35));
                }
                clipped.galley(
                    egui::pos2(left, inner.center().y - galley.size().y / 2.0),
                    galley,
                    TEXT,
                );
                clipped.vline(
                    left + caret,
                    inner.y_range(),
                    egui::Stroke::new(2.0, ACCENT),
                );
            });
    }
}

fn make_target(gpu: &mut Option<Gpu>, window: NativeWindow) -> Option<Target> {
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: window.window_handle().ok()?.as_raw(),
    };
    if gpu.is_none() {
        *gpu = make_gpu(&target);
    }
    let gpu = gpu.as_mut()?;
    // SAFETY: the window lives in the `Target`, which drops the surface first.
    let surface = match unsafe { gpu.instance.create_surface_unsafe(target) } {
        Ok(surface) => surface,
        Err(e) => {
            tracing::error!("Couldn't draw the UI in its view: {e}");
            return None;
        }
    };
    let formats = surface.get_capabilities(&gpu.adapter).formats;
    // The renderer's format where the new surface has it, so egui's textures stay.
    let kept = gpu.renderer.as_ref().map(|(format, _)| *format);
    let format = kept
        .into_iter()
        .chain([
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ])
        .find(|f| formats.contains(f))
        .or_else(|| formats.first().copied())?;
    if kept != Some(format) {
        if kept.is_some() {
            tracing::warn!("The UI's new surface is {format:?}: its textures are made again");
        }
        let renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );
        gpu.renderer = Some((format, renderer));
    }
    let mut target = Target {
        surface,
        format,
        size: (window.width().max(1) as u32, window.height().max(1) as u32),
        configured: false,
        _window: window,
    };
    configure(gpu, &mut target);
    Some(target)
}

/// Vulkan, else OpenGL ES, as the Ruffle build tries them.
fn make_gpu(target: &wgpu::SurfaceTargetUnsafe) -> Option<Gpu> {
    for backends in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let target = match target {
            wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle,
                raw_window_handle,
            } => wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: *raw_display_handle,
                raw_window_handle: *raw_window_handle,
            },
            _ => continue,
        };
        // SAFETY: only to pick an adapter that can draw in the window, dropped right away.
        let Ok(surface) = (unsafe { instance.create_surface_unsafe(target) }) else {
            continue;
        };
        let adapter =
            futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                ..Default::default()
            }));
        drop(surface);
        let Ok(adapter) = adapter else {
            continue;
        };
        let device = futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Turu UI"),
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }));
        match device {
            Ok((device, queue)) => {
                let info = adapter.get_info();
                tracing::info!(
                    "UI drawn with {} on {} (driver {} {})",
                    info.backend.to_str(),
                    info.name,
                    info.driver,
                    info.driver_info
                );
                return Some(Gpu {
                    instance,
                    adapter,
                    device,
                    queue,
                    renderer: None,
                });
            }
            Err(e) => tracing::warn!("UI: {backends:?} has no device: {e}"),
        }
    }
    tracing::error!("No graphics API can draw Turu's UI");
    None
}

/// Configures the surface: see-through where nothing is drawn, so the game shows below.
fn configure(gpu: &Gpu, target: &mut Target) {
    let capabilities = target.surface.get_capabilities(&gpu.adapter);
    let alpha_mode = [
        wgpu::CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::Inherit,
        wgpu::CompositeAlphaMode::PostMultiplied,
    ]
    .into_iter()
    .find(|mode| capabilities.alpha_modes.contains(mode))
    .unwrap_or_else(|| {
        tracing::warn!(
            "The UI's view can't be see-through ({:?}): the game will be hidden",
            capabilities.alpha_modes
        );
        wgpu::CompositeAlphaMode::Auto
    });
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    target.surface.configure(
        &gpu.device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: target.format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: target.size.0,
            height: target.size.1,
            present_mode: Default::default(),
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: Vec::new(),
        },
    );
    let error = futures::executor::block_on(validation.pop());
    target.configured = error.is_none();
    if let Some(error) = error {
        tracing::warn!("Couldn't set up the UI's view for drawing: {error}");
    }
}

fn draw(gpu: &mut Gpu, target: &mut Target, egui: &egui::Context, output: egui::FullOutput) {
    if !target.configured {
        configure(gpu, target);
        if !target.configured {
            return;
        }
    }
    let frame = match target.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame)
        | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
        wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
            configure(gpu, target);
            return;
        }
        _ => return,
    };
    let primitives = egui.tessellate(output.shapes, output.pixels_per_point);
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [target.size.0, target.size.1],
        pixels_per_point: output.pixels_per_point,
    };
    let Gpu {
        device,
        queue,
        renderer,
        ..
    } = gpu;
    let Some((_, renderer)) = renderer.as_mut() else {
        return;
    };
    for (id, deltas) in &output.textures_delta.set {
        for delta in deltas {
            renderer.update_texture(device, queue, *id, delta);
        }
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Turu UI"),
    });
    let mut buffers = renderer.update_buffers(device, queue, &mut encoder, &primitives, &screen);
    {
        let view = frame.texture.create_view(&Default::default());
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                label: Some("Turu UI"),
                ..Default::default()
            })
            .forget_lifetime();
        renderer.render(&mut pass, &primitives, &screen);
    }
    buffers.push(encoder.finish());
    queue.submit(buffers);
    for id in &output.textures_delta.free {
        renderer.free_texture(id);
    }
    gpu.queue.present(frame);
}

/// Tells Kotlin where the game goes (`Bridge.gameInsets(left, top)`, pixels); false when it
/// couldn't.
fn game_insets((left, top): (u32, u32)) -> bool {
    with_bridge(|env, class| {
        env.call_static_method(
            class,
            jni_str!("gameInsets"),
            jni_sig!("(II)V"),
            &[JValue::Int(left as i32), JValue::Int(top as i32)],
        )?;
        Ok(())
    })
}

/// Brings the soft keyboard up for the UI's text box, or down (`Bridge.softKeyboard(shown)`).
fn soft_keyboard(shown: bool) -> bool {
    with_bridge(|env, class| {
        env.call_static_method(
            class,
            jni_str!("softKeyboard"),
            jni_sig!("(Z)V"),
            &[JValue::Bool(shown)],
        )?;
        Ok(())
    })
}

fn copy_text(text: &str) {
    with_bridge(|env, class| {
        let text = env.new_string(text)?;
        env.call_static_method(
            class,
            jni_str!("copyText"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&text)],
        )?;
        Ok(())
    });
}

/// Runs `work` with Kotlin's `Bridge` on this thread; false when it couldn't.
fn with_bridge(
    work: impl FnOnce(&mut Env, &jni::objects::JClass) -> jni::errors::Result<()>,
) -> bool {
    let Some((vm, class)) = BRIDGE.get() else {
        return false;
    };
    crate::android::attach(vm, |env: &mut Env| work(env, class))
        .inspect_err(|e| tracing::warn!("A call to Kotlin's Bridge failed: {e}"))
        .is_ok()
}

/// egui's fonts, then Android's for what they lack (arrows, symbols), as the Ruffle build's
/// (`android/src/screen.rs`). Each group is the first of its files that's there.
fn system_fonts() -> egui::FontDefinitions {
    const GROUPS: [&[&str]; 2] = [
        &[
            "RobotoStatic-Regular.ttf",
            "Roboto-Regular.ttf",
            "DroidSans.ttf",
        ],
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
        fonts.font_data.insert(
            file.to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(data)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push(file.to_owned());
    }
    turu_frontend::gui::turu::theme::add_symbol_font(&mut fonts);
    fonts
}
