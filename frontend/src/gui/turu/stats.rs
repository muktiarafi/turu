//! Combat > Stats (VibeSkua `ScriptStatsView`): the script clock, the counters and, with
//! Advanced on, the live gameplay rates (`turu_bot::stats::Gameplay`, sampled by the bot's
//! background thread) and the rate history every client shares (`StatsRateHistory.json`).

use super::files::SaveText;
use super::theme::{CARD, DANGER, EDGE, INSET, MUTED, SUCCESS, TEXT, TRACK, WARNING};
use super::widgets::{self, Kind};
use crate::models::scheduler::Tint;
use crate::models::stats::{self as model, CounterKind};
use egui::{Align, Color32, Context, CornerRadius, Layout, Modal, RichText, ScrollArea, Sense, Stroke, Ui, vec2};
use std::cmp::Ordering;
use std::time::Duration;
use turu_bot::stats::{self, Gameplay, RateSnapshot, RateUnit};

/// VibeSkua's window: 480 wide; 760 high with Advanced on, so the history fills it.
const WIDTH: f32 = 480.0;
const ADVANCED_HEIGHT: f32 = 760.0;
/// Below this the rates take one column and the history's buttons a row of their own.
const NARROW: f32 = 400.0;

const BLUE: Color32 = Color32::from_rgb(0x60, 0xA5, 0xFA);
const PURPLE: Color32 = Color32::from_rgb(0xA7, 0x8B, 0xFA);

/// The history table's columns (VibeSkua's DataGrid, sortable by clicking a header).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Saved,
    Character,
    Session,
    Unit,
    Exp,
    Gold,
    ClassPoints,
    Rank,
    Kills,
    Deaths,
    Items,
}

impl Column {
    const ALL: [Column; 11] = [
        Column::Saved,
        Column::Character,
        Column::Session,
        Column::Unit,
        Column::Exp,
        Column::Gold,
        Column::ClassPoints,
        Column::Rank,
        Column::Kills,
        Column::Deaths,
        Column::Items,
    ];

    fn header(self) -> &'static str {
        match self {
            Column::Saved => "SAVED",
            Column::Character => "CHARACTER",
            Column::Session => "SESSION",
            Column::Unit => "UNIT",
            Column::Exp => "EXP",
            Column::Gold => "GOLD",
            Column::ClassPoints => "CP",
            Column::Rank => "RANK",
            Column::Kills => "KILLS",
            Column::Deaths => "DEATHS",
            Column::Items => "ITEMS",
        }
    }

    /// Numbers are right-aligned and monospaced; SAVED and UNIT are dimmed.
    fn numeric(self) -> bool {
        !matches!(self, Column::Saved | Column::Character | Column::Unit)
    }

    fn text(self, s: &RateSnapshot) -> String {
        match self {
            Column::Saved => s.saved_at_display(),
            Column::Character => s.character_display().to_owned(),
            Column::Session => s.session_display(),
            Column::Unit => s.rate_unit.clone(),
            Column::Exp => stats::format_number(s.experience_rate, 0),
            Column::Gold => stats::format_number(s.gold_rate, 0),
            Column::ClassPoints => stats::format_number(s.class_points_rate, 0),
            Column::Rank => s.class_rank.to_string(),
            Column::Kills => stats::format_number(s.kills_rate, 2),
            Column::Deaths => stats::format_number(s.deaths_rate, 2),
            Column::Items => stats::format_number(s.items_rate, 1),
        }
    }

    fn compare(self, a: &RateSnapshot, b: &RateSnapshot) -> Ordering {
        let num = |x: f64, y: f64| x.partial_cmp(&y).unwrap_or(Ordering::Equal);
        match self {
            Column::Saved => a.saved_at.cmp(&b.saved_at),
            Column::Character => a
                .character_display()
                .to_lowercase()
                .cmp(&b.character_display().to_lowercase()),
            Column::Session => num(a.session_seconds, b.session_seconds),
            Column::Unit => a.rate_unit.cmp(&b.rate_unit),
            Column::Exp => num(a.experience_rate, b.experience_rate),
            Column::Gold => num(a.gold_rate, b.gold_rate),
            Column::ClassPoints => num(a.class_points_rate, b.class_points_rate),
            Column::Rank => a.class_rank.cmp(&b.class_rank),
            Column::Kills => num(a.kills_rate, b.kills_rate),
            Column::Deaths => num(a.deaths_rate, b.deaths_rate),
            Column::Items => num(a.items_rate, b.items_rate),
        }
    }
}

