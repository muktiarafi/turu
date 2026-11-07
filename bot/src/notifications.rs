//! The Notification Center's feed (VibeSkua `NotificationCenterViewModel`): notable game and
//! script events (drops, deaths, relogins, map changes, quests, sales, script starts, stops and
//! errors), newest first, at most [`MAXIMUM_NOTIFICATIONS`]. Log lines stay in the Logs window.
//!
//! The feed collects from the moment the bot is made, whether the window is open or not, so it
//! reflects the whole session (VibeSkua makes its singleton view model at startup for this).

use crate::Bot;
use crate::events::BotEvent;
use chrono::{DateTime, Local};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

/// VibeSkua `MaximumNotifications`: past it, the oldest entry goes.
pub const MAXIMUM_NOTIFICATIONS: usize = 500;

/// VibeSkua `NotificationKind`: what an entry is about, which picks its filter chip, icon and
/// colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationKind {
    /// Quests, script starts and stops.
    Activity,
    /// Drops, pickups and sales.
    Item,
    /// Map changes (VibeSkua `System`, shown as "Map").
    System,
    /// Deaths and relogins.
    Warning,
    /// Script errors.
    Error,
}

impl NotificationKind {
    /// VibeSkua `NotificationEntry.KindName`: how the kind reads in a copied or saved line and
    /// in the icon's tooltip.
    pub fn name(self) -> &'static str {
        match self {
            Self::Item => "Item",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::System => "Map",
            Self::Activity => "Activity",
        }
    }
}

/// VibeSkua `NotificationEntry`: one event in the feed.
#[derive(Debug, Clone)]
pub struct Notification {
    /// Unique within the feed (for the window's selection).
    pub id: u64,
    pub time: DateTime<Local>,
    pub kind: NotificationKind,
    pub message: String,
}

impl Notification {
    /// VibeSkua `TimeDisplay`: `HH:mm:ss`.
    pub fn time_display(&self) -> String {
        self.time.format("%H:%M:%S").to_string()
    }

    /// VibeSkua `Line`: how the entry reads when copied or saved (time, kind, message).
    pub fn line(&self) -> String {
        format!(
            "{} [{}] {}",
            self.time_display(),
            self.kind.name(),
            self.message
        )
    }
}

/// VibeSkua `NotificationCenterViewModel.ToText`: `entries` oldest first, one line each, as
/// Copy and Save write them.
pub fn to_text<'a>(entries: impl IntoIterator<Item = &'a Notification>) -> String {
    let mut entries: Vec<&Notification> = entries.into_iter().collect();
    entries.sort_by_key(|e| (e.time, e.id));
    entries
        .iter()
        .map(|e| e.line())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Default)]
struct Inner {
    /// Newest first.
    entries: VecDeque<Notification>,
    next_id: u64,
    /// Bumped on every change, so a window redraws only what changed.
    revision: u64,
    /// VibeSkua `_runningScript`: the name of the script that last started, kept for its stop
    /// and error entries (another script may be loaded into the loader while it runs).
    running_script: Option<String>,
}

/// The feed of one bot (`Bot::notifications`).
#[derive(Default)]
pub struct Feed {
    inner: Mutex<Inner>,
}

impl Feed {
    /// VibeSkua `Add`: a new entry at the top; past [`MAXIMUM_NOTIFICATIONS`] the oldest goes.
    /// A blank message is ignored.
    pub fn add(&self, kind: NotificationKind, message: impl Into<String>) {
        let message = message.into();
        if message.trim().is_empty() {
            return;
        }
        let mut inner = self.inner.lock();
        inner.next_id += 1;
        let entry = Notification {
            id: inner.next_id,
            time: Local::now(),
            kind,
            message,
        };
        inner.entries.push_front(entry);
        inner.entries.truncate(MAXIMUM_NOTIFICATIONS);
        inner.revision += 1;
    }

    /// Every entry, newest first.
    pub fn entries(&self) -> Vec<Notification> {
        self.inner.lock().entries.iter().cloned().collect()
    }

