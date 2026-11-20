//! Tools & Helpers > Loadouts Manager (VibeSkua `LoadoutsView` / `LoadoutsViewModel`): the
//! logged-in character's saved gear sets (`turu_bot::loadouts`), where each item is now (worn, in
//! the bag, in the bank), and Save current gear, Replace with worn, Equip, Enhance, rename and
//! Delete. One [`State`] per tab, shared by the egui window and a native page, so both show the
//! same "Equipping..."; which loadout is picked and the name being typed stay each shell's.
//!
//! The gear cache is taken at most once a second and the rows are worked out then, not on every
//! read of the view (a shell reads it four times a second).

use super::scheduler::{Note, Tint};
use super::tab_state::{PerTab, lock};
use serde::Serialize;
use serde_json::Value as Json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::gear::{self, GearError, GearFile, Place};
use turu_bot::loadouts::{self, Enhanced, Loadout, SLOTS};

/// How often the gear cache is taken again.
const GEAR_EVERY: Duration = Duration::from_secs(1);

/// What a bot thread is doing for Loadouts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum LoadoutWork {
    Reading,
    Equipping,
    Enhancing,
}

/// Where a loadout's item is, from the gear cache.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum ItemPlace {
    /// No item in the slot, or not a saved loadout (the live entry).
    None,
    Worn,
    Bag,
    Bank,
    /// Neither in the bag nor in the bank as last read.
    Missing,
    /// Not in the bag, and the bank wasn't read yet.
    Unknown,
}

/// What the worn gear read on a bot thread is for.
#[derive(Debug, Clone)]
pub enum WornFor {
    /// A new loadout.
    New,
    /// Replacing the items of the loadout of this name.
    Replace(String),
}

#[derive(Default)]
pub struct State {
    /// Whose list is loaded (the logged-in username).
    pub username: Option<String>,
    pub list: Vec<Loadout>,
    pub busy: Option<LoadoutWork>,
    pub note: Option<Note>,
    /// The worn gear a bot thread read, and what for.
    worn: Option<(WornFor, Loadout)>,
    pub gear: Option<GearFile>,
    gear_at: Option<Instant>,
    /// The last loadout made from what's worn, and a count that changes with each: a shell
    /// selects it and offers its name for typing.
    pub created: Option<String>,
    pub created_count: u32,
    /// The rows as last worked out (`None`: work them out again).
    rows: Option<Rows>,
}

#[derive(Clone)]
struct Rows {
    current: LoadoutView,
    loadouts: Vec<LoadoutView>,
}

pub static STATE: PerTab<State> = PerTab::new();

fn note(text: impl Into<String>, tint: Tint) -> Option<Note> {
    Some(Note {
        text: text.into(),
        tint,
    })
}

/// Where `item` is in `gear`.
pub fn have(gear: &GearFile, item: &str) -> ItemPlace {
    let name = item.replace("&amp;", "&");
    let name = name.trim();
    let matching = |i: &&gear::GearItem| i.name.replace("&amp;", "&").trim().eq_ignore_ascii_case(name);
    let mut found = gear.items.iter().filter(matching).peekable();
    if found.peek().is_none() {
        return if gear.bank_as_of.is_some() {
            ItemPlace::Missing
        } else {
            ItemPlace::Unknown
        };
    }
    let mut best = ItemPlace::Bank;
    for item in found {
        if item.equipped {
            return ItemPlace::Worn;
        }
        if item.place == Place::Inventory {
            best = ItemPlace::Bag;
        }
    }
    best
}

/// Every item of the loadout is worn.
pub fn worn_now(gear: &GearFile, loadout: &Loadout) -> bool {
    let items = loadout.items();
    !items.is_empty() && items.iter().all(|i| have(gear, i) == ItemPlace::Worn)
}

/// "5 min ago", for the bank's date.
pub fn ago(ms: u64) -> String {
    let seconds = gear::now_ms().saturating_sub(ms) / 1000;
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86_400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

/// The running script's file name.
pub fn script_name(bot: &Bot) -> String {
    bot.scripts
        .loaded()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "A script".into())
}

/// The report as one line: "Equipped Farm", or what came up short.
pub fn equip_outcome(name: &str, report: &gear::EquipReport) -> Note {
    let stopped = report
        .stopped_script
        .as_ref()
        .map(|s| format!(" (stopped {s})"))
        .unwrap_or_default();
    if report.missing.is_empty() && report.failed.is_empty() {
        let text = if report.equipped.is_empty() {
            format!("{name} was already worn{stopped}")
        } else {
            format!("Equipped {name}{stopped}")
        };
        return Note {
            text,
            tint: Tint::Success,
        };
    }
    let mut short: Vec<String> = report.missing.iter().map(|m| format!("{m} (not owned)")).collect();
    short.extend(report.failed.iter().map(|f| format!("{} ({})", f.item, f.reason)));
    Note {
        text: format!("{name} partly equipped{stopped}. Not put on: {}", short.join(", ")),
        tint: Tint::Warning,
    }
}

