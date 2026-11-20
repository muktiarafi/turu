//! Combat > Skills (VibeSkua `AdvancedSkillsView`, "Advanced Skills"): "Create / Edit Skill"
//! builds a skill sequence with its use rules and saves it as one class's set in one mode;
//! "Saved Skills" lists the sets in `UserAdvancedSkills.json` and opens, edits, removes or
//! resets them. The editor and the sets are the tab's `models::skills`, shared with a native page.

use super::theme::{DANGER, INSET, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use crate::models::skills::{self as model, Skills};
use crate::models::tab_state::lock;
use egui::{ComboBox, Context, CornerRadius, Frame, Margin, Modal, RichText, ScrollArea, Ui, vec2};
use std::time::{Duration, Instant};
use turu_bot::skill_editor::{AuraRow, SkillRules};
use turu_bot::skills::{AdvancedSkill, CLASS_USE_MODES};

/// How long Reset Sets stays armed for its second click.
const CONFIRM_FOR: Duration = Duration::from_secs(4);

pub use crate::models::auto::SkillSets;

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Tab {
    #[default]
    Editor,
    Saved,
}

/// The window's own state; the rest is the tab's [`Skills`].
#[derive(Default)]
pub struct SkillsWindow {
    tab: Tab,
    /// The sequence's selected skill.
    selected_skill: Option<usize>,
    /// The Saved Skills row selected (an index in the sets).
    selected: Option<usize>,
    reset_armed: Option<Instant>,
    was_open: bool,
}

impl SkillsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        let bot = crate::turu::bot();
        if !*open {
            // Closed however (its X, the menu, a hotkey): the next opening reads the file again,
            // which another tab's process may have changed, and Edit Rules is dropped.
            if std::mem::take(&mut self.was_open) {
                let state = model::STATE.of(&bot);
                let mut skills = lock(&state);
                skills.forget();
                skills.editing = None;
            }
            return;
        }
        self.was_open = true;
        let state = model::STATE.of(&bot);
        let mut skills = lock(&state);
        if skills.ensure_loaded(&bot.paths) {
            // Read again: the rows may be others.
            self.selected = None;
        }
        if self.reset_armed.is_some_and(|at| at.elapsed() >= CONFIRM_FOR) {
            self.reset_armed = None;
        }
        let mut view = View {
            window: self,
            skills: &mut skills,
            paths: &bot.paths,
        };
        super::window(ctx, "Advanced Skills", [700.0, 530.0], 1)
            .open(open)
            .show(ctx, |ui| view.ui(ui));
        view.rules_dialog(ctx);
        if self.reset_armed.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

/// One frame of the window: its state and the tab's Skills.
struct View<'a> {
    window: &'a mut SkillsWindow,
    skills: &'a mut Skills,
    paths: &'a turu_bot::paths::Paths,
}

