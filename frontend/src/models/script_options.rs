//! Scripts > Edit Script Options (VibeSkua's "Script Options" dialog): the loaded script's options
//! (or a Scheduler entry's), edited as a copy and saved to `options/<Storage>.cfg`, with the script
//! waiting meanwhile when it asked (before a run, or `bot.config.configure`).
//!
//! One [`State`] per tab, shared by the egui window and a native page: the draft, whether it's a
//! queued entry's, the script's request it answers, the last error, and whether the window was
//! handed to the native shell. The request is finished only by Save, Cancel or the window's own
//! close, never by the egui window hiding while the native page shows it.
//!
//! What the editor shows is worked out here from the declaration ([`Plan`]): each group, a group
//! of many switches as a checklist (with its link to the choice that uses it), a choice's
//! explanation options shown under it instead of as rows, and an item option's candidates from
//! the account's items.

use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::gear::{self, GearFile, GearItem, Place, Slot};
use turu_bot::script::ScriptOptionsState;
use turu_bot::script::options::{ItemPick, Kind, MAIN, OptionDecl, ScriptOptions};

/// A group of this many switches or more is a checklist.
pub const CHECKLIST: usize = 8;
/// How often the account's items are taken from the gear cache again.
const GEAR_READ: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct State {
    /// The values being edited, taken when the editor opens.
    pub draft: Option<ScriptOptions>,
    /// The draft is a Scheduler (or Army Queue) entry's options (VibeSkua `EditScriptConfig`):
    /// Save writes its own file and leaves the loaded script alone.
    pub queued: bool,
    /// The script's request (`bot.config.configure`, or before a run) the editor answers.
    pub request: Option<u64>,
    /// Why the last Save failed.
    pub error: Option<String>,
    /// The native shell shows the editor: the egui window hiding isn't a Cancel.
    pub native: bool,
    /// The checklist groups that hide the items the account owns.
    pub hide_owned: HashSet<String>,
    gear: Option<GearFile>,
    gear_at: Option<Instant>,
    asked_gear: bool,
}

pub static STATE: PerTab<State> = PerTab::new();

/// What [`State::follow_request`] saw.
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    /// The script asks for its options now: show the editor.
    Opened,
    /// The script stopped while it waited: close the editor.
    Gone,
    None,
}

impl State {
    /// Follows the script's request for its options (called every frame by the egui window).
    pub fn follow_request(&mut self, bot: &Bot) -> Request {
        let pending = bot.scripts.configure_request();
        if pending.is_some() && pending != self.request {
            self.request = pending;
            // The script asks for its own options: a fresh draft of them, never one left over.
            self.queued = false;
            self.draft = None;
            self.error = None;
            Request::Opened
        } else if self.request.is_some() && pending != self.request {
            self.request = None;
            self.draft = None;
            self.error = None;
            self.native = false;
            Request::Gone
        } else {
            Request::None
        }
    }

    /// The loaded script's options as the draft, when none is being edited; a draft left from a
    /// script no longer loaded (another was loaded since) is dropped. A queued entry's stays.
    pub fn ensure_draft(&mut self, bot: &Bot) {
        if self.queued {
            return;
        }
        match bot.scripts.options() {
            ScriptOptionsState::Loaded(options) => {
                let stale = self
                    .draft
                    .as_ref()
                    .is_some_and(|d| d.declaration() != options.declaration() || d.path() != options.path());
                if self.draft.is_none() || stale {
                    self.draft = Some(options);
                    self.error = None;
                }
            }
            _ => {
                self.draft = None;
                self.error = None;
            }
        }
    }

    /// The native page went (the panel closed, another page, another tab): with a script
    /// waiting, the slot's own window takes the editor back; else the draft goes.
    pub fn leave(&mut self, bot: &Bot) {
        if self.request.is_some() {
            self.native = false;
        } else {
            self.close(bot);
        }
    }

    /// A Scheduler entry's options (read from its `Queue_<name>_<id>` file) to edit.
    pub fn edit_queued(&mut self, options: ScriptOptions) {
        self.draft = Some(options);
        self.queued = true;
        self.error = None;
    }

    pub fn set(&mut self, category: &str, name: &str, value: Value) {
        if let Some(draft) = self.draft.as_mut()
            && let Err(e) = draft.set_in(category, name, value)
        {
            self.error = Some(e);
        }
    }