    /// How many entries there are of `kind` (`None`: all), as the filter chips count them.
    pub fn count(&self, kind: Option<NotificationKind>) -> usize {
        let inner = self.inner.lock();
        match kind {
            None => inner.entries.len(),
            Some(kind) => inner.entries.iter().filter(|e| e.kind == kind).count(),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Changes with every added entry and every Clear.
    pub fn revision(&self) -> u64 {
        self.inner.lock().revision
    }

    /// VibeSkua `Clear`: removes every entry.
    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.entries.clear();
        inner.revision += 1;
    }

    /// VibeSkua `OnScriptStarted`: "Script started: name" (`script` is the script's file).
    pub fn script_started(&self, script: Option<&Path>) {
        let name = script.and_then(script_name);
        let message = match &name {
            Some(name) => format!("Script started: {name}"),
            None => "Script started".to_owned(),
        };
        self.inner.lock().running_script = name;
        self.add(NotificationKind::Activity, message);
    }

    /// VibeSkua `OnScriptStopped`: "Script stopped: name". `loaded` is the script in the loader,
    /// for a stop without a start.
    pub fn script_stopped(&self, loaded: Option<&Path>) {
        let name = self
            .inner
            .lock()
            .running_script
            .take()
            .or_else(|| loaded.and_then(script_name));
        self.add(
            NotificationKind::Activity,
            match name {
                Some(name) => format!("Script stopped: {name}"),
                None => "Script stopped".to_owned(),
            },
        );
    }

    /// VibeSkua `OnScriptError`: "Script error (name): message".
    pub fn script_error(&self, message: &str, loaded: Option<&Path>) {
        let name = self
            .inner
            .lock()
            .running_script
            .clone()
            .or_else(|| loaded.and_then(script_name));
        self.add(
            NotificationKind::Error,
            match name {
                Some(name) => format!("Script error ({name}): {message}"),
                None => format!("Script error: {message}"),
            },
        );
    }

    /// The game events the feed shows (VibeSkua's `GameEvents` handlers); others are ignored.
    pub fn on_event(&self, event: &BotEvent, loaded: impl FnOnce() -> Option<std::path::PathBuf>) {
        use NotificationKind::{Activity, Item, System, Warning};
        match event {
            // VibeSkua `OnItemDropped`: `addedToInv` reads "picked up".
            BotEvent::ItemDropped { item } => self.add(Item, format!("{} (dropped)", item.name)),
            BotEvent::ItemAdded { item, .. } => {
                self.add(Item, format!("{} (picked up)", item.name))
            }
            BotEvent::PlayerDeath => self.add(Warning, "Your character died."),
            BotEvent::ReloginTriggered { was_kicked } => self.add(
                Warning,
                if *was_kicked {
                    "Connection interrupted; reconnecting."
                } else {
                    "Reconnecting to the game."
                },
            ),
            BotEvent::MapChanged { map } => self.add(System, format!("Entered {map}")),
            BotEvent::QuestAccepted { quest_id } => {
                self.add(Activity, format!("Accepted quest #{quest_id}"))
            }
            BotEvent::QuestTurnedIn { quest_id } => {
                self.add(Activity, format!("Completed quest #{quest_id}"))
            }
            BotEvent::ItemSold {
                quantity_sold,
                cost,
                is_ac,
                ..
            } => self.add(
                Item,
                format!(
                    "Sold {} item(s) for {} {}",
                    thousands(*quantity_sold),
                    thousands(*cost),
                    if *is_ac { "AC" } else { "gold" }
                ),
            ),
            BotEvent::ScriptStopped => self.script_stopped(loaded().as_deref()),
            _ => {}
        }
    }
}

/// VibeSkua `ScriptName`: the file name without its extension.
fn script_name(path: &Path) -> Option<String> {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
}

/// .NET's `N0`: `12345` reads `12,345`.
pub fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Fills the bot's feed from its events, from now on (`Bot::new`).
pub(crate) fn start(bot: &Arc<Bot>) {
    let weak = Arc::downgrade(bot);
    // Runs inside `on_flash_call` too: only takes the feed's own lock.
    bot.events.subscribe(move |event| {
        let Some(bot) = weak.upgrade() else { return };
        bot.notifications.on_event(event, || bot.scripts.loaded());
    });
}

#[cfg(test)]
mod tests {
    use super::thousands;

    #[test]
    fn groups_thousands_like_n0() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(thousands(-12345), "-12,345");
    }
}
