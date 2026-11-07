//! Loading, starting, stopping and pausing scripts (VibeSkua `ScriptManager`).

mod loader;
pub mod options;
mod runtime;
pub mod transpile;

use crate::Bot;
use crate::control::mark_script_thread;
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

/// How long a script must have been stopping before Stop again forces it (interrupts its
/// stopping steps), so a double press of Stop or F10 only stops.
pub const FORCE_STOP_AFTER: Duration = Duration::from_millis(1500);

pub use loader::normalize;
pub use runtime::RunError;

/// Finds a script given on the command line, to the control API or in a playlist: absolute, or
/// relative to the scripts folder (else to the working folder), `.ts` optional.
pub fn resolve(scripts: &Path, path: &Path) -> Option<PathBuf> {
    let with_ts = path.with_extension("ts");
    let bases: &[&Path] = if path.is_absolute() {
        &[Path::new("")]
    } else {
        &[scripts, Path::new(".")]
    };
    bases
        .iter()
        .flat_map(|base| [base.join(path), base.join(&with_ts)])
        .find(|p| p.is_file())
}

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

/// Where a script's option values come from (`/script/start`'s and `/script/options`' `options`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OptionsSource {
    /// The script declares no options.
    None,
    /// Its options file (`options/<storage>.cfg`).
    Saved,
    /// The declared defaults: there is no options file yet.
    Defaults,
    /// The options couldn't be read.
    Failed,
}

impl OptionsSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Saved => "saved",
            Self::Defaults => "defaults",
            Self::Failed => "failed",
        }
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
    /// The status and when it last changed (for Force Stop's wait).
    status: Mutex<(ScriptStatus, std::time::Instant)>,
    running: AtomicBool,
    /// A Console snippet runs (`run_snippet`); it and a script never run together.
    snippet: AtomicBool,
    paths: Paths,
    options: Mutex<ScriptOptionsState>,
    /// A UI is there to show the Script Options window (not `--headless`, not the tests).
    interactive: AtomicBool,
    /// This run shows no Script Options window (VibeSkua `SilentConfig`).
    silent: AtomicBool,
    /// This run already said that its Script Options window wasn't shown.
    unshown_noted: AtomicBool,
    /// The open request for the Script Options window, until the UI closes it.
    configure: Mutex<Option<u64>>,
    configured: Condvar,
    next_configure: AtomicU64,
    /// The message box a script waits on (`bot.showMessageBox`), and the UI's answer.
    message_box: Mutex<MessageBoxSlot>,
    message_box_answered: Condvar,
    next_message_box: AtomicU64,
    /// This run reached `main` (VibeSkua compiles in `StartScript`: a script that fails before
    /// failed to start).
    reached_main: AtomicBool,
    /// How the last run ended, and how many runs have ended.
    last_end: Mutex<Option<RunEnd>>,
    ended: AtomicU64,
}

/// How a run ended, for the Scheduler and the Script Loader's status card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEnd {
    /// The script that ran.
    pub script: PathBuf,
    /// It reached the script's `main`.
    pub started: bool,
    /// `main` returned (not stopped, not failed).
    pub finished: bool,
    /// Why it failed (not when it finished or was stopped).
    pub error: Option<String>,
    /// From Start to the end.
    pub ran: Duration,
    pub ended: chrono::DateTime<chrono::Local>,
}

impl RunEnd {
    /// When it ended, as a clock ("04:12").
    pub fn ended_clock(&self) -> String {
        self.ended.format("%H:%M").to_string()
    }
}

/// A message box a script asked for (VibeSkua `ShowMessageBox`), until the UI answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageBox {
    pub id: u64,
    pub caption: String,
    pub message: String,
    /// At least one: `["OK"]`, `["No", "Yes"]` or the script's own.
    pub buttons: Vec<String>,
}

