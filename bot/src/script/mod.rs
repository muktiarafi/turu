//! Loading, starting, stopping and pausing scripts (VibeSkua `ScriptManager`).

mod loader;
pub mod options;
mod runtime;
pub mod transpile;

use crate::Bot;
use crate::control::{ScriptControl, mark_script_thread};
use crate::error::{BotError, BotResult};
use crate::paths::Paths;
use crate::webhooks::Notice;
use options::ScriptOptions;
use parking_lot::{Condvar, Mutex};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

pub use loader::normalize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptStatus {
    /// No script loaded: "[No Script Loaded]".
    Empty,
    /// A script is loaded but not running.
    Loaded,
    Compiling,
    Running,
    Paused,
    Stopping,
    /// The last run failed: "[Error]".
    Error,
}

impl ScriptStatus {
    /// The text in the Script Loader's status pill (VibeSkua's strings).
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "[No Script Loaded]",
            Self::Loaded => "[Loaded]",
            Self::Compiling => "Compiling...",
            Self::Running => "[Running]",
            Self::Paused => "[Paused]",
            Self::Stopping => "Stopping...",
            Self::Error => "[Error]",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Compiling | Self::Running | Self::Paused | Self::Stopping
        )
    }
}

/// The `/* name: ... description: ... tags: ... */` header of a script.
#[derive(Debug, Clone, Default)]
pub struct ScriptInfo {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

impl ScriptInfo {
    pub fn parse(source: &str) -> Self {
        let mut info = Self::default();
        let Some(start) = source.trim_start().strip_prefix("/*") else {
            return info;
        };
        let header = start.split("*/").next().unwrap_or_default();
        for line in header.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim().to_owned();
            match key.trim().to_ascii_lowercase().as_str() {
                "name" => info.name = Some(value),
                "description" => info.description = Some(value),
                "tags" => {
                    info.tags = value
                        .split(',')
                        .map(|t| t.trim().to_owned())
                        .filter(|t| !t.is_empty())
                        .collect()
                }
                _ => {}
            }
        }
        info
    }
}

/// The loaded script's options, as the Script Options window and `bot.config` see them.
#[derive(Debug, Clone, Default)]
pub enum ScriptOptionsState {
    /// No script is loaded, or the loaded one declares no options.
    #[default]
    NoOptions,
    /// The script's options and their values.
    Loaded(ScriptOptions),
    /// The options couldn't be read (the text says why).
    Failed(String),
}

pub struct ScriptManager {
    loaded: Mutex<Option<PathBuf>>,
    status: Mutex<ScriptStatus>,
    running: AtomicBool,
    paths: Paths,
    options: Mutex<ScriptOptionsState>,
    /// A UI is there to show the Script Options window (not `--headless`, not the tests).
    interactive: AtomicBool,
    /// This run shows no Script Options window (VibeSkua `SilentConfig`).
    silent: AtomicBool,
    /// The open request for the Script Options window, until the UI closes it.
    configure: Mutex<Option<u64>>,
    configured: Condvar,
    next_configure: AtomicU64,
}

impl ScriptManager {
    pub fn new(paths: Paths) -> Self {
        Self {
            loaded: Mutex::new(None),
            status: Mutex::new(ScriptStatus::Empty),
            running: AtomicBool::new(false),
            paths,
            options: Mutex::new(ScriptOptionsState::NoOptions),
            interactive: AtomicBool::new(false),
            silent: AtomicBool::new(false),
            configure: Mutex::new(None),
            configured: Condvar::new(),
            next_configure: AtomicU64::new(1),
        }
    }

    pub fn status(&self) -> ScriptStatus {
        *self.status.lock()
    }

    fn set_status(&self, status: ScriptStatus) {
        *self.status.lock() = status;
    }

