//! Options > CoreBots (VibeSkua `CoreBotsView`, 800x608): the per-character CoreBots Options
//! file (`options/CBO_Storage(<username>).txt`) that CoreBots' `setOptions()` reads. Tabs:
//! Loadout (classes, modes and gear per role), Options and Other. Opening the window loads the
//! file; closing it saves. The header says whether the window holds unsaved changes.

use super::game_options::Answer;
use super::skills::SkillSets;
use super::theme::{CARD, EDGE, HOVER, INSET, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::models::core_bots::{Draft, OPTION_GROUPS, Opt, PUBLIC_CONFIRMED, Purpose, Role, SLOTS, Value};
use crate::models::game_options::PUBLIC_ROOMS;
use egui::{
    Align, Color32, ComboBox, CornerRadius, Frame, Label, Layout, Margin, Modal, RichText, ScrollArea, Sense, Stroke,
    Ui, UiBuilder, vec2,
};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Loadout,
    Options,
    Other,
}

pub struct CoreBotsWindow {
    /// The options file's state (`crate::models::core_bots`): whose, as read, as changed.
    draft: Draft,
    was_open: bool,
    tab: Tab,
    search: String,
    /// Save is waiting for the public room warning's answer.
    confirm_public: bool,
    /// Revert to saved is waiting for its question's answer.
    confirm_revert: bool,
}

impl Default for CoreBotsWindow {
    fn default() -> Self {
        Self {
            draft: Draft::default(),
            was_open: false,
            tab: Tab::Loadout,
            search: String::new(),
            confirm_public: false,
            confirm_revert: false,
        }
    }
}

impl std::ops::Deref for CoreBotsWindow {
    type Target = Draft;
    fn deref(&self) -> &Draft {
        &self.draft
    }
}

