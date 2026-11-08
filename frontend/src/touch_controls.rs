//! Turu's on-screen controls for a touch screen: a joystick that walks the avatar and buttons for
//! the action bar's six slots (auto attack, skills 1 to 4, potion), drawn over the game. Each one
//! can be moved, resized and hidden (Menu > Edit Touch Controls), and the layout is kept in the
//! data folder's `touch-controls.json`.
//!
//! The joystick walks with `walkTo` (as a click on the ground does): a point ahead of the avatar
//! in the stick's direction, again every [`WALK_EVERY`] while it's held, and where the avatar
//! stands when it's let go. A button uses its slot as the game's key does (`useSkill`:
//! `world.testAction`, which picks a target as the key would).
//!
//! A skill button shows its slot's icon from the game's own action bar (`ui.mcInterface.actBar`),
//! cut from the game's picture each frame ([`set_game_picture`]): the class's icons, with the
//! game's cooldown shading on them.
//!
//! The controls take their own fingers ([`press`], called by the screen before egui and the game
//! see a touch), so the joystick and a button work at the same time: egui follows one finger.
//! While the layout is edited they take none, and egui drags them instead.

use egui::{Align2, Color32, Context, FontId, Id, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How often a held joystick walks on.
const WALK_EVERY: Duration = Duration::from_millis(200);
/// How far ahead of the avatar a walk goes, in stage pixels (the stage is 960 x 550): farther than
/// it gets in [`WALK_EVERY`], so it doesn't stop between two.
const WALK_AHEAD: f64 = 140.0;
/// The stick's part of its travel that does nothing (a resting thumb).
const DEAD_ZONE: f32 = 0.25;
/// The stage's size: walks stay inside it.
const STAGE: (f64, f64) = (960.0, 550.0);
/// The smallest and largest a control can be, in points.
const SIZES: std::ops::RangeInclusive<f32> = 40.0..=220.0;

/// What a control is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "slot")]
pub enum Kind {
    Joystick,
    /// The action bar's slot: 0 auto attack, 1-4 skills, 5 potion (the game's keys 1 to 6).
    Skill(u8),
}

impl Kind {
    fn label(self) -> String {
        match self {
            Kind::Joystick => "Joystick".into(),
            Kind::Skill(0) => "Attack".into(),
            Kind::Skill(5) => "Potion".into(),
            Kind::Skill(slot) => format!("Skill {slot}"),
        }
    }

    /// What the button shows: the game's key for the slot.
    fn key(self) -> String {
        match self {
            Kind::Joystick => String::new(),
            Kind::Skill(slot) => (slot + 1).to_string(),
        }
    }
}

/// One control's place and size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Control {
    pub kind: Kind,
    /// Its centre, as a fraction of the game's area (0 left or top, 1 right or bottom), so it keeps
    /// its place when the area changes (the side bar hidden, another phone).
    pub x: f32,
    pub y: f32,
    /// Across, in points.
    pub size: f32,
    pub visible: bool,
    /// Its own opacity, 0.1 to 1, times the layout's.
    pub opacity: f32,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            kind: Kind::Skill(0),
            x: 0.5,
            y: 0.5,
            size: 64.0,
            visible: true,
            opacity: 1.0,
        }
    }
}

/// `touch-controls.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    /// Shown at all (Menu > Touch Controls).
    pub enabled: bool,
    /// Hidden while the tab's script runs (it plays by itself then).
    pub hide_while_script: bool,
    /// How opaque the controls are, 0.15 to 1.
    pub opacity: f32,
    pub controls: Vec<Control>,
}

