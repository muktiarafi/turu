//! Search Scripts (VibeSkua's Script Repo): the scripts the repository and the scripts folder have,
//! searched (VibeSkua's `SearchRank`) and filtered (folder, Pinned, Local), pinned first then
//! sorted; what each row may do now (Start, Load, Queue) and why not; running a row's work; the
//! repository's task, its message and Update all, Download all, Refresh. The search is a tab's own
//! ([`Query`]); a shell's view carries the first [`ROWS`] results, not the whole list (a phone's
//! repository has nearly 2,000 scripts).

use super::Control;
use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::repo::{Entry, Scope, Snapshot, Status};

/// How many results a shell's view carries.
pub const ROWS: usize = 80;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Sort {
    #[default]
    Name,
    /// Date added (VibeSkua `CompareScripts`).
    Added,
}

/// What the list is searched and filtered for.
#[derive(Clone, PartialEq, Debug)]
pub struct Query {
    pub search: String,
    pub scope: Scope,
    /// "All" or a folder, capitalised ("Farm").
    pub category: String,
    pub pinned_only: bool,
    pub local_only: bool,
    pub sort: Sort,
    pub descending: bool,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            search: String::new(),
            scope: Scope::All,
            category: "All".into(),
            pinned_only: false,
            local_only: false,
            sort: Sort::Name,
            descending: false,
        }
    }
}

impl Query {
    /// How many filters are on (the search aside).
    pub fn active_filters(&self) -> usize {
        [
            self.scope != Scope::All,
            self.category != "All",
            self.pinned_only,
            self.local_only,
        ]
        .into_iter()
        .filter(|&on| on)
        .count()
    }

    pub fn clear_filters(&mut self) {
        let Query {
            search,
            sort,
            descending,
            ..
        } = std::mem::take(self);
        *self = Query {
            search,
            sort,
            descending,
            ..Query::default()
        };
    }
}

/// The search's outcome: the entries shown, in order, and what the filters can choose from.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Filtered {
    /// Indexes into the snapshot's entries.
    pub rows: Vec<usize>,
    /// "All", then the folders, capitalised.
    pub categories: Vec<String>,
    /// The scripts there are to search (libraries left out).
    pub total: usize,
}

/// A folder's name as the filter shows it.
pub fn capitalize(folder: &str) -> String {
    let mut chars = folder.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

/// Searches and filters `entries` (VibeSkua: pinned first, then the sort; with a search, its rank
/// orders what that gives). Libraries (CoreBots, ...) have nothing to run: they're left out.
pub fn filter(entries: &[Entry], query: &Query) -> Filtered {
    let mut folders: Vec<String> = entries
        .iter()
        .filter(|e| !e.info.library)
        .filter_map(|e| e.info.category())
        .map(capitalize)
        .collect();
    folders.sort();
    folders.dedup();
    let mut categories = vec!["All".to_owned()];
    categories.extend(folders);
    let total = entries.iter().filter(|e| !e.info.library).count();

    let mut rows: Vec<(usize, u8)> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.info.library)
        .filter(|(_, e)| query.category == "All" || turu_bot::repo::in_category(&e.info, &query.category))
        .filter(|(_, e)| !query.pinned_only || e.pinned)
        .filter(|(_, e)| !query.local_only || e.status == Status::Local)
        .filter_map(|(i, e)| turu_bot::repo::search_rank(&e.info, &query.search, query.scope).map(|rank| (i, rank)))
        .collect();
    rows.sort_by(|&(a, _), &(b, _)| {
        let (a, b) = (&entries[a], &entries[b]);
        b.pinned.cmp(&a.pinned).then_with(|| {
            let by_name = || a.info.name.to_lowercase().cmp(&b.info.name.to_lowercase());
            match query.sort {
                Sort::Name if query.descending => by_name().reverse(),
                Sort::Name => by_name(),
                // VibeSkua `CompareScripts`: scripts without a date go last either way, and
                // scripts of the same date are in name order.
                Sort::Added => {
                    let (a_dated, b_dated) = (!a.info.added.is_empty(), !b.info.added.is_empty());
                    b_dated.cmp(&a_dated).then_with(|| {
                        let dates = a.info.added.cmp(&b.info.added);
                        if query.descending { dates.reverse() } else { dates }
                    })
                }
                .then_with(by_name),
            }
        })
    });
    if !query.search.trim().is_empty() {
        rows.sort_by_key(|&(_, rank)| rank);
    }
    Filtered {
        rows: rows.into_iter().map(|(i, _)| i).collect(),
        categories,
        total,
    }
}

