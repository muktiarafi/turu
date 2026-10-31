//! Combat > Skills (VibeSkua `AdvancedSkillsView`, "Advanced Skills"): "Create / Edit Skill"
//! builds a skill sequence with its use rules and saves it as one class's set in one mode;
//! "Saved Skills" lists the sets in `UserAdvancedSkills.json` and opens, edits, removes or
//! resets them.

use super::theme::{DANGER, INSET, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use egui::{ComboBox, Context, CornerRadius, DragValue, Frame, Margin, Modal, RichText, ScrollArea, Ui, vec2};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use turu_bot::skill_editor::{self, AuraRow, SkillItem, SkillRules};
use turu_bot::skills::{self as bot_skills, AdvancedSkill, CLASS_USE_MODES, SkillUseMode};

/// How long Reset Sets stays armed for its second click.
const CONFIRM_FOR: Duration = Duration::from_secs(4);

/// The saved skill sets by class and mode, for the windows that pick a class's mode (Auto,
/// CoreBots Options).
#[derive(Default, Clone)]
pub struct SkillSets {
    classes: BTreeMap<String, Vec<String>>,
}

impl SkillSets {
    pub fn load() -> Self {
        Self::from_sets(&bot_skills::load_advanced_skills(&crate::turu::bot().paths))
    }

    fn from_sets(sets: &[AdvancedSkill]) -> Self {
        let mut classes: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for set in sets {
            classes
                .entry(set.class_name.clone())
                .or_default()
                .push(set.mode.clone());
        }
        for modes in classes.values_mut() {
            modes.sort();
            modes.dedup();
        }
        Self { classes }
    }

    pub fn classes(&self) -> impl Iterator<Item = &String> {
        self.classes.keys()
    }

    pub fn modes(&self, class: &str) -> Vec<String> {
        self.classes.get(class).cloned().unwrap_or_default()
    }

    /// The saved modes of `class`, sorted; the class name is matched ignoring case, since
    /// the inventory and the skill file may spell it differently.
    pub fn modes_for(&self, class: &str) -> Vec<String> {
        self.classes
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(class))
            .map(|(_, modes)| modes.clone())
            .unwrap_or_default()
    }
}

/// The "Create / Edit Skill" tab (VibeSkua `AdvancedSkillEditorViewModel`).
struct Editor {
    timeout: i64,
    wait_for_cooldown: bool,
    reset_on_target: bool,
    skills: Vec<SkillItem>,
    selected: Option<usize>,
    class_name: String,
    /// Index in [`CLASS_USE_MODES`].
    mode: usize,
    /// The rules the slot buttons give a new skill.
    rules: SkillRules,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            timeout: 100,
            wait_for_cooldown: false,
            reset_on_target: false,
            skills: Vec::new(),
            selected: None,
            class_name: String::new(),
            mode: 0,
            rules: SkillRules::default(),
        }
    }
}

impl Editor {
    /// VibeSkua `EditSkill(message)`: a saved set into the editor (its Use Rules stay).
    fn open(&mut self, set: &AdvancedSkill) {
        self.timeout = set.skill_timeout;
        self.wait_for_cooldown = set.skill_use_mode != SkillUseMode::UseIfAvailable;
        self.reset_on_target = set.reset_on_target;
        self.class_name = set.class_name.clone();
        self.mode = CLASS_USE_MODES
            .iter()
            .position(|m| m.eq_ignore_ascii_case(&set.mode))
            .unwrap_or(0);
        self.skills = skill_editor::parse_sequence(&set.skills);
        self.selected = None;
    }

    /// VibeSkua `SaveSkills`: the set the editor describes.
    fn to_set(&self) -> AdvancedSkill {
        AdvancedSkill {
            class_name: self.class_name.trim().to_owned(),
            mode: CLASS_USE_MODES[self.mode].to_owned(),
            skills: skill_editor::sequence_string(&self.skills),
            skill_timeout: self.timeout,
            skill_use_mode: if self.wait_for_cooldown {
                SkillUseMode::WaitForCooldown
            } else {
                SkillUseMode::UseIfAvailable
            },
            reset_on_target: self.reset_on_target,
        }
    }

