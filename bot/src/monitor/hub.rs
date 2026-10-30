//! The hub: every bot the monitor shows, one entry each, and the one place their game state
//! is kept (decision 4 of docs/turu/WEB_MONITOR_PROPOSAL.md).
//!
//! * **Tabs:** the "Turu Monitor Hub" thread copies each tab's latest snapshot and new events
//!   from its [`Sampler`](super::Sampler) every second. A closed tab's entry goes.
//! * **Army clients:** the Army poll (`crate::army`) hands over what each client's
//!   `GET /snapshot` answered. Army > Army Control reads them back from here.
//! * **Offline:** what was saved in `monitor.json` (every minute) for a character that isn't
//!   live now; at most a day old. A live entry for the same character replaces it.
//!
//! Each entry keeps its last [`EVENTS_KEPT`] events and a point a minute for
//! [`HISTORY_KEPT`] minutes.

use super::snapshot::now_ms;
use super::{EVENTS_KEPT, Event, Snapshot};
use crate::Bot;
use crate::army::{Army, Client};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// An Army client that hasn't answered for this long is offline.
pub const OFFLINE_AFTER_MS: u64 = 15_000;
/// Six hours of one point a minute.
pub const HISTORY_KEPT: usize = 360;
const POINT_EVERY_MS: u64 = 60_000;
const SAVED_FOR_MS: u64 = 24 * 60 * 60 * 1000;
const SAVE_EVERY: Duration = Duration::from_secs(60);
/// Events a card shows.
const RECENT: usize = 3;

/// A tab the page shows.
pub struct Source {
    /// Stays the same while the tab exists (`tab:3`).
    pub id: String,
    /// What the page calls it ("Tab 2").
    pub label: String,
    pub bot: Arc<Bot>,
}

/// This window's tabs now; the app answers.
pub type Sources = Arc<dyn Fn() -> Vec<Source> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Tab,
    Army,
    Offline,
}

/// A minute of a bot's progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub at: u64,
    pub gold: i64,
    pub xp: i64,
    pub kills: u64,
    pub drops: u64,
}

/// Where commands for an entry go.
pub enum Control {
    Tab(Arc<Bot>),
    Army(Arc<Army>, Arc<Client>),
}

enum Target {
    Tab(Weak<Bot>),
    Army(Weak<Army>, Weak<Client>),
    None,
}

struct Entry {
    label: String,
    kind: Kind,
    target: Target,
    snapshot: Option<Snapshot>,
    /// ms since 1970.
    last_seen: u64,
    events: VecDeque<Event>,
    /// The bot's next event number: what to ask for next.
    next_event: u64,
    history: VecDeque<Point>,
}

impl Entry {
    fn new(label: String, kind: Kind, target: Target) -> Self {
        Self {
            label,
            kind,
            target,
            snapshot: None,
            last_seen: 0,
            events: VecDeque::new(),
            next_event: 0,
            history: VecDeque::new(),
        }
    }

    fn online(&self, now: u64) -> bool {
        match (&self.kind, &self.target) {
            (Kind::Tab, _) => true,
            (Kind::Army, Target::Army(_, client)) => {
                client.upgrade().is_some_and(|c| c.is_running())
                    && now.saturating_sub(self.last_seen) < OFFLINE_AFTER_MS
            }
            _ => false,
        }
    }

    fn record(&mut self, snapshot: Option<Snapshot>, events: Vec<Event>, next_event: u64) {
        let now = now_ms();
        self.last_seen = now;
        if next_event < self.next_event {
            // The bot started counting again (a new client process).
            self.events.clear();
        }
        self.next_event = next_event;
        self.events.extend(events);
        while self.events.len() > EVENTS_KEPT {
            self.events.pop_front();
        }
        let Some(snapshot) = snapshot else { return };
        let due = self
            .history
            .back()
            .is_none_or(|p| now.saturating_sub(p.at) >= POINT_EVERY_MS);
        if snapshot.playing() && due {
            self.history.push_back(Point {
                at: now,
                gold: snapshot.gold,
                xp: snapshot.xp,
                kills: snapshot.stats.kills,
                drops: snapshot.stats.drops,
            });
            if self.history.len() > HISTORY_KEPT {
                self.history.pop_front();
            }
        }
        self.snapshot = Some(snapshot);
    }

