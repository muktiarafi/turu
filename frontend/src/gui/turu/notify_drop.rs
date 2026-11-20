//! Combat > Runtime > Notifications (VibeSkua `NotifyDropView`): the watched item names and the
//! beep settings, drawn from `models::notify_drop` (which keeps them per tab and watches the drops).

use super::theme::{DANGER, INSET, MUTED, TEXT};
use super::widgets::{self, Kind, Selection};
use crate::models::notify_drop::{self as model, NotifyDropView};
use egui::{Align, Context, Key, Label, Layout, RichText, ScrollArea, Ui, vec2};
use std::sync::Arc;
use turu_bot::Bot;

pub const TITLE: &str = "Notify Drop";

/// Below this width the beep settings and Test take a row each.
const NARROW: f32 = 440.0;
/// From this width the beep settings sit in the WATCHING heading's row.
const WIDE: f32 = 720.0;
#[derive(Default)]
pub struct NotifyDropWindow {
    input: String,
    selection: Selection,
}

impl NotifyDropWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        // VibeSkua's 534x337; taller on a touch screen, whose controls and rows are finger-sized.
        let size = if widgets::touch() {
            [560.0, 480.0]
        } else {
            [534.0, 337.0]
        };
        let mut window = super::window(ctx, TITLE, size, 5).open(open);
        // On a phone a floating window leaves the box under the keyboard: it fills the screen.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 560.0 || area.height() < 400.0) {
            window = window.sheet(ctx);
        }
        let bot = crate::turu::bot();
        window.show(ctx, |ui| self.ui(ui, &bot));
    }

    fn ui(&mut self, ui: &mut Ui, bot: &Arc<Bot>) {
        let view = model::view(bot);
        let control = widgets::target(28.0, 40.0);
        let spacing = ui.spacing().item_spacing;
        let width = ui.available_width();

        row(ui, control, |ui| {
            let box_width = (ui.available_width() - 84.0 - spacing.x).max(120.0);
            let hint = "Item name (separate several with |)";
            let mut response = ui
                .add_sized(
                    [box_width, control],
                    egui::TextEdit::singleline(&mut self.input)
                        .hint_text(hint)
                        .vertical_align(Align::Center)
                        .margin(egui::Margin::symmetric(8, 5))
                        .background_color(INSET),
                )
                .on_hover_text("Use the exact in-game item names");
            let kind = super::native_field::Kind::Line;
            super::native_field::follow(ui, &mut response, &mut self.input, hint, kind);
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let add = widgets::button(ui, "+  Add", Kind::Primary, true, vec2(84.0, control), "").clicked();
            if add || enter {
                model::add(bot, &self.input);
                self.input.clear();
            }
        });

        let items = &view.items;
        let clear_size = vec2(84.0, widgets::target(24.0, 40.0));
        // Wide (a phone on its side, a big window): the beep settings join the heading, so the
        // list keeps its rows on a short screen.
        let wide = width >= WIDE;
        row(ui, control, |ui| {
            // A row of their own, as high as they are: the badge would stretch to the row's height.
            ui.horizontal(|ui| {
                widgets::section_label(ui, "WATCHING");
                widgets::count_badge(ui, items.len());
            });
            if wide {
                if widgets::button(ui, "Clear all", Kind::Outline, !items.is_empty(), clear_size, "").clicked() {
                    self.clear(bot);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.test_button(ui, bot, 96.0, control);
                    ui.add_space(4.0);
                    self.beep_settings(ui, bot, &view, control, true);
                });
            } else {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, "Clear all", Kind::Outline, !items.is_empty(), clear_size, "").clicked() {
                        self.clear(bot);
                    }
                });
            }
        });

        let narrow = width < NARROW;
        let footer = if wide {
            0.0
        } else if narrow {
            2.0 * control + 2.0 * spacing.y
        } else {
            control + spacing.y
        };
        let height = (ui.available_height() - footer).max(3.0 * widgets::target(26.0, 44.0));
        self.list(ui, bot, items, height);

        if narrow {
            // A phone held upright: the settings, then Test across the window.
            row(ui, control, |ui| self.beep_settings(ui, bot, &view, control, false));
            ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                self.test_button(ui, bot, ui.available_width(), control);
            });
        } else if !wide {
            row(ui, control, |ui| {
                self.beep_settings(ui, bot, &view, control, false);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.test_button(ui, bot, 96.0, control)
                });
            });
        }
    }

    /// The watched items: a click picks rows (several, as VibeSkua's `SelectionMode="Multiple"`);
    /// the row's ✕, Delete or the right-click menu removes them.
    fn list(&mut self, ui: &mut Ui, bot: &Arc<Bot>, items: &[String], height: f32) {
        if self.selection.picked().iter().any(|&i| i >= items.len()) {
            self.selection.clear();
        }
        let touch = widgets::touch();
        let row_height = widgets::target(26.0, 44.0);
        let mut remove: Option<Vec<usize>> = None;
        let mut clear = false;
        widgets::list_well(ui, height, |ui| {
            if items.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() / 2.0 - 24.0).max(4.0));
                    ui.label(RichText::new("Nothing watched yet").color(TEXT));
                    ui.label(RichText::new("Turu beeps when a listed item drops.").color(MUTED));
                    ui.label(RichText::new("Add exact item names above.").color(MUTED));
                });
                return;
            }
            let shown: Vec<usize> = (0..items.len()).collect();
            ScrollArea::vertical()
                .id_salt("notify_drop_list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, item) in items.iter().enumerate() {
                        let selected = self.selection.contains(i);
                        let row = widgets::list_row(ui, selected, row_height, |ui| {
                            widgets::dot(ui, MUTED, 3.0);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let hovered = ui.rect_contains_pointer(ui.max_rect());
                                if (hovered || selected || touch)
                                    && ui
                                        .add(egui::Button::new(RichText::new("✕").color(DANGER)).frame(false))
                                        .on_hover_text("Stop watching this item")
                                        .clicked()
                                {
                                    remove = Some(vec![i]);
                                }
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.add(Label::new(RichText::new(item).color(TEXT)).truncate())
                                        .on_hover_text(item);
                                });
                            });
                        });
                        if row.clicked() {
                            let shift = ui.input(|i| i.modifiers.shift);
                            self.selection.click(i, &shown, true, shift);
                        }
                        row.context_menu(|ui| {
                            if ui.button("Remove Selected").clicked() {
                                remove = Some(self.selection.picked());
                                ui.close();
                            }
                            if ui.button("Clear All").clicked() {
                                clear = true;
                                ui.close();
                            }
                        });
                    }
                });
        });
        if remove.is_none()
            && !self.selection.is_empty()
            && ui.input(|i| i.key_pressed(Key::Delete))
            && !ui.ctx().egui_wants_keyboard_input()
        {
            remove = Some(self.selection.picked());
        }
        if clear {
            self.clear(bot);
        } else if let Some(rows) = remove {
            let names: Vec<String> = rows.iter().filter_map(|&i| items.get(i).cloned()).collect();
            model::remove(bot, &names);
            self.selection.clear();
        }
    }

    /// "Beep [n] times every [ms]" (VibeSkua's `NumericUpDown`s: 1 to 25, 10 to 2000 ms), in a
    /// row the caller lays out; `reversed` for a right-to-left row.
    fn beep_settings(&self, ui: &mut Ui, bot: &Arc<Bot>, view: &NotifyDropView, control: f32, reversed: bool) {
        let order = if reversed { [3, 2, 1, 0] } else { [0, 1, 2, 3] };
        for part in order {
            match part {
                0 => {
                    ui.label(RichText::new("Beep").color(MUTED));
                }
                1 => {
                    let mut count = view.count;
                    let size = vec2(56.0, control);
                    let range = view.count_min..=view.count_max;
                    let response = widgets::number(ui, "notify_drop_count", &mut count, range, 0.1, "", size)
                        .on_hover_text("How many beeps per drop");
                    if response.changed() {
                        model::set_count(bot, count);
                    }
                }
                2 => {
                    ui.label(RichText::new("times every").color(MUTED));
                }
                _ => {
                    let mut delay = view.delay;
                    let size = vec2(84.0, control);
                    let range = view.delay_min..=view.delay_max;
                    let response = widgets::number(ui, "notify_drop_delay", &mut delay, range, 10.0, " ms", size)
                        .on_hover_text("Milliseconds between beeps");
                    if response.changed() {
                        model::set_delay(bot, delay);
                    }
                }
            }
        }
    }

    fn test_button(&self, ui: &mut Ui, bot: &Arc<Bot>, width: f32, control: f32) {
        if widgets::button(ui, "Test beep", Kind::Normal, true, vec2(width, control), "")
            .on_hover_text("Play the alert")
            .clicked()
        {
            model::test(bot);
        }
    }

    /// VibeSkua `RemoveAllDropsCommand` (the list's `Clear`).
    fn clear(&mut self, bot: &Arc<Bot>) {
        model::clear(bot);
        self.selection.clear();
    }
}

/// A row `height` tall across `ui`, its contents centred on it.
fn row<R>(ui: &mut Ui, height: f32, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(vec2(width, height), Layout::left_to_right(Align::Center), add_contents)
        .inner
}
