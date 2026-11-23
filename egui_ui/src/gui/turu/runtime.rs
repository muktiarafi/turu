//! Combat > Runtime (VibeSkua `RuntimeHelpersView`): the drop pickup list and loop, the
//! registered quests and the boosts, editable while a script runs or without one.

use super::theme::{INSET, MUTED, TEXT};
use super::widgets::{self, Kind};
use crate::models::runtime::{self, RuntimeView};
use egui::{Context, CornerRadius, Frame, Margin, RichText, ScrollArea, Ui, vec2};
use std::collections::HashSet;
use std::time::Duration;

/// Below this width the two columns stack.
const STACK_BELOW: f32 = 680.0;

#[derive(Default)]
pub struct RuntimeWindow {
    drop_input: String,
    interval: String,
    selected_drops: HashSet<String>,
    quest_input: String,
    reward_input: String,
    selected_quests: HashSet<i64>,
    /// Notifications was clicked: the tab opens Notify Drop.
    pub open_notify_drop: bool,
}

impl RuntimeWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Runtime", [955.0, 596.0], 4)
            .open(open)
            .show(ctx, |ui| {
                let bot = crate::turu::bot();
                let view = runtime::view(&bot);
                if ui.available_width() >= STACK_BELOW {
                    ui.columns(2, |cols| {
                        self.drops_card(&mut cols[0], &view);
                        self.quests_card(&mut cols[1], &view);
                        cols[1].add_space(4.0);
                        self.boosts_card(&mut cols[1], &view);
                    });
                } else {
                    // Too narrow for two columns: one under the other (the window scrolls),
                    // each list a few rows high.
                    let width = ui.available_width();
                    ui.allocate_ui(vec2(width, 300.0), |ui| self.drops_card(ui, &view));
                    ui.add_space(4.0);
                    ui.allocate_ui(vec2(width, 420.0), |ui| self.quests_card(ui, &view));
                    ui.add_space(4.0);
                    self.boosts_card(ui, &view);
                }
            });
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn drops_card(&mut self, ui: &mut Ui, view: &RuntimeView) {
        let bot = crate::turu::bot();
        let items = &view.drops;
        self.selected_drops.retain(|s| items.contains(s));
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            header(ui, "DROPS HELPER", !self.selected_drops.is_empty(), || {
                let selected: Vec<String> = self.selected_drops.drain().collect();
                runtime::remove_drops(&bot, &selected);
            });
            let height = (ui.available_height() - 110.0).max(80.0);
            list(
                ui,
                "runtime_drops",
                height,
                items.is_empty(),
                "No drops in the pickup list.",
                |ui| {
                    for item in items {
                        let selected = self.selected_drops.contains(item);
                        if ui.selectable_label(selected, RichText::new(item).color(TEXT)).clicked() {
                            toggle(&mut self.selected_drops, item.clone());
                        }
                    }
                },
            );

            ui.horizontal(|ui| {
                let width = ui.available_width() - 90.0;
                let response = widgets::text_box(ui, &mut self.drop_input, "Drop Name (e.g. Unidentified 13)", width);
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let add = widgets::button(ui, "Add Drop", Kind::Primary, true, vec2(82.0, 26.0), "").clicked();
                if add || enter {
                    runtime::add_drops(&bot, &self.drop_input);
                    self.drop_input.clear();
                }
            });

            ui.horizontal(|ui| {
                let response = widgets::text_box(ui, &mut self.interval, "Interval (ms)", 96.0);
                if response.changed() {
                    let text = self.interval.trim();
                    if text.is_empty() {
                        bot.drops.set_interval(0);
                    } else if let Ok(ms) = text.parse::<u64>() {
                        bot.drops.set_interval(ms);
                    }
                }
                if !response.has_focus() {
                    self.interval = view.interval.to_string();
                }
                response.on_hover_text("Waited on top of the loop's 1 s tick");
                let mut reject_else = view.reject_else;
                if ui.checkbox(&mut reject_else, "Reject Else").changed() {
                    bot.drops.set_reject_else(reject_else);
                }
                let mut accept_ac = view.accept_ac;
                if ui.checkbox(&mut accept_ac, "Accept AC Drops").changed() {
                    bot.options.write().accept_ac_drops = accept_ac;
                }
            });

            ui.horizontal(|ui| {
                let width = ui.available_width() - 110.0;
                if toggle_button(ui, "Pick Drops", view.picking, width) {
                    runtime::set_picking(&bot, !view.picking);
                }
                // VibeSkua `OpenNotifyDrop`.
                if widgets::button(ui, "Notifications", Kind::Normal, true, vec2(102.0, 28.0), "")
                    .on_hover_text("Beep when a listed item drops")
                    .clicked()
                {
                    self.open_notify_drop = true;
                }
            });
        });
    }

    fn quests_card(&mut self, ui: &mut Ui, view: &RuntimeView) {
        let bot = crate::turu::bot();
        let quests = &view.quests;
        self.selected_quests.retain(|id| quests.iter().any(|q| q.id == *id));
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            header(ui, "QUESTS HELPER", !self.selected_quests.is_empty(), || {
                let selected: Vec<i64> = self.selected_quests.drain().collect();
                bot.quests.unregister(&selected);
            });
            let height = (ui.available_height() - 292.0).max(60.0);
            list(
                ui,
                "runtime_quests",
                height,
                quests.is_empty(),
                "No registered quests.",
                |ui| {
                    for quest in quests {
                        let selected = self.selected_quests.contains(&quest.id);
                        if ui
                            .selectable_label(selected, RichText::new(&quest.label).color(TEXT))
                            .clicked()
                        {
                            toggle(&mut self.selected_quests, quest.id);
                        }
                    }
                },
            );

            ui.horizontal(|ui| {
                let width = ui.available_width() - 130.0 - 92.0 - 2.0 * ui.spacing().item_spacing.x;
                let response = widgets::text_box(ui, &mut self.quest_input, "Quest ID", width);
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                widgets::text_box(ui, &mut self.reward_input, "Reward ID (opt)", 130.0);
                let add = widgets::button(ui, "Add Quest", Kind::Primary, true, vec2(84.0, 26.0), "").clicked();
                let reward = self.reward_input.trim().parse::<i64>().unwrap_or(-1);
                if (add || enter) && runtime::add_quests(&bot, &self.quest_input, reward).is_ok() {
                    self.quest_input.clear();
                    self.reward_input.clear();
                }
            });
        });
    }

    fn boosts_card(&self, ui: &mut Ui, view: &RuntimeView) {
        let bot = crate::turu::bot();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "BOOSTS HELPER");
            let searching = view.searching;
            ui.columns(2, |cols| {
                for (col, (label, bank)) in cols
                    .iter_mut()
                    .zip([("Search Inventory", false), ("Search Bank", true)])
                {
                    let width = col.available_width();
                    let tooltip = "Searching...";
                    if widgets::button(col, label, Kind::Normal, !searching, vec2(width, 26.0), tooltip).clicked() {
                        runtime::search(&bot, bank);
                    }
                }
            });

            Frame::new()
                .fill(INSET)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for boost in &view.boosts {
                        ui.horizontal(|ui| {
                            let mut uses = boost.uses;
                            if ui.checkbox(&mut uses, &boost.label).changed() {
                                let _ = runtime::set_boost(&bot, &boost.kind, uses);
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("ID: {}", boost.id)).color(MUTED));
                            });
                        });
                    }
                });

            if toggle_button(ui, "Use Boosts", view.boosting, ui.available_width()) {
                runtime::set_boosting(&bot, !view.boosting);
            }
        });
    }
}

