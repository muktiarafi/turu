//! Tools & Helpers > Junk Items (VibeSkua `JunkItemsView` / `JunkItemsViewModel`): the inventory
//! and bank items with a check box each, filtered by category, AC and highlight (Equipped,
//! favourites, near the stack limit, quantity goals, junk), marking the selected ones as junk
//! (`JunkItems.json`), quantity goals for them (`InventoryGoals`), and selling the junk in the
//! inventory. The list, marking and selling are the model's (`models::junk`, shared with a native
//! page); the window keeps its own search, filters and selection.

use super::theme::{DANGER, EDGE, MUTED, TEXT, TRACK, WARNING};
use super::widgets::{self, Kind};
use crate::models::junk::{self as junk_model, Key, Snapshot};
use egui::{
    Align, Checkbox, Color32, ComboBox, Context, CornerRadius, Frame, Label, Layout, Margin, Modal, Response, RichText,
    ScrollArea, Sense, Shape, Stroke, StrokeKind, Ui, vec2,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use turu_bot::Bot;
use turu_bot::junk::{self, AcFilter, Entry, Filter, Highlight};

const ROW_HEIGHT: f32 = 30.0;
/// A row on two lines (name, then chips and quantities) when the list is narrow.
const TWO_LINE_ROW_HEIGHT: f32 = 46.0;
const CONTROL_HEIGHT: f32 = 26.0;
/// Narrower than this, the filters, the goal row and the sell row take two lines each.
const NARROW: f32 = 520.0;
/// Narrower than this, the rows take two lines.
const NARROW_ROWS: f32 = 640.0;

/// The confirmation a button asked for.
#[derive(Clone, Copy, PartialEq)]
enum Confirm {
    SellSelected(usize),
    SellAll,
    SkipWarning,
}

#[derive(Default)]
pub struct JunkItemsWindow {
    /// The search box and the three filters (VibeSkua `ApplySearch`).
    filter: Filter,
    /// The selected items, kept over a refresh (by ID and category, as VibeSkua).
    selected: HashSet<Key>,
    /// VibeSkua `GoalQuantityInput`.
    goal_input: String,
    /// The window was open last frame (VibeSkua refreshes when it is activated).
    was_open: bool,
    confirm: Option<Confirm>,
}

impl JunkItemsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        let bot = crate::turu::bot();
        if *open && !self.was_open {
            junk_model::refresh(&bot);
        }
        self.was_open = *open;
        if !*open {
            return;
        }
        super::window(ctx, "Junk Items", [700.0, 500.0], 3)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &bot));
        self.confirm_dialog(ctx, &bot);
        if junk_model::snapshot(&bot).busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn ui(&mut self, ui: &mut Ui, bot: &Arc<Bot>) {
        let Snapshot {
            entries, busy, error, ..
        } = junk_model::snapshot(bot);
        // The selection keeps the items still listed; the category goes back to "All Categories"
        // when the chosen one is gone.
        let categories = junk::categories(&entries);
        if !entries.is_empty() {
            let keys: HashSet<Key> = entries.iter().map(junk_model::key).collect();
            self.selected.retain(|k| keys.contains(k));
            if !categories.contains(&self.filter.category) {
                self.filter.category = junk::ALL_CATEGORIES.into();
            }
        }
        let skip_warning = bot.settings.read().junk_skip_sell_warning;
        let control = widgets::target(CONTROL_HEIGHT, 40.0);
        let narrow = ui.available_width() < NARROW;
        // Narrow, the buttons take the width of their label.
        let width = |w: f32| if narrow { 0.0 } else { w };

        // Search and refresh.
        ui.horizontal(|ui| {
            let width = ui.available_width() - control.max(32.0) - 8.0;
            widgets::text_box(ui, &mut self.filter.search, "Search by ID or name", width);
            if widgets::button(ui, "⟳", Kind::Normal, !busy, vec2(control.max(32.0), control), "Busy")
                .on_hover_text("Refresh inventory and bank")
                .clicked()
            {
                junk_model::refresh(bot);
            }
        });

        // Item filters: category, AC and highlight (the category on its own line when narrow).
        if narrow {
            self.category_box(ui, &categories, ui.available_width());
            ui.horizontal(|ui| {
                let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                self.ac_box(ui, width);
                self.highlight_box(ui, width);
            });
        } else {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 2.0 * ui.spacing().item_spacing.x) / 3.0;
                self.category_box(ui, &categories, width);
                self.ac_box(ui, width);
                self.highlight_box(ui, width);
            });
        }

        let shown: Vec<&Entry> = entries.iter().filter(|e| self.filter.matches(e)).collect();
        let selected = self.selected.len();
        let total_junk = entries.iter().filter(|e| e.junk).count();

        // Selection bar: select all / clear, and what to do with the selection.
        ui.horizontal(|ui| {
            ui.add_space(9.0);
            let all_shown = !shown.is_empty() && shown.iter().all(|e| self.selected.contains(&junk_model::key(e)));
            let mut checked = selected > 0 && all_shown;
            let label = RichText::new(format!("{selected} selected")).strong();
            let response = ui
                .add(Checkbox::new(&mut checked, label).indeterminate(selected > 0 && !all_shown))
                .on_hover_text("Select or clear all shown items");
            if response.clicked() {
                // An unchecked box selects the shown items; a checked or partly checked one clears.
                if selected == 0 {
                    self.selected.extend(shown.iter().map(|e| junk_model::key(e)));
                } else {
                    self.selected.clear();
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Unmark", Kind::Normal, true, vec2(width(84.0), control), "").clicked()
                    && !entries.is_empty()
                {
                    junk_model::mark(bot, &std::mem::take(&mut self.selected), false);
                }
                if widgets::button(ui, "Mark as junk", Kind::Normal, true, vec2(width(112.0), control), "").clicked()
                    && !entries.is_empty()
                {
                    junk_model::mark(bot, &self.selected, true);
                }
            });
        });

        // Quantity goals for the selected rows (the label and note on their own line when narrow).
        let goal_label = |ui: &mut Ui| ui.label(RichText::new("🎯 Goal quantity").size(11.0).color(TEXT));
        if narrow {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                goal_label(ui);
                goal_note(ui);
            });
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 7.0;
            if !narrow {
                goal_label(ui);
            }
            widgets::text_box(ui, &mut self.goal_input, "Target", 90.0);
            if widgets::button(ui, "Set goal", Kind::Normal, true, vec2(0.0, control), "").clicked()
                && let Some(quantity) = junk::parse_goal(&self.goal_input)
            {
                junk_model::set_goal(bot, &self.selected, Some(quantity));
                self.goal_input.clear();
            }
            if widgets::button(ui, "Clear goal", Kind::Normal, true, vec2(0.0, control), "").clicked() {
                junk_model::set_goal(bot, &self.selected, None);
            }
            if !narrow {
                goal_note(ui);
            }
        });

        // Inventory and bank items.
        // Room for the sell row(s) below (and the error line, when there is one).
        let sell_rows = if narrow { 2.0 } else { 1.0 };
        let spacing = ui.spacing().item_spacing.y;
        let below = sell_rows * (control + spacing) + 18.0 + if error.is_some() { 20.0 } else { 0.0 };
        let row_height = if ui.available_width() < NARROW_ROWS {
            TWO_LINE_ROW_HEIGHT
        } else {
            ROW_HEIGHT
        };
        let list_height = (ui.available_height() - below).max(ROW_HEIGHT * 3.0);
        let empty = entries.is_empty();
        widgets::list_well(ui, list_height, |ui| self.list(ui, &shown, empty, busy, row_height));

        if let Some(error) = error {
            ui.label(RichText::new(error).small().color(DANGER));
        }

        // Selling (the buttons on their own line when narrow).
        if narrow && skip_box(ui, bot, skip_warning) {
            self.confirm = Some(Confirm::SkipWarning);
        }
        ui.horizontal(|ui| {
            if !narrow && skip_box(ui, bot, skip_warning) {
                self.confirm = Some(Confirm::SkipWarning);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = format!("Sell all junk ({total_junk})");
                if widgets::button(ui, &label, Kind::Danger, !busy, vec2(width(140.0), control), "Busy").clicked()
                    && total_junk > 0
                {
                    if skip_warning {
                        junk_model::sell_all(bot);
                    } else {
                        self.confirm = Some(Confirm::SellAll);
                    }
                }
                if widgets::button(
                    ui,
                    "Sell selected",
                    Kind::Outline,
                    !busy,
                    vec2(width(112.0), control),
                    "Busy",
                )
                .clicked()
                    && selected > 0
                {
                    if skip_warning {
                        junk_model::sell_selected(bot, &self.selected);
                    } else {
                        self.confirm = Some(Confirm::SellSelected(selected));
                    }
                }
                if widgets::button(ui, "Unmark all", Kind::Normal, true, vec2(width(112.0), control), "").clicked()
                    && !entries.is_empty()
                {
                    self.selected.clear();
                    junk_model::unmark_all(bot);
                }
            });
        });
    }

    fn category_box(&mut self, ui: &mut Ui, categories: &[String], width: f32) {
        ComboBox::from_id_salt("turu_junk_category")
            .selected_text(self.filter.category.as_str())
            .width(width)
            .show_ui(ui, |ui| {
                if categories.is_empty() {
                    ui.selectable_value(
                        &mut self.filter.category,
                        junk::ALL_CATEGORIES.into(),
                        junk::ALL_CATEGORIES,
                    );
                }
                for category in categories {
                    ui.selectable_value(&mut self.filter.category, category.clone(), category);
                }
            })
            .response
            .on_hover_text("Filter by item category");
    }

    fn ac_box(&mut self, ui: &mut Ui, width: f32) {
        ComboBox::from_id_salt("turu_junk_ac")
            .selected_text(self.filter.ac.label())
            .width(width)
            .show_ui(ui, |ui| {
                for ac in AcFilter::ALL {
                    ui.selectable_value(&mut self.filter.ac, ac, ac.label());
                }
            })
            .response
            .on_hover_text("Filter AC items or non-AC items");
    }

    fn highlight_box(&mut self, ui: &mut Ui, width: f32) {
        ComboBox::from_id_salt("turu_junk_highlight")
            .selected_text(self.filter.highlight.label())
            .width(width)
            .show_ui(ui, |ui| {
                for highlight in Highlight::ALL {
                    ui.selectable_value(&mut self.filter.highlight, highlight, highlight.label());
                }
            })
            .response
            .on_hover_text("Filter by inventory intelligence status");
    }

    fn list(&mut self, ui: &mut Ui, shown: &[&Entry], empty: bool, busy: bool, row_height: f32) {
        if shown.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                let text = if busy {
                    "Loading inventory and bank..."
                } else if empty {
                    "No items: log in, then press ⟳."
                } else {
                    "No items match."
                };
                ui.label(RichText::new(text).color(MUTED));
            });
            return;
        }
        let two_lines = row_height > ROW_HEIGHT;
        ScrollArea::vertical()
            .id_salt("turu_junk_items")
            .auto_shrink([false, false])
            .show_rows(ui, row_height + 2.0, shown.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for &entry in &shown[range] {
                    let key = junk_model::key(entry);
                    let selected = self.selected.contains(&key);
                    // The equipped tint goes under the row (VibeSkua `junk-row.equipped`); a
                    // selected row shows its selection instead.
                    let background = ui.painter().add(Shape::Noop);
                    let mut checked = selected;
                    let response = widgets::list_row(ui, selected, row_height, |ui| {
                        ui.add(Checkbox::without_text(&mut checked));
                        if two_lines {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 3.0;
                                ui.horizontal(|ui| {
                                    item_id(ui, entry, 18.0);
                                    item_name(ui, entry);
                                });
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    chips(ui, entry);
                                    quantities(ui, entry);
                                    if entry.junk {
                                        chip(ui, "Junk", Tone::Danger);
                                    }
                                });
                            });
                        } else {
                            item_id(ui, entry, ROW_HEIGHT);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                if entry.junk {
                                    chip(ui, "Junk", Tone::Danger);
                                }
                                quantities(ui, entry);
                                // Right to left: drawn in reverse.
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    let room = ui.available_width() - chips_width(ui, entry);
                                    ui.allocate_ui_with_layout(
                                        vec2(room.max(40.0), ROW_HEIGHT),
                                        Layout::left_to_right(Align::Center),
                                        |ui| item_name(ui, entry),
                                    );
                                    chips(ui, entry);
                                });
                            });
                        }
                    });
                    if entry.equipped && !selected {
                        let rect = response.rect;
                        let radius = CornerRadius::same(6);
                        ui.painter().set(
                            background,
                            Shape::Vec(vec![
                                Shape::rect_filled(rect, radius, WARNING.gamma_multiply(0.10)),
                                Shape::rect_stroke(
                                    rect,
                                    radius,
                                    Stroke::new(1.0, WARNING.gamma_multiply(0.45)),
                                    StrokeKind::Inside,
                                ),
                            ]),
                        );
                    }
                    // A click anywhere on the row toggles its check box.
                    if response.clicked() {
                        checked = !selected;
                    }
                    if checked != selected {
                        if checked {
                            self.selected.insert(key);
                        } else {
                            self.selected.remove(&key);
                        }
                    }
                }
            });
    }

    fn confirm_dialog(&mut self, ctx: &Context, bot: &Arc<Bot>) {
        let Some(confirm) = self.confirm else {
            return;
        };
        let (title, text) = match confirm {
            Confirm::SellSelected(count) => (
                "Sell Selected",
                format!("This will sell {count} selected items from your inventory. This cannot be undone. Continue?"),
            ),
            Confirm::SellAll => (
                "Sell All Junk",
                "This will sell all items marked as junk in your inventory. This cannot be undone. Continue?"
                    .to_owned(),
            ),
            Confirm::SkipWarning => (
                "Skip Sell Warning",
                "Are you sure you want to skip the confirmation when selling all junk?".to_owned(),
            ),
        };
        let mut answer = None;
        let response = Modal::new(egui::Id::new("turu_junk_confirm")).show(ctx, |ui| {
            ui.set_min_width(300.0_f32.min(ctx.content_rect().width() - 48.0));
            ui.set_max_width(420.0);
            ui.heading(title);
            ui.label(text);
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Yes", Kind::Primary, true, vec2(72.0, 28.0), "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "No", Kind::Normal, true, vec2(72.0, 28.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        if answer.is_none() && !response.should_close() {
            return;
        }
        self.confirm = None;
        if answer == Some(true) {
            match confirm {
                Confirm::SellSelected(_) => junk_model::sell_selected(bot, &self.selected),
                Confirm::SellAll => junk_model::sell_all(bot),
                Confirm::SkipWarning => junk_model::set_skip_warning(bot, true),
            }
        }
    }
}

/// "Skip sell warning": turning it off saves at once; true when turning it on wants the
/// confirmation (VibeSkua `OnSkipSellWarningChanged`).
fn skip_box(ui: &mut Ui, bot: &Arc<Bot>, skip_warning: bool) -> bool {
    let mut skip = skip_warning;
    if !ui.checkbox(&mut skip, "Skip sell warning").changed() {
        return false;
    }
    if !skip {
        junk_model::set_skip_warning(bot, false);
    }
    skip
}

/// The goal row's grey note.
fn goal_note(ui: &mut Ui) {
    ui.label(RichText::new("Counts inventory + bank").size(10.0).color(MUTED));
}

/// The item's ID, monospaced and grey, in a 52-point column.
fn item_id(ui: &mut Ui, entry: &Entry, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(52.0, height), Sense::hover());
    ui.painter().text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        entry.id.to_string(),
        egui::FontId::monospace(11.0),
        MUTED,
    );
}

