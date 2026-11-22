//! Combat > Skills (VibeSkua `AdvancedSkillsView`, "Advanced Skills"): the editor that builds a
//! skill sequence with its use rules and saves it as one class's set in one mode, and the saved
//! sets (`UserAdvancedSkills.json`) to open, remove or reset.
//!
//! One [`Skills`] per tab ([`STATE`]), which the egui window and a native page share, so Full
//! window keeps a sequence in progress. What stays each shell's: its tabs, which row is
//! selected, and how it asks before Reset. The view carries the class list and only the picked
//! class's sets: the file holds a few hundred.

use super::Control;
use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::paths::Paths;
use turu_bot::skill_editor::{self, AuraRow, SkillItem, SkillRules};
use turu_bot::skills::{self as bot_skills, AdvancedSkill, CLASS_USE_MODES, SkillUseMode};

pub use super::auto::SkillSets;

/// The longest skill timeout and wait the editor takes, in ms (VibeSkua's spinners).
pub const MAX_MS: i64 = 5000;

/// The "Create / Edit Skill" tab (VibeSkua `AdvancedSkillEditorViewModel`).
#[derive(Debug, Clone, PartialEq)]
pub struct Editor {
    pub timeout: i64,
    pub wait_for_cooldown: bool,
    pub reset_on_target: bool,
    pub skills: Vec<SkillItem>,
    pub class_name: String,
    /// Index in [`CLASS_USE_MODES`].
    pub mode: usize,
    /// The rules the slot buttons give a new skill.
    pub rules: SkillRules,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            timeout: 100,
            wait_for_cooldown: false,
            reset_on_target: false,
            skills: Vec::new(),
            class_name: String::new(),
            mode: 0,
            rules: SkillRules::default(),
        }
    }
}

impl Editor {
    /// VibeSkua `EditSkill(message)`: a saved set into the editor (its Use Rules stay).
    pub fn open(&mut self, set: &AdvancedSkill) {
        self.timeout = set.skill_timeout;
        self.wait_for_cooldown = set.skill_use_mode != SkillUseMode::UseIfAvailable;
        self.reset_on_target = set.reset_on_target;
        self.class_name = set.class_name.clone();
        self.mode = CLASS_USE_MODES
            .iter()
            .position(|m| m.eq_ignore_ascii_case(&set.mode))
            .unwrap_or(0);
        self.skills = skill_editor::parse_sequence(&set.skills);
    }

    /// VibeSkua `SaveSkills`: the set the editor describes.
    pub fn to_set(&self) -> AdvancedSkill {
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

    /// A slot button (0 auto attack .. 5 potion): the skill with a copy of the editor's rules.
    pub fn add(&mut self, slot: i64) {
        if (0..=5).contains(&slot) {
            self.skills.push(SkillItem::new(slot, &self.rules));
        }
    }

    /// VibeSkua `MoveSkill`: the skill at `index` one up or down; where it went.
    pub fn move_skill(&mut self, index: usize, down: bool) -> Option<usize> {
        let to = if down {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        }?;
        if index < self.skills.len() && to < self.skills.len() {
            self.skills.swap(index, to);
            Some(to)
        } else {
            None
        }
    }

    /// VibeSkua `RemoveSkill`; the skill above becomes the selection, which this returns.
    pub fn remove_skill(&mut self, index: usize) -> Option<usize> {
        if index < self.skills.len() {
            self.skills.remove(index);
        }
        index.checked_sub(1)
    }
}

/// The skill whose rules Edit Rules shows, and the copy it edits (Confirm keeps it).
#[derive(Debug, Clone, PartialEq)]
pub struct Editing {
    pub index: usize,
    pub rules: SkillRules,
}

/// The last save, remove or reset.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub text: String,
    pub failed: bool,
}

