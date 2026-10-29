use crate::backend::{NO_STRETCH, Stretch};
use crate::bitmap::BitmapSource;
use crate::shape_utils::{DistilledShape, DrawCommand, DrawPath, GradientType};
use indexmap::IndexSet;
use lyon::path::Path;
use lyon::tessellation::{
    self, FillTessellator, FillVertex, StrokeTessellator, StrokeVertex, StrokeVertexConstructor,
    geometry_builder::{BuffersBuilder, FillVertexConstructor, VertexBuffers},
};
use lyon::tessellation::{FillOptions, StrokeOptions};
use swf::GradientRecord;
use tracing::instrument;

pub struct ShapeTessellator {
    fill_tess: FillTessellator,
    stroke_tess: StrokeTessellator,
    mesh: Vec<Draw>,
    gradients: IndexSet<Gradient>,
    lyon_mesh: VertexBuffers<Vertex, u32>,
    mask_index_count: Option<u32>,
    is_stroke: bool,
}

const TESSELLATION_EPSILON: f32 = 0.0000001;

/// AQW build: Flash-style edge smoothing (see `edge_aa`), set by the stage quality.
///
/// Medium quality has the same soft edges as High: they cost the same, and narrower ones (a
/// quarter pixel each side was tried) are thinner than the pixels they're sampled at, so a thin
/// line's brightness depended on where it fell between pixels (faint 1-pixel outlines vanished).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EdgeMode {
    /// No soft edges (MSAA).
    Off = 0,
    /// Low quality: only thin lines (strokes up to 1.5 pixels wide) are soft; without, a
    /// 1-pixel line between two rows of pixels broke up into dots.
    Lines = 1,
    /// Half a pixel each side of the true edge (a box filter's ramp, like many-sample MSAA).
    Soft = 2,
}

static EDGE_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// AQW build: the soft edges of shapes tessellated from now on. A tessellation is made for one
/// mode: callers keep the mode with it and tessellate again when it changes (`edge_mode`).
pub fn set_edge_mode(mode: EdgeMode) {
    EDGE_MODE.store(mode as u8, std::sync::atomic::Ordering::Relaxed);
}

pub fn edge_mode() -> EdgeMode {
    match EDGE_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => EdgeMode::Lines,
        2 => EdgeMode::Soft,
        _ => EdgeMode::Off,
    }
}

/// AQW build: whether shapes get Flash-style soft edges.
pub fn edge_aa() -> bool {
    edge_mode() != EdgeMode::Off
}

/// AQW build: shapes with a fill drawn with hard edges, as its outline crosses itself or
/// another of its loops (see `edge_aa::centered_fill`), for the log: their ids, until taken.
static TANGLED_SHAPES: std::sync::Mutex<Vec<swf::CharacterId>> = std::sync::Mutex::new(Vec::new());

fn note_tangled(id: swf::CharacterId) {
    if let Ok(mut ids) = TANGLED_SHAPES.lock()
        && !ids.contains(&id)
        && ids.len() < 64
    {
        ids.push(id);
    }
}

/// Whether shape `id` had such a fill since last asked (then forgets it).
pub fn take_tangled(id: swf::CharacterId) -> bool {
    let Ok(mut ids) = TANGLED_SHAPES.lock() else {
        return false;
    };
    match ids.iter().position(|&i| i == id) {
        Some(index) => {
            ids.swap_remove(index);
            true
        }
        None => false,
    }
}

/// AQW build: whether Flash-style edge smoothing replaces MSAA: `RUFFLE_FLASH_AA` at start,
/// or `set_flash_aa` (Mugetsu's settings). 0 = not read yet, 1 = off, 2 = on.
static FLASH_AA: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn flash_aa_switch() -> bool {
    match FLASH_AA.load(std::sync::atomic::Ordering::Relaxed) {
        0 => {
            let on = std::env::var_os("RUFFLE_FLASH_AA").is_some();
            set_flash_aa(on);
            on
        }
        1 => false,
        _ => true,
    }
}

