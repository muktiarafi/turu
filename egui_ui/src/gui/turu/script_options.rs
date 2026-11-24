//! Scripts > Edit Script Options (VibeSkua's "Script Options" dialog): the loaded script's options,
//! each with its description and an editor, then each option group under its own title. The
//! window edits a copy taken when it opens. Save writes the values to `options/<Storage>.cfg`;
//! Cancel (or closing the window) drops them. It also opens by itself when the script asks (before
//! a run, or `bot.config.configure`); the script waits until it closes.
//!
//! Beyond VibeSkua's plain list:
//! * A row puts its editor under its label when the window is narrow (a phone, a narrowed window).
//! * Many options get a filter box.
//! * A group of many switches (a merge script's `Select`) is a checklist: All and None for the
//!   rows shown, a count, the items the account owns marked and hideable, and a note with a fix
//!   when a choice elsewhere makes the group unused (merge `Mode` isn't `select`).
//! * A choice's explanation options (`modeExplanationAll` for `mode`'s `all`, CoreAdvanced's
//!   "click here" boxes) aren't rows: the picked choice's explanation shows under the choice.
//! * An item option (`option.item`) is picked from the account's wearable items (the gear cache:
//!   inventory, and the bank once opened), or typed.

use super::theme::{ACCENT, EDGE, INSET, MUTED, SMALL_RADIUS, TEXT, WARNING};
use super::widgets::{self, Kind as ButtonKind};
use crate::models::script_options::{
    self as model, Category, Gear, Link, Plan, Request, State, candidates, default_text, matches, missing_note, noun,
    place_tag, plural, tidy, words,
};
use crate::models::tab_state::lock;
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Align2, ComboBox, Context, CornerRadius, FontId, Frame, Key, Label, Layout, Margin, RichText, ScrollArea,
    Sense, Stroke, StrokeKind, TextFormat, Ui, vec2,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use turu_bot::gear::{GearItem, Place};
use turu_bot::script::ScriptOptionsState;
use turu_bot::script::options::{ItemPick, Kind, MAIN, OptionDecl, ScriptOptions};

/// From this many options on, the window has a filter box.
const FILTER_FROM: usize = 10;
/// Narrower than this, a row puts its editor under its label.
const STACK_BELOW: f32 = 440.0;
/// The editor column's width beside the labels.
const EDITOR_WIDTH: f32 = 220.0;
/// The tallest the item list of a picker gets before it scrolls.
const PICKER_LIST: f32 = 220.0;

/// The window's own state; the draft, the script's request and the last error are the tab's
/// `models::script_options` state, shared with a native page.
#[derive(Default)]
pub struct ScriptOptionsWindow {
    /// What the filter box keeps.
    filter: String,
    /// The item picker that is open.
    picker: Picker,
    was_open: bool,
}

/// The open item picker: which option, and what its search box holds.
#[derive(Default)]
struct Picker {
    /// `(category, option name)`.
    open: Option<(String, String)>,
    search: String,
    /// Focus the search box on the next frame (just opened, with a pointer).
    focus: bool,
}

impl Picker {
    fn is_open(&self, category: &str, name: &str) -> bool {
        self.open.as_ref().is_some_and(|(c, n)| c == category && n == name)
    }

    fn toggle(&mut self, category: &str, name: &str) {
        if self.is_open(category, name) {
            self.close();
        } else {
            self.open = Some((category.to_owned(), name.to_owned()));
            self.search.clear();
            self.focus = !widgets::touch();
        }
    }

    fn close(&mut self) {
        *self = Self::default();
    }
}

