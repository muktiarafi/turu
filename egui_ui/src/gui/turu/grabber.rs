//! Tools & Helpers > Grabber (VibeSkua `GrabberView` / `GrabberListView`): a sidebar of lists
//! (shops, quests, items, monsters); each grabs its game data on a bot thread into a
//! searchable list with the selected row's details, Copy / Export, and its own actions (Buy,
//! Accept, Equip, Kill, ...). The rows, what runs and the questions actions ask are
//! `models::grabber`'s; the window keeps each list's search and selection.

use super::files::SaveText;
use super::theme::{ACCENT, CARD, EDGE, HOVER, MUTED, TEXT};
use super::widgets::{self, Kind, Selection};
use crate::models::grabber as model;
use egui::{
    Align, Context, CornerRadius, Label, Layout, Modal, Rect, RichText, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2,
    vec2,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use turu_bot::Bot;
use turu_bot::grabber::{self, GrabKind, Grabbed};

const ROW_HEIGHT: f32 = 22.0;
const CONTROL_HEIGHT: f32 = 26.0;
const SIDEBAR_WIDTH: f32 = 164.0;
const DETAILS_WIDTH: f32 = 232.0;
/// Narrower than this, the window shows its lists in a drop-down instead of the sidebar.
const NARROW_WIDTH: f32 = 720.0;

/// One sidebar list (VibeSkua `GrabberListViewModel`).
struct List {
    kind: GrabKind,
    items: Arc<Vec<Grabbed>>,
    revision: u64,
    search: String,
    multiple: bool,
    selection: Selection,
    shown: Vec<usize>,
    shown_for: Option<String>,
}

impl List {
    fn new(kind: GrabKind) -> Self {
        Self {
            kind,
            items: Arc::default(),
            revision: 0,
            search: String::new(),
            multiple: true,
            selection: Selection::default(),
            shown: Vec::new(),
            shown_for: None,
        }
    }

    /// Takes a fresh grab, and filters again when the search changed (VibeSkua `ApplyFilter`).
    fn refresh(&mut self, snapshot: &model::Snapshot) {
        if snapshot.revision != self.revision {
            self.items = snapshot.items.clone();
            self.revision = snapshot.revision;
            self.selection.clear();
            self.shown_for = None;
        }
        if self.shown_for.as_deref() != Some(self.search.as_str()) {
            self.shown = model::filter(&self.items, &self.search);
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
}

pub struct GrabberWindow {
    tab: GrabKind,
    lists: HashMap<GrabKind, List>,
    /// The quantity typed for the question an action asks.
    quantity: String,
    /// Export's text, waiting for the save dialog.
    export: Option<(String, String)>,
    save: SaveText,
}

impl Default for GrabberWindow {
    fn default() -> Self {
        Self {
            tab: GrabKind::ShopItems,
            lists: GrabKind::ALL.into_iter().map(|k| (k, List::new(k))).collect(),
            quantity: String::new(),
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
        let bot = crate::turu::bot();
        let snapshot = model::snapshot(&bot, self.tab);
        super::window(ctx, "Grabber", [800.0, 560.0], 5)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &bot, &snapshot));
        self.modals(ctx, &bot);
        if snapshot.busy || snapshot.message.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    /// Opens the save dialog for an Export.
    pub fn save_export(&mut self, ctx: &Context) {
        self.save.poll();
        if let Some((file_name, text)) = self.export.take() {
            self.save.ask(ctx, &file_name, text);
        }
    }

    fn ui(&mut self, ui: &mut Ui, bot: &Arc<Bot>, snapshot: &model::Snapshot) {
        let rect = ui.available_rect_before_wrap();
        if rect.width() < NARROW_WIDTH {
            // A phone: the lists in a drop-down above, the list below at the window's width.
            ui.horizontal(|ui| {
                widgets::section_label(ui, "LIST");
                self.list_picker(ui);
            });
            ui.add_space(4.0);
            self.list_ui(ui, bot, snapshot);
            return;
        }
        let gap = 14.0;
        let side = Rect::from_min_max(rect.min, pos2(rect.left() + SIDEBAR_WIDTH, rect.bottom()));
        let main = Rect::from_min_max(pos2(side.right() + gap, rect.top()), rect.max);
        let mut sidebar = ui.new_child(UiBuilder::new().id_salt("side").max_rect(side));
        self.sidebar(&mut sidebar);
        let mut content = ui.new_child(UiBuilder::new().id_salt("main").max_rect(main));
        self.list_ui(&mut content, bot, snapshot);
        ui.allocate_rect(rect, Sense::hover());
    }

    /// The lists in a drop-down, grouped as the sidebar (a narrow window has no room for it).
    fn list_picker(&mut self, ui: &mut Ui) {
        ui.spacing_mut().interact_size.y = widgets::target(CONTROL_HEIGHT, 40.0);
        egui::ComboBox::from_id_salt("turu_grabber_list")
            .selected_text(self.tab.title())
            .width(ui.available_width())
            .height(f32::INFINITY)
            .show_ui(ui, |ui| {
                let mut group = "";
                for kind in GrabKind::ALL {
                    if kind.group() != group {
                        group = kind.group();
                        widgets::section_label(ui, group);
                    }
                    if ui.selectable_label(self.tab == kind, kind.title()).clicked() {
                        self.tab = kind;
                    }
                }
            });
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
                    if nav_item(ui, kind.title(), self.tab == kind) {
                        self.tab = kind;
                    }
                }
            });
    }

    fn list_ui(&mut self, ui: &mut Ui, bot: &Arc<Bot>, snapshot: &model::Snapshot) {
        let fights_blocked = model::fights_blocked(bot);
        let list = self.lists.get_mut(&self.tab).expect("every kind has a list");
        list.refresh(snapshot);
        let (busy, message) = (snapshot.busy, snapshot.message.clone());

        // Grab, search, Multi-select.
        ui.horizontal(|ui| {
            let grab = widgets::button(
                ui,
                "⟳ Grab",
                Kind::Primary,
                !busy,
                vec2(112.0, widgets::target(CONTROL_HEIGHT, 40.0)),
                "Busy",
            );
            let grab = if list.kind == GrabKind::MapItems {
                grab.on_hover_text("Download the current map's SWF and read its map items (kept per map file)")
            } else {
                grab
            };
            if grab.clicked() {
                model::grab(bot, list.kind);
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
            // The heading, the status and Cancel; Unselect / Copy / Export beside them, or on a
            // row of their own below when the list is narrow (a phone).
            let narrow = left.width() < 480.0;
            ui.horizontal(|ui| {
                ui.set_min_height(CONTROL_HEIGHT);
                widgets::section_label(ui, "ITEMS");
                widgets::count_badge(ui, list.shown.len());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !narrow {
                        list_actions(ui, bot, list, busy, &mut self.export);
                    }
                    if busy && widgets::button(ui, "Cancel", Kind::Danger, true, vec2(64.0, 22.0), "").clicked() {
                        model::cancel(bot, list.kind);
                    }
                    let status = match &message {
                        Some(text) => Some(RichText::new(text).size(11.0).strong().color(MUTED)),
                        None if !list.selection.is_empty() => Some(
                            RichText::new(format!("{} selected", list.selection.len()))
                                .size(11.0)
                                .color(MUTED),
                        ),
                        None => None,
                    };
                    if let Some(status) = status {
                        ui.add(egui::Label::new(status).truncate());
                    }
                });
            });
            if narrow {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        list_actions(ui, bot, list, busy, &mut self.export);
                    });
                });
                ui.add_space(2.0);
            }
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
                    "Stop the script, Console snippet or Auto Attack/Hunt first"
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
            model::start(bot, list.kind, action, list.selected());
        }
    }

    /// The quantity box (VibeSkua's input dialog) and the message box, of whichever list asks.
    fn modals(&mut self, ctx: &Context, bot: &Arc<Bot>) {
        for kind in GrabKind::ALL {
            let snapshot = model::snapshot(bot, kind);
            if let Some((title, prompt)) = &snapshot.ask {
                let mut answer = None;
                let response = Modal::new(egui::Id::new(("turu_grabber_quantity", kind.title()))).show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.heading(title);
                    ui.label(prompt);
                    let edit = widgets::text_box(ui, &mut self.quantity, "", ui.available_width());
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
                    let text = std::mem::take(&mut self.quantity);
                    model::answer(bot, kind, ok.then_some(text.as_str()));
                }
            }
            if let Some((title, message)) = &snapshot.notice {
                let mut close = false;
                let response = Modal::new(egui::Id::new(("turu_grabber_notice", kind.title()))).show(ctx, |ui| {
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
                    model::dismiss(bot, kind);
                }
            }
        }
    }
}

