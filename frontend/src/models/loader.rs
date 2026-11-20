//! Tools & Helpers > Loader (VibeSkua `LoaderView`): loads a shop or quests by ID, and lists the
//! quest cache (`QuestData.json`, `turu_bot::quest_cache`) to load, copy or fake complete quests
//! from. The cache is read, downloaded and updated on bot threads; the list keeps IDs and names.
//! With no cache file, opening the Loader downloads the community one.
//!
//! Per tab ([`State`]): the cache as last read, what runs (with its progress and Cancel), the last
//! notice, and a native page's search and page (filtered outside the lock when either or the cache
//! changes). The egui window keeps its own search and selection. Every function here locks the
//! state only briefly and never while calling the bot, so a shell may call them from a frame.

use super::tab_state::{PerTab, Shared, lock};
use serde::Serialize;
use serde_json::Value as Json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::api::{player, quests, shops};
use turu_bot::quest_cache::{self, Downloaded, HttpSource, QuestSummary, UpdateMode};

/// A native page's quests per page.
pub const PAGE_SIZE: usize = 50;

#[derive(Default)]
pub struct State {
    quests: Arc<Vec<QuestSummary>>,
    /// Counts the lists read, so a filter knows when to run again.
    revision: u64,
    read_started: bool,
    /// Reading or updating the cache, or fake completing a quest.
    busy: bool,
    /// The cache update can be cancelled.
    cancellable: bool,
    /// VibeSkua's progress text while busy.
    progress: String,
    /// A message shown for a few seconds (a bad range, a failure, a download's result), until
    /// when, and whether it is good news.
    notice: Option<(String, Instant, bool)>,
    cancel: Arc<AtomicBool>,
    page: Page,
}

/// A native page's search, page and the list as last filtered (indices into the cache).
#[derive(Default)]
struct Page {
    search: String,
    page: usize,
    made: Option<(String, u64)>,
    filtered: Vec<usize>,
}

pub static STATE: PerTab<State> = PerTab::new();