/// A tab's Skills: the editor and the saved sets as read.
#[derive(Debug, Default)]
pub struct Skills {
    pub editor: Editor,
    pub editing: Option<Editing>,
    /// The saved sets in file order (VibeSkua `LoadedSkills`).
    pub sets: Vec<AdvancedSkill>,
    /// "Load Classes": the classes with saved sets, sorted (VibeSkua `AvailableClasses`).
    pub classes: Vec<String>,
    /// The class whose sets show (VibeSkua `FilteredLoadedSkills`); `None` shows all.
    pub class: Option<String>,
    /// The mode picked beside it, for Open.
    pub mode: Option<String>,
    pub outcome: Option<Outcome>,
    loaded: bool,
    /// `skill_sets_generation` when the sets were read: the launch update may add sets later.
    generation: u64,
}

pub static STATE: PerTab<Skills> = PerTab::new();

impl Skills {
    /// Reads the sets the first time, and again when the launch update added some (not while a
    /// skill's rules are being edited); whether it read them.
    pub fn ensure_loaded(&mut self, paths: &Paths) -> bool {
        if !self.loaded {
            self.reload(paths);
            if self.classes.is_empty() {
                self.load_classes();
            }
            self.loaded = true;
            true
        } else if self.generation != bot_skills::skill_sets_generation() && self.editing.is_none() {
            self.reload(paths);
            true
        } else {
            false
        }
    }

    /// The next [`Self::ensure_loaded`] reads the file again (the window closed).
    pub fn forget(&mut self) {
        self.loaded = false;
    }

    /// Reads the saved sets again (VibeSkua `LoadSkills`).
    pub fn reload(&mut self, paths: &Paths) {
        self.generation = bot_skills::skill_sets_generation();
        self.sets = bot_skills::load_advanced_skills(paths);
    }

    /// VibeSkua `LoadAvailableClasses`: the saved classes, sorted; a picked class gone is dropped.
    pub fn load_classes(&mut self) {
        self.classes = SkillSets::from_sets(&self.sets).classes().cloned().collect();
        if self.class.as_ref().is_some_and(|c| !self.classes.contains(c)) {
            self.class = None;
            self.mode = None;
        }
    }

    /// The picked class's saved modes.
    pub fn modes(&self) -> Vec<String> {
        self.class
            .as_deref()
            .map(|c| SkillSets::from_sets(&self.sets).modes(c))
            .unwrap_or_default()
    }

    pub fn pick_class(&mut self, class: Option<String>) {
        if self.class != class {
            self.class = class;
            self.mode = None;
        }
    }

    /// Whether the set at `index` shows under the picked class.
    pub fn shown(&self, set: &AdvancedSkill) -> bool {
        self.class
            .as_deref()
            .is_none_or(|c| set.class_name.eq_ignore_ascii_case(c))
    }

    fn write(&mut self, paths: &Paths, done: String) {
        self.outcome = Some(match bot_skills::save_advanced_skills(paths, &self.sets) {
            Ok(()) => Outcome {
                text: done,
                failed: false,
            },
            Err(e) => Outcome {
                text: format!("Couldn't save the skill sets: {e}"),
                failed: true,
            },
        });
        self.reload(paths);
    }

    /// Save Skill: the editor's set replaces the class's set in its mode, or is added.
    pub fn save(&mut self, paths: &Paths) -> Result<(), String> {
        let set = self.editor.to_set();
        if set.class_name.is_empty() {
            return Err("Enter a class name".into());
        }
        let done = format!("Saved {} ({}).", set.class_name, set.mode);
        bot_skills::override_skill_set(&mut self.sets, set);
        self.write(paths, done);
        self.load_classes();
        Ok(())
    }

    /// Remove Selected.
    pub fn remove_set(&mut self, paths: &Paths, index: usize) {
        if index < self.sets.len() {
            let set = self.sets.remove(index);
            self.write(paths, format!("Removed {} ({}).", set.class_name, set.mode));
            self.load_classes();
        }
    }

