//! Scripts > Scheduler (VibeSkua `ScriptSchedulerView`): the tab's playlist (`bot.scheduler`), run
//! top to bottom, with what Start / Stop queue, Load, Save and each row's Move, Script options and
//! Remove may do now, and VibeSkua's message boxes as a line (`note`). Load and Save go by name in
//! the playlists folder, so a shell needs no file dialog; the desktop's dialogs stay in its window.

use super::Control;
use super::tab_state::{PerTab, lock};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::scheduler::{Entry, EntryStatus};
use turu_bot::script::ScriptOptionsState;

/// How a status or a note reads: its colour's meaning, for each shell to map to its palette.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum Tint {
    Plain,
    Muted,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct SchedulerView {
    pub running: bool,
    /// "Started at 22:10:05 · running 2 of 4", or "4 scripts".
    pub summary: String,
    pub entries: Vec<EntryView>,
    /// Start queue, or Stop queue while it runs.
    pub toggle: Control,
    pub load: Control,
    pub save: Control,
    /// Reordering and removing (not while it runs).
    pub edit: Control,
    /// The saved playlists' file names ("Daily.json"), for Load.
    pub playlists: Vec<String>,
    pub note: Option<Note>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub id: String,
    pub name: String,
    pub path: String,
    /// "Queued", "Running", "File Not Found", ...
    pub status: &'static str,
    pub tint: Tint,
    /// "00:00" until a run ended, then "hh:mm:ss".
    pub time: String,
    /// The one running now (or waiting to).
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub text: String,
    pub tint: Tint,
}

/// The tab's Scheduler state that isn't the playlist: the last note, and an entry's options read
/// for Script options (the window that edits them takes it).
#[derive(Default)]
pub struct State {
    pub note: Option<Note>,
    pub read: Option<ScriptOptionsState>,
}

pub static STATE: PerTab<State> = PerTab::new();

/// Running green, stopped amber, problems red, completed dim, the rest plain.
pub fn tint(status: EntryStatus) -> Tint {
    match status {
        EntryStatus::Running => Tint::Success,
        EntryStatus::Stopped => Tint::Warning,
        s if s.is_problem() => Tint::Danger,
        EntryStatus::Completed => Tint::Muted,
        _ => Tint::Plain,
    }
}

pub fn view(bot: &Arc<Bot>) -> SchedulerView {
    let scheduler = &bot.scheduler;
    let running = scheduler.is_running();
    let current = scheduler.current();
    let entries: Vec<EntryView> = scheduler
        .entries()
        .iter()
        .enumerate()
        .map(|(i, e)| EntryView {
            id: e.id.clone(),
            name: e.name.clone(),
            path: e.path.display().to_string(),
            status: e.status.label(),
            tint: tint(e.status),
            time: e.duration_text(),
            current: current == Some(i),
        })
        .collect();
    let empty = entries.is_empty();
    SchedulerView {
        running,
        summary: scheduler.summary(),
        toggle: Control::when(running || !empty, "Add scripts to the playlist first"),
        load: Control::when(!running, "Cannot load a playlist while the queue is running"),
        save: Control::when(!empty, "The playlist is empty"),
        edit: Control::when(!running, "Cannot change the playlist while the queue is running"),
        playlists: playlists(bot),
        note: lock(&STATE.of(bot)).note.clone(),
        entries,
    }
}