/// The item's name, cut to fit, its category on hover; equipped, bold and amber.
fn item_name(ui: &mut Ui, entry: &Entry) {
    let text = if entry.equipped {
        RichText::new(&entry.name).strong().color(WARNING)
    } else {
        RichText::new(&entry.name).color(TEXT)
    };
    ui.add(Label::new(text).truncate().selectable(false))
        .on_hover_text(&entry.category);
}

/// "Inv: 3  |  Bank: 5" (VibeSkua `QuantityDisplay`).
fn quantities(ui: &mut Ui, entry: &Entry) {
    ui.add(
        Label::new(RichText::new(entry.quantity_display()).size(11.0).color(MUTED))
            .wrap_mode(egui::TextWrapMode::Extend)
            .selectable(false),
    );
}

/// The AC, Equipped, ★, Near cap and goal chips the entry has, in VibeSkua's order.
fn chips(ui: &mut Ui, entry: &Entry) {
    for (text, tone, tip) in chip_list(entry) {
        let response = chip(ui, &text, tone);
        if !tip.is_empty() {
            response.on_hover_text(tip);
        }
    }
}

/// How wide [`chips`] draws.
fn chips_width(ui: &Ui, entry: &Entry) -> f32 {
    let spacing = ui.spacing().item_spacing.x;
    chip_list(entry)
        .iter()
        .map(|(text, _, _)| chip_text_width(ui, text) + 14.0 + 2.0 + spacing)
        .sum()
}

