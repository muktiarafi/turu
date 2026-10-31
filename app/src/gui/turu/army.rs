//! Army > Army Control (VibeSkua's tabbed host "Army Control" menu): the clients Accounts
//! launched, and commands sent to them (and to this window's tabs) through their control API:
//! to all, to a group of Army > Accounts, or to the checked rows. Each row's ⋯ menu acts on that
//! client alone, and opens its Gear and Logs windows.

use super::client_logs::ClientLogs;
use super::files::{self, Pending};
use super::gear::GearWindow;
use super::theme::{DANGER, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use crate::gui::FilePicker;
use crate::turu::TabId;
use egui::{Align, Context, Layout, RichText, ScrollArea, Ui, vec2};
use rfd::AsyncFileDialog;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::accounts::{Accounts, Group};
use turu_bot::army::{Client, Command};
use turu_bot::monitor::Snapshot;
use turu_bot::monitor::snapshot::BotState;

/// How many command outcomes the window keeps.
const RESULTS: usize = 40;

/// VibeSkua's "Misc Options" (`WM_SKUA_SET_OPTION`): label and option name.
const MISC_OPTIONS: [(&str, &str); 7] = [
    ("Lag Killer", "lagKiller"),
    ("Headless Mode", "headlessMode"),
    ("Hide Players", "hidePlayers"),
    ("Disable FX", "disableFx"),
    ("Infinite Range", "infiniteRange"),
    ("Aggro Monsters", "aggroMonsters"),
    ("Streamer Mode", "streamerMode"),
];

/// A client a Gear or Logs window acts on: one Army launched (by username), or one of this
/// window's tabs.
#[derive(Clone)]
pub enum Peer {
    Client { username: String, label: String },
    Tab(Arc<Bot>),
}

impl Peer {
    pub fn label(&self) -> String {
        match self {
            Self::Client { label, .. } => label.clone(),
            Self::Tab(bot) => format!("{} (tab)", crate::turu::tab_label(bot)),
        }
    }

    /// The login its gear file is under ("" before a tab's first login).
    pub fn username(&self) -> String {
        match self {
            Self::Client { username, .. } => username.clone(),
            Self::Tab(bot) => bot.butler.username(),
        }
    }

    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Client { username: a, .. }, Self::Client { username: b, .. }) => a.eq_ignore_ascii_case(b),
            (Self::Tab(a), Self::Tab(b)) => Arc::ptr_eq(a, b),
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
        }
    }

    /// The running script's file name.
    pub fn running_script(&self) -> Option<String> {
        let (running, path) = match self {
            Self::Client { username, .. } => {
                let snapshot = crate::turu::monitor_hub().army_snapshot(username)?;
                (
                    snapshot.script.running,
                    snapshot.script.path.map(std::path::PathBuf::from),
                )
            }
            Self::Tab(bot) => (bot.scripts.is_running(), bot.scripts.loaded()),
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
        }
    }
}

/// Who the commands go to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum Target {
    #[default]
    All,
    /// The accounts of an Army > Accounts group.
    Group(String),
    /// The rows checked in the list.
    Checked,
}

/// What a row's ⋯ menu asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Start,
    Stop,
    Relog,
    Gear,
    Logs,
    Restart,
}

/// A row's key in the checked set.
fn client_key(username: &str) -> String {
    username.to_lowercase()
}

fn tab_key(tab: TabId) -> String {
    format!("tab:{tab:?}")
}

pub struct ArmyWindow {
    /// Commands also go to this window's tabs.
    include_tabs: bool,
    target: Target,
    checked: HashSet<String>,
    /// Army > Accounts' groups, read again every few seconds.
    groups: Vec<Group>,
    groups_read: Option<Instant>,
    gear: Vec<GearWindow>,
    logs: Vec<ClientLogs>,
    map: String,
    cell: String,
    player: String,
    quest: String,
    reward: String,
    misc: [bool; MISC_OPTIONS.len()],
    pending_script: Pending,
    results: Arc<Mutex<VecDeque<String>>>,
}

impl Default for ArmyWindow {
    fn default() -> Self {
        Self {
            include_tabs: true,
            target: Target::All,
            checked: HashSet::new(),
            groups: Vec::new(),
            groups_read: None,
            gear: Vec::new(),
            logs: Vec::new(),
            map: String::new(),
            cell: String::new(),
            player: String::new(),
            quest: String::new(),
            reward: String::new(),
            misc: [false; MISC_OPTIONS.len()],
            pending_script: Pending::default(),
            results: Arc::default(),
        }
    }
}