    pub fn loaded(&self) -> Option<PathBuf> {
        self.loaded.lock().clone()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// "Load Script": remembers the file and reads its options (`scriptOptions`) with their saved
    /// values. The script is compiled on Start.
    pub fn load(&self, path: &Path) {
        let path = normalize(&std::path::absolute(path).unwrap_or(path.into()));
        let options = self.read_options(&path);
        *self.options.lock() = options;
        *self.loaded.lock() = Some(path);
        if !self.is_running() {
            self.set_status(ScriptStatus::Loaded);
        }
    }

    fn read_options(&self, path: &Path) -> ScriptOptionsState {
        match options::declaration(path) {
            Ok(None) => ScriptOptionsState::NoOptions,
            Ok(Some(decl)) => {
                let file = self.paths.script_options_file(&decl.storage);
                match ScriptOptions::load(decl, file) {
                    Ok(options) => ScriptOptionsState::Loaded(options),
                    Err(e) => {
                        ScriptOptionsState::Failed(format!("couldn't read the saved options: {e}"))
                    }
                }
            }
            Err(e) => ScriptOptionsState::Failed(e),
        }
    }

    /// The loaded script's options (a copy: the window edits its own).
    pub fn options(&self) -> ScriptOptionsState {
        self.options.lock().clone()
    }

    /// Whether the loaded script declares options.
    pub fn has_options(&self) -> bool {
        matches!(*self.options.lock(), ScriptOptionsState::Loaded(_))
    }

    /// `bot.config.has`: whether the loaded script declares the option. `category` is
    /// [`options::MAIN`] or a group's name.
    pub fn has_option(&self, category: &str, name: &str) -> bool {
        matches!(&*self.options.lock(), ScriptOptionsState::Loaded(o) if o.get_in(category, name).is_some())
    }

    /// `bot.config.get`: the value of one of the loaded script's options.
    pub fn option_value(&self, category: &str, name: &str) -> Result<Value, String> {
        self.with_options(|options| {
            options.get_in(category, name).cloned().ok_or_else(|| {
                if category == options::MAIN {
                    format!("this script has no option named \"{name}\"")
                } else {
                    format!("this script has no option named \"{name}\" in \"{category}\"")
                }
            })
        })
    }

    /// `bot.config.set`: sets an option of the loaded script and saves it.
    pub fn set_option_value(&self, category: &str, name: &str, value: Value) -> Result<(), String> {
        self.with_options(|options| {
            options.set_in(category, name, value)?;
            options
                .save()
                .map_err(|e| format!("couldn't save the options: {e}"))
        })
    }

    /// Saves the values of the Script Options window (VibeSkua `SaveOptions`). Refused when the
    /// loaded script is no longer the one the window was opened for.
    pub fn save_options(&self, draft: ScriptOptions) -> Result<(), String> {
        let mut state = self.options.lock();
        let ScriptOptionsState::Loaded(current) = &*state else {
            return Err("the loaded script declares no options".into());
        };
        if current.declaration() != draft.declaration() {
            return Err("the loaded script changed; open Script Options again".into());
        }
        draft
            .save()
            .map_err(|e| format!("couldn't save {}: {e}", draft.path().display()))?;
        *state = ScriptOptionsState::Loaded(draft);
        Ok(())
    }

    /// Called by the desktop UI: it shows the Script Options window when a script asks for it.
    pub fn set_interactive(&self, interactive: bool) {
        self.interactive.store(interactive, Ordering::Release);
    }

    /// The request the UI should answer by showing the Script Options window, if any.
    pub fn configure_request(&self) -> Option<u64> {
        *self.configure.lock()
    }

    /// The UI closed the Script Options window it opened for `request` (Save or Cancel).
    pub fn finish_configure(&self, request: u64) {
        let mut pending = self.configure.lock();
        if *pending == Some(request) {
            *pending = None;
            self.configured.notify_all();
        }
    }

    /// `bot.config.configure` (VibeSkua `Config.Configure()`): shows the Script Options window
    /// and waits until it closes. Returns at once when the script declares no options, the run
    /// is silent or there is no UI; Stop ends the wait.
    pub fn configure(&self, control: &ScriptControl) -> BotResult<()> {
        if !self.has_options()
            || self.silent.load(Ordering::Acquire)
            || !self.interactive.load(Ordering::Acquire)
        {
            return Ok(());
        }
        let request = self.next_configure.fetch_add(1, Ordering::Relaxed);
        let mut pending = self.configure.lock();
        *pending = Some(request);
        while *pending == Some(request) {
            if control.should_exit() {
                *pending = None;
                return Err(BotError::Stopped);
            }
            self.configured
                .wait_for(&mut pending, Duration::from_millis(100));
        }
        Ok(())
    }

    /// Before the run (VibeSkua `StartScript`): shows the window unless the script set
    /// `dontPreconfigure`.
    fn preconfigure(&self, control: &ScriptControl) -> BotResult<()> {
        let needs_config = matches!(
            &*self.options.lock(),
            ScriptOptionsState::Loaded(o) if !o.declaration().dont_preconfigure
        );
        if needs_config {
            self.configure(control)?;
        }
        Ok(())
    }

    fn with_options<T>(
        &self,
        f: impl FnOnce(&mut ScriptOptions) -> Result<T, String>,
    ) -> Result<T, String> {
        match &mut *self.options.lock() {
            ScriptOptionsState::Loaded(options) => f(options),
            ScriptOptionsState::NoOptions => Err("the loaded script declares no options".into()),
            ScriptOptionsState::Failed(e) => {
                Err(format!("the script's options couldn't be read: {e}"))
            }
        }
    }
}

/// "Start Script": runs the loaded script on a new "Script Thread".
pub fn start(bot: &Arc<Bot>) -> Result<(), String> {
    start_with(bot, false)
}

/// Starts the script without showing the Script Options window (VibeSkua `SilentConfig`): for
/// runs nobody is there to answer it, such as the command line, the HTTP API and auto-relogin.
pub fn start_silent(bot: &Arc<Bot>) -> Result<(), String> {
    start_with(bot, true)
}

fn start_with(bot: &Arc<Bot>, silent: bool) -> Result<(), String> {
    let Some(path) = bot.scripts.loaded() else {
        return Err("No script loaded".into());
    };
    if bot.scripts.running.swap(true, Ordering::AcqRel) {
        return Err("A script is already running".into());
    }
    bot.scripts.silent.store(silent, Ordering::Release);
    bot.control.reset();
    // VibeSkua's `Handlers.Clear()` before the script thread starts.
    bot.handlers.clear();
    bot.scripts.set_status(ScriptStatus::Compiling);
    let thread_bot = bot.clone();
    std::thread::Builder::new()
        .name("Script Thread".into())
        .spawn(move || {
            let bot = thread_bot;
            mark_script_thread(true);
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            bot.log(format!("Started script {name}"));
            bot.webhooks.send(Notice::ScriptStarted {
                script: script_stem(&path),
            });
            bot.state.lock().stats.started = Some(std::time::Instant::now());
            let result = match bot.scripts.preconfigure(&bot.control) {
                Ok(()) => runtime::run(&bot, &path),
                Err(_) => Err(runtime::RunError::Stopped),
            };
            mark_script_thread(false);
            finish(&bot, &name, result);
        })
        .map_err(|e| {
            bot.scripts.running.store(false, Ordering::Release);
            e.to_string()
        })?;
    Ok(())
}

/// "Stop Script": the script's next bot call (or any running JS) stops it.
pub fn stop(bot: &Bot) {
    if bot.scripts.is_running() {
        bot.scripts.set_status(ScriptStatus::Stopping);
        bot.control.request_stop();
    }
}

pub fn set_paused(bot: &Bot, paused: bool) {
    if bot.scripts.is_running() {
        bot.control.set_paused(paused);
        bot.scripts.set_status(if paused {
            ScriptStatus::Paused
        } else {
            ScriptStatus::Running
        });
    }
}

pub(crate) fn set_running(bot: &Bot) {
    bot.scripts.set_status(ScriptStatus::Running);
}

/// The script's stopping handlers run ("Stopping...").
pub(crate) fn set_stopping(bot: &Bot) {
    bot.scripts.set_status(ScriptStatus::Stopping);
}

/// Cleanup after `main` returns, throws or is stopped (VibeSkua: ScriptStopping/Stopped).
fn finish(bot: &Arc<Bot>, name: &str, result: Result<(), runtime::RunError>) {
    bot.skills.stop();
    // VibeSkua `ScriptCombat.ScriptStopped` (the counter handler itself stays on).
    bot.combat.set_stop_attacking(false);
    bot.drops.stop();
    bot.boosts.reset();
    bot.quests.unregister_all();
    bot.handlers.clear();
    bot.options.write().reset_after_script();
    let mut error = None;
    let status = match result {
        Ok(()) => {
            bot.log(format!("Script {name} finished"));
            ScriptStatus::Loaded
        }
        Err(runtime::RunError::Stopped) => {
            bot.log(format!("Script {name} stopped"));
            ScriptStatus::Loaded
        }
        Err(runtime::RunError::Failed(message)) => {
            bot.log(format!("Script {name} failed:\n{message}"));
            error = Some(message);
            ScriptStatus::Error
        }
    };
    bot.control.reset();
    bot.scripts.set_status(status);
    bot.scripts.running.store(false, Ordering::Release);
    bot.events.emit(crate::events::BotEvent::ScriptStopped);
    let script = bot
        .scripts
        .loaded()
        .map_or_else(String::new, |p| script_stem(&p));
    bot.webhooks.send(Notice::ScriptStopped { script, error });
}

/// The script's name as the webhooks show it (VibeSkua: the file name without extension).
fn script_stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(|| "a script".into(), |s| s.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header() {
        let info = ScriptInfo::parse(
            "/*\nname: 0MaxBags\ndescription: Farms bags.\ntags: nation, bags\n*/\nimport x",
        );
        assert_eq!(info.name.as_deref(), Some("0MaxBags"));
        assert_eq!(info.tags, ["nation", "bags"]);
    }
}