    /// VibeSkua's `EditAdvancedSkillMessage`: the saved set at `index` into the editor.
    pub fn open_set(&mut self, index: usize) {
        if let Some(set) = self.sets.get(index).cloned() {
            self.editor.open(&set);
            self.editing = None;
            self.outcome = None;
        }
    }

    /// VibeSkua `LoadSelectedClassMode`: the picked class and mode into the editor; its index.
    pub fn open_chosen(&mut self) -> Option<usize> {
        let (class, mode) = (self.class.as_ref()?, self.mode.as_ref()?);
        let index = self
            .sets
            .iter()
            .position(|s| &s.class_name == class && s.mode.eq_ignore_ascii_case(mode))?;
        self.open_set(index);
        Some(index)
    }

    /// VibeSkua `ResetSkillsSets`: the shipped sets replace the user's.
    pub fn reset(&mut self, paths: &Paths) {
        self.outcome = Some(match paths.reset_advanced_skills() {
            Ok(()) => Outcome {
                text: "Reset the skill sets to the shipped ones.".into(),
                failed: false,
            },
            Err(e) => Outcome {
                text: format!("Couldn't reset the skill sets: {e}"),
                failed: true,
            },
        });
        self.reload(paths);
        self.load_classes();
    }

    /// Edit Rules on the skill at `index`: a copy of its rules to edit.
    pub fn edit_rules(&mut self, index: usize) {
        if let Some(item) = self.editor.skills.get(index) {
            self.editing = Some(Editing {
                index,
                rules: item.rules.clone(),
            });
        }
    }

    /// Confirm: the copy becomes the skill's rules.
    pub fn confirm_rules(&mut self) {
        if let Some(Editing { index, rules }) = self.editing.take()
            && let Some(item) = self.editor.skills.get_mut(index)
        {
            item.rules = rules;
        }
    }
}

// --- the view a shell reads -------------------------------------------------------------

/// A skill's use rules (VibeSkua `SkillRulesViewModel`): what the page edits and sends back
/// whole. A value of 0 means no such rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RulesView {
    pub use_rule: bool,
    pub multi_aura: bool,
    pub health_greater: bool,
    pub health: i64,
    pub health_percent: bool,
    pub mana_greater: bool,
    pub mana: i64,
    pub mana_percent: bool,
    pub party_health_greater: bool,
    pub party_health: i64,
    pub party_health_percent: bool,
    pub wait: i64,
    pub skip: bool,
    pub aura_greater: bool,
    pub aura_stacks: f64,
    pub aura_target: bool,
    pub aura_name: String,
    pub auras: Vec<AuraView>,
    pub auras_or: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct AuraView {
    pub name: String,
    pub stacks: f64,
    pub greater: bool,
    pub target: bool,
}

impl From<&SkillRules> for RulesView {
    fn from(r: &SkillRules) -> Self {
        Self {
            use_rule: r.use_rule,
            multi_aura: r.multi_aura,
            health_greater: r.health_greater,
            health: r.health,
            health_percent: r.health_percent,
            mana_greater: r.mana_greater,
            mana: r.mana,
            mana_percent: r.mana_percent,
            party_health_greater: r.party_health_greater,
            party_health: r.party_health,
            party_health_percent: r.party_health_percent,
            wait: r.wait,
            skip: r.skip,
            aura_greater: r.aura_greater,
            aura_stacks: f64::from(r.aura_stacks),
            aura_target: r.aura_target,
            aura_name: r.aura_name.clone(),
            auras: r
                .auras
                .iter()
                .map(|a| AuraView {
                    name: a.name.clone(),
                    stacks: f64::from(a.stacks),
                    greater: a.greater,
                    target: a.target,
                })
                .collect(),
            auras_or: r.auras_or,
        }
    }
}

/// A health, mana or party HP amount as the editor bounds it: a percentage stops at 100.
pub fn bound_amount(value: i64, percent: bool) -> i64 {
    value.clamp(0, if percent { 100 } else { i64::from(i32::MAX) })
}

