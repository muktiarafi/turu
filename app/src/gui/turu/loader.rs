//! Tools & Helpers > Loader (VibeSkua `LoaderView`): loads a shop or quests by ID, and lists
//! the quest cache (`QuestData.json`, `turu_bot::quest_cache`) to load, copy or fake complete
//! quests from. The cache is read and updated on bot threads; the list keeps IDs and names.

use super::theme::{MUTED, TEXT, WARNING};
use super::widgets::{self, Kind, Selection};
use egui::{Align, Context, Label, Layout, RichText, ScrollArea, Ui, vec2};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use turu_bot::api::{player, quests, shops};
use turu_bot::quest_cache::{self, QuestSummary, UpdateMode};

const ROW_HEIGHT: f32 = 22.0;
const CONTROL_HEIGHT: f32 = 26.0;

/// What the bot threads hand back to the window.
#[derive(Default)]
struct Shared {
    /// Reading or updating the cache, or fake completing a quest.
    busy: bool,
    /// The cache update can be cancelled.
    cancellable: bool,
    /// VibeSkua's progress text while busy.
    progress: String,
    /// The cache as read or updated, for the window to take.
    quests: Option<Arc<Vec<QuestSummary>>>,
    /// A message shown for a few seconds (a bad range, a failure).
    notice: Option<(String, Instant)>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

fn notify(shared: &Mutex<Shared>, text: impl Into<String>, seconds: u64) {
    lock(shared).notice = Some((text.into(), Instant::now() + Duration::from_secs(seconds)));
}

#[derive(Default)]
pub struct LoaderWindow {
    /// The Quests segment is picked (else Shop).
    quests_mode: bool,
    ids: String,
    search: String,
    range: String,
    quests: Arc<Vec<QuestSummary>>,
    /// Indices into `quests` that match the search, and what they were made for.
    shown: Vec<usize>,
    shown_for: Option<(String, usize)>,
    selection: Selection,
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
    read_started: bool,
}

impl LoaderWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        if !self.read_started {
            self.read_started = true;
            self.read_cache();
        }
        super::window(ctx, "Loader", [720.0, 520.0], 3)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        if lock(&self.shared).busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// VibeSkua `GetQuestsAsync`: the cache file's quests, when the window opens.
    fn read_cache(&self) {
        let shared = self.shared.clone();
        {
            let mut s = lock(&shared);
            s.busy = true;
            s.progress = "Getting quests".into();
        }
        crate::turu::spawn_action("Read the quest cache", move |bot| {
            match quest_cache::read_summaries(&bot.paths.quest_data_file()) {
                Ok(quests) => lock(&shared).quests = Some(Arc::new(quests)),
                Err(e) => notify(&shared, format!("Failed to load quests: {e}"), 6),
            }
            let mut s = lock(&shared);
            s.busy = false;
            s.progress.clear();
            Ok(())
        });
    }

    /// VibeSkua `UpdateQuests(getAll)` / `UpdateQuestsRange`.
    fn update_cache(&mut self, mode: UpdateMode) {
        self.cancel = Arc::default();
        let (shared, cancel) = (self.shared.clone(), self.cancel.clone());
        {
            let mut s = lock(&shared);
            s.busy = true;
            s.cancellable = true;
            s.progress.clear();
        }
        crate::turu::spawn_action("Update the quest cache", move |bot| {
            let path = bot.paths.quest_data_file();
            if !player::logged_in(bot).unwrap_or(false) {
                notify(&shared, "Log in to update the quest cache.", 4);
            }
            let progress = |text: String| lock(&shared).progress = text;
            match quest_cache::update(bot, &path, mode, &progress, &cancel) {
                Ok(quests) => {
                    let quests = quests.iter().map(QuestSummary::from).collect();
                    lock(&shared).quests = Some(Arc::new(quests));
                }
                Err(e) => notify(&shared, format!("Failed to update the quest cache: {e}"), 6),
            }
            let mut s = lock(&shared);
            s.busy = false;
            s.cancellable = false;
            s.progress.clear();
            Ok(())
        });
    }

