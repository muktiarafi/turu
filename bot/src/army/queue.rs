//! Army Queue (VibeSkua Manager's "Army Queue" tab, `ArmyQueueViewModel`): squads of accounts
//! that run one after another. Each squad's accounts are launched (staggered) with the squad's
//! script, or an account's own, and `--exit-on-script-stop`; once every one of them has quit,
//! a short "ghost session" cooldown passes and the next squad starts. Pause holds the queue
//! after the running squad; Stop ends it.
//!
//! The queue being edited is saved in `ArmyQueueActive.json`, presets in `ArmyQueues.json`
//! (by name), with VibeSkua's field names (`ArmyQueueConfig`), so VibeSkua's files read as
//! they are.

use super::{Army, Launch};
use parking_lot::{Condvar, Mutex};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const PRESETS_FILE: &str = "ArmyQueues.json";
pub const ACTIVE_FILE: &str = "ArmyQueueActive.json";
/// The queue's name when it has none (VibeSkua's default `QueueName`).
pub const DEFAULT_NAME: &str = "Daily Ultras";

/// One account of a squad (VibeSkua `ArmyQueueAccount`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct QueueAccount {
    pub username: String,
    pub display_name: String,
    /// Its own script, over the squad's.
    pub script_path: Option<String>,
}

impl QueueAccount {
    pub fn new(username: &str, display_name: &str) -> Self {
        Self {
            username: username.to_owned(),
            display_name: display_name.to_owned(),
            script_path: None,
        }
    }

    /// VibeSkua `DisplayOrUsername`.
    pub fn label(&self) -> &str {
        if self.display_name.trim().is_empty() {
            &self.username
        } else {
            &self.display_name
        }
    }

    pub fn has_script(&self) -> bool {
        self.script_path
            .as_deref()
            .is_some_and(|p| !p.trim().is_empty())
    }

    /// VibeSkua `DisplayScriptName`: the file name, or "(Uses Squad Script)".
    pub fn script_label(&self) -> String {
        match self.script_path.as_deref().filter(|p| !p.trim().is_empty()) {
            Some(path) => file_name(path),
            None => "(Uses Squad Script)".into(),
        }
    }
}

/// A squad (VibeSkua `ArmyQueueStep`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct QueueStep {
    pub name: String,
    /// The script every account without its own runs.
    pub step_script_path: Option<String>,
    pub accounts: Vec<QueueAccount>,
}

impl Default for QueueStep {
    fn default() -> Self {
        Self::new("Squad 1")
    }
}

impl QueueStep {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            step_script_path: None,
            accounts: Vec::new(),
        }
    }

    pub fn has_script(&self) -> bool {
        self.step_script_path
            .as_deref()
            .is_some_and(|p| !p.trim().is_empty())
    }

    /// The script `account` runs: its own, else the squad's ("" for none), and the Script
    /// Options storage it runs with (`None` without a script).
    pub fn script_for(&self, account: &QueueAccount) -> (String, Option<String>) {
        if account.has_script() {
            let script = account.script_path.clone().unwrap_or_default();
            let storage = member_storage(&self.name, &account.username, &script);
            return (script, Some(storage));
        }
        match self
            .step_script_path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
        {
            Some(script) => (script.to_owned(), Some(step_storage(&self.name, script))),
            None => (String::new(), None),
        }
    }
}

/// A queue (VibeSkua `ArmyQueueConfig`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct QueueConfig {
    pub name: String,
    pub steps: Vec<QueueStep>,
    pub delay_between_steps_seconds: u64,
    pub delay_between_launches_seconds: u64,
    pub timeout_minutes: u64,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            name: DEFAULT_NAME.into(),
            steps: vec![QueueStep::default()],
            delay_between_steps_seconds: 5,
            delay_between_launches_seconds: 3,
            timeout_minutes: 30,
        }
    }
}