    /// VibeSkua `MoveSkill`: with nothing selected it selects the first skill.
    fn move_selected(&mut self, down: bool) {
        let Some(index) = self.selected else {
            self.selected = (!self.skills.is_empty()).then_some(0);
            return;
        };
        let Some(to) = (if down {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        }) else {
            return;
        };
        if to < self.skills.len() {
            self.skills.swap(index, to);
            self.selected = Some(to);
        }
    }

    /// VibeSkua `RemoveSkill`: the skill above becomes the selection.
    fn remove_selected(&mut self) {
        if let Some(index) = self.selected.filter(|&i| i < self.skills.len()) {
            self.skills.remove(index);
            self.selected = index.checked_sub(1);
        }
    }
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Tab {
    #[default]
    Editor,
    Saved,
}

#[derive(Default)]
pub struct SkillsWindow {
    tab: Tab,
    loaded: bool,
    /// The saved sets in file order (VibeSkua `LoadedSkills`).
    sets: Vec<AdvancedSkill>,
    /// "Load Classes": the class drop-down's list (VibeSkua `AvailableClasses`).
    classes: Vec<String>,
    class: Option<String>,
    mode: Option<String>,
    /// The Saved Skills row selected (an index in `sets`).
    selected: Option<usize>,
    editor: Editor,
    /// Edit Rules: the skill being edited and a copy of its rules.
    editing: Option<(usize, SkillRules)>,
    /// The last save, remove or reset, and whether it failed.
    outcome: Option<(String, bool)>,
    reset_armed: Option<Instant>,
}

impl SkillsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            self.loaded = false;
            self.editing = None;
            return;
        }
        if !self.loaded {
            self.reload();
            if self.classes.is_empty() {
                self.load_classes();
            }
            self.loaded = true;
        }
        if self.reset_armed.is_some_and(|at| at.elapsed() >= CONFIRM_FOR) {
            self.reset_armed = None;
        }
        super::window(ctx, "Advanced Skills", [700.0, 530.0], 1)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        self.rules_dialog(ctx);
        if self.reset_armed.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// Reads the saved sets again (VibeSkua `LoadSkills`).
    fn reload(&mut self) {
        self.sets = bot_skills::load_advanced_skills(&crate::turu::bot().paths);
        self.selected = None;
    }

    /// VibeSkua `LoadAvailableClasses`: the saved classes, sorted.
    fn load_classes(&mut self) {
        self.classes = SkillSets::from_sets(&self.sets).classes().cloned().collect();
        if self.class.as_ref().is_some_and(|c| !self.classes.contains(c)) {
            self.class = None;
            self.mode = None;
        }
    }

    /// Writes the sets and reads them back, noting `done` (or the error).
    fn save(&mut self, done: String) {
        let paths = &crate::turu::bot().paths;
        self.outcome = Some(match bot_skills::save_advanced_skills(paths, &self.sets) {
            Ok(()) => (done, false),
            Err(e) => (format!("Couldn't save the skill sets: {e}"), true),
        });
        self.reload();
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if widgets::chip(ui, "Create / Edit Skill", self.tab == Tab::Editor) {
                self.tab = Tab::Editor;
            }
            if widgets::chip(ui, "Saved Skills", self.tab == Tab::Saved) {
                self.tab = Tab::Saved;
            }
        });
        ui.add_space(6.0);
        ScrollArea::vertical()
            .id_salt("skills_tab")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.tab {
                Tab::Editor => self.editor_tab(ui),
                Tab::Saved => self.saved_tab(ui),
            });
    }

    fn editor_tab(&mut self, ui: &mut Ui) {
        ui.columns(2, |cols| {
            self.sequence_column(&mut cols[0]);
            self.rules_column(&mut cols[1]);
        });
        self.outcome_line(ui);
    }

    fn sequence_column(&mut self, ui: &mut Ui) {
        let e = &mut self.editor;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "SKILL TIMEOUT");
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut e.timeout).range(0..=5000).speed(10));
                ui.label(RichText::new("Timeout (ms)").color(MUTED));
            });
        });
        ui.add_space(6.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "SKILLS SEQUENCE");
            ui.add_space(4.0);
            list_frame(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("skills_sequence")
                    .auto_shrink([false, false])
                    .min_scrolled_height(160.0)
                    .max_height(160.0)
                    .show(ui, |ui| {
                        if e.skills.is_empty() {
                            ui.label(RichText::new("Add skills with the 0 - 5 buttons.").color(MUTED));
                        }
                        for (i, item) in e.skills.iter().enumerate() {
                            let text = RichText::new(item.display()).color(TEXT);
                            let row = ui.add_sized(
                                [ui.available_width(), 22.0],
                                egui::Button::selectable(e.selected == Some(i), text),
                            );
                            if row.clicked() {
                                e.selected = Some(i);
                            }
                            if row.double_clicked() {
                                self.editing = Some((i, item.rules.clone()));
                            }
                        }
                    });
            });
            ui.add_space(4.0);
            let has_selection = e.selected.is_some_and(|i| i < e.skills.len());
            ui.horizontal(|ui| {
                let size = vec2(28.0, 24.0);
                if widgets::button(ui, "↑", Kind::Normal, true, size, "")
                    .on_hover_text("Move Skill Up")
                    .clicked()
                {
                    e.move_selected(false);
                }
                if widgets::button(ui, "↓", Kind::Normal, true, size, "")
                    .on_hover_text("Move Skill Down")
                    .clicked()
                {
                    e.move_selected(true);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, "✕", Kind::Normal, has_selection, size, "Select a skill")
                        .on_hover_text("Remove Selected Skill")
                        .clicked()
                    {
                        e.remove_selected();
                    }
                    if widgets::button(
                        ui,
                        "Edit",
                        Kind::Normal,
                        has_selection,
                        vec2(0.0, 24.0),
                        "Select a skill",
                    )
                    .on_hover_text("Edit Selected Skill's Rules")
                    .clicked()
                        && let Some(i) = e.selected
                    {
                        self.editing = Some((i, e.skills[i].rules.clone()));
                    }
                });
            });
        });
        ui.add_space(6.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::text_box(
                ui,
                &mut e.class_name,
                "Class Name (e.g. Void Highlord)",
                ui.available_width(),
            );
        });
    }

    fn rules_column(&mut self, ui: &mut Ui) {
        let e = &mut self.editor;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "USE MODE");
            ui.horizontal_wrapped(|ui| {
                let mut wait = e.wait_for_cooldown;
                let mut available = !wait;
                if ui
                    .checkbox(&mut wait, "Wait for Cooldown")
                    .on_hover_text("Will wait for either the skill to fully cooldown or the defined skill timeout.")
                    .changed()
                {
                    e.wait_for_cooldown = wait;
                }
                if ui
                    .checkbox(&mut available, "Use if available")
                    .on_hover_text("Whenever a skill is available it will be used, if not it will be skipped.")
                    .changed()
                {
                    e.wait_for_cooldown = !available;
                }
                ui.checkbox(&mut e.reset_on_target, "Reset on Target Change")
                    .on_hover_text("Reset skill combo sequence when the target changes or is lost.");
            });
        });
        ui.add_space(6.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            rules_form(ui, &mut e.rules, "skills_rules");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let size = vec2(36.0, 32.0);
                let width = 6.0 * size.x + 5.0 * ui.spacing().item_spacing.x;
                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                for slot in 0..=5 {
                    let hint = match slot {
                        0 => "Auto attack",
                        5 => "Potion",
                        _ => "Skill",
                    };
                    if widgets::button(ui, &slot.to_string(), Kind::Normal, true, size, "")
                        .on_hover_text(format!("{hint}: add it with these rules"))
                        .clicked()
                    {
                        e.skills.push(SkillItem::new(slot, &e.rules));
                    }
                }
            });
        });
        ui.add_space(6.0);
        let mut save = false;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let save_width = 100.0;
                ComboBox::from_id_salt("skills_save_mode")
                    .selected_text(CLASS_USE_MODES[e.mode])
                    .width((ui.available_width() - save_width - 8.0).max(80.0))
                    .show_ui(ui, |ui| {
                        for (i, mode) in CLASS_USE_MODES.iter().enumerate() {
                            ui.selectable_value(&mut e.mode, i, *mode);
                        }
                    });
                let named = !e.class_name.trim().is_empty();
                save = widgets::button(
                    ui,
                    "Save Skill",
                    Kind::Primary,
                    named,
                    vec2(save_width, 28.0),
                    "Enter a class name",
                )
                .clicked();
            });
        });
        if save {
            let set = self.editor.to_set();
            let done = format!("Saved {} ({}).", set.class_name, set.mode);
            bot_skills::override_skill_set(&mut self.sets, set);
            self.save(done);
            self.load_classes();
        }
    }

    fn saved_tab(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if widgets::button(ui, "Load Classes", Kind::Normal, true, vec2(0.0, 26.0), "").clicked() {
                    self.reload();
                    self.load_classes();
                }
                let open_width = 60.0;
                let combo_width = ((ui.available_width() - open_width - 3.0 * 8.0) / 2.0).max(90.0);
                ComboBox::from_id_salt("skills_class")
                    .selected_text(self.class.as_deref().unwrap_or("Select Class..."))
                    .width(combo_width)
                    .show_ui(ui, |ui| {
                        for class in &self.classes {
                            if ui.selectable_label(self.class.as_ref() == Some(class), class).clicked()
                                && self.class.as_ref() != Some(class)
                            {
                                self.class = Some(class.clone());
                                self.mode = None;
                            }
                        }
                    });
                let modes = self
                    .class
                    .as_deref()
                    .map(|c| SkillSets::from_sets(&self.sets).modes(c))
                    .unwrap_or_default();
                ComboBox::from_id_salt("skills_mode")
                    .selected_text(self.mode.as_deref().unwrap_or("Select Mode..."))
                    .width(combo_width)
                    .show_ui(ui, |ui| {
                        for mode in modes {
                            let selected = self.mode.as_deref() == Some(&mode);
                            if ui.selectable_label(selected, &mode).clicked() {
                                self.mode = Some(mode);
                            }
                        }
                    });
                let chosen = self.class.is_some() && self.mode.is_some();
                if widgets::button(
                    ui,
                    "Open",
                    Kind::Primary,
                    chosen,
                    vec2(open_width, 26.0),
                    "Select a class and a mode",
                )
                .clicked()
                {
                    self.open_chosen();
                }
            });
            ui.add_space(6.0);
            let mut edit = None;
            list_frame(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("skills_saved")
                    .auto_shrink([false, false])
                    .min_scrolled_height(260.0)
                    .max_height((ui.available_height() - 60.0).max(260.0))
                    .show(ui, |ui| {
                        if self.sets.is_empty() {
                            ui.label(RichText::new("No saved skill sets found.").color(MUTED));
                        }
                        for (i, set) in self.sets.iter().enumerate() {
                            let text = RichText::new(set.save_string()).color(TEXT);
                            let row = ui.add_sized(
                                [ui.available_width(), 22.0],
                                egui::Button::selectable(self.selected == Some(i), text).truncate(),
                            );
                            if row.clicked() {
                                self.selected = Some(i);
                            }
                            if row.double_clicked() {
                                edit = Some(i);
                            }
                            row.on_hover_text(set.save_string());
                        }
                    });
            });
            ui.add_space(6.0);
            let has_selection = self.selected.is_some_and(|i| i < self.sets.len());
            ui.horizontal(|ui| {
                let size = vec2(0.0, 26.0);
                if widgets::button(ui, "Edit Selected", Kind::Normal, has_selection, size, "Select a set").clicked() {
                    edit = self.selected;
                }
                if widgets::button(ui, "Remove Selected", Kind::Normal, has_selection, size, "Select a set").clicked()
                    && let Some(i) = self.selected
                {
                    let set = self.sets.remove(i);
                    self.save(format!("Removed {} ({}).", set.class_name, set.mode));
                    self.load_classes();
                }
                if widgets::button(ui, "Refresh", Kind::Normal, true, size, "").clicked() {
                    self.reload();
                }
                let armed = self.reset_armed.is_some();
                let (label, kind) = if armed {
                    ("Reset all sets?", Kind::Danger)
                } else {
                    ("Reset Sets", Kind::Outline)
                };
                if widgets::button(ui, label, kind, true, size, "")
                    .on_hover_text("Replace your skill sets with the shipped ones: click twice")
                    .clicked()
                {
                    if armed {
                        self.reset_armed = None;
                        self.reset();
                    } else {
                        self.reset_armed = Some(Instant::now());
                    }
                }
            });
            if let Some(i) = edit.filter(|&i| i < self.sets.len()) {
                self.selected = Some(i);
                let set = self.sets[i].clone();
                self.edit(&set);
            }
        });
        self.outcome_line(ui);
    }

    /// VibeSkua `LoadSelectedClassMode`: the chosen class and mode into the editor.
    fn open_chosen(&mut self) {
        let (Some(class), Some(mode)) = (&self.class, &self.mode) else {
            return;
        };
        let found = self
            .sets
            .iter()
            .position(|s| &s.class_name == class && s.mode.eq_ignore_ascii_case(mode));
        if let Some(i) = found {
            self.selected = Some(i);
            let set = self.sets[i].clone();
            self.edit(&set);
        }
    }

    /// VibeSkua's `EditAdvancedSkillMessage`: the set into the editor, which comes to the front.
    fn edit(&mut self, set: &AdvancedSkill) {
        self.editor.open(set);
        self.tab = Tab::Editor;
        self.outcome = None;
    }

    /// VibeSkua `ResetSkillsSets`.
    fn reset(&mut self) {
        let paths = &crate::turu::bot().paths;
        self.outcome = Some(match paths.reset_advanced_skills() {
            Ok(()) => ("Reset the skill sets to the shipped ones.".to_owned(), false),
            Err(e) => (format!("Couldn't reset the skill sets: {e}"), true),
        });
        self.reload();
        self.load_classes();
    }

    fn outcome_line(&self, ui: &mut Ui) {
        if let Some((text, failed)) = &self.outcome {
            ui.add_space(4.0);
            ui.label(
                RichText::new(text)
                    .small()
                    .color(if *failed { DANGER } else { SUCCESS }),
            );
        }
    }

    /// VibeSkua's "Edit Rules" dialog for the selected skill: Confirm keeps the changes.
    fn rules_dialog(&mut self, ctx: &Context) {
        let Some((index, rules)) = &mut self.editing else {
            return;
        };
        let index = *index;
        let mut confirm = false;
        let mut cancel = false;
        let response = Modal::new(egui::Id::new("turu_skill_rules")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Edit Rules");
            ui.label(
                RichText::new("Edit rule values then click Confirm.")
                    .small()
                    .color(MUTED),
            );
            ui.add_space(6.0);
            ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                rules_form(ui, rules, "skills_rules_dialog");
            });
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                confirm = widgets::button(ui, "Confirm", Kind::Primary, true, vec2(84.0, 28.0), "").clicked();
                cancel = widgets::button(ui, "Cancel", Kind::Normal, true, vec2(84.0, 28.0), "").clicked();
            });
        });
        if confirm {
            if let (Some((_, rules)), Some(item)) = (self.editing.take(), self.editor.skills.get_mut(index)) {
                item.rules = rules;
            }
        } else if cancel || response.should_close() {
            self.editing = None;
        }
    }
}