impl ArmyWindow {
    pub fn show(&mut self, ctx: &Context, picker: &FilePicker, open: &mut bool) {
        if let Some(path) = self.pending_script.poll() {
            self.send(Command::LoadScript(path.display().to_string()));
        }
        self.gear.retain(|w| w.open);
        for window in &mut self.gear {
            window.show(ctx);
        }
        self.logs.retain(|w| w.open);
        for window in &mut self.logs {
            window.show(ctx, picker);
        }
        if !*open {
            return;
        }
        if self.groups_read.is_none_or(|t| t.elapsed() >= Duration::from_secs(3)) {
            self.groups_read = Some(Instant::now());
            self.groups = Accounts::load(&crate::turu::bot().paths.accounts_file()).groups;
            if let Target::Group(name) = &self.target
                && !self.groups.iter().any(|g| &g.name == name)
            {
                self.target = Target::All;
            }
        }
        super::window(ctx, "Army Control", [560.0, 600.0], 5)
            .open(open)
            .show(ctx, |ui| self.ui(ui, picker));
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// Sends the command to the target's clients (and tabs, when included); outcomes are
    /// listed.
    fn send(&self, command: Command) {
        let label = command.label();
        let results = self.results.clone();
        let tabs = crate::turu::tabs().into_iter().filter(|(tab, bot)| match &self.target {
            Target::All => self.include_tabs,
            Target::Group(name) => {
                self.include_tabs
                    && self
                        .group_members(name)
                        .iter()
                        .any(|u| u.eq_ignore_ascii_case(&bot.butler.username()))
            }
            Target::Checked => self.checked.contains(&tab_key(*tab)),
        });
        let local = tabs
            .map(|(_, bot)| (format!("{} (tab)", crate::turu::tab_label(&bot)), bot))
            .collect();
        let only = match &self.target {
            Target::All => None,
            Target::Group(name) => Some(self.group_members(name)),
            Target::Checked => Some(
                self.checked
                    .iter()
                    .filter(|k| !k.starts_with("tab:"))
                    .cloned()
                    .collect(),
            ),
        };
        crate::turu::army().broadcast(command, local, only.as_deref(), move |name, result| {
            push_result(&results, outcome_line(name, &label, result));
        });
    }

    fn group_members(&self, name: &str) -> Vec<String> {
        self.groups
            .iter()
            .find(|g| g.name == name)
            .map(|g| g.accounts.clone())
            .unwrap_or_default()
    }

    /// A row's ⋯ choice for one client or tab.
    fn act(&mut self, peer: Peer, action: RowAction) {
        let command = match action {
            RowAction::Gear => {
                match self.gear.iter_mut().find(|w| w.peer.same(&peer)) {
                    Some(window) => window.reopen(),
                    None => self.gear.push(GearWindow::new(peer)),
                }
                return;
            }
            RowAction::Logs => {
                if let Some(window) = self.logs.iter_mut().find(|w| w.peer.same(&peer)) {
                    window.open = true;
                } else {
                    self.logs.push(ClientLogs::new(peer));
                }
                return;
            }
            RowAction::Start => Some(Command::StartScript(None)),
            RowAction::Stop => Some(Command::StopScript),
            RowAction::Relog => Some(Command::Relogin),
            RowAction::Restart => None,
        };
        let results = self.results.clone();
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
                    (None, Peer::Tab(_)) => return,
                };
                push_result(&results, line);
            });
        if let Err(e) = spawned {
            push_result(&self.results, format!("Army: {e}"));
        }
    }

    fn ui(&mut self, ui: &mut Ui, picker: &FilePicker) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        self.clients_card(ui);
        self.commands_card(ui, picker);
        self.results_card(ui);
    }

    fn clients_card(&mut self, ui: &mut Ui) {
        let army = crate::turu::army();
        let clients = army.clients();
        let queued = army.queued();
        let mut actions = Vec::new();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "CLIENTS");
                widgets::count_badge(ui, clients.iter().filter(|c| c.is_running()).count());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::small_button(ui, "Close All", !clients.is_empty()).clicked() {
                        army.close_all();
                    }
                    if clients.iter().any(|c| !c.is_running())
                        && widgets::small_button(ui, "Clear Ended", true).clicked()
                    {
                        army.clear_exited();
                    }
                });
            });
            ScrollArea::vertical()
                .id_salt("turu_army_clients")
                .max_height(180.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let checking = self.target == Target::Checked;
                    for (tab, bot) in crate::turu::tabs() {
                        let mut checked = self.checked.contains(&tab_key(tab));
                        if let Some(action) = tab_row(ui, tab, &bot, checking.then_some(&mut checked)) {
                            actions.push((Peer::Tab(bot.clone()), action));
                        }
                        set_checked(&mut self.checked, tab_key(tab), checked);
                    }
                    if clients.is_empty() && queued.is_empty() {
                        ui.label(RichText::new("No other clients: start accounts from Army > Accounts.").color(MUTED));
                    }
                    for client in &clients {
                        let key = client_key(&client.username);
                        let mut checked = self.checked.contains(&key);
                        if let Some(action) = client_row(ui, client, checking.then_some(&mut checked)) {
                            let peer = Peer::Client {
                                username: client.username.clone(),
                                label: client.label.clone(),
                            };
                            actions.push((peer, action));
                        }
                        set_checked(&mut self.checked, key, checked);
                    }
                    for username in &queued {
                        ui.label(RichText::new(format!("{username}: waiting to start")).color(MUTED));
                    }
                });
        });
        for (peer, action) in actions {
            self.act(peer, action);
        }
    }

    fn commands_card(&mut self, ui: &mut Ui, picker: &FilePicker) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "ARMY CONTROL");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.target != Target::Checked {
                        ui.label("Include this window's tabs");
                        widgets::switch(ui, &mut self.include_tabs);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Send to").color(MUTED));
                let shown = match &self.target {
                    Target::All => "All clients".to_owned(),
                    Target::Group(name) => format!("Group: {name}"),
                    Target::Checked => format!("Checked rows ({})", self.checked.len()),
                };
                egui::ComboBox::from_id_salt("turu_army_target")
                    .selected_text(shown)
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.target, Target::All, "All clients");
                        for group in &self.groups {
                            let target = Target::Group(group.name.clone());
                            let label = format!("Group: {} ({})", group.name, group.accounts.len());
                            ui.selectable_value(&mut self.target, target, label);
                        }
                        ui.selectable_value(&mut self.target, Target::Checked, "Checked rows");
                    });
                if self.target == Target::Checked {
                    ui.label(RichText::new("Check rows in CLIENTS").small().color(MUTED));
                }
            });
            let width = (ui.available_width() - 12.0) / 3.0;
            let size = vec2(width, 28.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, "Start All Scripts", Kind::Primary, true, size, "").clicked() {
                    self.send(Command::StartScript(None));
                }
                if widgets::button(ui, "Stop All Scripts", Kind::Danger, true, size, "").clicked() {
                    self.send(Command::StopScript);
                }
                let load = widgets::button(
                    ui,
                    "Load Script to All",
                    Kind::Normal,
                    !self.pending_script.is_open(),
                    size,
                    "",
                );
                if load.clicked() {
                    let start = files::scripts_start_dir(crate::turu::bot().paths.scripts_dir());
                    let dialog = AsyncFileDialog::new()
                        .set_title("Load Script to All")
                        .add_filter("Scripts", &["ts", "js"])
                        .set_directory(start);
                    self.pending_script.pick_file(picker, ui.ctx(), dialog);
                }
            });
            ui.horizontal(|ui| {
                if widgets::button(ui, "Login All Clients", Kind::Normal, true, size, "").clicked() {
                    self.send(Command::Login);
                }
                if widgets::button(ui, "Logout All Clients", Kind::Normal, true, size, "").clicked() {
                    self.send(Command::Logout);
                }
                if widgets::button(ui, "Jump All to House", Kind::Normal, true, size, "").clicked() {
                    self.send(Command::JumpHouse);
                }
            });
            ui.horizontal(|ui| {
                widgets::text_box(ui, &mut self.map, "Map, e.g. battleon-1e9", 170.0);
                widgets::text_box(ui, &mut self.cell, "Cell[, pad]", 110.0);
                let map = self.map.trim().to_owned();
                if widgets::button(
                    ui,
                    "Jump All to Map",
                    Kind::Normal,
                    !map.is_empty(),
                    vec2(0.0, 26.0),
                    "Enter a map",
                )
                .clicked()
                {
                    let (cell, pad) = match self.cell.split_once(',') {
                        Some((cell, pad)) => (cell.trim().to_owned(), pad.trim().to_owned()),
                        None => (self.cell.trim().to_owned(), String::new()),
                    };
                    self.send(Command::JumpMap { map, cell, pad });
                }
            });
            ui.horizontal(|ui| {
                widgets::text_box(ui, &mut self.player, "Player name", 170.0);
                let name = self.player.trim().to_owned();
                let goto = widgets::button(
                    ui,
                    "Jump All to Player",
                    Kind::Normal,
                    !name.is_empty(),
                    vec2(0.0, 26.0),
                    "Enter a name",
                );
                if goto.clicked() {
                    self.send(Command::JumpPlayer(name));
                }
            });
            ui.horizontal(|ui| {
                widgets::text_box(ui, &mut self.quest, "Quest ID", 80.0);
                widgets::text_box(ui, &mut self.reward, "Reward ID (optional)", 140.0);
                let id = self.quest.trim().parse::<i64>().ok().filter(|id| *id > 0);
                let accept = widgets::button(
                    ui,
                    "Accept Quest to All",
                    Kind::Normal,
                    id.is_some(),
                    vec2(0.0, 26.0),
                    "Enter a quest ID",
                );
                if accept.clicked()
                    && let Some(id) = id
                {
                    let reward = self.reward.trim().parse().unwrap_or(-1);
                    self.send(Command::AcceptQuest { id, reward });
                }
            });
            widgets::section_label(ui, "MISC OPTIONS");
            let mut changed = None;
            ui.horizontal_wrapped(|ui| {
                for (i, (label, name)) in MISC_OPTIONS.iter().enumerate() {
                    if ui.checkbox(&mut self.misc[i], *label).changed() {
                        changed = Some((*name, self.misc[i]));
                    }
                }
            });
            if let Some((name, on)) = changed {
                self.send(Command::SetOptions(json!({ name: on })));
            }
        });
    }

    fn results_card(&self, ui: &mut Ui) {
        let results: Vec<String> = self
            .results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "RESULTS");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::small_button(ui, "Clear", !results.is_empty()).clicked() {
                        self.results.lock().unwrap_or_else(|e| e.into_inner()).clear();
                    }
                });
            });
            ScrollArea::vertical()
                .id_salt("turu_army_results")
                .max_height(120.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for line in &results {
                        ui.label(RichText::new(line).monospace().size(11.0));
                    }
                });
        });
    }
}

