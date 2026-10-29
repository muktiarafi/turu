//! Loading, starting, stopping and pausing scripts (VibeSkua `ScriptManager`).

mod loader;
mod runtime;
pub mod transpile;

use crate::Bot;
use crate::control::mark_script_thread;
use parking_lot::Mutex;
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

pub struct ScriptManager {
    loaded: Mutex<Option<PathBuf>>,
    status: Mutex<ScriptStatus>,
    running: AtomicBool,
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self {
            loaded: Mutex::new(None),
            status: Mutex::new(ScriptStatus::Empty),
            running: AtomicBool::new(false),
        }
    }
}

impl ScriptManager {
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

    /// "Load Script": remembers the file; it is read and compiled on Start.
    pub fn load(&self, path: &Path) {
        *self.loaded.lock() = Some(normalize(&std::path::absolute(path).unwrap_or(path.into())));
        if !self.is_running() {
            self.set_status(ScriptStatus::Loaded);
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