impl QueueConfig {
    /// The queue being edited, from `ArmyQueueActive.json`; one empty "Squad 1" when there is
    /// none or it has no squads (VibeSkua `LoadActiveQueueOrDefaults`).
    pub fn load_active(path: &Path) -> Self {
        let read: Option<Self> = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok());
        match read {
            Some(mut config) if !config.steps.is_empty() => {
                if config.name.trim().is_empty() {
                    config.name = DEFAULT_NAME.into();
                }
                config.fix_delays();
                config
            }
            _ => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_json(path, self)
    }

    /// VibeSkua's load: delays of 0 or less take the defaults.
    fn fix_delays(&mut self) {
        if self.delay_between_steps_seconds == 0 {
            self.delay_between_steps_seconds = 5;
        }
        if self.delay_between_launches_seconds == 0 {
            self.delay_between_launches_seconds = 3;
        }
        if self.timeout_minutes == 0 {
            self.timeout_minutes = 30;
        }
    }

    /// "+ Squad" (VibeSkua `AddStep`): "Squad n". Returns its index.
    pub fn add_step(&mut self) -> usize {
        let name = format!("Squad {}", self.steps.len() + 1);
        self.steps.push(QueueStep::new(&name));
        self.steps.len() - 1
    }

    /// The squad's ✕ (VibeSkua `RemoveStepSpecific`): the last squad is emptied and renamed
    /// "Squad 1" instead. Returns the squad to select next.
    pub fn remove_step(&mut self, index: usize, selected: usize) -> usize {
        if index >= self.steps.len() {
            return selected.min(self.steps.len().saturating_sub(1));
        }
        if self.steps.len() <= 1 {
            self.steps[0] = QueueStep::default();
            return 0;
        }
        self.steps.remove(index);
        if selected == index {
            index.min(self.steps.len() - 1)
        } else if selected > index {
            selected - 1
        } else {
            selected
        }
    }

    /// A squad dragged onto another (VibeSkua `ReorderStep`): it takes the target's place.
    /// Returns the moved squad's new index.
    pub fn move_step(&mut self, from: usize, to: usize) -> usize {
        if from >= self.steps.len() || to >= self.steps.len() || from == to {
            return from;
        }
        let step = self.steps.remove(from);
        self.steps.insert(to, step);
        to
    }

    /// The account picker's Done (VibeSkua `ConfirmAccountPicker`): unchecked accounts leave
    /// the squad, newly checked ones join at the end (`picked`: username and label).
    pub fn set_members(&mut self, step: usize, picked: &[(String, String)]) {
        let Some(step) = self.steps.get_mut(step) else {
            return;
        };
        step.accounts.retain(|a| {
            picked
                .iter()
                .any(|(u, _)| u.eq_ignore_ascii_case(&a.username))
        });
        for (username, label) in picked {
            if !step
                .accounts
                .iter()
                .any(|a| a.username.eq_ignore_ascii_case(username))
            {
                step.accounts.push(QueueAccount::new(username, label));
            }
        }
    }

    /// Whether Start has anything to run (VibeSkua: "Please add at least one squad with
    /// accounts to the queue.").
    pub fn has_accounts(&self) -> bool {
        self.steps.iter().any(|s| !s.accounts.is_empty())
    }
}

/// `ArmyQueues.json`: the saved presets, by name (VibeSkua's `Dictionary<string,
/// ArmyQueueConfig>`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presets {
    pub queues: BTreeMap<String, QueueConfig>,
}

impl Presets {
    /// The presets in `path`; none when it can't be read.
    pub fn load(path: &Path) -> Self {
        let queues = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self { queues }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_json(path, &self.queues)
    }

    pub fn names(&self) -> Vec<String> {
        self.queues.keys().cloned().collect()
    }