impl ScriptOptionsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        let bot = crate::turu::bot();
        let state = model::STATE.of(&bot);
        // Held for the frame: only `State` methods below, never a function that locks it again.
        let mut st = lock(&state);
        match st.follow_request(&bot) {
            // The script waits for the window (VibeSkua's options dialog on Start).
            Request::Opened => *open = true,
            // The script was stopped while it waited.
            Request::Gone => *open = false,
            Request::None => {}
        }
        // A script still waits but no editor shows it (the native page went): the window again.
        if !*open && st.request.is_some() && !st.native {
            *open = true;
        }
        if !*open {
            if std::mem::take(&mut self.was_open) {
                self.filter.clear();
                self.picker.close();
                // Closed here (a menu, a hotkey): a Cancel, unless the native page shows it.
                if !st.native {
                    st.close(&bot);
                }
            }
            return;
        }
        self.was_open = true;
        st.native = false;
        st.ensure_draft(&bot);
        let mut done = false;
        let mut still_open = true;
        super::window(ctx, "Script Options", [600.0, 560.0], 2)
            .open(&mut still_open)
            .show(ctx, |ui| self.ui(ui, &mut st, &bot, &mut done));
        if done || !still_open {
            *open = false;
            self.was_open = false;
            self.filter.clear();
            self.picker.close();
            if !done {
                st.close(&bot);
            }
        }
        if st.gear(&bot).file.is_none() {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
    }

    fn ui(&mut self, ui: &mut Ui, st: &mut State, bot: &std::sync::Arc<turu_bot::Bot>, done: &mut bool) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let Self { filter, picker, .. } = self;
        let gear_file = st.gear(bot).file.cloned();
        let reading = st.gear(bot).reading;
        if st.draft.is_none() {
            // Nothing to edit: no script is loaded, or the loaded one declares no options.
            let message = match bot.scripts.options() {
                ScriptOptionsState::Failed(e) => {
                    format!("The script's options couldn't be read: {e}")
                }
                _ => "The loaded script declares no options.".into(),
            };
            ui.label(RichText::new(message).color(MUTED));
            if widgets::button(ui, "Close", ButtonKind::Normal, true, vec2(80.0, 28.0), "").clicked() {
                *done = true;
                st.close(bot);
            }
            return;
        }
        let (mut save, mut cancel) = (false, false);
        let State {
            draft,
            error,
            hide_owned,
            ..
        } = st;
        let draft = draft.as_mut().expect("checked above");

        let saved_in = draft
            .path()
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        ui.label(
            RichText::new(format!("Saved in options/{saved_in}"))
                .size(widgets::small_label_size())
                .color(MUTED),
        )
        .on_hover_text(draft.path().display().to_string());

        let plan = Plan::of(draft);
        if plan.option_count >= FILTER_FROM {
            let response = widgets::text_box(ui, filter, "Filter options", ui.available_width());
            if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                filter.clear();
            }
        }
        let query = filter.trim().to_lowercase();
        let gear = Gear {
            file: gear_file.as_ref(),
            reading,
        };

        let footer = widgets::target(28.0, 40.0) + 18.0 + if error.is_some() { 22.0 } else { 0.0 };
        let list_height = (ui.available_height() - footer).max(120.0);
        let mut edits: Vec<(String, String, Value)> = Vec::new();
        ScrollArea::vertical()
            .id_salt("script_options_list")
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut shown_any = false;
                for category in &plan.categories {
                    let category_hit = !query.is_empty() && category.title.to_lowercase().contains(&query);
                    let shown: Vec<&OptionDecl> = category
                        .options
                        .iter()
                        .filter(|o| !category.explanation_options.contains(&o.name))
                        .filter(|o| category_hit || matches(o, &query))
                        .collect();
                    if shown.is_empty() {
                        continue;
                    }
                    shown_any = true;
                    if category.name != MAIN {
                        ui.add_space(4.0);
                        // VibeSkua titles a group with its field name, `_` as spaces.
                        ui.label(RichText::new(&category.title).color(TEXT).strong());
                    }
                    widgets::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if category.checklist {
                            let link = plan.links.get(&category.name).map(|link| {
                                let now = draft
                                    .get_in(&link.category, &link.option)
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_owned();
                                (link, now)
                            });
                            checklist(
                                ui,
                                category,
                                &shown,
                                draft,
                                &gear,
                                hide_owned,
                                link.as_ref().map(|(l, now)| (*l, now.as_str())),
                                &mut edits,
                            );
                        } else {
                            for (index, option) in shown.iter().enumerate() {
                                if index > 0 {
                                    row_rule(ui);
                                }
                                let value = draft
                                    .get_in(&category.name, &option.name)
                                    .cloned()
                                    .unwrap_or(Value::Null);
                                let explanations = category.explanations.get(&option.name);
                                if let Some(value) =
                                    option_row(ui, &category.name, option, &value, explanations, picker, &gear)
                                {
                                    edits.push((category.name.clone(), option.name.clone(), value));
                                }
                            }
                        }
                    });
                }
                if !shown_any {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!("No options match \u{201c}{}\u{201d}.", filter.trim())).color(MUTED),
                    );
                }
            });

        for (category, name, value) in edits {
            if let Err(e) = draft.set_in(&category, &name, value) {
                *error = Some(e);
            }
        }

        if let Some(error) = error.as_ref() {
            ui.label(RichText::new(error).small().color(super::theme::DANGER));
        }
        let height = widgets::target(28.0, 40.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, "Reset Defaults", ButtonKind::Normal, true, vec2(0.0, height), "")
                .on_hover_text("Put every option back to the script's default (saved when you press Save)")
                .clicked()
            {
                draft.reset();
                picker.close();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Save", ButtonKind::Primary, true, vec2(80.0, height), "").clicked() {
                    save = true;
                }
                if widgets::button(ui, "Cancel", ButtonKind::Normal, true, vec2(80.0, height), "").clicked() {
                    cancel = true;
                }
            });
        });
        if save {
            // Saved: the editor closes (`State::save` finishes the script's request).
            if st.save(bot).is_ok() {
                *done = true;
            }
        } else if cancel {
            st.close(bot);
            *done = true;
        }
    }
}

