//! Army > Army Control (VibeSkua's tabbed host "Army Control" menu): one list of this window's tabs,
//! the shell's other tabs and the clients Army launched, each with what it runs and where it is,
//! and the commands that go to the checked rows, or to every row still running when none is
//! checked, through their control API. One per process, not per tab: the egui window and a native
//! shell's page list the same rows and the same outcomes ([`results`]).
//!
//! A shell keeps which rows are checked and sends their keys with each action; a key names the
//! same row whichever slot answers (a shell tab is `tab:<the shell's number>`, its own tab too).

use super::Control;
use super::scheduler::Tint;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::accounts::Accounts;
use turu_bot::army::{Client, Command};
use turu_bot::monitor::Snapshot;
use turu_bot::monitor::snapshot::BotState;

use crate::turu::TabId;

/// How many command outcomes are kept.
const RESULTS: usize = 40;
/// How often the Army > Accounts groups are read again.
const GROUPS_EVERY: Duration = Duration::from_secs(3);

/// VibeSkua's "Misc Options" (`WM_SKUA_SET_OPTION`): label and option name.
pub const MISC_OPTIONS: [(&str, &str); 7] = [
    ("Lag Killer", "lagKiller"),
    ("Headless Mode", "headlessMode"),
    ("Hide Players", "hidePlayers"),
    ("Disable FX", "disableFx"),
    ("Infinite Range", "infiniteRange"),
    ("Aggro Monsters", "aggroMonsters"),
    ("Streamer Mode", "streamerMode"),
];

/// A client a command, a Gear or a Logs window acts on: one Army launched (by username), one of
/// this window's tabs, or one of the shell's other tabs (in a client process, by the shell's
/// number; docs/TAB_PROCESSES_PROPOSAL.md, M5).
#[derive(Clone)]
pub enum Peer {
    Client {
        username: String,
        label: String,
    },
    Tab(Arc<Bot>),
    Shell {
        id: u32,
        label: String,
    },
    /// One the shell's Army launched (asked from a tab, so it outlives the tab's client), in a
    /// client: its commands go to its control API, its Close and Restart to the shell.
    ShellClient {
        username: String,
        label: String,
    },
}

/// The shell's Army client `username`, in a client.
fn shell_client(username: &str) -> Option<crate::turu::ShellClient> {
    crate::turu::shell_clients()
        .into_iter()
        .find(|client| client.username.eq_ignore_ascii_case(username))
}

/// A shell Army client's control API, while its process runs.
fn shell_client_api(username: &str) -> Result<(std::net::SocketAddr, String), String> {
    let client = shell_client(username).ok_or_else(|| format!("{username} isn't running"))?;
    match (client.api, &client.exited) {
        (Some(api), None) => Ok((api, client.token)),
        (_, Some(exit)) => Err(format!("{username} ended ({exit})")),
        (None, None) => Err(format!("{username} has no control API")),
    }
}

/// The shell's tab `id`, in a client.
fn shell_tab(id: u32) -> Option<crate::turu::ShellTab> {
    crate::turu::shell_tabs()?.0.into_iter().find(|tab| tab.id == id)
}

/// A shell tab's latest snapshot, from the monitor hub (which polls it).
fn shell_snapshot(id: u32) -> Option<Snapshot> {
    crate::turu::monitor_hub().snapshot(&format!("tab:{id}"))
}

/// A shell tab's control API, while its process runs.
fn shell_api(id: u32) -> Result<(std::net::SocketAddr, String), String> {
    let tab = shell_tab(id).ok_or_else(|| "that tab is closed".to_owned())?;
    match (tab.api, &tab.down) {
        (Some(api), None) => Ok((api, tab.token)),
        (_, Some(down)) => Err(format!("{} is {down}", tab.label)),
        (None, None) => Err(format!("{} has no control API", tab.label)),
    }
}

impl Peer {
    pub fn label(&self) -> String {
        match self {
            Self::Client { label, .. } => label.clone(),
            Self::Tab(bot) => format!("{} (tab)", crate::turu::tab_label(bot)),
            Self::Shell { label, .. } => format!("{label} (tab)"),
            Self::ShellClient { label, .. } => label.clone(),
        }
    }

