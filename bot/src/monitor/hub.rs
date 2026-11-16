//! The hub: every bot the monitor shows, one entry each, and the one place their game state
//! is kept (decision 4 of docs/WEB_MONITOR_PROPOSAL.md).
//!
//! * **Tabs:** the "Turu Monitor Hub" thread copies each tab's latest snapshot and new events
//!   from its [`Sampler`](super::Sampler) every second. A closed tab's entry goes.
//! * **Army clients:** the Army poll (`crate::army`) hands over what each client's
//!   `GET /snapshot` answered. Army > Army Control reads them back from here.
//! * **Peers:** tabs in other processes of this window (docs/TAB_PROCESSES_PROPOSAL.md, M5):
//!   the shell's clients, each with a control API of its own. A "Turu Monitor Peers" thread asks
//!   each for its `GET /snapshot` every [`crate::army::POLL_INTERVAL`]; commands go to it over
//!   HTTP. They show as tabs. The shell lists them all; each client lists the others.
//! * **Remote clients:** Turu on other machines reporting here (`POST /api/ingest`,
//!   [`super::push`]): their tabs and Army clients, as [`Report`]s. Commands for them wait in
//!   a queue that the next report's reply carries; the results come with the report after.
//!   Several processes may report under one machine name (one per account on a server, all
//!   sharing a data folder and so a name): each report carries its process's `instance`, and
//!   entries and queues are kept per process, under the one machine heading.
//! * **Offline:** what was saved in `monitor.json` (every minute) for a character that isn't
//!   live now; at most a day old. A live entry for the same character replaces it.
//!
//! Each entry keeps its last [`EVENTS_KEPT`] events and a point a minute for
//! [`HISTORY_KEPT`] minutes.

use super::resources::{
    self, HostHistory, MachineReading, MachineResources, MachineView, MinuteRing, ProcessHistory,
    ProcessReading, Recorder, TuruProcess,
};
use super::snapshot::now_ms;
use super::{EVENTS_KEPT, Event, Snapshot};
use crate::Bot;
use crate::army::{Army, Client};
use crossbeam_channel::{Sender, bounded};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// An Army client or a remote machine that hasn't answered for this long is offline.
pub const OFFLINE_AFTER_MS: u64 = 15_000;
/// Six hours of one point a minute.
pub const HISTORY_KEPT: usize = 360;
const POINT_EVERY_MS: u64 = 60_000;
const SAVED_FOR_MS: u64 = 24 * 60 * 60 * 1000;
const SAVE_EVERY: Duration = Duration::from_secs(60);
/// A peer's `GET /snapshot`.
const PEER_TIMEOUT: Duration = Duration::from_secs(3);
/// Events each entry carries in the state: the page marks the last hour's on its trace.
const RECENT: usize = 12;
/// The control API routes a command may name: the page's buttons and its Script Options form.
pub const COMMANDS: [&str; 12] = [
    "/script/start",
    "/script/options",
    "/script/stop",
    "/script/pause",
    "/script/resume",
    "/login",
    "/logout",
    "/relogin",
    "/map/join",
    "/map/house",
    "/auto/attack",
    "/auto/stop",
];

/// A command or a read the monitor may pass on: a listed POST, or `GET /log` and
/// `GET /script/options`.
pub fn allowed_route(post: bool, route: &str) -> bool {
    let path = route.split('?').next().unwrap_or_default();
    if post {
        COMMANDS.contains(&path) && (path != "/script/start" || !route.contains("path="))
    } else {
        matches!(path, "/log" | "/script/options")
    }
}

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

/// A tab in another process of this window, reached through its control API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSource {
    /// Its id on the page (`tab:<n>`, the shell's number).
    pub id: String,
    pub label: String,
    pub api: SocketAddr,
    pub token: String,
}

/// This window's tabs in other processes now; the app answers.
pub type Peers = Arc<dyn Fn() -> Vec<PeerSource> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Tab,
    Army,
    /// On another machine that reports here.
    Remote,
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
    /// A tab in another process of this window: its control API.
    Peer {
        api: SocketAddr,
        token: String,
    },
    /// The reporting process's queue, and the bot's id there.
    Remote {
        machine: String,
        instance: String,
        bot: String,
    },
}

enum Target {
    Tab(Weak<Bot>),
    Army(Weak<Army>, Weak<Client>),
    Peer {
        api: SocketAddr,
        token: String,
    },
    Remote {
        machine: String,
        instance: String,
        bot: String,
    },
    None,
}

struct Entry {
    label: String,
    /// The machine of a remote entry (or of the offline entry it became).
    machine: Option<String>,
    kind: Kind,
    target: Target,
    snapshot: Option<Snapshot>,
    /// ms since 1970.
    last_seen: u64,
    events: VecDeque<Event>,
    /// The bot's next event number: what to ask for next.
    next_event: u64,
    history: VecDeque<Point>,
    /// An Army client's own process (tabs share this one's), and its memory a minute at a time.
    process: Option<ProcessReading>,
    process_history: MinuteRing,
}