fn chip_list(entry: &Entry) -> Vec<(String, Tone, &'static str)> {
    let mut chips = Vec::new();
    if entry.ac {
        chips.push(("AC".to_owned(), Tone::Plain, ""));
    }
    if entry.equipped {
        chips.push(("Equipped".to_owned(), Tone::Plain, "Currently equipped"));
    }
    if entry.favorite {
        chips.push(("★".to_owned(), Tone::Plain, "Favorited in AQW"));
    }
    if entry.near_stack_limit {
        chips.push((
            "Near cap".to_owned(),
            Tone::Warning,
            "Inventory stack is at least 80% full",
        ));
    }
    if entry.goal_tracked() {
        chips.push((
            entry.goal_progress(),
            Tone::Plain,
            "Progress toward your tracked quantity goal; inventory and bank combined",
        ));
    }
    chips
}

/// VibeSkua's chip styles: `chip`, `chip warning`, `chip danger`.
#[derive(Clone, Copy)]
enum Tone {
    Plain,
    Warning,
    Danger,
}

fn chip_font() -> egui::FontId {
    egui::FontId::proportional(10.0)
}

fn chip_text_width(ui: &Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), chip_font(), TEXT).size().x
}

/// A small rounded chip.
fn chip(ui: &mut Ui, text: &str, tone: Tone) -> Response {
    let (fill, stroke, color) = match tone {
        Tone::Plain => (TRACK, EDGE, TEXT),
        Tone::Warning => (WARNING.gamma_multiply(0.18), WARNING.gamma_multiply(0.6), WARNING),
        Tone::Danger => (
            DANGER.gamma_multiply(0.18),
            DANGER.gamma_multiply(0.6),
            Color32::from_rgb(0xFC, 0xA5, 0xA5),
        ),
    };
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::symmetric(7, 1))
        .show(ui, |ui| {
            ui.add(
                Label::new(RichText::new(text).font(chip_font()).strong().color(color))
                    .wrap_mode(egui::TextWrapMode::Extend)
                    .selectable(false),
            );
        })
        .response
}

#[cfg(test)]
mod tests {
    /// The goal row's 🎯 has a glyph.
    #[test]
    fn goal_symbol_renders() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        super::super::theme::add_symbol_font(&mut fonts);
        ctx.set_fonts(fonts);
        super::super::theme::install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let font = egui::FontId::proportional(11.0);
        let uv = |c: char| {
            let galley = ctx.fonts_mut(|f| f.layout_no_wrap(c.to_string(), font.clone(), egui::Color32::WHITE));
            galley.rows[0].glyphs[0].uv_rect
        };
        assert_ne!(uv('🎯'), uv('\u{FFFF}'));
    }
}