#[derive(Default)]
struct MessageBoxSlot {
    open: Option<MessageBox>,
    /// The request answered last and the index of the clicked button (`None`: closed).
    answer: Option<(u64, Option<usize>)>,
}

impl ScriptManager {
    pub fn new(paths: Paths) -> Self {
        Self {
            loaded: Mutex::new(None),
            status: Mutex::new((ScriptStatus::Empty, std::time::Instant::now())),
            running: AtomicBool::new(false),
            snippet: AtomicBool::new(false),
            paths,
            options: Mutex::new(ScriptOptionsState::NoOptions),
            interactive: AtomicBool::new(false),
            silent: AtomicBool::new(false),
            unshown_noted: AtomicBool::new(false),
            configure: Mutex::new(None),
            configured: Condvar::new(),
            next_configure: AtomicU64::new(1),
            message_box: Mutex::new(MessageBoxSlot::default()),
            message_box_answered: Condvar::new(),
            next_message_box: AtomicU64::new(1),
            reached_main: AtomicBool::new(false),
            last_end: Mutex::new(None),
            ended: AtomicU64::new(0),
        }
    }

    pub fn status(&self) -> ScriptStatus {
        self.status.lock().0
    }

    /// Whether a Stop now would be a Force Stop that is allowed: the script has been stopping
    /// for [`FORCE_STOP_AFTER`], so a double press of Stop doesn't skip its stopping steps.
    pub fn can_force_stop(&self) -> bool {
        let (status, since) = *self.status.lock();
        status == ScriptStatus::Stopping && since.elapsed() >= FORCE_STOP_AFTER
    }

    fn set_status(&self, status: ScriptStatus) {
        let mut current = self.status.lock();
        if current.0 != status {
            *current = (status, std::time::Instant::now());
        }
    }