impl Default for Layout {
    fn default() -> Self {
        let skill = |slot: u8, x: f32, y: f32, size: f32| Control {
            kind: Kind::Skill(slot),
            x,
            y,
            size,
            visible: true,
            opacity: 1.0,
        };
        Self {
            enabled: false,
            hide_while_script: true,
            opacity: 0.6,
            controls: vec![
                Control {
                    kind: Kind::Joystick,
                    x: 0.13,
                    y: 0.62,
                    size: 150.0,
                    visible: true,
                    opacity: 1.0,
                },
                // Round the attack button at the bottom right, as phone games put them.
                skill(0, 0.9, 0.62, 86.0),
                skill(1, 0.76, 0.66, 58.0),
                skill(2, 0.77, 0.47, 58.0),
                skill(3, 0.84, 0.36, 58.0),
                skill(4, 0.94, 0.36, 58.0),
                skill(5, 0.96, 0.17, 48.0),
            ],
        }
    }
}

impl Layout {
    fn load(path: &std::path::Path) -> Self {
        let mut layout: Self = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        // Controls added after the file was written come in at their default places.
        for control in Self::default().controls {
            if !layout.controls.iter().any(|c| c.kind == control.kind) {
                layout.controls.push(control);
            }
        }
        layout.opacity = layout.opacity.clamp(0.15, 1.0);
        for control in &mut layout.controls {
            control.opacity = control.opacity.clamp(0.1, 1.0);
        }
        layout
    }

    fn save(&self, path: &std::path::Path) {
        let written = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)
            .and_then(|text| std::fs::write(path, text));
        if let Err(e) = written {
            tracing::warn!("Couldn't save {}: {e}", path.display());
        }
    }
}

/// A finger on a control.
#[derive(Clone, Copy, Debug)]
struct Finger {
    id: i32,
    /// Index into `Layout::controls`.
    control: usize,
    /// Where it is, in points.
    at: Pos2,
}

struct State {
    file: Option<PathBuf>,
    layout: Option<Layout>,
    /// The game's area on the last frame, in points (the controls' frame).
    area: Rect,
    /// Drawn on the last frame (enabled, in game, no script running when they hide for one).
    shown: bool,
    editing: bool,
    /// The control being edited (its size and visibility in the editor).
    selected: Option<usize>,
    fingers: Vec<Finger>,
    /// The joystick's direction, unit length scaled by how far it's pushed (0 in the dead zone).
    stick: Vec2,
    last_walk: Option<Instant>,
    /// The stick was pushed since the last walk that stopped the avatar.
    walking: bool,
    /// When the action bar's place was last asked for.
    act_bar_read: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State {
    file: None,
    layout: None,
    area: Rect::NOTHING,
    shown: false,
    editing: false,
    selected: None,
    fingers: Vec::new(),
    stick: Vec2::ZERO,
    last_walk: None,
    walking: false,
    act_bar_read: None,
});

/// The game's picture, for the skill buttons' icons: the texture the game is drawn into, as egui
/// knows it, its size in pixels, and the stage's matrix into it (`a b c d tx ty`, pixels).
#[derive(Clone, Copy, Debug)]
pub struct GamePicture {
    pub texture: egui::TextureId,
    pub size: [f32; 2],
    pub stage_to_pixels: [f32; 6],
}

impl GamePicture {
    /// A stage point (stage pixels) as a UV of the texture.
    fn uv(&self, x: f32, y: f32) -> Pos2 {
        let [a, b, c, d, tx, ty] = self.stage_to_pixels;
        pos2(
            (a * x + c * y + tx) / self.size[0].max(1.0),
            (b * x + d * y + ty) / self.size[1].max(1.0),
        )
    }
}

static PICTURE: Mutex<Option<GamePicture>> = Mutex::new(None);

/// The screen draws the game into `picture` (`None`: it has no picture to share).
pub fn set_game_picture(picture: Option<GamePicture>) {
    *PICTURE.lock().unwrap_or_else(|e| e.into_inner()) = picture;
}

/// The action bar's top left on the stage (`ui.mcInterface` + `actBar`), read now and then.
static ACT_BAR: Mutex<Option<(f32, f32)>> = Mutex::new(None);
/// How often the action bar's place is read again (it doesn't move, but a map or a relogin
/// makes the interface again).
const ACT_BAR_EVERY: Duration = Duration::from_secs(3);
/// The action bar's slot `i` (0 to 5): its icon's square, without the frame, from the action
/// bar's top left, in stage pixels (measured on AQW's interface: 41 x 41 slots, 45.35 apart).
fn slot_square(bar: (f32, f32), slot: u8) -> (Pos2, f32) {
    let side = 36.0;
    let left = bar.0 + 0.75 + f32::from(slot) * 45.35 + (41.0 - side) / 2.0;
    let top = bar.1 - 7.5 + (41.0 - side) / 2.0;
    (pos2(left + side / 2.0, top + side / 2.0), side)
}