/// One of this window's tabs: a dot while its script runs, its name, its script, Show and ⋯.
/// (Only what the bot keeps: asking its game would mean a Flash call from the UI thread.)
fn tab_row(ui: &mut Ui, tab: TabId, bot: &turu_bot::Bot, checked: Option<&mut bool>) -> Option<RowAction> {
    let status = bot.scripts.status();
    let active = tab == crate::turu::active_tab();
    let mut action = None;
    ui.horizontal(|ui| {
        if let Some(checked) = checked {
            ui.checkbox(checked, "");
        }
        widgets::dot(ui, if status.is_active() { SUCCESS } else { MUTED }, 4.0);
        ui.label(RichText::new(crate::turu::tab_label(bot)).strong().color(TEXT));
        widgets::pill(ui, |ui| ui.label(RichText::new("tab").size(10.0).color(MUTED)));
        let map = bot.state.lock().last_map.clone();
        if let Some(map) = map {
            ui.label(RichText::new(map).small().color(MUTED));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            action = row_menu(ui, false, status.is_active());
            if widgets::small_button(ui, "Show", !active).clicked() {
                crate::turu::request_tab(crate::turu::TabRequest::Select(tab));
            }
            if let Some(path) = bot.scripts.loaded() {
                let name = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let color = if status.is_active() { SUCCESS } else { MUTED };
                ui.label(RichText::new(name).monospace().size(11.0).color(color))
                    .on_hover_text(status.label());
            }
        });
    });
    action
}