    /// The login its gear file is under ("" before a tab's first login).
    pub fn username(&self) -> String {
        match self {
            Self::Client { username, .. } => username.clone(),
            Self::Tab(bot) => bot.butler.username(),
            Self::Shell { id, .. } => shell_snapshot(*id)
                .filter(|s| s.playing())
                .map(|s| s.name)
                .unwrap_or_default(),
            Self::ShellClient { username, .. } => username.clone(),
        }
    }

    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Client { username: a, .. }, Self::Client { username: b, .. }) => a.eq_ignore_ascii_case(b),
            (Self::Tab(a), Self::Tab(b)) => Arc::ptr_eq(a, b),
            (Self::Shell { id: a, .. }, Self::Shell { id: b, .. }) => a == b,
            (Self::ShellClient { username: a, .. }, Self::ShellClient { username: b, .. }) => a.eq_ignore_ascii_case(b),
            _ => false,
        }
    }

    /// Running and logged in.
    pub fn online(&self) -> bool {
        match self {
            Self::Client { username, .. } => {
                crate::turu::army().client(username).is_some()
                    && crate::turu::monitor_hub()
                        .army_snapshot(username)
                        .is_some_and(|s| s.playing())
            }
            Self::Tab(bot) => bot.gear.logged_in(),
            Self::Shell { id, .. } => {
                crate::turu::monitor_hub().online(&format!("tab:{id}"))
                    && shell_snapshot(*id).is_some_and(|s| s.playing())
            }
            Self::ShellClient { username, .. } => {
                shell_client(username).is_some_and(|c| c.exited.is_none())
                    && crate::turu::monitor_hub()
                        .army_snapshot(username)
                        .is_some_and(|s| s.playing())
            }
        }
    }

    /// The running script's file name.
    pub fn running_script(&self) -> Option<String> {
        let (running, path) = match self {
            Self::Client { username, .. } | Self::ShellClient { username, .. } => {
                let snapshot = crate::turu::monitor_hub().army_snapshot(username)?;
                (
                    snapshot.script.running,
                    snapshot.script.path.map(std::path::PathBuf::from),
                )
            }
            Self::Tab(bot) => (bot.scripts.is_running(), bot.scripts.loaded()),
            Self::Shell { id, .. } => {
                let snapshot = shell_snapshot(*id)?;
                (
                    snapshot.script.running,
                    snapshot.script.path.map(std::path::PathBuf::from),
                )
            }
        };
        running.then(|| file_name(path.as_deref()))
    }

    /// Sends a command and waits for the answer: not on the UI thread.
    pub fn send(&self, command: &Command) -> Result<Value, String> {
        match self {
            Self::Client { username, .. } => {
                let army = crate::turu::army();
                let client = army
                    .client(username)
                    .ok_or_else(|| format!("{username} isn't running"))?;
                army.send(&client, command)
            }
            Self::Tab(bot) => turu_bot::army::send_local(bot, command),
            Self::Shell { id, .. } => {
                let (api, token) = shell_api(*id)?;
                turu_bot::army::send_to_api(api, &token, command)
            }
            Self::ShellClient { username, .. } => {
                let (api, token) = shell_client_api(username)?;
                turu_bot::army::send_to_api(api, &token, command)
            }
        }
    }

    /// A control API GET (`/log?...`): not on the UI thread.
    pub fn get(&self, path: &str) -> Result<Value, String> {
        match self {
            Self::Client { username, .. } => {
                let army = crate::turu::army();
                let client = army
                    .client(username)
                    .ok_or_else(|| format!("{username} isn't running"))?;
                army.forward(&client, false, path, Vec::new())
            }
            Self::Tab(bot) => {
                let (status, reply) = turu_bot::control_api::local(bot, false, path, &[]);
                turu_bot::army::reply_result(status, reply)
            }
            Self::Shell { id, .. } => {
                let (api, token) = shell_api(*id)?;
                turu_bot::army::get_from_api(api, &token, path)
            }
            Self::ShellClient { username, .. } => {
                let (api, token) = shell_client_api(username)?;
                turu_bot::army::get_from_api(api, &token, path)
            }
        }
    }
}

/// What a row's own buttons ask for (Gear and Logs are windows: each shell opens its own).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    Start,
    Stop,
    Login,
    Logout,
    Relog,
    Restart,
    Close,
    AutoAttack,
    StopAuto,
}

impl RowAction {
    /// The action named `name` (`start`, `stop`, `login`, `logout`, `relog`, `restart`, `close`,
    /// `auto-attack`, `stop-auto`).
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "start" => Self::Start,
            "stop" => Self::Stop,
            "login" => Self::Login,
            "logout" => Self::Logout,
            "relog" => Self::Relog,
            "restart" => Self::Restart,
            "close" => Self::Close,
            "auto-attack" => Self::AutoAttack,
            "stop-auto" => Self::StopAuto,
            _ => return None,
        })
    }
}

