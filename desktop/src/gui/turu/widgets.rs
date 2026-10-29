//! Small widgets shared by Turu's windows: VibeSkua's button styles, cards, chips, the
//! Mugetsu-style switch, and the log list.

use super::theme::{
    ACCENT, ACCENT_HOVER, ACCENT_PRESSED, CARD, DANGER, DANGER_HOVER, DANGER_PRESSED, EDGE, HOVER, INSET, MUTED,
    RADIUS, SMALL_RADIUS, TEXT, TRACK,
};
use egui::{
    Align, Button, Color32, CornerRadius, FontId, Frame, Label, Layout, Margin, Response, RichText, ScrollArea, Sense,
    Stroke, TextStyle, TextWrapMode, Ui, Vec2, pos2, vec2,
};

/// VibeSkua's button classes: `skua-button`, `skua-primary`, `skua-danger`, `outline`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Normal,
    Primary,
    Danger,
    Outline,
}

/// A button of `size` in one of VibeSkua's styles. `tooltip` shows when it is disabled.
pub fn button(ui: &mut Ui, label: &str, kind: Kind, enabled: bool, size: Vec2, tooltip: &str) -> Response {
    let response = ui
        .scope(|ui| {
            let w = &mut ui.visuals_mut().widgets;
            let (base, hover, pressed, text) = match kind {
                Kind::Normal => (TRACK, HOVER, ACCENT_PRESSED, TEXT),
                Kind::Primary => (ACCENT, ACCENT_HOVER, ACCENT_PRESSED, Color32::WHITE),
                Kind::Danger => (DANGER, DANGER_HOVER, DANGER_PRESSED, Color32::WHITE),
                Kind::Outline => (Color32::TRANSPARENT, HOVER, TRACK, TEXT),
            };
            w.inactive.weak_bg_fill = base;
            w.hovered.weak_bg_fill = hover;
            w.active.weak_bg_fill = pressed;
            w.inactive.fg_stroke.color = text;
            w.hovered.fg_stroke.color = Color32::WHITE;
            if kind == Kind::Outline {
                w.inactive.bg_stroke = Stroke::new(1.0, EDGE);
            }
            ui.add_enabled(enabled, Button::new(label).min_size(size))
        })
        .inner;
    if !enabled && !tooltip.is_empty() {
        response.on_disabled_hover_text(tooltip)
    } else {
        response
    }
}

/// A small text button for a card header ("Copy", "Save", "Clear").
pub fn small_button(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    button(ui, label, Kind::Normal, enabled, vec2(0.0, 22.0), "")
}

/// The tiny uppercase heading of VibeSkua's cards (`section-label`).
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).strong().color(MUTED));
}

/// A rounded card (`card`).
pub fn card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(10))
        .show(ui, add_contents)
        .inner
}

/// A rounded pill with `add_contents` inside.
pub fn pill<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(TRACK)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, add_contents)
        .inner
}

/// A filled dot of `radius`.
pub fn dot(ui: &mut Ui, color: Color32, radius: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(radius * 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), radius, color);
}

/// The grey number next to a heading (`count`).
pub fn count_badge(ui: &mut Ui, count: usize) {
    pill(ui, |ui| {
        ui.label(RichText::new(count.to_string()).size(10.0).color(MUTED));
    });
}

/// A section chip ("All", "Visuals", ...); returns whether it was clicked.
pub fn chip(ui: &mut Ui, label: &str, selected: bool) -> bool {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), TEXT);
    let size = vec2(galley.size().x + 22.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if selected {
        ACCENT
    } else if response.hovered() {
        HOVER
    } else {
        TRACK
    };
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        if selected { Color32::WHITE } else { TEXT },
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// Mugetsu's on/off switch. Returns the response; `changed()` is set when it was toggled.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(34.0, 19.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
    let radius = rect.height() / 2.0;
    let track = lerp(TRACK, ACCENT, how_on);
    ui.painter().rect_filled(rect, CornerRadius::same(radius as u8), track);
    let knob_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
    ui.painter()
        .circle_filled(pos2(knob_x, rect.center().y), radius - 3.0, Color32::WHITE);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// A one-line text box with a hint, in Turu's inset style.
pub fn text_box(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .desired_width(width)
            .margin(Margin::symmetric(8, 5))
            .background_color(INSET),
    )
}

/// A cached, scrollable, selectable list of log lines (one row per text line).
#[derive(Default)]
pub struct LogView {
    version: Option<u64>,
    rows: Vec<String>,
    count: usize,
}

impl LogView {
    /// Re-reads the buffer when it changed since the last call.
    pub fn refresh(&mut self, buffer: &turu_bot::log::LogBuffer) {
        let version = buffer.version();
        if self.version == Some(version) {
            return;
        }
        let lines = buffer.snapshot();
        self.count = lines.len();
        self.rows = lines
            .iter()
            .flat_map(|line| line.formatted().lines().map(str::to_owned).collect::<Vec<_>>())
            .collect();
        self.version = Some(version);
    }

    /// Re-reads the rows from `lines` (already filtered) and records `count`.
    pub fn set_rows(&mut self, rows: Vec<String>, count: usize) {
        self.rows = rows;
        self.count = count;
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// Draws the list into the remaining space, following the end like a console.
    pub fn show(&self, ui: &mut Ui, id: &str, height: f32) {
        Frame::new()
            .fill(INSET)
            .stroke(Stroke::new(1.0, EDGE))
            .corner_radius(CornerRadius::same(SMALL_RADIUS))
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                let row_height = ui.text_style_height(&TextStyle::Monospace) + 2.0;
                ui.set_width(ui.available_width());
                ScrollArea::both()
                    .id_salt(id)
                    .auto_shrink([false, false])
                    .max_height((height - 14.0).max(40.0))
                    .stick_to_bottom(true)
                    .show_rows(ui, row_height, self.rows.len(), |ui, range| {
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for row in &self.rows[range] {
                                ui.add(
                                    Label::new(RichText::new(row).monospace().color(TEXT))
                                        .wrap_mode(TextWrapMode::Extend)
                                        .selectable(true),
                                );
                            }
                        });
                    });
            });
    }
}

/// The colour of the status dot for a script status.
pub fn status_color(status: turu_bot::script::ScriptStatus) -> Color32 {
    use turu_bot::script::ScriptStatus;
    match status {
        ScriptStatus::Empty | ScriptStatus::Loaded => MUTED,
        ScriptStatus::Running => super::theme::SUCCESS,
        ScriptStatus::Paused | ScriptStatus::Compiling | ScriptStatus::Stopping => super::theme::WARNING,
        ScriptStatus::Error => DANGER,
    }
}
