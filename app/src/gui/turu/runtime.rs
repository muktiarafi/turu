//! Combat > Runtime (VibeSkua `RuntimeHelpersView`): the drop pickup list and loop, the
//! registered quests and the boosts, editable while a script runs or without one.

use super::theme::{INSET, MUTED, TEXT};
use super::widgets::{self, Kind};
use egui::{Context, CornerRadius, Frame, Margin, RichText, ScrollArea, Ui, vec2};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use turu_bot::api::NameOrId;
use turu_bot::api::boosts::{self, BoostType};

#[derive(Default)]
pub struct RuntimeWindow {
    drop_input: String,
    interval: String,
    selected_drops: HashSet<String>,
    quest_input: String,
    reward_input: String,
    selected_quests: HashSet<i64>,
    /// A Search Inventory / Search Bank runs.
    searching: Arc<AtomicBool>,
}

impl RuntimeWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Runtime", [955.0, 596.0], 4)
            .open(open)
            .show(ctx, |ui| {
                ui.columns(2, |cols| {
                    self.drops_card(&mut cols[0]);
                    self.quests_card(&mut cols[1]);
                    cols[1].add_space(4.0);
                    self.boosts_card(&mut cols[1]);
                });
            });
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn drops_card(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        // VibeSkua's list: the names, then the IDs as text.
        let items: Vec<String> = bot
            .drops
            .to_pickup()
            .into_iter()
            .chain(bot.drops.to_pickup_ids().iter().map(i64::to_string))
            .collect();
        self.selected_drops.retain(|s| items.contains(s));
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            header(ui, "DROPS HELPER", !self.selected_drops.is_empty(), || {
                let selected: Vec<String> = self.selected_drops.drain().collect();
                bot.drops.remove(&names_or_ids(&selected));
            });
            let height = (ui.available_height() - 110.0).max(160.0);
            list(
                ui,
                "runtime_drops",
                height,
                items.is_empty(),
                "No drops in the pickup list.",
                |ui| {
                    for item in &items {
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
                    // VibeSkua `AddDrop`: `|`-separated, a number is an item ID.
                    let inputs: Vec<String> = self
                        .drop_input
                        .split('|')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                    bot.drops.add(&names_or_ids(&inputs));
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
                    self.interval = bot.drops.interval().to_string();
                }
                response.on_hover_text("Waited on top of the loop's 1 s tick");
                let mut reject_else = bot.drops.reject_else();
                if ui.checkbox(&mut reject_else, "Reject Else").changed() {
                    bot.drops.set_reject_else(reject_else);
                }
                let mut accept_ac = bot.opts().accept_ac_drops;
                if ui.checkbox(&mut accept_ac, "Accept AC Drops").changed() {
                    bot.options.write().accept_ac_drops = accept_ac;
                }
            });

            ui.horizontal(|ui| {
                let running = bot.drops.running();
                let width = ui.available_width() - 110.0;
                if toggle_button(ui, "Pick Drops", running, width) {
                    if running {
                        crate::turu::spawn_action("Stop drops", |bot| {
                            bot.drops.stop();
                            Ok(())
                        });
                    } else {
                        bot.drops.start(&bot);
                    }
                }
                // VibeSkua's Notify Drop window (beeps on a listed drop) isn't ported.
                widgets::button(
                    ui,
                    "Notifications",
                    Kind::Normal,
                    false,
                    vec2(102.0, 28.0),
                    "Coming soon",
                );
            });
        });
    }

    fn quests_card(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let quests = bot.quests.registered_rewards();
        self.selected_quests.retain(|id| quests.iter().any(|(q, _)| q == id));
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            header(ui, "QUESTS HELPER", !self.selected_quests.is_empty(), || {
                let selected: Vec<i64> = self.selected_quests.drain().collect();
                bot.quests.unregister(&selected);
            });
            let height = (ui.available_height() - 290.0).max(140.0);
            list(
                ui,
                "runtime_quests",
                height,
                quests.is_empty(),
                "No registered quests.",
                |ui| {
                    for &(id, reward) in &quests {
                        // VibeSkua `RegisteredQuestInfo.ToString`.
                        let label = if reward == -1 {
                            id.to_string()
                        } else {
                            format!("{id} (Reward: {reward})")
                        };
                        let selected = self.selected_quests.contains(&id);
                        if ui
                            .selectable_label(selected, RichText::new(label).color(TEXT))
                            .clicked()
                        {
                            toggle(&mut self.selected_quests, id);
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
                if (add || enter)
                    && let Some(ids) = parse_quest_ids(&self.quest_input)
                {
                    let reward = self.reward_input.trim().parse::<i64>().unwrap_or(-1);
                    let quests: Vec<(i64, i64)> = ids.into_iter().map(|id| (id, reward)).collect();
                    bot.quests.register(&bot, &quests);
                    self.quest_input.clear();
                    self.reward_input.clear();
                }
            });
        });
    }

    fn boosts_card(&self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "BOOSTS HELPER");
            let searching = self.searching.load(Ordering::Acquire);
            ui.columns(2, |cols| {
                for (col, (label, bank)) in cols
                    .iter_mut()
                    .zip([("Search Inventory", false), ("Search Bank", true)])
                {
                    let width = col.available_width();
                    let tooltip = "Searching...";
                    if widgets::button(col, label, Kind::Normal, !searching, vec2(width, 26.0), tooltip).clicked() {
                        let flag = self.searching.clone();
                        flag.store(true, Ordering::Release);
                        crate::turu::spawn_action(label, move |bot| {
                            let result = boosts::set_all_ids(bot, bank);
                            flag.store(false, Ordering::Release);
                            result
                        });
                    }
                }
            });

            Frame::new()
                .fill(INSET)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for (label, boost) in [
                        ("Class Boost", BoostType::Class),
                        ("Gold Boost", BoostType::Gold),
                        ("Experience Boost", BoostType::Experience),
                        ("Reputation Boost", BoostType::Reputation),
                    ] {
                        ui.horizontal(|ui| {
                            let mut uses = bot.boosts.uses(boost);
                            if ui.checkbox(&mut uses, label).changed() {
                                bot.boosts.set_uses(boost, uses);
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("ID: {}", bot.boosts.id(boost))).color(MUTED));
                            });
                        });
                    }
                });

            let enabled = bot.boosts.enabled();
            if toggle_button(ui, "Use Boosts", enabled, ui.available_width()) {
                if enabled {
                    crate::turu::spawn_action("Stop boosts", |bot| {
                        bot.boosts.stop();
                        Ok(())
                    });
                } else {
                    bot.boosts.start(&bot);
                }
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

/// Text from the drops list: a number is an item ID, anything else a name.
fn names_or_ids(items: &[String]) -> Vec<NameOrId<'_>> {
    items
        .iter()
        .map(|item| match item.parse::<i64>() {
            Ok(id) => NameOrId::Id(id),
            Err(_) => NameOrId::Name(item),
        })
        .collect()
}

/// VibeSkua `AddQuest`: IDs separated by `|`, `,` or spaces; nothing when anything else is typed.
fn parse_quest_ids(input: &str) -> Option<Vec<i64>> {
    let parts: Vec<&str> = input
        .split(['|', ',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() || !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    parts.iter().map(|p| p.parse().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_ids_follow_vibeskua() {
        assert_eq!(parse_quest_ids("1234"), Some(vec![1234]));
        assert_eq!(parse_quest_ids(" 1, 2 | 3  4 "), Some(vec![1, 2, 3, 4]));
        assert_eq!(parse_quest_ids("12a"), None);
        assert_eq!(parse_quest_ids(" , "), None);
    }

    #[test]
    fn a_number_in_the_drop_list_is_an_id() {
        let items = vec!["Unidentified 13".to_owned(), "5000".to_owned()];
        let parsed = names_or_ids(&items);
        assert!(matches!(parsed[0], NameOrId::Name("Unidentified 13")));
        assert!(matches!(parsed[1], NameOrId::Id(5000)));
    }
}
