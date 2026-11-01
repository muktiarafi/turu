//! Scripts > Scheduler (VibeSkua `ScriptSchedulerViewModel`): a playlist of scripts run top to
//! bottom, one after another. Each entry keeps its own script options
//! (`options/Queue_<name>_<id>.cfg`, VibeSkua `OverrideStorage`) and starts without the Script
//! Options window (`SilentConfig`).
//!
//! A script that ends in any way (it finished, threw, or you pressed Stop Script) completes its
//! entry and the next one starts 2 s later; only Stop queue stops the playlist. A script that
//! stopped while logged out (auto-relogin stops it) is "Disconnected": the playlist waits for
//! the login and then resumes it, or carries on with the script auto-relogin restarted.
//!
//! The playlist lives in memory, one per tab (`Bot::scheduler`); Save and Load write and read
//! VibeSkua's playlist files (`[{"Path", "Id", "Name"}]`).

use crate::Bot;
use crate::script::{self, RunEnd};
use crate::webhooks::Notice;
use parking_lot::{Condvar, Mutex};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

const GREEN: u32 = 0x00FF00;
const RED: u32 = 0xFF0000;
const GOLD: u32 = 0xFFD700;

/// What a playlist entry is doing (VibeSkua's status strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStatus {
    Queued,
    Running,
    Completed,
    /// Stop queue stopped it.
    Stopped,
    /// It couldn't start (or threw before reaching `main`).
    Failed,
    FileNotFound,
    /// It stopped while logged out; the playlist resumes it after the login.
    Disconnected,
}

impl EntryStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Stopped => "Stopped",
            Self::Failed => "Failed",
            Self::FileNotFound => "File Not Found",
            Self::Disconnected => "Disconnected",
        }
    }

    /// Shown in red.
    pub fn is_problem(self) -> bool {
        matches!(self, Self::Failed | Self::FileNotFound | Self::Disconnected)
    }
}

/// One script in the playlist (VibeSkua `ScriptItemViewModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// A GUID: names the entry's options file.
    pub id: String,
    /// The file name at first ("Farm.ts", "Farm (2).ts" for the second copy); renamable.
    pub name: String,
    pub path: PathBuf,
    pub status: EntryStatus,
    /// How long its last run took, once it ended.
    pub duration: Option<Duration>,
}

impl Entry {
    fn new(path: &Path, id: String) -> Self {
        Self {
            id,
            name: path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            path: path.to_owned(),
            status: EntryStatus::Queued,
            duration: None,
        }
    }

    /// The storage its options are saved in: `Queue_<file stem>_<id>`.
    pub fn storage(&self) -> String {
        let stem = self
            .path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        format!("Queue_{stem}_{}", self.id)
    }

    /// The TIME column: "00:00" until a run ended, then "hh:mm:ss".
    pub fn duration_text(&self) -> String {
        self.duration.map_or_else(|| "00:00".into(), clock)
    }
}

/// The waits between scripts (VibeSkua's); the tests shorten them.
#[derive(Debug, Clone, Copy)]
pub struct Delays {
    /// After a script ended, before the next one.
    pub after_stop: Duration,
    /// After a script failed to start.
    pub after_failure: Duration,
    /// After a script whose file is gone.
    pub after_missing: Duration,
    /// How often to check for the login while logged out, and how long to wait after it.
    pub login_poll: Duration,
    pub after_login: Duration,
    /// After stopping a script that isn't the playlist's, before starting the playlist's.
    pub after_lingering: Duration,
}

impl Default for Delays {
    fn default() -> Self {
        Self {
            after_stop: Duration::from_millis(2000),
            after_failure: Duration::from_millis(3000),
            after_missing: Duration::from_millis(1500),
            login_poll: Duration::from_millis(1500),
            after_login: Duration::from_millis(3000),
            after_lingering: Duration::from_millis(1500),
        }
    }
}

#[derive(Default)]
struct Queue {
    entries: Vec<Entry>,
    running: bool,
    /// "HH:MM:SS", while it runs.
    started_at: String,
    current: usize,
    /// When the current entry's run started.
    run_started: Option<Instant>,
    /// Bumped by every start, so a stopped run's thread can't continue a new one.
    generation: u64,
}

#[derive(Default)]
pub struct Scheduler {
    queue: Mutex<Queue>,
    wake: Condvar,
    delays: Mutex<Delays>,
}

/// One entry of a saved playlist (VibeSkua `SavedScriptItem`).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct SavedScriptItem {
    path: String,
    id: String,
    name: String,
}

impl Default for SavedScriptItem {
    fn default() -> Self {
        Self {
            path: String::new(),
            id: "00000000-0000-0000-0000-000000000000".into(),
            name: String::new(),
        }
    }
}