/// One line of the list: a tab or a launched client.
#[derive(Clone)]
pub struct Row {
    /// Its key in the checked set: the same whichever slot answers.
    pub key: String,
    pub peer: Peer,
    /// This process's tab.
    pub tab: Option<TabId>,
    /// The shell's number for it, when it's one of the shell's tabs (this process's own too).
    pub shell_tab: Option<u32>,
    /// The login, for group selection ("" before a tab's first login).
    pub username: String,
    pub headless: bool,
    /// Its state mark: waiting for the user, playing, ended, or quiet.
    pub tint: Tint,
    pub summary: String,
    pub error: Option<String>,
    /// The loaded script's file name, and whether it runs.
    pub script: Option<(String, bool)>,
    /// "Auto Attack" or "Auto Hunt" while one runs.
    pub auto: Option<String>,
    pub logged_in: bool,
    /// The process ended.
    pub ended: bool,
}

impl Row {
    fn of_tab(tab: TabId, bot: &Arc<Bot>, me: Option<&crate::turu::ShellTab>) -> Self {
        let status = bot.scripts.status();
        let auto = bot.auto.mode().map(|m| m.label().to_owned());
        let logged_in = bot.gear.logged_in();
        let waits = bot.scripts.waits_for();
        // Its monitor snapshot, as the other slots read this tab: a client reads the same from
        // every tab (place, HP, a green dot while playing).
        // It's sampled only while something watches the tab: an old one would show an old place.
        let fresh = |s: &Snapshot| turu_bot::gear::now_ms().saturating_sub(s.sampled_at) < 10_000;
        let snapshot = bot.monitor.latest().filter(fresh);
        let playing = snapshot.as_ref().map_or(logged_in, Snapshot::playing);
        let summary = match (waits, &snapshot, bot.state.lock().last_map.clone()) {
            (Some(what), _, _) => format!("waits for you: {what} (Show)"),
            (None, Some(snapshot), _) => summarize(snapshot),
            (None, None, Some(map)) if logged_in => map,
            (None, None, _) if logged_in => "logged in".into(),
            (None, None, _) => "login screen".into(),
        };
        Self {
            // In a shell's client, under the shell's number, as the other slots name it.
            key: me.map_or_else(|| format!("tab:{tab:?}"), crate::turu::ShellTab::hub_id),
            peer: Peer::Tab(bot.clone()),
            tab: Some(tab),
            shell_tab: me.map(|me| me.id),
            username: bot.butler.username(),
            headless: false,
            tint: match () {
                _ if waits.is_some() => Tint::Warning,
                _ if playing => Tint::Success,
                _ => Tint::Muted,
            },
            summary,
            error: None,
            script: bot.scripts.loaded().map(|p| (file_name(Some(&p)), status.is_active())),
            auto,
            logged_in,
            ended: false,
        }
    }

    /// Its game comes from the monitor's hub, which the Army poll feeds. The `n`th client is
    /// "Client n" while its Streamer Mode is on.
    fn of_client(n: usize, client: &Arc<Client>) -> Self {
        let state = client.state();
        let snapshot = crate::turu::monitor_hub().army_snapshot(&client.username);
        let streamer = snapshot.as_ref().is_some_and(|s| s.streamer);
        let (tint, summary) = match (&state.exited, state.answered, &snapshot) {
            (Some(exit), _, _) => (Tint::Danger, format!("ended ({exit})")),
            (None, true, Some(snapshot)) => (Tint::Success, summarize(snapshot)),
            (None, _, _) => (Tint::Muted, "starting…".to_owned()),
        };
        let live = snapshot.filter(|_| state.exited.is_none());
        Self {
            key: client.username.to_lowercase(),
            peer: Peer::Client {
                username: client.username.clone(),
                label: if streamer {
                    format!("Client {n}")
                } else {
                    client.label.clone()
                },
            },
            tab: None,
            shell_tab: None,
            username: client.username.clone(),
            headless: client.headless,
            tint,
            summary,
            error: state.error,
            script: script_of(live.as_ref()),
            auto: live.as_ref().and_then(|s| s.auto.clone()),
            logged_in: logged_in(live.as_ref()),
            ended: state.exited.is_some(),
        }
    }

    /// One of the clients the shell's Army launched: its game from the monitor's hub, which polls
    /// its process (`turu::shell_peers`).
    fn of_shell_client(n: usize, client: &crate::turu::ShellClient) -> Self {
        let snapshot = crate::turu::monitor_hub()
            .army_snapshot(&client.username)
            .filter(|_| client.exited.is_none());
        let streamer = snapshot.as_ref().is_some_and(|s| s.streamer);
        let (tint, summary) = match (&client.exited, &snapshot) {
            (Some(exit), _) => (Tint::Danger, format!("ended ({exit})")),
            (None, Some(snapshot)) => (Tint::Success, summarize(snapshot)),
            (None, None) => (Tint::Muted, "starting…".to_owned()),
        };
        Self {
            key: client.username.to_lowercase(),
            peer: Peer::ShellClient {
                username: client.username.clone(),
                label: if streamer {
                    format!("Client {n}")
                } else {
                    client.label.clone()
                },
            },
            tab: None,
            shell_tab: None,
            username: client.username.clone(),
            headless: client.headless,
            tint,
            summary,
            error: None,
            script: script_of(snapshot.as_ref()),
            auto: snapshot.as_ref().and_then(|s| s.auto.clone()),
            logged_in: logged_in(snapshot.as_ref()),
            ended: client.exited.is_some(),
        }
    }

