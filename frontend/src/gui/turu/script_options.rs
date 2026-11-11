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
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Align2, ComboBox, Context, CornerRadius, FontId, Frame, Key, Label, Layout, Margin, RichText, ScrollArea,
    Sense, Stroke, StrokeKind, TextFormat, Ui, vec2,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use turu_bot::gear::{self, GearFile, GearItem, Place, Slot};
use turu_bot::script::ScriptOptionsState;
use turu_bot::script::options::{ItemPick, Kind, MAIN, OptionDecl, ScriptOptions};

/// How often the window takes the account's items from the gear cache again.
const GEAR_READ: Duration = Duration::from_secs(1);
/// A group of this many switches or more is a checklist.
const CHECKLIST: usize = 8;
/// From this many options on, the window has a filter box.
const FILTER_FROM: usize = 10;
/// Narrower than this, a row puts its editor under its label.
const STACK_BELOW: f32 = 440.0;
/// The editor column's width beside the labels.
const EDITOR_WIDTH: f32 = 220.0;
/// The tallest the item list of a picker gets before it scrolls.
const PICKER_LIST: f32 = 220.0;

#[derive(Default)]
pub struct ScriptOptionsWindow {
    /// The values being edited. Taken from the bot when the window opens.
    draft: Option<ScriptOptions>,
    /// Why the last Save failed.
    error: Option<String>,
    /// The script's request (`bot.config.configure`, or before a run) the window is open for.
    request: Option<u64>,
    /// The draft is a Scheduler entry's options (VibeSkua `EditScriptConfig`): Save writes its
    /// own file and leaves the loaded script alone.
    queued: bool,
    /// What the filter box keeps.
    filter: String,
    /// The item picker that is open.
    picker: Picker,
    /// The checklist groups that hide the items the account owns.
    hide_owned: HashSet<String>,
    /// The account's wearable items, as the gear cache last had them.
    gear: Option<GearFile>,
    gear_at: Option<Instant>,
    /// A gear read was started for this opening (the cache had none yet).
    asked_gear: bool,
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
    /// Edits a Scheduler entry's options, read from its `Queue_<name>_<id>` file.
    pub fn edit_queued(&mut self, options: ScriptOptions) {
        self.draft = Some(options);
        self.queued = true;
        self.error = None;
    }

    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        let bot = crate::turu::bot();
        let scripts = &bot.scripts;
        let pending = scripts.configure_request();
        if pending.is_some() && pending != self.request {
            // The script waits for the window (VibeSkua's options dialog on Start).
            self.request = pending;
            if self.queued {
                self.queued = false;
                self.draft = None;
            }
            *open = true;
        } else if self.request.is_some() && pending != self.request {
            // The script was stopped while it waited.
            self.request = None;
            *open = false;
        }
        if !*open {
            if let Some(request) = self.request.take() {
                scripts.finish_configure(request);
            }
            self.draft = None;
            self.error = None;
            self.queued = false;
            self.filter.clear();
            self.picker.close();
            self.hide_owned.clear();
            self.gear = None;
            self.gear_at = None;
            self.asked_gear = false;
            return;
        }
        if self.draft.is_none()
            && let ScriptOptionsState::Loaded(options) = scripts.options()
        {
            self.draft = Some(options);
        }
        if self.gear_at.is_none_or(|t| t.elapsed() >= GEAR_READ) {
            self.gear_at = Some(Instant::now());
            self.gear = bot.gear.last();
            if self.gear.is_none() && bot.gear.logged_in() && !self.asked_gear {
                // Logged in, but the gear thread hasn't read the items yet: read them now.
                self.asked_gear = true;
                crate::turu::spawn_action("Script Options gear", |bot| {
                    gear::refresh(bot)?;
                    Ok(())
                });
            }
        }
        let mut done = false;
        super::window(ctx, "Script Options", [600.0, 560.0], 2)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &mut done));
        if done {
            *open = false;
        }
        if self.gear.is_none() {
            ctx.request_repaint_after(GEAR_READ);
        }
    }

    fn ui(&mut self, ui: &mut Ui, done: &mut bool) {
        let bot = crate::turu::bot();
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let Self {
            draft,
            error,
            queued,
            filter,
            picker,
            hide_owned,
            gear,
            asked_gear,
            ..
        } = self;

        let Some(draft) = draft.as_mut() else {
            // Nothing to edit: no script is loaded, or the loaded one declares no options.
            let message = match bot.scripts.options() {
                ScriptOptionsState::Failed(e) => format!("The script's options couldn't be read: {e}"),
                _ => "The loaded script declares no options.".into(),
            };
            ui.label(RichText::new(message).color(MUTED));
            if widgets::button(ui, "Close", ButtonKind::Normal, true, vec2(80.0, 28.0), "").clicked() {
                *done = true;
            }
            return;
        };

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
            file: gear.as_ref(),
            reading: *asked_gear && bot.gear.logged_in(),
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
                    let saved = if *queued {
                        draft
                            .save()
                            .map_err(|e| format!("couldn't save {}: {e}", draft.path().display()))
                    } else {
                        bot.scripts.save_options(draft.clone())
                    };
                    match saved {
                        Ok(()) => *done = true,
                        Err(e) => *error = Some(e),
                    }
                }
                if widgets::button(ui, "Cancel", ButtonKind::Normal, true, vec2(80.0, height), "").clicked() {
                    *done = true;
                }
            });
        });
    }
}