const RUNNING_ADD: &str = "Cannot add scripts while the playlist is running.";
const RUNNING_MOVE: &str = "Cannot reorder scripts while the playlist is running.";
const RUNNING_REMOVE: &str = "Cannot remove scripts while the playlist is running.";
const RUNNING_LOAD: &str = "Cannot load a playlist while the queue is running.";

impl Scheduler {
    pub fn entries(&self) -> Vec<Entry> {
        self.queue.lock().entries.clone()
    }

    pub fn is_running(&self) -> bool {
        self.queue.lock().running
    }

    /// The entry running now (or waiting to), while the playlist runs.
    pub fn current(&self) -> Option<usize> {
        let queue = self.queue.lock();
        (queue.running && queue.current < queue.entries.len()).then_some(queue.current)
    }

    /// "Started at 22:10:05 · running 2 of 4" while it runs, else "4 scripts".
    pub fn summary(&self) -> String {
        let queue = self.queue.lock();
        let count = queue.entries.len();
        if queue.running {
            format!(
                "Started at {} · running {} of {count}",
                queue.started_at,
                (queue.current + 1).min(count)
            )
        } else if count == 1 {
            "1 script".into()
        } else {
            format!("{count} scripts")
        }
    }

    pub fn set_delays(&self, delays: Delays) {
        *self.delays.lock() = delays;
    }

    /// Search Scripts' Queue (VibeSkua `OnQueueScript`): adds a downloaded script at the end.
    pub fn add(&self, path: &Path) -> Result<(), String> {
        let mut queue = self.queue.lock();
        if queue.running {
            return Err(RUNNING_ADD.into());
        }
        let mut entry = Entry::new(path, new_id());
        let copies = queue.entries.iter().filter(|e| e.path == path).count();
        if copies > 0 {
            let stem = path
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            let ext = path
                .extension()
                .map_or_else(String::new, |e| format!(".{}", e.to_string_lossy()));
            entry.name = format!("{stem} ({}){ext}", copies + 1);
        }
        queue.entries.push(entry);
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut queue = self.queue.lock();
        if queue.running {
            return Err(RUNNING_REMOVE.into());
        }
        queue.entries.retain(|e| e.id != id);
        Ok(())
    }

    /// Moves an entry one place up (`-1`) or down (`1`).
    pub fn move_by(&self, id: &str, by: isize) -> Result<(), String> {
        let mut queue = self.queue.lock();
        if queue.running {
            return Err(RUNNING_MOVE.into());
        }
        if let Some(i) = queue.entries.iter().position(|e| e.id == id) {
            let to = i as isize + by;
            if (0..queue.entries.len() as isize).contains(&to) {
                queue.entries.swap(i, to as usize);
            }
        }
        Ok(())
    }

    /// Renames an entry; a blank name keeps the old one. Allowed while it runs, as VibeSkua.
    pub fn rename(&self, id: &str, name: &str) {
        if name.trim().is_empty() {
            return;
        }
        if let Some(entry) = self.queue.lock().entries.iter_mut().find(|e| e.id == id) {
            entry.name = name.to_owned();
        }
    }

