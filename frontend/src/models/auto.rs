//! Auto (VibeSkua's `AutoViewModel` and `ScriptAuto`): Auto Attack and Auto Hunt with the class
//! and mode to fight with and the maps to hunt in, and Stop. The loop is `turu_bot::auto`. The
//! class drop-down lists the Class items owned, read again whenever it opens (which clears the
//! selection); picking a class equips it and selects its first saved mode.

use super::Control;
use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use turu_bot::Bot;
pub use turu_bot::auto::AutoMode;
use turu_bot::skills::{self as bot_skills, AdvancedSkill};

/// The saved skill sets by class and mode, for the windows that pick a class's mode (Auto,
/// CoreBots Options).
#[derive(Default, Clone)]
pub struct SkillSets {
    classes: BTreeMap<String, Vec<String>>,
}

impl SkillSets {
    /// The active tab's saved sets.
    pub fn load() -> Self {
        Self::load_for(&crate::turu::bot())
    }

    pub fn load_for(bot: &Bot) -> Self {
        Self::from_sets(&bot_skills::load_advanced_skills(&bot.paths))
    }

    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    pub fn from_sets(sets: &[AdvancedSkill]) -> Self {
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

    /// VibeSkua `EnsureSkillEntryExists`: gives `class` the default `Base` set in the user's
    /// file when it has none (saved at once), and lists it here.
    pub fn ensure(&mut self, class: &str) {
        match bot_skills::ensure_skill_set(&crate::turu::bot().paths, class) {
            Ok(true) => {
                tracing::info!("Added a Base skill set for {class}");
                self.classes.entry(class.to_owned()).or_default().push("Base".into());
            }
            Ok(false) => {}
            Err(e) => tracing::warn!("Couldn't add a skill set for {class}: {e}"),
        }
    }
}

#[derive(Default)]
struct State {
    sets: SkillSets,
    /// The Class items owned (VibeSkua `PlayerClasses`), sorted ignoring case.
    classes: Vec<String>,
    loading: bool,
    class: Option<String>,
    mode: Option<String>,
    map_ids: String,
    /// The equipped class, as last read with the owned ones.
    equipped: Option<String>,
    /// The last run started here: when, and what it fights with in words.
    run: Option<(std::time::Instant, String)>,
}

static STATE: PerTab<State> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct AutoView {
    /// Auto Attack or Auto Hunt runs.
    pub running: bool,
    /// Auto Attack and Auto Hunt (they share their reason).
    pub start: Control,
    pub stop: Control,
    /// The Class items owned, as last read ([`reload_classes`]).
    pub classes: Vec<String>,
    /// The classes are being read.
    pub loading: bool,
    pub class: Option<String>,
    /// The chosen class's saved modes.
    pub modes: Vec<String>,
    pub mode: Option<String>,
    /// Manual MapIDs, as typed ("1, 2").
    pub map_ids: String,
    /// What runs: "Attack" or "Hunt".
    pub running_mode: Option<String>,
    /// What the run fights with ("Void Highlord, Base", or "the equipped class, Base").
    pub fighting: Option<String>,
    /// How long it has run, while it runs.
    pub running_secs: Option<u64>,
    /// The equipped class, when the owned classes were read.
    pub equipped: Option<String>,
}

pub fn view(bot: &Arc<Bot>) -> AutoView {
    let state = STATE.of(bot);
    let state = lock(&state);
    let running = bot.auto.is_running();
    let run = state.run.as_ref().filter(|_| running);
    let reason = if running {
        Some("Already running")
    } else if bot.scripts.is_snippet_running() {
        Some("Stop the Console snippet first")
    } else if bot.scripts.status().is_active() {
        Some("Stop the script first")
    } else {
        None
    };
    AutoView {
        running,
        start: reason.map_or_else(Control::on, Control::off),
        stop: Control::when(running, "Nothing is running"),
        classes: state.classes.clone(),
        loading: state.loading,
        class: state.class.clone(),
        modes: state
            .class
            .as_deref()
            .map(|c| state.sets.modes_for(c))
            .unwrap_or_default(),
        mode: state.mode.clone(),
        map_ids: state.map_ids.clone(),
        running_mode: bot.auto.mode().map(|m| match m {
            AutoMode::Attack => "Attack".to_owned(),
            AutoMode::Hunt => "Hunt".to_owned(),
        }),
        fighting: run.map(|(_, with)| with.clone()),
        running_secs: run.map(|(at, _)| at.elapsed().as_secs()),
        equipped: state.equipped.clone(),
    }
}

/// VibeSkua `ReloadClasses` (the class drop-down opening): reads the owned classes and the skill
/// sets again, and clears the selection.
pub fn reload_classes(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    {
        let mut state = lock(&state);
        state.sets = SkillSets::load_for(bot);
        state.class = None;
        state.mode = None;
        state.loading = true;
    }
    crate::turu::spawn_action_for(bot.clone(), "Auto classes", move |bot| {
        let read = read_classes(bot);
        let mut state = lock(&state);
        state.loading = false;
        let (classes, equipped) = read?;
        state.classes = classes;
        state.equipped = equipped;
        Ok(())
    });
}

/// The inventory's Class items, sorted ignoring case, and the one equipped.
fn read_classes(bot: &Arc<Bot>) -> turu_bot::BotResult<(Vec<String>, Option<String>)> {
    let items: Vec<serde_json::Value> =
        serde_json::from_value(turu_bot::api::dispatch(bot, "inventory.items", &[])?).unwrap_or_default();
    let mut classes: Vec<String> = items
        .iter()
        .filter(|item| {
            item.get("category")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|c| c.eq_ignore_ascii_case("Class"))
        })
        .filter_map(|item| item.get("name").and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .collect();
    classes.sort_by_key(|name| name.to_lowercase());
    let equipped = items
        .iter()
        .filter(|item| item.get("equipped").and_then(serde_json::Value::as_bool) == Some(true))
        .filter_map(|item| item.get("name").and_then(serde_json::Value::as_str))
        .find(|name| classes.iter().any(|c| c == name))
        .map(str::to_owned);
    Ok((classes, equipped))
}

/// VibeSkua `SelectedClass`: its first saved mode, and the class equipped.
pub fn pick_class(bot: &Arc<Bot>, class: String) {
    let state = STATE.of(bot);
    {
        let mut state = lock(&state);
        if state.sets.is_empty() {
            state.sets = SkillSets::load_for(bot);
        }
        state.mode = state.sets.modes_for(&class).into_iter().next();
        state.class = Some(class.clone());
    }
    crate::turu::spawn_action_for(bot.clone(), "Equip class", move |bot| {
        turu_bot::api::dispatch(bot, "inventory.equipItem", &[json!(class)]).map(|_| ())
    });
}

pub fn pick_mode(bot: &Arc<Bot>, mode: String) {
    lock(&STATE.of(bot)).mode = Some(mode);
}

pub fn set_map_ids(bot: &Arc<Bot>, map_ids: String) {
    lock(&STATE.of(bot)).map_ids = map_ids;
}

/// Auto's actions (`POST /models/auto/do`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum AutoAct {
    /// Reads the classes in the inventory again.
    ReloadClasses,
    /// Picks (and equips) a class.
    Class {
        name: String,
    },
    /// Picks one of the class's saved modes.
    Mode {
        name: String,
    },
    /// Manual MapIDs.
    MapIds {
        ids: String,
    },
    Attack,
    Hunt,
    Stop,
}