/// What the rows may do now, before asking whether a script is ready ([`Rules::for_row`]).
pub struct Rules {
    /// A repository task runs: only ready scripts can be loaded or started.
    pub busy: bool,
    pub start: bool,
    pub load: bool,
    pub queue: bool,
    /// Why Start or Load can't (another script runs, ...).
    pub reason: String,
    /// Why a script that needs a download waits ([`wait_tip`]).
    pub wait: String,
    pub queue_tip: String,
}

/// What one row's buttons may do, and why not.
pub struct Allowed {
    /// Pick (another window choosing a script): a script that needs a download waits while a
    /// task runs.
    pub pick: bool,
    pub start: bool,
    pub load: bool,
    pub queue: bool,
    pub run_tip: String,
    pub queue_tip: String,
}

impl Rules {
    /// While a task runs, a script that's downloaded and current still loads and starts
    /// (`ScriptRepo::ready`); one that needs a download waits.
    pub fn for_row(&self, path: &str) -> Allowed {
        let can_run = !self.busy || crate::turu::script_repo().ready(path).is_some();
        let run_tip = if !can_run {
            format!("{} (this script needs a download first)", self.wait)
        } else {
            self.reason.clone()
        };
        Allowed {
            pick: can_run,
            start: self.start && can_run,
            load: self.load && can_run,
            queue: self.queue,
            run_tip,
            queue_tip: self.queue_tip.clone(),
        }
    }
}

/// The rows' rules for `bot` now.
pub fn rules(bot: &Bot, snapshot: &Snapshot) -> Rules {
    let busy = snapshot.busy.is_some();
    let active = bot.scripts.status().is_active();
    let auto = bot.auto.is_running();
    let snippet = bot.scripts.is_snippet_running();
    let queue_running = bot.scheduler.is_running();
    let wait = wait_tip(snapshot);
    let reason = if active {
        "Stop the script first"
    } else if snippet {
        "Stop the Console snippet first"
    } else {
        "Stop Auto Attack/Hunt first"
    };
    Rules {
        busy,
        start: !active && !auto && !snippet,
        load: !active,
        queue: !busy && !queue_running,
        reason: reason.into(),
        queue_tip: if busy {
            wait.clone()
        } else {
            "Cannot add scripts while the playlist is running".into()
        },
        wait,
    }
}

/// What Update all would update. Libraries count although the list hides them: an update that
/// only changes CoreBots must still enable the button.
pub fn outdated_count(entries: &[Entry]) -> usize {
    entries.iter().filter(|e| e.status == Status::Outdated).count()
}

/// Why the buttons wait: one task runs at a time, and it can be cancelled.
pub fn wait_tip(snapshot: &Snapshot) -> String {
    match &snapshot.busy {
        Some(progress) => format!("Wait for {} to finish, or Cancel it", progress.task),
        None => String::new(),
    }
}

/// The scope's words: "All fields" rather than a second "All".
pub fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::All => "All fields",
        other => other.label(),
    }
}

/// How a badge reads: its colour's meaning, for each shell to map to its palette.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum Tone {
    Success,
    Warning,
    Muted,
}

/// A status's badge (none for a script not downloaded): its words, what they mean, its tone.
pub fn badge(status: Status) -> Option<(&'static str, &'static str, Tone)> {
    Some(match status {
        Status::Outdated => ("Update", "The repository has a newer version", Tone::Warning),
        Status::Modified => ("Modified", "Edited here: updates keep it", Tone::Warning),
        Status::Local => ("Local", "Your own script, not in the repository", Tone::Muted),
        Status::Removed => ("Removed", "No longer in the repository", Tone::Muted),
        Status::UpToDate => ("Downloaded", "Up to date", Tone::Success),
        Status::NotDownloaded => return None,
    })
}

/// What Delete's confirmation says for a script of `status`.
pub fn delete_text(status: Status) -> &'static str {
    match status {
        Status::Local => {
            "This is your own script: the repository has no copy of it. The file moves to \
             scripts-backup in the data folder, where you can get it back."
        }
        Status::Removed => {
            "The repository no longer has this script, so this is the only copy. The file moves to \
             scripts-backup in the data folder, where you can get it back."
        }
        Status::Modified => {
            "Your edits go with it. The file moves to scripts-backup in the data folder, where you \
             can get it back; the repository's version can be downloaded again."
        }
        _ => {
            "The file moves to scripts-backup in the data folder; you can download it again from \
             the repository."
        }
    }
}

