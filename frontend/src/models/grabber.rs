//! Tools & Helpers > Grabber (VibeSkua `GrabberView` / `GrabberListView`): lists of shops,
//! quests, items and monsters; each grabs its game data on a bot thread into a searchable list
//! with each row's details, Copy / Export, and its own actions (Buy, Accept, Equip, Kill, ...).
//! The readers and actions are `turu_bot::grabber`.
//!
//! Per tab ([`State`]): each list's rows as last grabbed (with a revision), what runs, its status
//! text, the quantity an action asks for and the message box it shows (both shells answer the same
//! question), and a native page's list, search, page and the row whose details it shows. Shells
//! keep their own selection, as rows' positions in the list. Every function here locks the state
//! only briefly and never while calling the bot, so a shell may call them from a frame.

use super::Control;
use super::tab_state::{PerTab, Shared, lock};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::grabber::{self, GrabAction, GrabKind, Grabbed, Prepared};

/// A native page's rows per page.
pub const PAGE_SIZE: usize = 100;

/// One list's state (VibeSkua `GrabberListViewModel`).
#[derive(Default)]
pub struct List {
    items: Arc<Vec<Grabbed>>,
    revision: u64,
    /// Grabbing or running an action.
    busy: bool,
    /// The status text, and when it goes away (never while busy).
    message: Option<(String, Option<Instant>)>,
    /// A message box an action asked for (title, text).
    notice: Option<(String, String)>,
    /// The quantity an action asks for first.
    ask: Option<Ask>,
    cancel: Arc<AtomicBool>,
}

struct Ask {
    action: GrabAction,
    selected: Vec<Grabbed>,
    title: String,
    prompt: String,
}

pub struct State {
    lists: HashMap<GrabKind, List>,
    native: Native,
}

impl Default for State {
    fn default() -> Self {
        Self {
            lists: GrabKind::ALL.into_iter().map(|k| (k, List::default())).collect(),
            native: Native::default(),
        }
    }
}

/// A native page's list, search and page, the rows as last filtered, and the row whose details
/// it shows.
struct Native {
    kind: GrabKind,
    search: String,
    page: usize,
    made: Option<(GrabKind, String, u64)>,
    filtered: Vec<usize>,
    detail: Option<usize>,
}

impl Default for Native {
    fn default() -> Self {
        Self {
            kind: GrabKind::ShopItems,
            search: String::new(),
            page: 0,
            made: None,
            filtered: Vec::new(),
            detail: None,
        }
    }
}

pub static STATE: PerTab<State> = PerTab::new();

impl State {
    fn list(&mut self, kind: GrabKind) -> &mut List {
        self.lists.entry(kind).or_default()
    }
}

/// A list's key, as the views and actions name it: its title.
pub fn kind_of(key: &str) -> Option<GrabKind> {
    GrabKind::ALL.into_iter().find(|k| k.title() == key)
}

fn action_of(kind: GrabKind, key: &str) -> Option<GrabAction> {
    kind.actions().iter().copied().find(|a| a.label() == key)
}

/// VibeSkua `ApplyFilter`: the rows whose summary contains the search, ignoring case.
pub fn filter(items: &[Grabbed], search: &str) -> Vec<usize> {
    let query = search.trim().to_lowercase();
    (0..items.len())
        .filter(|&i| query.is_empty() || items[i].summary().to_lowercase().contains(&query))
        .collect()
}

/// Whether fights (Kill) must wait: a script, a Console snippet or Auto Attack/Hunt runs.
pub fn fights_blocked(bot: &Bot) -> bool {
    bot.scripts.status().is_active() || bot.scripts.is_snippet_running() || bot.auto.is_running()
}

// --- what a shell reads ----------------------------------------------------------------------

/// One list as a shell shows it: its rows and what runs.
pub struct Snapshot {
    pub items: Arc<Vec<Grabbed>>,
    pub revision: u64,
    pub busy: bool,
    pub message: Option<String>,
    pub notice: Option<(String, String)>,
    /// The quantity question (title, prompt).
    pub ask: Option<(String, String)>,
}