/// AQW build: switches Flash-style edge smoothing; it applies once the drawing surface is made
/// again (`RenderBackend::set_quality`), and shapes are tessellated again as they're drawn.
pub fn set_flash_aa(on: bool) {
    FLASH_AA.store(if on { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

impl ShapeTessellator {
    pub fn new() -> Self {
        Self {
            fill_tess: FillTessellator::new(),
            stroke_tess: StrokeTessellator::new(),
            mesh: Vec::new(),
            gradients: IndexSet::new(),
            lyon_mesh: VertexBuffers::new(),
            mask_index_count: None,
            is_stroke: false,
        }
    }

    #[instrument(level = "debug", skip_all)]
    pub fn tessellate_shape(
        &mut self,
        shape: DistilledShape,
        bitmap_source: &dyn BitmapSource,
    ) -> Mesh {
        self.tessellate_shape_with_scale(shape, bitmap_source, 1.0)
    }

    pub fn tessellate_shape_with_scale(
        &mut self,
        shape: DistilledShape,
        bitmap_source: &dyn BitmapSource,
        scale: f32,
    ) -> Mesh {
        self.tessellate_shape_stretched(shape, bitmap_source, scale, NO_STRETCH)
    }

    /// AQW build: `tessellate_shape_with_scale` for a shape drawn squashed: Flash-style soft
    /// edges are made half a pixel wide on screen in every direction (see `Stretch`).
    #[instrument(level = "debug", skip_all)]
    pub fn tessellate_shape_stretched(
        &mut self,
        shape: DistilledShape,
        bitmap_source: &dyn BitmapSource,
        scale: f32,
        stretch: Stretch,
    ) -> Mesh {
        let squash = edge_aa::Squash::new(stretch);
        // AQW build: the scale hairlines get their minimum width from: Ruffle's (the average over
        // both axes). A squashed shape's `scale` is its area's, so with that a hairline was a full
        // pixel even across the squashed direction, where it's usually a third of one (a
        // background's red guide lines, stretched 7x wide, showed up as a stripe).
        let line_scale = scale * stretch[3];
        // Curves are flattened as finely as Ruffle's own scale asks (a squashed shape's `scale`
        // is its area's, smaller: curves came out coarse).
        let detail = line_scale;
        self.mesh = Vec::new();
        self.gradients = IndexSet::new();
        self.lyon_mesh = VertexBuffers::new();

        // AQW build: for Flash-style edges, which fill edges are shared between two fills.
        let mode = edge_mode();
        let edge_aa_on = mode != EdgeMode::Off;
        let shared_edges = if mode == EdgeMode::Soft {
            edge_aa::Shared::new(&shape.paths)
        } else {
            Default::default()
        };

        let shape_id = shape.id;
        for (path_index, path) in shape.paths.into_iter().enumerate() {
            // AQW build: whether this path's edges are soft (see `EdgeMode`).
            let edge_aa_on = edge_aa_on
                && match &path {
                    DrawPath::Fill { .. } => mode == EdgeMode::Soft,
                    DrawPath::Stroke { style, .. } => {
                        mode == EdgeMode::Soft
                            || style.width().to_pixels() as f32 * scale.abs() <= 1.5
                    }
                };
            let (fill_style, lyon_path, next_is_stroke) = match &path {
                DrawPath::Fill {
                    style,
                    commands,
                    winding_rule: _,
                } => (*style, ruffle_path_to_lyon_path(commands, true), false),
                DrawPath::Stroke {
                    style,
                    commands,
                    is_closed,
                } => (
                    style.fill_style(),
                    ruffle_path_to_lyon_path(commands, *is_closed),
                    true,
                ),
            };

            let (draw, color, needs_flush) = match fill_style {
                swf::FillStyle::Color(color) => (DrawType::Color, *color, false),
                swf::FillStyle::LinearGradient(gradient) => {
                    let uniform =
                        swf_gradient_to_uniforms(GradientType::Linear, gradient, swf::Fixed8::ZERO);
                    let (gradient_index, _) = self.gradients.insert_full(uniform);

                    (
                        DrawType::Gradient {
                            matrix: swf_to_gl_matrix(gradient.matrix.into()),
                            gradient: gradient_index,
                        },
                        swf::Color::WHITE,
                        true,
                    )
                }
                swf::FillStyle::RadialGradient(gradient) => {
                    let uniform =
                        swf_gradient_to_uniforms(GradientType::Radial, gradient, swf::Fixed8::ZERO);
                    let (gradient_index, _) = self.gradients.insert_full(uniform);
                    (
                        DrawType::Gradient {
                            matrix: swf_to_gl_matrix(gradient.matrix.into()),
                            gradient: gradient_index,
                        },
                        swf::Color::WHITE,
                        true,
                    )
                }
                swf::FillStyle::FocalGradient {
                    gradient,
                    focal_point,
                } => {
                    let uniform =
                        swf_gradient_to_uniforms(GradientType::Focal, gradient, *focal_point);
                    let (gradient_index, _) = self.gradients.insert_full(uniform);
                    (
                        DrawType::Gradient {
                            matrix: swf_to_gl_matrix(gradient.matrix.into()),
                            gradient: gradient_index,
                        },
                        swf::Color::WHITE,
                        true,
                    )
                }
                swf::FillStyle::Bitmap {
                    id,
                    matrix,
                    is_smoothed,
                    is_repeating,
                } => {
                    if let Some(bitmap) = bitmap_source.bitmap_size(*id) {
                        (
                            DrawType::Bitmap(Bitmap {
                                matrix: swf_bitmap_to_gl_matrix(
                                    (*matrix).into(),
                                    bitmap.width,
                                    bitmap.height,
                                ),
                                bitmap_id: *id,
                                is_smoothed: *is_smoothed,
                                is_repeating: *is_repeating,
                            }),
                            swf::Color::WHITE,
                            true,
                        )
                    } else {
                        // Missing bitmap -- incorrect character ID in SWF?
                        continue;
                    }
                }
            };

            if needs_flush || (self.is_stroke && !next_is_stroke) {
                // We flush separate draw calls in these cases:
                // * Non-solid color fills which require their own shader.
                // * Strokes followed by fills, because strokes need to be omitted
                //   when using this shape as a mask.
                self.flush_draw(DrawType::Color);
            } else if !self.is_stroke && next_is_stroke {
                // Bake solid color fills followed by strokes into a single draw call, and adjust
                // the index count to omit the strokes when rendering this shape as a mask.
                assert!(self.mask_index_count.is_none());
                self.mask_index_count = Some(self.lyon_mesh.indices.len() as u32);
            }
            self.is_stroke = next_is_stroke;

            let mut buffers_builder =
                BuffersBuilder::new(&mut self.lyon_mesh, RuffleVertexCtor { color });
            // AQW build: Flash-style edges: the fill's outline moved half a pixel in (out where
            // it lies under another fill), plus a fade across the edge (see `edge_aa`).
            let centered = match &path {
                DrawPath::Fill {
                    commands,
                    winding_rule,
                    ..
                } if edge_aa_on => {
                    let centered = edge_aa::centered_fill(
                        commands,
                        &shared_edges,
                        path_index,
                        (*winding_rule).into(),
                        scale,
                        &squash,
                    );
                    if edge_aa::TANGLED.with(|t| t.replace(false)) {
                        note_tangled(shape_id);
                    }
                    centered
                }
                _ => None,
            };
            let result = match path {
                DrawPath::Fill { winding_rule, .. } => {
                    // Larger scales require more precise tessellation to avoid artifacts
                    let tolerance = FillOptions::DEFAULT_TOLERANCE / detail;
                    // A moved outline is wound so its inside is positive (see `centered_fill`).
                    let fill_rule = if centered.is_some() {
                        lyon::path::FillRule::Positive
                    } else {
                        winding_rule.into()
                    };
                    self.fill_tess.tessellate_path(
                        centered.as_ref().map_or(&lyon_path, |c| &c.outline),
                        &FillOptions::default()
                            .with_fill_rule(fill_rule)
                            .with_tolerance(tolerance),
                        &mut buffers_builder,
                    )
                }
                DrawPath::Stroke { style, .. } => {
                    // This calculation ensures that hairline strokes are rendered with
                    // a minimum width of 1 pixel, while still allowing for proper scaling
                    let width = style.width().to_pixels() as f32;
                    let min_screen_width = if f32::abs(line_scale) > TESSELLATION_EPSILON {
                        1.0 / line_scale
                    } else {
                        1.0
                    };
                    let width = width.max(min_screen_width);
                    // AQW build: with Flash-style edges, the body is a pixel narrower and the
                    // fades on both sides make up the rest (see `edge_aa::stroke`).
                    let width = if edge_aa_on {
                        edge_aa::stroke_body_width(width, scale)
                    } else {
                        width
                    };

                    // Larger scales require more precise tessellation to avoid artifacts
                    let tolerance = StrokeOptions::DEFAULT_TOLERANCE / detail;
                    let mut stroke_options = StrokeOptions::default()
                        .with_line_width(width)
                        .with_tolerance(tolerance)
                        .with_start_cap(match style.start_cap() {
                            swf::LineCapStyle::None => tessellation::LineCap::Butt,
                            swf::LineCapStyle::Round => tessellation::LineCap::Round,
                            swf::LineCapStyle::Square => tessellation::LineCap::Square,
                        })
                        .with_end_cap(match style.end_cap() {
                            swf::LineCapStyle::None => tessellation::LineCap::Butt,
                            swf::LineCapStyle::Round => tessellation::LineCap::Round,
                            swf::LineCapStyle::Square => tessellation::LineCap::Square,
                        });

                    let line_join = match style.join_style() {
                        swf::LineJoinStyle::Round => tessellation::LineJoin::Round,
                        swf::LineJoinStyle::Bevel => tessellation::LineJoin::Bevel,
                        swf::LineJoinStyle::Miter(limit) => {
                            // Avoid lyon assert with small miter limits.
                            let limit = limit.to_f32();
                            if limit >= StrokeOptions::MINIMUM_MITER_LIMIT {
                                stroke_options = stroke_options.with_miter_limit(limit);
                                tessellation::LineJoin::MiterClip
                            } else {
                                tessellation::LineJoin::Bevel
                            }
                        }
                    };
                    stroke_options = stroke_options.with_line_join(line_join);
                    if width <= TESSELLATION_EPSILON {
                        // A hairline: only its fades are drawn.
                        Ok(())
                    } else {
                        self.stroke_tess.tessellate_path(
                            &lyon_path,
                            &stroke_options,
                            &mut buffers_builder,
                        )
                    }
                }
            };
            // AQW build: Flash-style soft edges (see `edge_aa`).
            if result.is_ok() && edge_aa_on {
                match &path {
                    DrawPath::Fill { winding_rule, .. } => match &centered {
                        Some(centered) => {
                            for (part, alpha) in &centered.thin {
                                let mut builder = BuffersBuilder::new(
                                    &mut self.lyon_mesh,
                                    RuffleVertexCtor { color: edge_aa::with_alpha(color, *alpha) },
                                );
                                let _ = self.fill_tess.tessellate_path(
                                    part,
                                    &FillOptions::default()
                                        .with_tolerance(FillOptions::DEFAULT_TOLERANCE / detail),
                                    &mut builder,
                                );
                            }
                            edge_aa::push_fades(&mut self.lyon_mesh, centered, color)
                        }
                        None => edge_aa::fill(
                            &mut self.lyon_mesh,
                            &lyon_path,
                            (*winding_rule).into(),
                            color,
                            scale,
                            &squash,
                        ),
                    },
                    DrawPath::Stroke { style, .. } => {
                        let width = (style.width().to_pixels() as f32)
                            .max(if line_scale.abs() > TESSELLATION_EPSILON { 1.0 / line_scale } else { 1.0 });
                        edge_aa::stroke(
                            &mut self.lyon_mesh,
                            &lyon_path,
                            width,
                            (style.start_cap(), style.end_cap()),
                            color,
                            scale,
                            &squash,
                        );
                    }
                }
            }
            match result {
                Ok(_) => {
                    if needs_flush {
                        // Non-solid color fills are isolated draw calls; flush immediately.
                        self.flush_draw(draw);
                    }
                }
                Err(e) => {
                    // This may simply be a degenerate path.
                    tracing::error!("Tessellation failure: {:?}", e);
                }
            }
        }

        // Flush the final pending draw.
        self.flush_draw(DrawType::Color);

        self.lyon_mesh = VertexBuffers::new();
        Mesh {
            draws: std::mem::take(&mut self.mesh),
            gradients: std::mem::take(&mut self.gradients).into_iter().collect(),
        }
    }

    fn flush_draw(&mut self, draw: DrawType) {
        if self.lyon_mesh.vertices.is_empty() || self.lyon_mesh.indices.len() < 3 {
            // Ignore degenerate fills
            self.lyon_mesh = VertexBuffers::new();
            self.mask_index_count = None;
            return;
        }
        let draw_mesh = std::mem::replace(&mut self.lyon_mesh, VertexBuffers::new());
        self.mesh.push(Draw {
            draw_type: draw,
            mask_index_count: self
                .mask_index_count
                .unwrap_or(draw_mesh.indices.len() as u32),
            vertices: draw_mesh.vertices,
            indices: draw_mesh.indices,
        });
        self.mask_index_count = None;
    }
}

impl Default for ShapeTessellator {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Mesh {
    pub draws: Vec<Draw>,
    pub gradients: Vec<Gradient>,
}

pub struct Draw {
    pub draw_type: DrawType,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub mask_index_count: u32,
}

pub enum DrawType {
    Color,
    Gradient {
        matrix: [[f32; 3]; 3],
        gradient: usize,
    },
    Bitmap(Bitmap),
}

impl DrawType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Color => "Color",
            Self::Gradient { .. } => "Gradient",
            Self::Bitmap { .. } => "Bitmap",
        }
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Gradient {
    pub gradient_type: GradientType,
    pub repeat_mode: swf::GradientSpread,
    pub focal_point: swf::Fixed8,
    pub interpolation: swf::GradientInterpolation,
    pub records: Vec<GradientRecord>,
}

#[derive(Clone, Debug)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub color: swf::Color,
}

#[derive(Clone, Debug)]
pub struct Bitmap {
    pub matrix: [[f32; 3]; 3],
    pub bitmap_id: u16,
    pub is_smoothed: bool,
    pub is_repeating: bool,
}

#[expect(clippy::many_single_char_names)]
fn swf_to_gl_matrix(m: crate::matrix::Matrix) -> [[f32; 3]; 3] {
    let tx = m.tx.get() as f32;
    let ty = m.ty.get() as f32;
    let det = m.a * m.d - m.c * m.b;
    let mut a = m.d / det;
    let mut b = -m.c / det;
    let mut c = -(tx * m.d - m.c * ty) / det;
    let mut d = -m.b / det;
    let mut e = m.a / det;
    let mut f = (tx * m.b - m.a * ty) / det;

    a *= 20.0 / 32768.0;
    b *= 20.0 / 32768.0;
    d *= 20.0 / 32768.0;
    e *= 20.0 / 32768.0;

    c /= 32768.0;
    f /= 32768.0;
    c += 0.5;
    f += 0.5;
    [[a, d, 0.0], [b, e, 0.0], [c, f, 1.0]]
}

#[expect(clippy::many_single_char_names)]
fn swf_bitmap_to_gl_matrix(
    m: crate::matrix::Matrix,
    bitmap_width: u32,
    bitmap_height: u32,
) -> [[f32; 3]; 3] {
    let bitmap_width = bitmap_width as f32;
    let bitmap_height = bitmap_height as f32;

    let tx = m.tx.get() as f32;
    let ty = m.ty.get() as f32;
    let det = m.a * m.d - m.c * m.b;
    let mut a = m.d / det;
    let mut b = -m.c / det;
    let mut c = -(tx * m.d - m.c * ty) / det;
    let mut d = -m.b / det;
    let mut e = m.a / det;
    let mut f = (tx * m.b - m.a * ty) / det;

    a *= 20.0 / bitmap_width;
    b *= 20.0 / bitmap_width;
    d *= 20.0 / bitmap_height;
    e *= 20.0 / bitmap_height;

    c /= bitmap_width;
    f /= bitmap_height;

    [[a, d, 0.0], [b, e, 0.0], [c, f, 1.0]]
}

fn ruffle_path_to_lyon_path(commands: &[DrawCommand], is_closed: bool) -> Path {
    fn point(point: swf::Point<swf::Twips>) -> lyon::math::Point {
        lyon::math::Point::new(point.x.to_pixels() as f32, point.y.to_pixels() as f32)
    }

    let mut builder = Path::builder();
    let mut cursor = Some(swf::Point::ZERO);
    for command in commands {
        match command {
            DrawCommand::MoveTo(move_to) => {
                if cursor.is_none() {
                    builder.end(false);
                }
                cursor = Some(*move_to);
            }
            DrawCommand::LineTo(line_to) => {
                if let Some(cursor) = cursor.take() {
                    builder.begin(point(cursor));
                }
                builder.line_to(point(*line_to));
            }
            DrawCommand::QuadraticCurveTo { control, anchor } => {
                if let Some(cursor) = cursor.take() {
                    builder.begin(point(cursor));
                }
                builder.quadratic_bezier_to(point(*control), point(*anchor));
            }
            DrawCommand::CubicCurveTo {
                control_a,
                control_b,
                anchor,
            } => {
                if let Some(cursor) = cursor.take() {
                    builder.begin(point(cursor));
                }
                builder.cubic_bezier_to(point(*control_a), point(*control_b), point(*anchor));
            }
        }
    }

    if cursor.is_none() {
        if is_closed {
            builder.close();
        } else {
            builder.end(false);
        }
    }

    builder.build()
}

/// AQW build: Flash-style edge smoothing. Flash didn't use MSAA: it computed how much of
/// each edge pixel a shape covers and used that as its alpha. Here each shape is drawn as usual
/// (pixels whose centres are inside are fully covered) plus a thin strip along its outline,
/// outside the shape, fading from half the shape's alpha at the edge to nothing half a pixel
/// out: the coverage a 1-pixel box filter gives the pixels just outside (the same scheme as
/// NanoVG's antialiasing). It costs a few triangles per edge, instead of 2-4 samples per pixel
/// and a resolve for every layer (MSAA), which is what made Medium and High slow on weak
/// graphics. Solid colors carry the fade in their vertex alpha; gradients and bitmaps in their
/// vertex color too (white, its alpha passed as the coverage of their `uv.z`).
mod edge_aa {
    use super::{FillOptions, Path, Vertex, VertexBuffers};
    use crate::shape_utils::{DrawCommand, DrawPath};
    use lyon::math::{Point, Vector};
    use lyon::path::PathEvent;
    use lyon::path::iterator::PathIterator as _;

    const EPSILON: f32 = 1e-12;

    /// Maps a shape's points to where it isn't squashed (screen space, up to a scale and a
    /// rotation; see `Stretch`), and back. The soft edges are worked out there, so they're
    /// half a pixel wide on screen in every direction (a floor circle drawn flat kept its thin
    /// line's edges too narrow across, and the line came out dashed).
    #[derive(Clone, Copy)]
    pub struct Squash {
        to: [f32; 4],
        from: [f32; 4],
    }

    impl Squash {
        pub fn new(stretch: super::Stretch) -> Self {
            // The symmetric square root of the stretch (its determinant is 1).
            let [xx, xy, yy, _] = stretch;
            let root = (xx + yy + 2.0).max(EPSILON).sqrt();
            let to = [(xx + 1.0) / root, xy / root, xy / root, (yy + 1.0) / root];
            let det = to[0] * to[3] - to[1] * to[2];
            let det = if det.abs() > EPSILON { det } else { 1.0 };
            let from = [to[3] / det, -to[1] / det, -to[2] / det, to[0] / det];
            Self { to, from }
        }

        fn map(m: &[f32; 4], p: Point) -> Point {
            Point::new(m[0] * p.x + m[1] * p.y, m[2] * p.x + m[3] * p.y)
        }

        fn apply(&self, p: Point) -> Point {
            Self::map(&self.to, p)
        }

        fn undo(&self, p: Point) -> Point {
            Self::map(&self.from, p)
        }

        /// How much farther a stroke reaches in direction `v` (a unit vector here) than in
        /// the shape: a stroke is round in the shape, and squashed into an ellipse here.
        fn widen(&self, v: Vector) -> f32 {
            let p = Self::map(&self.to, Point::new(v.x, v.y));
            p.x.hypot(p.y)
        }

        /// Maps the vertices pushed since `start` back to the shape.
        fn undo_vertices(&self, mesh: &mut VertexBuffers<Vertex, u32>, start: usize) {
            for v in &mut mesh.vertices[start..] {
                let p = self.undo(Point::new(v.x, v.y));
                (v.x, v.y) = (p.x, p.y);
            }
        }

        fn polylines(&self, path: &Path, tolerance: f32) -> Vec<(Vec<Point>, bool)> {
            let mut lines = polylines(path, tolerance);
            for (points, _) in &mut lines {
                for p in points.iter_mut() {
                    *p = self.apply(*p);
                }
            }
            lines
        }
    }

    /// The polylines of `path`, flattened; `true` for closed ones.
    fn polylines(path: &Path, tolerance: f32) -> Vec<(Vec<Point>, bool)> {
        let mut lines: Vec<(Vec<Point>, bool)> = Vec::new();
        for event in path.iter().flattened(tolerance) {
            match event {
                PathEvent::Begin { at } => lines.push((vec![at], false)),
                PathEvent::Line { to, .. } => {
                    if let Some((line, _)) = lines.last_mut()
                        && line.last().is_none_or(|last| (*last - to).square_length() > EPSILON)
                    {
                        line.push(to);
                    }
                }
                PathEvent::End { close, .. } => {
                    if let Some((line, closed)) = lines.last_mut() {
                        if line.len() > 2
                            && (line[0] - line[line.len() - 1]).square_length() <= EPSILON
                        {
                            line.pop();
                            *closed = true;
                        }
                        *closed |= close;
                    }
                }
                _ => {}
            }
        }
        lines
    }

    fn left_normal(a: Point, b: Point) -> Vector {
        let d = (b - a).normalize();
        Vector::new(-d.y, d.x)
    }

    /// Unit left normals averaged at each vertex, lengthened for the miter (at most 2x).
    fn vertex_normals(points: &[Point], closed: bool) -> Vec<Vector> {
        let n = points.len();
        (0..n)
            .map(|i| {
                let prev = if i > 0 {
                    Some(left_normal(points[i - 1], points[i]))
                } else if closed {
                    Some(left_normal(points[n - 1], points[0]))
                } else {
                    None
                };
                let next = if i + 1 < n {
                    Some(left_normal(points[i], points[i + 1]))
                } else if closed {
                    Some(left_normal(points[n - 1], points[0]))
                } else {
                    None
                };
                match (prev, next) {
                    (Some(a), Some(b)) => {
                        let sum = a + b;
                        if sum.square_length() < EPSILON {
                            b
                        } else {
                            let miter = sum.normalize();
                            miter / miter.dot(b).max(0.5)
                        }
                    }
                    (Some(a), None) => a,
                    (None, Some(b)) => b,
                    (None, None) => Vector::zero(),
                }
            })
            .collect()
    }

    /// A strip from `points` (at half the alpha) out by `offsets` (to none).
    fn push_strip(
        mesh: &mut VertexBuffers<Vertex, u32>,
        points: &[Point],
        offsets: &[Vector],
        closed: bool,
        color: swf::Color,
    ) {
        push_strip_from(mesh, points, offsets, closed, swf::Color { a: color.a / 2, ..color }, color);
    }

    /// A strip from `points` (in `edge`) out by `offsets` (to `color` with no alpha).
    fn push_strip_from(
        mesh: &mut VertexBuffers<Vertex, u32>,
        points: &[Point],
        offsets: &[Vector],
        closed: bool,
        edge: swf::Color,
        color: swf::Color,
    ) {
        let n = points.len();
        if n < 2 {
            return;
        }
        let clear = swf::Color { a: 0, ..color };
        let base = mesh.vertices.len() as u32;
        for (point, offset) in points.iter().zip(offsets) {
            let outer = *point + *offset;
            mesh.vertices.push(Vertex { x: point.x, y: point.y, color: edge });
            mesh.vertices.push(Vertex { x: outer.x, y: outer.y, color: clear });
        }
        let segments = if closed { n } else { n - 1 } as u32;
        for i in 0..segments {
            let j = (i + 1) % n as u32;
            let (inner_i, outer_i) = (base + 2 * i, base + 2 * i + 1);
            let (inner_j, outer_j) = (base + 2 * j, base + 2 * j + 1);
            mesh.indices
                .extend_from_slice(&[inner_i, outer_i, outer_j, inner_i, outer_j, inner_j]);
        }
    }

    /// Half a pixel at the scale the shape is tessellated at, and the flattening tolerance.
    fn sizes(scale: f32) -> Option<(f32, f32)> {
        let half = 0.5;
        (scale.abs() > 1e-7).then(|| (half / scale.abs(), FillOptions::DEFAULT_TOLERANCE / scale.abs()))
    }

    /// An edge as the SWF gives it, the same whichever way it's walked: its end points (and
    /// control points) in twips.
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct EdgeKey([i32; 8]);

    fn bytemuck_bytes(key: &[i32; 8]) -> &[u8] {
        // SAFETY: `[i32; 8]` is 32 plain bytes with no padding.
        unsafe { std::slice::from_raw_parts(key.as_ptr().cast::<u8>(), 32) }
    }

    /// Also whether it's walked forward (from its lower end): fills are wound with their inside
    /// on the same side of the way they walk (`shape_utils` flips fill style 0's edges), so two
    /// fills on opposite sides of an edge walk it opposite ways, and a fill lying over another
    /// along it (a later layer of the shape ending on the same outline) walks it the same way.
    fn edge_key(from: swf::Point<swf::Twips>, command: &DrawCommand) -> Option<(EdgeKey, bool)> {
        let p = |q: swf::Point<swf::Twips>| [q.x.get(), q.y.get()];
        let (a, b) = (p(from), p(command.end_point()));
        let forward = a <= b;
        let (lo, hi) = if forward { (a, b) } else { (b, a) };
        let key = EdgeKey(match command {
            DrawCommand::MoveTo(_) => return None,
            DrawCommand::LineTo(_) => [lo[0], lo[1], hi[0], hi[1], 0, 0, i32::MIN, 0],
            DrawCommand::QuadraticCurveTo { control, .. } => {
                let c = p(*control);
                [lo[0], lo[1], hi[0], hi[1], c[0], c[1], i32::MIN + 1, 0]
            }
            DrawCommand::CubicCurveTo {
                control_a,
                control_b,
                ..
            } => {
                let (c1, c2) = if forward {
                    (p(*control_a), p(*control_b))
                } else {
                    (p(*control_b), p(*control_a))
                };
                [lo[0], lo[1], hi[0], hi[1], c1[0], c1[1], c2[0], c2[1]]
            }
        });
        Some((key, forward))
    }

    /// Which fills each edge of a shape belongs to (see `Side`).
    /// Each fill edge's `Side`, worked out for a whole shape at once (by sorting its edges, so
    /// edges shared between fills end up next to each other).
    #[derive(Default)]
    pub struct Shared {
        /// Per path (by index in `paths`; empty for strokes), per command.
        sides: Vec<Vec<Side>>,
    }

    /// The line a straight edge is on (its direction, reduced, and offset) and where along the
    /// line it starts and ends, in twips; the same whichever way it's walked.
    fn line_of(
        from: swf::Point<swf::Twips>,
        to: swf::Point<swf::Twips>,
    ) -> Option<([i64; 3], i64, i64, bool)> {
        let (x0, y0) = (i64::from(from.x.get()), i64::from(from.y.get()));
        let (x1, y1) = (i64::from(to.x.get()), i64::from(to.y.get()));
        let (mut dx, mut dy) = (x1 - x0, y1 - y0);
        if dx == 0 && dy == 0 {
            return None;
        }
        let (mut a, mut b) = (dx.abs(), dy.abs());
        while b != 0 {
            (a, b) = (b, a % b);
        }
        (dx, dy) = (dx / a, dy / a);
        if dx < 0 || (dx == 0 && dy < 0) {
            (dx, dy) = (-dx, -dy);
        }
        let (t0, t1) = (dx * x0 + dy * y0, dx * x1 + dy * y1);
        Some(([dx, dy, dy * x0 - dx * y0], t0.min(t1), t0.max(t1), t0 < t1))
    }

    impl Shared {
        pub fn new(paths: &[DrawPath]) -> Self {
            let mut sides: Vec<Vec<Side>> = paths
                .iter()
                .map(|path| match path {
                    DrawPath::Fill { commands, .. } => vec![Side::Faded; commands.len()],
                    DrawPath::Stroke { .. } => Vec::new(),
                })
                .collect();
            // (edge's hash, edge, fill, command), and straight edges as (line's hash, line, fill,
            // command, start, end): sorted by the hash first (cheaper than the whole key).
            let hash = |bytes: &[u8]| {
                use std::hash::BuildHasher;
                static STATE: std::sync::LazyLock<hashbrown::DefaultHashBuilder> =
                    std::sync::LazyLock::new(Default::default);
                STATE.hash_one(bytes)
            };
            let fills = paths.iter().filter(|path| matches!(path, DrawPath::Fill { .. })).count();
            // (Each also with the direction it's walked in, see `edge_key`.)
            let mut edges: Vec<(u64, EdgeKey, usize, usize, bool)> = Vec::new();
            let mut lines: Vec<(u64, [i64; 3], usize, usize, i64, i64, bool)> = Vec::new();
            for (fill, path) in paths.iter().enumerate() {
                let DrawPath::Fill { commands, .. } = path else {
                    continue;
                };
                let mut cursor = swf::Point::ZERO;
                for (command_index, command) in commands.iter().enumerate() {
                    if let Some((key, forward)) = edge_key(cursor, command) {
                        let h = hash(bytemuck_bytes(&key.0));
                        edges.push((h, key, fill, command_index, forward));
                    }
                    if fills > 1
                        && let DrawCommand::LineTo(to) = command
                        && let Some((line, t0, t1, forward)) = line_of(cursor, *to)
                    {
                        let h = hash(&[line[0].to_le_bytes(), line[1].to_le_bytes(), line[2].to_le_bytes()].concat());
                        lines.push((h, line, fill, command_index, t0, t1, forward));
                    }
                    cursor = command.end_point();
                }
            }
            // Split differently (one fill has an edge whole, another in parts, which the exact
            // match below misses; the background then showed through a seam): below another
            // fill where most of it lies along a later fill's straight edges and no earlier's.
            lines.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
            for group in lines.chunk_by(|a, b| a.0 == b.0 && a.1 == b.1) {
                if group.len() < 2 || group.len() > 64 {
                    continue;
                }
                for &(_, _, fill, command, t0, t1, forward) in group {
                    // Neighbours walk the line the other way (see `edge_key`). Where a later
                    // fill on the same side lies over this edge all along, it's left as it is.
                    let (mut later, mut earlier, mut overlaid) = (false, false, false);
                    for &(_, _, other, _, s0, s1, other_forward) in group {
                        if other == fill {
                            continue;
                        }
                        if other_forward == forward {
                            overlaid |= other > fill && s0 <= t0 && s1 >= t1;
                        } else if (s1.min(t1) - s0.max(t0)) * 2 >= t1 - t0 {
                            later |= other > fill;
                            earlier |= other < fill;
                        }
                    }
                    if later && !earlier && !overlaid {
                        sides[fill][command] = if group
                            .iter()
                            .all(|&(_, _, other, ..)| opaque(&paths[other]))
                        {
                            Side::Under
                        } else {
                            Side::Hard
                        };
                    }
                }
            }
            // The same edge in several fills: below another fill when one drawn later shares it
            // and none drawn earlier does. Between two parts of the same fill there's no edge to
            // see: both extend under.
            edges.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
            for group in edges.chunk_by(|a, b| a.0 == b.0 && a.1 == b.1) {
                if group.len() < 2 {
                    continue;
                }
                let all_opaque = group.iter().all(|e| opaque(&paths[e.2]));
                for &(_, _, fill, command, forward) in group {
                    // Only fills on the other side (walking the edge the other way) are
                    // neighbours. A later fill on the same side lies over this one, ending on
                    // the same outline (a dark part drawn over a red glow, both ending at an
                    // eye's rim): extended out under it, this one showed past the edge (a red
                    // line around the eye). Then both just fade, like two of Flash's layers.
                    // (Nor under a neighbour, while a later fill lies over it along the edge.)
                    let overlaid = group.iter().any(|e| e.2 > fill && e.4 == forward);
                    let mut neighbours = group.iter().filter(|e| e.4 != forward).map(|e| e.2);
                    let other = match neighbours.next() {
                        Some(other) if !overlaid => other,
                        _ => {
                            sides[fill][command] = if all_opaque { Side::Faded } else { Side::Hard };
                            continue;
                        }
                    };
                    let (first, last) = neighbours
                        .fold((fill.min(other), fill.max(other)), |(lo, hi), e| (lo.min(e), hi.max(e)));
                    sides[fill][command] = if !all_opaque {
                        Side::Hard
                    } else if first == last || (last > fill && first >= fill) {
                        Side::Under
                    } else {
                        Side::Faded
                    };
                }
            }
            Self { sides }
        }
    }

    /// A fill's outline for Flash-style edges, and the fades to draw across its edges.
    pub struct Centered {
        pub outline: Path,
        /// Parts thinner than a pixel (see `centered_fill`), each drawn on its own at the
        /// alpha given (times the fill's).
        pub thin: Vec<(Path, f32)>,
        /// The fades: strips through these points, inner (at the alpha given, times the fill's)
        /// and outer (none) ...
        fade_points: Vec<(Point, Point)>,
        /// ... as (first point, points, alpha, closed).
        fade_runs: Vec<(usize, usize, f32, bool)>,
    }

    /// The winding numbers of a fill's outline at `probes`, in one pass: the probes are sorted
    /// by height, so each segment only looks at the probes beside it (testing each contour
    /// against the whole outline made big shapes slow to prepare).
    fn windings_at(rings: &[(Vec<Point>, Vec<Side>)], probes: &[Point]) -> Vec<i16> {
        let mut order: Vec<usize> = (0..probes.len()).collect();
        order.sort_unstable_by(|&i, &j| probes[i].y.total_cmp(&probes[j].y));
        let ys: Vec<f32> = order.iter().map(|&i| probes[i].y).collect();
        let mut windings = vec![0i16; probes.len()];
        for (points, _) in rings {
            for i in 0..points.len() {
                let (a, b) = (points[i], points[(i + 1) % points.len()]);
                if a.y == b.y {
                    continue;
                }
                // The probes with `min.y <= y < max.y` (where the edge crosses their row).
                let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
                let start = ys.partition_point(|&y| y < lo);
                let end = ys.partition_point(|&y| y < hi);
                for &k in &order[start..end] {
                    let p = probes[k];
                    if a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) > p.x {
                        windings[k] += if b.y > a.y { 1 } else { -1 };
                    }
                }
            }
        }
        windings
    }

    /// Whether `p` is inside the closed polygon `points` (even-odd).
    fn in_polygon(points: &[Point], p: Point) -> bool {
        let mut inside = false;
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            if (a.y <= p.y) != (b.y <= p.y)
                && a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) > p.x
            {
                inside = !inside;
            }
        }
        inside
    }

    /// `color` at `alpha` times its alpha.
    pub fn with_alpha(color: swf::Color, alpha: f32) -> swf::Color {
        swf::Color { a: (f32::from(color.a) * alpha).round() as u8, ..color }
    }

    /// Where an edge is: against the background or above another fill (faded across the
    /// edge, the fill moved in half a pixel), or below another fill (the fill extended half a
    /// pixel under it, no fade: the fill above fades over it, so no background shows through),
    /// or shared with a see-through fill (`Hard`: left where it is, not faded; the two fills
    /// meet exactly, as MSAA's samples would split them, since with see-through fills the
    /// extended one showed through the faded one: a bright line along a glow's two halves).
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Side {
        Faded,
        Under,
        Hard,
    }

    /// Whether a fill is opaque everywhere (bitmaps may not be).
    fn opaque(path: &DrawPath) -> bool {
        match path {
            DrawPath::Fill { style, .. } => match style {
                swf::FillStyle::Color(color) => color.a == 255,
                swf::FillStyle::LinearGradient(gradient)
                | swf::FillStyle::RadialGradient(gradient)
                | swf::FillStyle::FocalGradient { gradient, .. } => {
                    gradient.records.iter().all(|record| record.color.a == 255)
                }
                swf::FillStyle::Bitmap { .. } => false,
            },
            DrawPath::Stroke { .. } => false,
        }
    }

    thread_local! {
        /// Set when `centered_fill` finds a fill it can't move (see there).
        pub static TANGLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// The outline of a fill moved for centered soft edges (see `Side`), or `None` where that
    /// can't be done cleanly (a fill whose outline crosses itself): then the outer-only fade
    /// is used.
    pub fn centered_fill(
        commands: &[DrawCommand],
        shared: &Shared,
        index: usize,
        fill_rule: lyon::path::FillRule,
        scale: f32,
        squash: &Squash,
    ) -> Option<Centered> {
        let (width, tolerance) = sizes(scale)?;
        // The outline, flattened, each segment with its side.
        let mut contours: Vec<(Vec<Point>, Vec<Side>)> = Vec::new();
        let point = |q: swf::Point<swf::Twips>| {
            squash.apply(Point::new(q.x.to_pixels() as f32, q.y.to_pixels() as f32))
        };
        let mut cursor = swf::Point::ZERO;
        let sides = shared.sides.get(index);
        for (command_index, command) in commands.iter().enumerate() {
            let side = sides
                .and_then(|sides| sides.get(command_index))
                .copied()
                .unwrap_or(Side::Faded);
            let from = point(cursor);
            let push = |to: Point, contours: &mut Vec<(Vec<Point>, Vec<Side>)>| {
                if let Some((points, sides)) = contours.last_mut()
                    && points.last().is_some_and(|last| (*last - to).square_length() > EPSILON)
                {
                    points.push(to);
                    sides.push(side);
                }
            };
            match command {
                DrawCommand::MoveTo(to) => contours.push((vec![point(*to)], Vec::new())),
                DrawCommand::LineTo(to) => push(point(*to), &mut contours),
                DrawCommand::QuadraticCurveTo { control, anchor } => {
                    let curve = lyon::geom::QuadraticBezierSegment {
                        from,
                        ctrl: point(*control),
                        to: point(*anchor),
                    };
                    curve.for_each_flattened(tolerance, &mut |line| push(line.to, &mut contours));
                }
                DrawCommand::CubicCurveTo {
                    control_a,
                    control_b,
                    anchor,
                } => {
                    let curve = lyon::geom::CubicBezierSegment {
                        from,
                        ctrl1: point(*control_a),
                        ctrl2: point(*control_b),
                        to: point(*anchor),
                    };
                    curve.for_each_flattened(tolerance, &mut |line| push(line.to, &mut contours));
                }
            }
            cursor = command.end_point();
        }

        // Closed rings (the last point isn't the first again; an open one closes with a faded
        // edge).
        let rings: Vec<(Vec<Point>, Vec<Side>)> = contours
            .into_iter()
            .filter_map(|(mut points, mut sides)| {
                if points.len() > 1
                    && (points[0] - points[points.len() - 1]).square_length() <= EPSILON
                {
                    points.pop();
                } else {
                    sides.push(Side::Faded);
                }
                (points.len() >= 3 && sides.len() == points.len()).then_some((points, sides))
            })
            .collect();
        let area = |pts: &[Point]| -> f32 {
            (0..pts.len())
                .map(|i| pts[i].to_vector().cross(pts[(i + 1) % pts.len()].to_vector()))
                .sum::<f32>()
        };

        struct Ring {
            points: Vec<Point>,
            sides: Vec<Side>,
            left_is_inside: bool,
            moved: Vec<Point>,
            outer: Vec<Point>,
            /// Twice the area it fills (negative for a hole).
            filled: f32,
            /// Turned inside out by moving: thinner than a pixel.
            all_thin: bool,
        }
        // Which side of each ring is inside: tested just beside the middle of each edge, on
        // both sides; where the fill is on one side only (not on an edge between two rings of
        // the fill), that side gets the edge's length as a vote. (A single test beside the
        // longest edge could land outside a thin part, turning the whole ring the wrong way:
        // it was then left out, a hole in the fill.)
        let mut side_probes = Vec::new();
        let mut side_edges = Vec::new();
        for (r, (points, _)) in rings.iter().enumerate() {
            let n = points.len();
            for i in 0..n {
                let (a, b) = (points[i], points[(i + 1) % n]);
                let length = (b - a).length();
                if length < width * 0.25 {
                    continue;
                }
                let (middle, beside) = (a.lerp(b, 0.5), left_normal(a, b) * (width * 0.25));
                side_probes.push(middle + beside);
                side_probes.push(middle - beside);
                side_edges.push((r, length));
            }
        }
        let side_windings = windings_at(&rings, &side_probes);
        // (Left, right) votes per ring.
        let mut votes = vec![(0.0f32, 0.0f32); rings.len()];
        for (k, &(r, length)) in side_edges.iter().enumerate() {
            let left = fill_rule.is_in(side_windings[2 * k]);
            let right = fill_rule.is_in(side_windings[2 * k + 1]);
            if left && !right {
                votes[r].0 += length;
            } else if right && !left {
                votes[r].1 += length;
            }
        }
        // A ring with a quarter of its length or more the other way crosses itself or another
        // ring of this fill (overlapping loops, a figure eight): it can't be moved as a whole,
        // as those stretches would be left out. Such a fill is drawn as it is, with plain
        // edges. (Less than that is mostly misjudged tests at sharp corners and thin parts: in
        // test renders against an 8x reference, drawing those fills plain was worse.)
        if votes.iter().any(|&(left, right)| {
            let (more, less) = (left.max(right), left.min(right));
            less > width * 4.0 && less > more * 0.25
        }) {
            TANGLED.with(|t| t.set(true));
            return None;
        }
        // Rings without votes (too small to test): beside the middle of the longest edge.
        let fallback_probes: Vec<Point> = rings
            .iter()
            .map(|(points, _)| {
                let n = points.len();
                let longest = (0..n)
                    .max_by(|&i, &j| {
                        let li = (points[(i + 1) % n] - points[i]).square_length();
                        let lj = (points[(j + 1) % n] - points[j]).square_length();
                        li.total_cmp(&lj)
                    })
                    .unwrap_or(0);
                let (a, b) = (points[longest], points[(longest + 1) % n]);
                a.lerp(b, 0.5) + left_normal(a, b) * (width * 0.25)
            })
            .collect();
        let fallback_windings = windings_at(&rings, &fallback_probes);
        let left_inside: Vec<bool> = votes
            .iter()
            .zip(fallback_windings)
            .map(|(&(left, right), winding)| {
                if left > 0.0 || right > 0.0 {
                    left >= right
                } else {
                    fill_rule.is_in(winding)
                }
            })
            .collect();
        let rings: Vec<Ring> = rings
            .into_iter()
            .zip(left_inside)
            .map(|((points, sides), left_is_inside)| {
                let n = points.len();
                let sign = if left_is_inside { -1.0 } else { 1.0 };
                let normal = |i: usize| left_normal(points[i], points[(i + 1) % n]) * sign;
                let offset = |i: usize| match sides[i] {
                    Side::Faded => -width,
                    Side::Under => width,
                    Side::Hard => 0.0,
                };
                // Each moved vertex: where the two moved edges meet.
                let moved: Vec<Point> = (0..n)
                    .map(|i| {
                        let prev = (i + n - 1) % n;
                        let (n0, n1) = (normal(prev), normal(i));
                        let (d0, d1) = (offset(prev), offset(i));
                        let a0 = points[prev] + n0 * d0;
                        let a1 = points[i] + n1 * d1;
                        let u0 = (points[i] - points[prev]).normalize();
                        let u1 = (points[(i + 1) % n] - points[i]).normalize();
                        let cross = u0.cross(u1);
                        let fallback = points[i] + (n0 * d0 + n1 * d1) * 0.5;
                        if cross.abs() < 1e-3 {
                            return fallback;
                        }
                        let t = (a1 - a0).cross(u1) / cross;
                        let q = a0 + u0 * t;
                        if (q - points[i]).length() > 4.0 * width {
                            fallback
                        } else {
                            q
                        }
                    })
                    .collect();
                let outer: Vec<Point> = vertex_normals(&points, true)
                    .into_iter()
                    .zip(&points)
                    .map(|(v, p)| *p + v * (sign * width))
                    .collect();
                let (original_area, moved_area) = (area(&points), area(&moved));
                let all_thin = original_area * moved_area <= 0.0
                    || moved_area.abs() < 0.05 * original_area.abs();
                // In `area`'s sense the fill is on the left of a positive outline.
                let filled = if left_is_inside { original_area } else { -original_area };
                Ring {
                    points,
                    sides,
                    left_is_inside,
                    moved,
                    outer,
                    filled,
                    all_thin,
                }
            })
            .collect();

        // Parts thinner than a pixel all over (with their holes, so a thin ring stays a ring)
        // keep their outline, drawn see-through so they have as much ink as they cover (MSAA's
        // look), with the outer-only fade from there. Their thickness t is about 2 * area /
        // perimeter; at alpha k, the body and the fades on both sides (half a pixel each, k
        // fading to none) hold k * (t + width) of ink across, so k = t / (t + width). Edges
        // under another fill get no fade (and must stay opaque, the fill above fades over
        // them): only the faded share of the perimeter counts.
        let thin_outers: Vec<usize> =
            (0..rings.len()).filter(|&r| rings[r].all_thin && rings[r].filled > 0.0).collect();
        let thin_part: Vec<bool> = rings
            .iter()
            .map(|ring| {
                ring.all_thin
                    && (ring.filled > 0.0
                        || thin_outers
                            .iter()
                            .any(|&o| in_polygon(&rings[o].points, ring.points[0])))
            })
            .collect();
        let (mut filled, mut perimeter, mut faded) = (0.0f32, 0.0f32, 0.0f32);
        for (ring, _) in rings.iter().zip(&thin_part).filter(|(_, in_thin)| **in_thin) {
            let n = ring.points.len();
            filled += ring.filled;
            for i in 0..n {
                let length = (ring.points[(i + 1) % n] - ring.points[i]).length();
                perimeter += length;
                if ring.sides[i] == Side::Faded {
                    faded += length;
                }
            }
        }
        let thin_alpha = {
            let thickness = filled.max(0.0) / perimeter.max(EPSILON);
            let fade = width * faded / perimeter.max(EPSILON);
            (thickness / (thickness + fade).max(EPSILON)).clamp(0.0, 1.0)
        };

        let mut builder = Path::builder();
        let mut thin = Path::builder();
        let mut fade_points = Vec::new();
        let mut fade_runs = Vec::new();
        for (ring, in_thin) in rings.into_iter().zip(thin_part) {
            let n = ring.points.len();
            let moved = if in_thin { &ring.points } else { &ring.moved };
            let alpha = if in_thin { thin_alpha } else { 1.0 };
            // The fades, from the moved outline (inside) to half a pixel outside: strips over
            // runs of faded segments (each point shared by the segments on both sides).
            let point = |i: usize| (squash.undo(moved[i]), squash.undo(ring.outer[i]));
            match (0..n).find(|&i| ring.sides[i] != Side::Faded) {
                None => {
                    fade_runs.push((fade_points.len(), n, alpha, true));
                    fade_points.extend((0..n).map(point));
                }
                Some(under) => {
                    // Starting after an edge under another fill, each run of faded ones.
                    let mut run: Option<usize> = None;
                    for k in 1..=n {
                        let i = (under + k) % n;
                        if ring.sides[i] == Side::Faded {
                            if run.is_none() {
                                run = Some(fade_points.len());
                                fade_points.push(point(i));
                            }
                            fade_points.push(point((i + 1) % n));
                        } else if let Some(start) = run.take() {
                            fade_runs.push((start, fade_points.len() - start, alpha, false));
                        }
                    }
                    if let Some(start) = run {
                        fade_runs.push((start, fade_points.len() - start, alpha, false));
                    }
                }
            }
            if in_thin {
                thin.begin(squash.undo(moved[0]));
                for q in &moved[1..] {
                    thin.line_to(squash.undo(*q));
                }
                thin.end(true);
            } else {
                // Wound with the fill on the right, which lyon counts as positive (holes then
                // cancel the outline around them; bits turned inside out wind negative and
                // are left out by the `Positive` fill rule).
                let mut ordered = ring.moved;
                if ring.left_is_inside {
                    ordered.reverse();
                }
                builder.begin(squash.undo(ordered[0]));
                for q in &ordered[1..] {
                    builder.line_to(squash.undo(*q));
                }
                builder.end(true);
            }
        }
        let thin = thin.build();
        Some(Centered {
            outline: builder.build(),
            thin: if thin.iter().next().is_some() { vec![(thin, thin_alpha)] } else { Vec::new() },
            fade_points,
            fade_runs,
        })
    }

    /// The fades of a centered fill.
    pub fn push_fades(mesh: &mut VertexBuffers<Vertex, u32>, centered: &Centered, color: swf::Color) {
        if color.a == 0 {
            return;
        }
        let clear = swf::Color { a: 0, ..color };
        for &(start, count, alpha, closed) in &centered.fade_runs {
            let edge = with_alpha(color, alpha);
            let base = mesh.vertices.len() as u32;
            for (inner, outer) in &centered.fade_points[start..start + count] {
                mesh.vertices.push(Vertex { x: inner.x, y: inner.y, color: edge });
                mesh.vertices.push(Vertex { x: outer.x, y: outer.y, color: clear });
            }
            let segments = if closed { count } else { count.saturating_sub(1) };
            for i in 0..segments as u32 {
                let j = (i + 1) % count as u32;
                let (inner_i, outer_i) = (base + 2 * i, base + 2 * i + 1);
                let (inner_j, outer_j) = (base + 2 * j, base + 2 * j + 1);
                mesh.indices
                    .extend_from_slice(&[inner_i, inner_j, outer_j, inner_i, outer_j, outer_i]);
            }
        }
    }

    /// The fade around a fill.
    pub fn fill(
        mesh: &mut VertexBuffers<Vertex, u32>,
        path: &Path,
        fill_rule: lyon::path::FillRule,
        color: swf::Color,
        scale: f32,
        squash: &Squash,
    ) {
        let Some((width, tolerance)) = sizes(scale) else {
            return;
        };
        if color.a == 0 {
            return;
        }
        let start = mesh.vertices.len();
        for (points, _) in squash.polylines(path, tolerance) {
            // Fills are always closed.
            let n = points.len();
            if n < 3 {
                continue;
            }
            // Which side is outside: test just beside the middle of the longest edge.
            let longest = (0..n)
                .max_by(|&i, &j| {
                    let li = (points[(i + 1) % n] - points[i]).square_length();
                    let lj = (points[(j + 1) % n] - points[j]).square_length();
                    li.total_cmp(&lj)
                })
                .unwrap_or(0);
            let (a, b) = (points[longest], points[(longest + 1) % n]);
            let probe = squash.undo(a.lerp(b, 0.5) + left_normal(a, b) * (width * 0.25));
            let left_is_inside =
                lyon::algorithms::hit_test::hit_test_path(&probe, path.iter(), fill_rule, tolerance);
            let outward = if left_is_inside { -width } else { width };
            let offsets: Vec<Vector> =
                vertex_normals(&points, true).into_iter().map(|v| v * outward).collect();
            push_strip(mesh, &points, &offsets, true, color);
        }
        squash.undo_vertices(mesh, start);
    }

    /// How much narrower a stroke's solid body is drawn with Flash-style edges: a pixel (half
    /// on each side), which the fades on both sides cover again (see `stroke`).
    pub fn stroke_body_width(stroke_width: f32, scale: f32) -> f32 {
        match sizes(scale) {
            Some((width, _)) => (stroke_width - 2.0 * width).max(0.0),
            None => stroke_width,
        }
    }

    /// The fades of a stroke, centered on its true edges: from the narrowed body (full alpha,
    /// see `stroke_body_width`) to half a pixel outside the edge (none), along both sides and
    /// around open ends' caps. A 1-pixel line is then a 2-pixel tent: one pixel of ink, like MSAA.
    pub fn stroke(
        mesh: &mut VertexBuffers<Vertex, u32>,
        path: &Path,
        stroke_width: f32,
        caps: (swf::LineCapStyle, swf::LineCapStyle),
        color: swf::Color,
        scale: f32,
        squash: &Squash,
    ) {
        let Some((width, tolerance)) = sizes(scale) else {
            return;
        };
        if color.a == 0 {
            return;
        }
        // Across the line, in the shape. In a squashed shape the stroke (round in the shape)
        // reaches farther here in some directions than others, and so does its narrowed body:
        // the fades follow it, scaled by `widen` in each direction (else they missed the body:
        // a gap down the middle of the line, or an extra dark band).
        let inner = stroke_body_width(stroke_width, scale) / 2.0;
        let outer = stroke_width / 2.0 + width;
        let fade = outer - inner;
        let clear = swf::Color { a: 0, ..color };
        let start = mesh.vertices.len();
        // A quad of fade: `inner` (full alpha) to `outer` (none) at both ends.
        let mut quad = |a_in: Point, b_in: Point, b_out: Point, a_out: Point| {
            let base = mesh.vertices.len() as u32;
            for (p, c) in [(a_in, color), (b_in, color), (b_out, clear), (a_out, clear)] {
                mesh.vertices.push(Vertex { x: p.x, y: p.y, color: c });
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };
        for (points, closed) in squash.polylines(path, tolerance) {
            let n = points.len();
            if n < 2 {
                continue;
            }
            let segments = if closed { n } else { n - 1 };
            let normal = |i: usize| left_normal(points[i], points[(i + 1) % n]);
            // At gentle turns both segments share the miter direction; at sharp ones (over
            // 60 degrees) each keeps its own normal and a round wedge fills the outside of the
            // turn. (One miter for a sharp turn, e.g. a thin line going down and back up, made
            // its fades wrong all along both segments: dashed balloon strings.)
            let joint = |k: usize| -> Option<Vector> {
                if !closed && (k == 0 || k == n - 1) {
                    return None;
                }
                let (n0, n1) = (normal((k + n - 1) % n), normal(k));
                (n0.dot(n1) >= 0.5).then(|| {
                    let sum = n0 + n1;
                    sum * (2.0 / sum.square_length())
                })
            };
            // Where the fade lines meet at a vertex, as the offset from it per unit of width:
            // at a shared (miter) joint, where the two segments' lines (each at its own
            // distance, see `widen`) cross; else on segment `own`'s normal.
            let widen_segment = |i: usize| squash.widen(normal(i));
            let corner = |k: usize, own: usize| -> Vector {
                let Some(miter) = joint(k) else {
                    return normal(own) * widen_segment(own);
                };
                let prev = (k + n - 1) % n;
                let (k0, k1) = (widen_segment(prev), widen_segment(k));
                if (k0 - k1).abs() < 1e-6 {
                    return miter * k0;
                }
                let (n0, n1) = (normal(prev), normal(k));
                let det = n0.cross(n1);
                if det.abs() < 1e-3 {
                    return miter * ((k0 + k1) / 2.0);
                }
                Vector::new((k0 * n1.y - k1 * n0.y) / det, (n0.x * k1 - n1.x * k0) / det)
            };
            for i in 0..segments {
                let j = (i + 1) % n;
                let (va, vb) = (corner(i, i), corner(j, i));
                for side in [1.0f32, -1.0] {
                    let (a, b) = (points[i], points[j]);
                    quad(
                        a + va * (side * inner),
                        b + vb * (side * inner),
                        b + vb * (side * outer),
                        a + va * (side * outer),
                    );
                }
            }
            for k in 0..n {
                if (!closed && (k == 0 || k == n - 1)) || joint(k).is_some() {
                    continue;
                }
                let prev = (k + n - 1) % n;
                let (n0, n1) = (normal(prev), normal(k));
                let forward = (points[k] - points[prev]).normalize();
                for side in [1.0f32, -1.0] {
                    // The outside of the turn: where the next segment heads away from this
                    // side (for a full reversal, one side's wedge covers the tip).
                    if (points[(k + 1) % n] - points[k]).dot(n0 * side) >= 0.0 {
                        continue;
                    }
                    let (from, to) = (n0 * side, n1 * side);
                    let start = from.y.atan2(from.x);
                    let mut delta = to.y.atan2(to.x) - start;
                    while delta > std::f32::consts::PI {
                        delta -= std::f32::consts::TAU;
                    }
                    while delta < -std::f32::consts::PI {
                        delta += std::f32::consts::TAU;
                    }
                    // Nearly reversed: go round the tip (ahead of the line).
                    let mid = start + delta / 2.0;
                    if Vector::new(mid.cos(), mid.sin()).dot(forward) < 0.0 {
                        delta -= std::f32::consts::TAU * delta.signum();
                    }
                    let steps = (delta.abs() / (std::f32::consts::PI / 8.0)).ceil().max(1.0) as usize;
                    let dir = |t: usize| {
                        let angle = start + delta * t as f32 / steps as f32;
                        Vector::new(angle.cos(), angle.sin())
                    };
                    let centre = points[k];
                    for t in 0..steps {
                        let (d0, d1) = (dir(t), dir(t + 1));
                        let (k0, k1) = (squash.widen(d0), squash.widen(d1));
                        quad(
                            centre + d0 * (inner * k0),
                            centre + d1 * (inner * k1),
                            centre + d1 * (outer * k1),
                            centre + d0 * (outer * k0),
                        );
                    }
                }
            }
            if closed {
                continue;
            }
            // Caps: at the start the outside is backwards along the line, at the end forwards.
            for (end, cap) in [(0, caps.0), (n - 1, caps.1)] {
                let tangent = if end == 0 {
                    -(points[1] - points[0]).normalize()
                } else {
                    (points[n - 1] - points[n - 2]).normalize()
                };
                let normal = Vector::new(-tangent.y, tangent.x);
                let centre = points[end];
                let (kn, kt) = (squash.widen(normal), squash.widen(tangent));
                let (cap_points, offsets): (Vec<Point>, Vec<Vector>) = match cap {
                    swf::LineCapStyle::None => (
                        vec![centre - normal * (inner * kn), centre + normal * (inner * kn)],
                        vec![tangent * (width * kt); 2],
                    ),
                    swf::LineCapStyle::Square => {
                        let tip = centre + tangent * (inner * kt);
                        (
                            vec![tip - normal * (inner * kn), tip + normal * (inner * kn)],
                            vec![tangent * (fade * kt); 2],
                        )
                    }
                    swf::LineCapStyle::Round => (0..=8)
                        .map(|k| {
                            let angle = -std::f32::consts::FRAC_PI_2
                                + std::f32::consts::PI * k as f32 / 8.0;
                            let radial = tangent * angle.cos() + normal * angle.sin();
                            let kr = squash.widen(radial);
                            (centre + radial * (inner * kr), radial * (fade * kr))
                        })
                        .unzip(),
                };
                for c in 0..cap_points.len().saturating_sub(1) {
                    let (a, b) = (cap_points[c], cap_points[c + 1]);
                    quad(a, b, b + offsets[c + 1], a + offsets[c]);
                }
            }
        }
        squash.undo_vertices(mesh, start);
    }
}

/// Converts a gradient to the uniforms used by the shader.
fn swf_gradient_to_uniforms(
    gradient_type: GradientType,
    gradient: &swf::Gradient,
    focal_point: swf::Fixed8,
) -> Gradient {
    Gradient {
        records: gradient.records.clone(),
        gradient_type,
        repeat_mode: gradient.spread,
        focal_point,
        interpolation: gradient.interpolation,
    }
}

struct RuffleVertexCtor {
    color: swf::Color,
}

impl FillVertexConstructor<Vertex> for RuffleVertexCtor {
    fn new_vertex(&mut self, vertex: FillVertex) -> Vertex {
        Vertex {
            x: vertex.position().x,
            y: vertex.position().y,
            color: self.color,
        }
    }
}

impl StrokeVertexConstructor<Vertex> for RuffleVertexCtor {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> Vertex {
        Vertex {
            x: vertex.position().x,
            y: vertex.position().y,
            color: self.color,
        }
    }
}