/// A row's work.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Work {
    Start,
    Load,
    Queue,
    Download,
    /// Download over the user's edits.
    Replace,
}

/// How a row's work went at once.
pub enum Ran {
    /// It runs as the repository's task.
    Spawned,
    /// Done at once (a ready script); whether it worked.
    Done(bool),
    /// Another task runs.
    Refused,
}

/// Runs `work` for the script at `path` in `bot`'s tab: a Load or Start of a ready script at once
/// (it needs no download, so it doesn't wait for a task), the rest on the repository's thread
/// (one task at a time).
pub fn run(bot: &Arc<Bot>, path: &str, work: Work) -> Ran {
    let repo = crate::turu::script_repo();
    if matches!(work, Work::Start | Work::Load)
        && let Some(local) = repo.ready(path)
    {
        bot.scripts.load(&local);
        bot.log(format!("Loaded script {}", local.display()));
        let result = if work == Work::Start {
            turu_bot::script::start(bot)
                .map(|()| format!("Started {path}."))
                .map_err(|e| format!("Start failed: couldn't start the script: {e}"))
        } else {
            Ok(format!("Loaded {path}."))
        };
        let worked = result.is_ok();
        let (message, failed) = match result {
            Ok(message) => (message, false),
            Err(message) => (message, true),
        };
        repo.set_message(message, failed);
        return Ran::Done(worked);
    }
    let path = path.to_owned();
    let bot = bot.clone();
    let spawned = match work {
        // Downloads the script (and what it imports), then loads it in the tab and, for Start,
        // starts it.
        Work::Start | Work::Load => {
            let start = work == Work::Start;
            repo.spawn(if start { "Start" } else { "Load" }, move |repo| {
                let local = repo.ensure(&path)?;
                bot.scripts.load(&local);
                bot.log(format!("Loaded script {}", local.display()));
                if start {
                    turu_bot::script::start(&bot).map_err(|e| format!("couldn't start the script: {e}"))?;
                    return Ok(format!("Started {path}."));
                }
                Ok(format!("Loaded {path}."))
            })
        }
        // Scheduler's Queue (VibeSkua `QueueScript`): downloads the script if needed, then adds
        // it to the tab's playlist.
        Work::Queue => repo.spawn("Queue", move |repo| {
            let local = repo.ensure(&path)?;
            bot.scheduler.add(&local)?;
            Ok(format!("Queued {path}."))
        }),
        Work::Download | Work::Replace => repo.spawn("Download", move |repo| {
            let count = repo.download(&[path], work == Work::Replace)?;
            Ok(format!("Downloaded {count} files."))
        }),
    };
    if spawned { Ran::Spawned } else { Ran::Refused }
}

// A shell's view.

#[derive(Default)]
struct State {
    query: Query,
    /// The script the last row work ran for, while the repository's task runs.
    working: Option<String>,
}

