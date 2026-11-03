//! Tools & Helpers > Junk Items (VibeSkua `JunkItemsView` / `JunkItemsViewModel`): the inventory
//! and bank items with a check box each, marking the selected ones as junk (`JunkItems.json`),
//! and selling the junk in the inventory. The list and the sale are `turu_bot::junk`, run on
//! bot threads.

use super::theme::{DANGER, MUTED, TEXT};
use super::widgets::{self, Kind};
use egui::{
    Align, Checkbox, Color32, Context, CornerRadius, Frame, Label, Layout, Margin, Modal, RichText, ScrollArea, Sense,
    Ui, vec2,
};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use turu_bot::junk::{self, Entry};

const ROW_HEIGHT: f32 = 30.0;
const CONTROL_HEIGHT: f32 = 26.0;

/// What the bot threads hand back.
#[derive(Default)]
struct Shared {
    /// Refreshing or selling.
    busy: bool,
    /// A fresh inventory and bank list for the window to take.
    fetched: Option<Vec<Entry>>,
    /// Why the last refresh or sale failed.
    error: Option<String>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

struct Row {
    entry: Entry,
    selected: bool,
}

/// The confirmation a button asked for.
#[derive(Clone, Copy, PartialEq)]
enum Confirm {
    SellSelected(usize),
    SellAll,
    SkipWarning,
}

#[derive(Default)]
pub struct JunkItemsWindow {
    rows: Vec<Row>,
    search: String,
    /// The window was open last frame (VibeSkua refreshes when it is activated).
    was_open: bool,
    confirm: Option<Confirm>,
    shared: Arc<Mutex<Shared>>,
}

impl JunkItemsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if *open && !self.was_open {
            self.refresh();
        }
        self.was_open = *open;
        if !*open {
            return;
        }
        super::window(ctx, "Junk Items", [550.0, 500.0], 3)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        self.confirm_dialog(ctx);
        if lock(&self.shared).busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// VibeSkua `Refresh`: reads the list again, then the inventory and bank on a bot thread
    /// (loading the bank first if needed). Selections are kept.
    fn refresh(&self) {
        let shared = self.shared.clone();
        {
            let mut s = lock(&shared);
            if s.busy {
                return;
            }
            s.busy = true;
            s.error = None;
        }
        crate::turu::spawn_action("Junk Items", move |bot| {
            let result = junk::entries(bot);
            let mut s = lock(&shared);
            match result {
                Ok(entries) => s.fetched = Some(entries),
                Err(e) => s.error = Some(format!("Couldn't read the inventory: {e}")),
            }
            s.busy = false;
            Ok(())
        });
    }

    /// Takes a fresh list, keeping what was selected (by ID and category, as VibeSkua).
    fn take_fetched(&mut self) {
        let Some(entries) = lock(&self.shared).fetched.take() else {
            return;
        };
        let selected: std::collections::HashSet<(i64, String)> = self
            .rows
            .iter()
            .filter(|r| r.selected)
            .map(|r| (r.entry.id, r.entry.category.clone()))
            .collect();
        self.rows = entries
            .into_iter()
            .map(|entry| Row {
                selected: selected.contains(&(entry.id, entry.category.clone())),
                entry,
            })
            .collect();
    }

    /// VibeSkua `PersistFromEntries`: the junk rows become the list.
    fn persist(&self) {
        let path = crate::turu::bot().paths.junk_items_file();
        let items = self.rows.iter().filter(|r| r.entry.junk).map(|r| r.entry.to_junk());
        if let Err(e) = junk::set(&path, items) {
            lock(&self.shared).error = Some(format!("Couldn't save {}: {e}", path.display()));
        }
    }