    /// VibeSkua `UpdateSelectedQuest`: a client-side quest-complete packet.
    fn fake_complete(&self, quest: QuestSummary) {
        let shared = self.shared.clone();
        {
            let mut s = lock(&shared);
            s.busy = true;
            s.progress = format!("Fake completing quest: {}.", quest.name);
        }
        crate::turu::spawn_action("Fake complete", move |bot| {
            match quests::update_quest(bot, quest.id) {
                Ok(true) => {}
                Ok(false) => notify(&shared, format!("Failed to update quest: {} didn't load.", quest.id), 4),
                Err(e) => notify(&shared, format!("Failed to update quest: {e}"), 4),
            }
            let mut s = lock(&shared);
            s.busy = false;
            s.progress.clear();
            Ok(())
        });
    }

    /// Takes a list the bot threads read, and matches the search again when needed.
    fn refresh(&mut self) {
        if let Some(quests) = lock(&self.shared).quests.take() {
            self.quests = quests;
            self.selection.clear();
        }
        let key = (self.search.clone(), Arc::as_ptr(&self.quests) as usize);
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

    fn ui(&mut self, ui: &mut Ui) {
        self.refresh();
        self.load_bar(ui);
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
        widgets::list_well(ui, height, |ui| self.quest_list(ui));
        self.cache_bar(ui);
        self.selection_bar(ui);
    }

    /// Shop / Quests, the IDs box and Load.
    fn load_bar(&mut self, ui: &mut Ui) {
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
                if let Some(id) = quest_cache::parse_shop_id(&self.ids) {
                    crate::turu::spawn_action("Load shop", move |bot| shops::load(bot, id).map(|_| ()));
                }
            } else if let Some(ids) = ids {
                crate::turu::spawn_action("Load quests", move |bot| quests::load(bot, &ids));
            }
        });
    }

    fn quest_list(&mut self, ui: &mut Ui) {
        if self.shown.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                let text = if lock(&self.shared).busy && self.quests.is_empty() {
                    "Getting quests..."
                } else if self.quests.is_empty() {
                    "The quest cache is empty. Log in and use Add missing to fill it."
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

    /// Update cache (Add missing, Rebuild all) or the progress with Cancel, and Update range.
    fn cache_bar(&mut self, ui: &mut Ui) {
        let (busy, cancellable, progress, notice) = {
            let mut s = lock(&self.shared);
            if s.notice.as_ref().is_some_and(|(_, until)| Instant::now() >= *until) {
                s.notice = None;
            }
            (
                s.busy,
                s.cancellable,
                s.progress.clone(),
                s.notice.as_ref().map(|(text, _)| text.clone()),
            )
        };
        if notice.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
        widgets::card(ui, |ui| {
            ui.set_min_height(CONTROL_HEIGHT);
            ui.horizontal(|ui| {
                let small = vec2(0.0, CONTROL_HEIGHT);
                if busy {
                    ui.label(RichText::new(&progress).size(11.0).strong().color(MUTED));
                    if cancellable
                        && widgets::button(ui, "Cancel", Kind::Danger, true, vec2(84.0, CONTROL_HEIGHT), "").clicked()
                    {
                        self.cancel.store(true, Ordering::Release);
                        lock(&self.shared).progress = "Cancelling task...".into();
                    }
                } else {
                    match &notice {
                        Some(text) => ui.label(RichText::new(text).size(11.0).color(WARNING)),
                        None => ui.label(RichText::new("Update cache").size(11.0).color(MUTED)),
                    };
                    if widgets::button(ui, "+ Add missing", Kind::Normal, true, vec2(112.0, CONTROL_HEIGHT), "")
                        .on_hover_text("Keep the quests already cached and add new ones")
                        .clicked()
                    {
                        self.update_cache(UpdateMode::AddMissing);
                    }
                    if widgets::button(ui, "Rebuild all", Kind::Normal, true, small, "")
                        .on_hover_text("Replace every quest in the cache")
                        .clicked()
                    {
                        self.update_cache(UpdateMode::RebuildAll);
                    }
                }
                ui.separator();
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
                        match quest_cache::parse_range(&self.range) {
                            Ok((start, end)) => self.update_cache(UpdateMode::Range(start, end)),
                            Err(message) => notify(&self.shared, message, 2),
                        }
                    }
                });
            });
        });
    }

    /// The selection count, Copy, Fake complete and Load selected.
    fn selection_bar(&self, ui: &mut Ui) {
        let busy = lock(&self.shared).busy;
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
                    crate::turu::spawn_action("Load quests", move |bot| quests::load(bot, &ids));
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
                    self.fake_complete(quest);
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