/// The inset frame around a list (`list-frame`).
fn list_frame<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(INSET)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

/// The ≥ / ≤ toggle (`compare`).
fn compare_toggle(ui: &mut Ui, greater: &mut bool) {
    let label = if *greater { "≥" } else { "≤" };
    if widgets::button(ui, label, Kind::Normal, true, vec2(28.0, 22.0), "")
        .on_hover_text("Comparison: ≥ (greater or equal) / ≤ (less or equal)")
        .clicked()
    {
        *greater = !*greater;
    }
}

/// The % / # toggle (`value-type`).
fn value_type_toggle(ui: &mut Ui, percent: &mut bool) {
    let label = if *percent { "%" } else { "#" };
    if widgets::button(ui, label, Kind::Normal, true, vec2(28.0, 22.0), "")
        .on_hover_text("Value Type: % (percentage) / # (flat value)")
        .clicked()
    {
        *percent = !*percent;
    }
}

/// A Self / Target drop-down.
fn target_combo(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, target: &mut bool) {
    ComboBox::from_id_salt(id)
        .selected_text(if *target { "Target" } else { "Self" })
        .width(70.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(target, false, "Self");
            ui.selectable_value(target, true, "Target");
        });
}

/// A health / mana / party HP row: comparison, value, % or #. A percentage stops at 100.
fn amount_row(ui: &mut Ui, label: &str, greater: &mut bool, value: &mut i64, percent: &mut bool) {
    ui.horizontal(|ui| {
        compare_toggle(ui, greater);
        let max = if *percent { 100 } else { i64::from(i32::MAX) };
        ui.add(DragValue::new(value).range(0..=max));
        ui.label(RichText::new(label).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            value_type_toggle(ui, percent);
        });
    });
    if *percent {
        *value = (*value).min(100);
    }
}