/// A hairline between two rows of a card.
fn row_rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, EDGE));
}

/// One option: its label and description, and its editor beside them (or under them when the
/// window is narrow). Returns the new value when it changed.
fn option_row(
    ui: &mut Ui,
    category: &str,
    option: &OptionDecl,
    value: &Value,
    explanations: Option<&HashMap<String, String>>,
    picker: &mut Picker,
    gear: &Gear,
) -> Option<Value> {
    let width = ui.available_width();
    let stacked = width < STACK_BELOW && option.kind != Kind::Bool;
    let mut edit = None;
    if stacked {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            labels(ui, option, value, explanations);
            ui.add_space(2.0);
            edit = editor(ui, category, option, value, width, picker);
        });
    } else {
        let editor_width = if option.kind == Kind::Bool {
            48.0
        } else {
            EDITOR_WIDTH.min(width * 0.45)
        };
        ui.horizontal(|ui| {
            ui.set_min_height(widgets::target(26.0, 40.0));
            let text_width = (width - editor_width - 16.0).max(80.0);
            ui.allocate_ui_with_layout(vec2(text_width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                labels(ui, option, value, explanations);
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                edit = editor(ui, category, option, value, editor_width, picker);
            });
        });
    }
    if let Some(pick) = option.item {
        let current = edit.as_ref().unwrap_or(value).as_str().unwrap_or_default().to_owned();
        if picker.is_open(category, &option.name) {
            if let Some(picked) = picker_panel(ui, pick, &current, picker, gear) {
                edit = Some(picked);
            }
        } else if let Some(note) = missing_note(pick, &current, gear) {
            ui.label(RichText::new(note).size(widgets::small_label_size()).color(WARNING));
        }
    }
    edit
}

/// The option's name, description, and for an explained choice what the picked name does.
fn labels(ui: &mut Ui, option: &OptionDecl, value: &Value, explanations: Option<&HashMap<String, String>>) {
    let title = ui.add(Label::new(RichText::new(&option.display_name).color(TEXT).strong()).wrap());
    title.on_hover_text(format!("Default: {}", default_text(&option.default_value)));
    let description = tidy(&option.description);
    if !description.is_empty() {
        ui.add(
            Label::new(
                RichText::new(description)
                    .size(widgets::small_label_size())
                    .color(MUTED),
            )
            .wrap(),
        );
    }
    if let Some(text) = explanations
        .zip(value.as_str())
        .and_then(|(texts, picked)| texts.get(picked))
    {
        let picked = words(value.as_str().unwrap_or_default());
        let mut job = LayoutJob::default();
        let size = widgets::small_label_size();
        job.append(
            &format!("{picked}: "),
            0.0,
            TextFormat::simple(FontId::proportional(size), TEXT),
        );
        job.append(text, 0.0, TextFormat::simple(FontId::proportional(size), TEXT));
        ui.add(Label::new(job).wrap());
    }
}