    /// Save: the queued entry's file, or the loaded script's options; then the editor closes.
    pub fn save(&mut self, bot: &Bot) -> Result<(), String> {
        let draft = self.draft.as_ref().ok_or("Nothing to save")?;
        let saved = if self.queued {
            draft
                .save()
                .map_err(|e| format!("couldn't save {}: {e}", draft.path().display()))
        } else {
            bot.scripts.save_options(draft.clone())
        };
        match saved {
            Ok(()) => {
                self.close(bot);
                Ok(())
            }
            Err(e) => {
                self.error = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Cancel, Save done, or the window closed: the waiting script goes on and the draft goes.
    pub fn close(&mut self, bot: &Bot) {
        if let Some(request) = self.request.take() {
            bot.scripts.finish_configure(request);
        }
        self.draft = None;
        self.queued = false;
        self.error = None;
        self.native = false;
        self.hide_owned.clear();
    }

    /// The account's items, from the gear cache (read once now when logged in and the cache has
    /// none yet).
    pub fn gear(&mut self, bot: &Arc<Bot>) -> Gear<'_> {
        if self.gear_at.is_none_or(|t| t.elapsed() >= GEAR_READ) {
            self.gear_at = Some(Instant::now());
            self.gear = bot.gear.last();
            if self.gear.is_none() && bot.gear.logged_in() && !self.asked_gear {
                self.asked_gear = true;
                crate::turu::spawn_action_for(bot.clone(), "Script Options gear", |bot| {
                    gear::refresh(bot)?;
                    Ok(())
                });
            }
        }
        Gear {
            file: self.gear.as_ref(),
            reading: self.asked_gear && bot.gear.logged_in(),
        }
    }
}

/// The native shell took the editor (its page shows the draft): the egui window hiding is no
/// Cancel.
pub fn hand_over(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    s.native = true;
    s.ensure_draft(bot);
}

// --- what the editor lays out ------------------------------------------------------------

/// What the editor lays out, worked out from the declaration.
pub struct Plan {
    pub categories: Vec<Category>,
    /// Per checklist group, the choice that makes it used (merge `Select` and `Generic:mode`).
    pub links: HashMap<String, Link>,
    /// The options with a row (explanations aren't counted).
    pub option_count: usize,
}

pub struct Category {
    pub name: String,
    /// The name as shown: `_` as spaces.
    pub title: String,
    pub options: Vec<OptionDecl>,
    /// A group of switches long enough to show as a checklist.
    pub checklist: bool,
    /// Per choice option, the explanation of each of its choices.
    pub explanations: HashMap<String, HashMap<String, String>>,
    /// The text options that are explanations, shown under their choice instead of as rows.
    pub explanation_options: HashSet<String>,
}

/// A choice one of whose names is a checklist group's name: the group is used only when that
/// choice is picked (CoreAdvanced's merge `Mode` and its `Select` group).
pub struct Link {
    pub category: String,
    pub option: String,
    pub display_name: String,
    pub choice: String,
}

impl Plan {
    pub fn of(draft: &ScriptOptions) -> Self {
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

    /// Its rows: every option but the explanations.
    pub fn rows(&self) -> impl Iterator<Item = &OptionDecl> {
        self.options
            .iter()
            .filter(|o| !self.explanation_options.contains(&o.name))
    }
}

/// The account's items for the pickers and checklists.
pub struct Gear<'a> {
    pub file: Option<&'a GearFile>,
    /// The items are being read (logged in, the cache had none yet).
    pub reading: bool,
}

impl Gear<'_> {
    /// Where the item with this ID is: the inventory before the bank.
    pub fn place_of(&self, id: i64) -> Option<Place> {
        let items = self.file?.items.iter().filter(|i| i.id == id);
        items.map(|i| i.place).min_by_key(|p| *p == Place::Bank)
    }

    pub fn bank_read(&self) -> bool {
        self.file.is_some_and(|f| f.bank_as_of.is_some())
    }
}

/// Whether the filter keeps the option.
pub fn matches(option: &OptionDecl, query: &str) -> bool {
    query.is_empty() || option.display_name.to_lowercase().contains(query) || option.name.to_lowercase().contains(query)
}

/// The account's items an item option offers: its slot's (or every wearable but ground items),
/// inventory first, then bank, each by name, one per name.
pub fn candidates(file: &GearFile, pick: ItemPick) -> Vec<&GearItem> {
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
pub fn place_tag(item: &GearItem) -> Option<&'static str> {
    if item.equipped {
        Some("Worn")
    } else if item.place == Place::Bank {
        Some("Bank")
    } else {
        None
    }
}

/// A warning under an item option whose name isn't among the account's items.
pub fn missing_note(pick: ItemPick, current: &str, gear: &Gear) -> Option<String> {
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

/// What an item option picks: `("helm", "a helm")`.
pub fn noun(pick: ItemPick) -> (&'static str, &'static str) {
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

pub fn plural(noun: &str) -> String {
    if noun.ends_with('s') {
        format!("{noun}es")
    } else {
        format!("{noun}s")
    }
}

/// "acOnly" -> "AcOnly", as CoreAdvanced names its explanation options.
pub fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// An explanation option's description without its "Mode [all]:" lead.
pub fn explanation(description: &str) -> String {
    let text = description.split_once("]:").map_or(description, |(_, rest)| rest);
    tidy(text)
}

/// A description on one line's terms: tabs and runs of spaces as one space, each line trimmed.
pub fn tidy(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// A default as the window's tooltip shows it (VibeSkua: "Default: True").
pub fn default_text(value: &Value) -> String {
    match value {
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(text) if text.is_empty() => "(empty)".into(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

// --- the view a native page reads -----------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum ScriptOptionKind {
    Switch,
    Number,
    Text,
    Choice,
    /// A text holding an item's name, picked from the account's items or typed.
    Item,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ScriptOptionsView {
    /// A draft is open (else `message` says why not).
    pub editing: bool,
    pub message: String,
    /// "Saved in options/Farm.cfg".
    pub saved_in: String,
    /// A Scheduler entry's options, not the loaded script's.
    pub queued: bool,
    /// The script waits until the editor closes.
    pub waiting: bool,
    pub error: String,
    /// Logged in and the account's items are being read.
    pub reading_items: bool,
    pub groups: Vec<ScriptOptionGroupView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ScriptOptionGroupView {
    /// Its key for the actions ("" for the script's own options, shown untitled).
    pub name: String,
    pub title: String,
    /// Many switches: a checklist with All, None and Hide owned.
    pub checklist: bool,
    pub picked: u32,
    pub total: u32,
    /// How many of its items the account owns, and whether those are hidden.
    pub owned: u32,
    pub hiding_owned: bool,
    /// "Used only when Mode is set to select. It's set to all now." ("" when it's used).
    pub unused_note: String,
    /// The choice that makes it used, for the note's button ("Use select").
    pub use_category: String,
    pub use_option: String,
    pub use_choice: String,
    pub rows: Vec<ScriptOptionRowView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ScriptOptionRowView {
    pub name: String,
    pub label: String,
    pub description: String,
    pub kind: ScriptOptionKind,
    pub on: bool,
    pub number: i64,
    pub text: String,
    /// A choice's names (`key`) and words (`_` as spaces).
    pub choices: Vec<ScriptOptionChoice>,
    /// What the picked choice does ("" without an explanation).
    pub explanation: String,
    /// "Default: True".
    pub default_words: String,
    /// In a checklist: "Owned", "In bank" or "".
    pub owned: String,
    /// An item option: "helm", and the account's candidates, and the warning when the name
    /// isn't among them.
    pub item_noun: String,
    pub candidates: Vec<ScriptOptionItem>,
    pub missing: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ScriptOptionChoice {
    pub key: String,
    pub words: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ScriptOptionItem {
    pub name: String,
    /// "Worn", "Bank" or "".
    pub tag: String,
}

fn row_view(category: &Category, option: &OptionDecl, value: &Value, gear: &Gear) -> ScriptOptionRowView {
    let kind = match (option.kind, option.item) {
        (Kind::Bool, _) => ScriptOptionKind::Switch,
        (Kind::Int, _) => ScriptOptionKind::Number,
        (Kind::Choice, _) => ScriptOptionKind::Choice,
        (Kind::Text, Some(_)) => ScriptOptionKind::Item,
        (Kind::Text, None) => ScriptOptionKind::Text,
    };
    let text = value.as_str().unwrap_or_default().to_owned();
    let explanation = category
        .explanations
        .get(&option.name)
        .and_then(|texts| texts.get(&text))
        .cloned()
        .unwrap_or_default();
    let owned = if category.checklist {
        match option.name.parse::<i64>().ok().and_then(|id| gear.place_of(id)) {
            Some(Place::Bank) => "In bank",
            Some(_) => "Owned",
            None => "",
        }
    } else {
        ""
    };
    let (item_noun, candidates, missing) = match option.item {
        Some(pick) => (
            noun(pick).0.to_owned(),
            gear.file
                .map(|file| {
                    candidates(file, pick)
                        .into_iter()
                        .map(|i| ScriptOptionItem {
                            name: i.name.clone(),
                            tag: place_tag(i).unwrap_or("").to_owned(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            missing_note(pick, &text, gear).unwrap_or_default(),
        ),
        None => (String::new(), Vec::new(), String::new()),
    };
    ScriptOptionRowView {
        name: option.name.clone(),
        label: option.display_name.clone(),
        description: tidy(&option.description),
        kind,
        on: value.as_bool().unwrap_or(false),
        number: value.as_i64().unwrap_or(0),
        text,
        choices: option
            .choices
            .iter()
            .map(|c| ScriptOptionChoice {
                key: c.clone(),
                words: c.replace('_', " "),
            })
            .collect(),
        explanation,
        default_words: format!("Default: {}", default_text(&option.default_value)),
        owned: owned.to_owned(),
        item_noun,
        candidates,
        missing,
    }
}

impl State {
    /// What a native page shows. Only reads: the page's `open` takes the draft.
    pub fn view(&mut self, bot: &Arc<Bot>) -> ScriptOptionsView {
        let message = match (&self.draft, bot.scripts.options()) {
            (Some(_), _) => String::new(),
            (None, ScriptOptionsState::Failed(e)) => format!("The script's options couldn't be read: {e}"),
            (None, _) => {
                "The loaded script declares no options. Load one that has some, from the Script Loader.".to_owned()
            }
        };
        let mut groups = Vec::new();
        let (saved_in, queued, waiting, error) = (
            self.draft
                .as_ref()
                .and_then(|d| d.path().file_name())
                .map(|n| format!("Saved in options/{}", n.to_string_lossy()))
                .unwrap_or_default(),
            self.queued,
            self.request.is_some(),
            self.error.clone().unwrap_or_default(),
        );
        let hide_owned = self.hide_owned.clone();
        let draft = self.draft.clone();
        let gear = self.gear(bot);
        let reading_items = gear.file.is_none() && gear.reading;
        if let Some(draft) = &draft {
            let plan = Plan::of(draft);
            for category in &plan.categories {
                let value_of = |name: &str| draft.get_in(&category.name, name).cloned().unwrap_or(Value::Null);
                let mut rows: Vec<ScriptOptionRowView> = category
                    .rows()
                    .map(|o| row_view(category, o, &value_of(&o.name), &gear))
                    .collect();
                let picked = rows
                    .iter()
                    .filter(|r| r.kind == ScriptOptionKind::Switch && r.on)
                    .count() as u32;
                let owned = rows.iter().filter(|r| !r.owned.is_empty()).count() as u32;
                let hiding = category.checklist && owned > 0 && hide_owned.contains(&category.name);
                let total = rows.len() as u32;
                if hiding {
                    rows.retain(|r| r.owned.is_empty());
                }
                let (unused_note, use_category, use_option, use_choice) = match plan.links.get(&category.name) {
                    Some(link) => {
                        let now = draft
                            .get_in(&link.category, &link.option)
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        if now == link.choice {
                            Default::default()
                        } else {
                            (
                                format!(
                                    "Used only when \u{201c}{}\u{201d} is set to {}. It's set to {} now.",
                                    link.display_name,
                                    link.choice.replace('_', " "),
                                    now.replace('_', " ")
                                ),
                                link.category.clone(),
                                link.option.clone(),
                                link.choice.clone(),
                            )
                        }
                    }
                    None => Default::default(),
                };
                groups.push(ScriptOptionGroupView {
                    name: category.name.clone(),
                    title: if category.name == MAIN {
                        String::new()
                    } else {
                        category.title.clone()
                    },
                    checklist: category.checklist,
                    picked,
                    total,
                    owned,
                    hiding_owned: hiding,
                    unused_note,
                    use_category,
                    use_option,
                    use_choice,
                    rows,
                });
            }
        }
        ScriptOptionsView {
            editing: draft.is_some(),
            message,
            saved_in,
            queued,
            waiting,
            error,
            reading_items,
            groups,
        }
    }
}

pub fn view(bot: &Arc<Bot>) -> ScriptOptionsView {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    s.view(bot)
}

/// Script Options' actions (`POST /models/script-options/do`); `category` is a group's name.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum ScriptOptionsAct {
    /// Takes the loaded script's options, unless a draft is open.
    Open,
    /// The page went.
    Leave,
    Set {
        category: String,
        name: String,
        value: super::form::OptionValue,
    },
    /// Sets each of `names` to `value` (All and None).
    SetAll {
        category: String,
        names: Vec<String>,
        value: super::form::OptionValue,
    },
    HideOwned {
        category: String,
        on: bool,
    },
    /// Every option back to its default.
    Reset,
    Save,
    Cancel,
}

pub fn act(bot: &Arc<Bot>, act: ScriptOptionsAct) -> Result<(), String> {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    match act {
        ScriptOptionsAct::Open => {
            s.native = true;
            s.ensure_draft(bot);
        }
        ScriptOptionsAct::Set { category, name, value } => {
            s.error = None;
            s.set(&category, &name, value.to_json());
        }
        ScriptOptionsAct::SetAll { category, names, value } => {
            for name in &names {
                s.set(&category, name, value.to_json());
            }
        }
        ScriptOptionsAct::HideOwned { category, on } => {
            if on {
                s.hide_owned.insert(category);
            } else {
                s.hide_owned.remove(&category);
            }
        }
        ScriptOptionsAct::Reset => {
            if let Some(draft) = s.draft.as_mut() {
                draft.reset();
            }
        }
        ScriptOptionsAct::Save => s.save(bot)?,
        ScriptOptionsAct::Cancel => s.close(bot),
        ScriptOptionsAct::Leave => s.leave(bot),
    }
    Ok(())
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

    /// Answers the few game calls a script makes, as `bot/tests/runtime.rs`'s fake does.
    struct FakeGame;

    impl turu_bot::FlashHost for FakeGame {
        fn call(&self, name: &str, args: Vec<turu_bot::FlashArg>) -> Result<String, turu_bot::HostError> {
            if let Some(answer) = turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default()) {
                return Ok(answer);
            }
            Ok(if name == "isLoggedIn" { "true" } else { "null" }.into())
        }
    }

    fn set_on(on: bool) -> ScriptOptionsAct {
        ScriptOptionsAct::Set {
            category: MAIN.into(),
            name: "On".into(),
            value: super::super::form::OptionValue::Switch(on),
        }
    }

    fn wait(what: &str, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < Duration::from_secs(10), "{what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A script asks for its options: the egui window opens for it, hands it to the native
    /// page, and only the page's Save (or Cancel) lets the script go on; the page going away
    /// meanwhile gives the editor back to the window.
    #[test]
    fn a_waiting_script_goes_on_only_after_save_or_cancel() {
        let dir = std::env::temp_dir().join(format!("turu-script-options-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        turu_bot::paths::set_default_root(dir.join("data"));
        let script = dir.join("main.ts");
        std::fs::write(
            &script,
            r#"import { bot, defineScriptOptions, option } from "turu";
export const scriptOptions = defineScriptOptions("Questwalker", [option.bool("On", "On", "")], true);
export default function main(): void {
    bot.config.configure();
    bot.log(`done ${bot.config.get("On")}`);
}
"#,
        )
        .unwrap();
        let bot = Arc::new(Bot::new());
        bot.set_host(Some(Arc::new(FakeGame)));
        bot.scripts.set_interactive(true);
        bot.scripts.load(&script);
        let state = STATE.of(&bot);

        // Save: the change is saved and the script goes on with it.
        turu_bot::script::start(&bot).unwrap();
        wait("no request", || bot.scripts.configure_request().is_some());
        assert_eq!(lock(&state).follow_request(&bot), Request::Opened);
        hand_over(&bot);
        assert!(view(&bot).waiting && view(&bot).editing);
        act(&bot, set_on(true)).unwrap();
        // The page going away gives the editor back to the window, still waiting.
        act(&bot, ScriptOptionsAct::Leave).unwrap();
        assert!(!lock(&state).native && view(&bot).waiting);
        act(&bot, ScriptOptionsAct::Save).unwrap();
        wait("the script didn't go on", || !bot.scripts.is_running());
        assert!(
            bot.logs.script.text().contains("done true"),
            "{}",
            bot.logs.script.text()
        );
        assert_eq!(bot.scripts.configure_request(), None);
        assert!(!view(&bot).editing);

        // Cancel: the script goes on with what was saved.
        turu_bot::script::start(&bot).unwrap();
        wait("no second request", || bot.scripts.configure_request().is_some());
        assert_eq!(lock(&state).follow_request(&bot), Request::Opened);
        hand_over(&bot);
        act(&bot, set_on(false)).unwrap();
        act(&bot, ScriptOptionsAct::Cancel).unwrap();
        wait("the script didn't go on after Cancel", || !bot.scripts.is_running());
        assert_eq!(bot.logs.script.text().matches("done true").count(), 2);
    }

    #[test]
    fn nothing_open_says_why() {
        let bot = Arc::new(Bot::new());
        let view = view(&bot);
        assert!(!view.editing && !view.waiting);
        assert!(view.message.contains("declares no options"));
        // Cancel with nothing open does nothing, and Save refuses.
        assert!(act(&bot, ScriptOptionsAct::Cancel).is_ok());
        assert!(act(&bot, ScriptOptionsAct::Save).is_err());
    }
}