/// The playlists folder's `.json` files, by name.
pub fn playlists(bot: &Bot) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(bot.paths.playlists_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
        .filter_map(|p| Some(p.file_name()?.to_string_lossy().into_owned()))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

fn set_note(bot: &Arc<Bot>, note: Option<Note>) {
    lock(&STATE.of(bot)).note = note;
}

fn warn(text: impl Into<String>) -> Option<Note> {
    Some(Note {
        text: text.into(),
        tint: Tint::Warning,
    })
}

/// Start queue, or Stop queue while it runs (which asks the script to stop and waits for nothing).
pub fn toggle(bot: &Arc<Bot>) {
    let scheduler = &bot.scheduler;
    if scheduler.is_running() {
        scheduler.stop(bot);
        return;
    }
    let note = scheduler.start(bot).err().and_then(warn);
    set_note(bot, note);
}

/// A playlist file chosen however the shell chooses one.
pub fn load_file(bot: &Arc<Bot>, path: &Path) {
    let note = match bot.scheduler.load(path, &bot.paths.scripts_dir()) {
        Ok(_) => None,
        Err(e) => warn(format!("Failed to load playlist: {e}")),
    };
    set_note(bot, note);
}

pub fn save_file(bot: &Arc<Bot>, path: &Path) {
    let note = match bot.scheduler.save(path, &bot.paths.scripts_dir()) {
        Ok(()) => Some(Note {
            text: "Playlist saved successfully.".into(),
            tint: Tint::Success,
        }),
        Err(e) => warn(format!("Failed to save playlist: {e}")),
    };
    set_note(bot, note);
}

/// The playlists folder's file `name` (".json" added when missing; no folders).
pub fn named(bot: &Bot, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err("Name the playlist (letters, numbers, spaces)".into());
    }
    let folder = bot.paths.playlists_dir();
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let file = if name.to_lowercase().ends_with(".json") {
        name.to_owned()
    } else {
        format!("{name}.json")
    };
    Ok(folder.join(file))
}

pub fn load(bot: &Arc<Bot>, name: &str) -> Result<(), String> {
    load_file(bot, &named(bot, name)?);
    Ok(())
}

pub fn save(bot: &Arc<Bot>, name: &str) -> Result<(), String> {
    save_file(bot, &named(bot, name)?);
    Ok(())
}

fn result(bot: &Arc<Bot>, result: Result<(), String>) {
    if let Err(e) = result {
        set_note(bot, warn(e));
    }
}

pub fn move_by(bot: &Arc<Bot>, id: &str, by: isize) {
    result(bot, bot.scheduler.move_by(id, by));
}

pub fn remove(bot: &Arc<Bot>, id: &str) {
    result(bot, bot.scheduler.remove(id));
}

pub fn rename(bot: &Arc<Bot>, id: &str, name: &str) {
    bot.scheduler.rename(id, name);
}

/// Script options: reads the entry's options in its own storage off the caller's thread (it runs
/// the script's top level); the window that edits them takes them ([`take_read`]).
pub fn options(bot: &Arc<Bot>, id: &str) {
    if bot.scripts.is_running() {
        set_note(
            bot,
            warn("Script currently running. Stop the script to change its options."),
        );
        return;
    }
    let Some(entry) = bot.scheduler.entries().into_iter().find(|e| e.id == id) else {
        return;
    };
    let bot = bot.clone();
    let spawned = std::thread::Builder::new()
        .name("Turu Script Options".into())
        .spawn(move || read(&bot, &entry));
    if let Err(e) = spawned {
        tracing::error!("Couldn't read the script's options: {e}");
    }
}

fn read(bot: &Arc<Bot>, entry: &Entry) {
    let state = bot.scripts.read_options(&entry.path, Some(&entry.storage()));
    lock(&STATE.of(bot)).read = Some(state);
}

/// An entry's options, read for Script options: the options to edit, or `None` after a note
/// says why there are none.
pub fn take_read(bot: &Arc<Bot>) -> Option<turu_bot::script::options::ScriptOptions> {
    let state = STATE.of(bot);
    let mut state = lock(&state);
    match state.read.take()? {
        ScriptOptionsState::Loaded(options) => Some(options),
        ScriptOptionsState::NoOptions => {
            state.note = warn("The loaded script has no options to configure.");
            None
        }
        ScriptOptionsState::Failed(e) => {
            state.note = Some(Note {
                text: format!("Script cannot be configured as it has errors: {e}"),
                tint: Tint::Danger,
            });
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_in_the_playlists_folder() {
        let bot = Bot::new();
        assert!(named(&bot, "../x").is_err());
        assert!(named(&bot, " ").is_err());
        assert!(named(&bot, ".hidden").is_err());
        let path = named(&bot, "Daily").unwrap();
        assert_eq!(path, bot.paths.playlists_dir().join("Daily.json"));
        assert_eq!(named(&bot, "a.JSON").unwrap().file_name().unwrap(), "a.JSON");
    }

    #[test]
    fn an_empty_playlist_offers_nothing_to_start_or_save() {
        let bot = Bot::new();
        let view = view(&bot);
        assert_eq!(view.toggle, Control::off("Add scripts to the playlist first"));
        assert_eq!(view.save, Control::off("The playlist is empty"));
        assert!(view.load.enabled);
    }
}