    pub fn loaded(&self) -> Option<PathBuf> {
        self.loaded.lock().clone()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Whether a Console snippet runs.
    pub fn is_snippet_running(&self) -> bool {
        self.snippet.load(Ordering::Acquire)
    }

    /// "Load Script": remembers the file and reads its options (`scriptOptions`) with their saved
    /// values. The script is compiled on Start.
    pub fn load(&self, path: &Path) {
        self.load_with_storage(path, None);
    }

    /// [`Self::load`] with the options saved in `options/<storage>.cfg` instead of the script's
    /// own storage (VibeSkua `OverrideStorage`), until another script is loaded.
    pub fn load_with_storage(&self, path: &Path, storage: Option<&str>) {
        let path = normalize(&std::path::absolute(path).unwrap_or(path.into()));
        let options = self.read_options(&path, storage);
        *self.options.lock() = options;
        *self.loaded.lock() = Some(path);
        if !self.is_running() {
            self.set_status(ScriptStatus::Loaded);
        }
    }

    /// The options `path` declares with their values saved in `storage` (default: the script's
    /// own). Runs the script's top level: not on the UI thread.
    pub fn read_options(&self, path: &Path, storage: Option<&str>) -> ScriptOptionsState {
        match options::declaration(path) {
            Ok(None) => ScriptOptionsState::NoOptions,
            Ok(Some(decl)) => {
                let file = self
                    .paths
                    .script_options_file(storage.unwrap_or(&decl.storage));
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

    /// How many runs have ended, and how the last one did.
    pub fn last_end(&self) -> (u64, Option<RunEnd>) {
        let end = self.last_end.lock().clone();
        (self.ended.load(Ordering::Acquire), end)
    }

    /// The loaded script's options (a copy: the window edits its own).
    pub fn options(&self) -> ScriptOptionsState {
        self.options.lock().clone()
    }

    /// Whether the loaded script declares options.
    pub fn has_options(&self) -> bool {
        matches!(*self.options.lock(), ScriptOptionsState::Loaded(_))
    }

    /// Where the loaded script's option values come from: its options file, or the defaults
    /// when there is none yet.
    pub fn options_source(&self) -> OptionsSource {
        match &*self.options.lock() {
            ScriptOptionsState::NoOptions => OptionsSource::None,
            ScriptOptionsState::Loaded(o) if o.saved() => OptionsSource::Saved,
            ScriptOptionsState::Loaded(_) => OptionsSource::Defaults,
            ScriptOptionsState::Failed(_) => OptionsSource::Failed,
        }
    }

    /// Whether a UI is there to show the Script Options window ([`Self::set_interactive`]).
    pub fn is_interactive(&self) -> bool {
        self.interactive.load(Ordering::Acquire)
    }

    /// Whether the current (or last) run may show the Script Options window: there is a UI and
    /// it wasn't a silent start.
    pub fn may_show_options(&self) -> bool {
        self.is_interactive() && !self.silent.load(Ordering::Acquire)
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

    /// What the script waits for the user to answer, if anything: its Script Options window
    /// or a message box. In a tab the window doesn't show, it waits until that tab is shown.
    pub fn waits_for(&self) -> Option<&'static str> {
        if self.configure_request().is_some() {
            Some("Script Options")
        } else if self.message_box.lock().open.is_some() {
            Some("a message box")
        } else {
            None
        }
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
    /// is silent or there is no UI (saying, once a run, which values it uses); Stop ends the
    /// wait.
    pub fn configure(&self, bot: &Bot) -> BotResult<()> {
        if !self.has_options() {
            return Ok(());
        }
        if !self.may_show_options() {
            self.note_unshown(bot);
            return Ok(());
        }
        let control = &bot.control;
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

    /// Says, once a run, that the Script Options window wasn't shown and which values the
    /// script uses: the log, and a monitor event when they're the defaults.
    fn note_unshown(&self, bot: &Bot) {
        if self.unshown_noted.swap(true, Ordering::AcqRel) {
            return;
        }
        let file = match &*self.options.lock() {
            ScriptOptionsState::Loaded(o) => o.path().to_owned(),
            _ => return,
        };
        let why = if self.is_interactive() {
            "this start doesn't show it"
        } else {
            "no window"
        };
        let shown = file
            .file_name()
            .map_or_else(String::new, |n| format!("options/{}", n.to_string_lossy()));
        if file.is_file() {
            bot.log(format!(
                "Script Options not shown ({why}): using the options saved in {shown}."
            ));
        } else {
            bot.log(format!(
                "Script Options not shown ({why}): there is no {shown}, so the defaults are used."
            ));
            let name = self.loaded().map_or_else(String::new, |p| script_stem(&p));
            bot.monitor.event(
                crate::monitor::EventKind::ScriptOptions,
                format!("{name}: default Script Options (no {shown})"),
            );
        }
    }

    /// The message box the UI should show for the script, if any.
    pub fn message_box(&self) -> Option<MessageBox> {
        self.message_box.lock().open.clone()
    }

    /// The UI answered message box `id`: the index of the clicked button, or `None` when it was
    /// closed without one.
    pub fn answer_message_box(&self, id: u64, button: Option<usize>) {
        let mut slot = self.message_box.lock();
        if slot.open.as_ref().is_some_and(|m| m.id == id) {
            slot.open = None;
            slot.answer = Some((id, button));
            self.message_box_answered.notify_all();
        }
    }

    /// `bot.showMessageBox` (VibeSkua `ShowMessageBox`): shows a message box with `buttons`
    /// and waits for the click. Returns the clicked button's index, or `None` when the box was
    /// closed. With no one to answer (no UI, or a silent run) it logs the message and returns
    /// `None` at once; Stop ends the wait.
    pub fn show_message_box(
        &self,
        bot: &Bot,
        caption: &str,
        message: &str,
        buttons: Vec<String>,
    ) -> BotResult<Option<usize>> {
        if self.silent.load(Ordering::Acquire) || !self.interactive.load(Ordering::Acquire) {
            bot.log(format!(
                "[Message box, no one to answer] {caption}: {message}"
            ));
            return Ok(None);
        }
        let buttons = if buttons.is_empty() {
            vec!["OK".to_owned()]
        } else {
            buttons
        };
        let id = self.next_message_box.fetch_add(1, Ordering::Relaxed);
        let mut slot = self.message_box.lock();
        slot.open = Some(MessageBox {
            id,
            caption: caption.to_owned(),
            message: message.to_owned(),
            buttons,
        });
        loop {
            if let Some((answered, button)) = slot.answer
                && answered == id
            {
                slot.answer = None;
                return Ok(button);
            }
            if bot.control.should_exit() {
                slot.open = None;
                return Err(BotError::Stopped);
            }
            self.message_box_answered
                .wait_for(&mut slot, Duration::from_millis(100));
        }
    }

    /// Before the run (VibeSkua `StartScript`): shows the window unless the script set
    /// `dontPreconfigure`.
    fn preconfigure(&self, bot: &Bot) -> BotResult<()> {
        let needs_config = matches!(
            &*self.options.lock(),
            ScriptOptionsState::Loaded(o) if !o.declaration().dont_preconfigure
        );
        if needs_config {
            self.configure(bot)?;
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
/// runs nobody is there to answer it, such as the HTTP API, auto-relogin and the Scheduler.
pub fn start_silent(bot: &Arc<Bot>) -> Result<(), String> {
    start_with(bot, true)
}

fn start_with(bot: &Arc<Bot>, silent: bool) -> Result<(), String> {
    let Some(path) = bot.scripts.loaded() else {
        return Err("No script loaded".into());
    };
    // SeqCst with `run_snippet`'s swap: each sets its flag and then reads the other's.
    if bot.scripts.running.swap(true, Ordering::SeqCst) {
        return Err("A script is already running".into());
    }
    if bot.scripts.snippet.load(Ordering::SeqCst) {
        bot.scripts.running.store(false, Ordering::Release);
        return Err("A Console snippet is running".into());
    }
    bot.scripts.silent.store(silent, Ordering::Release);
    bot.scripts.unshown_noted.store(false, Ordering::Release);
    bot.scripts.reached_main.store(false, Ordering::Release);
    bot.control.reset();
    // VibeSkua's `Handlers.Clear()` before the script thread starts.
    bot.handlers.clear();
    bot.scripts.set_status(ScriptStatus::Compiling);
    // VibeSkua sends `ScriptStartedMessage` just before the thread starts, so a script that ends
    // at once can't log its stop first (Notification Center).
    bot.notifications.script_started(Some(&path));
    crate::quick_scripts::update(&bot.paths.quick_scripts_file(), |lists| {
        lists.note_started(&path)
    });
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
            bot.monitor.event(
                crate::monitor::EventKind::ScriptStarted,
                format!("Started {name}"),
            );
            bot.webhooks.send(Notice::ScriptStarted {
                script: script_stem(&path),
                quiet: false,
            });
            let run_started = std::time::Instant::now();
            bot.state.lock().stats.started = Some(run_started);
            let result = match bot.scripts.preconfigure(&bot) {
                Ok(()) => runtime::run(&bot, &path),
                Err(_) => Err(runtime::RunError::Stopped),
            };
            mark_script_thread(false);
            finish(&bot, &name, &path, run_started, result);
        })
        .map_err(|e| {
            bot.scripts.running.store(false, Ordering::Release);
            bot.notifications.script_stopped(None);
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

/// Diagnostics > Console: runs a TypeScript snippet on the calling thread with `bot` (and the
/// rest of the "turu" module) in scope. The thread counts as a script thread meanwhile, so
/// [`stop_snippet`] stops it. Returns the value of the snippet's last expression as JSON;
/// `on_log` gets each `bot.log` line, which also goes to the script log. Refused while a script
/// runs (and a script won't start while a snippet runs).
pub fn run_snippet(
    bot: &Arc<Bot>,
    source: &str,
    on_log: impl Fn(&str) + Send + Sync + 'static,
) -> Result<String, RunError> {
    if bot.scripts.snippet.swap(true, Ordering::SeqCst) {
        return Err(RunError::Failed("A snippet is already running".into()));
    }
    if bot.scripts.running.load(Ordering::SeqCst) {
        bot.scripts.snippet.store(false, Ordering::Release);
        return Err(RunError::Failed("Stop the script first".into()));
    }
    bot.control.reset();
    mark_script_thread(true);
    let result = transpile::transpile_snippet(source)
        .map_err(RunError::Failed)
        .and_then(|js| runtime::run_snippet(bot, &js, Arc::new(on_log)));
    mark_script_thread(false);
    // Its handlers ran in its JS, which is gone.
    bot.handlers.clear();
    bot.army.release_all();
    bot.control.reset();
    bot.scripts.snippet.store(false, Ordering::Release);
    result
}

/// Stops the running Console snippet: its next bot call (or any running JS) stops it.
pub fn stop_snippet(bot: &Bot) {
    if bot.scripts.is_snippet_running() {
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
    bot.scripts.reached_main.store(true, Ordering::Release);
    bot.scripts.set_status(ScriptStatus::Running);
}

/// The script's stopping handlers run ("Stopping...").
pub(crate) fn set_stopping(bot: &Bot) {
    bot.scripts.set_status(ScriptStatus::Stopping);
}

/// Cleanup after `main` returns, throws or is stopped (VibeSkua: ScriptStopping/Stopped).
fn finish(
    bot: &Arc<Bot>,
    name: &str,
    path: &Path,
    run_started: std::time::Instant,
    result: Result<(), runtime::RunError>,
) {
    let finished = result.is_ok();
    bot.skills.stop();
    // VibeSkua `ScriptCombat.ScriptStopped` (the counter handler itself stays on).
    bot.combat.set_stop_attacking(false);
    bot.drops.stop();
    bot.boosts.reset();
    bot.quests.unregister_all();
    bot.handlers.clear();
    bot.army.release_all();
    bot.options.write().reset_after_script();
    bot.state.lock().options_applied.restore_world();
    use crate::monitor::EventKind;
    let mut error = None;
    let status = match result {
        Ok(()) => {
            bot.log(format!("Script {name} finished"));
            bot.monitor
                .event(EventKind::ScriptStopped, format!("{name} finished"));
            ScriptStatus::Loaded
        }
        Err(runtime::RunError::Stopped) => {
            bot.log(format!("Script {name} stopped"));
            bot.monitor
                .event(EventKind::ScriptStopped, format!("{name} stopped"));
            ScriptStatus::Loaded
        }
        Err(runtime::RunError::Failed(message)) => {
            bot.log(format!("Script {name} failed:\n{message}"));
            let first = message.lines().next().unwrap_or_default();
            bot.notifications.script_error(first, Some(path));
            bot.monitor
                .event(EventKind::ScriptError, format!("{name} failed: {first}"));
            error = Some(message);
            ScriptStatus::Error
        }
    };
    bot.control.reset();
    bot.scripts.set_status(status);
    *bot.scripts.last_end.lock() = Some(RunEnd {
        script: path.to_owned(),
        started: bot.scripts.reached_main.load(Ordering::Acquire),
        finished,
        error: error.clone(),
        ran: run_started.elapsed(),
        ended: chrono::Local::now(),
    });
    bot.scripts.running.store(false, Ordering::Release);
    bot.scripts.ended.fetch_add(1, Ordering::AcqRel);
    bot.events.emit(crate::events::BotEvent::ScriptStopped);
    let script = bot
        .scripts
        .loaded()
        .map_or_else(String::new, |p| script_stem(&p));
    bot.webhooks.send(Notice::ScriptStopped {
        script,
        error,
        quiet: false,
    });
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