    /// VibeSkua `SellAllJunk` then `Refresh`, on a bot thread.
    fn sell(&self) {
        let shared = self.shared.clone();
        {
            let mut s = lock(&shared);
            if s.busy {
                return;
            }
            s.busy = true;
            s.error = None;
        }
        crate::turu::spawn_action("Sell junk", move |bot| {
            let result = junk::sell_all(bot).and_then(|_| junk::entries(bot));
            let mut s = lock(&shared);
            match result {
                Ok(entries) => s.fetched = Some(entries),
                Err(e) => s.error = Some(format!("Selling failed: {e}")),
            }
            s.busy = false;
            Ok(())
        });
    }

    /// The rows the search shows: ID, name or category containing it, ignoring case.
    fn shown(&self) -> Vec<usize> {
        let filter = self.search.trim().to_lowercase();
        (0..self.rows.len())
            .filter(|&i| {
                let e = &self.rows[i].entry;
                filter.is_empty()
                    || e.name.to_lowercase().contains(&filter)
                    || e.id.to_string().contains(&filter)
                    || e.category.to_lowercase().contains(&filter)
            })
            .collect()
    }

    fn sell_selected(&mut self) {
        for row in self.rows.iter_mut().filter(|r| r.selected) {
            row.entry.junk = true;
        }
        self.persist();
        self.sell();
    }

    fn ui(&mut self, ui: &mut Ui) {
        self.take_fetched();
        let (busy, error) = {
            let s = lock(&self.shared);
            (s.busy, s.error.clone())
        };
        let skip_warning = crate::turu::bot().settings.read().junk_skip_sell_warning;
        let shown = self.shown();
        let selected = self.rows.iter().filter(|r| r.selected).count();
        let total_junk = self.rows.iter().filter(|r| r.entry.junk).count();

        // Search and refresh.
        ui.horizontal(|ui| {
            let width = ui.available_width() - 40.0;
            widgets::text_box(ui, &mut self.search, "Search by ID or name", width);
            if widgets::button(ui, "⟳", Kind::Normal, !busy, vec2(32.0, CONTROL_HEIGHT), "Busy")
                .on_hover_text("Refresh inventory and bank")
                .clicked()
            {
                self.refresh();
            }
        });

        // Selection bar: select all / clear, and what to do with the selection.
        ui.horizontal(|ui| {
            ui.add_space(9.0);
            let all_shown = !shown.is_empty() && shown.iter().all(|&i| self.rows[i].selected);
            let mut checked = selected > 0 && all_shown;
            let label = RichText::new(format!("{selected} selected")).strong();
            let response = ui
                .add(Checkbox::new(&mut checked, label).indeterminate(selected > 0 && !all_shown))
                .on_hover_text("Select or clear all shown items");
            if response.clicked() {
                // An unchecked box selects the shown items; a checked or partly checked one clears.
                if selected == 0 {
                    for &i in &shown {
                        self.rows[i].selected = true;
                    }
                } else {
                    self.rows.iter_mut().for_each(|r| r.selected = false);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Unmark", Kind::Normal, true, vec2(84.0, CONTROL_HEIGHT), "").clicked()
                    && !self.rows.is_empty()
                {
                    for row in self.rows.iter_mut().filter(|r| r.selected) {
                        row.entry.junk = false;
                        row.selected = false;
                    }
                    self.persist();
                }
                if widgets::button(ui, "Mark as junk", Kind::Normal, true, vec2(112.0, CONTROL_HEIGHT), "").clicked()
                    && !self.rows.is_empty()
                {
                    for row in self.rows.iter_mut().filter(|r| r.selected) {
                        row.entry.junk = true;
                    }
                    self.persist();
                }
            });
        });

        // Inventory and bank items.
        // Room for the sell row below (and the error line, when there is one).
        let below = CONTROL_HEIGHT + 26.0 + if error.is_some() { 20.0 } else { 0.0 };
        let list_height = (ui.available_height() - below).max(ROW_HEIGHT * 3.0);
        widgets::list_well(ui, list_height, |ui| self.list(ui, &shown, busy));

        if let Some(error) = error {
            ui.label(RichText::new(error).small().color(DANGER));
        }

        // Selling.
        ui.horizontal(|ui| {
            let mut skip = skip_warning;
            if ui.checkbox(&mut skip, "Skip sell warning").changed() {
                if skip {
                    self.confirm = Some(Confirm::SkipWarning);
                } else {
                    set_skip_warning(false);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = format!("Sell all junk ({total_junk})");
                if widgets::button(ui, &label, Kind::Danger, !busy, vec2(140.0, CONTROL_HEIGHT), "Busy").clicked()
                    && total_junk > 0
                {
                    if skip_warning {
                        self.sell();
                    } else {
                        self.confirm = Some(Confirm::SellAll);
                    }
                }
                if widgets::button(
                    ui,
                    "Sell selected",
                    Kind::Outline,
                    !busy,
                    vec2(112.0, CONTROL_HEIGHT),
                    "Busy",
                )
                .clicked()
                    && selected > 0
                {
                    if skip_warning {
                        self.sell_selected();
                    } else {
                        self.confirm = Some(Confirm::SellSelected(selected));
                    }
                }
                if widgets::button(ui, "Unmark all", Kind::Normal, true, vec2(112.0, CONTROL_HEIGHT), "").clicked()
                    && !self.rows.is_empty()
                {
                    for row in &mut self.rows {
                        row.entry.junk = false;
                        row.selected = false;
                    }
                    self.persist();
                }
            });
        });
    }