/// A RATE HISTORY button clicked.
enum HistoryAction {
    Save,
    Export,
    Clear,
}

#[derive(Default)]
pub struct StatsWindow {
    /// Whether the window showed last frame, and in which mode (to size it on a switch).
    shown: Option<bool>,
    /// The rate history as last read, newest first (VibeSkua `RateHistory`; `models::stats`).
    history: Vec<RateSnapshot>,
    /// The column the table is sorted by, ascending or not; none: as saved.
    sort: Option<(Column, bool)>,
    confirm_clear: bool,
    export: SaveText,
}

impl StatsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            self.shown = None;
            return;
        }
        self.export.poll();
        let bot = crate::turu::bot();
        let advanced = bot.settings.read().stats_advanced_mode;
        if advanced {
            self.history = model::history(&bot);
        }
        let area = super::window_area(ctx);
        let height = if advanced { ADVANCED_HEIGHT } else { 250.0 };
        let mut window = super::window(ctx, "Stats", [WIDTH, height], 3).open(open);
        if !advanced {
            // Off, the window fits the counters (VibeSkua `SizeToContent.Height`).
            window = window.resizable(false);
        } else if widgets::touch() && (area.width() < 560.0 || area.height() < 400.0) {
            // On a phone the history needs the whole screen.
            window = window.sheet(ctx);
        } else if self.shown != Some(true) {
            // Turned on (or opened on): VibeSkua's fixed 760, within the screen; resizable after.
            let size = vec2(WIDTH, ADVANCED_HEIGHT).min(area.size() - vec2(24.0, 24.0));
            window = window.fixed_size(size);
        }
        self.shown = Some(advanced);
        window.show(ctx, |ui| self.ui(ui));
        self.dialogs(ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let elapsed = model::script_time(&bot);
        let stats = bot.state.lock().stats.clone();
        let (mut advanced, unit) = {
            let settings = bot.settings.read();
            (settings.stats_advanced_mode, RateUnit::parse(&settings.stats_rate_unit))
        };
        let control = widgets::target(26.0, 40.0);

        // Script time, Advanced switch, Reset.
        ui.horizontal(|ui| {
            ui.set_min_height(control);
            ui.label(
                RichText::new(stats::format_hms(elapsed))
                    .monospace()
                    .strong()
                    .color(TEXT),
            );
            ui.label(
                RichText::new("script time")
                    .size(widgets::small_label_size())
                    .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Reset", Kind::Normal, true, vec2(84.0, control), "")
                    .on_hover_text("Reset all combat and drop counters")
                    .clicked()
                {
                    model::reset(&bot);
                }
                ui.label("Advanced");
                if widgets::switch(ui, &mut advanced)
                    .on_hover_text("Show gameplay rates, level and class rank progress, and rate history")
                    .changed()
                {
                    model::set_advanced(&bot, advanced);
                }
            });
        });
        ui.add_space(4.0);
        counters(ui, &stats);
        if !advanced {
            return;
        }
        ui.add_space(6.0);
        gameplay(ui, &stats.gameplay, unit);
        ui.add_space(6.0);
        self.history_panel(ui);
    }

    /// RATE HISTORY: Save, Export, Clear and the table (VibeSkua's manual rate snapshots).
    fn history_panel(&mut self, ui: &mut Ui) {
        panel(ui, |ui| {
            let narrow = ui.available_width() < NARROW;
            let size = vec2(if narrow { 0.0 } else { 84.0 }, widgets::target(26.0, 40.0));
            let count = self.history.len();
            let has_history = count > 0;
            let mut clicked = None;
            let mut buttons = |ui: &mut Ui| {
                // Right to left: Clear, Export, Save.
                if widgets::button(ui, "Clear", Kind::Danger, has_history, size, "")
                    .on_hover_text("Delete all saved rate snapshots")
                    .clicked()
                {
                    clicked = Some(HistoryAction::Clear);
                }
                let platform = crate::platform::platform();
                if (platform.has_file_dialogs() || platform.shares_text())
                    && widgets::button(ui, "Export", Kind::Normal, has_history, size, "")
                        .on_hover_text("Export the history to a CSV file")
                        .clicked()
                {
                    clicked = Some(HistoryAction::Export);
                }
                if widgets::button(ui, "Save", Kind::Normal, true, size, "")
                    .on_hover_text("Save the current gameplay rates to the history")
                    .clicked()
                {
                    clicked = Some(HistoryAction::Save);
                }
            };
            ui.horizontal(|ui| {
                widgets::section_label(ui, "RATE HISTORY");
                widgets::count_badge(ui, count);
                if !narrow {
                    ui.with_layout(Layout::right_to_left(Align::Center), &mut buttons);
                }
            });
            if narrow {
                ui.horizontal(|ui| ui.with_layout(Layout::right_to_left(Align::Center), &mut buttons));
            }
            match clicked {
                Some(HistoryAction::Save) => {
                    let bot = crate::turu::bot();
                    model::save_snapshot(&bot);
                    self.history = model::history(&bot);
                }
                Some(HistoryAction::Export) => self.export_history(ui.ctx()),
                Some(HistoryAction::Clear) => self.confirm_clear = true,
                None => {}
            }
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Save a checkpoint of the current rates to compare runs. Rates are in the unit shown when saved.",
                    )
                    .size(10.0)
                    .color(MUTED),
                )
                .wrap(),
            );
            let height = (ui.available_height() - 4.0).max(130.0);
            egui::Frame::new()
                .fill(INSET)
                .stroke(Stroke::new(1.0, EDGE))
                .corner_radius(CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(height);
                    self.table(ui, height);
                });
        });
    }

    fn table(&mut self, ui: &mut Ui, height: f32) {
        if self.history.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(height / 2.0 - 12.0);
                ui.label(
                    RichText::new("No snapshots yet. Save one to start a history.")
                        .size(11.0)
                        .color(MUTED),
                );
            });
            return;
        }
        let mut rows: Vec<&RateSnapshot> = self.history.iter().collect();
        if let Some((column, ascending)) = self.sort {
            rows.sort_by(|a, b| {
                let order = column.compare(a, b);
                if ascending { order } else { order.reverse() }
            });
        }
        let mut clicked = None;
        let row_height = widgets::target(20.0, 32.0);
        ScrollArea::both()
            .id_salt("turu_stats_history")
            .max_height(height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("turu_stats_history_grid")
                    .striped(true)
                    .min_row_height(row_height)
                    .spacing(vec2(16.0, 2.0))
                    .show(ui, |ui| {
                        for column in Column::ALL {
                            let arrow = match self.sort {
                                Some((c, true)) if c == column => " ↑",
                                Some((c, false)) if c == column => " ↓",
                                _ => "",
                            };
                            let text = RichText::new(format!("{}{arrow}", column.header()))
                                .size(10.0)
                                .strong()
                                .color(MUTED);
                            let layout = if column.numeric() {
                                Layout::right_to_left(Align::Center)
                            } else {
                                Layout::left_to_right(Align::Center)
                            };
                            let response =
                                ui.with_layout(layout, |ui| ui.add(egui::Label::new(text).sense(Sense::click())));
                            if response.inner.clicked() {
                                clicked = Some(column);
                            }
                        }
                        ui.end_row();
                        for row in rows {
                            for column in Column::ALL {
                                let mut text = RichText::new(column.text(row)).size(11.0);
                                text = match column {
                                    Column::Saved | Column::Unit => text.color(MUTED),
                                    _ => text.color(TEXT),
                                };
                                if column.numeric() {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(text.monospace());
                                    });
                                } else {
                                    ui.label(text);
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        if let Some(column) = clicked {
            // Ascending, then descending, then as saved.
            self.sort = match self.sort {
                Some((c, true)) if c == column => Some((column, false)),
                Some((c, false)) if c == column => None,
                _ => Some((column, true)),
            };
        }
    }

    /// VibeSkua `ExportHistory`: what is saved now, as CSV.
    fn export_history(&mut self, ctx: &Context) {
        let Some(csv) = model::export(&crate::turu::bot()) else {
            return;
        };
        let platform = crate::platform::platform();
        if platform.has_file_dialogs() {
            self.export.ask(ctx, "StatsRateHistory.csv", csv);
        } else {
            platform.share_text("Turu rate history", &csv);
        }
    }

    fn dialogs(&mut self, ctx: &Context) {
        if self.confirm_clear {
            let count = self.history.len();
            let mut answer = None;
            let response = Modal::new(egui::Id::new("turu_stats_clear")).show(ctx, |ui| {
                ui.set_min_width(260.0);
                ui.set_max_width(420.0);
                ui.heading("Clear Rate History");
                ui.label(format!(
                    "Delete all {count} saved rate snapshots? This clears them for every client and cannot be undone."
                ));
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let size = vec2(72.0, widgets::target(28.0, 40.0));
                    if widgets::button(ui, "Yes", Kind::Primary, true, size, "").clicked() {
                        answer = Some(true);
                    }
                    if widgets::button(ui, "No", Kind::Normal, true, size, "").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if answer.is_some() || response.should_close() {
                self.confirm_clear = false;
            }
            if answer == Some(true) {
                let bot = crate::turu::bot();
                model::clear_history(&bot);
                self.history = model::history(&bot);
            }
        }
        let bot = crate::turu::bot();
        if let Some(text) = model::view_note(&bot) {
            let title = "Rate History";
            let mut close = false;
            let response = Modal::new(egui::Id::new("turu_stats_message")).show(ctx, |ui| {
                ui.set_min_width(260.0);
                ui.set_max_width(420.0);
                ui.heading(title);
                ui.label(text.as_str());
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let size = vec2(72.0, widgets::target(28.0, 40.0));
                    close = widgets::button(ui, "OK", Kind::Primary, true, size, "").clicked();
                });
            });
            if close || response.should_close() {
                model::dismiss_note(&bot);
            }
        }
    }
}

/// A bordered section (VibeSkua `Border.panel`).
fn panel<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

/// The six counter tiles, three to a row (VibeSkua `stat-tile`s).
fn counters(ui: &mut Ui, stats: &turu_bot::state::Stats) {
    let counters: Vec<(String, String, Color32)> = model::counters(stats)
        .into_iter()
        .map(|c| {
            let color = match c.kind {
                CounterKind::Kills | CounterKind::Completed => SUCCESS,
                CounterKind::Deaths => DANGER,
                CounterKind::Pickups => WARNING,
                CounterKind::Accepted => BLUE,
                CounterKind::Relogins => PURPLE,
            };
            (c.label, c.value, color)
        })
        .collect();
    let width = (ui.available_width() - 2.0 * ui.spacing().item_spacing.x) / 3.0;
    for row in counters.chunks(3) {
        ui.horizontal(|ui| {
            for (label, value, color) in row {
                egui::Frame::new()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, EDGE))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.set_width((width - 22.0).max(0.0));
                        ui.horizontal(|ui| {
                            widgets::dot(ui, *color, 3.5);
                            widgets::section_label(ui, label);
                        });
                        ui.label(RichText::new(value.as_str()).size(16.0).strong().color(TEXT));
                    });
            }
        });
    }
}