    /// One of the shell's other tabs: its game from the monitor's hub, which polls its process.
    fn of_shell_tab(tab: &crate::turu::ShellTab) -> Self {
        let snapshot = shell_snapshot(tab.id).filter(|_| tab.down.is_none());
        let (tint, summary) = match (&tab.down, &snapshot) {
            (Some(down), _) => (Tint::Danger, down.clone()),
            (None, Some(snapshot)) if snapshot.playing() => (Tint::Success, summarize(snapshot)),
            (None, Some(_)) => (Tint::Muted, "login screen".to_owned()),
            (None, None) => (Tint::Muted, "starting…".to_owned()),
        };
        Self {
            key: tab.hub_id(),
            peer: Peer::Shell {
                id: tab.id,
                label: tab.label.clone(),
            },
            tab: None,
            shell_tab: Some(tab.id),
            username: snapshot
                .as_ref()
                .filter(|s| s.playing())
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            headless: false,
            tint,
            summary,
            error: None,
            script: script_of(snapshot.as_ref()),
            auto: snapshot.as_ref().and_then(|s| s.auto.clone()),
            logged_in: logged_in(snapshot.as_ref()),
            ended: tab.down.is_some(),
        }
    }

    /// A tab of this window: in this process, or the shell's in another.
    pub fn is_tab(&self) -> bool {
        self.tab.is_some() || matches!(self.peer, Peer::Shell { .. })
    }

    pub fn label(&self) -> String {
        match &self.peer {
            Peer::Client { label, .. } => label.clone(),
            Peer::Tab(bot) => crate::turu::tab_label(bot),
            Peer::Shell { label, .. } => label.clone(),
            Peer::ShellClient { label, .. } => label.clone(),
        }
    }

    /// What it is beside its name: "tab", "no window", or nothing.
    pub fn kind(&self) -> Option<&'static str> {
        if self.is_tab() {
            Some("tab")
        } else if self.headless {
            Some("no window")
        } else {
            None
        }
    }

    /// Its own ▶ Start: a loaded script, logged in.
    pub fn start(&self) -> Control {
        match &self.script {
            None => Control::off("No script loaded"),
            Some(_) if !self.logged_in => Control::off("Log in first"),
            Some(_) => Control::on(),
        }
    }

    /// Its Start Auto-Attack: logged in, with no script running.
    pub fn auto_attack(&self) -> Control {
        match () {
            _ if !self.logged_in => Control::off("Log in first"),
            _ if self.script.as_ref().is_some_and(|(_, running)| *running) => Control::off("A script runs"),
            _ => Control::on(),
        }
    }
}

fn script_of(snapshot: Option<&Snapshot>) -> Option<(String, bool)> {
    let snapshot = snapshot?;
    let path = snapshot.script.path.as_deref()?;
    Some((file_name(Some(std::path::Path::new(path))), snapshot.script.running))
}

fn logged_in(snapshot: Option<&Snapshot>) -> bool {
    snapshot.is_some_and(|s| !matches!(s.state, BotState::NotLoaded | BotState::LoggedOut))
}

/// Every tab (in a shell's client: its own, then the shell's others), then every launched client.
pub fn rows() -> Vec<Row> {
    let shell_tabs = crate::turu::shell_tabs().map(|(tabs, _)| tabs).unwrap_or_default();
    let me = shell_tabs.iter().find(|tab| tab.me);
    // A shell's client has one tab: the shell's `me`.
    let tabs = crate::turu::tabs()
        .into_iter()
        .map(|(tab, bot)| Row::of_tab(tab, &bot, me));
    let others = shell_tabs.iter().filter(|tab| !tab.me).map(Row::of_shell_tab);
    let clients = crate::turu::army().clients();
    let shell_clients = crate::turu::shell_clients();
    let own = clients.len();
    tabs.chain(others)
        .chain(clients.iter().enumerate().map(|(i, c)| Row::of_client(i + 1, c)))
        .chain(
            shell_clients
                .iter()
                .enumerate()
                .map(|(i, c)| Row::of_shell_client(own + i + 1, c)),
        )
        .collect()
}