/// A card heading with VibeSkua's "Remove Selected" on the right.
fn header(ui: &mut Ui, title: &str, any_selected: bool, remove: impl FnOnce()) {
    ui.horizontal(|ui| {
        widgets::section_label(ui, title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let tooltip = "Select rows in the list first";
            if widgets::button(
                ui,
                "Remove Selected",
                Kind::Outline,
                any_selected,
                vec2(0.0, 24.0),
                tooltip,
            )
            .clicked()
            {
                remove();
            }
        });
    });
}

/// A framed, scrolling list of `height` (VibeSkua's `list-frame`), or `empty` when there is nothing.
fn list(ui: &mut Ui, id: &str, height: f32, is_empty: bool, empty: &str, rows: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(INSET)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(height);
            ScrollArea::vertical()
                .id_salt(id)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if is_empty {
                        ui.label(RichText::new(empty).color(MUTED));
                    } else {
                        rows(ui);
                    }
                });
        });
}

/// VibeSkua's `option-chip` toggle: accent while on. Returns whether it was clicked.
fn toggle_button(ui: &mut Ui, label: &str, on: bool, width: f32) -> bool {
    let kind = if on { Kind::Primary } else { Kind::Normal };
    widgets::button(ui, label, kind, true, vec2(width, 28.0), "").clicked()
}

fn toggle<T: Eq + std::hash::Hash>(set: &mut HashSet<T>, value: T) {
    if !set.remove(&value) {
        set.insert(value);
    }
}