impl Entry {
    fn new(label: String, kind: Kind, target: Target) -> Self {
        Self {
            label,
            machine: None,
            kind,
            target,
            snapshot: None,
            last_seen: 0,
            events: VecDeque::new(),
            next_event: 0,
            history: VecDeque::new(),
            process: None,
            process_history: MinuteRing::default(),
        }
    }

    /// An Army client's process reading, from its `/snapshot` or a report.
    fn set_process(&mut self, process: Option<ProcessReading>) {
        if let Some(process) = &process {
            self.process_history.push((now_ms(), process.memory(), 0));
        }
        self.process = process;
    }

    fn online(&self, now: u64) -> bool {
        match (&self.kind, &self.target) {
            (Kind::Tab, Target::Peer { .. }) => {
                now.saturating_sub(self.last_seen) < OFFLINE_AFTER_MS
            }
            (Kind::Tab, _) => true,
            (Kind::Army, Target::Army(_, client)) => {
                client.upgrade().is_some_and(|c| c.is_running())
                    && now.saturating_sub(self.last_seen) < OFFLINE_AFTER_MS
            }
            (Kind::Remote, _) => now.saturating_sub(self.last_seen) < OFFLINE_AFTER_MS,
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
            machine: self.machine.clone(),
            kind: self.kind,
            online: self.online(now),
            last_seen: self.last_seen,
            snapshot: self.snapshot.clone(),
            recent: self.events.iter().rev().take(RECENT).cloned().collect(),
            process: self.process.clone().filter(|_| self.online(now)),
        }
    }

    /// A remote entry from the process `instance` on `machine`.
    fn reported_by(&self, machine: &str, instance: &str) -> bool {
        matches!(&self.target, Target::Remote { machine: m, instance: i, .. }
            if m.eq_ignore_ascii_case(machine) && i == instance)
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
    /// `None` for this machine's bots.
    pub machine: Option<String>,
    pub kind: Kind,
    pub online: bool,
    pub last_seen: u64,
    pub snapshot: Option<Snapshot>,
    /// The newest events first.
    pub recent: Vec<Event>,
    /// An Army client's own process, while it's live (tabs share the window's: the machine's
    /// line has it).
    pub process: Option<ProcessReading>,
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
    #[serde(default)]
    machine: Option<String>,
    snapshot: Snapshot,
    last_seen: u64,
    events: Vec<Event>,
    history: Vec<Point>,
}

/// What a machine reporting here sends (`POST /api/ingest`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Report {
    /// Its name on the page ("vps-1").
    pub machine: String,
    /// The reporting process, random for each run (`push::instance`): processes on one machine
    /// share its name. Empty from a Turu before 2026-11 (one process per name, as then).
    pub instance: String,
    pub version: String,
    /// The computer it runs on ([`resources::host_id`]): processes and containers on one
    /// computer share it. Empty from a Turu before 2026-11.
    pub host: String,
    /// Its machine's CPU and memory and its own, with its last hour. `None` from a Turu before
    /// 2026-11.
    pub resources: Option<MachineResources>,
    /// Its live tabs and Army clients.
    pub bots: Vec<Pushed>,
    /// What the commands of earlier replies did.
    pub results: Vec<CommandResult>,
}

/// One bot in a [`Report`]: its snapshot, and its events from the last number acknowledged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pushed {
    /// Its id where it runs (`tab:0`, `army:alt`).
    pub id: String,
    pub label: String,
    pub snapshot: Option<Snapshot>,
    pub events: Vec<Event>,
    pub next: u64,
    /// An Army client's own process; `None` for a tab, and from a Turu before 2026-11.
    #[serde(default)]
    pub process: Option<ProcessReading>,
}

/// A command waiting for a remote machine's next report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCommand {
    pub seq: u64,
    /// The bot's id on that machine.
    pub bot: String,
    pub post: bool,
    /// A control API route ([`allowed_route`]).
    pub route: String,
    /// The request's body (`/script/options`' values), if any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub seq: u64,
    pub status: u16,
    pub body: Value,
}

/// The reply to a [`Report`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportReply {
    pub commands: Vec<RemoteCommand>,
}

#[derive(Default)]
struct Queue {
    next_seq: u64,
    /// By reporting process ([`remote_source`]).
    pending: HashMap<String, Vec<RemoteCommand>>,
    /// Who waits for each command's result.
    waiting: HashMap<u64, Sender<(u16, Value)>>,
}