/// One option's editor, `width` wide at most: a switch, a number box, a text box, a drop-down, or
/// an item field that opens its picker. Returns the new value when it changed.
fn editor(
    ui: &mut Ui,
    category: &str,
    option: &OptionDecl,
    value: &Value,
    width: f32,
    picker: &mut Picker,
) -> Option<Value> {
    match option.kind {
        Kind::Bool => {
            let mut on = value.as_bool().unwrap_or(false);
            widgets::switch(ui, &mut on).changed().then_some(Value::Bool(on))
        }
        Kind::Int => {
            let mut number = value.as_i64().unwrap_or(0);
            let size = vec2(width.min(140.0), widgets::target(26.0, 40.0));
            widgets::number(
                ui,
                (category, &option.name),
                &mut number,
                i64::MIN..=i64::MAX,
                1.0,
                "",
                size,
            )
            .changed()
            .then(|| json!(number))
        }
        Kind::Text => {
            let text = value.as_str().unwrap_or_default();
            if let Some(pick) = option.item {
                let open = picker.is_open(category, &option.name);
                if item_field(ui, text, pick, open, width).clicked() {
                    picker.toggle(category, &option.name);
                }
                return None;
            }
            let mut text = text.to_owned();
            widgets::text_box(ui, &mut text, "", width)
                .changed()
                .then_some(Value::String(text))
        }
        Kind::Choice => {
            // VibeSkua shows an enum's members with `_` as spaces.
            let current = value.as_str().unwrap_or_default();
            let mut picked = None;
            ComboBox::from_id_salt(("script_option", category, &option.name))
                .width(width - 8.0)
                .height(320.0)
                .selected_text(words(current))
                .show_ui(ui, |ui| {
                    for choice in &option.choices {
                        let label = RichText::new(words(choice));
                        let row = ui.add_sized(
                            vec2(ui.available_width(), widgets::target(22.0, 40.0)),
                            egui::Button::selectable(choice == current, label),
                        );
                        if row.clicked() {
                            picked = Some(Value::String(choice.clone()));
                        }
                    }
                });
            picked
        }
    }
}