impl State {
    /// Follows the logged-in character: its file when it changed, the gear cache every second,
    /// and what a bot thread read.
    pub fn sync(&mut self, bot: &Bot) {
        let username = bot
            .gear
            .logged_in()
            .then(|| bot.state.lock().username.clone())
            .flatten()
            .filter(|u| !u.trim().is_empty());
        if username != self.username {
            self.username = username;
            self.gear = None;
            self.gear_at = None;
            self.load(bot);
        }
        if self.gear_at.is_none_or(|t| t.elapsed() >= GEAR_EVERY) {
            let gear = bot.gear.last();
            if gear.as_ref().map(|g| (&g.items, g.bank_as_of)) != self.gear.as_ref().map(|g| (&g.items, g.bank_as_of)) {
                self.rows = None;
            }
            self.gear = gear;
            self.gear_at = Some(Instant::now());
        }
        self.take_worn(bot);
    }

    /// Read the file and the worn gear again (the window or page opened, or Refresh).
    pub fn reread(&mut self) {
        self.username = None;
        self.gear_at = None;
    }

    fn path(&self, bot: &Bot) -> Option<std::path::PathBuf> {
        let username = self.username.as_deref()?;
        Some(loadouts::path_for(&bot.paths.loadouts_dir(), username))
    }

    /// VibeSkua `LoadFromFile`. A file that can't be read is kept beside it as `.bad`.
    fn load(&mut self, bot: &Bot) {
        self.list.clear();
        self.rows = None;
        let Some(path) = self.path(bot) else { return };
        match loadouts::load(&path) {
            Ok(list) => self.list = list,
            Err(e) => {
                let bad = path.with_extension("json.bad");
                let _ = std::fs::rename(&path, &bad);
                self.note = note(
                    format!("Couldn't read {} ({e}): kept as {}", path.display(), bad.display()),
                    Tint::Danger,
                );
            }
        }
    }

    /// VibeSkua `SaveToFile`, after every change.
    fn save(&mut self, bot: &Bot) {
        self.rows = None;
        let Some(path) = self.path(bot) else { return };
        if let Err(e) = loadouts::save(&path, &self.list) {
            self.note = note(format!("Couldn't save {}: {e}", path.display()), Tint::Danger);
        }
    }

    /// `base`, or `base 2`, `base 3`, ... when a loadout has that name.
    pub fn unique_name(&self, base: &str) -> String {
        let taken = |n: &str| self.list.iter().any(|l| l.name.eq_ignore_ascii_case(n)) || n == loadouts::CURRENT;
        if !taken(base) {
            return base.to_owned();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|n| !taken(n))
            .expect("a free name")
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.list.iter().position(|l| l.name == name)
    }

    /// Takes what a bot thread read: a new loadout, or new items for one.
    fn take_worn(&mut self, bot: &Bot) {
        let Some((target, worn)) = self.worn.take() else {
            return;
        };
        match target {
            WornFor::New => {
                let base = if worn.class.trim().is_empty() {
                    "Loadout".to_owned()
                } else {
                    worn.class.clone()
                };
                let name = self.unique_name(&base);
                self.list.push(worn.renamed(&name));
                self.save(bot);
                self.note = note(
                    format!("Saved as \u{201C}{name}\u{201D}: rename it if you like"),
                    Tint::Success,
                );
                self.created = Some(name);
                self.created_count = self.created_count.wrapping_add(1);
            }
            WornFor::Replace(name) => {
                if let Some(i) = self.index_of(&name) {
                    self.list[i] = worn.renamed(&name);
                    self.save(bot);
                    self.note = note(format!("{name} now holds what you wear"), Tint::Success);
                }
            }
        }
    }

    /// VibeSkua `SelectedLoadoutName`'s setter, without its question (a rename is undone by
    /// renaming back): the name it has after, the new one or, refused, the old one.
    pub fn rename(&mut self, bot: &Bot, old: &str, new: &str) -> String {
        let Some(index) = self.index_of(old) else {
            return old.to_owned();
        };
        let new = new.trim().to_owned();
        if new == old {
            return new;
        }
        if let Some(problem) = self.rename_problem(index, &new) {
            self.note = note(format!("{problem}: kept \u{201C}{old}\u{201D}"), Tint::Warning);
            return old.to_owned();
        }
        self.list[index].name = new.clone();
        self.save(bot);
        self.note = note(format!("Renamed to \u{201C}{new}\u{201D}"), Tint::Muted);
        new
    }