/// One client: a dot, its name, where it is, its script, Close and ⋯. Its game comes from the
/// monitor's hub, which the Army poll feeds.
fn client_row(ui: &mut Ui, client: &Arc<Client>, checked: Option<&mut bool>) -> Option<RowAction> {
    let state = client.state();
    let snapshot = crate::turu::monitor_hub().army_snapshot(&client.username);
    let (color, summary) = match (&state.exited, state.answered, &snapshot) {
        (Some(exit), _, _) => (DANGER, format!("ended ({exit})")),
        (None, true, Some(snapshot)) => (SUCCESS, summarize(snapshot)),
        (None, _, _) => (MUTED, "starting…".to_owned()),
    };
    let mut action = None;
    ui.horizontal(|ui| {
        if let Some(checked) = checked {
            ui.checkbox(checked, "");
        }
        widgets::dot(ui, color, 4.0);
        ui.label(RichText::new(&client.label).strong().color(TEXT));
        if client.headless {
            widgets::pill(ui, |ui| ui.label(RichText::new("headless").size(10.0).color(MUTED)));
        }
        let summary = ui.label(RichText::new(summary).small().color(MUTED));
        if let Some(error) = &state.error {
            summary.on_hover_text(error);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let running = snapshot.as_ref().is_some_and(|s| s.script.running);
            action = row_menu(ui, true, running);
            if widgets::small_button(ui, "Close", true)
                .on_hover_text("End this client's process")
                .clicked()
            {
                crate::turu::army().close(&client.username);
            }
            if state.exited.is_none()
                && let Some(snapshot) = &snapshot
                && let Some(path) = &snapshot.script.path
            {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let color = if snapshot.script.running { SUCCESS } else { MUTED };
                ui.label(RichText::new(name).monospace().size(11.0).color(color))
                    .on_hover_text(&snapshot.script.status);
            }
        });
    });
    action
}