    /// The preset with that name, any case.
    pub fn get(&self, name: &str) -> Option<&QueueConfig> {
        self.queues
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name.trim()))
            .map(|(_, c)| c)
    }

    /// "Save Preset": saved under its trimmed name, replacing one with that name (any case).
    /// Returns the name, or why it can't be saved.
    pub fn put(&mut self, config: &QueueConfig) -> Result<String, String> {
        let name = config.name.trim().to_owned();
        if name.is_empty() {
            return Err("Please enter a name for this Army Queue preset.".into());
        }
        self.queues.retain(|n, _| !n.eq_ignore_ascii_case(&name));
        let mut config = config.clone();
        config.name = name.clone();
        self.queues.insert(name.clone(), config);
        Ok(name)
    }

    /// A preset to edit (VibeSkua `LoadSelectedPreset`): its delays of 0 take the defaults.
    pub fn load_preset(&self, name: &str) -> Option<QueueConfig> {
        let mut config = self.get(name)?.clone();
        config.fix_delays();
        if config.steps.is_empty() {
            config.steps.push(QueueStep::default());
        }
        Some(config)
    }

    pub fn remove(&mut self, name: &str) {
        self.queues
            .retain(|n, _| !n.eq_ignore_ascii_case(name.trim()));
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    std::fs::write(path, text)
}

/// VibeSkua's `[^\w\.-]` replaced by `_`.
fn safe(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '.' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

/// The Script Options storage of a squad's script (VibeSkua `GetStepStorageName`):
/// `ArmyQueue_<squad>_<script>`.
pub fn step_storage(squad: &str, script: &str) -> String {
    let script = if script.trim().is_empty() {
        "Script".to_owned()
    } else {
        file_stem(script)
    };
    format!("ArmyQueue_{}_{script}", safe(squad))
}

/// The Script Options storage of an account's own script (VibeSkua `GetMemberStorageName`):
/// `ArmyQueue_<squad>_<username>_<script>`.
pub fn member_storage(squad: &str, username: &str, script: &str) -> String {
    let script = if script.trim().is_empty() {
        "Script".to_owned()
    } else {
        file_stem(script)
    };
    format!("ArmyQueue_{}_{}_{script}", safe(squad), safe(username))
}

/// An account's state in a run (VibeSkua `RuntimeStatus`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MemberStatus {
    #[default]
    Idle,
    Waiting,
    AccountMissing,
    Launching,
    Running,
    Finished,
    LaunchFailed,
}

impl MemberStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Waiting => "Waiting",
            Self::AccountMissing => "Account Missing",
            Self::Launching => "Launching...",
            Self::Running => "Running",
            Self::Finished => "Finished",
            Self::LaunchFailed => "Launch Failed",
        }
    }
}

/// A squad's state in a run (VibeSkua `StepStatus`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StepStatus {
    #[default]
    Pending,
    Running,
    Completed,
}

impl StepStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Running => "Running",
            Self::Completed => "Completed",
        }
    }
}

/// How the run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Every squad ran: VibeSkua's "Army Queue Finished" box.
    Finished,
    Stopped,
}

/// What the window shows of a run.
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub running: bool,
    pub paused: bool,
    pub text: String,
    /// The running squad, from 1 (VibeSkua `ActiveStepIndex`).
    pub active_step: usize,
    /// 0 to 100.
    pub progress: f64,
    /// By squad, as the queue was when it started.
    pub steps: Vec<StepStatus>,
    /// By squad, then account.
    pub members: Vec<Vec<MemberStatus>>,
    /// The last run's end, until [`QueueRunner::take_outcome`].
    pub outcome: Option<Outcome>,
}

impl Default for QueueStatus {
    fn default() -> Self {
        Self {
            running: false,
            paused: false,
            text: "Ready to start queue.".into(),
            active_step: 0,
            progress: 0.0,
            steps: Vec::new(),
            members: Vec::new(),
            outcome: None,
        }
    }
}

impl QueueStatus {
    /// VibeSkua `CanPause`.
    pub fn can_pause(&self) -> bool {
        self.running && !self.paused
    }

    /// VibeSkua `CanResume`.
    pub fn can_resume(&self) -> bool {
        self.running && self.paused
    }

    /// An account's state in the run, `Idle` outside one.
    pub fn member(&self, step: usize, account: usize) -> MemberStatus {
        self.members
            .get(step)
            .and_then(|m| m.get(account))
            .copied()
            .unwrap_or_default()
    }
}

/// What launching an account came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launched {
    Started,
    /// It isn't in Army > Accounts.
    Missing,
    Failed(String),
}

