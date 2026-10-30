//! Loading, starting, stopping and pausing scripts (VibeSkua `ScriptManager`).

mod loader;
pub mod options;
mod runtime;
pub mod transpile;

use crate::Bot;
use crate::control::mark_script_thread;
use crate::paths::Paths;
use options::ScriptOptions;
use parking_lot::Mutex;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
}

impl ScriptManager {
    pub fn new(paths: Paths) -> Self {
        Self {
            loaded: Mutex::new(None),
            status: Mutex::new(ScriptStatus::Empty),
            running: AtomicBool::new(false),
            paths,
            options: Mutex::new(ScriptOptionsState::NoOptions),
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

    /// `bot.config.get`: the value of one of the loaded script's options.
    pub fn option_value(&self, name: &str) -> Result<Value, String> {
        self.with_options(|options| {
            options
                .get(name)
                .cloned()
                .ok_or_else(|| format!("this script has no option named \"{name}\""))
        })
    }

    /// `bot.config.set`: sets an option of the loaded script and saves it.
    pub fn set_option_value(&self, name: &str, value: Value) -> Result<(), String> {
        self.with_options(|options| {
            options.set(name, value)?;
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
    let Some(path) = bot.scripts.loaded() else {
        return Err("No script loaded".into());
    };
    if bot.scripts.running.swap(true, Ordering::AcqRel) {
        return Err("A script is already running".into());
    }
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
            bot.state.lock().stats.started = Some(std::time::Instant::now());
            let result = runtime::run(&bot, &path);
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
    bot.drops.stop();
    bot.boosts.reset();
    bot.quests.unregister_all();
    bot.handlers.clear();
    bot.options.write().reset_after_script();
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
            ScriptStatus::Error
        }
    };
    bot.control.reset();
    bot.scripts.set_status(status);
    bot.scripts.running.store(false, Ordering::Release);
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
