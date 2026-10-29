//! Combat > Skills. The "Saved Skills" tab lists the sets in `AdvancedSkills.json` (class ->
//! mode -> skill entries); the editor tab is not built yet.

use super::theme::{MUTED, TEXT};
use super::widgets::{self, Kind};
use egui::{ComboBox, Context, RichText, ScrollArea, Ui, vec2};
use serde_json::Value;
use std::collections::BTreeMap;

/// The saved skill sets: class name -> mode name -> `{ skillUseMode, skillTimeout, skills }`.
#[derive(Default, Clone)]
pub struct SkillSets {
    classes: BTreeMap<String, BTreeMap<String, Value>>,
}

impl SkillSets {
    pub fn load() -> Self {
        let text = crate::turu::bot().paths.load_advanced_skills();
        Self {
            classes: serde_json::from_str(&text).unwrap_or_default(),
        }
    }

    pub fn classes(&self) -> impl Iterator<Item = &String> {
        self.classes.keys()
    }

    pub fn modes(&self, class: &str) -> Vec<String> {
        self.classes
            .get(class)
            .map(|modes| modes.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn set(&self, class: &str, mode: &str) -> Option<&Value> {
        self.classes.get(class)?.get(mode)
    }

    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

/// One skill entry as a line: `4 | H>70% M<70% A:target<0 "incinerate"`.
fn describe(entry: &Value) -> String {
    let id = entry.get("skillId").map_or_else(|| "?".to_owned(), ToString::to_string);
    let mut out = format!("Skill {id}");
    let Some(rules) = entry.get("rules").and_then(Value::as_array) else {
        return out;
    };
    let rules: Vec<String> = rules
        .iter()
        .map(|rule| {
            let kind = rule.get("type").and_then(Value::as_str).unwrap_or("?");
            let op = match rule.get("comparison").and_then(Value::as_str) {
                Some("greater") => ">",
                Some("less") => "<",
                Some(other) => other,
                None => "?",
            };
            let value = rule.get("value").map_or_else(String::new, ToString::to_string);
            let pct = if rule.get("isPercentage").and_then(Value::as_bool).unwrap_or(false) {
                "%"
            } else {
                ""
            };
            match kind {
                "Aura" => {
                    let name = rule.get("auraName").and_then(Value::as_str).unwrap_or("?");
                    let target = rule.get("auraTarget").and_then(Value::as_str).unwrap_or("target");
                    format!("aura \"{name}\" ({target}) {op} {value}")
                }
                other => format!("{} {op} {value}{pct}", other.to_lowercase()),
            }
        })
        .collect();
    if !rules.is_empty() {
        out.push_str(" if ");
        out.push_str(&rules.join(", "));
    }
    out
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Tab {
    Editor,
    #[default]
    Saved,
}

#[derive(Default)]
pub struct SkillsWindow {
    tab: Tab,
    sets: SkillSets,
    loaded: bool,
    class: Option<String>,
    mode: Option<String>,
    selected: Option<usize>,
}

impl SkillsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            self.loaded = false;
            return;
        }
        if !self.loaded {
            self.sets = SkillSets::load();
            self.loaded = true;
        }
        super::window(ctx, "Skills", [420.0, 440.0], 1)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
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
        ui.add_space(4.0);
        match self.tab {
            Tab::Editor => {
                widgets::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    widgets::section_label(ui, "SKILLS SEQUENCE");
                    ui.add_space(4.0);
                    ui.label(RichText::new("Coming soon").color(MUTED));
                    ui.label(
                        RichText::new("Edit AdvancedSkills.json in the data folder for now.")
                            .small()
                            .color(MUTED),
                    );
                });
            }
            Tab::Saved => self.saved(ui),
        }
    }

    fn saved(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Load Classes", Kind::Normal, true, vec2(0.0, 26.0), "").clicked() {
                self.sets = SkillSets::load();
            }
            ComboBox::from_id_salt("skills_class")
                .selected_text(self.class.as_deref().unwrap_or("Select Class..."))
                .width(150.0)
                .show_ui(ui, |ui| {
                    for class in self.sets.classes() {
                        if ui
                            .selectable_label(self.class.as_deref() == Some(class), class)
                            .clicked()
                        {
                            if self.class.as_deref() != Some(class) {
                                self.mode = None;
                                self.selected = None;
                            }
                            self.class = Some(class.clone());
                        }
                    }
                });
            let modes = self.class.as_deref().map(|c| self.sets.modes(c)).unwrap_or_default();
            ComboBox::from_id_salt("skills_mode")
                .selected_text(self.mode.as_deref().unwrap_or("Select Mode..."))
                .width(100.0)
                .show_ui(ui, |ui| {
                    for mode in modes {
                        if ui
                            .selectable_label(self.mode.as_deref() == Some(&mode), &mode)
                            .clicked()
                        {
                            self.mode = Some(mode);
                            self.selected = None;
                        }
                    }
                });
        });
        ui.add_space(2.0);
        let entry_set = match (&self.class, &self.mode) {
            (Some(class), Some(mode)) => self.sets.set(class, mode),
            _ => None,
        };
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(set) = entry_set {
                let use_mode = set.get("skillUseMode").and_then(Value::as_str).unwrap_or("?");
                let timeout = set
                    .get("skillTimeout")
                    .map_or_else(|| "?".to_owned(), ToString::to_string);
                widgets::section_label(ui, "SKILLS SEQUENCE");
                ui.label(
                    RichText::new(format!("{use_mode}, timeout {timeout} ms"))
                        .small()
                        .color(MUTED),
                );
                ui.add_space(4.0);
                ScrollArea::vertical()
                    .id_salt("skills_entries")
                    .auto_shrink([false, false])
                    .max_height(ui.available_height().max(120.0) - 40.0)
                    .show(ui, |ui| {
                        let entries = set.get("skills").and_then(Value::as_array);
                        for (i, entry) in entries.into_iter().flatten().enumerate() {
                            let text = RichText::new(describe(entry)).monospace().color(TEXT);
                            if ui.selectable_label(self.selected == Some(i), text).clicked() {
                                self.selected = Some(i);
                            }
                        }
                    });
            } else if self.sets.is_empty() {
                ui.label(RichText::new("No saved skill sets found.").color(MUTED));
            } else {
                ui.label(RichText::new("Select a class and a mode.").color(MUTED));
            }
        });
        ui.horizontal(|ui| {
            for label in ["Open", "Edit Selected", "Remove Selected"] {
                widgets::button(ui, label, Kind::Normal, false, vec2(0.0, 26.0), "Coming soon");
            }
            if widgets::button(ui, "Refresh", Kind::Normal, true, vec2(0.0, 26.0), "").clicked() {
                self.sets = SkillSets::load();
            }
            widgets::button(ui, "Reset Sets", Kind::Outline, false, vec2(0.0, 26.0), "Coming soon");
        });
    }
}