pub fn act(bot: &Arc<Bot>, act: AutoAct) -> Result<(), String> {
    match act {
        AutoAct::ReloadClasses => reload_classes(bot),
        AutoAct::Class { name } => pick_class(bot, name),
        AutoAct::Mode { name } => pick_mode(bot, name),
        AutoAct::MapIds { ids } => set_map_ids(bot, ids),
        AutoAct::Attack => return start(bot, AutoMode::Attack),
        AutoAct::Hunt => return start(bot, AutoMode::Hunt),
        AutoAct::Stop => stop(bot),
    }
    Ok(())
}

/// Auto Attack or Auto Hunt with the chosen class and mode (the equipped class in Base when
/// either is unset) and Manual MapIDs.
pub fn start(bot: &Arc<Bot>, mode: AutoMode) -> Result<(), String> {
    if let Some(reason) = Some(view(bot).start).filter(|c| !c.enabled).map(|c| c.reason) {
        return Err(reason);
    }
    let (class, class_mode, map_ids) = {
        let state = STATE.of(bot);
        let state = lock(&state);
        (
            state.class.clone(),
            state.mode.clone(),
            turu_bot::auto::parse_map_ids(&state.map_ids),
        )
    };
    let (with, started) = match (class, class_mode) {
        (Some(class), Some(class_mode)) => (
            format!("{class}, {class_mode}"),
            turu_bot::auto::start(bot, mode, Some(class), class_mode, map_ids),
        ),
        _ => (
            "the equipped class, Base".to_owned(),
            turu_bot::auto::start(bot, mode, None, "Base".into(), map_ids),
        ),
    };
    if started {
        lock(&STATE.of(bot)).run = Some((std::time::Instant::now(), with));
    }
    Ok(())
}

/// Stop Automation.
pub fn stop(bot: &Arc<Bot>) {
    turu_bot::auto::stop(bot);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_offers_start_and_keeps_its_choices() {
        let bot = Bot::new();
        let view = view(&bot);
        assert!(!view.running);
        assert!(view.start.enabled);
        assert_eq!(view.stop, Control::off("Nothing is running"));
        set_map_ids(&bot, "1, 2".into());
        pick_mode(&bot, "Base".into());
        let view = super::view(&bot);
        assert_eq!((view.map_ids.as_str(), view.mode.as_deref()), ("1, 2", Some("Base")));
        // Nothing runs: no run to tell of.
        assert_eq!(
            (view.running_mode, view.fighting, view.running_secs),
            (None, None, None)
        );
    }
}
