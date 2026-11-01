//! Tools & Helpers > Grabber (VibeSkua `GrabberView` / `GrabberListView`): a sidebar of lists
//! (shops, quests, items, monsters); each grabs its game data on a bot thread into a
//! searchable list with the selected row's details, Copy / Export, and its own actions (Buy,
//! Accept, Equip, Kill, ...). The readers and actions are `turu_bot::grabber`.

use super::files::SaveText;
use super::theme::{ACCENT, CARD, EDGE, HOVER, MUTED, TEXT};
use super::widgets::{self, Kind, Selection};
use crate::gui::FilePicker;
use egui::{
    Align, Context, CornerRadius, Label, Layout, Modal, Rect, RichText, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2,
    vec2,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use turu_bot::grabber::{self, GrabAction, GrabKind, Grabbed, Prepared};

const ROW_HEIGHT: f32 = 22.0;
const CONTROL_HEIGHT: f32 = 26.0;
const SIDEBAR_WIDTH: f32 = 164.0;
const DETAILS_WIDTH: f32 = 232.0;

/// What a list's bot threads hand back.
#[derive(Default)]
struct Shared {
    /// Grabbing or running an action.
    busy: bool,
    /// The status text, and when it goes away (never while busy).
    message: Option<(String, Option<Instant>)>,
    /// A fresh grab for the list to take.
    grabbed: Option<Vec<Grabbed>>,
    /// A message box an action asked for (title, text).
    notice: Option<(String, String)>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// Shows `text` as the status for `seconds` (0: until replaced).
fn report(shared: &Mutex<Shared>, text: impl Into<String>, seconds: u64) {
    let until = (seconds > 0).then(|| Instant::now() + Duration::from_secs(seconds));
    lock(shared).message = Some((text.into(), until));
}

/// One sidebar list (VibeSkua `GrabberListViewModel`).
struct List {
    kind: GrabKind,
    items: Vec<Grabbed>,
    search: String,
    multiple: bool,
    selection: Selection,
    shown: Vec<usize>,
    shown_for: Option<String>,
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
}

impl List {
    fn new(kind: GrabKind) -> Self {
        Self {
            kind,
            items: Vec::new(),
            search: String::new(),
            multiple: true,
            selection: Selection::default(),
            shown: Vec::new(),
            shown_for: None,
            shared: Arc::default(),
            cancel: Arc::default(),
        }
    }

    /// Takes a fresh grab, and filters again when the search changed (VibeSkua `ApplyFilter`:
    /// the summary contains the search, ignoring case).
    fn refresh(&mut self) {
        if let Some(items) = lock(&self.shared).grabbed.take() {
            self.items = items;
            self.selection.clear();
            self.shown_for = None;
        }
        if self.shown_for.as_deref() != Some(self.search.as_str()) {
            let query = self.search.trim().to_lowercase();
            self.shown = (0..self.items.len())
                .filter(|&i| query.is_empty() || self.items[i].summary().to_lowercase().contains(&query))
                .collect();
            self.shown_for = Some(self.search.clone());
        }
    }

    fn selected(&self) -> Vec<Grabbed> {
        self.selection
            .picked()
            .into_iter()
            .filter_map(|i| self.items.get(i).cloned())
            .collect()
    }

    /// The selected rows, or every shown row when none is selected (Copy, Export).
    fn targets(&self) -> Vec<&Grabbed> {
        if self.selection.is_empty() {
            self.shown.iter().map(|&i| &self.items[i]).collect()
        } else {
            self.selection
                .picked()
                .into_iter()
                .filter_map(|i| self.items.get(i))
                .collect()
        }
    }

    /// VibeSkua `Grab`.
    fn grab(&self) {
        let (shared, kind) = (self.shared.clone(), self.kind);
        {
            let mut s = lock(&shared);
            s.busy = true;
            s.message = Some(("Working...".into(), None));
        }
        crate::turu::spawn_action("Grab", move |bot| {
            match grabber::grab(bot, kind) {
                Ok(items) => {
                    lock(&shared).grabbed = Some(items);
                    report(&shared, "Finished.", 1);
                }
                Err(e) => report(&shared, format!("Grab failed: {e}"), 4),
            }
            lock(&shared).busy = false;
            Ok(())
        });
    }

    /// VibeSkua `GrabberTask`: runs the action on the selected rows.
    fn run(&mut self, action: GrabAction, selected: Vec<Grabbed>, quantity: Option<i64>) {
        self.cancel = Arc::default();
        let (shared, cancel) = (self.shared.clone(), self.cancel.clone());
        lock(&shared).busy = true;
        crate::turu::spawn_action(action.label(), move |bot| {
            let refs: Vec<&Grabbed> = selected.iter().collect();
            let progress = |text: String| report(&shared, text, 0);
            match grabber::run(bot, action, &refs, quantity, &progress, &cancel) {
                Ok(notice) => lock(&shared).notice = notice,
                Err(_) if cancel.load(Ordering::Acquire) => progress("Task cancelled.".into()),
                Err(e) => progress(format!("Operation failed: {e}")),
            }
            let mut s = lock(&shared);
            s.busy = false;
            // VibeSkua clears the status a second after the task.
            if let Some((_, until)) = s.message.as_mut() {
                *until = Some(Instant::now() + Duration::from_secs(1));
            }
            Ok(())
        });
    }
}

/// The quantity box an action asks for first.
struct Ask {
    kind: GrabKind,
    action: GrabAction,
    selected: Vec<Grabbed>,
    title: String,
    prompt: String,
    text: String,
}

pub struct GrabberWindow {
    tab: GrabKind,
    lists: HashMap<GrabKind, List>,
    ask: Option<Ask>,
    /// A message box (title, text).
    notice: Option<(String, String)>,
    /// Export's text, waiting for the save dialog.
    export: Option<(String, String)>,
    save: SaveText,
}

impl Default for GrabberWindow {
    fn default() -> Self {
        Self {
            tab: GrabKind::ShopItems,
            lists: GrabKind::ALL.into_iter().map(|k| (k, List::new(k))).collect(),
            ask: None,
            notice: None,
            export: None,
            save: SaveText::default(),
        }
    }
}

impl GrabberWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Grabber", [800.0, 560.0], 5)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        self.modals(ctx);
        let list = &self.lists[&self.tab];
        let s = lock(&list.shared);
        if s.busy || s.message.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    /// Opens the save dialog for an Export (needs the file picker, which the window doesn't).
    pub fn save_export(&mut self, picker: &FilePicker, ctx: &Context) {
        self.save.poll();
        if let Some((file_name, text)) = self.export.take() {
            self.save.ask(picker, ctx, &file_name, text);
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        let gap = 14.0;
        let side = Rect::from_min_max(rect.min, pos2(rect.left() + SIDEBAR_WIDTH, rect.bottom()));
        let main = Rect::from_min_max(pos2(side.right() + gap, rect.top()), rect.max);
        let mut sidebar = ui.new_child(UiBuilder::new().id_salt("side").max_rect(side));
        self.sidebar(&mut sidebar);
        let mut content = ui.new_child(UiBuilder::new().id_salt("main").max_rect(main));
        self.list_ui(&mut content);
        ui.allocate_rect(rect, Sense::hover());
    }

    /// The grouped sidebar: a heading before each group's first list.
    fn sidebar(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .id_salt("turu_grabber_nav")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let mut group = "";
                for kind in GrabKind::ALL {
                    if kind.group() != group {
                        group = kind.group();
                        ui.add_space(if kind == GrabKind::ALL[0] { 0.0 } else { 4.0 });
                        widgets::section_label(ui, group);
                    }
                    if nav_item(ui, kind.title(), self.tab == kind, kind.available()) {
                        self.tab = kind;
                    }
                }
            });
    }

    fn list_ui(&mut self, ui: &mut Ui) {
        let fights_blocked = super::busy(&crate::turu::bot());
        let list = self.lists.get_mut(&self.tab).expect("every kind has a list");
        list.refresh();
        let (busy, message) = {
            let mut s = lock(&list.shared);
            if s.message
                .as_ref()
                .is_some_and(|(_, until)| until.is_some_and(|u| Instant::now() >= u))
            {
                s.message = None;
            }
            if let Some(notice) = s.notice.take() {
                self.notice = Some(notice);
            }
            (s.busy, s.message.as_ref().map(|(text, _)| text.clone()))
        };

        // Grab, search, Multi-select.
        ui.horizontal(|ui| {
            let available = list.kind.available();
            let tip = if available {
                "Busy"
            } else {
                "Needs the map SWF decompiled (FFDec): not in Turu"
            };
            if widgets::button(
                ui,
                "⟳ Grab",
                Kind::Primary,
                !busy && available,
                vec2(112.0, CONTROL_HEIGHT),
                tip,
            )
            .clicked()
            {
                list.grab();
            }
            let chip = if list.kind.select_multiple() { 120.0 } else { 0.0 };
            let width = ui.available_width() - chip;
            widgets::text_box(ui, &mut list.search, "Search", width);
            if list.kind.select_multiple() && widgets::chip(ui, "Multi-select", list.multiple) {
                list.multiple = !list.multiple;
                if !list.multiple {
                    list.selection.keep_one();
                }
            }
        });
        ui.add_space(4.0);

        // ITEMS (count, status, Cancel, Unselect / Copy / Export) and DETAILS.
        let actions = list.kind.actions();
        let footer = if actions.is_empty() {
            0.0
        } else {
            CONTROL_HEIGHT + ui.spacing().item_spacing.y + 4.0
        };
        let body = ui.available_rect_before_wrap();
        let body = Rect::from_min_max(
            body.min,
            pos2(body.right(), (body.bottom() - footer).max(body.top() + 120.0)),
        );
        let details_width = DETAILS_WIDTH.min(body.width() * 0.4);
        let left = Rect::from_min_max(body.min, pos2(body.right() - details_width - 10.0, body.bottom()));
        let right = Rect::from_min_max(pos2(body.right() - details_width, body.top()), body.max);
        {
            let ui = &mut ui.new_child(UiBuilder::new().id_salt("items").max_rect(left));
            ui.horizontal(|ui| {
                ui.set_min_height(CONTROL_HEIGHT);
                widgets::section_label(ui, "ITEMS");
                widgets::count_badge(ui, list.shown.len());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if widgets::small_button(ui, "Export", true)
                            .on_hover_text("Export everything grabbed for the selected items (or all) to a .txt file")
                            .clicked()
                        {
                            let targets = list.targets();
                            if targets.is_empty() {
                                report(&list.shared, "No items to export.", 3);
                            } else {
                                let text = grabber::detailed_export_text(list.kind.title(), &targets);
                                let file = format!("{}.txt", list.kind.title().replace(' ', ""));
                                report(&list.shared, format!("Exported {} item(s) to file.", targets.len()), 3);
                                self.export = Some((file, text));
                            }
                        }
                        if widgets::small_button(ui, "Copy", true)
                            .on_hover_text("Copy the selected items (or all) to the clipboard")
                            .clicked()
                        {
                            let targets = list.targets();
                            if targets.is_empty() {
                                report(&list.shared, "No items to copy.", 3);
                            } else {
                                ui.ctx().copy_text(grabber::export_text(list.kind.title(), &targets));
                                report(
                                    &list.shared,
                                    format!("Copied {} item(s) to clipboard.", targets.len()),
                                    3,
                                );
                            }
                        }
                        if widgets::small_button(ui, "Unselect", true)
                            .on_hover_text("Unselect all")
                            .clicked()
                        {
                            list.selection.clear();
                        }
                    });
                    if busy && widgets::button(ui, "Cancel", Kind::Danger, true, vec2(64.0, 22.0), "").clicked() {
                        list.cancel.store(true, Ordering::Release);
                    }
                    match &message {
                        Some(text) => {
                            ui.label(RichText::new(text).size(11.0).strong().color(MUTED));
                        }
                        None if !list.selection.is_empty() => {
                            ui.label(
                                RichText::new(format!("{} selected", list.selection.len()))
                                    .size(11.0)
                                    .color(MUTED),
                            );
                        }
                        None => {}
                    }
                });
            });
            let height = ui.available_height().max(60.0);
            widgets::list_well(ui, height, |ui| item_list(ui, list));
        }
        {
            let ui = &mut ui.new_child(UiBuilder::new().id_salt("details").max_rect(right));
            ui.horizontal(|ui| {
                ui.set_min_height(CONTROL_HEIGHT);
                widgets::section_label(ui, "DETAILS");
            });
            let current = list
                .selection
                .current()
                .or_else(|| list.selection.picked().first().copied());
            details(ui, current.and_then(|i| list.items.get(i)));
        }
        ui.allocate_rect(body, Sense::hover());

        // The list's own actions, right-aligned.
        if actions.is_empty() {
            return;
        }
        ui.add_space(4.0);
        let mut clicked = None;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for &action in actions.iter().rev() {
                let kind = if action.destructive() {
                    Kind::Danger
                } else {
                    Kind::Normal
                };
                let blocked = action.fights() && fights_blocked;
                let tip = if blocked {
                    "Stop the script, Console snippet or Auto Attack first"
                } else {
                    "Busy"
                };
                if widgets::button(
                    ui,
                    action.label(),
                    kind,
                    !busy && !blocked,
                    vec2(84.0, CONTROL_HEIGHT),
                    tip,
                )
                .clicked()
                {
                    clicked = Some(action);
                }
            }
        });
        if let Some(action) = clicked {
            let selected = list.selected();
            let refs: Vec<&Grabbed> = selected.iter().collect();
            match grabber::prepare(action, &refs) {
                Prepared::Run => list.run(action, selected, None),
                Prepared::AskQuantity { title, prompt } => {
                    report(&list.shared, format!("{title}, input quantity..."), 0);
                    self.ask = Some(Ask {
                        kind: list.kind,
                        action,
                        selected,
                        title,
                        prompt,
                        text: String::new(),
                    });
                }
                Prepared::Warn {
                    title,
                    message,
                    progress,
                } => {
                    if !progress.is_empty() {
                        report(&list.shared, progress, 2);
                    }
                    self.notice = Some((title, message));
                }
                Prepared::Report(text) => report(&list.shared, text, 2),
            }
        }
    }

    /// The quantity box (VibeSkua's input dialog) and the message box.
    fn modals(&mut self, ctx: &Context) {
        if let Some(ask) = &mut self.ask {
            let mut answer = None;
            let response = Modal::new(egui::Id::new("turu_grabber_quantity")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading(&ask.title);
                ui.label(&ask.prompt);
                let edit = widgets::text_box(ui, &mut ask.text, "", ui.available_width());
                edit.request_focus();
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, "OK", Kind::Primary, true, vec2(72.0, 28.0), "").clicked() || enter {
                        answer = Some(true);
                    }
                    if widgets::button(ui, "Cancel", Kind::Normal, true, vec2(72.0, 28.0), "").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if response.should_close() && answer.is_none() {
                answer = Some(false);
            }
            if let Some(ok) = answer {
                let ask = self.ask.take().expect("shown");
                let list = self.lists.get_mut(&ask.kind).expect("every kind has a list");
                match ok.then(|| ask.text.trim().parse::<i64>().ok()) {
                    Some(Some(quantity)) => list.run(ask.action, ask.selected, Some(quantity)),
                    // VibeSkua does nothing for a quantity that isn't a number.
                    Some(None) => lock(&list.shared).message = None,
                    None => report(&list.shared, "Cancelled.", 1),
                }
            }
        }
        if let Some((title, message)) = &self.notice {
            let mut close = false;
            let response = Modal::new(egui::Id::new("turu_grabber_notice")).show(ctx, |ui| {
                ui.set_min_width(280.0);
                ui.set_max_width(420.0);
                ui.heading(title);
                ui.label(message);
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = widgets::button(ui, "OK", Kind::Primary, true, vec2(72.0, 28.0), "").clicked();
                });
            });
            if close || response.should_close() {
                self.notice = None;
            }
        }
    }
}