pub struct Hub {
    entries: Mutex<BTreeMap<String, Entry>>,
    queue: Mutex<Queue>,
    tabs: Mutex<Option<Sources>>,
    peers: Mutex<Option<Peers>>,
    file: Option<PathBuf>,
    /// Whether this process saves `file` (one of several on a data folder may: M3).
    saving: std::sync::atomic::AtomicBool,
    started: Instant,
    /// This process's and this machine's CPU and memory.
    resources: Mutex<Recorder>,
    /// What each process reporting here sent of its resources, by [`remote_source`]: sorted, so
    /// a machine's processes keep their order on the page.
    reporters: Mutex<BTreeMap<String, Reported>>,
}

/// A reporting process's resources.
struct Reported {
    /// Its machine's name on the page.
    machine: String,
    /// [`resources::host_id`] there; empty from a Turu before 2026-11.
    host: String,
    /// ms since 1970 here, of its last report.
    last_seen: u64,
    latest: Option<(MachineReading, ProcessReading, u64)>,
    history: MinuteRing,
}

/// A Turu process for [`Hub::machines`] and [`Hub::machine_history`].
struct Part {
    host: String,
    turu: TuruProcess,
    /// Its machine's reading, and when it was taken.
    machine: Option<(MachineReading, u64)>,
    history: Vec<resources::Point>,
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
            entry.machine = saved.machine;
            entry.snapshot = Some(saved.snapshot);
            entry.last_seen = saved.last_seen;
            entry.events = saved.events.into();
            entry.history = saved.history.into();
            entries.insert(id, entry);
        }
        Arc::new(Self {
            entries: Mutex::new(entries),
            queue: Mutex::default(),
            tabs: Mutex::new(None),
            peers: Mutex::new(None),
            file,
            saving: std::sync::atomic::AtomicBool::new(true),
            started: Instant::now(),
            resources: Mutex::default(),
            reporters: Mutex::default(),
        })
    }

    pub fn set_tabs(&self, sources: Sources) {
        *self.tabs.lock() = Some(sources);
    }

    /// Lists this window's tabs in other processes, and starts polling them (once).
    pub fn set_peers(self: &Arc<Self>, peers: Peers) {
        let first = self.peers.lock().replace(peers).is_none();
        if !first {
            return;
        }
        let weak = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name("Turu Monitor Peers".into())
            .spawn(move || {
                let http = reqwest::blocking::Client::new();
                while let Some(hub) = weak.upgrade() {
                    hub.poll_peers(&http);
                    drop(hub);
                    std::thread::sleep(crate::army::POLL_INTERVAL);
                }
            });
        if let Err(e) = spawned {
            tracing::error!("Couldn't start polling the other tabs' processes: {e}");
        }
    }

    /// Asks each peer for its snapshot and new events; forgets peers no longer listed.
    fn poll_peers(&self, http: &reqwest::blocking::Client) {
        let Some(peers) = self.peers.lock().clone() else {
            return;
        };
        let peers = peers();
        self.entries.lock().retain(|id, entry| {
            !matches!(entry.target, Target::Peer { .. }) || peers.iter().any(|p| p.id == *id)
        });
        let slow = u8::from(super::watched());
        for peer in peers {
            let since = self
                .entries
                .lock()
                .get(&peer.id)
                .map_or(0, |e| e.next_event);
            let path = format!("/snapshot?since={since}&slow={slow}");
            let reply = crate::army::api_request(
                http,
                peer.api,
                &peer.token,
                false,
                &path,
                Vec::new(),
                PEER_TIMEOUT,
            );
            let mut entries = self.entries.lock();
            let target = || Target::Peer {
                api: peer.api,
                token: peer.token.clone(),
            };
            let entry = entries
                .entry(peer.id.clone())
                .or_insert_with(|| Entry::new(peer.label.clone(), Kind::Tab, target()));
            entry.target = target();
            entry.kind = Kind::Tab;
            entry.label = peer.label;
            // Starting up (its API isn't listening yet), restarting, or busy: it goes offline.
            let reply = match reply {
                Ok(reply) => reply,
                Err(e) => {
                    if entry.online(now_ms()) || entry.last_seen == 0 {
                        tracing::info!("Monitor: {} ({}) doesn't answer: {e}", peer.id, peer.api);
                        // Logged once until it answers again.
                        entry.last_seen = 1;
                    }
                    continue;
                }
            };
            let snapshot = serde_json::from_value(reply["snapshot"].clone()).ok();
            let events = serde_json::from_value(reply["events"].clone()).unwrap_or_default();
            let next = reply["next"].as_u64().unwrap_or(since);
            entry.record(snapshot, events, next);
            entry.set_process(serde_json::from_value(reply["process"].clone()).ok());
            let character = entry.character().map(str::to_owned);
            if let Some(name) = character {
                entries.remove(&offline_id(&name));
            }
        }
    }

    /// An entry's latest snapshot, by its id (Army Control reads a peer's).
    pub fn snapshot(&self, id: &str) -> Option<Snapshot> {
        self.entries.lock().get(id).and_then(|e| e.snapshot.clone())
    }

    /// Whether an entry is live now.
    pub fn online(&self, id: &str) -> bool {
        self.entries
            .lock()
            .get(id)
            .is_some_and(|e| e.online(now_ms()))
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// The "Turu Monitor Hub" thread: refreshes the tabs every second, reads this process's and
    /// this machine's CPU and memory every 2 s while the page is open and every minute always,
    /// and saves every minute, until the hub is gone.
    pub fn start(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name("Turu Monitor Hub".into())
            .spawn(move || {
                let mut saved = Instant::now();
                while let Some(hub) = weak.upgrade() {
                    hub.refresh_tabs();
                    hub.resources.lock().tick(now_ms(), super::watched());
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
        entries.retain(|id, entry| {
            !id.starts_with("tab:")
                || matches!(entry.target, Target::Peer { .. })
                || tabs.iter().any(|t| t.0.id == *id)
        });
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
        process: Option<ProcessReading>,
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
        entry.set_process(process);
        let character = entry.character().map(str::to_owned);
        if let Some(name) = character {
            entries.remove(&offline_id(&name));
        }
    }

    /// Takes in a remote process's report: its bots replace that process's entries, and the
    /// results go to whoever waits for them.
    pub fn record_report(&self, report: Report) {
        let machine = machine_name(&report.machine);
        let instance = instance_name(&report.instance);
        {
            let now = now_ms();
            let mut reporters = self.reporters.lock();
            // A process that stopped reporting an hour ago won't again (a new run has a new
            // instance).
            reporters.retain(|_, r| now.saturating_sub(r.last_seen) < 60 * POINT_EVERY_MS);
            let reported = reporters
                .entry(remote_source(&machine, &instance))
                .or_insert_with(|| Reported {
                    machine: machine.clone(),
                    host: String::new(),
                    last_seen: now,
                    latest: None,
                    history: MinuteRing::default(),
                });
            reported.machine = machine.clone();
            reported.host = host_name(&report.host);
            reported.last_seen = now;
            if let Some(res) = report.resources {
                reported.history.seed(&res.history);
                reported
                    .history
                    .push((res.at, res.process.memory(), res.machine.memory_available));
                reported.latest = Some((res.machine, res.process, res.at));
            }
        }
        {
            let mut entries = self.entries.lock();
            let prefix = format!("remote:{}:", remote_source(&machine, &instance));
            let ids: Vec<String> = report
                .bots
                .iter()
                .map(|b| format!("{prefix}{}", b.id))
                .collect();
            entries.retain(|id, e| !e.reported_by(&machine, &instance) || ids.contains(id));
            let mut characters = Vec::new();
            for (pushed, id) in report.bots.into_iter().zip(ids) {
                let target = || Target::Remote {
                    machine: machine.clone(),
                    instance: instance.clone(),
                    bot: pushed.id.clone(),
                };
                let entry = entries
                    .entry(id)
                    .or_insert_with(|| Entry::new(pushed.label.clone(), Kind::Remote, target()));
                entry.target = target();
                entry.machine = Some(machine.clone());
                entry.label = pushed.label;
                entry.record(pushed.snapshot, pushed.events, pushed.next);
                entry.set_process(pushed.process);
                let character = entry.character().map(str::to_owned);
                if let Some(name) = character {
                    entries.remove(&offline_id(&name));
                    characters.push(name.to_lowercase());
                }
            }
            // An earlier run of a process on this machine (each run has its own instance) left
            // entries that went offline: drop those this report now shows, and those that never
            // had a character, which would pile up as unnamed rows with every restart.
            let now = now_ms();
            entries.retain(|_, e| {
                let earlier = match &e.target {
                    Target::Remote {
                        machine: m,
                        instance: i,
                        ..
                    } => m.eq_ignore_ascii_case(&machine) && *i != instance,
                    _ => false,
                };
                !earlier
                    || e.online(now)
                    || e.character()
                        .is_some_and(|c| !characters.contains(&c.to_lowercase()))
            });
        }
        let mut queue = self.queue.lock();
        for result in report.results {
            if let Some(waiter) = queue.waiting.remove(&result.seq) {
                let _ = waiter.send((result.status, result.body));
            }
        }
    }

    /// The commands waiting for a reporting process, for the reply to its report.
    pub fn take_commands(&self, machine: &str, instance: &str) -> Vec<RemoteCommand> {
        let key = remote_source(&machine_name(machine), &instance_name(instance));
        self.queue.lock().pending.remove(&key).unwrap_or_default()
    }

    /// Runs a control API route (checked with [`allowed_route`]) for an entry and waits for its
    /// answer: in this process for a tab, over HTTP for an Army client, through the queue for a
    /// remote one. Blocks up to a couple of minutes (a login).
    pub fn run(&self, control: Control, post: bool, route: &str, body: &[u8]) -> (u16, Value) {
        if !allowed_route(post, route) {
            return (403, json!({ "error": "the monitor doesn't pass that on" }));
        }
        match control {
            Control::Tab(bot) => crate::control_api::local(&bot, post, route, body),
            Control::Army(army, client) => {
                match army.forward(&client, post, route, body.to_vec()) {
                    Ok(value) => (200, value),
                    Err(e) => (502, json!({ "error": e })),
                }
            }
            Control::Peer { api, token } => {
                let http = reqwest::blocking::Client::new();
                let timeout = crate::army::COMMAND_TIMEOUT;
                match crate::army::api_request(
                    &http,
                    api,
                    &token,
                    post,
                    route,
                    body.to_vec(),
                    timeout,
                ) {
                    Ok(value) => (200, value),
                    Err(e) => (502, json!({ "error": e })),
                }
            }
            Control::Remote {
                machine,
                instance,
                bot,
            } => {
                let (tx, rx) = bounded(1);
                let seq = {
                    let mut queue = self.queue.lock();
                    let seq = queue.next_seq;
                    queue.next_seq += 1;
                    queue.waiting.insert(seq, tx);
                    let command = RemoteCommand {
                        seq,
                        bot,
                        post,
                        route: route.to_owned(),
                        body: String::from_utf8_lossy(body).into_owned(),
                    };
                    queue
                        .pending
                        .entry(remote_source(&machine, &instance))
                        .or_default()
                        .push(command);
                    seq
                };
                match rx.recv_timeout(crate::army::COMMAND_TIMEOUT) {
                    Ok(answer) => answer,
                    Err(_) => {
                        let mut queue = self.queue.lock();
                        queue.waiting.remove(&seq);
                        for list in queue.pending.values_mut() {
                            list.retain(|c| c.seq != seq);
                        }
                        (504, json!({ "error": format!("{machine} didn't answer") }))
                    }
                }
            }
        }
    }

    /// This machine's live bots for a report, with their events from the numbers in `sent`.
    pub fn export(&self, sent: &HashMap<String, u64>) -> Vec<Pushed> {
        let now = now_ms();
        self.entries
            .lock()
            .iter()
            .filter(|(_, e)| matches!(e.kind, Kind::Tab | Kind::Army) && e.online(now))
            .map(|(id, e)| {
                let since = sent.get(id).copied().unwrap_or(0);
                Pushed {
                    id: id.clone(),
                    label: e.label.clone(),
                    snapshot: e.snapshot.clone(),
                    events: e
                        .events
                        .iter()
                        .filter(|ev| ev.seq >= since)
                        .cloned()
                        .collect(),
                    next: e.next_event,
                    process: e.process.clone(),
                }
            })
            .collect()
    }

    /// This process's latest reading and last hour, for a report.
    pub fn resources(&self) -> Option<MachineResources> {
        self.resources.lock().resources()
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

    /// Every entry: tabs, then Army clients, then remote ones by machine, then offline ones.
    pub fn views(&self) -> Vec<View> {
        let now = now_ms();
        let entries = self.entries.lock();
        let mut views: Vec<View> = entries.iter().map(|(id, e)| e.view(id, now)).collect();
        let rank = |v: &View| match v.kind {
            Kind::Tab => 0,
            Kind::Army => 1,
            Kind::Remote => 2,
            Kind::Offline => 3,
        };
        let tab_number = |v: &View| {
            v.id.strip_prefix("tab:")
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0)
        };
        let key = |v: &View| {
            (
                rank(v),
                v.machine.as_deref().unwrap_or_default().to_lowercase(),
                tab_number(v),
                v.label.to_lowercase(),
            )
        };
        views.sort_by_key(key);
        views
    }

    /// Every live Turu process this monitor knows of: this one, its Army clients, the processes
    /// reporting here and their Army clients.
    fn parts(&self) -> Vec<Part> {
        let now = now_ms();
        let this_host = resources::host_id();
        let mut parts = Vec::new();
        {
            let recorder = self.resources.lock();
            if let Some(sample) = recorder.latest() {
                parts.push(Part {
                    host: this_host.clone(),
                    turu: TuruProcess {
                        id: None,
                        machine: None,
                        label: "Turu".into(),
                        process: sample.process,
                    },
                    machine: Some((sample.machine, sample.at)),
                    history: recorder.history(),
                });
            }
        }
        // Each reporting process's host, for its Army clients.
        let mut hosts: HashMap<String, String> = HashMap::new();
        for (source, reported) in self.reporters.lock().iter() {
            // A Turu before 2026-11 sends no host: its own, so it's never merged with another.
            let host = if reported.host.is_empty() {
                format!("remote:{source}")
            } else {
                reported.host.clone()
            };
            hosts.insert(source.clone(), host.clone());
            let Some((machine, process, at)) = &reported.latest else {
                continue;
            };
            if now.saturating_sub(reported.last_seen) >= OFFLINE_AFTER_MS {
                continue;
            }
            parts.push(Part {
                host,
                turu: TuruProcess {
                    id: Some(format!("remote:{source}:")),
                    machine: Some(reported.machine.clone()),
                    label: format!("Turu on {}", reported.machine),
                    process: process.clone(),
                },
                machine: Some((machine.clone(), *at)),
                history: reported.history.points(),
            });
        }
        for (id, entry) in self.entries.lock().iter() {
            let Some(process) = entry.process.clone().filter(|_| entry.online(now)) else {
                continue;
            };
            let host = match &entry.target {
                Target::Remote {
                    machine, instance, ..
                } => match hosts.get(&remote_source(machine, instance)) {
                    Some(host) => host.clone(),
                    None => continue,
                },
                _ => this_host.clone(),
            };
            parts.push(Part {
                host,
                turu: TuruProcess {
                    id: Some(id.clone()),
                    machine: entry.machine.clone(),
                    label: entry.label.clone(),
                    process,
                },
                machine: None,
                history: entry.process_history.points(),
            });
        }
        parts
    }

    /// The machines and the Turu processes on each, one per computer ([`resources::host_id`]):
    /// this one first, then the others by name. A machine's reading is the latest any of its
    /// processes took.
    pub fn machines(&self) -> Vec<MachineView> {
        let mut machines: Vec<MachineView> = Vec::new();
        for part in self.parts() {
            let view = match machines.iter_mut().find(|m| m.host == part.host) {
                Some(view) => view,
                None => {
                    machines.push(MachineView {
                        host: part.host.clone(),
                        at: 0,
                        machine: MachineReading::default(),
                        turu: Vec::new(),
                    });
                    machines.last_mut().expect("just pushed")
                }
            };
            if let Some((machine, at)) = part.machine
                && at >= view.at
            {
                view.machine = machine;
                view.at = at;
            }
            view.turu.push(part.turu);
        }
        // A host whose processes are all Army clients has no machine reading yet.
        machines.retain(|m| m.at > 0);
        let this_host = resources::host_id();
        machines.sort_by_key(|m| {
            (
                m.host != this_host,
                m.turu
                    .iter()
                    .find_map(|t| t.machine.clone())
                    .unwrap_or_default()
                    .to_lowercase(),
            )
        });
        machines
    }

    /// A machine's last hour, by its host id: the line's points and each process's own.
    pub fn machine_history(&self, host: &str) -> Option<HostHistory> {
        let parts: Vec<Part> = self
            .parts()
            .into_iter()
            .filter(|p| p.host == host)
            .collect();
        let base = parts
            .iter()
            .filter(|p| p.machine.is_some())
            .max_by_key(|p| p.history.len())?
            .history
            .clone();
        let turu = parts
            .into_iter()
            .map(|p| ProcessHistory {
                id: p.turu.id,
                points: p.history,
            })
            .collect();
        Some(HostHistory::new(&base, turu))
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
            Target::Peer { api, token } => entries[id].online(now_ms()).then(|| Control::Peer {
                api: *api,
                token: token.clone(),
            }),
            Target::Army(army, client) => {
                let client = client.upgrade().filter(|c| c.is_running())?;
                Some(Control::Army(army.upgrade()?, client))
            }
            Target::Remote {
                machine,
                instance,
                bot,
            } => entries.get(id)?.online(now_ms()).then(|| Control::Remote {
                machine: machine.clone(),
                instance: instance.clone(),
                bot: bot.clone(),
            }),
            Target::None => None,
        }
    }

    /// Writes every entry that has a character to `monitor.json`, newest per character.
    /// Whether this process saves the entries to the hub's file.
    pub fn set_saving(&self, on: bool) {
        self.saving.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn save(&self) {
        let Some(file) = &self.file else { return };
        if !self.saving.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
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
                    machine: entry.machine.clone(),
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

/// A machine name as the page shows it: letters, digits, `-`, `_`, `.` and spaces, at most 40;
/// "remote" when nothing is left.
pub fn machine_name(name: &str) -> String {
    let clean: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '-'
            }
        })
        .take(40)
        .collect();
    let clean = clean.trim().to_owned();
    if clean.is_empty() {
        "remote".into()
    } else {
        clean
    }
}

/// A reported host id: letters, digits, `-`, `_` and `.`, at most 64.
fn host_name(host: &str) -> String {
    host.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(64)
        .collect::<String>()
        .to_lowercase()
}

/// A reporting process's instance as ids carry it: letters and digits, at most 16.
fn instance_name(instance: &str) -> String {
    instance
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(16)
        .collect::<String>()
        .to_lowercase()
}

/// A reporting process in ids and queue keys: the machine (lowercase), then its instance
/// (`vps-1:3fa9c2`); the machine alone for a Turu that sends none.
fn remote_source(machine: &str, instance: &str) -> String {
    let machine = machine.to_lowercase();
    if instance.is_empty() {
        machine
    } else {
        format!("{machine}:{instance}")
    }
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
        let tabs = Arc::new(Mutex::new(vec![bot]));
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

    fn pushed(id: &str, name: &str) -> Pushed {
        Pushed {
            id: id.into(),
            label: "Tab 1".into(),
            snapshot: Some(playing(name, 1)),
            events: Vec::new(),
            next: 0,
            process: None,
        }
    }

    #[test]
    fn remote_machines_come_and_go() {
        let hub = Hub::new(None);
        let report = |bots: Vec<Pushed>| Report {
            machine: "VPS/1".into(),
            instance: String::new(),
            version: "test".into(),
            bots,
            results: Vec::new(),
            ..Default::default()
        };
        hub.record_report(report(vec![
            pushed("tab:0", "Hero"),
            pushed("army:alt", "Alt"),
        ]));
        let views = hub.views();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].machine.as_deref(), Some("VPS-1"));
        assert!(views.iter().all(|v| v.kind == Kind::Remote && v.online));
        // The machine's report is the whole list: a bot left out has gone.
        hub.record_report(report(vec![pushed("tab:0", "Hero")]));
        assert_eq!(hub.views().len(), 1);
        // A machine that stops reporting goes offline, and loses its control.
        hub.entries
            .lock()
            .values_mut()
            .for_each(|e| e.last_seen -= OFFLINE_AFTER_MS);
        assert!(!hub.views()[0].online);
        assert!(hub.control("remote:vps-1:tab:0").is_none());
        assert!(hub.forget("remote:vps-1:tab:0"));
    }

    #[test]
    fn remote_commands_wait_for_the_next_report() {
        let hub = Hub::new(None);
        let report = |results| Report {
            machine: "vps".into(),
            instance: String::new(),
            version: "test".into(),
            bots: vec![pushed("tab:0", "Hero")],
            results,
            ..Default::default()
        };
        hub.record_report(report(Vec::new()));
        let control = hub.control("remote:vps:tab:0").expect("online");
        let waiting = {
            let hub = hub.clone();
            std::thread::spawn(move || hub.run(control, true, "/script/stop", b""))
        };
        let commands = loop {
            let commands = hub.take_commands("VPS", "");
            if !commands.is_empty() {
                break commands;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            (commands[0].bot.as_str(), commands[0].route.as_str()),
            ("tab:0", "/script/stop")
        );
        hub.record_report(report(vec![CommandResult {
            seq: commands[0].seq,
            status: 200,
            body: json!({ "stopping": true }),
        }]));
        assert_eq!(waiting.join().unwrap(), (200, json!({ "stopping": true })));
        // Routes the page can't send aren't queued.
        let control = hub.control("remote:vps:tab:0").unwrap();
        assert_eq!(hub.run(control, true, "/game/options", b"").0, 403);
        assert!(hub.take_commands("vps", "").is_empty());
    }

    fn from(instance: &str, bots: Vec<Pushed>) -> Report {
        Report {
            machine: "vps".into(),
            instance: instance.into(),
            version: "test".into(),
            bots,
            results: Vec::new(),
            ..Default::default()
        }
    }

    const MB: u64 = 1 << 20;

    fn resources(at: u64, memory: u64, available: u64) -> MachineResources {
        MachineResources {
            at,
            machine: MachineReading {
                os: "linux".into(),
                cores: 4,
                memory_total: 8192 * MB,
                memory_available: available,
                ..Default::default()
            },
            process: ProcessReading {
                rss: memory,
                ..Default::default()
            },
            history: vec![(at - 60_000, memory - MB, available)],
        }
    }

    fn reporting(
        instance: &str,
        host: &str,
        res: Option<MachineResources>,
        bots: Vec<Pushed>,
    ) -> Report {
        Report {
            host: host.into(),
            resources: res,
            ..from(instance, bots)
        }
    }

    #[test]
    fn machines_are_kept_by_host() {
        let hub = Hub::new(None);
        let now = now_ms();
        let mut army = pushed("army:alt", "Alt");
        army.process = Some(ProcessReading {
            rss: 300 * MB,
            ..Default::default()
        });
        // Two processes on one VPS (one with an Army client), and an older Turu elsewhere.
        hub.record_report(reporting(
            "aaaa",
            "BOOT-1",
            Some(resources(now - 5_000, 400 * MB, 2048 * MB)),
            vec![pushed("tab:0", "Hero"), army],
        ));
        hub.record_report(reporting(
            "bbbb",
            "boot-1",
            Some(resources(now - 1_000, 420 * MB, 2000 * MB)),
            vec![pushed("tab:0", "Mage")],
        ));
        hub.record_report(reporting("cccc", "", None, vec![pushed("tab:0", "Old")]));
        let machines = hub.machines();
        // The older Turu sent no reading: no machine for it.
        assert_eq!(machines.len(), 1, "{machines:?}");
        let vps = &machines[0];
        assert_eq!(vps.host, "boot-1");
        // The latest reading of the two.
        assert_eq!(vps.machine.memory_available, 2000 * MB);
        let ids: Vec<_> = vps.turu.iter().map(|t| t.id.clone().unwrap()).collect();
        assert_eq!(
            ids,
            [
                "remote:vps:aaaa:",
                "remote:vps:bbbb:",
                "remote:vps:aaaa:army:alt"
            ]
        );
        assert!(vps.turu.iter().all(|t| t.machine.as_deref() == Some("vps")));
        // The Army client's process shows on its entry too.
        let view = hub
            .views()
            .into_iter()
            .find(|v| v.id == "remote:vps:aaaa:army:alt")
            .unwrap();
        assert_eq!(view.process.unwrap().rss, 300 * MB);
        // Its hour: the first report's hour taken on, and the processes added up.
        let history = hub.machine_history("boot-1").unwrap();
        assert_eq!(history.turu.len(), 3);
        assert_eq!(history.turu[0].points.len(), 2);
        assert!(hub.machine_history("elsewhere").is_none());
        // A process that stops reporting leaves the machine.
        hub.reporters
            .lock()
            .values_mut()
            .for_each(|r| r.last_seen -= OFFLINE_AFTER_MS);
        assert!(hub.machines().is_empty());
    }

    #[test]
    fn takes_reports_from_older_turu() {
        // A report as a Turu before 2026-11 sends it: no host, resources or process.
        let report: Report = serde_json::from_value(json!({
            "machine": "old", "instance": "dddd", "version": "0.2.0", "results": [],
            "bots": [{ "id": "tab:0", "label": "Tab 1", "snapshot": null, "events": [], "next": 0 }]
        }))
        .unwrap();
        assert!(report.host.is_empty() && report.resources.is_none());
        assert!(report.bots[0].process.is_none());
        let hub = Hub::new(None);
        hub.record_report(report);
        assert_eq!(hub.views().len(), 1);
        assert!(hub.machines().is_empty());
    }

    #[test]
    fn processes_on_one_machine_stay_apart() {
        let hub = Hub::new(None);
        // Two clients on one server, both called "vps", both with a tab 0.
        hub.record_report(from("aaaa", vec![pushed("tab:0", "Hero")]));
        hub.record_report(from("bbbb", vec![pushed("tab:0", "Alt")]));
        hub.record_report(from("aaaa", vec![pushed("tab:0", "Hero")]));
        let views = hub.views();
        let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["remote:vps:aaaa:tab:0", "remote:vps:bbbb:tab:0"]);
        assert!(
            views
                .iter()
                .all(|v| v.online && v.machine.as_deref() == Some("vps"))
        );

        // A command goes to the process whose bot it names, not to whichever reports first.
        let control = hub.control("remote:vps:bbbb:tab:0").unwrap();
        let waiting = {
            let hub = hub.clone();
            std::thread::spawn(move || hub.run(control, true, "/script/stop", b""))
        };
        let commands = loop {
            assert!(hub.take_commands("vps", "aaaa").is_empty());
            let commands = hub.take_commands("vps", "bbbb");
            if !commands.is_empty() {
                break commands;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let mut done = from("bbbb", vec![pushed("tab:0", "Alt")]);
        done.results.push(CommandResult {
            seq: commands[0].seq,
            status: 200,
            body: json!({}),
        });
        hub.record_report(done);
        assert_eq!(waiting.join().unwrap().0, 200);
    }

    #[test]
    fn a_restarted_process_replaces_its_old_entries() {
        let hub = Hub::new(None);
        hub.record_report(from(
            "aaaa",
            vec![pushed("tab:0", "Hero"), pushed("tab:1", "Turu Bot")],
        ));
        hub.record_report(from("bbbb", vec![pushed("tab:0", "Other")]));
        // The first process stops; the second is still reporting.
        hub.entries
            .lock()
            .iter_mut()
            .filter(|(id, _)| id.contains(":aaaa:"))
            .for_each(|(_, e)| e.last_seen -= OFFLINE_AFTER_MS);
        // It starts again with a new instance and the same character.
        hub.record_report(from("cccc", vec![pushed("tab:0", "Hero")]));
        let ids: Vec<String> = hub.views().into_iter().map(|v| v.id).collect();
        assert_eq!(ids, ["remote:vps:bbbb:tab:0", "remote:vps:cccc:tab:0"]);
    }

    #[test]
    fn checks_routes() {
        assert!(allowed_route(true, "/script/stop"));
        assert!(allowed_route(true, "/map/join?map=battleon"));
        assert!(!allowed_route(true, "/script/start?path=/tmp/evil.ts"));
        assert!(!allowed_route(true, "/game/options"));
        assert!(allowed_route(false, "/log?type=script&since=0"));
        assert!(!allowed_route(false, "/script/stop"));
        assert!(allowed_route(false, "/script/options"));
        assert!(allowed_route(true, "/script/options?reset=1"));
        assert!(!allowed_route(false, "/scheduler/options?id=1"));
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