impl std::ops::DerefMut for CoreBotsWindow {
    fn deref_mut(&mut self) -> &mut Draft {
        &mut self.draft
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl CoreBotsWindow {
    pub fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        if *open && !self.was_open {
            // VibeSkua OnActivated: load without explaining why when logged out.
            self.sets = SkillSets::load();
            self.draft.fetch(&crate::turu::bot(), Purpose::Open);
        } else if !*open && self.was_open && !self.username.is_empty() {
            // VibeSkua OnDeactivated: save without a dialog.
            self.save(false);
        }
        self.was_open = *open;
        self.apply_fetched();
        if !*open {
            return;
        }
        super::window(ctx, "CoreBots Options", [800.0, 608.0], 2)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        self.dialogs(ctx);
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
        self.header(ui);
        self.tab_row(ui);
        // Logged out there is no character to save to: what you changed would be lost when the
        // window closes, so the tabs only show the values.
        let editable = !self.username.is_empty();
        ui.add_enabled_ui(editable, |ui| match self.tab {
            Tab::Loadout => self.loadout_tab(ui),
            Tab::Options => self.options_tab(ui),
            Tab::Other => self.other_tab(ui),
        });
    }

    /// The header's second line: whose file this is and whether it's saved.
    fn save_state(&self) -> (String, Color32) {
        if self.busy {
            ("Reading the game...".into(), MUTED)
        } else if self.username.is_empty() {
            ("Not logged in · log in, then press Refresh from game".into(), MUTED)
        } else if self.has_changes() {
            ("Unsaved changes · saved when you close this window".into(), WARNING)
        } else if self.has_file {
            ("Saved".into(), MUTED)
        } else {
            ("CoreBots' defaults · not saved yet".into(), MUTED)
        }
    }

    /// The character, whether its options are saved, and the actions. The name and state get
    /// the width the buttons leave (cut with "…" past it); in a narrow window (a small screen,
    /// a phone) the buttons go on a row of their own under them.
    /// Returns where the text and the buttons went.
    fn header(&mut self, ui: &mut Ui) -> (egui::Rect, egui::Rect) {
        let narrow = ui.available_width() < HEADER_BUTTONS_WIDTH + HEADER_TEXT_MIN_WIDTH;
        let buttons = |this: &mut Self, ui: &mut Ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                this.header_buttons(ui);
                ui.min_rect()
            })
            .inner
        };
        if narrow {
            let text = self.header_text(ui, ui.available_width());
            // In a row, or right-to-left would take all the height a scrolling window offers.
            (text, ui.horizontal(|ui| buttons(self, ui)).inner)
        } else {
            ui.horizontal(|ui| {
                let width = ui.available_width() - HEADER_BUTTONS_WIDTH;
                let text = self.header_text(ui, width);
                (text, buttons(self, ui))
            })
            .inner
        }
    }

    /// The character's name and the save state, `width` wide.
    fn header_text(&self, ui: &mut Ui, width: f32) -> egui::Rect {
        ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 2.0;
            let name = if self.username.is_empty() {
                RichText::new("No character").size(15.0).strong().color(MUTED)
            } else {
                let shown = crate::models::core_bots::shown_name(&crate::turu::bot(), &self.username);
                RichText::new(shown).size(15.0).strong().color(TEXT)
            };
            ui.add(Label::new(name).truncate());
            let (state, color) = self.save_state();
            ui.add(Label::new(RichText::new(&state).size(12.0).color(color)).truncate())
                .on_hover_text(&state);
        })
        .response
        .rect
    }

    /// Save, Refresh from game and Revert to saved, right to left, with the spinner or "Saved".
    fn header_buttons(&mut self, ui: &mut Ui) {
        let enabled = !self.busy;
        if widgets::button(ui, "Save", Kind::Primary, enabled, vec2(84.0, 28.0), "")
            .on_hover_text("Save current CoreBots options")
            .clicked()
        {
            let public = self
                .options
                .iter()
                .any(|o| o.tag == "PrivateRooms" && o.value == Value::Bool(false));
            if public {
                self.confirm_public = true;
            } else {
                self.draft.fetch(&crate::turu::bot(), Purpose::Save);
            }
        }
        if widgets::button(ui, "Refresh from game", Kind::Normal, enabled, vec2(140.0, 28.0), "")
                .on_hover_text(
                    "Reads your classes and gear again and keeps your picks; for a character the \
                     window hasn't loaded, loads its saved options (VibeSkua: Load character, Reload Classes, Refresh Inventory)",
                )
                .clicked()
            {
                self.draft.fetch(&crate::turu::bot(), Purpose::LoadCharacter);
            }
        let changed = self.has_changes();
        if widgets::button(
            ui,
            "Revert to saved",
            Kind::Normal,
            enabled && changed,
            vec2(120.0, 28.0),
            "Nothing to revert: the window shows the saved options",
        )
        .on_hover_text("Puts back the options as last saved (VibeSkua: Load)")
        .clicked()
        {
            self.confirm_revert = true;
        }
        if self.busy {
            ui.spinner();
        } else if let Some(at) = self.saved_at {
            if at.elapsed() < SAVED_SHOWN_FOR {
                ui.label(RichText::new("Saved").size(12.0).color(SUCCESS));
                ui.ctx().request_repaint_after(SAVED_SHOWN_FOR - at.elapsed());
            } else {
                self.saved_at = None;
            }
        }
    }

    /// The tabs; on Options and Other, the search on the right.
    fn tab_row(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::Loadout, "Loadout"),
                (Tab::Options, "Options"),
                (Tab::Other, "Other"),
            ] {
                if widgets::chip(ui, label, self.tab == tab) {
                    self.tab = tab;
                }
            }
            if self.tab != Tab::Loadout {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let width = ui.available_width().min(220.0);
                    if width >= 100.0 {
                        widgets::text_box(ui, &mut self.search, "Search options", width);
                    }
                });
            }
        });
    }

    /// One row per role: its class, skill preset and dedicated gear switch, with the role's gear
    /// under it while the switch is on. Wide, the rows line up as a table under column names;
    /// narrow (a phone), each row stacks its name, then its class and preset.
    fn loadout_tab(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .id_salt("cbo_loadout")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                let width = ui.available_width();
                let columns = (width >= LOADOUT_WIDE).then(|| RoleColumns::new(width - 2.0 * ROW_PAD.x));
                if let Some(columns) = &columns {
                    column_names(ui, columns);
                }
                surface(ui, |ui| {
                    for i in 0..self.roles.len() {
                        if i > 0 {
                            hairline(ui);
                        }
                        self.role_row(ui, i, columns.as_ref());
                    }
                });
            });
    }

    fn role_row(&mut self, ui: &mut Ui, i: usize, columns: Option<&RoleColumns>) {
        let draft = &mut self.draft;
        let (sets, current_class, owned) = (&mut draft.sets, draft.current_class.as_deref(), &draft.owned);
        let role = &mut draft.roles[i];
        Frame::new()
            .inner_margin(Margin::symmetric(ROW_PAD.x as i8, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 8.0;
                match columns {
                    Some(c) => {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = COLUMN_GAP;
                            role_name(ui, role, c.name);
                            class_combo(ui, i, role, &owned.classes, sets, current_class, c.class);
                            mode_combo(ui, i, role, c.mode);
                            gear_switch(ui, role);
                        });
                        role_gear(ui, i, role, &owned.gear, c.name + COLUMN_GAP, 3);
                    }
                    None => {
                        ui.horizontal(|ui| {
                            role_name(ui, role, 160.0);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                gear_switch(ui, role);
                                ui.label(
                                    RichText::new("Dedicated gear")
                                        .size(widgets::small_label_size())
                                        .color(MUTED),
                                );
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = COLUMN_GAP;
                            let mode = 104.0;
                            let class = (ui.available_width() - mode - COLUMN_GAP).max(80.0);
                            class_combo(ui, i, role, &owned.classes, sets, current_class, class);
                            mode_combo(ui, i, role, mode);
                        });
                        role_gear(ui, i, role, &owned.gear, 0.0, 2);
                    }
                }
            });
    }

    /// The Options tab: its options by what they're about, in [`OPTION_GROUPS`]' order.
    fn options_tab(&mut self, ui: &mut Ui) {
        let needle = self.search.trim().to_lowercase();
        let options = &mut self.draft.options;
        ScrollArea::vertical()
            .id_salt("cbo_options")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut shown = 0;
                for (title, tags) in OPTION_GROUPS {
                    let mut group: Vec<&mut Opt> = options
                        .iter_mut()
                        .filter(|o| tags.contains(&o.tag) && matches(o, &needle))
                        .collect();
                    group.sort_by_key(|o| tags.iter().position(|t| *t == o.tag));
                    shown += group.len();
                    option_group(ui, title, group);
                }
                if shown == 0 {
                    no_matches(ui, &self.search);
                }
            });
    }

    fn other_tab(&mut self, ui: &mut Ui) {
        let needle = self.search.trim().to_lowercase();
        let other = &mut self.draft.other;
        ScrollArea::vertical()
            .id_salt("cbo_other")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut shown = 0;
                for (category, opts) in other.iter_mut() {
                    let group: Vec<&mut Opt> = opts.iter_mut().filter(|o| matches(o, &needle)).collect();
                    shown += group.len();
                    option_group(ui, category, group);
                }
                if shown == 0 {
                    no_matches(ui, &self.search);
                }
            });
    }

    /// VibeSkua's message boxes: the public room warning and the Load/Save messages.
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_public {
            // Turu: Game Options' public room question (VibeSkua's CoreBots asks in its own words).
            let answer = super::game_options::warning_modal(ctx, egui::Id::new("cbo_public_rooms"), &PUBLIC_ROOMS);
            match answer {
                // Escape or a click outside: no save, as on Android.
                Some(Answer::Dismiss) => self.confirm_public = false,
                Some(answer) => {
                    self.confirm_public = false;
                    // The answer holds for this character until the window saves a different one.
                    let yes = answer == Answer::Confirm;
                    *lock(&PUBLIC_CONFIRMED) = yes.then(|| self.username.clone());
                    self.draft.fetch(&crate::turu::bot(), Purpose::Save);
                }
                None => {}
            }
        }
        if self.confirm_revert {
            let mut answer = None;
            Modal::new(egui::Id::new("cbo_revert")).show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.heading("Revert to saved?");
                ui.label("Your unsaved changes to this character's options will be lost.");
                ui.horizontal(|ui| {
                    if widgets::button(ui, "Revert", Kind::Danger, true, vec2(84.0, 28.0), "").clicked() {
                        answer = Some(true);
                    }
                    if widgets::button(ui, "Keep editing", Kind::Normal, true, vec2(110.0, 28.0), "").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if let Some(revert) = answer {
                self.confirm_revert = false;
                if revert {
                    self.draft.fetch(&crate::turu::bot(), Purpose::Load);
                }
            }
        }
        if let Some((title, text)) = &self.message {
            let mut close = false;
            let response = Modal::new(egui::Id::new("cbo_message")).show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.heading(*title);
                ui.label(crate::models::core_bots::shown_text(
                    &crate::turu::bot(),
                    &self.username,
                    text,
                ));
                if widgets::button(ui, "OK", Kind::Primary, true, vec2(72.0, 28.0), "").clicked() {
                    close = true;
                }
            });
            if close || response.should_close() {
                self.message = None;
            }
        }
    }
}