pub fn snapshot(bot: &Arc<Bot>, kind: GrabKind) -> Snapshot {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    let list = s.list(kind);
    expire(list);
    Snapshot {
        items: list.items.clone(),
        revision: list.revision,
        busy: list.busy,
        message: list.message.as_ref().map(|(text, _)| text.clone()),
        notice: list.notice.clone(),
        ask: list.ask.as_ref().map(|a| (a.title.clone(), a.prompt.clone())),
    }
}

fn expire(list: &mut List) {
    if list
        .message
        .as_ref()
        .is_some_and(|(_, until)| until.is_some_and(|u| Instant::now() >= u))
    {
        list.message = None;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GrabberView {
    pub lists: Vec<GrabListView>,
    /// The list on show (its title).
    pub kind: String,
    /// Several rows may be checked (not Shop IDs, Temp Inventory).
    pub multiple: bool,
    pub actions: Vec<GrabActionView>,
    pub busy: bool,
    /// "Working...", "Buying 3 items...", "Copied 4 item(s) to clipboard." ...
    pub message: Option<String>,
    pub notice: Option<GrabNoticeView>,
    /// An action asks for a quantity first.
    pub ask: Option<GrabNoticeView>,
    /// The rows' revision: a shell's checked rows belong to one revision.
    pub revision: u64,
    pub grabbed: u32,
    /// The search as the slot has it, so the page knows its typing reached it.
    pub search: String,
    pub matches: u32,
    pub page: u32,
    pub pages: u32,
    pub rows: Vec<GrabRowView>,
    /// The fields of the row whose details are asked for.
    pub details: Option<GrabDetailsView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GrabListView {
    pub title: String,
    /// "SHOPS", "QUESTS", "ITEMS", "MONSTERS".
    pub group: String,
    /// Rows grabbed into it.
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GrabActionView {
    pub label: String,
    /// Sell and Sell All, shown in red.
    pub destructive: bool,
    pub control: Control,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GrabNoticeView {
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GrabRowView {
    /// Its place in the list (the actions' `positions`).
    pub position: u32,
    pub id: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GrabDetailsView {
    pub position: u32,
    pub title: String,
    pub fields: Vec<GrabFieldView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GrabFieldView {
    pub key: String,
    pub value: String,
}

pub fn view(bot: &Arc<Bot>) -> GrabberView {
    let blocked = fights_blocked(bot);
    let state = STATE.of(bot);
    // Filter outside the lock when the list, the search or the rows changed.
    let (want, made, items) = {
        let mut s = lock(&state);
        let (kind, search) = (s.native.kind, s.native.search.clone());
        let list = s.list(kind);
        (
            (kind, search, list.revision),
            s.native.made.clone(),
            s.lists[&kind].items.clone(),
        )
    };
    if made.as_ref() != Some(&want) {
        let filtered = filter(&items, &want.1);
        let mut s = lock(&state);
        if (s.native.kind, s.native.search.as_str()) == (want.0, want.1.as_str()) && s.lists[&want.0].revision == want.2
        {
            s.native.filtered = filtered;
            s.native.made = Some(want);
        }
    }
    let mut s = lock(&state);
    let kind = s.native.kind;
    let lists = GrabKind::ALL
        .into_iter()
        .map(|k| GrabListView {
            title: k.title().to_owned(),
            group: k.group().to_owned(),
            count: s.lists.get(&k).map_or(0, |l| l.items.len() as u32),
        })
        .collect();
    let detail = s.native.detail;
    let list = s.list(kind);
    expire(list);
    let busy = list.busy;
    let actions = kind
        .actions()
        .iter()
        .map(|&a| GrabActionView {
            label: a.label().to_owned(),
            destructive: a.destructive(),
            control: if busy {
                Control::off("Busy")
            } else if a.fights() && blocked {
                Control::off("Stop the script, Console snippet or Auto Attack/Hunt first")
            } else {
                Control::on()
            },
        })
        .collect();
    let message = list.message.as_ref().map(|(text, _)| text.clone());
    let notice = list.notice.as_ref().map(|(title, text)| GrabNoticeView {
        title: title.clone(),
        text: text.clone(),
    });
    let ask = list.ask.as_ref().map(|a| GrabNoticeView {
        title: a.title.clone(),
        text: a.prompt.clone(),
    });
    let (items, revision) = (list.items.clone(), list.revision);
    let details = detail
        .and_then(|i| items.get(i).map(|item| (i, item)))
        .map(|(i, item)| GrabDetailsView {
            position: i as u32,
            title: item.row(),
            fields: item
                .details()
                .into_iter()
                .map(|(key, value)| GrabFieldView { key, value })
                .collect(),
        });
    let n = &s.native;
    let pages = n.filtered.len().div_ceil(PAGE_SIZE).max(1);
    let page = n.page.min(pages - 1);
    let rows = n
        .filtered
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .filter_map(|&i| items.get(i).map(|item| (i, item)))
        .map(|(i, item)| GrabRowView {
            position: i as u32,
            id: item.id(),
            text: item.row(),
        })
        .collect();
    GrabberView {
        lists,
        kind: kind.title().to_owned(),
        multiple: kind.select_multiple(),
        actions,
        busy,
        message,
        notice,
        ask,
        revision,
        grabbed: items.len() as u32,
        search: n.search.clone(),
        matches: n.filtered.len() as u32,
        page: page as u32,
        pages: pages as u32,
        rows,
        details,
    }
}

// --- what a shell does -----------------------------------------------------------------------

/// Shows `text` as `kind`'s status for `seconds` (0: until replaced).
pub fn report(bot: &Arc<Bot>, kind: GrabKind, text: impl Into<String>, seconds: u64) {
    report_in(&STATE.of(bot), kind, text, seconds);
}

fn report_in(state: &Shared<State>, kind: GrabKind, text: impl Into<String>, seconds: u64) {
    let until = (seconds > 0).then(|| Instant::now() + Duration::from_secs(seconds));
    lock(state).list(kind).message = Some((text.into(), until));
}

/// VibeSkua `Grab`.
pub fn grab(bot: &Arc<Bot>, kind: GrabKind) {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        let list = s.list(kind);
        if list.busy {
            return;
        }
        list.busy = true;
        list.message = Some(("Working...".into(), None));
    }
    crate::turu::spawn_action_for(bot.clone(), "Grab", move |bot| {
        match grabber::grab(bot, kind, &crate::map_items::scan) {
            Ok(items) => {
                {
                    let mut s = lock(&state);
                    let list = s.list(kind);
                    list.items = Arc::new(items);
                    list.revision += 1;
                    if s.native.kind == kind {
                        s.native.detail = None;
                    }
                }
                report_in(&state, kind, "Finished.", 1);
            }
            Err(e) => report_in(&state, kind, format!("Grab failed: {e}"), 4),
        }
        lock(&state).list(kind).busy = false;
        Ok(())
    });
}

/// The rows at `positions` of `kind`'s rows of `revision` (a shell's checked rows).
fn rows(state: &Shared<State>, kind: GrabKind, positions: &[usize], revision: u64) -> Result<Vec<Grabbed>, String> {
    let mut s = lock(state);
    let list = s.list(kind);
    if list.revision != revision {
        return Err("The list was grabbed again: check the rows again".into());
    }
    Ok(positions.iter().filter_map(|&i| list.items.get(i).cloned()).collect())
}

/// An action's button: VibeSkua's checks first (`grabber::prepare`), then it runs, asks for a
/// quantity, shows a message box or a status.
pub fn start(bot: &Arc<Bot>, kind: GrabKind, action: GrabAction, selected: Vec<Grabbed>) {
    let refs: Vec<&Grabbed> = selected.iter().collect();
    match grabber::prepare(action, &refs) {
        Prepared::Run => run(bot, kind, action, selected, None),
        Prepared::AskQuantity { title, prompt } => {
            report(bot, kind, format!("{title}, input quantity..."), 0);
            lock(&STATE.of(bot)).list(kind).ask = Some(Ask {
                action,
                selected,
                title,
                prompt,
            });
        }
        Prepared::Warn {
            title,
            message,
            progress,
        } => {
            if !progress.is_empty() {
                report(bot, kind, progress, 2);
            }
            lock(&STATE.of(bot)).list(kind).notice = Some((title, message));
        }
        Prepared::Report(text) => report(bot, kind, text, 2),
    }
}

/// The quantity question's answer: `None` is Cancel. VibeSkua does nothing for a quantity that
/// isn't a number.
pub fn answer(bot: &Arc<Bot>, kind: GrabKind, text: Option<&str>) {
    let Some(ask) = lock(&STATE.of(bot)).list(kind).ask.take() else {
        return;
    };
    match text.map(|t| t.trim().parse::<i64>().ok()) {
        Some(Some(quantity)) => run(bot, kind, ask.action, ask.selected, Some(quantity)),
        Some(None) => lock(&STATE.of(bot)).list(kind).message = None,
        None => report(bot, kind, "Cancelled.", 1),
    }
}

/// Closes the message box.
pub fn dismiss(bot: &Arc<Bot>, kind: GrabKind) {
    lock(&STATE.of(bot)).list(kind).notice = None;
}

/// Cancel: the task stops between items and ends a kill.
pub fn cancel(bot: &Arc<Bot>, kind: GrabKind) {
    lock(&STATE.of(bot)).list(kind).cancel.store(true, Ordering::Release);
}

/// VibeSkua `GrabberTask`: runs the action on the selected rows.
fn run(bot: &Arc<Bot>, kind: GrabKind, action: GrabAction, selected: Vec<Grabbed>, quantity: Option<i64>) {
    let state = STATE.of(bot);
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut s = lock(&state);
        let list = s.list(kind);
        list.cancel = cancel.clone();
        list.busy = true;
    }
    crate::turu::spawn_action_for(bot.clone(), action.label(), move |bot| {
        let refs: Vec<&Grabbed> = selected.iter().collect();
        let progress = |text: String| report_in(&state, kind, text, 0);
        match grabber::run(bot, action, &refs, quantity, &progress, &cancel) {
            Ok(notice) => lock(&state).list(kind).notice = notice,
            Err(_) if cancel.load(Ordering::Acquire) => progress("Task cancelled.".into()),
            Err(e) => progress(format!("Operation failed: {e}")),
        }
        let mut s = lock(&state);
        let list = s.list(kind);
        list.busy = false;
        // VibeSkua clears the status a second after the task.
        if let Some((_, until)) = list.message.as_mut() {
            *until = Some(Instant::now() + Duration::from_secs(1));
        }
        Ok(())
    });
}

/// Copy's text for `targets` (the selected rows, or every shown row), with its status.
pub fn copy_text(bot: &Arc<Bot>, kind: GrabKind, targets: &[&Grabbed]) -> Option<String> {
    if targets.is_empty() {
        report(bot, kind, "No items to copy.", 3);
        return None;
    }
    report(bot, kind, format!("Copied {} item(s) to clipboard.", targets.len()), 3);
    Some(grabber::export_text(kind.title(), targets))
}

/// The control API's actions (`POST /models/grabber/do`, `action` in the body). Copy answers
/// `{ "text": ... }`.
pub fn act(bot: &Arc<Bot>, action: &str, body: &Json) -> Result<Json, String> {
    let state = STATE.of(bot);
    let kind = || match body.get("kind").and_then(Json::as_str) {
        Some(key) => kind_of(key).ok_or_else(|| format!("no list {key}")),
        None => Ok(lock(&state).native.kind),
    };
    let positions = || -> Vec<usize> {
        body.get("positions")
            .and_then(Json::as_array)
            .map(|a| a.iter().filter_map(Json::as_u64).map(|p| p as usize).collect())
            .unwrap_or_default()
    };
    let revision = || body.get("revision").and_then(Json::as_u64).ok_or("revision is missing");
    match action {
        "select" => {
            let kind = kind()?;
            let mut s = lock(&state);
            if s.native.kind != kind {
                s.native.kind = kind;
                s.native.search.clear();
                s.native.page = 0;
                s.native.detail = None;
            }
        }
        "search" => {
            let mut s = lock(&state);
            if let Some(search) = body.get("search").and_then(Json::as_str)
                && search != s.native.search
            {
                s.native.search = search.to_owned();
                s.native.page = 0;
            }
            if let Some(page) = body.get("page").and_then(Json::as_u64) {
                s.native.page = page as usize;
            }
        }
        "detail" => lock(&state).native.detail = body.get("position").and_then(Json::as_u64).map(|p| p as usize),
        "grab" => grab(bot, kind()?),
        "run" => {
            let kind = kind()?;
            let key = body.get("label").and_then(Json::as_str).ok_or("label is missing")?;
            let action = action_of(kind, key).ok_or_else(|| format!("{} has no {key}", kind.title()))?;
            if action.fights() && fights_blocked(bot) {
                return Err("Stop the script, Console snippet or Auto Attack/Hunt first".into());
            }
            let selected = rows(&state, kind, &positions(), revision()?)?;
            start(bot, kind, action, selected);
        }
        "quantity" => answer(
            bot,
            kind()?,
            Some(body.get("value").and_then(Json::as_str).unwrap_or("")),
        ),
        "cancel-ask" => answer(bot, kind()?, None),
        "dismiss" => dismiss(bot, kind()?),
        "cancel" => cancel(bot, kind()?),
        "copy" => {
            let kind = kind()?;
            let picked = rows(&state, kind, &positions(), revision()?)?;
            // None checked: every row the search shows.
            let all: Vec<Grabbed> = if picked.is_empty() {
                let mut s = lock(&state);
                let filtered = s.native.filtered.clone();
                let list = s.list(kind);
                filtered.iter().filter_map(|&i| list.items.get(i).cloned()).collect()
            } else {
                picked
            };
            let refs: Vec<&Grabbed> = all.iter().collect();
            return Ok(json!({ "text": copy_text(bot, kind, &refs) }));
        }
        other => return Err(format!("no action {other}")),
    }
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_list_and_action_has_a_key() {
        for kind in GrabKind::ALL {
            assert_eq!(kind_of(kind.title()), Some(kind));
            for &action in kind.actions() {
                assert_eq!(action_of(kind, action.label()), Some(action));
            }
        }
    }

    #[test]
    fn the_page_follows_the_list_and_its_search() {
        let bot = Bot::new();
        let v = view(&bot);
        assert_eq!(
            (v.kind.as_str(), v.grabbed, v.lists.len()),
            ("Shop Items", 0, GrabKind::ALL.len())
        );
        assert_eq!(v.actions.len(), 1);
        act(&bot, "select", &json!({ "kind": "Inventory" })).unwrap();
        act(&bot, "search", &json!({ "search": "x" })).unwrap();
        let v = view(&bot);
        assert_eq!((v.kind.as_str(), v.search.as_str()), ("Inventory", "x"));
        // Another list starts unsearched.
        act(&bot, "select", &json!({ "kind": "Bank Items" })).unwrap();
        assert_eq!(view(&bot).search, "");
        // Rows checked in another revision are refused.
        let err = act(
            &bot,
            "run",
            &json!({ "label": "To Inventory", "positions": [0], "revision": 9 }),
        );
        assert!(err.is_err());
        // Nothing checked: VibeSkua's "No items found/selected."
        act(
            &bot,
            "run",
            &json!({ "label": "To Inventory", "positions": [], "revision": 0 }),
        )
        .unwrap();
        assert_eq!(view(&bot).message.as_deref(), Some("No items found/selected."));
    }
}