/// What runs and the last notice, as every shell shows them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LoaderStatus {
    pub busy: bool,
    /// Cancel is offered (a cache update runs).
    pub cancellable: bool,
    /// "Getting quests", "Loading quests 2001 to 2100", ...
    pub progress: String,
    pub notice: Option<LoaderNotice>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LoaderNotice {
    pub text: String,
    /// Good news ("Downloaded 9,000 quests."), else a warning.
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LoaderView {
    pub status: LoaderStatus,
    /// The quests in the cache.
    pub total: u32,
    /// The search as the slot has it, so the page knows its typing reached it.
    pub search: String,
    /// How many match, the page shown (from 0) and how many pages.
    pub matches: u32,
    pub page: u32,
    pub pages: u32,
    pub quests: Vec<LoaderQuest>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LoaderQuest {
    pub id: i64,
    pub name: String,
}

/// The cache as last read and its revision (for a shell that filters it itself).
pub fn quests(bot: &Arc<Bot>) -> (Arc<Vec<QuestSummary>>, u64) {
    let state = STATE.of(bot);
    let s = lock(&state);
    (s.quests.clone(), s.revision)
}

/// What runs and the last notice (dropped once its time is up).
pub fn status(bot: &Arc<Bot>) -> LoaderStatus {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    status_of(&mut s)
}

fn status_of(s: &mut State) -> LoaderStatus {
    if s.notice.as_ref().is_some_and(|(_, until, _)| Instant::now() >= *until) {
        s.notice = None;
    }
    LoaderStatus {
        busy: s.busy,
        cancellable: s.cancellable,
        progress: s.progress.clone(),
        notice: s.notice.as_ref().map(|(text, _, ok)| LoaderNotice {
            text: text.clone(),
            ok: *ok,
        }),
    }
}

pub fn view(bot: &Arc<Bot>) -> LoaderView {
    let state = STATE.of(bot);
    // Filter outside the lock when the search or the cache changed (the cache is large).
    let (want, made, quests, revision) = {
        let s = lock(&state);
        (s.page.search.clone(), s.page.made.clone(), s.quests.clone(), s.revision)
    };
    if made.as_ref() != Some(&(want.clone(), revision)) {
        let filtered = quest_cache::search(&quests, &want);
        let mut s = lock(&state);
        if s.page.search == want && s.revision == revision {
            s.page.filtered = filtered;
            s.page.made = Some((want, revision));
        }
    }
    let mut s = lock(&state);
    let status = status_of(&mut s);
    let p = &s.page;
    let pages = p.filtered.len().div_ceil(PAGE_SIZE).max(1);
    let page = p.page.min(pages - 1);
    let rows = p
        .filtered
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .filter_map(|&i| s.quests.get(i))
        .map(|q| LoaderQuest {
            id: q.id,
            name: q.name.clone(),
        })
        .collect();
    LoaderView {
        status,
        total: s.quests.len() as u32,
        search: p.search.clone(),
        matches: p.filtered.len() as u32,
        page: page as u32,
        pages: pages as u32,
        quests: rows,
    }
}

fn notify(state: &Shared<State>, text: impl Into<String>, seconds: u64, ok: bool) {
    lock(state).notice = Some((text.into(), Instant::now() + Duration::from_secs(seconds), ok));
}

/// Hands a list read or updated to every shell.
fn take(state: &Shared<State>, quests: Vec<QuestSummary>) {
    let mut s = lock(state);
    s.quests = Arc::new(quests);
    s.revision += 1;
}

/// Starts `work` on a bot thread as `progress`, busy until it ends.
fn run(
    bot: &Arc<Bot>,
    name: &str,
    progress: String,
    cancellable: bool,
    work: impl FnOnce(&Bot, &Shared<State>) + Send + 'static,
) {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        s.busy = true;
        s.cancellable = cancellable;
        s.progress = progress;
    }
    crate::turu::spawn_action_for(bot.clone(), name, move |bot| {
        work(bot, &state);
        let mut s = lock(&state);
        s.busy = false;
        s.cancellable = false;
        s.progress.clear();
        Ok(())
    });
}

/// Downloads the community cache into the file (`quest_cache::download`) and hands the list
/// over; reports the result as a notice.
fn download(bot: &Bot, state: &Shared<State>) {
    lock(state).progress = "Downloading quest cache...".into();
    match quest_cache::download(&bot.paths.quest_data_file(), &HttpSource::from_env()) {
        Ok(Downloaded::Updated {
            quests,
            downloaded,
            kept,
        }) => {
            take(state, quests.iter().map(QuestSummary::from).collect());
            let kept = if kept > 0 {
                format!(", kept {kept} of yours")
            } else {
                String::new()
            };
            notify(state, format!("Downloaded {downloaded} quests{kept}."), 5, true);
        }
        Ok(Downloaded::Unchanged) => notify(state, "The quest cache is up to date.", 4, true),
        Err(e) => notify(state, format!("Couldn't download the quest cache: {e}"), 8, false),
    }
}

/// When a shell opens the Loader: the cache is read the first time (VibeSkua `GetQuestsAsync`);
/// without a file, the community cache is downloaded first (VibeSkua ships one).
pub fn open(bot: &Arc<Bot>) {
    {
        let state = STATE.of(bot);
        let mut s = lock(&state);
        if s.read_started {
            return;
        }
        s.read_started = true;
    }
    run(
        bot,
        "Read the quest cache",
        "Getting quests".into(),
        false,
        |bot, state| {
            let path = bot.paths.quest_data_file();
            // Not in tests, which would download into the real data folder.
            if !path.exists() && !cfg!(test) {
                download(bot, state);
            } else {
                match quest_cache::read_summaries(&path) {
                    Ok(quests) => take(state, quests),
                    Err(e) => notify(state, format!("Failed to load quests: {e}"), 6, false),
                }
            }
        },
    );
}

/// Download: the community cache merged into the file.
pub fn download_cache(bot: &Arc<Bot>) {
    run(bot, "Download the quest cache", String::new(), false, download);
}

/// VibeSkua `UpdateQuests(getAll)` / `UpdateQuestsRange`.
pub fn update_cache(bot: &Arc<Bot>, mode: UpdateMode) {
    let cancel = Arc::new(AtomicBool::new(false));
    lock(&STATE.of(bot)).cancel = cancel.clone();
    run(bot, "Update the quest cache", String::new(), true, move |bot, state| {
        let path = bot.paths.quest_data_file();
        if !player::logged_in(bot).unwrap_or(false) {
            notify(state, "Log in to update the quest cache.", 4, false);
        }
        let progress = |text: String| lock(state).progress = text;
        match quest_cache::update(bot, &path, mode, &progress, &cancel) {
            Ok(quests) => take(state, quests.iter().map(QuestSummary::from).collect()),
            Err(e) => notify(state, format!("Failed to update the quest cache: {e}"), 6, false),
        }
    });
}

/// Update range: "2566, 2599"; a bad range is a notice.
pub fn update_range(bot: &Arc<Bot>, input: &str) {
    match quest_cache::parse_range(input) {
        Ok((start, end)) => update_cache(bot, UpdateMode::Range(start, end)),
        Err(message) => notify(&STATE.of(bot), message, 2, false),
    }
}

/// Cancel: the update stops after the request it waits for.
pub fn cancel(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    if s.cancellable {
        s.cancel.store(true, Ordering::Release);
        s.progress = "Cancelling task...".into();
    }
}

/// VibeSkua `UpdateSelectedQuest`: a client-side quest-complete packet.
pub fn fake_complete(bot: &Arc<Bot>, id: i64) {
    let name = {
        let state = STATE.of(bot);
        let s = lock(&state);
        if s.busy {
            return;
        }
        s.quests
            .iter()
            .find(|q| q.id == id)
            .map_or_else(|| id.to_string(), |q| q.name.clone())
    };
    run(
        bot,
        "Fake complete",
        format!("Fake completing quest: {name}."),
        false,
        move |bot, state| match quests::update_quest(bot, id) {
            Ok(true) => {}
            Ok(false) => notify(state, format!("Failed to update quest: {id} didn't load."), 4, false),
            Err(e) => notify(state, format!("Failed to update quest: {e}"), 4, false),
        },
    );
}

/// Load for Shop: one ID (VibeSkua loads a shop only for a single ID).
pub fn load_shop(bot: &Arc<Bot>, input: &str) -> Result<(), String> {
    let id = quest_cache::parse_shop_id(input).ok_or("A shop loads by one ID")?;
    crate::turu::spawn_action_for(bot.clone(), "Load shop", move |bot| shops::load(bot, id).map(|_| ()));
    Ok(())
}

/// Load for Quests, and Load selected.
pub fn load_quests(bot: &Arc<Bot>, ids: Vec<i64>) {
    crate::turu::spawn_action_for(bot.clone(), "Load quests", move |bot| quests::load(bot, &ids));
}

/// The control API's actions (`POST /models/loader/do`, `action` in the body).
pub fn act(bot: &Arc<Bot>, action: &str, body: &Json) -> Result<(), String> {
    let text = |key: &str| {
        body.get(key)
            .and_then(Json::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{key} is missing"))
    };
    match action {
        "open" => open(bot),
        "search" => {
            let state = STATE.of(bot);
            let mut s = lock(&state);
            if let Some(search) = body.get("search").and_then(Json::as_str)
                && search != s.page.search
            {
                s.page.search = search.to_owned();
                s.page.page = 0;
            }
            if let Some(page) = body.get("page").and_then(Json::as_u64) {
                s.page.page = page as usize;
            }
        }
        "load-shop" => load_shop(bot, &text("input")?)?,
        "load-quests" => {
            let ids = match body.get("ids").and_then(Json::as_array) {
                Some(ids) => ids.iter().filter_map(Json::as_i64).collect(),
                None => quest_cache::parse_ids(&text("input")?).ok_or("Quest IDs are digits, commas and spaces")?,
            };
            load_quests(bot, ids);
        }
        "download" => download_cache(bot),
        "add-missing" => update_cache(bot, UpdateMode::AddMissing),
        "rebuild-all" => update_cache(bot, UpdateMode::RebuildAll),
        "update-range" => update_range(bot, &text("input")?),
        "cancel" => cancel(bot),
        "fake-complete" => fake_complete(bot, body.get("id").and_then(Json::as_i64).ok_or("id is missing")?),
        other => return Err(format!("no action {other}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quest(id: i64, name: &str) -> QuestSummary {
        QuestSummary {
            id,
            name: name.to_owned(),
        }
    }

    #[test]
    fn the_page_follows_the_search_and_the_cache() {
        let bot = Bot::new();
        let state = STATE.of(&bot);
        take(&state, (1..=120).map(|i| quest(i, &format!("Quest {i}"))).collect());
        let v = view(&bot);
        assert_eq!((v.total, v.matches, v.pages, v.quests.len()), (120, 120, 3, PAGE_SIZE));
        act(&bot, "search", &serde_json::json!({ "page": 2 })).unwrap();
        assert_eq!(view(&bot).quests.len(), 20);
        // A new search starts on its first page.
        act(&bot, "search", &serde_json::json!({ "search": "quest 11" })).unwrap();
        let v = view(&bot);
        assert_eq!((v.page, v.matches), (0, 11)); // 11, 110 to 119
        take(&state, vec![quest(11, "Quest 11")]);
        assert_eq!(view(&bot).matches, 1);
    }

    #[test]
    fn a_bad_range_is_a_notice_and_a_shop_takes_one_id() {
        let bot = Bot::new();
        update_range(&bot, "12");
        assert!(status(&bot).notice.is_some_and(|n| !n.ok));
        assert!(load_shop(&bot, "1, 2").is_err());
        assert!(act(&bot, "load-quests", &serde_json::json!({ "input": "12a" })).is_err());
    }
}
