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

/// Whether the screen is touched (`Platform::is_touch`): no hover, finger-sized targets.
pub fn touch() -> bool {
    crate::platform::platform().is_touch()
}

/// `height` for a pointer; on a touch screen at least `touch_height`.
pub fn target(height: f32, touch_height: f32) -> f32 {
    if touch() { height.max(touch_height) } else { height }
}

/// The size of the small labels: 11 points, 12 on a touch screen (a phone held at arm's length).
pub fn small_label_size() -> f32 {
    if touch() { 12.0 } else { 11.0 }
}

/// A small text button for a card header ("Copy", "Save", "Clear").
pub fn small_button(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    button(ui, label, Kind::Normal, enabled, vec2(0.0, 22.0), "")
}

/// The tiny uppercase heading of VibeSkua's cards (`section-label`).
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(small_label_size()).strong().color(MUTED));
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
        ui.label(RichText::new(count.to_string()).size(small_label_size()).color(MUTED));
    });
}

/// A section chip ("All", "Visuals", ...); returns whether it was clicked.
pub fn chip(ui: &mut Ui, label: &str, selected: bool) -> bool {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), TEXT);
    // 40 points high on a touch screen, as the nav bar's chip.
    let size = vec2(galley.size().x + 22.0, target(24.0, 40.0));
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

/// A number box `size` big that keeps `value` in `range`, with `suffix` after it (" ms"). With a
/// pointer it is egui's DragValue (drag it, or click and type); on a touch screen it is
/// [`touch_number`], as a drag there is the finger scrolling the window.
pub fn number<N: egui::emath::Numeric>(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
    speed: f64,
    suffix: &str,
    size: Vec2,
) -> Response {
    if touch() {
        touch_number(ui, id_salt, value, range, suffix, size)
    } else {
        ui.add_sized(
            size,
            egui::DragValue::new(value).range(range).speed(speed).suffix(suffix),
        )
    }
}

/// A number box for a finger: it shows the number, a tap opens it for typing (with the soft
/// keyboard), and a drag that starts on it scrolls what it sits in instead of changing the
/// number, which a DragValue does. What's typed is kept in `range`; leaving the box with nothing
/// typed keeps the last number.
pub fn touch_number<N: egui::emath::Numeric>(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
    suffix: &str,
    size: Vec2,
) -> Response {
    let id = ui.make_persistent_id(("turu_number", id_salt));
    let edit_id = id.with("edit");
    let shown = |v: N| {
        if N::INTEGRAL {
            format!("{}", v.to_f64().round() as i64)
        } else {
            format!("{}", v.to_f64())
        }
    };
    // While typing, the text as typed (it may be empty, or "1" on the way to "100").
    let Some(mut text) = ui.data(|d| d.get_temp::<String>(id)) else {
        let response = ui.add_sized(size, Button::new(format!("{}{suffix}", shown(*value))));
        if response.clicked() {
            ui.data_mut(|d| d.insert_temp(id, shown(*value)));
            ui.memory_mut(|m| m.request_focus(edit_id));
            ui.ctx().request_repaint();
        }
        return response;
    };
    let mut response = ui.add_sized(
        size,
        egui::TextEdit::singleline(&mut text)
            .id(edit_id)
            .horizontal_align(Align::Center)
            .vertical_align(Align::Center)
            .background_color(INSET),
    );
    if response.changed() {
        text.retain(|c| c.is_ascii_digit() || c == '-' || (!N::INTEGRAL && c == '.'));
        if let Ok(typed) = text.parse::<f64>() {
            let (low, high) = (range.start().to_f64(), range.end().to_f64());
            let typed = N::from_f64(typed.clamp(low, high));
            if typed != *value {
                *value = typed;
                response.mark_changed();
            }
        }
    }
    // A number kept in range, or changed from outside (a Reset), shows as it is now.
    if !response.changed()
        && let Ok(typed) = text.parse::<f64>()
        && typed != value.to_f64()
    {
        text = shown(*value);
    }
    if response.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
    } else {
        ui.data_mut(|d| d.insert_temp(id, text));
    }
    response
}

/// A one-line text box with a hint, in Turu's inset style; on a touch screen as tall as the
/// small buttons (40 points).
pub fn text_box(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .desired_width(width)
            .min_size(vec2(0.0, target(0.0, 40.0)))
            .vertical_align(Align::Center)
            .margin(Margin::symmetric(8, 5))
            .background_color(INSET),
    )
}