    /// Save (VibeSkua `SavePlaylist`): the entries as indented JSON.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let items: Vec<SavedScriptItem> = self
            .queue
            .lock()
            .entries
            .iter()
            .map(|e| SavedScriptItem {
                path: e.path.to_string_lossy().into_owned(),
                id: e.id.clone(),
                name: e.name.clone(),
            })
            .collect();
        if items.is_empty() {
            return Err("The playlist is empty.".into());
        }
        let text = serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }

    /// Load (VibeSkua `LoadPlaylist`): replaces the playlist, skipping scripts whose file is
    /// gone. Returns how many were loaded.
    pub fn load(&self, path: &Path) -> Result<usize, String> {
        if self.is_running() {
            return Err(RUNNING_LOAD.into());
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let items: Vec<SavedScriptItem> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let entries: Vec<Entry> = items
            .into_iter()
            .filter(|item| !item.path.is_empty() && Path::new(&item.path).is_file())
            .map(|item| {
                let mut entry = Entry::new(Path::new(&item.path), item.id);
                if !item.name.is_empty() {
                    entry.name = item.name;
                }
                entry
            })
            .collect();
        let mut queue = self.queue.lock();
        if queue.running {
            return Err(RUNNING_LOAD.into());
        }
        let count = entries.len();
        queue.entries = entries;
        Ok(count)
    }

    /// Start queue: runs the playlist from the top on a thread of its own.
    pub fn start(&self, bot: &Arc<Bot>) -> Result<(), String> {
        let generation = {
            let mut queue = self.queue.lock();
            if queue.running {
                return Err("The playlist is already running.".into());
            }
            if queue.entries.is_empty() {
                return Err("The playlist is empty.".into());
            }
            queue.running = true;
            queue.started_at = chrono::Local::now().format("%H:%M:%S").to_string();
            queue.generation += 1;
            queue.current = 0;
            queue.run_started = None;
            for entry in &mut queue.entries {
                entry.status = EntryStatus::Queued;
            }
            queue.generation
        };
        bot.webhooks.set_quiet(true);
        let weak = Arc::downgrade(bot);
        let spawned = std::thread::Builder::new()
            .name("Turu Scheduler".into())
            .spawn(move || run(weak, generation));
        if let Err(e) = spawned {
            self.queue.lock().running = false;
            bot.webhooks.set_quiet(false);
            return Err(e.to_string());
        }
        Ok(())
    }

    /// Stop queue (VibeSkua `StopQueue`): the current entry is "Stopped" and the running script
    /// is stopped.
    pub fn stop(&self, bot: &Bot) {
        {
            let mut queue = self.queue.lock();
            if !queue.running {
                return;
            }
            queue.running = false;
            queue.started_at.clear();
            let elapsed = queue.run_started.map(|s| s.elapsed());
            let current = queue.current;
            if let Some(entry) = queue.entries.get_mut(current) {
                entry.status = EntryStatus::Stopped;
                entry.duration = elapsed.or(Some(Duration::ZERO));
            }
            self.wake.notify_all();
        }
        bot.webhooks.set_quiet(false);
        script::stop(bot);
        embed(
            bot,
            "Scheduler Stopped",
            "**Scheduler** playlist execution manually halted.",
            RED,
            vec![],
        );
    }

    fn alive(queue: &Queue, generation: u64) -> bool {
        queue.running && queue.generation == generation
    }

    /// Waits `time`; false when the run was stopped meanwhile.
    fn wait(&self, generation: u64, time: Duration) -> bool {
        let deadline = Instant::now() + time;
        let mut queue = self.queue.lock();
        while Self::alive(&queue, generation) {
            if self.wake.wait_until(&mut queue, deadline).timed_out() {
                return Self::alive(&queue, generation);
            }
        }
        false
    }

    /// Changes the current entry while the run is alive; false when it was stopped.
    fn update(&self, generation: u64, f: impl FnOnce(&mut Queue)) -> bool {
        let mut queue = self.queue.lock();
        if !Self::alive(&queue, generation) {
            return false;
        }
        f(&mut queue);
        true
    }
}

/// The playlist's thread (VibeSkua `RunNextScript` and `OnScriptStopped`).
fn run(weak: Weak<Bot>, generation: u64) {
    loop {
        let Some(bot) = weak.upgrade() else { return };
        if !next(&bot, generation) {
            return;
        }
    }
}