    fn view(&self, id: &str, now: u64) -> View {
        View {
            id: id.to_owned(),
            label: self.label.clone(),
            kind: self.kind,
            online: self.online(now),
            last_seen: self.last_seen,
            snapshot: self.snapshot.clone(),
            recent: self.events.iter().rev().take(RECENT).cloned().collect(),
        }
    }

    /// The character, when the bot has logged in once.
    fn character(&self) -> Option<&str> {
        self.snapshot
            .as_ref()
            .filter(|s| s.state != super::snapshot::BotState::NotLoaded)
            .map(|s| s.name.as_str())
            .filter(|n| !n.trim().is_empty() && *n != "Turu Bot")
    }
}

/// An entry as the page gets it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub online: bool,
    pub last_seen: u64,
    pub snapshot: Option<Snapshot>,
    /// The newest events first.
    pub recent: Vec<Event>,
}

/// An entry with all its events and history.
#[derive(Debug, Clone, Serialize)]
pub struct Detail {
    #[serde(flatten)]
    pub view: View,
    pub events: Vec<Event>,
    pub history: Vec<Point>,
}

/// What `monitor.json` keeps of an entry.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    label: String,
    snapshot: Snapshot,
    last_seen: u64,
    events: Vec<Event>,
    history: Vec<Point>,
}

pub struct Hub {
    entries: Mutex<BTreeMap<String, Entry>>,
    tabs: Mutex<Option<Sources>>,
    file: Option<PathBuf>,
    started: Instant,
}

impl Hub {
    /// With `file`, loads the entries saved there as offline ones and saves to it.
    pub fn new(file: Option<PathBuf>) -> Arc<Self> {
        let mut entries = BTreeMap::new();
        let saved: Vec<Saved> = file
            .as_ref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let now = now_ms();
        for saved in saved {
            if now.saturating_sub(saved.last_seen) > SAVED_FOR_MS {
                continue;
            }
            let id = offline_id(&saved.snapshot.name);
            let mut entry = Entry::new(saved.label, Kind::Offline, Target::None);
            entry.snapshot = Some(saved.snapshot);
            entry.last_seen = saved.last_seen;
            entry.events = saved.events.into();
            entry.history = saved.history.into();
            entries.insert(id, entry);
        }
        Arc::new(Self {
            entries: Mutex::new(entries),
            tabs: Mutex::new(None),
            file,
            started: Instant::now(),
        })
    }