/// What the window lays out, worked out from the declaration.
struct Plan {
    categories: Vec<Category>,
    /// Per checklist group, the choice that makes it used (merge `Select` and `Generic:mode`).
    links: HashMap<String, Link>,
    /// The options with a row (explanations aren't counted).
    option_count: usize,
}

struct Category {
    name: String,
    /// The name as shown: `_` as spaces.
    title: String,
    options: Vec<OptionDecl>,
    /// A group of switches long enough to show as a checklist.
    checklist: bool,
    /// Per choice option, the explanation of each of its choices.
    explanations: HashMap<String, HashMap<String, String>>,
    /// The text options that are explanations, shown under their choice instead of as rows.
    explanation_options: HashSet<String>,
}

/// A choice one of whose names is a checklist group's name: the group is used only when that
/// choice is picked (CoreAdvanced's merge `Mode` and its `Select` group).
struct Link {
    category: String,
    option: String,
    display_name: String,
    choice: String,
}

impl Plan {
    fn of(draft: &ScriptOptions) -> Self {
        let categories: Vec<Category> = draft
            .declaration()
            .categories()
            .filter(|(_, options)| !options.is_empty())
            .map(|(name, options)| Category::of(name, options))
            .collect();
        let mut links = HashMap::new();
        for category in &categories {
            for option in category.options.iter().filter(|o| o.kind == Kind::Choice) {
                for choice in &option.choices {
                    let group = categories
                        .iter()
                        .find(|g| g.checklist && g.name != MAIN && g.name.eq_ignore_ascii_case(choice));
                    if let Some(group) = group {
                        links.insert(
                            group.name.clone(),
                            Link {
                                category: category.name.clone(),
                                option: option.name.clone(),
                                display_name: option.display_name.clone(),
                                choice: choice.clone(),
                            },
                        );
                    }
                }
            }
        }
        let option_count = categories
            .iter()
            .map(|c| c.options.len() - c.explanation_options.len())
            .sum();
        Self {
            categories,
            links,
            option_count,
        }
    }
}

impl Category {
    fn of(name: &str, options: &[OptionDecl]) -> Self {
        let mut explanations = HashMap::new();
        let mut explanation_options = HashSet::new();
        for option in options.iter().filter(|o| o.kind == Kind::Choice) {
            let mut texts = HashMap::new();
            for choice in &option.choices {
                let key = format!("{}Explanation{}", option.name, capitalized(choice));
                if let Some(text) = options.iter().find(|o| o.kind == Kind::Text && o.name == key) {
                    texts.insert(choice.clone(), explanation(&text.description));
                    explanation_options.insert(key);
                }
            }
            if !texts.is_empty() {
                explanations.insert(option.name.clone(), texts);
            }
        }
        Self {
            name: name.to_owned(),
            title: name.replace('_', " "),
            checklist: name != MAIN && options.len() >= CHECKLIST && options.iter().all(|o| o.kind == Kind::Bool),
            options: options.to_vec(),
            explanations,
            explanation_options,
        }
    }
}

/// The account's items for the pickers and checklists.
struct Gear<'a> {
    file: Option<&'a GearFile>,
    /// The items are being read (logged in, the cache had none yet).
    reading: bool,
}