/// Starts the queue's clients and tells when they quit (the real one: [`ArmyLauncher`]).
pub trait QueueLauncher: Send + Sync {
    /// Launches `username` running `script` ("" for none) with its Script Options from
    /// `storage`, to quit once the script stops.
    fn launch(&self, username: &str, script: &str, storage: Option<&str>) -> Launched;
    /// The client launched for `username` has quit.
    fn exited(&self, username: &str) -> bool;
}

/// Launches through [`Army`], as Army > Accounts does: the account's saved password, the
/// Accounts launch server (VibeSkua's `LastServer`), a window of its own (or none when
/// Accounts launches headless).
pub struct ArmyLauncher {
    pub army: Arc<Army>,
    pub accounts_file: PathBuf,
}

impl QueueLauncher for ArmyLauncher {
    fn launch(&self, username: &str, script: &str, storage: Option<&str>) -> Launched {
        let accounts = crate::accounts::Accounts::load(&self.accounts_file);
        let Some(account) = accounts.find(username) else {
            return Launched::Missing;
        };
        let password = match crate::accounts::password(&account.username) {
            Ok(password) => password,
            Err(e) => return Launched::Failed(e),
        };
        let script = script.trim();
        let launch = Launch {
            username: account.username.clone(),
            label: account.label().to_owned(),
            password,
            server: Some(accounts.server.trim().to_owned()).filter(|s| !s.is_empty()),
            script: (!script.is_empty()).then(|| PathBuf::from(script)),
            run_script: !script.is_empty(),
            headless: accounts.mode == crate::accounts::LaunchMode::Headless,
            options_storage: storage.map(str::to_owned),
            exit_on_stop: true,
        };
        match self.army.launch(launch) {
            Ok(_) => Launched::Started,
            Err(e) => Launched::Failed(e),
        }
    }

    fn exited(&self, username: &str) -> bool {
        self.army
            .clients()
            .iter()
            .find(|c| c.username.eq_ignore_ascii_case(username))
            .is_none_or(|c| c.has_exited())
    }
}

/// The run was stopped.
struct Stopped;

/// Runs a queue on its own thread and holds its state (VibeSkua `StartQueue`, `PauseQueue`,
/// `ResumeQueue`, `StopQueue`).
pub struct QueueRunner {
    status: Mutex<QueueStatus>,
    wake: Condvar,
    stop: AtomicBool,
    /// VibeSkua's one second: the poll and countdown step (tests shorten it).
    second: Duration,
}

impl QueueRunner {
    pub fn new() -> Arc<Self> {
        Self::with_second(Duration::from_secs(1))
    }

    /// A runner whose "seconds" last `second` (tests).
    pub fn with_second(second: Duration) -> Arc<Self> {
        Arc::new(Self {
            status: Mutex::default(),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            second,
        })
    }

    pub fn status(&self) -> QueueStatus {
        self.status.lock().clone()
    }

    pub fn is_running(&self) -> bool {
        self.status.lock().running
    }

    /// How the last run ended, once.
    pub fn take_outcome(&self) -> Option<Outcome> {
        self.status.lock().outcome.take()
    }

    /// "Start Queue": runs `config` on a "Turu Army Queue" thread.
    pub fn start(
        self: &Arc<Self>,
        config: QueueConfig,
        launcher: Arc<dyn QueueLauncher>,
    ) -> Result<(), String> {
        self.begin(&config)?;
        let runner = self.clone();
        std::thread::Builder::new()
            .name("Turu Army Queue".into())
            .spawn(move || runner.run_begun(&config, launcher.as_ref()))
            .map(|_| ())
            .map_err(|e| {
                self.end("Queue Error: couldn't start".into(), None);
                e.to_string()
            })
    }

    /// Runs `config` on this thread until it ends (tests; [`Self::start`] runs it on its own).
    pub fn run(&self, config: &QueueConfig, launcher: &dyn QueueLauncher) -> Result<(), String> {
        self.begin(config)?;
        self.run_begun(config, launcher);
        Ok(())
    }