/// An aura row: comparison, name, stacks, self or target (and a remove button for a multi-aura
/// check, whose click this returns).
fn aura_row(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    greater: &mut bool,
    name: &mut String,
    stacks: &mut f32,
    target: &mut bool,
    removable: bool,
) -> bool {
    let mut removed = false;
    ui.horizontal(|ui| {
        compare_toggle(ui, greater);
        let extra = if removable { 34.0 } else { 0.0 };
        let name_width = (ui.available_width() - 70.0 - 60.0 - extra - 3.0 * 8.0).max(80.0);
        widgets::text_box(ui, name, "Aura name", name_width);
        ui.add(DragValue::new(stacks).range(0.0..=f32::MAX).speed(0.1).max_decimals(2))
            .on_hover_text("Stacks");
        target_combo(ui, (id, "target"), target);
        if removable {
            removed = widgets::button(ui, "✕", Kind::Normal, true, vec2(26.0, 22.0), "").clicked();
        }
    });
    removed
}

/// The USE RULES card's contents (VibeSkua `SkillRulesViewModel`), shared with Edit Rules.
fn rules_form(ui: &mut Ui, rules: &mut SkillRules, id: &str) {
    ui.horizontal(|ui| {
        widgets::section_label(ui, "USE RULES");
        ui.add_space(6.0);
        ui.checkbox(&mut rules.use_rule, "Enable Rule");
        ui.checkbox(&mut rules.multi_aura, "Multi Aura");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::small_button(ui, "Reset", true).clicked() {
                *rules = SkillRules::default();
            }
        });
    });
    ui.add_space(4.0);
    amount_row(
        ui,
        "Health",
        &mut rules.health_greater,
        &mut rules.health,
        &mut rules.health_percent,
    );
    amount_row(
        ui,
        "Mana",
        &mut rules.mana_greater,
        &mut rules.mana,
        &mut rules.mana_percent,
    );
    amount_row(
        ui,
        "Party HP",
        &mut rules.party_health_greater,
        &mut rules.party_health,
        &mut rules.party_health_percent,
    );
    if rules.multi_aura {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Operator:").small().color(MUTED));
            ComboBox::from_id_salt((id, "operator"))
                .selected_text(if rules.auras_or { "OR" } else { "AND" })
                .width(70.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut rules.auras_or, false, "AND");
                    ui.selectable_value(&mut rules.auras_or, true, "OR");
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::small_button(ui, "Add Aura", true).clicked() {
                    rules.auras.push(AuraRow::default());
                }
            });
        });
        let mut remove = None;
        for (i, check) in rules.auras.iter_mut().enumerate() {
            let AuraRow {
                name,
                stacks,
                greater,
                target,
            } = check;
            if aura_row(ui, (id, "aura", i), greater, name, stacks, target, true) {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            rules.remove_aura(i);
        }
    } else {
        aura_row(
            ui,
            (id, "single"),
            &mut rules.aura_greater,
            &mut rules.aura_name,
            &mut rules.aura_stacks,
            &mut rules.aura_target,
            false,
        );
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.checkbox(&mut rules.skip, "Skip if not available");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("Wait (ms)").color(MUTED));
            ui.add(DragValue::new(&mut rules.wait).range(0..=5000).speed(10));
        });
    });
}