/// A walk sent and not answered yet: the next waits.
static WALK_BUSY: AtomicBool = AtomicBool::new(false);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

impl State {
    fn layout(&mut self) -> &mut Layout {
        if self.layout.is_none() {
            let file = crate::turu::bot().paths.root.join("touch-controls.json");
            self.layout = Some(Layout::load(&file));
            self.file = Some(file);
        }
        self.layout.as_mut().expect("loaded above")
    }

    fn save(&mut self) {
        if let (Some(layout), Some(file)) = (&self.layout, &self.file) {
            layout.save(file);
        }
    }

    /// Control `i`'s circle (centre, radius) in points.
    fn circle(&self, control: &Control) -> (Pos2, f32) {
        let centre = pos2(
            self.area.left() + control.x * self.area.width(),
            self.area.top() + control.y * self.area.height(),
        );
        (centre, control.size / 2.0)
    }
}

/// Whether the controls are on (Menu > Touch Controls).
pub fn enabled() -> bool {
    state().layout().enabled
}

pub fn set_enabled(on: bool) {
    let mut state = state();
    state.layout().enabled = on;
    if !on {
        state.editing = false;
    }
    state.save();
}

/// Opens the layout editor (it turns the controls on).
pub fn edit() {
    let mut state = state();
    state.layout().enabled = true;
    state.editing = true;
    state.selected = None;
    state.save();
}

/// A finger came down at `at` (points): the controls take it when it's on one of theirs. The
/// screen then sends its moves and its release here instead of to egui and the game.
pub fn press(id: i32, at: Pos2) -> bool {
    let mut state = state();
    if !state.shown || state.editing {
        return false;
    }
    let hit = {
        let layout = state.layout.as_ref().expect("shown means loaded");
        layout
            .controls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.visible)
            .find(|(_, c)| {
                let (centre, radius) = state.circle(c);
                // A finger is wider than its point: a little leeway round a button.
                centre.distance(at) <= radius + 8.0
            })
            .map(|(i, c)| (i, c.kind))
    };
    let Some((control, kind)) = hit else {
        return false;
    };
    state.fingers.push(Finger { id, control, at });
    match kind {
        Kind::Joystick => update_stick(&mut state),
        Kind::Skill(slot) => use_skill(slot),
    }
    true
}

/// A finger [`press`] took moved.
pub fn moved(id: i32, at: Pos2) {
    let mut state = state();
    if let Some(finger) = state.fingers.iter_mut().find(|f| f.id == id) {
        finger.at = at;
        update_stick(&mut state);
    }
}

/// Whether [`press`] took the finger `id` (its moves and release are the controls').
pub fn holds(id: i32) -> bool {
    state().fingers.iter().any(|f| f.id == id)
}

/// A finger [`press`] took lifted.
pub fn release(id: i32) {
    let mut state = state();
    state.fingers.retain(|f| f.id != id);
    update_stick(&mut state);
}

/// Every finger is gone (the window lost the touches).
pub fn cancel() {
    let mut state = state();
    state.fingers.clear();
    update_stick(&mut state);
}

/// The joystick's direction from the finger on it, if one is.
fn update_stick(state: &mut State) {
    let Some(layout) = state.layout.as_ref() else {
        return;
    };
    let stick = state
        .fingers
        .iter()
        .find_map(|f| {
            let control = layout.controls.get(f.control)?;
            (control.kind == Kind::Joystick).then(|| {
                let (centre, radius) = state.circle(control);
                let offset = (f.at - centre) / radius.max(1.0);
                let length = offset.length();
                if length < DEAD_ZONE {
                    Vec2::ZERO
                } else {
                    offset / length * ((length - DEAD_ZONE) / (1.0 - DEAD_ZONE)).min(1.0)
                }
            })
        })
        .unwrap_or(Vec2::ZERO);
    state.stick = stick;
}