    fn begin(&self, config: &QueueConfig) -> Result<(), String> {
        if !config.has_accounts() {
            return Err("Please add at least one squad with accounts to the queue.".into());
        }
        let mut status = self.status.lock();
        if status.running {
            return Err("The queue is running already.".into());
        }
        self.stop.store(false, Ordering::SeqCst);
        *status = QueueStatus {
            running: true,
            text: status.text.clone(),
            steps: vec![StepStatus::Pending; config.steps.len()],
            members: config
                .steps
                .iter()
                .map(|s| vec![MemberStatus::Waiting; s.accounts.len()])
                .collect(),
            ..QueueStatus::default()
        };
        Ok(())
    }

    /// "Pause Queue": the running squad finishes; the next one waits for Resume.
    pub fn pause(&self) {
        let mut status = self.status.lock();
        if !status.can_pause() {
            return;
        }
        status.paused = true;
        status.text =
            "Queue PAUSED. Current squad will finish, but won't start next squad until resumed."
                .into();
    }

    /// "Resume Queue".
    pub fn resume(&self) {
        let mut status = self.status.lock();
        if !status.can_resume() {
            return;
        }
        status.paused = false;
        status.text = "Queue resumed. Proceeding...".into();
        self.wake.notify_all();
    }

    /// "Stop Queue" (after the window's "Are you sure"): the clients it launched keep running,
    /// as in VibeSkua.
    pub fn stop(&self) {
        let mut status = self.status.lock();
        if !status.running {
            return;
        }
        self.stop.store(true, Ordering::SeqCst);
        status.paused = false;
        status.text = "Stopping Army Queue...".into();
        self.wake.notify_all();
    }

    fn set(&self, f: impl FnOnce(&mut QueueStatus)) {
        f(&mut self.status.lock());
    }

    fn check(&self) -> Result<(), Stopped> {
        if self.stop.load(Ordering::SeqCst) {
            Err(Stopped)
        } else {
            Ok(())
        }
    }