/// GAMEPLAY: session time and unit, Level and Rank progress with their rates and time left, then
/// Net gold, Items, Kills and Deaths, every rate in the chosen unit.
fn gameplay(ui: &mut Ui, g: &Gameplay, unit: RateUnit) {
    panel(ui, |ui| {
        ui.horizontal(|ui| {
            ui.set_min_height(widgets::target(26.0, 40.0));
            widgets::section_label(ui, "GAMEPLAY");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let bot = crate::turu::bot();
                let mut chosen = unit;
                egui::ComboBox::from_id_salt("turu_stats_rate_unit")
                    .width(64.0)
                    .selected_text(unit.label())
                    .show_ui(ui, |ui| {
                        for u in RateUnit::ALL {
                            ui.selectable_value(&mut chosen, u, u.label());
                        }
                    })
                    .response
                    .on_hover_text("Show rates per hour, minute or second");
                if chosen != unit {
                    bot.settings.write().stats_rate_unit = chosen.label().to_owned();
                    bot.save_settings();
                }
                ui.label(
                    RichText::new(stats::format_hms(g.session_time))
                        .monospace()
                        .size(11.0)
                        .color(TEXT),
                );
                ui.label(RichText::new("session").size(10.0).color(MUTED))
                    .on_hover_text("Time since the gameplay session started (login or last reset)");
            });
        });
        ui.add_space(4.0);
        let view = model::gameplay(g, unit);
        let level = &view.level;
        progress_row(ui, &level.label, None, SUCCESS, level.percent, &level.value);
        hint(ui, &level.hint);
        ui.add_space(6.0);
        let rank = &view.rank;
        progress_row(
            ui,
            &rank.label,
            Some("Current class rank; the bar runs to rank 10"),
            WARNING,
            rank.percent,
            &rank.value,
        );
        hint(ui, &rank.hint);
        ui.add_space(6.0);
        ui.separator();
        let rates: Vec<(String, String, Color32, Option<String>)> = view
            .rates
            .into_iter()
            .map(|r| {
                let color = match r.tint {
                    Tint::Success => SUCCESS,
                    Tint::Danger => DANGER,
                    _ => WARNING,
                };
                (r.label, r.value, color, Some(r.note).filter(|n| !n.is_empty()))
            })
            .collect();
        let columns = if ui.available_width() < NARROW { 1 } else { 2 };
        let gap = 20.0;
        let width = (ui.available_width() - gap * (columns - 1) as f32) / columns as f32;
        for row in rates.chunks(columns) {
            ui.horizontal(|ui| {
                for (i, (label, value, color, tip)) in row.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(gap - ui.spacing().item_spacing.x);
                    }
                    ui.allocate_ui_with_layout(vec2(width, 18.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_width(width);
                        let response = ui
                            .horizontal(|ui| {
                                widgets::dot(ui, *color, 3.5);
                                ui.label(RichText::new(label).size(11.0).color(TEXT));
                            })
                            .response;
                        if let Some(tip) = tip {
                            response.on_hover_text(tip.as_str());
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(value.as_str())
                                    .monospace()
                                    .size(11.0)
                                    .strong()
                                    .color(TEXT),
                            );
                        });
                    });
                }
            });
        }
    });
}