/// The field of an item option: the item's name (or a muted "Choose a helm") in a text-box-like
/// well with a chevron; clicking it opens or closes the picker under the row.
fn item_field(ui: &mut Ui, text: &str, pick: ItemPick, open: bool, width: f32) -> egui::Response {
    let size = vec2(width, widgets::target(28.0, 40.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let stroke = if open || response.has_focus() {
        Stroke::new(1.0, ACCENT)
    } else if hovered {
        Stroke::new(1.0, MUTED)
    } else {
        Stroke::new(1.0, EDGE)
    };
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(6), INSET, stroke, StrokeKind::Inside);
    let font = FontId::proportional(13.0);
    let (label, color) = if text.trim().is_empty() {
        (format!("Choose {}", noun(pick).1), MUTED)
    } else {
        (text.to_owned(), TEXT)
    };
    let room = (rect.width() - 34.0).max(10.0);
    let mut job = LayoutJob::simple_singleline(label, font.clone(), color);
    job.wrap = TextWrapping::truncate_at_width(room);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let text_pos = egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(text_pos, galley, color);
    painter.text(
        egui::pos2(rect.right() - 12.0, rect.center().y),
        Align2::CENTER_CENTER,
        if open { "\u{25b4}" } else { "\u{25be}" },
        font,
        if open { ACCENT } else { MUTED },
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let what = noun(pick).0;
    response.on_hover_text(if text.trim().is_empty() {
        format!("Pick the {what} from your items, or type its name")
    } else {
        format!("{text}\nClick to pick another {what}")
    })
}

/// The picker under an item option: a search box, the account's matching items (inventory first,
/// then bank), and a box to type the name instead. Returns the picked or typed name.
fn picker_panel(ui: &mut Ui, pick: ItemPick, current: &str, picker: &mut Picker, gear: &Gear) -> Option<Value> {
    let (what, _) = noun(pick);
    let plural = plural(what);
    let mut edit = None;
    let mut picked = false;
    // Outlined only, so its search and name boxes read as fields.
    Frame::new()
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(CornerRadius::same(SMALL_RADIUS))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            let items = gear.file.map(|file| candidates(file, pick)).unwrap_or_default();
            if !items.is_empty() {
                let search = widgets::text_box(
                    ui,
                    &mut picker.search,
                    &format!("Search your {plural}"),
                    ui.available_width(),
                );
                if picker.focus {
                    search.request_focus();
                    picker.focus = false;
                }
            }
            let query = picker.search.trim().to_lowercase();
            let shown: Vec<&GearItem> = items
                .iter()
                .copied()
                .filter(|i| query.is_empty() || i.name.to_lowercase().contains(&query))
                .collect();
            if gear.file.is_none() {
                let text = if gear.reading {
                    "Reading your items\u{2026}".to_owned()
                } else {
                    format!("Log in to choose from your {plural}. You can type the name below meanwhile.")
                };
                ui.label(RichText::new(text).size(widgets::small_label_size()).color(MUTED));
            } else if items.is_empty() {
                let place = if gear.bank_read() {
                    "inventory or bank"
                } else {
                    "inventory"
                };
                ui.label(
                    RichText::new(format!("No {plural} in your {place}. Type the name below."))
                        .size(widgets::small_label_size())
                        .color(MUTED),
                );
            } else if shown.is_empty() {
                ui.label(
                    RichText::new(format!("No {plural} match \u{201c}{}\u{201d}.", picker.search.trim()))
                        .size(widgets::small_label_size())
                        .color(MUTED),
                );
            } else {
                let row_height = widgets::target(28.0, 40.0);
                // Nested in the window's list, a scroll area shrinks to its minimum: ask for the
                // rows' height (up to PICKER_LIST) outright.
                let height = (shown.len() as f32 * (row_height + 2.0)).min(PICKER_LIST);
                ScrollArea::vertical()
                    .id_salt(("script_option_picker", what))
                    .min_scrolled_height(height)
                    .max_height(height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for item in shown {
                            let selected = item.name.eq_ignore_ascii_case(current.trim());
                            let row = widgets::list_row(ui, selected, row_height, |ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if let Some(tag) = place_tag(item) {
                                        ui.label(RichText::new(tag).size(widgets::small_label_size()).color(MUTED));
                                    }
                                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                        ui.add(Label::new(RichText::new(&item.name).color(TEXT)).truncate());
                                    });
                                });
                            });
                            if row.clicked() {
                                edit = Some(Value::String(item.name.clone()));
                                picked = true;
                            }
                        }
                    });
                if !gear.bank_read() {
                    ui.label(
                        RichText::new("Bank items show here once the bank has been opened.")
                            .size(widgets::small_label_size())
                            .color(MUTED),
                    );
                }
            }
            ui.horizontal(|ui| {
                let mut typed = current.to_owned();
                let clear_width = if current.is_empty() { 0.0 } else { 64.0 };
                let response =
                    widgets::text_box(ui, &mut typed, "Or type the name", ui.available_width() - clear_width);
                if response.changed() {
                    edit = Some(Value::String(typed));
                }
                if !current.is_empty()
                    && widgets::button(
                        ui,
                        "Clear",
                        ButtonKind::Normal,
                        true,
                        vec2(56.0, widgets::target(26.0, 40.0)),
                        "",
                    )
                    .clicked()
                {
                    edit = Some(Value::String(String::new()));
                }
            });
        });
    if picked || ui.input(|i| i.key_pressed(Key::Escape)) {
        picker.close();
    }
    edit
}