/// The header's buttons with "Saved" beside them: Save, Refresh from game, Revert to saved.
const HEADER_BUTTONS_WIDTH: f32 = 84.0 + 140.0 + 120.0 + 3.0 * 8.0 + 44.0;
/// Below this much room left for the name and state, the buttons go on their own row.
const HEADER_TEXT_MIN_WIDTH: f32 = 300.0;

/// How long "Saved" shows next to Save.
const SAVED_SHOWN_FOR: Duration = Duration::from_secs(3);

/// A row's padding inside its surface.
const ROW_PAD: egui::Vec2 = vec2(12.0, 10.0);
/// The space between the Loadout's columns.
const COLUMN_GAP: f32 = 12.0;
/// From this width the Loadout is a table; below it, each role stacks.
const LOADOUT_WIDE: f32 = 600.0;
/// The width of a number's box and of a text option's box.
const NUMBER_WIDTH: f32 = 96.0;
const TEXT_WIDTH: f32 = 150.0;
/// About the width of a Reset button.
const RESET_WIDTH: f32 = 50.0;

/// The Loadout table's column widths; the class gets what the others leave.
struct RoleColumns {
    name: f32,
    class: f32,
    mode: f32,
    gear: f32,
}

impl RoleColumns {
    fn new(width: f32) -> Self {
        let (name, mode, gear) = (128.0, 120.0, 100.0);
        let class = (width - name - mode - gear - 3.0 * COLUMN_GAP).max(120.0);
        Self {
            name,
            class,
            mode,
            gear,
        }
    }
}