static STATE: PerTab<State> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RepoView {
    /// Scripts there are to search.
    pub total: usize,
    /// Scripts the search and filters find.
    pub found: usize,
    pub query: QueryView,
    pub categories: Vec<String>,
    /// The search scopes: (key, words).
    pub scopes: Vec<(String, String)>,
    /// The first [`ROWS`] found.
    pub rows: Vec<RowView>,
    /// The repository's task while one runs.
    pub busy: Option<BusyView>,
    /// Its last outcome ("Started farm/Gold.ts.", or what failed).
    pub message: Option<MessageView>,
    /// Scripts Update all would update.
    pub outdated: usize,
    pub update_all: Control,
    pub download_all: Control,
    pub refresh: Control,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct QueryView {
    pub search: String,
    pub scope: String,
    pub category: String,
    pub pinned_only: bool,
    pub local_only: bool,
    /// "name" or "added".
    pub sort: String,
    pub descending: bool,
    pub active_filters: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RowView {
    pub path: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub badge: Option<BadgeView>,
    /// The repository's task runs for it.
    pub working: bool,
    pub start: Control,
    pub load: Control,
    pub queue: Control,
    /// Download (or with edits, Replace with the repository's): what the row's menu offers.
    pub download: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct BadgeView {
    pub label: String,
    pub meaning: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct BusyView {
    pub task: String,
    pub message: String,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct MessageView {
    pub text: String,
    pub failed: bool,
}

fn scope_key(scope: Scope) -> &'static str {
    match scope {
        Scope::All => "all",
        Scope::Name => "name",
        Scope::Tag => "tag",
        Scope::Description => "description",
        Scope::File => "file",
    }
}

/// Search Scripts' view of `bot`'s tab now: its query, and the first [`ROWS`] it finds.
pub fn view(bot: &Arc<Bot>) -> RepoView {
    let state = STATE.of(bot);
    let mut state = lock(&state);
    let snapshot = crate::turu::script_repo().snapshot();
    if snapshot.busy.is_none() {
        state.working = None;
    }
    let query = state.query.clone();
    let found = filter(&snapshot.entries, &query);
    let rules = rules(bot, &snapshot);
    let control = |enabled: bool, reason: &str| Control::when(enabled, reason);
    let rows = found
        .rows
        .iter()
        .take(ROWS)
        .map(|&i| {
            let entry = &snapshot.entries[i];
            let path = entry.info.path.clone();
            let allowed = rules.for_row(&path);
            RowView {
                name: entry.info.name.clone(),
                description: entry.info.description.clone(),
                tags: entry.info.tags.clone(),
                pinned: entry.pinned,
                badge: badge(entry.status).map(|(label, meaning, tone)| BadgeView {
                    label: label.into(),
                    meaning: meaning.into(),
                    tone,
                }),
                working: state.working.as_deref() == Some(path.as_str()),
                start: control(allowed.start, &allowed.run_tip),
                load: control(allowed.load, &allowed.run_tip),
                queue: control(allowed.queue, &allowed.queue_tip),
                download: match entry.status {
                    Status::NotDownloaded => Some("Download".into()),
                    Status::Outdated => Some("Update".into()),
                    Status::Modified => Some("Replace with the repository's".into()),
                    _ => None,
                },
                path,
            }
        })
        .collect();
    let busy = snapshot.busy.is_some();
    let wait = wait_tip(&snapshot);
    let outdated = outdated_count(&snapshot.entries);
    RepoView {
        total: found.total,
        found: found.rows.len(),
        query: QueryView {
            search: query.search.clone(),
            scope: scope_key(query.scope).into(),
            category: query.category.clone(),
            pinned_only: query.pinned_only,
            local_only: query.local_only,
            sort: match query.sort {
                Sort::Name => "name",
                Sort::Added => "added",
            }
            .into(),
            descending: query.descending,
            active_filters: query.active_filters(),
        },
        categories: found.categories,
        scopes: Scope::ALL
            .iter()
            .map(|&s| (scope_key(s).to_owned(), scope_label(s).to_owned()))
            .collect(),
        rows,
        busy: snapshot.busy.map(|p| BusyView {
            task: p.task,
            message: p.message,
            done: p.done,
            total: p.total,
        }),
        message: (!snapshot.message.is_empty()).then(|| MessageView {
            text: snapshot.message.clone(),
            failed: snapshot.failed,
        }),
        outdated,
        update_all: if busy {
            Control::off(wait.clone())
        } else {
            Control::when(outdated > 0, "Everything is up to date")
        },
        download_all: Control::when(!busy, wait.clone()),
        refresh: Control::when(!busy, wait),
    }
}

/// A change to the tab's query from a shell: the fields given, after `clearFilters`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RepoQuery {
    pub search: Option<String>,
    /// A scope's key ([`scope_key`]).
    pub scope: Option<String>,
    pub category: Option<String>,
    pub pinned_only: Option<bool>,
    pub local_only: Option<bool>,
    /// "name" or "added".
    pub sort: Option<String>,
    pub descending: Option<bool>,
    /// Clears the filters first.
    pub clear_filters: Option<bool>,
}

/// Changes the tab's query from a shell.
pub fn set_query(bot: &Arc<Bot>, change: RepoQuery) -> Result<(), String> {
    let state = STATE.of(bot);
    let mut state = lock(&state);
    let query = &mut state.query;
    if change.clear_filters == Some(true) {
        query.clear_filters();
    }
    if let Some(search) = change.search {
        query.search = search;
    }
    if let Some(scope) = change.scope {
        query.scope = Scope::ALL
            .into_iter()
            .find(|&s| scope_key(s) == scope)
            .ok_or_else(|| format!("no scope named {scope}"))?;
    }
    if let Some(category) = change.category {
        query.category = category;
    }
    if let Some(on) = change.pinned_only {
        query.pinned_only = on;
    }
    if let Some(on) = change.local_only {
        query.local_only = on;
    }
    if let Some(sort) = change.sort {
        query.sort = match sort.as_str() {
            "name" => Sort::Name,
            "added" => Sort::Added,
            other => return Err(format!("no sort named {other}")),
        };
    }
    if let Some(on) = change.descending {
        query.descending = on;
    }
    Ok(())
}

/// A row's work from a shell; why not, when it can't run now.
pub fn work(bot: &Arc<Bot>, path: &str, work: Work) -> Result<(), String> {
    let snapshot = crate::turu::script_repo().snapshot();
    let allowed = rules(bot, &snapshot).for_row(path);
    let ok = match work {
        Work::Start => allowed.start,
        Work::Load => allowed.load,
        Work::Queue => allowed.queue,
        Work::Download | Work::Replace => snapshot.busy.is_none(),
    };
    if !ok {
        return Err(match work {
            Work::Queue => allowed.queue_tip,
            Work::Download | Work::Replace => wait_tip(&snapshot),
            _ => allowed.run_tip,
        });
    }
    match run(bot, path, work) {
        Ran::Spawned => {
            lock(&STATE.of(bot)).working = Some(path.to_owned());
            Ok(())
        }
        Ran::Done(true) => Ok(()),
        Ran::Done(false) => Err(crate::turu::script_repo().snapshot().message),
        Ran::Refused => Err(wait_tip(&crate::turu::script_repo().snapshot())),
    }
}

/// Makes the script at `path` Army > Accounts' launch script (`accounts.json`'s `script`, the
/// local file, as the egui Accounts' Search… sets it), downloading it first when it needs that
/// (as Search Scripts' pick mode does). True when it's set now; false while the repository's
/// task downloads it, which sets it once done (a native shell waits for the task to end).
pub fn set_launch_script(bot: &Arc<Bot>, path: &str) -> Result<bool, String> {
    let file = bot.paths.accounts_file();
    let set = move |local: std::path::PathBuf| {
        let mut list = turu_bot::accounts::Accounts::load(&file);
        list.script = Some(local);
        list.save(&file).map_err(|e| format!("Couldn't save the accounts: {e}"))
    };
    let repo = crate::turu::script_repo();
    if let Some(local) = repo.ready(path) {
        set(local)?;
        return Ok(true);
    }
    let owned = path.to_owned();
    let spawned = repo.spawn("Download", move |repo| {
        let local = repo.ensure(&owned)?;
        set(local)?;
        Ok(format!("Downloaded {owned}; launched accounts run it."))
    });
    if spawned {
        Ok(false)
    } else {
        Err(wait_tip(&repo.snapshot()))
    }
}

/// Pins or unpins the script at `path`.
pub fn set_pinned(path: &str, pinned: bool) {
    crate::turu::script_repo().set_pinned(path, pinned);
}

/// Update all, Download all, Refresh: the repository's task; false when another runs.
pub fn task(name: &str) -> Result<(), String> {
    let repo = crate::turu::script_repo();
    let spawned = match name {
        "update-all" => repo.spawn("Update all", |repo| repo.update_all()),
        "download-all" => repo.spawn("Download all", |repo| repo.download_all()),
        "refresh" => repo.spawn("Refresh", |repo| repo.refresh()),
        "cancel" => {
            repo.cancel();
            true
        }
        other => return Err(format!("no task named {other}")),
    };
    if spawned {
        Ok(())
    } else {
        Err(wait_tip(&repo.snapshot()))
    }
}

/// Launch's answer: whether the script is set now (false while it downloads: it's set after).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LaunchScript {
    pub ready: bool,
}

/// Search Scripts' actions (`POST /models/repo/do`); `path` is a script's in the repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum RepoAct {
    Query(RepoQuery),
    Start {
        path: String,
    },
    Load {
        path: String,
    },
    Queue {
        path: String,
    },
    Download {
        path: String,
    },
    /// Downloads over the user's edits.
    Replace {
        path: String,
    },
    /// Makes it the accounts' launch script: answers `{ "ready": ... }`, false while it downloads.
    Launch {
        path: String,
    },
    Pin {
        path: String,
        pinned: bool,
    },
    UpdateAll,
    DownloadAll,
    Refresh,
    Cancel,
}

pub fn act(bot: &Arc<Bot>, act: RepoAct) -> Result<Value, String> {
    match act {
        RepoAct::Query(change) => set_query(bot, change)?,
        RepoAct::Start { path } => work(bot, &path, Work::Start)?,
        RepoAct::Load { path } => work(bot, &path, Work::Load)?,
        RepoAct::Queue { path } => work(bot, &path, Work::Queue)?,
        RepoAct::Download { path } => work(bot, &path, Work::Download)?,
        RepoAct::Replace { path } => work(bot, &path, Work::Replace)?,
        RepoAct::Launch { path } => {
            return super::to_json(&LaunchScript {
                ready: set_launch_script(bot, &path)?,
            });
        }
        RepoAct::Pin { path, pinned } => set_pinned(&path, pinned),
        RepoAct::UpdateAll => task("update-all")?,
        RepoAct::DownloadAll => task("download-all")?,
        RepoAct::Refresh => task("refresh")?,
        RepoAct::Cancel => task("cancel")?,
    }
    Ok(serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use turu_bot::repo::ScriptInfo;

    fn entry(path: &str, name: &str, added: &str, status: Status, pinned: bool) -> Entry {
        Entry {
            info: Arc::new(ScriptInfo {
                path: path.into(),
                name: name.into(),
                added: added.into(),
                ..Default::default()
            }),
            status,
            pinned,
        }
    }

    fn names(entries: &[Entry], query: &Query) -> Vec<String> {
        filter(entries, query)
            .rows
            .iter()
            .map(|&i| entries[i].info.name.clone())
            .collect()
    }

    #[test]
    fn filters_and_sorts_like_vibeskua() {
        let mut library = entry("core/CoreBots.ts", "CoreBots", "", Status::UpToDate, true);
        library.info = Arc::new(ScriptInfo {
            library: true,
            ..(*library.info).clone()
        });
        let entries = vec![
            entry(
                "farm/GoldFarm.ts",
                "Gold Farm",
                "2026-01-02T00:00:00Z",
                Status::UpToDate,
                false,
            ),
            entry(
                "nation/Bags.ts",
                "Bags",
                "2026-01-03T00:00:00Z",
                Status::NotDownloaded,
                false,
            ),
            entry(
                "farm/Exp.ts",
                "Experience",
                "2026-01-01T00:00:00Z",
                Status::Outdated,
                true,
            ),
            entry("mine/Gold.ts", "My Gold", "", Status::Local, false),
            library,
        ];
        let mut query = Query::default();
        assert_eq!(names(&entries, &query), ["Experience", "Bags", "Gold Farm", "My Gold"]);
        assert_eq!(filter(&entries, &query).categories, ["All", "Farm", "Mine", "Nation"]);
        query.sort = Sort::Added;
        query.descending = true;
        assert_eq!(names(&entries, &query), ["Experience", "Bags", "Gold Farm", "My Gold"]);
        query.search = "gold".into();
        assert_eq!(names(&entries, &query), ["Gold Farm", "My Gold"]);
        query.search.clear();
        query.category = "Farm".into();
        assert_eq!(names(&entries, &query), ["Experience", "Gold Farm"]);
        assert_eq!(query.active_filters(), 1);
        query.clear_filters();
        query.local_only = true;
        assert_eq!(names(&entries, &query), ["My Gold"]);
        query.local_only = false;
        query.pinned_only = true;
        assert_eq!(names(&entries, &query), ["Experience"]);
    }

    #[test]
    fn a_shell_sets_the_query_by_its_fields() {
        let bot = Bot::new();
        let read = |body| serde_json::from_value::<RepoQuery>(body).unwrap();
        set_query(
            &bot,
            read(serde_json::json!({ "search": "gold", "scope": "name", "sort": "added" })),
        )
        .unwrap();
        let query = lock(&STATE.of(&bot)).query.clone();
        assert_eq!(
            (query.search.as_str(), query.scope, query.sort),
            ("gold", Scope::Name, Sort::Added)
        );
        assert!(set_query(&bot, read(serde_json::json!({ "scope": "nothing" }))).is_err());
        set_query(
            &bot,
            read(serde_json::json!({ "category": "Farm", "clearFilters": true })),
        )
        .unwrap();
        assert_eq!(lock(&STATE.of(&bot)).query.category, "Farm");
    }
}