/// The game's area in points: the screen less the side bar and the tab strip.
fn game_area(ctx: &Context) -> Rect {
    let screen = ctx.content_rect();
    Rect::from_min_max(
        pos2(
            screen.left() + crate::gui::left_width() as f32,
            screen.top() + crate::gui::top_height() as f32,
        ),
        screen.max,
    )
}

/// Draws the controls (or the layout editor) and walks while the joystick is held. Every frame,
/// on a touch screen.
pub fn show(ctx: &Context) {
    if !crate::platform::platform().is_touch() {
        return;
    }
    let bot = crate::turu::bot();
    let mut state = state();
    let layout = state.layout().clone();
    state.area = game_area(ctx);
    let in_game = bot.state.lock().in_game;
    let script = bot.scripts.status().is_active();
    let shown = layout.enabled && in_game && !(layout.hide_while_script && script) && !state.editing;
    if !shown && state.shown {
        state.fingers.clear();
        update_stick(&mut state);
    }
    state.shown = shown;
    if state.editing {
        drop(state);
        editor(ctx, layout);
        return;
    }
    if !shown {
        return;
    }
    walk(&bot, &mut state);
    if state.act_bar_read.is_none_or(|at| at.elapsed() >= ACT_BAR_EVERY) {
        state.act_bar_read = Some(Instant::now());
        read_act_bar(&bot);
    }
    if !state.fingers.is_empty() || state.walking {
        ctx.request_repaint_after(Duration::from_millis(50));
    }
    let picture = *PICTURE.lock().unwrap_or_else(|e| e.into_inner());
    let bar = *ACT_BAR.lock().unwrap_or_else(|e| e.into_inner());
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, Id::new("turu_touch_controls")));
    for (i, control) in layout.controls.iter().enumerate() {
        if !control.visible {
            continue;
        }
        let alpha = layout.opacity * control.opacity;
        let (centre, radius) = state.circle(control);
        let held = state.fingers.iter().any(|f| f.control == i);
        match control.kind {
            Kind::Joystick => paint_joystick(&painter, centre, radius, state.stick, alpha),
            Kind::Skill(slot) => {
                let icon = picture.zip(bar).map(|(picture, bar)| (picture, slot_square(bar, slot)));
                paint_button(&painter, centre, radius, &control.kind.key(), icon, held, alpha);
            }
        }
    }
}

/// While the stick is pushed: a walk ahead every [`WALK_EVERY`]; when it's let go, one to where
/// the avatar stands (it would walk on to the last point otherwise).
fn walk(bot: &Arc<turu_bot::Bot>, state: &mut State) {
    let stick = state.stick;
    let due = state.last_walk.is_none_or(|at| at.elapsed() >= WALK_EVERY);
    if stick == Vec2::ZERO {
        if state.walking && !WALK_BUSY.load(Ordering::Acquire) {
            state.walking = false;
            send_walk(bot, None);
        }
        return;
    }
    if due && !WALK_BUSY.load(Ordering::Acquire) {
        state.last_walk = Some(Instant::now());
        state.walking = true;
        send_walk(bot, Some(stick));
    }
}

/// `walkTo` a point ahead in `direction`, or where the avatar is with `None`; off the UI thread.
fn send_walk(bot: &Arc<turu_bot::Bot>, direction: Option<Vec2>) {
    WALK_BUSY.store(true, Ordering::Release);
    crate::turu::spawn_action_for(bot.clone(), "Joystick", move |bot| {
        let result = (|| {
            let x = turu_bot::api::player::x(bot)?;
            let y = turu_bot::api::player::y(bot)?;
            let (tx, ty) = match direction {
                Some(d) => (
                    (x + f64::from(d.x) * WALK_AHEAD).clamp(0.0, STAGE.0),
                    (y + f64::from(d.y) * WALK_AHEAD).clamp(0.0, STAGE.1),
                ),
                None => (x, y),
            };
            let speed = turu_bot::api::player::walk_speed(bot)?;
            bot.flash.call(
                "walkTo",
                vec![(tx.round() as i64).into(), (ty.round() as i64).into(), speed.into()],
            )?;
            Ok(())
        })();
        WALK_BUSY.store(false, Ordering::Release);
        result
    });
}