/// A sidebar entry: a boxed row, accent-tinted when picked.
fn nav_item(ui: &mut Ui, title: &str, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let (fill, stroke) = if selected {
        (ACCENT.gamma_multiply(0.25), ACCENT)
    } else if response.hovered() {
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
    let color = if selected { TEXT } else { MUTED };
    let font = egui::FontId::proportional(13.0);
    ui.painter().text(
        rect.left_center() + vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        font,
        color,
    );
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

/// Unselect / Copy / Export, laid out right to left (Export rightmost).
fn list_actions(ui: &mut Ui, bot: &Arc<Bot>, list: &mut List, busy: bool, export: &mut Option<(String, String)>) {
    ui.add_enabled_ui(!busy, |ui| {
        if widgets::small_button(ui, "Export", true)
            .on_hover_text("Export everything grabbed for the selected items (or all) to a .txt file")
            .clicked()
        {
            let targets = list.targets();
            if targets.is_empty() {
                model::report(bot, list.kind, "No items to export.", 3);
            } else {
                let text = grabber::detailed_export_text(list.kind.title(), &targets);
                let file = format!("{}.txt", list.kind.title().replace(' ', ""));
                model::report(
                    bot,
                    list.kind,
                    format!("Exported {} item(s) to file.", targets.len()),
                    3,
                );
                *export = Some((file, text));
            }
        }
        if widgets::small_button(ui, "Copy", true)
            .on_hover_text("Copy the selected items (or all) to the clipboard")
            .clicked()
            && let Some(text) = model::copy_text(bot, list.kind, &list.targets())
        {
            ui.ctx().copy_text(text);
        }
        if widgets::small_button(ui, "Unselect", true)
            .on_hover_text("Unselect all")
            .clicked()
        {
            list.selection.clear();
        }
    });
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
