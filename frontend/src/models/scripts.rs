//! Scripts (Script Loader, the nav bar's script chip, the run controls): the loaded script, its
//! state, how its last run ended, and what Start, Stop, Pause/Resume, Force Stop, Load and Edit
//! Script Options may do now, with the reasons when they may not (VibeSkua's ScriptLoader and its
//! ToggleScript hotkey, with Turu's Force Stop wait).

use super::Control;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use turu_bot::Bot;
use turu_bot::script::{self, ScriptOptionsState, ScriptStatus};

/// A running script whose log has been quiet this long may be stuck: its last log is `stale`.
pub const QUIET_FOR: Duration = Duration::from_secs(5 * 60);

/// Why Force Stop is disabled at first ([`script::FORCE_STOP_AFTER`]).
pub const FORCE_STOP_WAIT: &str = "Force Stop is offered after 1.5 s of stopping";

/// What Force Stop does, while it's offered.
pub const FORCE_STOP_DOES: &str = "Interrupts the script's stopping steps (such as going to the stop location)";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ScriptsView {
    pub status: Status,
    /// The state in words: "No script loaded", "Ready", "Running", ...
    pub state: &'static str,
    /// How long the running script has run (the Stats clock).
    pub elapsed_secs: Option<u64>,
    pub script: Option<Loaded>,
    /// After a failure: the error, and its first line for one-line places.
    pub error: Option<ScriptError>,
    /// How the loaded script's last run ended, while it's what the Script Loader shows (stopped,
    /// and the same script).
    pub last_run: Option<LastRun>,
    /// While it runs: the script log's newest line's age.
    pub last_log: Option<LastLog>,
    /// What the script waits for the user to answer ("Script Options", "a message box").
    pub waits: Option<&'static str>,
    pub controls: Controls,
}

/// [`ScriptStatus`] for a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum Status {
    Empty,
    Loaded,
    Compiling,
    Running,
    Paused,
    Stopping,
    Error,
}

impl From<ScriptStatus> for Status {
    fn from(status: ScriptStatus) -> Self {
        match status {
            ScriptStatus::Empty => Self::Empty,
            ScriptStatus::Loaded => Self::Loaded,
            ScriptStatus::Compiling => Self::Compiling,
            ScriptStatus::Running => Self::Running,
            ScriptStatus::Paused => Self::Paused,
            ScriptStatus::Stopping => Self::Stopping,
            ScriptStatus::Error => Self::Error,
        }
    }
}

impl Status {
    /// A script runs, starts or stops.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Compiling | Self::Running | Self::Paused | Self::Stopping)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct Loaded {
    /// Its full path (a string: views cross to other languages).
    pub path: String,
    /// Its file name ("Gold.ts").
    pub name: String,
    /// On the side bar's quick list (`quick_scripts`).
    pub favourite: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ScriptError {
    pub first_line: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    /// `main` returned (else it was stopped, or failed: then `error` says why).
    pub finished: bool,
    pub ran_secs: u64,
    /// When it ended, as a clock ("04:12").
    pub ended_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LastLog {
    pub quiet_secs: u64,
    /// Its time ("14:02:31").
    pub at: String,
    /// Quiet for [`QUIET_FOR`] while running: it may be stuck.
    pub stale: bool,
}

/// The run and setup controls. `start`, `stop`, `pause` and `force_stop` are each `None` when
/// not shown now: Start while nothing runs, Stop and Pause (or Resume) while it runs, Force Stop
/// in Pause's place while it stops.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct Controls {
    pub start: Option<Control>,
    pub stop: Option<Control>,
    /// Pause, or Resume while paused (`paused`).
    pub pause: Option<Control>,
    pub paused: bool,
    pub force_stop: Option<Control>,
    /// Load Script (or loading one from Search Scripts): not while a script runs.
    pub load: Control,
    pub edit_options: Control,
}

/// The state in words (`ScriptStatus::label` keeps VibeSkua's "[Running]" for the control API).
pub fn state_text(status: ScriptStatus) -> &'static str {
    match status {
        ScriptStatus::Empty => "No script loaded",
        ScriptStatus::Loaded => "Ready",
        ScriptStatus::Compiling => "Starting...",
        ScriptStatus::Running => "Running",
        ScriptStatus::Paused => "Paused",
        ScriptStatus::Stopping => "Stopping...",
        ScriptStatus::Error => "Stopped with an error",
    }
}

/// Scripts' view of `bot` now.
pub fn view(bot: &Bot) -> ScriptsView {
    let status = bot.scripts.status();
    let loaded = bot.scripts.loaded();
    let active = status.is_active();
    let end = bot.scripts.last_end().1;
    let elapsed_secs = active
        .then(|| bot.state.lock().stats.started)
        .flatten()
        .map(|started| started.elapsed().as_secs());
    let error = (status == ScriptStatus::Error)
        .then(|| end.as_ref().and_then(|end| end.error.clone()))
        .flatten()
        .map(|text| ScriptError {
            first_line: text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or_default()
                .trim()
                .to_owned(),
            text: text.trim().to_owned(),
        });
    let last_run = end
        .filter(|end| {
            matches!(status, ScriptStatus::Loaded | ScriptStatus::Error)
                && loaded.as_deref() == Some(end.script.as_path())
        })
        .map(|end| LastRun {
            finished: end.finished,
            ran_secs: end.ran.as_secs(),
            ended_at: end.ended_clock(),
        });
    let last_log = active
        .then(|| bot.logs.script.last_time())
        .flatten()
        .map(|(time, quiet)| LastLog {
            quiet_secs: quiet.as_secs(),
            at: time.format("%H:%M:%S").to_string(),
            stale: status == ScriptStatus::Running && quiet >= QUIET_FOR,
        });
    let script = loaded.as_deref().map(|path| Loaded {
        path: path.to_string_lossy().into_owned(),
        name: path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        favourite: is_favourite(bot, path),
    });
    ScriptsView {
        status: status.into(),
        state: state_text(status),
        elapsed_secs,
        script,
        error,
        last_run,
        last_log,
        waits: bot.scripts.waits_for(),
        controls: controls(bot, status, loaded.is_some()),
    }
}