/// Reads where the action bar is, off the UI thread.
fn read_act_bar(bot: &Arc<turu_bot::Bot>) {
    crate::turu::spawn_action_for(bot.clone(), "Action bar", |bot| {
        let get = |path: &str| bot.flash.get_or::<f64>(path, f64::NAN);
        let place = (|| {
            let x = get("ui.mcInterface.x")? + get("ui.mcInterface.actBar.x")?;
            let y = get("ui.mcInterface.y")? + get("ui.mcInterface.actBar.y")?;
            turu_bot::BotResult::Ok((x.is_finite() && y.is_finite()).then_some((x as f32, y as f32)))
        })();
        *ACT_BAR.lock().unwrap_or_else(|e| e.into_inner()) = place.ok().flatten();
        Ok(())
    });
}

/// Uses action bar slot `slot` on the active tab, as its key does.
fn use_skill(slot: u8) {
    crate::turu::spawn_action("Skill button", move |bot| {
        turu_bot::skills::use_skill(bot, i64::from(slot)).map(|_| ())
    });
}

fn ink(alpha: f32) -> Color32 {
    Color32::from_white_alpha((255.0 * alpha) as u8)
}

fn fill(alpha: f32, pressed: bool) -> Color32 {
    let base = if pressed { 0.55 } else { 0.35 };
    Color32::from_black_alpha((255.0 * base * alpha.max(0.4)) as u8)
}

fn paint_joystick(painter: &egui::Painter, centre: Pos2, radius: f32, stick: Vec2, alpha: f32) {
    painter.circle(centre, radius, fill(alpha, false), Stroke::new(2.0, ink(alpha * 0.8)));
    let knob = centre + stick * (radius * 0.55);
    painter.circle(knob, radius * 0.42, ink(alpha * 0.55), Stroke::new(2.0, ink(alpha)));
}

/// A skill button: its slot's icon from the game's action bar in a circle when there is one (with
/// the game's key on it already), else the key.
fn paint_button(
    painter: &egui::Painter,
    centre: Pos2,
    radius: f32,
    label: &str,
    icon: Option<(GamePicture, (Pos2, f32))>,
    pressed: bool,
    alpha: f32,
) {
    match icon {
        Some((picture, (stage_centre, side))) => {
            painter.circle_filled(centre, radius, fill(alpha, false));
            // A fan over the circle, each point showing the icon's point under it.
            let mut mesh = egui::Mesh::with_texture(picture.texture);
            let shade = if pressed { 0.7 } else { 1.0 };
            let tint = Color32::from_white_alpha((255.0 * alpha) as u8).gamma_multiply(shade);
            let inner = radius - 2.0;
            let half = side / 2.0;
            mesh.colored_vertex(centre, tint);
            mesh.vertices.last_mut().expect("just added").uv = picture.uv(stage_centre.x, stage_centre.y);
            const STEPS: u32 = 40;
            for i in 0..=STEPS {
                let angle = std::f32::consts::TAU * i as f32 / STEPS as f32;
                let (sin, cos) = angle.sin_cos();
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: centre + vec2(cos, sin) * inner,
                    uv: picture.uv(stage_centre.x + cos * half, stage_centre.y + sin * half),
                    color: tint,
                });
                if i > 0 {
                    mesh.add_triangle(0, i, i + 1);
                }
            }
            painter.add(egui::Shape::mesh(mesh));
            painter.circle_stroke(centre, radius, Stroke::new(2.0, ink(alpha * 0.8)));
        }
        None => {
            painter.circle(centre, radius, fill(alpha, pressed), Stroke::new(2.0, ink(alpha * 0.8)));
            painter.text(
                centre,
                Align2::CENTER_CENTER,
                label,
                FontId::proportional((radius * 0.7).clamp(14.0, 40.0)),
                ink(alpha.max(0.5)),
            );
        }
    }
}