    /// Why the loadout at `index` can't be called `new`.
    pub fn rename_problem(&self, index: usize, new: &str) -> Option<String> {
        let clash = self
            .list
            .iter()
            .enumerate()
            .any(|(i, l)| i != index && l.name.eq_ignore_ascii_case(new));
        if new.is_empty() {
            Some("A loadout needs a name".to_owned())
        } else if new.eq_ignore_ascii_case(loadouts::CURRENT) {
            Some(format!(
                "\u{201C}{}\u{201D} is the live entry's name",
                loadouts::CURRENT
            ))
        } else if clash {
            Some(format!("Another loadout is called \u{201C}{new}\u{201D}"))
        } else {
            None
        }
    }

    pub fn delete(&mut self, bot: &Bot, name: &str) {
        if let Some(i) = self.index_of(name) {
            self.list.remove(i);
            self.save(bot);
            self.note = note(format!("Deleted \u{201C}{name}\u{201D}"), Tint::Muted);
        }
    }

    /// The live entry: what's worn now.
    pub fn current(&self) -> Loadout {
        self.gear
            .as_ref()
            .map(|g| Loadout::from_worn(loadouts::CURRENT, &g.items))
            .unwrap_or_default()
    }
}

/// Saves what's worn, as a new loadout or into one ([`WornFor`]), read on a bot thread.
pub fn read_worn(bot: &Arc<Bot>, target: WornFor) {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        if s.busy.is_some() {
            return;
        }
        s.busy = Some(LoadoutWork::Reading);
    }
    crate::turu::spawn_action_for(bot.clone(), "Loadouts save", move |bot| {
        let file = gear::refresh(bot);
        let mut s = lock(&state);
        s.busy = None;
        match file {
            Ok(Some(file)) => s.worn = Some((target, Loadout::from_worn("", &file.items))),
            Ok(None) => s.note = note("Not in a map: log in first", Tint::Danger),
            Err(e) => s.note = note(format!("Couldn't read your gear: {e}"), Tint::Danger),
        }
        Ok(())
    });
}

/// Reads the gear now rather than within the gear thread's 30 s.
pub fn read_gear(bot: &Arc<Bot>) {
    lock(&STATE.of(bot)).reread();
    crate::turu::spawn_action_for(bot.clone(), "Loadouts gear", |bot| {
        gear::refresh(bot)?;
        Ok(())
    });
}

/// Equip (VibeSkua `EquipLoadoutAsync`): held while a script runs unless `stop`.
pub fn equip(bot: &Arc<Bot>, loadout: Loadout, stop: bool) {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        if s.busy.is_some() {
            return;
        }
        s.busy = Some(LoadoutWork::Equipping);
        s.note = note(format!("Equipping {}...", loadout.name), Tint::Muted);
    }
    crate::turu::spawn_action_for(bot.clone(), "Loadouts equip", move |bot| {
        let outcome = loadouts::equip(bot, &loadout, stop);
        let _ = gear::refresh(bot);
        let mut s = lock(&state);
        s.busy = None;
        s.gear_at = None;
        s.note = Some(match outcome {
            Ok(report) => equip_outcome(&loadout.name, &report),
            Err(GearError::ScriptRunning(name)) => Note {
                text: format!("{name} started meanwhile: press Equip again to stop it"),
                tint: Tint::Warning,
            },
            Err(e) => Note {
                text: format!("Couldn't equip {}: {e}", loadout.name),
                tint: Tint::Danger,
            },
        });
        Ok(())
    });
}