/// A cached, scrollable, selectable list of log lines (one row per text line). With
/// [`Self::set_wrap`], long rows wrap to the view's width; [`Self::filter_mut`] keeps only the
/// lines that contain a text.
#[derive(Default)]
pub struct LogView {
    version: Option<u64>,
    /// The log's lines, formatted (one may hold several text lines).
    lines: Vec<String>,
    count: usize,
    wrap: bool,
    filter: String,
    /// What the empty list says ([`Self::set_empty_text`]).
    empty_text: &'static str,
    /// The rows as drawn: filtered, split and wrapped, for `shown_for`.
    shown: Vec<String>,
    /// The lines that pass the filter.
    matches: usize,
    /// What `shown` was built for: the columns it wraps at and the filter (`None`: rebuild).
    shown_for: Option<(usize, String)>,
    /// The text [`Self::reveal`] scrolls to on the next draw.
    reveal: Option<String>,
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
        self.lines = lines.iter().map(turu_bot::log::LogLine::formatted).collect();
        self.version = Some(version);
        self.shown_for = None;
    }

    /// Re-reads the rows from `lines` (already filtered) and records `count`.
    pub fn set_rows(&mut self, rows: Vec<String>, count: usize) {
        self.lines = rows;
        self.count = count;
        self.shown_for = None;
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// Wraps long rows to the view's width, continuing under the text after the time.
    pub fn set_wrap(&mut self, wrap: bool) {
        if self.wrap != wrap {
            self.wrap = wrap;
            self.shown_for = None;
        }
    }

    /// What the list says while it has no lines (a filter that matches none says so instead).
    pub fn set_empty_text(&mut self, text: &'static str) {
        self.empty_text = text;
    }

    /// The text the shown lines must contain (any case); empty shows them all.
    pub fn filter_mut(&mut self) -> &mut String {
        &mut self.filter
    }

    /// How many lines pass the filter (as of the last draw).
    pub fn matches(&self) -> usize {
        self.matches
    }

    /// Scrolls to the last line containing `text` on the next draw; whether there is one. The
    /// filter stays when that line passes it, else it is cleared (the Find box empties).
    pub fn reveal(&mut self, text: &str) -> bool {
        let Some(line) = self.lines.iter().rev().find(|line| line.contains(text)) else {
            return false;
        };
        if !passes(line, &self.filter.trim().to_lowercase()) {
            self.filter.clear();
        }
        self.reveal = Some(text.to_owned());
        self.shown_for = None;
        true
    }

    /// Draws the list into the remaining space, following the end like a console.
    pub fn show(&mut self, ui: &mut Ui, id: &str, height: f32) {
        Frame::new()
            .fill(INSET)
            .stroke(Stroke::new(1.0, EDGE))
            .corner_radius(CornerRadius::same(SMALL_RADIUS))
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                let font = TextStyle::Monospace.resolve(ui.style());
                let row_height = ui.text_style_height(&TextStyle::Monospace) + 2.0;
                ui.set_width(ui.available_width());
                let cols = if self.wrap {
                    let char_width = ui.ctx().fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0);
                    let width = ui.available_width() - ui.spacing().scroll.bar_width - 2.0;
                    ((width / char_width).floor() as usize).max(20)
                } else {
                    usize::MAX
                };
                let target = self.rebuild(cols);
                let mut scroll = if self.wrap {
                    ScrollArea::vertical()
                } else {
                    ScrollArea::both()
                }
                .id_salt(id)
                .auto_shrink([false, false])
                .max_height((height - 14.0).max(40.0))
                .stick_to_bottom(true);
                if let Some(row) = target {
                    // `show_rows` spaces rows by the outer item spacing; two rows of context above.
                    let step = row_height + ui.spacing().item_spacing.y;
                    scroll = scroll.vertical_scroll_offset(row.saturating_sub(2) as f32 * step);
                }
                // Empty: say why, where the lines would be.
                let empty = if !self.filter.trim().is_empty() {
                    Some(format!("No lines contain \"{}\"", self.filter.trim()))
                } else {
                    (!self.empty_text.is_empty()).then(|| self.empty_text.to_owned())
                };
                if self.shown.is_empty()
                    && let Some(text) = empty
                {
                    ui.set_min_height((height - 14.0).max(40.0));
                    ui.add(Label::new(RichText::new(text).color(MUTED)).wrap());
                    return;
                }
                scroll.show_rows(ui, row_height, self.shown.len(), |ui, range| {
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for row in &self.shown[range] {
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

    /// Rebuilds `shown` when the lines, the width or the filter changed; the row to reveal.
    fn rebuild(&mut self, cols: usize) -> Option<usize> {
        if self
            .shown_for
            .as_ref()
            .is_some_and(|(c, filter)| *c == cols && *filter == self.filter)
        {
            return None;
        }
        let needle = self.filter.trim().to_lowercase();
        let reveal = self.reveal.take();
        let mut target = None;
        self.shown.clear();
        self.matches = 0;
        for line in &self.lines {
            if !passes(line, &needle) {
                continue;
            }
            self.matches += 1;
            if reveal.as_deref().is_some_and(|text| line.contains(text)) {
                target = Some(self.shown.len());
            }
            for row in line.lines() {
                wrap_row(row, cols, &mut self.shown);
            }
        }
        self.shown_for = Some((cols, self.filter.clone()));
        target
    }
}

/// Whether `line` contains `needle` (already lowercased; empty passes everything).
fn passes(line: &str, needle: &str) -> bool {
    needle.is_empty() || line.to_lowercase().contains(needle)
}

/// Adds `row` to `out` in pieces of at most `cols` characters, breaking after a space near the
/// end where there is one. Pieces after the first start under the text after the time
/// (`[12:00:00] `), so the times stay a clean column.
fn wrap_row(row: &str, cols: usize, out: &mut Vec<String>) {
    let chars: Vec<char> = row.chars().collect();
    if chars.len() <= cols {
        out.push(row.to_owned());
        return;
    }
    let indent = match row.find("] ") {
        Some(i) if row.starts_with('[') && i < 12 => (i + 2).min(cols / 2),
        _ => 0,
    };
    let mut start = 0;
    while start < chars.len() {
        let (prefix, width) = if start == 0 { (0, cols) } else { (indent, cols - indent) };
        let end = if chars.len() - start <= width {
            chars.len()
        } else {
            let hard = start + width;
            (start + width / 2..hard)
                .rev()
                .find(|&i| chars[i] == ' ')
                .map_or(hard, |i| i + 1)
        };
        let piece: String = chars[start..end].iter().collect();
        out.push(format!("{}{}", " ".repeat(prefix), piece.trim_end()));
        start = end;
        // A piece never starts with the space it broke at.
        while chars.get(start) == Some(&' ') {
            start += 1;
        }
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

/// VibeSkua's fallback server list for the server boxes (when neither the server list API
/// nor the game's login screen gave one).
const SERVERS: [&str; 12] = [
    "Artix",
    "Twilly",
    "Galanoth",
    "Gravelyn",
    "Safiria",
    "Sir Ver",
    "Sepulchure",
    "Yorumi",
    "Alteon",
    "Espada",
    "Yokai",
    "Zorbot",
];

/// VibeSkua `ServersList`: the names from the server list API, else from the game's login
/// screen, else [`SERVERS`]; without duplicates (ignoring case), sorted.
pub fn server_names(bot: &turu_bot::Bot) -> Vec<String> {
    let names = |servers: Vec<turu_bot::models::Server>| -> Vec<String> {
        servers
            .into_iter()
            .map(|s| s.name.trim().to_owned())
            .filter(|n| !n.is_empty())
            .collect()
    };
    let mut list = names(bot.servers.cached());
    if list.is_empty() {
        list = names(bot.servers.listed());
    }
    if list.is_empty() {
        list = SERVERS.iter().map(|&n| n.to_owned()).collect();
    }
    list.sort_by_key(|n| n.to_lowercase());
    list.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    list
}

/// A server drop-down (Options > Game's Relogin Server, Accounts' launch server): VibeSkua's
/// server list ([`server_names`], refreshed in the background while it is open), plus
/// `current` when it isn't in it. Returns the name picked.
pub fn server_combo(ui: &mut Ui, id: &str, bot: &std::sync::Arc<turu_bot::Bot>, current: &str) -> Option<String> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .width(150.0)
        .show_ui(ui, |ui| {
            turu_bot::api::servers::refresh_in_background(bot);
            if bot.servers.is_refreshing() {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
            }
            let mut names = server_names(bot);
            if !current.is_empty() && !names.iter().any(|n| n.eq_ignore_ascii_case(current)) {
                names.insert(0, current.to_owned());
            }
            for name in names {
                if ui.selectable_label(current == name, &name).clicked() {
                    picked = Some(name);
                }
            }
        });
    picked
}

/// A list well `height` tall and as wide as `ui` (inset colour, rounded edge).
pub fn list_well(ui: &mut Ui, height: f32, add_contents: impl FnOnce(&mut Ui)) {
    ui.allocate_ui(vec2(ui.available_width(), height), |ui| {
        Frame::new()
            .fill(INSET)
            .stroke(Stroke::new(1.0, EDGE))
            .corner_radius(CornerRadius::same(SMALL_RADIUS))
            .inner_margin(Margin::same(4))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                add_contents(ui);
            });
    });
}

/// A clickable list row `height` tall: hover fill, and the track fill with an accent bar when
/// selected. `add_contents` lays out left to right, centred.
pub fn list_row(ui: &mut Ui, selected: bool, height: f32, add_contents: impl FnOnce(&mut Ui)) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let fill = if selected {
        TRACK
    } else if response.hovered() {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    if selected {
        let bar = egui::Rect::from_min_size(rect.min + vec2(0.0, 4.0), vec2(3.0, (height - 8.0).max(2.0)));
        ui.painter().rect_filled(bar, CornerRadius::same(2), ACCENT);
    }
    let builder = egui::UiBuilder::new()
        .max_rect(rect.shrink2(vec2(8.0, 0.0)))
        .layout(Layout::left_to_right(Align::Center));
    add_contents(&mut ui.new_child(builder));
    response
}

/// The picked rows of a list (VibeSkua's `ListBox` selection), as indices into the full list:
/// a click toggles a row in multiple mode or picks only it, Shift+click picks the shown rows
/// from the last clicked one.
#[derive(Default)]
pub struct Selection {
    picked: std::collections::BTreeSet<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.picked.clear();
        self.anchor = None;
    }

    pub fn contains(&self, index: usize) -> bool {
        self.picked.contains(&index)
    }

    pub fn len(&self) -> usize {
        self.picked.len()
    }

    pub fn is_empty(&self) -> bool {
        self.picked.is_empty()
    }

    /// The picked indices, in list order.
    pub fn picked(&self) -> Vec<usize> {
        self.picked.iter().copied().collect()
    }

    /// The row clicked last, when it is still picked (VibeSkua's `SelectedItem`).
    pub fn current(&self) -> Option<usize> {
        self.anchor.filter(|a| self.picked.contains(a))
    }

    /// Keeps only one row (Multi-select turned off).
    pub fn keep_one(&mut self) {
        if self.picked.len() > 1 {
            let keep = self.current().or_else(|| self.picked.first().copied());
            self.picked.retain(|i| Some(*i) == keep);
        }
    }

    /// A click on `index`; `shown` are the indices the list shows, in order.
    pub fn click(&mut self, index: usize, shown: &[usize], multiple: bool, shift: bool) {
        let range = (multiple && shift)
            .then(|| {
                let from = shown.iter().position(|i| Some(*i) == self.anchor)?;
                let to = shown.iter().position(|i| *i == index)?;
                Some(shown[from.min(to)..=from.max(to)].to_vec())
            })
            .flatten();
        match range {
            Some(rows) => self.picked.extend(rows),
            None if multiple => {
                if !self.picked.remove(&index) {
                    self.picked.insert(index);
                }
            }
            None => {
                self.picked.clear();
                self.picked.insert(index);
            }
        }
        if !shift || self.anchor.is_none() {
            self.anchor = Some(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LogView, touch_number, wrap_row};
    use egui::{Event, PointerButton, Pos2, pos2, vec2};

    /// Runs frames of a scrolling list of 20 number boxes 40 points high (`touch_number`), the
    /// sixth holding `value` (at about 215 to 255 before scrolling), with `events` in the frames
    /// they're given for; returns where that box was drawn last.
    fn number_list(value: &mut i32, frames: &[Vec<Event>]) -> f32 {
        let ctx = egui::Context::default();
        let mut top = 0.0;
        for events in frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                events: events.clone(),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                egui::ScrollArea::vertical()
                    .scroll_source(egui::scroll_area::ScrollSource::ALL)
                    .show(ui, |ui| {
                        let mut other = 0;
                        for n in 0..20 {
                            if n == 5 {
                                top = touch_number(ui, n, value, 0..=1000, " ms", vec2(120.0, 40.0))
                                    .rect
                                    .top();
                            } else {
                                touch_number(ui, n, &mut other, 0..=1000, "", vec2(120.0, 40.0));
                            }
                        }
                    });
            });
            output.textures_delta.clear();
        }
        top
    }

    fn press(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    /// A finger dragged from a touch number box scrolls the list and leaves the number alone.
    #[test]
    fn a_drag_on_a_touch_number_scrolls() {
        let mut value = 700;
        let (x, y) = (60.0, 235.0);
        // Two frames first: the list knows it's too long to fit only after one, and a press finds
        // what to drag among the last frame's widgets.
        let mut frames = vec![
            vec![],
            vec![],
            vec![Event::PointerMoved(pos2(x, y)), press(pos2(x, y), true)],
        ];
        for step in 1..=10 {
            frames.push(vec![Event::PointerMoved(pos2(x, y - step as f32 * 15.0))]);
        }
        frames.push(vec![press(pos2(x, y - 150.0), false), Event::PointerGone]);
        let top = number_list(&mut value, &frames);
        assert_eq!(value, 700);
        assert!(top < 150.0, "the list didn't scroll: the box is at {top}");
    }

    /// A tap opens a touch number box for typing; what's typed is kept in its range.
    #[test]
    fn a_tap_on_a_touch_number_types() {
        let mut value = 700;
        let at = pos2(60.0, 235.0);
        let frames = vec![
            vec![],
            vec![],
            vec![Event::PointerMoved(at), press(at, true)],
            vec![press(at, false)],
            vec![],
            vec![Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            vec![Event::Text("12".into())],
            vec![Event::Text("x5000".into())],
            vec![],
        ];
        number_list(&mut value, &frames);
        assert_eq!(value, 1000, "12 then 5000 typed, kept under 1000");
    }

    /// While typing, a number past the range shows as the number kept.
    #[test]
    fn a_touch_number_shows_what_it_keeps() {
        let ctx = egui::Context::default();
        let mut value = 700;
        let at = pos2(60.0, 20.0);
        let frames = [
            vec![],
            vec![Event::PointerMoved(at), press(at, true)],
            vec![press(at, false)],
            vec![],
            vec![Event::Text("9".into())],
            vec![],
            vec![],
        ];
        let mut shown = String::new();
        for events in frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                touch_number(ui, "n", &mut value, 0..=1000, "", vec2(120.0, 40.0));
            });
            output.textures_delta.clear();
            shown = output
                .shapes
                .iter()
                .find_map(|clipped| match &clipped.shape {
                    egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                    _ => None,
                })
                .unwrap_or_default();
        }
        assert_eq!(value, 1000);
        assert_eq!(shown, "1000");
    }

    fn view(lines: &[&str]) -> LogView {
        let mut view = LogView::default();
        view.set_rows(lines.iter().map(|l| (*l).to_owned()).collect(), lines.len());
        view
    }

    /// Revealing a line keeps the user's filter when the line passes it, else clears it.
    #[test]
    fn reveal_keeps_a_filter_the_line_passes() {
        let mut log = view(&["[1] Hunting", "[2] Error: no target", "[3] Hunting"]);
        *log.filter_mut() = "error".into();
        assert!(log.reveal("no target"));
        assert_eq!(log.filter_mut(), "error");
        *log.filter_mut() = "hunting".into();
        assert!(log.reveal("no target"));
        assert_eq!(log.filter_mut(), "");
        assert!(!log.reveal("missing"));
    }

    /// A filter of spaces filters nothing; spaces around a word are ignored.
    #[test]
    fn filters_ignore_surrounding_spaces() {
        let mut log = view(&["[1] Hunting", "[2] Error"]);
        *log.filter_mut() = "   ".into();
        log.rebuild(80);
        assert_eq!(log.matches(), 2);
        *log.filter_mut() = " error ".into();
        log.rebuild(80);
        assert_eq!(log.matches(), 1);
    }

    fn wrapped(row: &str, cols: usize) -> Vec<String> {
        let mut out = Vec::new();
        wrap_row(row, cols, &mut out);
        out
    }

    #[test]
    fn short_rows_stay_whole() {
        assert_eq!(wrapped("[12:00:00] Hunting", 40), ["[12:00:00] Hunting"]);
    }

    /// Long rows break after a word and continue under the text, not under the time.
    #[test]
    fn long_rows_wrap_under_the_text() {
        assert_eq!(
            wrapped("[12:00:00] Killing Escherion for the Escherion's Helm", 30),
            [
                "[12:00:00] Killing Escherion",
                "           for the Escherion's",
                "           Helm"
            ]
        );
    }

    /// A word longer than the row breaks where the row ends.
    #[test]
    fn long_words_break_hard() {
        assert_eq!(wrapped("abcdefghij", 4), ["abcd", "efgh", "ij"]);
    }
}
