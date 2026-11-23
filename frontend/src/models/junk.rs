//! Tools & Helpers > Junk Items (VibeSkua `JunkItemsView` / `JunkItemsViewModel`): the inventory
//! and bank items, marking some as junk (`JunkItems.json`), quantity goals for them
//! (`InventoryGoals`), and selling the junk in the inventory. The list and the sale are
//! `turu_bot::junk`, run on bot threads.
//!
//! Per tab ([`State`]): the items as last read (with whether each is junk and its goal), whether a
//! refresh or sale runs, and the last error. The egui window keeps its own search, filters and
//! selection and passes the selected items in; a native page's search, filters, page and selection
//! are kept here. Every function here locks the state only briefly and never while calling the
//! bot, so a shell may call them from a frame.

use super::tab_state::{PerTab, Shared, lock};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::junk::{self, AcFilter, Entry, Filter, Highlight};

/// A native page's items per page.
pub const PAGE_SIZE: usize = 100;

/// An item as the selection knows it: its ID and category (as VibeSkua keeps a selection over a
/// refresh).
pub type Key = (i64, String);

pub fn key(entry: &Entry) -> Key {
    (entry.id, entry.category.clone())
}

#[derive(Default)]
pub struct State {
    entries: Arc<Vec<Entry>>,
    /// Counts the lists read or changed.
    revision: u64,
    /// Refreshing or selling.
    busy: bool,
    /// Why the last refresh, save or sale failed.
    error: Option<String>,
    native: Native,
}

/// A native page's search, filters, page and selection.
#[derive(Default)]
struct Native {
    filter: Filter,
    page: usize,
    selected: HashSet<Key>,
}

pub static STATE: PerTab<State> = PerTab::new();

/// What the egui window draws from.
pub struct Snapshot {
    pub entries: Arc<Vec<Entry>>,
    pub revision: u64,
    pub busy: bool,
    pub error: Option<String>,
}