/// A label (84 wide), a 6-high bar and the value on the right.
fn progress_row(ui: &mut Ui, label: &str, tip: Option<&str>, color: Color32, percent: f64, value: &str) {
    ui.horizontal(|ui| {
        let response = ui
            .allocate_ui_with_layout(vec2(84.0, 18.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(84.0);
                widgets::dot(ui, color, 3.5);
                ui.label(RichText::new(label).size(11.0).color(TEXT));
            })
            .response;
        if let Some(tip) = tip {
            response.on_hover_text(tip);
        }
        let value = RichText::new(value).monospace().size(11.0).color(TEXT);
        let value_width = ui
            .painter()
            .layout_no_wrap(value.text().to_owned(), egui::FontId::monospace(11.0), TEXT)
            .size()
            .x;
        let bar_width = (ui.available_width() - value_width - 2.0 * ui.spacing().item_spacing.x).max(24.0);
        let (rect, _) = ui.allocate_exact_size(vec2(bar_width, 6.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(3), TRACK);
        let fraction = (percent / 100.0).clamp(0.0, 1.0) as f32;
        if fraction > 0.0 {
            let mut fill = rect;
            fill.set_width(rect.width() * fraction);
            painter.rect_filled(fill, CornerRadius::same(3), color);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(value));
    });
}

/// The muted rate / time-left line under a bar, from the bar's column.
fn hint(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(84.0 + ui.spacing().item_spacing.x);
        ui.label(RichText::new(text).size(10.0).color(MUTED));
    });
}
