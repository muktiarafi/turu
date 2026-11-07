//! Diagnostics > Notification Center (VibeSkua `NotificationCenterView`): the bot's feed of game
//! and script events (`turu_bot::notifications`), newest first, with filter chips and their
//! counts, an icon per kind, and the Logs window's Copy / Deselect / Save / Clear. The bot
//! collects the feed from its start, so the window shows the whole session whenever it opens.

use super::files::SaveText;
use super::theme::{ACCENT, ACCENT_PRESSED, CARD, DANGER, EDGE, HOVER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{
    Align, Button, Color32, Context, CornerRadius, Frame, Label, Layout, Margin, Painter, Pos2, Rect, RichText,
    ScrollArea, Sense, Shape, Stroke, Ui, pos2, vec2,
};
use std::collections::HashSet;
use turu_bot::notifications::{Notification, NotificationKind, thousands, to_text};

pub const TITLE: &str = "Notification Center";

/// VibeSkua `Filters`: each chip's kind (`None`: all) and label.
const FILTERS: [(Option<NotificationKind>, &str); 6] = [
    (None, "All"),
    (Some(NotificationKind::Item), "Items"),
    (Some(NotificationKind::Activity), "Activity"),
    (Some(NotificationKind::Warning), "Warnings"),
    (Some(NotificationKind::Error), "Errors"),
    (Some(NotificationKind::System), "Maps"),
];

/// Below this width the footer's count goes on a line of its own (a phone held upright).
const NARROW: f32 = 420.0;

#[derive(Default)]
pub struct NotificationCenterWindow {
    /// VibeSkua `SelectedFilter` (`None`: All).
    filter: Option<NotificationKind>,
    /// The feed as last read (newest first) and its revision.
    entries: Vec<Notification>,
    revision: Option<u64>,
    /// The selected entries' ids (clicking a row toggles it, as VibeSkua's multiple selection).
    selected: HashSet<u64>,
    /// "Copied N events", in place of the count until the list or the selection changes.
    status: Option<String>,
    save: SaveText,
}

impl NotificationCenterWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        self.save.poll();
        if !*open {
            return;
        }
        super::window(ctx, TITLE, [560.0, 480.0], 5)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        // New events arrive from other threads.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    /// Reads the feed again when it changed.
    fn refresh(&mut self) {
        let bot = crate::turu::bot();
        let revision = bot.notifications.revision();
        if self.revision == Some(revision) {
            return;
        }
        self.revision = Some(revision);
        self.entries = bot.notifications.entries();
        let ids: HashSet<u64> = self.entries.iter().map(|e| e.id).collect();
        self.selected.retain(|id| ids.contains(id));
        self.status = None;
    }

    fn matches(&self, entry: &Notification) -> bool {
        self.filter.is_none_or(|kind| entry.kind == kind)
    }

    fn ui(&mut self, ui: &mut Ui) {
        self.refresh();
        let spacing = 10.0;

        // Filter chips with counts; they wrap on a narrow window.
        ui.horizontal_wrapped(|ui| {
            for (kind, title) in FILTERS {
                let count = match kind {
                    None => self.entries.len(),
                    Some(kind) => self.entries.iter().filter(|e| e.kind == kind).count(),
                };
                if widgets::chip(ui, &format!("{title}  {count}"), self.filter == kind) && self.filter != kind {
                    self.filter = kind;
                    self.selected.clear();
                    self.status = None;
                }
            }
        });
        ui.add_space(spacing);

        let narrow = ui.available_width() < NARROW;
        let button_height = widgets::target(22.0, 40.0);
        let footer = if narrow { button_height + 20.0 } else { button_height } + spacing;
        // The gaps egui adds after the feed and the spacer, and the feed's border.
        let gaps = 2.0 * ui.spacing().item_spacing.y + 2.0;
        let feed_height = (ui.available_height() - footer - gaps).max(widgets::target(80.0, 120.0));
        self.feed(ui, feed_height);
        ui.add_space(spacing);
        self.footer(ui, narrow, button_height);
    }

    /// The feed: one row per event (icon, time, message), or the empty notice.
    fn feed(&mut self, ui: &mut Ui, height: f32) {
        let shown: Vec<&Notification> = self.entries.iter().filter(|e| self.matches(e)).collect();
        let mut toggled = None;
        let response = Frame::new()
            .fill(CARD)
            .stroke(Stroke::new(1.0, EDGE))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(4))
            .show(ui, |ui| {
                let inner = height - 8.0;
                ui.set_min_size(vec2(ui.available_width(), inner));
                ui.set_max_height(inner);
                if shown.is_empty() {
                    empty_notice(ui, self.filter, inner);
                    return;
                }
                ScrollArea::vertical()
                    .id_salt("turu_notifications_feed")
                    .auto_shrink([false, false])
                    .max_height(inner)
                    .show(ui, |ui| {
                        for entry in &shown {
                            if row(ui, entry, self.selected.contains(&entry.id)) {
                                toggled = Some(entry.id);
                            }
                        }
                    });
            })
            .response;
        if let Some(id) = toggled {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
            self.status = None;
        }
        // Ctrl+C over the feed copies the selected events, as in the Logs window.
        let copy = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        if copy && !self.selected.is_empty() && response.contains_pointer() {
            self.copy(ui.ctx());
        }
    }

    /// The selected events, or every shown event when none is selected (VibeSkua `CopyAsync`).
    fn copy(&mut self, ctx: &Context) {
        let selected: Vec<&Notification> = self
            .entries
            .iter()
            .filter(|e| self.matches(e) && self.selected.contains(&e.id))
            .collect();
        let entries = if selected.is_empty() {
            self.entries.iter().filter(|e| self.matches(e)).collect()
        } else {
            selected
        };
        if entries.is_empty() {
            return;
        }
        ctx.copy_text(to_text(entries.iter().copied()));
        self.status = Some(if entries.len() == 1 {
            "Copied 1 event".to_owned()
        } else {
            format!("Copied {} events", thousands(entries.len() as i64))
        });
    }

    /// The count and selection on the left, Copy, Deselect, Save (Share on Android) and Clear on
    /// the right; on a narrow window the count goes above the buttons.
    fn footer(&mut self, ui: &mut Ui, narrow: bool, height: f32) {
        let shown = self.entries.iter().filter(|e| self.matches(e)).count();
        let selected = self
            .entries
            .iter()
            .filter(|e| self.matches(e) && self.selected.contains(&e.id))
            .count();
        let text = self.status.clone().unwrap_or_else(|| {
            let events = if shown == 1 {
                "1 event".to_owned()
            } else {
                format!("{} events", thousands(shown as i64))
            };
            if selected > 0 {
                format!("{events} · {selected} selected")
            } else {
                events
            }
        });
        let count = |ui: &mut Ui| {
            ui.add(
                Label::new(RichText::new(&text).size(widgets::small_label_size()).color(MUTED))
                    .truncate()
                    .selectable(false),
            );
        };
        if narrow {
            count(ui);
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            if !narrow {
                count(ui);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // A fixed width (VibeSkua's `w84`) where there's room.
                let size = vec2(if narrow { 0.0 } else { 84.0 }, height);
                let platform = crate::platform::platform();
                if danger_soft_button(ui, "Clear", size)
                    .on_hover_text("Clear every event")
                    .clicked()
                {
                    crate::turu::bot().notifications.clear();
                    self.selected.clear();
                    self.refresh();
                }
                if platform.has_file_dialogs()
                    && widgets::button(ui, "Save", Kind::Normal, true, size, "")
                        .on_hover_text("Save the shown events to a file")
                        .clicked()
                    && shown > 0
                {
                    let text = to_text(self.entries.iter().filter(|e| self.matches(e)));
                    self.save.ask(ui.ctx(), "notifications.txt", text);
                }
                if platform.shares_text()
                    && widgets::button(ui, "Share", Kind::Normal, true, size, "")
                        .on_hover_text("Share the shown events")
                        .clicked()
                    && shown > 0
                {
                    let text = to_text(self.entries.iter().filter(|e| self.matches(e)));
                    platform.share_text("Turu notifications", &text);
                }
                if widgets::button(ui, "Deselect", Kind::Normal, true, size, "")
                    .on_hover_text("Clear the selected events")
                    .clicked()
                {
                    self.selected.clear();
                    self.status = None;
                }
                if widgets::button(ui, "Copy", Kind::Normal, true, size, "")
                    .on_hover_text("Copy the selected events (or all shown)")
                    .clicked()
                {
                    self.copy(ui.ctx());
                }
            });
        });
    }
}