/// A row's ⋯ menu. Launched clients also get Logs (a tab has its own) and Restart.
fn row_menu(ui: &mut Ui, launched: bool, script_running: bool) -> Option<RowAction> {
    let mut action = None;
    ui.menu_button("⋯", |ui| {
        let mut item = |ui: &mut Ui, label: &str, enabled: bool, choice: RowAction| {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                action = Some(choice);
                ui.close();
            }
        };
        item(ui, "Start script", !script_running, RowAction::Start);
        item(ui, "Stop script", script_running, RowAction::Stop);
        item(ui, "Relog", true, RowAction::Relog);
        ui.separator();
        item(ui, "Gear…", true, RowAction::Gear);
        if launched {
            item(ui, "Logs…", true, RowAction::Logs);
            ui.separator();
            item(ui, "Restart", true, RowAction::Restart);
        }
    });
    action
}

fn set_checked(checked: &mut HashSet<String>, key: String, on: bool) {
    if on {
        checked.insert(key);
    } else {
        checked.remove(&key);
    }
}

/// "Alt1: Equip Legion Revenant: done".
fn outcome_line(name: &str, label: &str, result: Result<Value, String>) -> String {
    match result {
        Ok(reply) => match serde_json::from_value::<turu_bot::gear::EquipReport>(reply) {
            Ok(report) if label.starts_with("Equip ") => format!("{name}: {label}: {}", report.summary()),
            _ => format!("{name}: {label}: done"),
        },
        Err(e) => format!("{name}: {label}: {e}"),
    }
}

fn push_result(results: &Mutex<VecDeque<String>>, line: String) {
    let mut results = results.lock().unwrap_or_else(|e| e.into_inner());
    if results.len() == RESULTS {
        results.pop_front();
    }
    results.push_back(line);
}

/// The file name of a script path ("" for none).
fn file_name(path: Option<&std::path::Path>) -> String {
    path.and_then(|p| p.file_name())
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// "battleon, Enter · 2400/2400 HP", "login screen", "relogging", ...
fn summarize(snapshot: &Snapshot) -> String {
    if snapshot.relogging {
        return "relogging".into();
    }
    let map = match snapshot.state {
        BotState::NotLoaded => return "loading the game".into(),
        BotState::LoggedOut => return "login screen".into(),
        _ if snapshot.map.is_empty() => return "joining".into(),
        _ => &snapshot.map,
    };
    let mut text = format!("{map}, {} · {}/{} HP", snapshot.cell, snapshot.hp.cur, snapshot.hp.max);
    if !snapshot.server.is_empty() {
        text = format!("{} · {text}", snapshot.server);
    }
    text
}