/// Stacks stay a finite count of 0 or more.
fn bound_stacks(value: f64) -> f32 {
    if value.is_finite() {
        (value as f32).max(0.0)
    } else {
        0.0
    }
}

impl RulesView {
    /// The rules this describes, bounded as the editor bounds them; `before` decides VibeSkua's
    /// "the last aura removed turns Multi Aura off".
    pub fn to_rules(&self, before: &SkillRules) -> SkillRules {
        let auras: Vec<AuraRow> = self
            .auras
            .iter()
            .map(|a| AuraRow {
                name: a.name.clone(),
                stacks: bound_stacks(a.stacks),
                greater: a.greater,
                target: a.target,
            })
            .collect();
        let emptied = !before.auras.is_empty() && auras.is_empty();
        SkillRules {
            use_rule: self.use_rule,
            multi_aura: self.multi_aura && !emptied,
            health_greater: self.health_greater,
            health: bound_amount(self.health, self.health_percent),
            health_percent: self.health_percent,
            mana_greater: self.mana_greater,
            mana: bound_amount(self.mana, self.mana_percent),
            mana_percent: self.mana_percent,
            party_health_greater: self.party_health_greater,
            party_health: bound_amount(self.party_health, self.party_health_percent),
            party_health_percent: self.party_health_percent,
            wait: self.wait.clamp(0, MAX_MS),
            skip: self.skip,
            aura_greater: self.aura_greater,
            aura_stacks: bound_stacks(self.aura_stacks),
            aura_target: self.aura_target,
            aura_name: self.aura_name.clone(),
            auras,
            auras_or: self.auras_or,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct SkillsView {
    pub timeout: i64,
    pub wait_for_cooldown: bool,
    pub reset_on_target: bool,
    /// The sequence, a line per skill (`4 - [Health > 50%] - [Skip if not available]`).
    pub sequence: Vec<String>,
    pub class_name: String,
    pub mode: String,
    /// The modes a set can be saved in (VibeSkua `ClassUseMode`).
    pub modes: Vec<String>,
    /// The rules the slot buttons give a new skill.
    pub rules: RulesView,
    pub save: Control,
    /// A saved set of this class in this mode exists: Save replaces it.
    pub replaces: bool,
    /// Edit Rules is open on a skill.
    pub editing: Option<EditingView>,
    /// The classes with saved sets.
    pub classes: Vec<String>,
    /// The class picked ("" for none).
    pub class: String,
    /// The picked class's sets (none while no class is picked).
    pub sets: Vec<SetView>,
    pub outcome: Option<Outcome>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct EditingView {
    /// The skill's place in the sequence (the actions' `index`).
    pub position: u32,
    /// The skill's line as it was.
    pub skill: String,
    pub rules: RulesView,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct SetView {
    /// Its place in the file (the actions' `index`).
    pub position: u32,
    pub class: String,
    pub mode: String,
    /// The sequence in skill-string syntax.
    pub sequence: String,
    /// "Use if available", or "Wait for cooldown · 100 ms".
    pub use_mode: String,
    pub reset_on_target: bool,
}

/// How a set uses its skills, in words.
pub fn use_mode_words(set: &AdvancedSkill) -> String {
    match set.skill_use_mode {
        SkillUseMode::UseIfAvailable => "Use if available".into(),
        SkillUseMode::WaitForCooldown => format!("Wait for cooldown · {} ms", set.skill_timeout),
    }
}

impl Skills {
    pub fn view(&self) -> SkillsView {
        let e = &self.editor;
        let to_save = e.to_set();
        let named = !to_save.class_name.is_empty();
        SkillsView {
            timeout: e.timeout,
            wait_for_cooldown: e.wait_for_cooldown,
            reset_on_target: e.reset_on_target,
            sequence: e.skills.iter().map(SkillItem::display).collect(),
            class_name: e.class_name.clone(),
            mode: CLASS_USE_MODES[e.mode].to_owned(),
            modes: CLASS_USE_MODES.iter().map(|m| (*m).to_owned()).collect(),
            rules: (&e.rules).into(),
            save: Control::when(named, "Enter a class name"),
            replaces: named && self.sets.iter().any(|s| s.same_set(&to_save)),
            editing: self.editing.as_ref().map(|ed| EditingView {
                position: ed.index as u32,
                skill: e.skills.get(ed.index).map(SkillItem::display).unwrap_or_default(),
                rules: (&ed.rules).into(),
            }),
            classes: self.classes.clone(),
            class: self.class.clone().unwrap_or_default(),
            sets: match &self.class {
                None => Vec::new(),
                Some(_) => self
                    .sets
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| self.shown(s))
                    .map(|(i, s)| SetView {
                        position: i as u32,
                        class: s.class_name.clone(),
                        mode: s.mode.clone(),
                        sequence: s.skills.clone(),
                        use_mode: use_mode_words(s),
                        reset_on_target: s.reset_on_target,
                    })
                    .collect(),
            },
            outcome: self.outcome.clone(),
        }
    }

    /// What a shell's page does (`POST /models/skills/do`).
    pub fn act(&mut self, paths: &Paths, act: SkillsAct) -> Result<(), String> {
        let e = &mut self.editor;
        match act {
            SkillsAct::Open | SkillsAct::Refresh => {
                self.reload(paths);
                self.load_classes();
            }
            SkillsAct::Editor(change) => {
                if let Some(ms) = change.timeout {
                    e.timeout = ms.clamp(0, MAX_MS);
                }
                if let Some(on) = change.wait_for_cooldown {
                    e.wait_for_cooldown = on;
                }
                if let Some(on) = change.reset_on_target {
                    e.reset_on_target = on;
                }
                if let Some(name) = change.class_name {
                    e.class_name = name;
                }
                if let Some(mode) = change.mode {
                    e.mode = CLASS_USE_MODES
                        .iter()
                        .position(|m| m.eq_ignore_ascii_case(&mode))
                        .ok_or_else(|| format!("no mode {mode}"))?;
                }
            }
            SkillsAct::Add { slot } => {
                e.add(slot);
            }
            SkillsAct::Move { position, down } => {
                e.move_skill(position as usize, down);
            }
            SkillsAct::Remove { position } => {
                let i = position as usize;
                if self.editing.as_ref().is_some_and(|ed| ed.index >= i) {
                    self.editing = None;
                }
                self.editor.remove_skill(i);
            }
            SkillsAct::EditRules { position } => self.edit_rules(position as usize),
            SkillsAct::ConfirmRules => self.confirm_rules(),
            SkillsAct::CancelRules => self.editing = None,
            SkillsAct::Rules { target, rules: view } => {
                let rules = match target.as_str() {
                    "new" => &mut self.editor.rules,
                    "editing" => &mut self.editing.as_mut().ok_or("no skill's rules are open")?.rules,
                    other => return Err(format!("no rules {other}")),
                };
                *rules = view.to_rules(rules);
            }
            SkillsAct::Save { name } => {
                // The class name as typed, when the page sends it with Save.
                if let Some(name) = name {
                    self.editor.class_name = name;
                }
                self.save(paths)?
            }
            SkillsAct::Class { name } => self.pick_class((!name.is_empty()).then_some(name)),
            SkillsAct::OpenSet { position } => self.open_set(position as usize),
            SkillsAct::RemoveSet { position } => self.remove_set(paths, position as usize),
            SkillsAct::ResetSets => self.reset(paths),
            SkillsAct::Dismiss => self.outcome = None,
        }
        Ok(())
    }
}

/// The tab's view (its sets read the first time).
pub fn view(bot: &Arc<Bot>) -> SkillsView {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    s.ensure_loaded(&bot.paths);
    s.view()
}

/// A change to the editor from a shell: the fields given.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct EditorChange {
    /// Milliseconds, kept within 0 and the most it takes.
    pub timeout: Option<i64>,
    pub wait_for_cooldown: Option<bool>,
    pub reset_on_target: Option<bool>,
    pub class_name: Option<String>,
    /// One of the class use modes, by name.
    pub mode: Option<String>,
}

/// Skills' actions (`POST /models/skills/do`). `position` is a skill's in the sequence, or (the
/// saved sets') a set's.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum SkillsAct {
    Open,
    Refresh,
    Editor(EditorChange),
    /// Adds the skill in `slot`.
    Add {
        slot: i64,
    },
    Move {
        position: u32,
        down: bool,
    },
    Remove {
        position: u32,
    },
    /// Opens Edit Rules for a skill.
    EditRules {
        position: u32,
    },
    ConfirmRules,
    CancelRules,
    /// Sets the rules of `target`: "new" (the editor's) or "editing" (the skill Edit Rules has open).
    Rules {
        target: String,
        rules: RulesView,
    },
    /// Saves the sequence, as the class `name` when it's given (the name as typed).
    Save {
        name: Option<String>,
    },
    /// Shows the saved sets of the class `name` ("" for all).
    Class {
        name: String,
    },
    OpenSet {
        position: u32,
    },
    RemoveSet {
        position: u32,
    },
    ResetSets,
    /// Dismisses the outcome.
    Dismiss,
}

pub fn act(bot: &Arc<Bot>, act: SkillsAct) -> Result<(), String> {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    s.ensure_loaded(&bot.paths);
    s.act(&bot.paths, act)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value as Json, json};

    fn paths() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            root: dir.path().to_owned(),
        };
        (dir, paths)
    }

    /// `action` with `body`, read as a shell's JSON is.
    fn read(action: &str, mut body: Json) -> SkillsAct {
        body["action"] = json!(action);
        serde_json::from_value(body).unwrap()
    }

    fn rules_body(skills: &Skills, target: &str, edit: impl FnOnce(&mut RulesView)) -> Json {
        let mut view = match target {
            "new" => RulesView::from(&skills.editor.rules),
            _ => RulesView::from(&skills.editing.as_ref().unwrap().rules),
        };
        edit(&mut view);
        json!({ "target": target, "rules": view })
    }

    #[test]
    fn a_sequence_is_built_saved_and_opened_again() {
        let (_dir, paths) = paths();
        let mut s = Skills::default();
        s.ensure_loaded(&paths);
        let shipped = s.sets.len();
        assert!(shipped > 0, "the shipped sets seed the user's");
        assert_eq!(s.view().save, Control::off("Enter a class name"));

        let body = rules_body(&s, "new", |r| {
            r.use_rule = true;
            r.health = 250;
            r.skip = true;
        });
        s.act(&paths, read("rules", body)).unwrap();
        assert_eq!(s.editor.rules.health, 100, "a percentage stops at 100");
        for slot in [1, 2, 4] {
            s.act(&paths, read("add", json!({ "slot": slot }))).unwrap();
        }
        s.act(&paths, read("move", json!({ "position": 2, "down": false })))
            .unwrap();
        s.act(
            &paths,
            read(
                "editor",
                json!({ "className": " Questwalker Mage ", "mode": "farm", "timeout": 9000 }),
            ),
        )
        .unwrap();
        let view = s.view();
        assert_eq!(view.sequence[0], "1 - [Health > 100%] - [Skip if not available]");
        assert!(view.sequence[1].starts_with('4'));
        assert_eq!((view.mode.as_str(), view.timeout), ("Farm", MAX_MS));
        assert!(view.save.enabled && !view.replaces);

        // The name as typed comes with Save.
        s.act(&paths, read("save", json!({ "name": "Questwalker Mage" })))
            .unwrap();
        assert_eq!(s.outcome.as_ref().unwrap().text, "Saved Questwalker Mage (Farm).");
        assert_eq!(s.sets.len(), shipped + 1);
        assert!(s.view().replaces, "saving again replaces it");

        s.act(&paths, read("class", json!({ "name": "Questwalker Mage" })))
            .unwrap();
        let view = s.view();
        assert_eq!(view.sets.len(), 1);
        assert_eq!(view.sets[0].use_mode, "Use if available");
        s.editor = Editor::default();
        s.act(&paths, read("open-set", json!({ "position": view.sets[0].position })))
            .unwrap();
        assert_eq!(s.editor.skills.len(), 3);
        assert_eq!(s.editor.class_name, "Questwalker Mage");

        s.act(&paths, read("remove-set", json!({ "position": view.sets[0].position })))
            .unwrap();
        assert_eq!(s.sets.len(), shipped);
        assert_eq!(s.class, None, "the class went with its last set");
    }

    #[test]
    fn edit_rules_keeps_its_copy_until_confirmed() {
        let (_dir, paths) = paths();
        let mut s = Skills::default();
        s.ensure_loaded(&paths);
        s.act(&paths, read("add", json!({ "slot": 3 }))).unwrap();
        s.act(&paths, read("edit-rules", json!({ "position": 0 }))).unwrap();
        let body = rules_body(&s, "editing", |r| {
            r.use_rule = true;
            r.multi_aura = true;
            r.auras.push(AuraView {
                name: "Focus".into(),
                stacks: -2.0,
                greater: true,
                target: false,
            });
        });
        s.act(&paths, read("rules", body)).unwrap();
        assert_eq!(s.editing.as_ref().unwrap().rules.auras[0].stacks, 0.0);
        s.act(&paths, read("cancel-rules", json!({}))).unwrap();
        assert!(!s.editor.skills[0].rules.use_rule, "Cancel keeps the skill's rules");

        s.act(&paths, read("edit-rules", json!({ "position": 0 }))).unwrap();
        let body = rules_body(&s, "editing", |r| {
            r.use_rule = true;
            r.multi_aura = true;
            r.auras.push(AuraView {
                name: "Focus".into(),
                stacks: 3.0,
                greater: true,
                target: true,
            });
        });
        s.act(&paths, read("rules", body)).unwrap();
        // The last aura removed turns Multi Aura off.
        let body = rules_body(&s, "editing", |r| r.auras.clear());
        s.act(&paths, read("rules", body)).unwrap();
        assert!(!s.editing.as_ref().unwrap().rules.multi_aura);
        s.act(&paths, read("confirm-rules", json!({}))).unwrap();
        assert!(s.editing.is_none());
        assert!(s.editor.skills[0].rules.use_rule);
    }

    #[test]
    fn removing_the_skill_being_edited_closes_edit_rules() {
        let (_dir, paths) = paths();
        let mut s = Skills::default();
        s.ensure_loaded(&paths);
        s.act(&paths, read("add", json!({ "slot": 1 }))).unwrap();
        s.act(&paths, read("add", json!({ "slot": 2 }))).unwrap();
        s.act(&paths, read("edit-rules", json!({ "position": 1 }))).unwrap();
        s.act(&paths, read("remove", json!({ "position": 0 }))).unwrap();
        assert!(s.editing.is_none());
        assert_eq!(s.view().sequence, ["2"]);
        assert!(serde_json::from_value::<SkillsAct>(json!({ "action": "nothing" })).is_err());
        assert!(s.act(&paths, read("editor", json!({ "mode": "Sleep" }))).is_err());
    }

    #[test]
    fn no_class_picked_serves_no_sets() {
        let (_dir, paths) = paths();
        let mut s = Skills::default();
        s.ensure_loaded(&paths);
        let view = s.view();
        assert!(view.sets.is_empty() && !view.classes.is_empty());
        let class = view.classes[0].clone();
        s.act(&paths, read("class", json!({ "name": class }))).unwrap();
        assert!(s.view().sets.iter().all(|set| set.class.eq_ignore_ascii_case(&class)));
    }
}