    pub fn set_tabs(&self, sources: Sources) {
        *self.tabs.lock() = Some(sources);
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// The "Turu Monitor Hub" thread: refreshes the tabs every second and saves every minute,
    /// until the hub is gone.
    pub fn start(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name("Turu Monitor Hub".into())
            .spawn(move || {
                let mut saved = Instant::now();
                while let Some(hub) = weak.upgrade() {
                    hub.refresh_tabs();
                    if saved.elapsed() >= SAVE_EVERY {
                        saved = Instant::now();
                        hub.save();
                    }
                    drop(hub);
                    std::thread::sleep(Duration::from_secs(1));
                }
            });
        if let Err(e) = spawned {
            tracing::error!("Couldn't start the monitor hub: {e}");
        }
    }

    /// Copies the tabs' snapshots and events; forgets closed tabs.
    pub fn refresh_tabs(&self) {
        let Some(sources) = self.tabs.lock().clone() else {
            return;
        };
        // Read the bots first, then lock the entries.
        let tabs: Vec<_> = sources()
            .into_iter()
            .map(|source| {
                let since = self
                    .entries
                    .lock()
                    .get(&source.id)
                    .map_or(0, |e| e.next_event);
                let (events, next) = source.bot.monitor.events_since(since);
                let snapshot = source.bot.monitor.latest();
                (source, snapshot, events, next)
            })
            .collect();
        let mut entries = self.entries.lock();
        entries.retain(|id, _| !id.starts_with("tab:") || tabs.iter().any(|t| t.0.id == *id));
        for (source, snapshot, events, next) in tabs {
            let fresh = snapshot.as_ref().map(|s| s.sampled_at);
            let entry = entries.entry(source.id.clone()).or_insert_with(|| {
                Entry::new(
                    source.label.clone(),
                    Kind::Tab,
                    Target::Tab(Arc::downgrade(&source.bot)),
                )
            });
            entry.label = source.label;
            let changed = fresh != entry.snapshot.as_ref().map(|s| s.sampled_at);
            entry.record(changed.then_some(snapshot).flatten(), events, next);
            let character = entry.character().map(str::to_owned);
            if let Some(name) = character {
                entries.remove(&offline_id(&name));
            }
        }
    }

    /// What an Army client's `GET /snapshot` answered.
    pub(crate) fn record_army(
        &self,
        army: &Arc<Army>,
        client: &Arc<Client>,
        snapshot: Snapshot,
        events: Vec<Event>,
        next_event: u64,
    ) {
        let id = army_id(&client.username);
        let mut entries = self.entries.lock();
        let entry = entries.entry(id).or_insert_with(|| {
            Entry::new(
                client.label.clone(),
                Kind::Army,
                Target::Army(Arc::downgrade(army), Arc::downgrade(client)),
            )
        });
        entry.target = Target::Army(Arc::downgrade(army), Arc::downgrade(client));
        entry.label = client.label.clone();
        entry.record(Some(snapshot), events, next_event);
        let character = entry.character().map(str::to_owned);
        if let Some(name) = character {
            entries.remove(&offline_id(&name));
        }
    }

    /// The event number an Army client's next `/snapshot` asks from.
    pub(crate) fn next_event(&self, username: &str) -> u64 {
        self.entries
            .lock()
            .get(&army_id(username))
            .map_or(0, |e| e.next_event)
    }

    pub fn remove(&self, id: &str) {
        self.entries.lock().remove(id);
    }

    /// Forgets an entry that isn't live (an offline one, or an Army client that ended).
    pub fn forget(&self, id: &str) -> bool {
        let now = now_ms();
        let mut entries = self.entries.lock();
        match entries.get(id) {
            Some(entry) if entry.kind != Kind::Tab && !entry.online(now) => {
                entries.remove(id);
                true
            }
            _ => false,
        }
    }

    /// An Army client's latest snapshot (Army Control reads it).
    pub fn army_snapshot(&self, username: &str) -> Option<Snapshot> {
        self.entries
            .lock()
            .get(&army_id(username))
            .and_then(|e| e.snapshot.clone())
    }

    /// Every entry: tabs, then Army clients, then offline ones.
    pub fn views(&self) -> Vec<View> {
        let now = now_ms();
        let entries = self.entries.lock();
        let mut views: Vec<View> = entries.iter().map(|(id, e)| e.view(id, now)).collect();
        let rank = |v: &View| match v.kind {
            Kind::Tab => 0,
            Kind::Army => 1,
            Kind::Offline => 2,
        };
        let tab_number = |v: &View| {
            v.id.strip_prefix("tab:")
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0)
        };
        views.sort_by(|a, b| {
            (rank(a), tab_number(a), a.label.to_lowercase()).cmp(&(
                rank(b),
                tab_number(b),
                b.label.to_lowercase(),
            ))
        });
        views
    }

    pub fn detail(&self, id: &str) -> Option<Detail> {
        let entries = self.entries.lock();
        let entry = entries.get(id)?;
        Some(Detail {
            view: entry.view(id, now_ms()),
            events: entry.events.iter().cloned().collect(),
            history: entry.history.iter().cloned().collect(),
        })
    }

    /// Where commands for `id` go, while it's live.
    pub fn control(&self, id: &str) -> Option<Control> {
        let entries = self.entries.lock();
        match &entries.get(id)?.target {
            Target::Tab(bot) => bot.upgrade().map(Control::Tab),
            Target::Army(army, client) => {
                let client = client.upgrade().filter(|c| c.is_running())?;
                Some(Control::Army(army.upgrade()?, client))
            }
            Target::None => None,
        }
    }

    /// Writes every entry that has a character to `monitor.json`, newest per character.
    pub fn save(&self) {
        let Some(file) = &self.file else { return };
        let mut saved: BTreeMap<String, Saved> = BTreeMap::new();
        for entry in self.entries.lock().values() {
            let (Some(name), Some(snapshot)) = (entry.character(), &entry.snapshot) else {
                continue;
            };
            let key = name.to_lowercase();
            if saved
                .get(&key)
                .is_some_and(|s| s.last_seen >= entry.last_seen)
            {
                continue;
            }
            saved.insert(
                key,
                Saved {
                    label: entry.label.clone(),
                    snapshot: snapshot.clone(),
                    last_seen: entry.last_seen,
                    events: entry.events.iter().cloned().collect(),
                    history: entry.history.iter().cloned().collect(),
                },
            );
        }
        let list: Vec<Saved> = saved.into_values().collect();
        let written = serde_json::to_vec(&list)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                let temp = file.with_extension("json.tmp");
                std::fs::write(&temp, bytes)
                    .and_then(|()| std::fs::rename(&temp, file))
                    .map_err(|e| e.to_string())
            });
        if let Err(e) = written {
            tracing::warn!("Couldn't save {}: {e}", file.display());
        }
    }
}