/// VibeSkua's `skua-danger-soft` button: danger text on a faint danger fill.
fn danger_soft_button(ui: &mut Ui, label: &str, size: egui::Vec2) -> egui::Response {
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        w.inactive.weak_bg_fill = DANGER.gamma_multiply(0.16);
        w.hovered.weak_bg_fill = DANGER.gamma_multiply(0.28);
        w.active.weak_bg_fill = DANGER.gamma_multiply(0.4);
        ui.add(Button::new(RichText::new(label).color(DANGER)).min_size(size))
    })
    .inner
}

/// One event: its kind's icon (tooltip: the kind), the time and the message, wrapping. A click
/// toggles its selection; returns whether it was clicked.
fn row(ui: &mut Ui, entry: &Notification, selected: bool) -> bool {
    let background = ui.painter().add(Shape::Noop);
    let min_height = widgets::target(20.0, 40.0);
    let inner = ui
        .horizontal_top(|ui| {
            ui.set_min_height(min_height);
            ui.add_space(4.0);
            let (icon, response) = ui.allocate_exact_size(vec2(15.0, 17.0), Sense::hover());
            kind_icon(
                ui.painter(),
                entry.kind,
                Rect::from_min_size(icon.min + vec2(0.0, 2.0), vec2(15.0, 15.0)),
                kind_color(entry.kind),
            );
            response.on_hover_text(entry.kind.name());
            ui.add_space(4.0);
            ui.add(
                Label::new(RichText::new(entry.time_display()).monospace().size(11.0).color(MUTED)).selectable(false),
            );
            ui.add_space(4.0);
            ui.add(
                Label::new(RichText::new(&entry.message).size(12.0).color(TEXT))
                    .wrap()
                    .selectable(false),
            );
        })
        .response;
    let rect =
        Rect::from_min_max(inner.rect.min, pos2(ui.max_rect().right(), inner.rect.max.y)).expand2(vec2(0.0, 2.0));
    let response = ui.interact(rect, ui.id().with(("turu_notification", entry.id)), Sense::click());
    let fill = if selected {
        ACCENT_PRESSED.gamma_multiply(0.45)
    } else if response.hovered() && !widgets::touch() {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .set(background, Shape::rect_filled(rect, CornerRadius::same(6), fill));
    ui.add_space(2.0);
    response.clicked()
}

/// VibeSkua's kind colours: items green, warnings amber, errors red, maps accent, activity muted.
fn kind_color(kind: NotificationKind) -> Color32 {
    match kind {
        NotificationKind::Item => SUCCESS,
        NotificationKind::Warning => WARNING,
        NotificationKind::Error => DANGER,
        NotificationKind::System => ACCENT,
        NotificationKind::Activity => MUTED,
    }
}

/// VibeSkua `EmptyText` under a bell.
fn empty_notice(ui: &mut Ui, filter: Option<NotificationKind>, height: f32) {
    let text = match filter {
        None => "No events yet. Drops, deaths, relogins, quests and script starts and stops show up here.".to_owned(),
        Some(kind) => format!(
            "No {} events yet.",
            match kind {
                NotificationKind::Item => "item",
                NotificationKind::Warning => "warning",
                NotificationKind::Error => "error",
                NotificationKind::System => "map",
                NotificationKind::Activity => "activity",
            }
        ),
    };
    let width = ui.available_width().min(300.0);
    ui.vertical_centered(|ui| {
        ui.add_space(((height - 80.0) / 2.0).max(8.0));
        let (rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
        bell(ui.painter(), rect.center(), 28.0, MUTED);
        ui.add_space(8.0);
        ui.set_max_width(width);
        ui.add(
            Label::new(RichText::new(text).size(12.0).color(MUTED))
                .wrap()
                .halign(Align::Center)
                .selectable(false),
        );
    });
}

/// The kind's icon in `rect` (15 points), in VibeSkua's Material outlines: a gift (items), a
/// triangle with "!" (warnings), a circle with "!" (errors), a map pin (maps), a play button
/// (activity).
fn kind_icon(painter: &Painter, kind: NotificationKind, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.3, color);
    let c = rect.center();
    let (x0, y0, x1, y1) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    match kind {
        NotificationKind::Item => {
            let lid = Rect::from_min_max(pos2(x0 + 1.0, y0 + 4.0), pos2(x1 - 1.0, y0 + 7.0));
            let body = Rect::from_min_max(pos2(x0 + 2.0, y0 + 7.0), pos2(x1 - 2.0, y1 - 1.0));
            painter.rect_stroke(lid, 1.0, stroke, egui::StrokeKind::Middle);
            painter.rect_stroke(body, 1.0, stroke, egui::StrokeKind::Middle);
            painter.line_segment([pos2(c.x, y0 + 4.0), pos2(c.x, y1 - 1.0)], stroke);
            painter.circle_stroke(pos2(c.x - 2.0, y0 + 2.3), 1.7, stroke);
            painter.circle_stroke(pos2(c.x + 2.0, y0 + 2.3), 1.7, stroke);
        }
        NotificationKind::Warning => {
            let points = vec![pos2(c.x, y0 + 1.0), pos2(x1 - 0.5, y1 - 1.5), pos2(x0 + 0.5, y1 - 1.5)];
            painter.add(Shape::closed_line(points, stroke));
            painter.line_segment([pos2(c.x, y0 + 5.5), pos2(c.x, y0 + 9.5)], stroke);
            painter.circle_filled(pos2(c.x, y0 + 11.6), 0.9, color);
        }
        NotificationKind::Error => {
            painter.circle_stroke(c, 6.5, stroke);
            painter.line_segment([pos2(c.x, c.y - 3.5), pos2(c.x, c.y + 0.8)], stroke);
            painter.circle_filled(pos2(c.x, c.y + 3.2), 0.9, color);
        }
        NotificationKind::System => {
            // A pin: an arc over the top, closing at the tip.
            let head = pos2(c.x, y0 + 5.8);
            let r = 4.6;
            let mut points: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let a = (145.0 + 250.0 * i as f32 / 20.0).to_radians();
                    head + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            points.push(pos2(c.x, y1 - 0.5));
            painter.add(Shape::closed_line(points, stroke));
            painter.circle_stroke(head, 1.6, stroke);
        }
        NotificationKind::Activity => {
            painter.circle_stroke(c, 6.5, stroke);
            let points = vec![
                pos2(c.x - 2.0, c.y - 3.2),
                pos2(c.x + 3.4, c.y),
                pos2(c.x - 2.0, c.y + 3.2),
            ];
            painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
        }
    }
}

/// VibeSkua's `BellOutline`, `size` points across, centred on `c`.
fn bell(painter: &Painter, c: Pos2, size: f32, color: Color32) {
    let s = size;
    let stroke = Stroke::new(1.8, color);
    let base = c.y + 0.22 * s;
    let dome = pos2(c.x, c.y - 0.08 * s);
    let r = 0.27 * s;
    let mut points = vec![pos2(c.x - 0.36 * s, base), pos2(c.x - r, dome.y + 0.12 * s)];
    points.extend((0..=16).map(|i| {
        let a = (180.0 + 180.0 * i as f32 / 16.0).to_radians();
        dome + vec2(a.cos(), a.sin()) * r
    }));
    points.extend([pos2(c.x + r, dome.y + 0.12 * s), pos2(c.x + 0.36 * s, base)]);
    painter.add(Shape::line(points, stroke));
    painter.line_segment([pos2(c.x - 0.42 * s, base), pos2(c.x + 0.42 * s, base)], stroke);
    painter.line_segment([pos2(c.x, dome.y - r), pos2(c.x, dome.y - r - 0.06 * s)], stroke);
    painter.circle_stroke(pos2(c.x, base + 0.09 * s), 0.07 * s, stroke);
}