/// A group of switches as a checklist: a count, All and None for the rows shown, Hide owned, the
/// note when the group is unused, then one row per item.
#[allow(clippy::too_many_arguments)]
fn checklist(
    ui: &mut Ui,
    category: &Category,
    shown: &[&OptionDecl],
    draft: &ScriptOptions,
    gear: &Gear,
    hide_owned: &mut HashSet<String>,
    link: Option<(&Link, &str)>,
    edits: &mut Vec<(String, String, Value)>,
) {
    let is_on = |name: &str| {
        draft
            .get_in(&category.name, name)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let owned = |option: &OptionDecl| option.name.parse::<i64>().ok().and_then(|id| gear.place_of(id));
    let total = category.options.len();
    let on = category.options.iter().filter(|o| is_on(&o.name)).count();
    let owned_count = category.options.iter().filter(|o| owned(o).is_some()).count();
    let hiding = hide_owned.contains(&category.name) && owned_count > 0;
    let rows: Vec<&OptionDecl> = shown
        .iter()
        .copied()
        .filter(|o| !hiding || owned(o).is_none())
        .collect();

    if let Some((link, now)) = link
        && now != link.choice
    {
        Frame::new()
            .fill(INSET)
            .stroke(Stroke::new(1.0, WARNING.gamma_multiply(0.45)))
            .corner_radius(CornerRadius::same(SMALL_RADIUS))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let choice = words(&link.choice);
                let note = format!(
                    "Used only when \u{201c}{}\u{201d} is set to {choice}. It's set to {} now.",
                    link.display_name,
                    words(now)
                );
                let button_width = 120.0;
                let note = RichText::new(note).size(widgets::small_label_size()).color(WARNING);
                if ui.available_width() < 360.0 {
                    ui.add(Label::new(note).wrap());
                    use_choice(ui, link, button_width, edits);
                } else {
                    ui.horizontal(|ui| {
                        let text_width = ui.available_width() - button_width - 12.0;
                        ui.allocate_ui_with_layout(vec2(text_width, 0.0), Layout::top_down(Align::Min), |ui| {
                            ui.add(Label::new(note).wrap());
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            use_choice(ui, link, button_width, edits);
                        });
                    });
                }
            });
    }

    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("{on} of {total} picked"))
                .size(widgets::small_label_size())
                .color(if on > 0 { TEXT } else { MUTED }),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let some_off = rows.iter().any(|o| !is_on(&o.name));
            let some_on = rows.iter().any(|o| is_on(&o.name));
            let scope = if rows.len() < total { "shown" } else { "" };
            if widgets::small_button(ui, "None", some_on)
                .on_hover_text(format!(
                    "Unpick every item {}",
                    if scope.is_empty() { "here" } else { scope }
                ))
                .clicked()
            {
                for option in rows.iter().filter(|o| is_on(&o.name)) {
                    edits.push((category.name.clone(), option.name.clone(), Value::Bool(false)));
                }
            }
            if widgets::small_button(ui, "All", some_off)
                .on_hover_text(format!(
                    "Pick every item {}",
                    if scope.is_empty() { "here" } else { scope }
                ))
                .clicked()
            {
                for option in rows.iter().filter(|o| !is_on(&o.name)) {
                    edits.push((category.name.clone(), option.name.clone(), Value::Bool(true)));
                }
            }
            if owned_count > 0 {
                let label = format!("Hide owned ({owned_count})");
                if widgets::chip(ui, &label, hiding) {
                    if hiding {
                        hide_owned.remove(&category.name);
                    } else {
                        hide_owned.insert(category.name.clone());
                    }
                }
            }
        });
    });

    if rows.is_empty() {
        let text = if hiding && !shown.is_empty() {
            "You own every item shown.".to_owned()
        } else {
            "No items match the filter.".to_owned()
        };
        ui.label(RichText::new(text).size(widgets::small_label_size()).color(MUTED));
        return;
    }
    let row_height = widgets::target(30.0, 44.0);
    ui.spacing_mut().item_spacing.y = 2.0;
    for option in rows {
        let mut on = is_on(&option.name);
        let place = owned(option);
        let mut toggled = false;
        let row = widgets::list_row(ui, on, row_height, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                toggled = widgets::switch(ui, &mut on).changed();
                if let Some(place) = place {
                    let tag = if place == Place::Bank { "In bank" } else { "Owned" };
                    ui.label(RichText::new(tag).size(widgets::small_label_size()).color(MUTED));
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(
                        Label::new(RichText::new(&option.display_name).color(TEXT))
                            .truncate()
                            .selectable(false),
                    );
                });
            });
        });
        let tooltip = match option.name.parse::<i64>() {
            Ok(id) => format!("Item ID {id}"),
            Err(_) => tidy(&option.description),
        };
        let row = if tooltip.is_empty() {
            row
        } else {
            row.on_hover_text(tooltip)
        };
        if row.clicked() {
            on = !on;
            toggled = true;
        }
        if toggled {
            edits.push((category.name.clone(), option.name.clone(), Value::Bool(on)));
        }
    }
}

/// The note's button: sets the linked choice ("Use select").
fn use_choice(ui: &mut Ui, link: &Link, width: f32, edits: &mut Vec<(String, String, Value)>) {
    let choice = words(&link.choice);
    let size = vec2(width, widgets::target(24.0, 40.0));
    if widgets::button(ui, &format!("Use {choice}"), ButtonKind::Normal, true, size, "")
        .on_hover_text(format!("Set {} to {choice}", link.display_name))
        .clicked()
    {
        edits.push((
            link.category.clone(),
            link.option.clone(),
            Value::String(link.choice.clone()),
        ));
    }
}
