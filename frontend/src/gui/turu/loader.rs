//! Tools & Helpers > Loader (VibeSkua `LoaderView`): loads a shop or quests by ID, and lists
//! the quest cache to load, copy or fake complete quests from. The cache and its work are
//! `models::loader`'s; the window keeps its search and selection.

use super::theme::{MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind, Selection};
use crate::models::loader::{self as model, LoaderStatus};
use egui::{Align, Context, Label, Layout, RichText, ScrollArea, Ui, vec2};
use std::sync::Arc;
use std::time::Duration;
use turu_bot::Bot;
use turu_bot::quest_cache::{self, QuestSummary, UpdateMode};

const ROW_HEIGHT: f32 = 22.0;
const CONTROL_HEIGHT: f32 = 26.0;

#[derive(Default)]
pub struct LoaderWindow {
    /// The Quests segment is picked (else Shop).
    quests_mode: bool,
    ids: String,
    search: String,
    range: String,
    quests: Arc<Vec<QuestSummary>>,
    revision: Option<u64>,
    /// Indices into `quests` that match the search, and what they were made for.
    shown: Vec<usize>,
    shown_for: Option<(String, u64)>,
    selection: Selection,
}

impl LoaderWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        let bot = crate::turu::bot();
        model::open(&bot);
        let status = model::status(&bot);
        super::window(ctx, "Loader", [720.0, 520.0], 3)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &bot, &status));
        if status.busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if status.notice.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    /// Takes the list as last read, and matches the search again when needed.
    fn refresh(&mut self, bot: &Arc<Bot>) {
        let (quests, revision) = model::quests(bot);
        if self.revision != Some(revision) {
            self.quests = quests;
            self.revision = Some(revision);
            self.selection.clear();
        }
        let key = (self.search.clone(), revision);
        if self.shown_for.as_ref() != Some(&key) {
            self.shown = quest_cache::search(&self.quests, &self.search);
            self.shown_for = Some(key);
        }
    }

    fn selected(&self) -> Vec<&QuestSummary> {
        self.selection
            .picked()
            .into_iter()
            .filter_map(|i| self.quests.get(i))
            .collect()
    }

    fn ui(&mut self, ui: &mut Ui, bot: &Arc<Bot>, status: &LoaderStatus) {
        self.refresh(bot);
        self.load_bar(ui, bot);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            widgets::section_label(ui, "QUEST CACHE");
            widgets::count_badge(ui, self.shown.len());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::text_box(ui, &mut self.search, "Search by name", 240.0);
            });
        });
        let footer = 46.0 + CONTROL_HEIGHT + 3.0 * ui.spacing().item_spacing.y + 4.0;
        let height = (ui.available_height() - footer).max(80.0);
        widgets::list_well(ui, height, |ui| self.quest_list(ui, status.busy));
        self.cache_bar(ui, bot, status);
        self.selection_bar(ui, bot, status.busy);
    }

    /// Shop / Quests, the IDs box and Load.
    fn load_bar(&mut self, ui: &mut Ui, bot: &Arc<Bot>) {
        ui.horizontal(|ui| {
            for (label, quests) in [("Shop", false), ("Quests", true)] {
                if widgets::chip(ui, label, self.quests_mode == quests) {
                    self.quests_mode = quests;
                }
            }
            let width = ui.available_width() - 84.0 - ui.spacing().item_spacing.x;
            let response = widgets::text_box(ui, &mut self.ids, "IDs, e.g. 2566 or 101, 102", width);
            let ids = quest_cache::parse_ids(&self.ids);
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let tip = "Enter IDs: digits, commas and spaces";
            let clicked = widgets::button(
                ui,
                "Load",
                Kind::Primary,
                ids.is_some(),
                vec2(84.0, CONTROL_HEIGHT),
                tip,
            )
            .clicked();
            if !(clicked || enter && ids.is_some()) {
                return;
            }
            if !self.quests_mode {
                // VibeSkua loads a shop only for a single ID.
                let _ = model::load_shop(bot, &self.ids);
            } else if let Some(ids) = ids {
                model::load_quests(bot, ids);
            }
        });
    }

    fn quest_list(&mut self, ui: &mut Ui, busy: bool) {
        if self.shown.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                let text = if busy && self.quests.is_empty() {
                    "Getting quests..."
                } else if self.quests.is_empty() {
                    "The quest cache is empty: use Download, or log in and use Add missing."
                } else {
                    "No quests match."
                };
                ui.label(RichText::new(text).color(MUTED));
            });
            return;
        }
        let shift = ui.input(|i| i.modifiers.shift);
        let mut clicked = None;
        ScrollArea::vertical()
            .id_salt("turu_loader_quests")
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, self.shown.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for &index in &self.shown[range] {
                    let quest = &self.quests[index];
                    let response = widgets::list_row(ui, self.selection.contains(index), ROW_HEIGHT, |ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(quest.id.to_string()).monospace().size(11.0).color(MUTED));
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.add(
                                    Label::new(RichText::new(&quest.name).color(TEXT))
                                        .truncate()
                                        .selectable(false),
                                );
                            });
                        });
                    });
                    if response.clicked() {
                        clicked = Some(index);
                    }
                }
            });
        if let Some(index) = clicked {
            self.selection.click(index, &self.shown, true, shift);
        }
    }

    /// Update cache (Add missing, Rebuild all, Download) or the progress with Cancel, and Update
    /// range.
    fn cache_bar(&mut self, ui: &mut Ui, bot: &Arc<Bot>, status: &LoaderStatus) {
        let (busy, cancellable, progress) = (status.busy, status.cancellable, &status.progress);
        let notice = status.notice.as_ref().map(|n| (n.text.clone(), n.ok));
        // On a phone the range gets a row of its own: beside the buttons it has no room.
        let narrow = ui.available_width() < 640.0;
        widgets::card(ui, |ui| {
            ui.set_min_height(CONTROL_HEIGHT);
            let body = |ui: &mut Ui| {
                let small = vec2(0.0, CONTROL_HEIGHT);
                if busy {
                    ui.label(RichText::new(progress).size(11.0).strong().color(MUTED));
                    if cancellable
                        && widgets::button(ui, "Cancel", Kind::Danger, true, vec2(84.0, CONTROL_HEIGHT), "").clicked()
                    {
                        model::cancel(bot);
                    }
                } else {
                    match &notice {
                        Some((text, ok)) => {
                            let color = if *ok { SUCCESS } else { WARNING };
                            ui.scope(|ui| {
                                ui.set_max_width(170.0);
                                ui.add(Label::new(RichText::new(text).size(11.0).color(color)).truncate())
                                    .on_hover_text(text);
                            });
                        }
                        None => {
                            ui.label(RichText::new("Update cache").size(11.0).color(MUTED));
                        }
                    }
                    if widgets::button(ui, "+ Add missing", Kind::Normal, true, vec2(112.0, CONTROL_HEIGHT), "")
                        .on_hover_text("Keep the quests already cached and add new ones")
                        .clicked()
                    {
                        model::update_cache(bot, UpdateMode::AddMissing);
                    }
                    if widgets::button(ui, "Rebuild all", Kind::Normal, true, small, "")
                        .on_hover_text("Replace every quest in the cache")
                        .clicked()
                    {
                        model::update_cache(bot, UpdateMode::RebuildAll);
                    }
                    if widgets::button(ui, "Download", Kind::Normal, true, small, "")
                        .on_hover_text(
                            "Download the community quest cache (auqw/Scripts) and merge it in, \
                             keeping the quests only yours has",
                        )
                        .clicked()
                    {
                        model::download_cache(bot);
                    }
                }
                if !narrow {
                    ui.separator();
                    self.range_row(ui, bot, busy);
                }
            };
            if narrow {
                ui.horizontal_wrapped(body);
            } else {
                ui.horizontal(body);
            }
            if narrow {
                ui.add_space(4.0);
                ui.horizontal(|ui| self.range_row(ui, bot, busy));
            }
        });
    }

    /// The range box and Update range, right-aligned.
    fn range_row(&mut self, ui: &mut Ui, bot: &Arc<Bot>, busy: bool) {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let clicked = widgets::button(
                ui,
                "Update range",
                Kind::Normal,
                !busy,
                vec2(112.0, CONTROL_HEIGHT),
                "Busy",
            )
            .clicked();
            let width = ui.available_width();
            widgets::text_box(ui, &mut self.range, "Range, e.g. 2566, 2599", width);
            if clicked {
                model::update_range(bot, &self.range);
            }
        });
    }

    /// The selection count, Copy, Fake complete and Load selected.
    fn selection_bar(&self, ui: &mut Ui, bot: &Arc<Bot>, busy: bool) {
        ui.horizontal(|ui| {
            let count = self.selection.len();
            let text = if count == 0 {
                "None selected".to_owned()
            } else {
                format!("{count} selected")
            };
            ui.label(RichText::new(text).size(11.0).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let some = count > 0;
                let size = vec2(140.0, CONTROL_HEIGHT);
                if widgets::button(ui, "Load selected", Kind::Primary, some, size, "Select quests first")
                    .on_hover_text("Load the selected quests in game")
                    .clicked()
                {
                    let ids: Vec<i64> = self.selected().iter().map(|q| q.id).collect();
                    model::load_quests(bot, ids);
                }
                // VibeSkua fakes the quest clicked last.
                let current = self.selection.current().and_then(|i| self.quests.get(i)).cloned();
                let tip = if busy { "Busy" } else { "Select a quest first" };
                let enabled = current.is_some() && !busy;
                if widgets::button(
                    ui,
                    "Fake complete",
                    Kind::Normal,
                    enabled,
                    vec2(112.0, CONTROL_HEIGHT),
                    tip,
                )
                .on_hover_text("Send a quest-complete packet to the client")
                .clicked()
                    && let Some(quest) = current
                {
                    model::fake_complete(bot, quest.id);
                }
                ui.add_enabled_ui(some, |ui| {
                    ui.spacing_mut().interact_size.y = CONTROL_HEIGHT;
                    ui.menu_button("Copy ▾", |ui| {
                        let selected = self.selected();
                        if ui.button("Copy IDs").clicked() {
                            ui.ctx().copy_text(quest_cache::copy_ids(&selected));
                        }
                        if ui.button("Copy names").clicked() {
                            ui.ctx().copy_text(quest_cache::copy_names(&selected));
                        }
                        if ui.button("Copy as // Name | ID").clicked() {
                            ui.ctx().copy_text(quest_cache::copy_comments(&selected));
                        }
                    })
                    .response
                    .on_hover_text("Copy the selected quests");
                });
            });
        });
    }
}