/// The rows commands go to: the checked ones, else all that run.
pub fn targets(rows: &[Row], checked: &HashSet<String>) -> Vec<usize> {
    let everyone = checked.is_empty();
    (0..rows.len())
        .filter(|&i| !rows[i].ended && (everyone || checked.contains(&rows[i].key)))
        .collect()
}

/// Whether a command to these targets should be asked twice (Stop script, Log out): none checked
/// and more than one row would get it.
pub fn asks_twice(checked: &HashSet<String>, targets: usize) -> bool {
    checked.is_empty() && targets > 1
}

/// The outcomes, oldest first.
static OUTCOMES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

pub fn results() -> Vec<String> {
    OUTCOMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

pub fn clear_results() {
    OUTCOMES.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

pub fn push_result(line: String) {
    let mut results = OUTCOMES.lock().unwrap_or_else(|e| e.into_inner());
    if results.len() == RESULTS {
        results.pop_front();
    }
    results.push_back(line);
}

/// Sends the command to these rows at once; the outcomes are listed ([`results`]).
pub fn send_to(rows: &[Row], command: Command) {
    let label = command.label();
    let mut local = Vec::new();
    let mut only = Vec::new();
    let mut shell = Vec::new();
    // The outcomes name a client by its launch label: "Client 2" instead in Streamer Mode.
    let mut shown = HashMap::new();
    for row in rows {
        match &row.peer {
            Peer::Tab(bot) => local.push((format!("{} (tab)", crate::turu::tab_label(bot)), bot.clone())),
            Peer::Client { username, label } => {
                only.push(username.clone());
                if let Some(client) = crate::turu::army().client(username) {
                    shown.insert(client.label.clone(), label.clone());
                }
            }
            Peer::Shell { .. } | Peer::ShellClient { .. } => shell.push(row.peer.clone()),
        }
    }
    for peer in shell {
        let (label, command) = (label.clone(), command.clone());
        let spawned = std::thread::Builder::new()
            .name("Turu Army Command".into())
            .spawn(move || push_result(outcome_line(&peer.label(), &label, peer.send(&command))));
        if let Err(e) = spawned {
            push_result(format!("Army: {e}"));
        }
    }
    crate::turu::army().broadcast(command, local, Some(&only), move |name, result| {
        let name = shown.get(name).map_or(name, String::as_str);
        push_result(outcome_line(name, &label, result));
    });
}

/// Sends the command to the rows commands go to.
pub fn send(rows: &[Row], checked: &HashSet<String>, command: Command) {
    let targets: Vec<Row> = targets(rows, checked).into_iter().map(|i| rows[i].clone()).collect();
    send_to(&targets, command);
}

/// A row's own button; returns at once (the command runs on a thread of its own).
pub fn act(row: &Row, action: RowAction) {
    let peer = row.peer.clone();
    let command = match action {
        RowAction::Close => {
            match &peer {
                Peer::Client { username, .. } => crate::turu::army().close(username),
                Peer::ShellClient { username, .. } => {
                    crate::turu::shell_army(crate::turu::ShellArmyRequest::Close(username.clone()));
                }
                Peer::Tab(_) | Peer::Shell { .. } => {}
            }
            return;
        }
        RowAction::Start => Some(Command::StartScript(None)),
        RowAction::Stop => Some(Command::StopScript),
        RowAction::Login => Some(Command::Login),
        RowAction::Logout => Some(Command::Logout),
        RowAction::Relog => Some(Command::Relogin),
        RowAction::AutoAttack => Some(Command::StartAutoAttack),
        RowAction::StopAuto => Some(Command::StopAuto),
        RowAction::Restart => None,
    };
    let name = peer.label();
    let spawned = std::thread::Builder::new()
        .name("Turu Army Command".into())
        .spawn(move || {
            let line = match (command, &peer) {
                (Some(command), _) => outcome_line(&name, &command.label(), peer.send(&command)),
                (None, Peer::Client { username, .. }) => outcome_line(
                    &name,
                    "Restart",
                    crate::turu::army().restart(username).map(|_| Value::Null),
                ),
                (None, Peer::ShellClient { username, .. }) => {
                    crate::turu::shell_army(crate::turu::ShellArmyRequest::Restart(username.clone()));
                    outcome_line(&name, "Restart", Ok(Value::Null))
                }
                (None, Peer::Tab(_) | Peer::Shell { .. }) => return,
            };
            push_result(line);
        });
    if let Err(e) = spawned {
        push_result(format!("Army: {e}"));
    }
}

/// Army > Accounts' groups, read again every few seconds.
pub fn groups() -> Vec<turu_bot::accounts::Group> {
    static READ: Mutex<Option<(Instant, Vec<turu_bot::accounts::Group>)>> = Mutex::new(None);
    let mut read = READ.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, groups)) = read.as_ref()
        && at.elapsed() < GROUPS_EVERY
    {
        return groups.clone();
    }
    let groups = Accounts::load(&crate::turu::bot().paths.accounts_file()).groups;
    *read = Some((Instant::now(), groups.clone()));
    groups
}