/// The names over the Loadout's columns, lined up with the rows' cells.
fn column_names(ui: &mut Ui, c: &RoleColumns) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
    let mut x = rect.left() + ROW_PAD.x;
    for (text, width) in [
        ("Role", c.name),
        ("Class", c.class),
        ("Skill preset", c.mode),
        ("Dedicated gear", c.gear),
    ] {
        ui.painter().text(
            egui::pos2(x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::proportional(widgets::small_label_size()),
            MUTED,
        );
        x += width + COLUMN_GAP;
    }
}

/// The role's dot and name with what it is for under it, `width` wide.
fn role_name(ui: &mut Ui, role: &Role, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 32.0), Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(
        egui::pos2(rect.left() + 4.0, rect.top() + 8.0),
        4.0,
        egui::Color32::from_rgb((role.color >> 16) as u8, (role.color >> 8) as u8, role.color as u8),
    );
    painter.text(
        egui::pos2(rect.left() + 14.0, rect.top() + 8.0),
        egui::Align2::LEFT_CENTER,
        role.name,
        egui::FontId::proportional(13.0),
        TEXT,
    );
    painter.text(
        egui::pos2(rect.left() + 14.0, rect.top() + 24.0),
        egui::Align2::LEFT_CENTER,
        role.purpose,
        egui::FontId::proportional(widgets::small_label_size()),
        MUTED,
    );
}

fn class_combo(
    ui: &mut Ui,
    i: usize,
    role: &mut Role,
    classes: &[String],
    sets: &mut SkillSets,
    current_class: Option<&str>,
    width: f32,
) {
    let mut picked = None;
    let selected = match role.class.as_deref() {
        Some(class) if !class.is_empty() => RichText::new(class),
        _ => RichText::new("Not set").color(MUTED),
    };
    ComboBox::from_id_salt(("cbo_class", i))
        .selected_text(selected)
        .width(width)
        .truncate()
        .show_ui(ui, |ui| {
            for class in classes {
                if ui.selectable_label(role.class.as_ref() == Some(class), class).clicked() {
                    picked = Some(class.clone());
                }
            }
        })
        .response
        .on_hover_text(format!(
            "The class CoreBots equips for {} ({})",
            role.name,
            role.purpose.to_lowercase()
        ));
    if let Some(class) = picked {
        role.select_class(class, sets, current_class);
    }
}

fn mode_combo(ui: &mut Ui, i: usize, role: &mut Role, width: f32) {
    ComboBox::from_id_salt(("cbo_mode", i))
        .selected_text(role.mode.as_deref().unwrap_or_default())
        .width(width)
        .truncate()
        .show_ui(ui, |ui| {
            for mode in &role.modes {
                if ui.selectable_label(role.mode.as_ref() == Some(mode), mode).clicked() {
                    role.mode = Some(mode.clone());
                }
            }
        })
        .response
        .on_hover_text("The skill set the class uses (VibeSkua: Skill Rule / Preset)");
}