pub fn snapshot(bot: &Arc<Bot>) -> Snapshot {
    let state = STATE.of(bot);
    let s = lock(&state);
    Snapshot {
        entries: s.entries.clone(),
        revision: s.revision,
        busy: s.busy,
        error: s.error.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct JunkView {
    /// Refreshing or selling.
    pub busy: bool,
    pub error: Option<String>,
    /// Items read (inventory and bank).
    pub total: u32,
    pub search: String,
    /// The category filter's options, "All Categories" first, and the one chosen.
    pub categories: Vec<String>,
    pub category: String,
    /// The AC filter's options and the one chosen.
    pub acs: Vec<String>,
    pub ac: String,
    /// The highlight filter's options and the one chosen.
    pub highlights: Vec<String>,
    pub highlight: String,
    /// Items selected, shown or not.
    pub selected: u32,
    /// Every shown item is selected (and some are).
    pub all_shown: bool,
    /// Items marked as junk.
    pub junk: u32,
    /// VibeSkua `JunkSkipSellWarning`.
    pub skip_warning: bool,
    pub matches: u32,
    pub page: u32,
    pub pages: u32,
    pub rows: Vec<JunkRowView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct JunkRowView {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub ac: bool,
    pub equipped: bool,
    pub favorite: bool,
    /// An inventory stack is at least 80% full.
    pub near_cap: bool,
    /// "Inv: 3  |  Bank: 5".
    pub quantities: String,
    /// "12/50" toward a quantity goal.
    pub goal: Option<String>,
    pub junk: bool,
    pub selected: bool,
}

fn options<T: Copy>(all: &[T], label: impl Fn(T) -> &'static str) -> Vec<String> {
    all.iter().map(|&o| label(o).to_owned()).collect()
}

pub fn view(bot: &Arc<Bot>) -> JunkView {
    let state = STATE.of(bot);
    let (entries, filter) = {
        let s = lock(&state);
        (s.entries.clone(), s.native.filter.clone())
    };
    let shown: Vec<usize> = (0..entries.len()).filter(|&i| filter.matches(&entries[i])).collect();
    let skip_warning = bot.settings.read().junk_skip_sell_warning;
    let s = lock(&state);
    let n = &s.native;
    let pages = shown.len().div_ceil(PAGE_SIZE).max(1);
    let page = n.page.min(pages - 1);
    let rows = shown
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .map(|&i| {
            let e = &entries[i];
            JunkRowView {
                id: e.id,
                name: e.name.clone(),
                category: e.category.clone(),
                ac: e.ac,
                equipped: e.equipped,
                favorite: e.favorite,
                near_cap: e.near_stack_limit,
                quantities: e.quantity_display(),
                goal: e.goal_tracked().then(|| e.goal_progress()),
                junk: e.junk,
                selected: n.selected.contains(&key(e)),
            }
        })
        .collect();
    let mut categories = junk::categories(&entries);
    if entries.is_empty() {
        categories = vec![junk::ALL_CATEGORIES.into()];
    }
    JunkView {
        busy: s.busy,
        error: s.error.clone(),
        total: entries.len() as u32,
        search: n.filter.search.clone(),
        categories,
        category: n.filter.category.clone(),
        acs: options(&AcFilter::ALL, AcFilter::label),
        ac: n.filter.ac.label().into(),
        highlights: options(&Highlight::ALL, Highlight::label),
        highlight: n.filter.highlight.label().into(),
        selected: n.selected.len() as u32,
        all_shown: !shown.is_empty() && shown.iter().all(|&i| n.selected.contains(&key(&entries[i]))),
        junk: entries.iter().filter(|e| e.junk).count() as u32,
        skip_warning,
        matches: shown.len() as u32,
        page: page as u32,
        pages: pages as u32,
        rows,
    }
}

/// Hands a fresh list to every shell: the native page's selection keeps the items still there,
/// and its category goes back to "All Categories" when the chosen one is gone.
fn take(state: &Shared<State>, entries: Vec<Entry>) {
    let mut s = lock(state);
    let keys: HashSet<Key> = entries.iter().map(key).collect();
    s.native.selected.retain(|k| keys.contains(k));
    if !junk::categories(&entries).contains(&s.native.filter.category) {
        s.native.filter.category = junk::ALL_CATEGORIES.into();
    }
    s.entries = Arc::new(entries);
    s.revision += 1;
}

/// Starts `work` on a bot thread, busy until it ends; false when something runs already.
fn run(bot: &Arc<Bot>, name: &str, work: impl FnOnce(&Bot) -> Result<Vec<Entry>, String> + Send + 'static) -> bool {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        if s.busy {
            return false;
        }
        s.busy = true;
        s.error = None;
    }
    crate::turu::spawn_action_for(bot.clone(), name, move |bot| {
        let result = work(bot);
        match result {
            Ok(entries) => take(&state, entries),
            Err(e) => lock(&state).error = Some(e),
        }
        lock(&state).busy = false;
        Ok(())
    });
    true
}

/// VibeSkua `Refresh`: reads the list again, then the inventory and bank (loading the bank first
/// if needed). Selections are kept.
pub fn refresh(bot: &Arc<Bot>) {
    run(bot, "Junk Items", |bot| {
        junk::entries(bot).map_err(|e| format!("Couldn't read the inventory: {e}"))
    });
}

/// VibeSkua `SellAllJunk` then `Refresh`.
pub fn sell_all(bot: &Arc<Bot>) {
    run(bot, "Sell junk", |bot| {
        junk::sell_all(bot)
            .and_then(|_| junk::entries(bot))
            .map_err(|e| format!("Selling failed: {e}"))
    });
}

/// Marks `keys` as junk (`true`) or not, then saves the list (VibeSkua `PersistFromEntries`).
pub fn mark(bot: &Arc<Bot>, keys: &HashSet<Key>, on: bool) {
    change(bot, |e| {
        if keys.contains(&key(e)) {
            e.junk = on;
        }
    });
}

/// Unmark all.
pub fn unmark_all(bot: &Arc<Bot>) {
    change(bot, |e| e.junk = false);
}

/// VibeSkua "Sell selected": marks `keys` as junk, saves, then sells all the junk.
pub fn sell_selected(bot: &Arc<Bot>, keys: &HashSet<Key>) {
    mark(bot, keys, true);
    sell_all(bot);
}

/// VibeSkua `SetGoalForSelected` (`Some`) and `ClearGoalForSelected` (`None`), saved in the
/// application settings.
pub fn set_goal(bot: &Arc<Bot>, keys: &HashSet<Key>, quantity: Option<i64>) {
    let ids: Vec<i64> = keys.iter().map(|(id, _)| *id).collect();
    junk::set_goals(bot, &ids, quantity);
    let ids: HashSet<i64> = ids.into_iter().collect();
    edit(bot, |e| {
        if ids.contains(&e.id) {
            e.goal = quantity.unwrap_or(0);
        }
    });
}

/// "Skip sell warning", saved with the application settings (VibeSkua `JunkSkipSellWarning`).
pub fn set_skip_warning(bot: &Arc<Bot>, on: bool) {
    bot.settings.write().junk_skip_sell_warning = on;
    bot.save_settings();
}

/// Applies `f` to every item and hands the list over.
fn edit(bot: &Arc<Bot>, f: impl FnMut(&mut Entry)) -> Arc<Vec<Entry>> {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    let mut entries = s.entries.as_ref().clone();
    entries.iter_mut().for_each(f);
    s.entries = Arc::new(entries);
    s.revision += 1;
    s.entries.clone()
}

/// [`edit`], then saves the junk ones as the list.
fn change(bot: &Arc<Bot>, f: impl FnMut(&mut Entry)) {
    let entries = edit(bot, f);
    let path = bot.paths.junk_items_file();
    if let Err(e) = junk::set(&path, entries.iter().filter(|e| e.junk).map(Entry::to_junk)) {
        lock(&STATE.of(bot)).error = Some(format!("Couldn't save {}: {e}", path.display()));
    }
}

/// The native page's selection.
fn selected(bot: &Arc<Bot>) -> HashSet<Key> {
    lock(&STATE.of(bot)).native.selected.clone()
}

/// Junk's actions (`POST /models/junk/do`), on the native page's selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum JunkAct {
    Refresh,
    /// Changes the filter's fields given (`ac` and `highlight` by their labels), back to page 1.
    Filter {
        search: Option<String>,
        category: Option<String>,
        ac: Option<String>,
        highlight: Option<String>,
    },
    Page {
        page: u32,
    },
    /// Selects or unselects an item (by its ID and category).
    Select {
        id: i64,
        category: String,
        on: bool,
    },
    /// On, selects the shown items; off, clears the selection.
    SelectShown {
        on: bool,
    },
    Mark,
    Unmark,
    UnmarkAll,
    /// Sets the selection's goal to `input` (a whole number above 0).
    Goal {
        input: String,
    },
    ClearGoal,
    SellSelected,
    SellAll,
    SkipWarning {
        on: bool,
    },
}