/// Enhance the worn items as the loadout saved them (VibeSkua's `LoadoutEnhancerTemp`, as a
/// script: it may travel and spend gold).
pub fn enhance(bot: &Arc<Bot>, loadout: Loadout) {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        if s.busy.is_some() {
            return;
        }
        s.busy = Some(LoadoutWork::Enhancing);
        s.note = note(
            format!("Enhancing {}: Stop in the Script Loader stops it", loadout.name),
            Tint::Muted,
        );
    }
    crate::turu::spawn_action_for(bot.clone(), "Loadouts enhance", move |bot| {
        let outcome = loadouts::apply_enhancements(bot, &loadout);
        let _ = gear::refresh(bot);
        let mut s = lock(&state);
        s.busy = None;
        s.gear_at = None;
        s.note = Some(match outcome {
            Ok(Enhanced::AlreadyDone) => Note {
                text: "Already enhanced as saved".to_owned(),
                tint: Tint::Success,
            },
            Ok(Enhanced::Ran { items, finished: true }) => Note {
                text: format!(
                    "Enhanced {items} item{} of {}",
                    if items == 1 { "" } else { "s" },
                    loadout.name
                ),
                tint: Tint::Success,
            },
            Ok(Enhanced::Ran { finished: false, .. }) => Note {
                text: "Enhancing was stopped".to_owned(),
                tint: Tint::Warning,
            },
            Err(e) => Note {
                text: format!("Couldn't enhance: {e}"),
                tint: Tint::Danger,
            },
        });
        Ok(())
    });
}