impl Gear<'_> {
    /// Where the item with this ID is: the inventory before the bank.
    fn place_of(&self, id: i64) -> Option<Place> {
        let items = self.file?.items.iter().filter(|i| i.id == id);
        items.map(|i| i.place).min_by_key(|p| *p == Place::Bank)
    }

    fn bank_read(&self) -> bool {
        self.file.is_some_and(|f| f.bank_as_of.is_some())
    }
}

/// Whether the filter keeps the option.
fn matches(option: &OptionDecl, query: &str) -> bool {
    query.is_empty() || option.display_name.to_lowercase().contains(query) || option.name.to_lowercase().contains(query)
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
        let picked = value.as_str().unwrap_or_default().replace('_', " ");
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
                .selected_text(current.replace('_', " "))
                .show_ui(ui, |ui| {
                    for choice in &option.choices {
                        let label = RichText::new(choice.replace('_', " "));
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

/// The account's items an item option offers: its slot's (or every wearable but ground items),
/// inventory first, then bank, each by name, one per name.
fn candidates(file: &GearFile, pick: ItemPick) -> Vec<&GearItem> {
    let mut items: Vec<&GearItem> = file
        .items
        .iter()
        .filter(|i| match pick.slot() {
            Some(slot) => i.slot == slot,
            None => i.slot != Slot::Ground,
        })
        .collect();
    items.sort_by_key(|i| (i.place == Place::Bank, i.name.to_lowercase()));
    let mut seen = HashSet::new();
    items.retain(|i| seen.insert(i.name.to_lowercase()));
    items
}

/// "Worn" or "Bank"; nothing for an item in the inventory.
fn place_tag(item: &GearItem) -> Option<&'static str> {
    if item.equipped {
        Some("Worn")
    } else if item.place == Place::Bank {
        Some("Bank")
    } else {
        None
    }
}

/// A warning under an item option whose name isn't among the account's items.
fn missing_note(pick: ItemPick, current: &str, gear: &Gear) -> Option<String> {
    let file = gear.file?;
    let name = current.trim();
    if name.is_empty() || candidates(file, pick).iter().any(|i| i.name.eq_ignore_ascii_case(name)) {
        return None;
    }
    let what = noun(pick).0;
    Some(if gear.bank_read() {
        format!("No {what} of this name in your inventory or bank")
    } else {
        format!("No {what} of this name in your inventory (the bank hasn't been opened yet)")
    })
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
                let choice = link.choice.replace('_', " ");
                let note = format!(
                    "Used only when \u{201c}{}\u{201d} is set to {choice}. It's set to {} now.",
                    link.display_name,
                    now.replace('_', " ")
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
    let choice = link.choice.replace('_', " ");
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

/// What an item option picks: `("helm", "a helm")`.
fn noun(pick: ItemPick) -> (&'static str, &'static str) {
    match pick {
        ItemPick::Any => ("item", "an item"),
        ItemPick::Class => ("class", "a class"),
        ItemPick::Weapon => ("weapon", "a weapon"),
        ItemPick::Armor => ("armor", "an armor"),
        ItemPick::Helm => ("helm", "a helm"),
        ItemPick::Cape => ("cape", "a cape"),
        ItemPick::Pet => ("pet", "a pet"),
        ItemPick::Necklace => ("necklace", "a necklace"),
    }
}

fn plural(noun: &str) -> String {
    if noun.ends_with('s') {
        format!("{noun}es")
    } else {
        format!("{noun}s")
    }
}

/// "acOnly" -> "AcOnly", as CoreAdvanced names its explanation options.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// An explanation option's description without its "Mode [all]:" lead.
fn explanation(description: &str) -> String {
    let text = description.split_once("]:").map_or(description, |(_, rest)| rest);
    tidy(text)
}

/// A description on one line's terms: tabs and runs of spaces as one space, each line trimmed.
fn tidy(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// A default as the window's tooltip shows it (VibeSkua: "Default: True").
fn default_text(value: &Value) -> String {
    match value {
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(text) if text.is_empty() => "(empty)".into(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explanations_lose_their_lead_and_tabs() {
        assert_eq!(
            explanation("Mode [all]: \t\tYou get all the items from shop."),
            "You get all the items from shop."
        );
        assert_eq!(capitalized("acOnly"), "AcOnly");
        assert_eq!(
            tidy("Mode: [select] only\nShould  it\tbuy?"),
            "Mode: [select] only\nShould it buy?"
        );
        assert_eq!(plural("class"), "classes");
    }
}