pub fn act(bot: &Arc<Bot>, act: JunkAct) -> Result<(), String> {
    let state = STATE.of(bot);
    match act {
        JunkAct::Refresh => refresh(bot),
        JunkAct::Filter {
            search,
            category,
            ac,
            highlight,
        } => {
            let mut s = lock(&state);
            let f = &mut s.native.filter;
            if let Some(search) = search {
                f.search = search;
            }
            if let Some(category) = category {
                f.category = category;
            }
            if let Some(ac) = ac {
                f.ac = AcFilter::ALL
                    .into_iter()
                    .find(|a| a.label() == ac)
                    .ok_or("no such AC filter")?;
            }
            if let Some(highlight) = highlight {
                f.highlight = Highlight::ALL
                    .into_iter()
                    .find(|h| h.label() == highlight)
                    .ok_or("no such highlight")?;
            }
            s.native.page = 0;
        }
        JunkAct::Page { page } => lock(&state).native.page = page as usize,
        JunkAct::Select { id, category, on } => {
            let item = (id, category);
            let mut s = lock(&state);
            if on {
                s.native.selected.insert(item);
            } else {
                s.native.selected.remove(&item);
            }
        }
        // An unchecked box selects the shown items; a checked or partly checked one clears all.
        JunkAct::SelectShown { on } => {
            let mut s = lock(&state);
            if on {
                let shown: Vec<Key> = s
                    .entries
                    .iter()
                    .filter(|e| s.native.filter.matches(e))
                    .map(key)
                    .collect();
                s.native.selected.extend(shown);
            } else {
                s.native.selected.clear();
            }
        }
        JunkAct::Mark => mark(bot, &selected(bot), true),
        JunkAct::Unmark => {
            let keys = std::mem::take(&mut lock(&state).native.selected);
            mark(bot, &keys, false);
        }
        JunkAct::UnmarkAll => {
            lock(&state).native.selected.clear();
            unmark_all(bot);
        }
        JunkAct::Goal { input } => {
            let quantity = junk::parse_goal(&input).ok_or("A goal is a whole number above 0.")?;
            set_goal(bot, &selected(bot), Some(quantity));
        }
        JunkAct::ClearGoal => set_goal(bot, &selected(bot), None),
        JunkAct::SellSelected => sell_selected(bot, &selected(bot)),
        JunkAct::SellAll => sell_all(bot),
        JunkAct::SkipWarning { on } => set_skip_warning(bot, on),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(id: i64, name: &str, category: &str) -> Entry {
        Entry {
            id,
            name: name.into(),
            category: category.into(),
            inventory_quantity: 1,
            ..Entry::default()
        }
    }

    #[test]
    fn native_selection_filters_and_marks() {
        // Another test may have set the data folder first: a temporary one too.
        let dir = std::env::temp_dir().join(format!("turu-junk-{}", std::process::id()));
        turu_bot::paths::set_default_root(dir.join("data"));
        let bot = Bot::new();
        assert!(bot.paths.root.starts_with(std::env::temp_dir()));
        take(
            &STATE.of(&bot),
            vec![
                entry(1, "Bone Dust", "Item"),
                entry(2, "Void Aura", "Resource"),
                entry(3, "Ash", "Item"),
            ],
        );
        act(
            &bot,
            JunkAct::Filter {
                search: None,
                category: Some("Item".into()),
                ac: None,
                highlight: None,
            },
        )
        .unwrap();
        let v = view(&bot);
        assert_eq!((v.total, v.matches, v.selected, v.all_shown), (3, 2, 0, false));
        act(&bot, JunkAct::SelectShown { on: true }).unwrap();
        assert!(view(&bot).all_shown);
        act(&bot, JunkAct::Mark).unwrap();
        let v = view(&bot);
        assert_eq!((v.junk, v.selected), (2, 2));
        let saved: Vec<i64> = junk::load(&bot.paths.junk_items_file()).iter().map(|j| j.id).collect();
        assert_eq!(saved, [1, 3]);
        act(
            &bot,
            JunkAct::Select {
                id: 1,
                category: "Item".into(),
                on: false,
            },
        )
        .unwrap();
        act(&bot, JunkAct::Unmark).unwrap();
        let v = view(&bot);
        assert_eq!((v.junk, v.selected), (1, 0));
        // A refresh drops the category when it's gone.
        take(&STATE.of(&bot), vec![entry(2, "Void Aura", "Resource")]);
        assert_eq!(view(&bot).category, junk::ALL_CATEGORIES);
        assert!(act(&bot, JunkAct::Goal { input: "0".into() }).is_err());
        assert!(serde_json::from_value::<JunkAct>(json!({ "action": "nope" })).is_err());
    }
}