// --- the view a shell reads -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LoadoutsView {
    /// The character whose loadouts these are ("" while logged out).
    pub username: String,
    /// The gear wasn't read yet.
    pub reading: bool,
    /// What's worn now, as a loadout ("Currently equipped").
    pub current: LoadoutView,
    pub loadouts: Vec<LoadoutView>,
    pub work: Option<LoadoutWork>,
    pub note: Option<Note>,
    /// The running script's file name ("" while none runs): Equip stops it first.
    pub script: String,
    /// When the bank was last read, in words ("5 min ago"; "" when never).
    pub bank_read: String,
    /// The last loadout made from what's worn, and a count that changes with each.
    pub created: String,
    pub created_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LoadoutView {
    pub name: String,
    /// Its class, or "No class".
    pub class: String,
    /// Every item of it is worn now.
    pub worn: bool,
    /// How many items it holds.
    pub items: u32,
    pub slots: Vec<GearSlotView>,
    /// Worn items whose enhancement isn't the saved one: Enhance applies them.
    pub changes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GearSlotView {
    /// "Class", "Weapon", ...
    pub label: String,
    /// "" for an empty slot.
    pub item: String,
    pub place: ItemPlace,
    /// The enhancement this loadout keeps ("" when none is known).
    pub enhancement: String,
    /// The worn item's enhancement when it isn't the saved one ("" when it is).
    pub worn_with: String,
}

fn loadout_view(loadout: &Loadout, gear: Option<&GearFile>, saved: bool) -> LoadoutView {
    let changes = match gear {
        Some(g) if saved => loadouts::enhancement_changes(loadout, &g.items),
        _ => Vec::new(),
    };
    let slots = SLOTS
        .iter()
        .map(|&slot| {
            let item = loadout.item(slot).trim();
            let place = match gear {
                Some(g) if saved && !item.is_empty() => have(g, item),
                _ => ItemPlace::None,
            };
            let enhancement = loadout.enhancement_name(slot).trim();
            let enhancement = if loadouts::enhanced(slot) && !item.is_empty() && enhancement != "Unknown" {
                enhancement.to_owned()
            } else {
                String::new()
            };
            GearSlotView {
                label: loadouts::slot_label(slot).to_owned(),
                item: item.to_owned(),
                place,
                worn_with: changes
                    .iter()
                    .find(|c| c.slot == slot)
                    .map(|c| c.worn.name().to_owned())
                    .unwrap_or_default(),
                enhancement,
            }
        })
        .collect();
    LoadoutView {
        name: loadout.name.clone(),
        class: if loadout.class.trim().is_empty() {
            "No class".to_owned()
        } else {
            loadout.class.clone()
        },
        worn: saved && gear.is_some_and(|g| worn_now(g, loadout)),
        items: loadout.items().len() as u32,
        slots,
        changes: changes.len() as u32,
    }
}

impl State {
    fn rows(&mut self) -> Rows {
        if self.rows.is_none() {
            let gear = self.gear.as_ref();
            let mut current = loadout_view(&self.current(), gear, false);
            current.name = "Currently equipped".to_owned();
            self.rows = Some(Rows {
                current,
                loadouts: self.list.iter().map(|l| loadout_view(l, gear, true)).collect(),
            });
        }
        self.rows.clone().expect("worked out above")
    }

    pub fn view(&mut self, bot: &Bot) -> LoadoutsView {
        let rows = self.rows();
        LoadoutsView {
            username: self.username.clone().unwrap_or_default(),
            reading: self.gear.is_none(),
            current: rows.current,
            loadouts: rows.loadouts,
            work: self.busy,
            note: self.note.clone(),
            script: if bot.scripts.is_running() {
                script_name(bot)
            } else {
                String::new()
            },
            bank_read: self
                .gear
                .as_ref()
                .and_then(|g| g.bank_as_of)
                .map(ago)
                .unwrap_or_default(),
            created: self.created.clone().unwrap_or_default(),
            created_count: self.created_count,
        }
    }
}

/// The tab's view.
pub fn view(bot: &Arc<Bot>) -> LoadoutsView {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    s.sync(bot);
    s.view(bot)
}

/// `open` (read the file and the gear again), `save-current`, `replace` (`name`), `equip` (`name`,
/// `stop`: stop a running script first), `enhance` (`name`), `rename` (`name`, `to`), `delete`
/// (`name`), `dismiss` (the note).
pub fn act(bot: &Arc<Bot>, action: &str, body: &Json) -> Result<(), String> {
    let name = || {
        body.get("name")
            .and_then(Json::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "name is missing".to_owned())
    };
    let state = STATE.of(bot);
    let loadout = |name: &str| {
        let s = lock(&state);
        s.list
            .iter()
            .find(|l| l.name == name)
            .cloned()
            .ok_or_else(|| format!("no loadout {name}"))
    };
    match action {
        "open" => read_gear(bot),
        "save-current" => read_worn(bot, WornFor::New),
        "replace" => read_worn(bot, WornFor::Replace(name()?)),
        "equip" => {
            let stop = body.get("stop").and_then(Json::as_bool).unwrap_or(false);
            equip(bot, loadout(&name()?)?, stop);
        }
        "enhance" => enhance(bot, loadout(&name()?)?),
        "rename" => {
            let to = body.get("to").and_then(Json::as_str).ok_or("to is missing")?;
            lock(&state).rename(bot, &name()?, to);
        }
        "delete" => lock(&state).delete(bot, &name()?),
        "dismiss" => lock(&state).note = None,
        other => return Err(format!("no action {other}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use turu_bot::gear::{GearItem, Slot};

    fn item(name: &str, slot: Slot, equipped: bool, place: Place) -> GearItem {
        GearItem {
            id: 1,
            name: name.into(),
            category: String::new(),
            slot,
            equipped,
            place,
            enhancement_level: 0,
            enhancement_pattern_id: 9,
            proc_id: 0,
        }
    }

    #[test]
    fn says_where_each_item_is() {
        let mut gear = GearFile {
            items: vec![
                item("Legion Revenant", Slot::Class, true, Place::Inventory),
                item("Blade & Shield", Slot::Weapon, false, Place::Inventory),
                item("Cape", Slot::Cape, false, Place::Bank),
            ],
            ..Default::default()
        };
        assert_eq!(have(&gear, "legion revenant"), ItemPlace::Worn);
        assert_eq!(have(&gear, "Blade &amp; Shield"), ItemPlace::Bag);
        assert_eq!(have(&gear, "Cape"), ItemPlace::Bank);
        assert_eq!(have(&gear, "Pet"), ItemPlace::Unknown);
        gear.bank_as_of = Some(1);
        assert_eq!(have(&gear, "Pet"), ItemPlace::Missing);
        let loadout = Loadout {
            name: "Farm".into(),
            class: "Legion Revenant".into(),
            weapon: "Blade & Shield".into(),
            ..Default::default()
        };
        assert!(!worn_now(&gear, &loadout), "the weapon is in the bag");
        assert!(!worn_now(&gear, &Loadout::default()));
        let view = loadout_view(&loadout, Some(&gear), true);
        assert_eq!((view.items, view.worn), (2, false));
        let weapon = view.slots.iter().find(|s| s.item == "Blade & Shield").unwrap();
        assert_eq!(weapon.place, ItemPlace::Bag);
        assert_eq!(loadout_view(&Loadout::default(), None, true).class, "No class");
    }

    #[test]
    fn names_new_loadouts_apart_and_refuses_bad_renames() {
        let mut state = State::default();
        state.list.push(Loadout {
            name: "Legion Revenant".into(),
            ..Default::default()
        });
        state.list.push(Loadout {
            name: "Farm".into(),
            ..Default::default()
        });
        assert_eq!(state.unique_name("Legion Revenant"), "Legion Revenant 2");
        assert_eq!(state.unique_name("Quest"), "Quest");
        assert_eq!(state.rename_problem(1, "").as_deref(), Some("A loadout needs a name"));
        assert!(
            state
                .rename_problem(1, "legion revenant")
                .unwrap()
                .starts_with("Another loadout")
        );
        assert!(
            state
                .rename_problem(1, loadouts::CURRENT)
                .unwrap()
                .contains("live entry")
        );
        assert_eq!(state.rename_problem(1, "Questwalker's farm"), None);
    }
}