impl View<'_> {
    fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if widgets::chip(ui, "Create / Edit Skill", self.window.tab == Tab::Editor) {
                self.window.tab = Tab::Editor;
            }
            if widgets::chip(ui, "Saved Skills", self.window.tab == Tab::Saved) {
                self.window.tab = Tab::Saved;
            }
        });
        ui.add_space(6.0);
        ScrollArea::vertical()
            .id_salt("skills_tab")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.window.tab {
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
        let skills = &mut *self.skills;
        let selected = &mut self.window.selected_skill;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "SKILL TIMEOUT");
            ui.horizontal(|ui| {
                let size = vec2(64.0, widgets::target(24.0, 40.0));
                let e = &mut skills.editor;
                widgets::number(ui, "skill_timeout", &mut e.timeout, 0..=model::MAX_MS, 10.0, "", size);
                ui.label(RichText::new("Timeout (ms)").color(MUTED));
            });
        });
        ui.add_space(6.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "SKILLS SEQUENCE");
            ui.add_space(4.0);
            let mut edit = None;
            list_frame(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("skills_sequence")
                    .auto_shrink([false, false])
                    .min_scrolled_height(160.0)
                    .max_height(160.0)
                    .show(ui, |ui| {
                        if skills.editor.skills.is_empty() {
                            ui.label(RichText::new("Add skills with the 0 - 5 buttons.").color(MUTED));
                        }
                        for (i, item) in skills.editor.skills.iter().enumerate() {
                            let text = RichText::new(item.display()).color(TEXT);
                            let row = ui.add_sized(
                                [ui.available_width(), 22.0],
                                egui::Button::selectable(*selected == Some(i), text),
                            );
                            if row.clicked() {
                                *selected = Some(i);
                            }
                            if row.double_clicked() {
                                edit = Some(i);
                            }
                        }
                    });
            });
            ui.add_space(4.0);
            let count = skills.editor.skills.len();
            let has_selection = selected.is_some_and(|i| i < count);
            ui.horizontal(|ui| {
                let size = vec2(28.0, 24.0);
                for (label, hint, down) in [("↑", "Move Skill Up", false), ("↓", "Move Skill Down", true)] {
                    if widgets::button(ui, label, Kind::Normal, true, size, "")
                        .on_hover_text(hint)
                        .clicked()
                    {
                        // VibeSkua `MoveSkill`: with nothing selected it selects the first skill.
                        match *selected {
                            None => *selected = (count > 0).then_some(0),
                            Some(i) => {
                                if let Some(to) = skills.editor.move_skill(i, down) {
                                    *selected = Some(to);
                                }
                            }
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, "✕", Kind::Normal, has_selection, size, "Select a skill")
                        .on_hover_text("Remove Selected Skill")
                        .clicked()
                        && let Some(i) = *selected
                    {
                        if skills.editing.as_ref().is_some_and(|ed| ed.index >= i) {
                            skills.editing = None;
                        }
                        *selected = skills.editor.remove_skill(i);
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
                    {
                        edit = *selected;
                    }
                });
            });
            if let Some(i) = edit {
                skills.edit_rules(i);
            }
        });
        ui.add_space(6.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::text_box(
                ui,
                &mut skills.editor.class_name,
                "Class Name (e.g. Void Highlord)",
                ui.available_width(),
            );
        });
    }

    fn rules_column(&mut self, ui: &mut Ui) {
        let e = &mut self.skills.editor;
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
                        e.add(slot);
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
            let _ = self.skills.save(self.paths);
        }
    }

    fn saved_tab(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if widgets::button(ui, "Load Classes", Kind::Normal, true, vec2(0.0, 26.0), "").clicked() {
                    self.skills.reload(self.paths);
                    self.skills.load_classes();
                    self.window.selected = None;
                }
                let open_width = 60.0;
                let combo_width = ((ui.available_width() - open_width - 3.0 * 8.0) / 2.0).max(90.0);
                let mut picked = None;
                ComboBox::from_id_salt("skills_class")
                    .selected_text(self.skills.class.as_deref().unwrap_or("Select Class..."))
                    .width(combo_width)
                    .show_ui(ui, |ui| {
                        for class in &self.skills.classes {
                            if ui
                                .selectable_label(self.skills.class.as_ref() == Some(class), class)
                                .clicked()
                            {
                                picked = Some(class.clone());
                            }
                        }
                    });
                if picked.is_some() {
                    self.skills.pick_class(picked);
                }
                let modes = self.skills.modes();
                ComboBox::from_id_salt("skills_mode")
                    .selected_text(self.skills.mode.as_deref().unwrap_or("Select Mode..."))
                    .width(combo_width)
                    .show_ui(ui, |ui| {
                        for mode in modes {
                            let selected = self.skills.mode.as_deref() == Some(&mode);
                            if ui.selectable_label(selected, &mode).clicked() {
                                self.skills.mode = Some(mode);
                            }
                        }
                    });
                let chosen = self.skills.class.is_some() && self.skills.mode.is_some();
                if widgets::button(
                    ui,
                    "Open",
                    Kind::Primary,
                    chosen,
                    vec2(open_width, 26.0),
                    "Select a class and a mode",
                )
                .clicked()
                    && let Some(i) = self.skills.open_chosen()
                {
                    self.window.selected = Some(i);
                    self.window.tab = Tab::Editor;
                }
            });
            ui.add_space(6.0);
            let mut edit = None;
            list_frame(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("skills_saved")
                    .auto_shrink([false, false])
                    .min_scrolled_height(120.0)
                    .max_height((ui.available_height() - 60.0).max(120.0))
                    .show(ui, |ui| {
                        if self.skills.sets.is_empty() {
                            ui.label(RichText::new("No saved skill sets found.").color(MUTED));
                        }
                        let skills = &*self.skills;
                        for (i, set) in skills.sets.iter().enumerate().filter(|(_, set)| skills.shown(set)) {
                            let text = RichText::new(set.save_string()).color(TEXT);
                            let row = ui.add_sized(
                                [ui.available_width(), 22.0],
                                egui::Button::selectable(self.window.selected == Some(i), text).truncate(),
                            );
                            if row.clicked() {
                                self.window.selected = Some(i);
                            }
                            if row.double_clicked() {
                                edit = Some(i);
                            }
                            row.on_hover_text(set.save_string());
                        }
                    });
            });
            ui.add_space(6.0);
            // A selection the class filter hides is dropped, as VibeSkua's.
            if self.window.selected.is_some_and(|i| {
                self.skills
                    .sets
                    .get(i)
                    .is_none_or(|set: &AdvancedSkill| !self.skills.shown(set))
            }) {
                self.window.selected = None;
            }
            let has_selection = self.window.selected.is_some();
            ui.horizontal(|ui| {
                let size = vec2(0.0, 26.0);
                if widgets::button(ui, "Edit Selected", Kind::Normal, has_selection, size, "Select a set").clicked() {
                    edit = self.window.selected;
                }
                if widgets::button(ui, "Remove Selected", Kind::Normal, has_selection, size, "Select a set").clicked()
                    && let Some(i) = self.window.selected.take()
                {
                    self.skills.remove_set(self.paths, i);
                }
                if widgets::button(ui, "Refresh", Kind::Normal, true, size, "").clicked() {
                    self.skills.reload(self.paths);
                    self.window.selected = None;
                }
                let armed = self.window.reset_armed.is_some();
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
                        self.window.reset_armed = None;
                        self.window.selected = None;
                        self.skills.reset(self.paths);
                    } else {
                        self.window.reset_armed = Some(Instant::now());
                    }
                }
            });
            if let Some(i) = edit.filter(|&i| i < self.skills.sets.len()) {
                self.window.selected = Some(i);
                self.skills.open_set(i);
                self.window.tab = Tab::Editor;
            }
        });
        self.outcome_line(ui);
    }

    fn outcome_line(&self, ui: &mut Ui) {
        if let Some(outcome) = &self.skills.outcome {
            ui.add_space(4.0);
            ui.label(
                RichText::new(&outcome.text)
                    .small()
                    .color(if outcome.failed { DANGER } else { SUCCESS }),
            );
        }
    }

    /// VibeSkua's "Edit Rules" dialog for the selected skill: Confirm keeps the changes.
    fn rules_dialog(&mut self, ctx: &Context) {
        let Some(editing) = &mut self.skills.editing else {
            return;
        };
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
                rules_form(ui, &mut editing.rules, "skills_rules_dialog");
            });
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                confirm = widgets::button(ui, "Confirm", Kind::Primary, true, vec2(84.0, 28.0), "").clicked();
                cancel = widgets::button(ui, "Cancel", Kind::Normal, true, vec2(84.0, 28.0), "").clicked();
            });
        });
        if confirm {
            self.skills.confirm_rules();
        } else if cancel || response.should_close() {
            self.skills.editing = None;
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
        widgets::number(
            ui,
            label,
            value,
            0..=max,
            1.0,
            "",
            vec2(80.0, widgets::target(24.0, 40.0)),
        );
        ui.label(RichText::new(label).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            value_type_toggle(ui, percent);
        });
    });
    *value = model::bound_amount(*value, *percent);
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
        let format = widgets::NumberFormat {
            max_decimals: Some(2),
            ..Default::default()
        };
        let size = vec2(60.0, widgets::target(24.0, 40.0));
        widgets::number(ui, (id, "stacks"), stacks, 0.0..=f32::MAX, 0.1, format, size).on_hover_text("Stacks");
        target_combo(ui, (id, "target"), target);
        if removable {
            removed = widgets::button(ui, "✕", Kind::Normal, true, vec2(26.0, 22.0), "").clicked();
        }
    });
    removed
}

/// The USE RULES card's contents (VibeSkua `SkillRulesViewModel`), shared with Edit Rules.
fn rules_form(ui: &mut Ui, rules: &mut SkillRules, id: &str) {
    // In a narrow card (a phone) the checkboxes go on a row of their own, under the heading.
    let narrow = ui.available_width() < 380.0;
    ui.horizontal(|ui| {
        widgets::section_label(ui, "USE RULES");
        ui.add_space(6.0);
        if !narrow {
            ui.checkbox(&mut rules.use_rule, "Enable Rule");
            ui.checkbox(&mut rules.multi_aura, "Multi Aura");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::small_button(ui, "Reset", true).clicked() {
                *rules = SkillRules::default();
            }
        });
    });
    if narrow {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut rules.use_rule, "Enable Rule");
            ui.checkbox(&mut rules.multi_aura, "Multi Aura");
        });
    }
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
            let size = vec2(64.0, widgets::target(24.0, 40.0));
            widgets::number(ui, "wait", &mut rules.wait, 0..=model::MAX_MS, 10.0, "", size);
        });
    });
}