fn controls(bot: &Bot, status: ScriptStatus, loaded: bool) -> Controls {
    let active = status.is_active();
    let stopping = status == ScriptStatus::Stopping;
    let running = matches!(status, ScriptStatus::Running | ScriptStatus::Paused);
    let start = (!active).then(|| match start_blocker(bot, loaded) {
        Some(reason) => Control::off(reason),
        None => Control::on(),
    });
    // While stopping, Stop rests (a second press would skip the stopping steps).
    let stop = active.then(|| Control::when(!stopping, "Stopping: the script is running its stopping steps"));
    let pause = running.then(Control::on);
    let force_stop = stopping.then(|| Control::when(bot.scripts.can_force_stop(), FORCE_STOP_WAIT));
    let edit_options = match (loaded, bot.scripts.options()) {
        (false, _) => Control::off("Load a script first"),
        (true, ScriptOptionsState::Loaded(_)) => Control::on(),
        (true, ScriptOptionsState::Failed(_)) => Control::off("The script's options couldn't be read"),
        (true, ScriptOptionsState::NoOptions) => Control::off("This script declares no options"),
    };
    Controls {
        start,
        stop,
        pause,
        paused: status == ScriptStatus::Paused,
        force_stop,
        load: Control::when(!active, "Stop the script first"),
        edit_options,
    }
}

/// Edit Script Options' state (also the menus').
pub fn view_controls_edit_options(bot: &Bot) -> Control {
    controls(bot, bot.scripts.status(), bot.scripts.loaded().is_some()).edit_options
}

/// Why Start can't start now: no script, or Auto or a Console snippet runs.
fn start_blocker(bot: &Bot, loaded: bool) -> Option<&'static str> {
    if bot.auto.is_running() {
        Some("Stop Auto Attack/Hunt first")
    } else if bot.scripts.is_snippet_running() {
        Some("Stop the Console snippet first")
    } else if !loaded {
        Some("Load a script first")
    } else {
        None
    }
}

fn is_favourite(bot: &Bot, path: &Path) -> bool {
    turu_bot::quick_scripts::get(&bot.paths.quick_scripts_file()).is_favorite(path)
}

/// Start Script; why not, as the log says it.
pub fn start(bot: &Arc<Bot>) -> Result<(), String> {
    if bot.scripts.status().is_active() {
        return Err("A script is running".into());
    }
    if let Some(reason) = start_blocker(bot, bot.scripts.loaded().is_some()) {
        return Err(reason.into());
    }
    script::start(bot).map_err(|e| e.to_string())
}

/// Stop Script, or Force Stop while stopping once it's offered; nothing otherwise.
pub fn stop(bot: &Bot) {
    let status = bot.scripts.status();
    if status == ScriptStatus::Stopping && !bot.scripts.can_force_stop() {
        return;
    }
    if status.is_active() {
        script::stop(bot);
    }
}

/// Pause, or Resume with `false`.
pub fn set_paused(bot: &Bot, paused: bool) {
    script::set_paused(bot, paused);
}

/// VibeSkua's ToggleScript hotkey: Start, or Stop (Force Stop while stopping, once offered).
pub fn toggle(bot: &Arc<Bot>) {
    if bot.scripts.status().is_active() {
        stop(bot);
    } else if let Err(e) = start(bot) {
        // The hotkey says nothing when there's nothing to start.
        if bot.scripts.loaded().is_some() {
            bot.log(format!("Couldn't start the script: {e}"));
        }
    }
}

/// Load Script: an absolute path, or one in the scripts folder (as `/script/load` takes it);
/// refused while a script runs.
pub fn load(bot: &Bot, path: &Path) -> Result<(), String> {
    if bot.scripts.status().is_active() {
        return Err("Stop the script first".into());
    }
    let found =
        script::resolve(&bot.paths.scripts_dir(), path).ok_or_else(|| format!("No script at {}", path.display()))?;
    bot.scripts.load(&found);
    bot.log(format!("Loaded script {}", found.display()));
    Ok(())
}

/// Adds the loaded script to the favourites, or takes it off.
pub fn toggle_favourite(bot: &Bot) {
    if let Some(path) = bot.scripts.loaded() {
        turu_bot::quick_scripts::update(&bot.paths.quick_scripts_file(), |lists| lists.toggle_favorite(&path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_stop_wait_names_the_wait() {
        let secs = script::FORCE_STOP_AFTER.as_secs_f32();
        assert!(FORCE_STOP_WAIT.contains(&format!("{secs} s")));
    }

    /// No script: Start is offered but off, saying why; nothing else runs.
    #[test]
    fn an_empty_tab_offers_start_with_its_reason() {
        let bot = turu_bot::Bot::new();
        let view = view(&bot);
        assert_eq!(view.status, Status::Empty);
        assert_eq!(view.state, "No script loaded");
        assert_eq!(view.controls.start, Some(Control::off("Load a script first")));
        assert_eq!(view.controls.stop, None);
        assert_eq!(view.controls.pause, None);
        assert!(view.controls.load.enabled);
        assert_eq!(view.controls.edit_options, Control::off("Load a script first"));
        assert!(start(&bot).is_err());
    }
}