/// The keys of a group's rows that still run.
pub fn group_keys(rows: &[Row], group: &turu_bot::accounts::Group) -> Vec<String> {
    rows.iter()
        .filter(|r| !r.ended && group.accounts.iter().any(|a| a.eq_ignore_ascii_case(&r.username)))
        .map(|r| r.key.clone())
        .collect()
}

/// Clear ended: drops the launched clients whose process ended.
pub fn clear_ended() {
    crate::turu::army().clear_exited();
}

/// Close all launched clients.
pub fn close_all() {
    crate::turu::army().close_all();
}

/// "Alt1: Equip Legion Revenant: done".
pub fn outcome_line(name: &str, label: &str, result: Result<Value, String>) -> String {
    match result {
        Ok(reply) => {
            // A client without a window started without its Script Options, and none were saved.
            let defaults = reply.get("options").and_then(Value::as_str) == Some("defaults")
                && reply.get("optionsWindow").and_then(Value::as_bool) == Some(false);
            match serde_json::from_value::<turu_bot::gear::EquipReport>(reply) {
                Ok(report) if label.starts_with("Equip ") => {
                    format!("{name}: {label}: {}", report.summary())
                }
                _ if defaults => {
                    format!("{name}: {label}: done, with the default Script Options (none saved)")
                }
                _ => format!("{name}: {label}: done"),
            }
        }
        // The client said no (a script runs, not logged in, ...): nothing started there.
        Err(e) if label == Command::StartAutoAttack.label() => {
            format!("{name}: {label}: skipped ({e})")
        }
        Err(e) => format!("{name}: {label}: {e}"),
    }
}

