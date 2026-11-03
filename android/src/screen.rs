//! What's on screen, as the desktop's `gui/controller.rs` does it: the game draws into a texture
//! of its own ([`MovieTexture`]), and each frame that texture is drawn below Turu's nav bar with
//! the bot windows (`turu_frontend::gui::turu`, egui) over it. Everything here lives on the
//! engine thread, which runs the player ([`SCREEN`]).
//!
//! The GPU is set up without a window ([`gpu`]), so the game can run before one exists (started
//! by the service after Android killed the app). A window gets a screen and a surface; a new
//! window only a new surface; the player's renderer and its texture stay. Touches go
//! to the UI when they start on it (the nav bar, a window), else to the game; keys go to the UI
//! while one of its text boxes has the focus, which also brings up the soft keyboard.

use crate::input::{Key, Touch};
use ndk::native_window::NativeWindow;
use ruffle_core::Player;
use ruffle_render::backend::ViewportDimensions;
use ruffle_render_wgpu::backend::{WgpuRenderBackend, create_wgpu_instance, request_adapter_and_device};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::target::{RenderTarget, RenderTargetFrame};
use std::any::Any;
use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;
use turu_frontend::gui::turu::TuruUi;
use turu_frontend::turu;
use wgpu::util::DeviceExt;

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
    /// Whether the touch going on started on the UI.
    touch_on_ui: bool,
    keyboard_shown: bool,
    /// The game's viewport, last set.
    movie_size: (u32, u32),
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
                let info = adapter.get_info();
                tracing::info!("Graphics: {} on {}", info.backend.to_str(), info.name);
                Some(Arc::new(Descriptors::new(instance, adapter, device, queue)))
            }
            Err(e) => {
                tracing::warn!("Graphics: {backend:?} failed: {e}");
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
            touch_on_ui: false,
            keyboard_shown: false,
            movie_size: (0, 0),
        };
        screen.configure();
        Some(screen)
    }

    fn configure(&self) {
        if let Some(surface) = &self.surface {
            surface.configure(
                &self.descriptors.device,
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
                self.size = crate::window_size(window);
                self.configure();
            }
            Err(e) => tracing::error!("Couldn't draw in the new window: {e}"),
        }
    }

    /// The window is going away: no more drawing in it.
    pub fn detach(&mut self) {
        self.surface = None;
        self.keyboard_shown = false;
    }

    pub fn resize(&mut self, window: &NativeWindow) {
        self.size = crate::window_size(window);
        self.configure();
    }

    /// Turu's bar above the game, in pixels.
    fn top(&self) -> f32 {
        turu_frontend::gui::top_height() as f32 * self.pixels_per_point
    }

    /// The game's part of the window, in pixels.
    fn movie_area(&self) -> (u32, u32) {
        let top = self.top().round() as u32;
        (self.size.0, self.size.1.saturating_sub(top).max(1))
    }

    /// A touch: the UI's when it starts on the nav bar or on a window, menu or popup, else the
    /// game's. The root UI covers the whole screen on egui's background layer, so that layer
    /// doesn't count. egui sees every touch: one on the game closes an open menu, as a click
    /// elsewhere does on the desktop.
    pub fn touch(&mut self, player: &mut Player, touch: Touch) {
        if let Touch::Down(x, y) = touch {
            let pos = egui::pos2(x as f32 / self.pixels_per_point, y as f32 / self.pixels_per_point);
            let on_layer = self
                .egui
                .layer_id_at(pos)
                .is_some_and(|layer| layer.order != egui::Order::Background);
            self.touch_on_ui = y < f64::from(self.top()) || on_layer;
        }
        self.events.extend(touch.egui_events(self.pixels_per_point));
        if !self.touch_on_ui {
            player.handle_event(touch.player_event(f64::from(self.top())));
        }
    }

    /// A key: the UI's while a text box of it has the focus, else the game's. A game text field
    /// takes no more characters once it's full ([`Typing::MAX_CHARS`]).
    pub fn key(&mut self, player: &mut Player, key: &Key) {
        if self.egui.egui_wants_keyboard_input() {
            self.events.extend(key.egui_events());
            return;
        }
        let full = key.typed.is_some() && key.control.is_none() && Typing::read(player).is_some_and(|t| t.is_full());
        for event in key.player_events() {
            if !(full && matches!(event, ruffle_core::PlayerEvent::TextInput { .. })) {
                player.handle_event(event);
            }
        }
    }

    /// Draws a frame, when there's a window: the game (rendered again when it changed) below the
    /// bar, the UI over it.
    pub fn draw(&mut self, player: &mut Player) {
        let Some(surface) = &self.surface else {
            return;
        };
        let movie_size = self.movie_area();
        if movie_size != self.movie_size {
            self.movie_size = movie_size;
            player.set_viewport_dimensions(ViewportDimensions {
                width: movie_size.0,
                height: movie_size.1,
                scale_factor: f64::from(self.pixels_per_point),
            });
        }
        if player.needs_render() {
            player.render();
        }
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure();
                return;
            }
            _ => return,
        };

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
        let typing = (keyboard > 0.0 && !self.egui.egui_wants_keyboard_input())
            .then(|| Typing::read(player))
            .flatten();
        let ui = &mut self.ui;
        let output = self.egui.run_ui(raw_input, |root| {
            ui.show_bar(root);
            if turu::headless_mode() {
                turu_frontend::gui::turu::show_headless_screen(root.ctx(), top);
            }
            ui.show_windows(root.ctx());
            if let Some(typing) = &typing {
                typing.show(root.ctx(), keyboard);
            }
        });
        self.soft_keyboard();

        let primitives = self.egui.tessellate(output.shapes, output.pixels_per_point);
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
                    0.0,
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
        queue.submit(buffers);
        for id in &output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        queue.present(frame);
    }

    /// Brings the soft keyboard up while a text box of the UI has the focus.
    fn soft_keyboard(&mut self) {
        let wanted = self.egui.egui_wants_keyboard_input();
        if wanted != self.keyboard_shown
            && let Some(app) = crate::app()
        {
            self.keyboard_shown = wanted;
            if wanted {
                app.show_soft_input(true);
            } else {
                app.hide_soft_input(true);
            }
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

    /// The strip, as wide as the screen, its bottom `keyboard` points above the screen's.
    fn show(&self, ctx: &egui::Context, keyboard: f32) {
        use turu_frontend::gui::turu::theme::{ACCENT, CARD, DANGER, EDGE, MUTED, TEXT};
        let screen = ctx.content_rect();
        let bottom = (screen.bottom() - keyboard).max(screen.top() + Self::HEIGHT);
        let rect = egui::Rect::from_min_max(
            egui::pos2(screen.left(), bottom - Self::HEIGHT),
            egui::pos2(screen.right(), bottom),
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
    fn new(blit: Arc<Blit>, device: &wgpu::Device, width: u32, height: u32) -> Self {
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