    /// Waits `seconds` of the runner's seconds, or until Stop.
    fn sleep(&self, seconds: u64) -> Result<(), Stopped> {
        let until = Instant::now() + self.second * seconds as u32;
        let mut status = self.status.lock();
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return Err(Stopped);
            }
            let now = Instant::now();
            if now >= until {
                return Ok(());
            }
            self.wake.wait_for(&mut status, until - now);
        }
    }

    /// While paused, waits for Resume, saying so; Stop ends the wait.
    fn wait_while_paused(&self, finished: &str, next: &str) -> Result<(), Stopped> {
        let mut status = self.status.lock();
        while status.paused {
            if self.stop.load(Ordering::SeqCst) {
                return Err(Stopped);
            }
            status.text = format!(
                "Squad '{finished}' completed. Queue is PAUSED. Click 'Resume Queue' to start {next}."
            );
            self.wake.wait_for(&mut status, self.second);
        }
        drop(status);
        self.check()
    }

    fn end(&self, text: String, outcome: Option<Outcome>) {
        let mut status = self.status.lock();
        status.running = false;
        status.paused = false;
        status.text = text;
        status.outcome = outcome;
        if outcome == Some(Outcome::Finished) {
            status.progress = 100.0;
        }
    }

    fn run_begun(&self, config: &QueueConfig, launcher: &dyn QueueLauncher) {
        match self.run_steps(config, launcher) {
            Ok(()) => self.end(
                "All Army Queue squads completed successfully!".into(),
                Some(Outcome::Finished),
            ),
            Err(Stopped) => self.end("Army Queue was stopped.".into(), Some(Outcome::Stopped)),
        }
    }

    /// VibeSkua `RunQueueAsync`.
    fn run_steps(&self, config: &QueueConfig, launcher: &dyn QueueLauncher) -> Result<(), Stopped> {
        let total = config.steps.len();
        for (i, step) in config.steps.iter().enumerate() {
            self.check()?;
            self.set(|s| {
                s.active_step = i + 1;
                s.steps[i] = StepStatus::Running;
                s.text = format!(
                    "Running Squad {}/{total}: '{}' ({} accounts)...",
                    i + 1,
                    step.name,
                    step.accounts.len()
                );
            });
            // 1. Launch the squad, staggered.
            let mut launched: Vec<usize> = Vec::new();
            let last = step.accounts.len().saturating_sub(1);
            for (j, account) in step.accounts.iter().enumerate() {
                self.check()?;
                self.set(|s| s.members[i][j] = MemberStatus::Launching);
                let (script, storage) = step.script_for(account);
                let state = match launcher.launch(&account.username, &script, storage.as_deref()) {
                    Launched::Started => {
                        launched.push(j);
                        MemberStatus::Running
                    }
                    Launched::Missing => {
                        // VibeSkua skips the account, and its launch delay with it.
                        self.set(|s| s.members[i][j] = MemberStatus::AccountMissing);
                        continue;
                    }
                    Launched::Failed(_) => MemberStatus::LaunchFailed,
                };
                self.set(|s| s.members[i][j] = state);
                if config.delay_between_launches_seconds > 0 && j != last {
                    self.sleep(config.delay_between_launches_seconds)?;
                }
            }
            // 2. Wait until every launched account has quit (no time limit).
            let started = Instant::now();
            loop {
                let exited: Vec<bool> = launched
                    .iter()
                    .map(|&j| launcher.exited(&step.accounts[j].username))
                    .collect();
                if exited.iter().all(|&e| e) {
                    break;
                }
                self.check()?;
                let done = exited.iter().filter(|&&e| e).count();
                let count = launched.len();
                self.set(|s| {
                    for (&j, &e) in launched.iter().zip(&exited) {
                        if e && s.members[i][j] == MemberStatus::Running {
                            s.members[i][j] = MemberStatus::Finished;
                        }
                    }
                    s.text = format!(
                        "Squad {}/{total} '{}': {done}/{count} accounts finished (Runtime: {})",
                        i + 1,
                        step.name,
                        runtime(started.elapsed())
                    );
                    s.progress = (i as f64 * 100.0) / total as f64
                        + (done as f64 * 100.0) / (total as f64 * count.max(1) as f64);
                });
                self.sleep(1)?;
            }
            self.set(|s| {
                s.steps[i] = StepStatus::Completed;
                for m in &mut s.members[i] {
                    if *m == MemberStatus::Running {
                        *m = MemberStatus::Finished;
                    }
                }
                s.progress = ((i + 1) as f64 * 100.0) / total as f64;
            });
            // 3. Paused: wait. Then the ghost session cooldown before the next squad.
            if i + 1 < total {
                let next = &config.steps[i + 1].name;
                self.wait_while_paused(&step.name, next)?;
                let cooldown = config.delay_between_steps_seconds.max(1);
                for left in (1..=cooldown).rev() {
                    self.check()?;
                    self.wait_while_paused(&step.name, next)?;
                    self.set(|s| {
                        s.text = format!(
                            "Squad '{}' completed! Waiting {left}s ghost session cooldown before next squad...",
                            step.name
                        );
                    });
                    self.sleep(1)?;
                }
            }
        }
        Ok(())
    }
}

/// "1h 05m", or "3m 07s".
fn runtime(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs >= 3600 {
        format!("{}h {:02}m", secs / 3600, (secs / 60) % 60)
    } else {
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storages_follow_vibeskua() {
        assert_eq!(
            step_storage("Squad 1", "C:/Scripts/Farm/Ultra Dage.ts"),
            "ArmyQueue_Squad_1_Ultra Dage"
        );
        assert_eq!(
            member_storage("Daily/Ultras", "my.alt-1", "LoO.ts"),
            "ArmyQueue_Daily_Ultras_my.alt-1_LoO"
        );
        assert_eq!(step_storage("Squad", ""), "ArmyQueue_Squad_Script");
    }

    #[test]
    fn formats_runtimes() {
        assert_eq!(runtime(Duration::from_secs(187)), "3m 07s");
        assert_eq!(runtime(Duration::from_secs(3900)), "1h 05m");
    }
}