/// A sidebar entry: a boxed row, accent-tinted when picked; greyed when not available.
fn nav_item(ui: &mut Ui, title: &str, selected: bool, available: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let (fill, stroke) = if selected {
        (ACCENT.gamma_multiply(0.25), ACCENT)
    } else if response.hovered() && available {
        (HOVER, EDGE)
    } else {
        (CARD, EDGE)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, stroke),
        egui::StrokeKind::Inside,
    );
    let color = if !available {
        MUTED.gamma_multiply(0.6)
    } else if selected {
        TEXT
    } else {
        MUTED
    };
    let font = egui::FontId::proportional(13.0);
    ui.painter().text(
        rect.left_center() + vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        font,
        color,
    );
    if !available {
        response.on_hover_text("Needs the map SWF decompiled (FFDec): not in Turu");
        return false;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

fn item_list(ui: &mut Ui, list: &mut List) {
    if list.shown.is_empty() {
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            let text = if list.items.is_empty() {
                "Nothing grabbed yet: press Grab."
            } else {
                "No items match."
            };
            ui.label(RichText::new(text).color(MUTED));
        });
        return;
    }
    let shift = ui.input(|i| i.modifiers.shift);
    let multiple = list.multiple && list.kind.select_multiple();
    let mut clicked = None;
    ScrollArea::vertical()
        .id_salt(("turu_grabber_items", list.kind.title()))
        .auto_shrink([false, false])
        .show_rows(ui, ROW_HEIGHT, list.shown.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for &index in &list.shown[range] {
                let item = &list.items[index];
                let response = widgets::list_row(ui, list.selection.contains(index), ROW_HEIGHT, |ui| {
                    let (id_rect, _) = ui.allocate_exact_size(vec2(52.0, ROW_HEIGHT), Sense::hover());
                    ui.painter().text(
                        id_rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        item.id().to_string(),
                        egui::FontId::monospace(11.0),
                        MUTED,
                    );
                    ui.add(
                        Label::new(RichText::new(item.row()).color(TEXT))
                            .truncate()
                            .selectable(false),
                    );
                });
                if response.clicked() {
                    clicked = Some(index);
                }
            }
        });
    if let Some(index) = clicked {
        list.selection.click(index, &list.shown, multiple, shift);
    }
}

/// The DETAILS card: the row's fields and values (VibeSkua's property grid).
fn details(ui: &mut Ui, item: Option<&Grabbed>) {
    let height = ui.available_height();
    widgets::card(ui, |ui| {
        ui.set_min_size(vec2(ui.available_width(), (height - 22.0).max(40.0)));
        let Some(item) = item else {
            ui.label(
                RichText::new("Select an item to see its fields.")
                    .size(11.0)
                    .color(MUTED),
            );
            return;
        };
        ScrollArea::vertical()
            .id_salt("turu_grabber_details")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (key, value) in item.details() {
                    widgets::section_label(ui, &key);
                    ui.add(Label::new(RichText::new(value).monospace().size(11.0).color(TEXT)).wrap());
                    ui.add_space(3.0);
                }
            });
    });
}