/// The file name of a script path ("" for none).
pub fn file_name(path: Option<&std::path::Path>) -> String {
    path.and_then(|p| p.file_name())
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// "battleon, Enter · 2400/2400 HP", "login screen", "relogging", ...
pub fn summarize(snapshot: &Snapshot) -> String {
    if snapshot.relogging {
        return "relogging".into();
    }
    let map = match snapshot.state {
        BotState::NotLoaded => return "loading the game".into(),
        BotState::LoggedOut => return "login screen".into(),
        _ if snapshot.map.is_empty() => return "joining".into(),
        // Streamer Mode: the map without its room number.
        _ if snapshot.streamer => snapshot.map.split('-').next().unwrap_or_default(),
        _ => &snapshot.map,
    };
    let mut text = format!("{map}, {} · {}/{} HP", snapshot.cell, snapshot.hp.cur, snapshot.hp.max);
    if !snapshot.server.is_empty() {
        text = format!("{} · {text}", snapshot.server);
    }
    text
}

// The view a shell reads.

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ArmyView {
    pub rows: Vec<ArmyRow>,
    /// Accounts waiting for their turn to start.
    pub waiting: Vec<String>,
    /// Army > Accounts' groups, each with its rows' keys.
    pub groups: Vec<ArmyGroup>,
    /// Some rows are launched clients (Clear ended, Close all).
    pub launched: bool,
    /// The commands' outcomes, oldest first.
    pub results: Vec<String>,
    /// Misc options' switches, as (label, name) pairs for `option`.
    pub options: Vec<ArmyOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ArmyRow {
    pub key: String,
    pub label: String,
    /// "tab", "no window", or empty.
    pub kind: String,
    pub tint: Tint,
    /// Where it is ("Artix · battleon, Enter · 2400/2400 HP"), or its state ("login screen").
    pub summary: String,
    /// Why it couldn't start; empty when nothing went wrong.
    pub error: String,
    /// The loaded script's file name; empty with none.
    pub script: String,
    pub running: bool,
    /// "Auto Attack" or "Auto Hunt" while one runs; empty otherwise.
    pub auto: String,
    pub logged_in: bool,
    pub ended: bool,
    /// The shell's number for it while it's one of the shell's tabs ("Show its tab"); -1 otherwise.
    pub shell_tab: i64,
    pub start: Control,
    pub auto_attack: Control,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ArmyGroup {
    pub name: String,
    pub accounts: u32,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ArmyOption {
    pub label: String,
    pub name: String,
}

/// The view, for a shell: reading it counts as Army Control being open, so the shell's other tabs
/// are polled while a page reads it.
pub fn view() -> ArmyView {
    crate::turu::army_control_drawn();
    let rows = rows();
    let groups = groups()
        .iter()
        .map(|g| ArmyGroup {
            name: g.name.clone(),
            accounts: g.accounts.len() as u32,
            keys: group_keys(&rows, g),
        })
        .collect();
    ArmyView {
        waiting: crate::turu::army().queued(),
        groups,
        launched: rows.iter().any(|r| !r.is_tab()),
        results: results(),
        options: MISC_OPTIONS
            .iter()
            .map(|(label, name)| ArmyOption {
                label: (*label).to_owned(),
                name: (*name).to_owned(),
            })
            .collect(),
        rows: rows.iter().map(row_view).collect(),
    }
}

fn row_view(row: &Row) -> ArmyRow {
    let (script, running) = row.script.clone().unwrap_or_default();
    ArmyRow {
        key: row.key.clone(),
        label: row.label(),
        kind: row.kind().unwrap_or_default().to_owned(),
        tint: row.tint,
        summary: row.summary.clone(),
        error: row.error.clone().unwrap_or_default(),
        script,
        running,
        auto: row.auto.clone().unwrap_or_default(),
        logged_in: row.logged_in,
        ended: row.ended,
        shell_tab: row.shell_tab.map_or(-1, i64::from),
        start: row.start(),
        auto_attack: row.auto_attack(),
    }
}

/// The checked keys in a shell's action (`keys`: none means every row still running).
fn checked(body: &Value) -> HashSet<String> {
    body.get("keys")
        .and_then(Value::as_array)
        .map(|keys| keys.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// `send`: a command to the checked rows (`keys`), or to every row still running. `command` is
/// `start-script`, `stop-script`, `login`, `logout`, `relog`, `house`, `auto-attack`,
/// `stop-auto`, `script` (`path`: a Search Scripts path, downloaded first when it needs it),
/// `tab-script` (the script `bot`'s tab has loaded), `quest` (`id`, optional `reward`), `option`
/// (`name`, `on`), `jump` (`map`, `room`, `cell`, `pad`: remembered for Map Travel), `jump-player`
/// (`name`) or `change-server` (`server`).
pub fn command(bot: &Arc<Bot>, body: &Value) -> Result<(), String> {
    let name = body
        .get("command")
        .and_then(Value::as_str)
        .ok_or("command is missing")?;
    let rows = rows();
    let reach = !targets(&rows, &checked(body)).is_empty();
    let travel = &super::army_travel::TRAVEL;
    let command = match name {
        "start-script" => Command::StartScript(None),
        "stop-script" => Command::StopScript,
        "login" => Command::Login,
        "logout" => Command::Logout,
        "relog" => Command::Relogin,
        "house" => Command::JumpHouse,
        "auto-attack" => Command::StartAutoAttack,
        "stop-auto" => Command::StopAuto,
        "script" => return load_from_repo(&text(body, "path")?, &checked(body)),
        "quest" => {
            let id = body
                .get("id")
                .and_then(Value::as_i64)
                .filter(|id| *id > 0)
                .ok_or("Enter a quest ID")?;
            let reward = body.get("reward").and_then(Value::as_i64).unwrap_or(-1);
            Command::AcceptQuest { id, reward }
        }
        "option" => {
            let option = text(body, "name")?;
            if !MISC_OPTIONS.iter().any(|(_, n)| *n == option) {
                return Err(format!("no option named {option}"));
            }
            let on = body.get("on").and_then(Value::as_bool).ok_or("on is missing")?;
            Command::SetOptions(json!({ option: on }))
        }
        "tab-script" => {
            let path = bot.scripts.loaded().ok_or("Load a script in this tab's Script Loader first")?;
            Command::LoadScript(path.display().to_string())
        }
        "jump" | "jump-player" | "change-server" if !reach => return Err("None of the checked clients runs".into()),
        "jump" => {
            let destination = super::army_travel::destination(body);
            if destination.map.is_empty() {
                return Err("Type a map to go to".into());
            }
            travel.jump(&destination)
        }
        "jump-player" => {
            let player = text(body, "name")?;
            if player.trim().is_empty() {
                return Err("Enter a player's name".into());
            }
            travel.jump_player(&player)
        }
        "change-server" => {
            let server = text(body, "server")?.trim().to_owned();
            if server.is_empty() {
                return Err("Pick a server".into());
            }
            Command::ChangeServer(server)
        }
        other => return Err(format!("no command named {other}")),
    };
    if !reach {
        return Err("None of the checked clients runs".into());
    }
    send(&rows, &checked(body), command);
    Ok(())
}

/// Loads Search Scripts' script at `path` in the rows commands go to: at once when it's
/// downloaded and current, else once the repository's task has downloaded it (as the egui Search
/// Scripts' pick mode does). The outcomes are listed as for any command.
pub fn load_from_repo(path: &str, checked: &HashSet<String>) -> Result<(), String> {
    let rows = rows();
    let targets: Vec<Row> = targets(&rows, checked).into_iter().map(|i| rows[i].clone()).collect();
    if targets.is_empty() {
        return Err("None of the checked clients runs".into());
    }
    let repo = crate::turu::script_repo();
    if let Some(local) = repo.ready(path) {
        send_to(&targets, Command::LoadScript(local.display().to_string()));
        return Ok(());
    }
    let owned = path.to_owned();
    let spawned = repo.spawn("Download", move |repo| {
        let local = repo.ensure(&owned)?;
        let count = targets.len();
        send_to(&targets, Command::LoadScript(local.display().to_string()));
        Ok(format!("Downloaded {owned}; loading it in {count} clients."))
    });
    if spawned {
        Ok(())
    } else {
        Err(super::repo::wait_tip(&repo.snapshot()))
    }
}

/// `row`: one row's own button (`key`, `action`: see [`RowAction::named`]).
pub fn row(body: &Value) -> Result<(), String> {
    let key = text(body, "key")?;
    let name = text(body, "action")?;
    let action = RowAction::named(&name).ok_or_else(|| format!("no action named {name}"))?;
    let rows = rows();
    let row = rows.iter().find(|r| r.key == key).ok_or("That client is gone")?;
    act(row, action);
    Ok(())
}

fn text(body: &Value, name: &str) -> Result<String, String> {
    body.get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{name} is missing"))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn row(name: &str, ended: bool, script: Option<(&str, bool)>) -> Row {
        Row {
            key: name.to_lowercase(),
            peer: Peer::Client {
                username: name.into(),
                label: name.into(),
            },
            tab: None,
            shell_tab: None,
            username: name.into(),
            headless: true,
            tint: Tint::Success,
            summary: "Artix · ultradage-123456, r6, Left · 120000/120000 HP".into(),
            error: None,
            script: script.map(|(n, running)| (n.into(), running)),
            auto: None,
            logged_in: !ended,
            ended,
        }
    }

    /// Commands go to the checked rows, or to every row still running when none is checked.
    #[test]
    fn actions_go_to_the_checked_rows_or_everyone() {
        let rows = [
            row("Alt1", false, None),
            row("Alt2", false, None),
            row("Gone", true, None),
        ];
        let mut checked = HashSet::new();
        assert_eq!(targets(&rows, &checked), [0, 1]);
        assert!(asks_twice(&checked, 2));
        checked.insert("alt2".into());
        assert_eq!(targets(&rows, &checked), [1]);
        assert!(!asks_twice(&checked, 1));
        // A checked row that ended isn't sent to.
        let checked = ["gone".to_owned()].into();
        assert!(targets(&rows, &checked).is_empty());
    }

    #[test]
    fn a_row_says_why_it_cant_start() {
        assert_eq!(row("A", false, None).start(), Control::off("No script loaded"));
        let mut out = row("A", false, Some(("Farm.ts", false)));
        assert!(out.start().enabled);
        out.logged_in = false;
        assert_eq!(out.start(), Control::off("Log in first"));
        assert_eq!(
            row("A", false, Some(("Farm.ts", true))).auto_attack(),
            Control::off("A script runs")
        );
    }

    #[test]
    fn a_row_views_flat() {
        let view = row_view(&row("Alt1", false, Some(("Farm.ts", true))));
        assert_eq!(view.script, "Farm.ts");
        assert!(view.running);
        assert_eq!(view.kind, "no window");
        assert_eq!(view.shell_tab, -1);
        assert_eq!(view.error, "");
    }

    #[test]
    fn options_and_commands_are_checked() {
        let bot = Bot::new();
        let err = command(&bot, &json!({ "command": "option", "name": "nope", "on": true }));
        assert_eq!(err, Err("no option named nope".into()));
        assert_eq!(
            command(&bot, &json!({ "command": "quest", "id": 0 })),
            Err("Enter a quest ID".into())
        );
        assert!(command(&bot, &json!({ "command": "dance" })).is_err());
        assert_eq!(
            command(&bot, &json!({ "command": "tab-script" })),
            Err("Load a script in this tab's Script Loader first".into())
        );
        assert_eq!(
            command(&bot, &json!({ "command": "jump", "map": " " })),
            Err("Type a map to go to".into())
        );
        assert_eq!(RowAction::named("auto-attack"), Some(RowAction::AutoAttack));
        assert_eq!(RowAction::named("gear"), None);
    }
}