pub fn army_id(username: &str) -> String {
    format!("army:{}", username.to_lowercase())
}

fn offline_id(name: &str) -> String {
    format!("offline:{}", name.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::EventKind;
    use crate::monitor::snapshot::BotState;

    fn playing(name: &str, gold: i64) -> Snapshot {
        Snapshot {
            name: name.into(),
            state: BotState::Playing,
            gold,
            sampled_at: now_ms(),
            ..Snapshot::default()
        }
    }

    #[test]
    fn tabs_come_and_go() {
        let hub = Hub::new(None);
        let bot = Bot::new();
        bot.monitor.event(EventKind::Death, "Died");
        let tabs = Arc::new(Mutex::new(vec![bot.clone()]));
        let list = tabs.clone();
        hub.set_tabs(Arc::new(move || {
            list.lock()
                .iter()
                .enumerate()
                .map(|(i, bot)| Source {
                    id: format!("tab:{i}"),
                    label: format!("Tab {}", i + 1),
                    bot: bot.clone(),
                })
                .collect()
        }));
        hub.refresh_tabs();
        let views = hub.views();
        assert_eq!(views.len(), 1);
        assert_eq!((views[0].kind, views[0].online), (Kind::Tab, true));
        assert_eq!(views[0].recent[0].text, "Died");
        assert!(matches!(hub.control("tab:0"), Some(Control::Tab(_))));
        assert!(!hub.forget("tab:0"), "a live tab can't be forgotten");

        // Events are copied once.
        hub.refresh_tabs();
        assert_eq!(hub.detail("tab:0").unwrap().events.len(), 1);

        tabs.lock().clear();
        hub.refresh_tabs();
        assert!(hub.views().is_empty());
    }

    #[test]
    fn saves_characters_and_brings_them_back_offline() {
        let file = std::env::temp_dir().join(format!("turu-monitor-{}.json", std::process::id()));
        let hub = Hub::new(Some(file.clone()));
        {
            let mut entries = hub.entries.lock();
            let mut entry = Entry::new("Tab 1".into(), Kind::Tab, Target::None);
            entry.record(Some(playing("Hero", 10)), Vec::new(), 0);
            entries.insert("tab:0".into(), entry);
            // Without a character: not saved.
            entries.insert(
                "tab:1".into(),
                Entry::new("Tab 2".into(), Kind::Tab, Target::None),
            );
        }
        hub.save();

        let again = Hub::new(Some(file.clone()));
        let views = again.views();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].id, "offline:hero");
        assert_eq!((views[0].kind, views[0].online), (Kind::Offline, false));
        assert_eq!(again.detail("offline:hero").unwrap().history.len(), 1);
        assert!(again.control("offline:hero").is_none());

        // The same character live again replaces it.
        let bot = Bot::new();
        *bot.monitor.latest.lock() = Some(playing("HERO", 20));
        again.set_tabs(Arc::new(move || {
            vec![Source {
                id: "tab:0".into(),
                label: "Tab 1".into(),
                bot: bot.clone(),
            }]
        }));
        again.refresh_tabs();
        let ids: Vec<String> = again.views().into_iter().map(|v| v.id).collect();
        assert_eq!(ids, ["tab:0"]);
        assert!(!again.forget("offline:hero"));
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn keeps_a_point_a_minute() {
        let mut entry = Entry::new("Tab 1".into(), Kind::Tab, Target::None);
        entry.record(Some(playing("Hero", 1)), Vec::new(), 0);
        entry.record(Some(playing("Hero", 2)), Vec::new(), 0);
        assert_eq!(entry.history.len(), 1);
        entry.history[0].at -= POINT_EVERY_MS;
        entry.record(Some(playing("Hero", 3)), Vec::new(), 0);
        assert_eq!(entry.history.len(), 2);
        assert_eq!(entry.history[1].gold, 3);
    }
}