/// The layout editor: each control is dragged where it goes; the panel sizes the picked one,
/// hides it, and sets the opacity. Changes are saved as they're made.
fn editor(ctx: &Context, mut layout: Layout) {
    let area = state().area;
    let mut changed = false;
    let mut selected = state().selected;
    for (i, control) in layout.controls.iter_mut().enumerate() {
        let centre = pos2(
            area.left() + control.x * area.width(),
            area.top() + control.y * area.height(),
        );
        let size = Vec2::splat(control.size);
        // Under the editor's panel, which stays above whatever is dragged.
        let response = egui::Area::new(Id::new(("turu_touch_edit", i)))
            .order(egui::Order::Middle)
            .fixed_pos(centre - size / 2.0)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                let picked = selected == Some(i);
                let alpha = if control.visible { 0.9 } else { 0.35 };
                let painter = ui.painter();
                let stroke = Stroke::new(
                    if picked { 3.0 } else { 1.5 },
                    if picked {
                        crate::gui::turu::theme::ACCENT
                    } else {
                        ink(alpha)
                    },
                );
                painter.circle(rect.center(), control.size / 2.0, fill(alpha, picked), stroke);
                let text = match control.kind {
                    Kind::Joystick => "Move".to_owned(),
                    kind => kind.key(),
                };
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(16.0),
                    ink(alpha),
                );
                response
            })
            .inner;
        if response.drag_started() || response.clicked() {
            selected = Some(i);
        }
        if response.dragged() {
            let delta = response.drag_delta();
            control.x = ((centre.x + delta.x - area.left()) / area.width().max(1.0)).clamp(0.0, 1.0);
            control.y = ((centre.y + delta.y - area.top()) / area.height().max(1.0)).clamp(0.0, 1.0);
        }
        if response.drag_stopped() {
            changed = true;
        }
    }
    let mut done = false;
    let mut reset = false;
    egui::Window::new("Touch Controls")
        .id(Id::new("turu_touch_editor"))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_TOP, vec2(0.0, 8.0))
        .show(ctx, |ui| {
            ui.set_width(320.0);
            ui.label("Drag a control to move it; tap one to size or hide it.");
            match selected.and_then(|i| layout.controls.get_mut(i)) {
                Some(control) => {
                    ui.horizontal(|ui| {
                        ui.strong(control.kind.label());
                        changed |= ui.checkbox(&mut control.visible, "Shown").changed();
                    });
                    changed |= ui
                        .add(egui::Slider::new(&mut control.size, SIZES).text("Size").suffix(" pt"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut control.opacity, 0.1..=1.0).text("Its opacity"))
                        .changed();
                }
                None => {
                    ui.weak("No control picked");
                }
            }
            ui.separator();
            changed |= ui
                .add(egui::Slider::new(&mut layout.opacity, 0.15..=1.0).text("All controls' opacity"))
                .changed();
            changed |= ui
                .checkbox(&mut layout.hide_while_script, "Hide while a script runs")
                .changed();
            ui.horizontal(|ui| {
                reset = ui.add(egui::Button::new("Reset").min_size(vec2(96.0, 40.0))).clicked();
                done = ui.add(egui::Button::new("Done").min_size(vec2(96.0, 40.0))).clicked();
            });
        });
    let mut state = state();
    if reset {
        layout = Layout {
            enabled: true,
            ..Layout::default()
        };
        changed = true;
        selected = None;
    }
    state.selected = selected;
    // The drag moves the control as it goes; it's saved when it stops.
    let moved = state.layout.as_ref() != Some(&layout);
    state.layout = Some(layout);
    if changed {
        state.save();
    }
    if done {
        state.editing = false;
        state.selected = None;
        state.save();
    }
    if moved || done {
        ctx.request_repaint();
    }
}