fn gear_switch(ui: &mut Ui, role: &mut Role) {
    widgets::switch(ui, &mut role.use_equipment).on_hover_text(
        "Wear this role's own gear with its class, picked under the row (VibeSkua: Use Dedicated \
         Equipment Loadout)",
    );
}

/// The role's six gear slots, `per_row` to a row and `indent` in, while its dedicated gear is
/// on; while it's off, how many picks it keeps.
fn role_gear(ui: &mut Ui, i: usize, role: &mut Role, lists: &[Vec<String>; 6], indent: f32, per_row: usize) {
    if !role.use_equipment {
        let kept = role.gear.iter().filter(|g| !g.is_empty()).count();
        if kept > 0 {
            ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                ui.add_space(indent);
                ui.label(
                    RichText::new(format!(
                        "{kept} gear pick{} kept, worn when Dedicated gear is on",
                        if kept == 1 { "" } else { "s" }
                    ))
                    .size(widgets::small_label_size())
                    .color(MUTED),
                );
            });
        }
        return;
    }
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.add_space(indent);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(COLUMN_GAP, 8.0);
            let cell = (ui.available_width() - COLUMN_GAP * (per_row - 1) as f32) / per_row as f32;
            for row in (0..SLOTS.len()).collect::<Vec<_>>().chunks(per_row) {
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    for &slot in row {
                        ui.allocate_ui_with_layout(vec2(cell, 0.0), Layout::top_down(Align::Min), |ui| {
                            ui.set_width(cell);
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.label(
                                RichText::new(SLOTS[slot].0)
                                    .size(widgets::small_label_size())
                                    .color(MUTED),
                            );
                            gear_combo(ui, (i, slot), &mut role.gear[slot], &lists[slot], cell);
                        });
                    }
                });
            }
        });
    });
}

fn gear_combo(ui: &mut Ui, id: (usize, usize), current: &mut String, list: &[String], width: f32) {
    let selected = if current.is_empty() {
        RichText::new("None").color(MUTED)
    } else {
        RichText::new(current.as_str())
    };
    ComboBox::from_id_salt(("cbo_gear", id))
        .selected_text(selected)
        .width(width)
        .truncate()
        .show_ui(ui, |ui| {
            // Turu: an empty entry clears the slot (VibeSkua leaves it blank).
            if ui.selectable_label(current.is_empty(), "None").clicked() {
                current.clear();
            }
            for name in list {
                if ui.selectable_label(current == name, name).clicked() {
                    *current = name.clone();
                }
            }
        });
}

/// Whether the option's label or description holds `needle` (lowercase).
fn matches(opt: &Opt, needle: &str) -> bool {
    needle.is_empty() || opt.label.to_lowercase().contains(needle) || opt.description.to_lowercase().contains(needle)
}

fn no_matches(ui: &mut Ui, search: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(format!("No options match \"{}\".", search.trim())).color(MUTED));
}

/// The rounded surface a group's rows sit on, edge to edge.
fn surface<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(CornerRadius::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            add_contents(ui)
        })
        .inner
}

/// The line between two rows of a surface.
fn hairline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, EDGE));
}

/// A heading and its options on one surface; nothing when no option is left.
fn option_group(ui: &mut Ui, title: &str, opts: Vec<&mut Opt>) {
    if opts.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(13.0).strong().color(TEXT));
    ui.add_space(-2.0);
    surface(ui, |ui| {
        for (n, opt) in opts.into_iter().enumerate() {
            if n > 0 {
                hairline(ui);
            }
            option_row(ui, opt);
        }
    });
    ui.add_space(8.0);
}