/// Runs the current entry until it ends. False when the playlist is done or was stopped.
fn next(bot: &Arc<Bot>, generation: u64) -> bool {
    let scheduler = &bot.scheduler;
    let delays = *scheduler.delays.lock();
    if !logged_in(bot) {
        embed(
            bot,
            "Scheduler Paused",
            "**Scheduler** is waiting for account to reconnect before running next script...",
            GOLD,
            vec![],
        );
        while !logged_in(bot) {
            if !scheduler.wait(generation, delays.login_poll) {
                return false;
            }
        }
        if !scheduler.wait(generation, delays.after_login) {
            return false;
        }
    }

    let (index, entry, total) = {
        let mut queue = scheduler.queue.lock();
        if !Scheduler::alive(&queue, generation) {
            return false;
        }
        if queue.current >= queue.entries.len() {
            queue.running = false;
            queue.started_at.clear();
            drop(queue);
            bot.webhooks.set_quiet(false);
            embed(
                bot,
                "Scheduler Finished",
                "All scripts in the **Scheduler** playlist have completed!",
                GREEN,
                vec![],
            );
            return false;
        }
        (
            queue.current,
            queue.entries[queue.current].clone(),
            queue.entries.len(),
        )
    };
    let place = format!("[{}/{total}]", index + 1);

    if !entry.path.is_file() {
        let alive = scheduler.update(generation, |q| {
            q.entries[index].status = EntryStatus::FileNotFound;
            q.current += 1;
        });
        return alive && scheduler.wait(generation, delays.after_missing);
    }

    let mut resumed = false;
    if bot.scripts.is_running() {
        if bot
            .scripts
            .loaded()
            .is_some_and(|p| same_file(&p, &entry.path))
        {
            // Auto-relogin restarted it after a disconnection.
            resumed = true;
            if !scheduler.update(generation, |q| {
                q.entries[index].status = EntryStatus::Running;
                q.run_started = Some(Instant::now());
            }) {
                return false;
            }
            embed(
                bot,
                &format!("Script Resumed {place}"),
                &format!(
                    "**{{bot}}** has resumed execution of **{}** after reconnection.",
                    entry.name
                ),
                GREEN,
                vec![],
            );
        } else {
            tracing::info!(
                "Stopping lingering script before launching scheduled script ({})...",
                entry.name
            );
            script::stop(bot);
            while bot.scripts.is_running() {
                if !scheduler.wait(generation, Duration::from_millis(50)) {
                    return false;
                }
            }
            if !scheduler.wait(generation, delays.after_lingering) {
                return false;
            }
        }
    }

    let (before, _) = bot.scripts.last_end();
    if !resumed {
        if !scheduler.update(generation, |q| {
            q.entries[index].status = EntryStatus::Running;
            q.run_started = Some(Instant::now());
        }) {
            return false;
        }
        embed(
            bot,
            &format!("Script Started {place}"),
            &format!("**{{bot}}** has begun execution of **{}**.", entry.name),
            GREEN,
            vec![],
        );
        bot.scripts
            .load_with_storage(&entry.path, Some(&entry.storage()));
        if let Err(e) = script::start_silent(bot) {
            return failed(bot, generation, index, &entry, &place, &e, delays);
        }
    }

    // OnScriptStopped: whatever ended it.
    let end = loop {
        let (ended, end) = bot.scripts.last_end();
        if ended > before && !bot.scripts.is_running() {
            break end;
        }
        if !scheduler.wait(generation, Duration::from_millis(100)) {
            return false;
        }
    };
    if !logged_in(bot) {
        return scheduler.update(generation, |q| {
            q.entries[index].status = EntryStatus::Disconnected;
        }) && scheduler.wait(generation, delays.after_stop);
    }
    if let Some(RunEnd {
        started: false,
        error: Some(error),
    }) = end
    {
        // VibeSkua compiles in StartScript: a script that fails before `main` failed to start.
        return failed(bot, generation, index, &entry, &place, &error, delays);
    }
    let mut duration = Duration::ZERO;
    if !scheduler.update(generation, |q| {
        duration = q.run_started.map_or(Duration::ZERO, |s| s.elapsed());
        let entry = &mut q.entries[index];
        entry.status = EntryStatus::Completed;
        entry.duration = Some(duration);
        q.current += 1;
    }) {
        return false;
    }
    embed(
        bot,
        &format!("Farming Session Concluded {place}"),
        &format!("**{{bot}}** has finished execution of **{}**.", entry.name),
        RED,
        vec![("Duration".into(), clock(duration), true)],
    );
    scheduler.wait(generation, delays.after_stop)
}

/// The entry couldn't start: "Failed", then the next one after 3 s.
fn failed(
    bot: &Bot,
    generation: u64,
    index: usize,
    entry: &Entry,
    place: &str,
    error: &str,
    delays: Delays,
) -> bool {
    let scheduler = &bot.scheduler;
    if !scheduler.update(generation, |q| {
        let duration = q.run_started.map_or(Duration::ZERO, |s| s.elapsed());
        q.entries[index].status = EntryStatus::Failed;
        q.entries[index].duration = Some(duration);
        q.current += 1;
    }) {
        return false;
    }
    let first = error.lines().next().unwrap_or_default();
    bot.log(format!(
        "[Scheduler] {} failed to start: {first}",
        entry.name
    ));
    embed(
        bot,
        &format!("Script Error {place}"),
        &format!("**{{bot}}** failed to start **{}**.", entry.name),
        RED,
        vec![("Error".into(), format!("```{first}```"), false)],
    );
    scheduler.wait(generation, delays.after_failure)
}

fn logged_in(bot: &Bot) -> bool {
    crate::api::player::logged_in(bot).unwrap_or(false)
}

/// VibeSkua compares the full paths ignoring case.
fn same_file(a: &Path, b: &Path) -> bool {
    let full = |p: &Path| {
        script::normalize(&std::path::absolute(p).unwrap_or_else(|_| p.to_owned()))
            .to_string_lossy()
            .to_lowercase()
    };
    full(a) == full(b)
}

fn embed(bot: &Bot, title: &str, text: &str, color: u32, fields: Vec<(String, String, bool)>) {
    bot.webhooks.send(Notice::Embed {
        title: title.into(),
        text: text.into(),
        color,
        fields,
    });
}

/// "hh:mm:ss", hours not wrapping (VibeSkua `GetFormattedDuration`).
fn clock(time: Duration) -> String {
    let s = time.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A random GUID, as .NET writes it ("d" format).
fn new_id() -> String {
    let mut b = [0u8; 16];
    if getrandom::fill(&mut b).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        b = nanos.to_le_bytes();
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