    fn list(&mut self, ui: &mut Ui, shown: &[usize], busy: bool) {
        if shown.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                let text = if busy {
                    "Loading inventory and bank..."
                } else if self.rows.is_empty() {
                    "No items: log in, then press ⟳."
                } else {
                    "No items match."
                };
                ui.label(RichText::new(text).color(MUTED));
            });
            return;
        }
        ScrollArea::vertical()
            .id_salt("turu_junk_items")
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT + 2.0, shown.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for &index in &shown[range] {
                    let row = &mut self.rows[index];
                    let mut checked = row.selected;
                    let response = widgets::list_row(ui, row.selected, ROW_HEIGHT, |ui| {
                        if ui.add(Checkbox::without_text(&mut checked)).changed() {
                            row.selected = checked;
                        }
                        let (id_rect, _) = ui.allocate_exact_size(vec2(52.0, ROW_HEIGHT), Sense::hover());
                        ui.painter().text(
                            id_rect.left_center(),
                            egui::Align2::LEFT_CENTER,
                            row.entry.id.to_string(),
                            egui::FontId::monospace(11.0),
                            MUTED,
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if row.entry.junk {
                                junk_chip(ui);
                            }
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.add(
                                    Label::new(RichText::new(&row.entry.name).color(TEXT))
                                        .truncate()
                                        .selectable(false),
                                )
                                .on_hover_text(&row.entry.category);
                            });
                        });
                    });
                    // A click anywhere on the row toggles its check box.
                    if response.clicked() {
                        row.selected = !row.selected;
                    }
                }
            });
    }

    fn confirm_dialog(&mut self, ctx: &Context) {
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
            ui.set_min_width(300.0);
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
                Confirm::SellSelected(_) => self.sell_selected(),
                Confirm::SellAll => self.sell(),
                Confirm::SkipWarning => set_skip_warning(true),
            }
        }
    }
}

/// "Skip sell warning", saved with the application settings (VibeSkua `JunkSkipSellWarning`).
fn set_skip_warning(on: bool) {
    let bot = crate::turu::bot();
    bot.settings.write().junk_skip_sell_warning = on;
    bot.save_settings();
}

/// The red "Junk" chip (VibeSkua `chip danger`).
fn junk_chip(ui: &mut Ui) {
    Frame::new()
        .fill(DANGER.gamma_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, DANGER.gamma_multiply(0.6)))
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::symmetric(7, 1))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Junk")
                    .size(10.0)
                    .strong()
                    .color(Color32::from_rgb(0xFC, 0xA5, 0xA5)),
            );
        });
}