/// One option: its label and the whole of its description on the left, its control on the
/// right. A click anywhere on a switch's row flips it. A number or text that isn't the default
/// gets a Reset beside it (a switch's other value is one click away).
fn option_row(ui: &mut Ui, opt: &mut Opt) {
    let is_switch = matches!(opt.value, Value::Bool(_));
    let show_reset = !is_switch && opt.value != opt.default;
    let control = match opt.value {
        Value::Bool(_) => 34.0,
        Value::Int(_) => NUMBER_WIDTH,
        Value::Text(_) => TEXT_WIDTH,
    } + if show_reset { RESET_WIDTH + 8.0 } else { 0.0 };
    let width = ui.available_width();
    let text_width = (width - 2.0 * ROW_PAD.x - control - 20.0).max(80.0);
    let label = ui
        .painter()
        .layout(opt.label.to_owned(), egui::FontId::proportional(13.0), TEXT, text_width);
    let description = (!opt.description.is_empty()).then(|| {
        ui.painter().layout(
            opt.description.to_owned(),
            egui::FontId::proportional(widgets::small_label_size()),
            MUTED,
            text_width,
        )
    });
    let text_height = label.size().y + description.as_ref().map_or(0.0, |d| 3.0 + d.size().y);
    let height = (text_height.max(24.0) + 2.0 * ROW_PAD.y).max(widgets::target(0.0, 52.0));
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let row = ui.interact(
        rect,
        ui.make_persistent_id(("cbo_row", opt.tag)),
        if is_switch { Sense::click() } else { Sense::hover() },
    );
    if is_switch && row.hovered() {
        ui.painter()
            .rect_filled(rect.shrink(3.0), CornerRadius::same(7), HOVER.gamma_multiply(0.7));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut top = rect.center().y - text_height / 2.0;
    let left = rect.left() + ROW_PAD.x;
    let label_height = label.size().y;
    ui.painter().galley(egui::pos2(left, top), label, TEXT);
    if let Some(description) = description {
        top += label_height + 3.0;
        ui.painter().galley(egui::pos2(left, top), description, MUTED);
    }

    let slot = egui::Rect::from_min_max(
        egui::pos2(rect.right() - ROW_PAD.x - control, rect.top()),
        egui::pos2(rect.right() - ROW_PAD.x, rect.bottom()),
    );
    let ui = &mut ui.new_child(
        UiBuilder::new()
            .id_salt(opt.tag)
            .max_rect(slot)
            .layout(Layout::right_to_left(Align::Center)),
    );
    match &mut opt.value {
        Value::Bool(on) => {
            if row.clicked() {
                *on = !*on;
            }
            widgets::switch(ui, on);
        }
        Value::Int(n) => {
            let speed = (n.unsigned_abs() as f64 / 200.0).max(1.0);
            let suffix = if opt.unit.is_empty() {
                String::new()
            } else {
                format!(" {}", opt.unit)
            };
            let size = vec2(NUMBER_WIDTH, widgets::target(26.0, 40.0));
            widgets::number(ui, opt.tag, n, 0..=i32::MAX, speed, &suffix, size);
        }
        Value::Text(text) => {
            let mut response = ui.add(
                egui::TextEdit::singleline(&mut *text)
                    .desired_width(TEXT_WIDTH - 16.0)
                    .min_size(vec2(0.0, widgets::target(26.0, 40.0)))
                    .vertical_align(Align::Center)
                    .margin(Margin::symmetric(8, 4))
                    .background_color(INSET),
            );
            super::native_field::follow(ui, &mut response, text, opt.tag, super::native_field::Kind::Line);
        }
    }
    if show_reset {
        let default = match &opt.default {
            Value::Int(n) if !opt.unit.is_empty() => format!("{n} {}", opt.unit),
            other => other.to_file(),
        };
        if widgets::small_button(ui, "Reset", true)
            .on_hover_text(format!("Back to the default, {default}"))
            .clicked()
        {
            opt.value = opt.default.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The header tells a fresh file, saved values and unsaved changes apart, and logged out
    /// asks for a login.
    #[test]
    fn header_says_whether_the_options_are_saved() {
        let mut window = CoreBotsWindow::default();
        assert!(window.save_state().0.starts_with("Not logged in"));
        window.username = "Hero".into();
        window.baseline = Some(window.lines());
        assert_eq!(window.save_state().0, "CoreBots' defaults · not saved yet");
        window.has_file = true;
        assert_eq!(window.save_state().0, "Saved");
        window.options[0].value = Value::Bool(false);
        assert!(window.has_changes());
        assert!(window.save_state().0.starts_with("Unsaved changes"));
    }

    /// Every option says what it does, and the delays say they're milliseconds.
    #[test]
    fn every_option_is_described() {
        let window = CoreBotsWindow::default();
        let all = window
            .options
            .iter()
            .chain(window.other.iter().flat_map(|(_, opts)| opts));
        for opt in all {
            assert!(!opt.description.is_empty(), "{} has no description", opt.label);
            if opt.tag.ends_with("DelayNr") || opt.tag == "ExitCombatNr" {
                assert_eq!(opt.unit, "ms", "{}", opt.label);
            }
        }
    }

    /// The Options tab shows every option once, in its group.
    #[test]
    fn every_option_is_in_one_group() {
        let window = CoreBotsWindow::default();
        for opt in &window.options {
            let groups = OPTION_GROUPS.iter().filter(|(_, tags)| tags.contains(&opt.tag)).count();
            assert_eq!(groups, 1, "{} is in {groups} groups", opt.tag);
        }
        let grouped: usize = OPTION_GROUPS.iter().map(|(_, tags)| tags.len()).sum();
        assert_eq!(
            grouped,
            window.options.len(),
            "a group names an option that doesn't exist"
        );
    }

    /// Refreshing the class and gear lists keeps each role's picks (VibeSkua's Reload Classes
    /// cleared them), so it changes nothing to save.
    #[test]
    fn refresh_keeps_the_picks() {
        let mut window = CoreBotsWindow::default();
        window.username = "Hero".into();
        window.roles[1].select_class("Legion Revenant".into(), &mut SkillSets::default(), None);
        window.roles[1].mode = Some("Farm".into());
        window.roles[1].gear[3] = "Necrotic Sword of Doom".into();
        window.baseline = Some(window.lines());
        window.refresh_keeping_picks();
        assert_eq!(window.roles[1].class.as_deref(), Some("Legion Revenant"));
        assert_eq!(window.roles[1].mode.as_deref(), Some("Farm"));
        assert_eq!(window.roles[1].gear[3], "Necrotic Sword of Doom");
        assert!(!window.has_changes());
    }

    /// A long name and state are cut, and in a narrow window (a phone's) the buttons go on a row
    /// of their own: the text never runs under the buttons, and keeps room to be read.
    #[test]
    fn header_fits_narrow_windows() {
        for width in [800.0, 440.0] {
            let ctx = egui::Context::default();
            let mut window = CoreBotsWindow::default();
            window.username = "AVeryLongCharacterNameThatGoesOnAndOnForeverAndEver".into();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 1000.0))),
                ..Default::default()
            };
            let mut placed = None;
            let mut output = ctx.run_ui(input, |ui| {
                ui.allocate_ui(vec2(width, 200.0), |ui| {
                    ui.set_width(width);
                    placed = Some((window.header(ui), ui.max_rect()));
                });
            });
            output.textures_delta.clear();
            let ((text, buttons), area) = placed.unwrap();
            assert!(
                !text.intersects(buttons),
                "at {width}: text {text:?} under buttons {buttons:?}"
            );
            assert!(
                text.width() >= 200.0,
                "at {width}: the text has only {} px",
                text.width()
            );
            assert!(buttons.right() <= area.right() + 1.0 && text.right() <= area.right() + 1.0);
        }
    }

    /// The window keeps its width with long class and item names (egui windows grow to fit
    /// content that asks for more than it was given).
    #[test]
    fn loadout_fits_the_window() {
        let ctx = egui::Context::default();
        let mut window = CoreBotsWindow::default();
        let long = "Legendary Elemental Warrior of the Very Long Item Name Showcase".to_owned();
        window.owned.classes.push(long.clone());
        for list in &mut window.owned.gear {
            list.push(long.clone());
        }
        for role in &mut window.roles {
            role.class = Some(long.clone());
            role.mode = Some("Base".into());
            role.gear = std::array::from_fn(|_| long.clone());
        }
        for role in &mut window.roles {
            role.use_equipment = true;
        }
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 1000.0))),
            ..Default::default()
        };
        let mut widths = Vec::new();
        for tab in [Tab::Loadout, Tab::Options, Tab::Other] {
            window.tab = tab;
            for _ in 0..8 {
                let mut output = ctx.run_ui(input(), |ui| {
                    let ctx = ui.ctx().clone();
                    let shown = egui::Window::new("CoreBots Options")
                        .default_size(vec2(800.0, 608.0))
                        .resizable(true)
                        .show(&ctx, |ui| window.ui(ui));
                    widths.push(shown.unwrap().response.rect.width());
                });
                output.textures_delta.clear();
            }
        }
        let first = widths[0];
        assert!(widths.iter().all(|w| *w <= first + 1.0), "the window grew: {widths:?}");
        assert!(first < 840.0, "the window is wider than its default: {first}");
    }
}
