//! Army > Army Control (VibeSkua's tabbed host "Army Control" menu): one list of this window's
//! tabs and the clients Accounts launched, each with buttons of its own (script, log in/out,
//! Gear, and a ⋯ menu), and ACTIONS that go to the checked rows, or to every row when none is
//! checked, through their control API.

use super::client_logs::ClientLogs;
use super::files::{self, Pending};
use super::gear::GearWindow;
use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::gui::FilePicker;
use crate::turu::TabId;
use egui::{Align, CollapsingHeader, Context, Layout, RichText, ScrollArea, Ui, vec2};
use rfd::AsyncFileDialog;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::accounts::Accounts;
use turu_bot::army::{Client, Command};
use turu_bot::monitor::Snapshot;
use turu_bot::monitor::snapshot::BotState;

/// How many command outcomes the window keeps.
const RESULTS: usize = 40;
/// How long a "click again to confirm" stays armed.
const CONFIRM_FOR: Duration = Duration::from_secs(4);

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

/// What a row's buttons asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Start,
    Stop,
    Login,
    Logout,
    Relog,
    Gear,
    Logs,
    Restart,
    Close,
    AutoAttack,
    StopAuto,
}

/// One line of the list: a tab or a launched client, as the UI shows it.
#[derive(Clone)]
struct Row {
    /// Its key in the checked set.
    key: String,
    peer: Peer,
    tab: Option<TabId>,
    /// The login, for group selection ("" before a tab's first login).
    username: String,
    headless: bool,
    color: egui::Color32,
    summary: String,
    error: Option<String>,
    /// The loaded script's file name, and whether it runs.
    script: Option<(String, bool)>,
    /// "Auto Attack" or "Auto Hunt" while one runs.
    auto: Option<String>,
    logged_in: bool,
    /// The process ended.
    ended: bool,
}

impl Row {
    fn of_tab(tab: TabId, bot: &Arc<Bot>) -> Self {
        let status = bot.scripts.status();
        let auto = bot.auto.mode().map(|m| m.label().to_owned());
        let logged_in = bot.gear.logged_in();
        let waits = bot.scripts.waits_for();
        let summary = match (waits, bot.state.lock().last_map.clone()) {
            (Some(what), _) => format!("waits for you: {what} (Show)"),
            (None, Some(map)) if logged_in => map,
            (None, _) if logged_in => "logged in".into(),
            (None, _) => "login screen".into(),
        };
        Self {
            key: format!("tab:{tab:?}"),
            peer: Peer::Tab(bot.clone()),
            tab: Some(tab),
            username: bot.butler.username(),
            headless: false,
            color: match () {
                _ if waits.is_some() => WARNING,
                _ if status.is_active() || auto.is_some() => SUCCESS,
                _ => MUTED,
            },
            summary,
            error: None,
            script: bot.scripts.loaded().map(|p| (file_name(Some(&p)), status.is_active())),
            auto,
            logged_in,
            ended: false,
        }
    }

    /// Its game comes from the monitor's hub, which the Army poll feeds.
    fn of_client(client: &Arc<Client>) -> Self {
        let state = client.state();
        let snapshot = crate::turu::monitor_hub().army_snapshot(&client.username);
        let (color, summary) = match (&state.exited, state.answered, &snapshot) {
            (Some(exit), _, _) => (DANGER, format!("ended ({exit})")),
            (None, true, Some(snapshot)) => (SUCCESS, summarize(snapshot)),
            (None, _, _) => (MUTED, "starting…".to_owned()),
        };
        let live = snapshot.filter(|_| state.exited.is_none());
        Self {
            key: client.username.to_lowercase(),
            peer: Peer::Client {
                username: client.username.clone(),
                label: client.label.clone(),
            },
            tab: None,
            username: client.username.clone(),
            headless: client.headless,
            color,
            summary,
            error: state.error,
            script: live.as_ref().and_then(|s| {
                let path = s.script.path.as_deref()?;
                Some((file_name(Some(std::path::Path::new(path))), s.script.running))
            }),
            auto: live.as_ref().and_then(|s| s.auto.clone()),
            logged_in: live
                .as_ref()
                .is_some_and(|s| !matches!(s.state, BotState::NotLoaded | BotState::LoggedOut)),
            ended: state.exited.is_some(),
        }
    }

    fn label(&self) -> String {
        match &self.peer {
            Peer::Client { label, .. } => label.clone(),
            Peer::Tab(bot) => crate::turu::tab_label(bot),
        }
    }
}

pub struct ArmyWindow {
    /// The checked rows' keys; none checked means every row.
    checked: HashSet<String>,
    /// Army > Accounts' groups, read again every few seconds.
    groups: Vec<turu_bot::accounts::Group>,
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
    /// The rows the script being picked goes to.
    pending_targets: Option<Vec<Row>>,
    /// An action on every row waiting for its second click, and since when.
    confirm: Option<(&'static str, Instant)>,
    results: Arc<Mutex<VecDeque<String>>>,
}

impl Default for ArmyWindow {
    fn default() -> Self {
        Self {
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
            pending_targets: None,
            confirm: None,
            results: Arc::default(),
        }
    }
}

impl ArmyWindow {
    pub fn show(&mut self, ctx: &Context, picker: &FilePicker, open: &mut bool) {
        if let Some(path) = self.pending_script.poll()
            && let Some(targets) = self.pending_targets.take()
        {
            self.send_to(&targets, Command::LoadScript(path.display().to_string()));
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
        super::window(ctx, "Army Control", [560.0, 620.0], 5)
            .open(open)
            .show(ctx, |ui| self.ui(ui, picker));
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// Every tab, then every launched client.
    fn rows() -> Vec<Row> {
        let tabs = crate::turu::tabs().into_iter().map(|(tab, bot)| Row::of_tab(tab, &bot));
        tabs.chain(crate::turu::army().clients().iter().map(Row::of_client))
            .collect()
    }

    /// The rows ACTIONS go to: the checked ones, else all that run.
    fn targets(&self, rows: &[Row]) -> Vec<usize> {
        let everyone = self.checked.is_empty();
        (0..rows.len())
            .filter(|&i| !rows[i].ended && (everyone || self.checked.contains(&rows[i].key)))
            .collect()
    }

    /// Sends the command to these rows at once; outcomes are listed.
    fn send_to(&self, rows: &[Row], command: Command) {
        let label = command.label();
        let results = self.results.clone();
        let mut local = Vec::new();
        let mut only = Vec::new();
        for row in rows {
            match &row.peer {
                Peer::Tab(bot) => local.push((format!("{} (tab)", crate::turu::tab_label(bot)), bot.clone())),
                Peer::Client { username, .. } => only.push(username.clone()),
            }
        }
        crate::turu::army().broadcast(command, local, Some(&only), move |name, result| {
            push_result(&results, outcome_line(name, &label, result));
        });
    }

    /// Sends the command to the rows ACTIONS go to.
    fn send(&self, rows: &[Row], command: Command) {
        let targets: Vec<Row> = self.targets(rows).into_iter().map(|i| rows[i].clone()).collect();
        self.send_to(&targets, command);
    }

    /// A row's own button.
    fn act(&mut self, row: &Row, action: RowAction) {
        let peer = row.peer.clone();
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
            RowAction::Close => {
                if let Peer::Client { username, .. } = &peer {
                    crate::turu::army().close(username);
                    self.checked.remove(&row.key);
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
        let rows = Self::rows();
        self.checked
            .retain(|key| rows.iter().any(|r| &r.key == key && !r.ended));
        self.clients_card(ui, &rows);
        self.actions_card(ui, picker, &rows);
        self.results_card(ui);
    }

    fn clients_card(&mut self, ui: &mut Ui, rows: &[Row]) {
        let army = crate::turu::army();
        let queued = army.queued();
        let mut actions = Vec::new();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "CLIENTS");
                widgets::count_badge(ui, rows.iter().filter(|r| !r.ended).count());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let launched = rows.iter().any(|r| r.tab.is_none());
                    ui.add_enabled_ui(launched, |ui| {
                        ui.menu_button("⋯", |ui| {
                            if ui.button("Clear ended").clicked() {
                                army.clear_exited();
                                ui.close();
                            }
                            if ui
                                .button(RichText::new("Close all launched clients").color(DANGER))
                                .clicked()
                            {
                                army.close_all();
                                self.checked.retain(|k| k.starts_with("tab:"));
                                ui.close();
                            }
                        });
                    });
                    self.selection_buttons(ui, rows);
                });
            });
            ScrollArea::vertical()
                .id_salt("turu_army_clients")
                .max_height(260.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for row in rows {
                        let mut checked = self.checked.contains(&row.key);
                        if let Some(action) = row_ui(ui, row, &mut checked) {
                            actions.push((row, action));
                        }
                        if checked && !row.ended {
                            self.checked.insert(row.key.clone());
                        } else {
                            self.checked.remove(&row.key);
                        }
                    }
                    if rows.iter().all(|r| r.tab.is_some()) && queued.is_empty() {
                        ui.label(
                            RichText::new("Start more accounts from Army > Accounts.")
                                .small()
                                .color(MUTED),
                        );
                    }
                    for username in &queued {
                        ui.label(RichText::new(format!("{username}: waiting to start")).color(MUTED));
                    }
                });
        });
        for (row, action) in actions {
            self.act(row, action);
        }
    }

    /// All, None, and Group ▾ (an Army > Accounts group's rows).
    fn selection_buttons(&mut self, ui: &mut Ui, rows: &[Row]) {
        if self.groups_read.is_none_or(|t| t.elapsed() >= Duration::from_secs(3)) {
            self.groups_read = Some(Instant::now());
            self.groups = Accounts::load(&crate::turu::bot().paths.accounts_file()).groups;
        }
        let groups = self.groups.clone();
        ui.add_enabled_ui(!groups.is_empty(), |ui| {
            ui.menu_button("Group ▾", |ui| {
                for group in &groups {
                    if ui
                        .button(format!("{} ({})", group.name, group.accounts.len()))
                        .clicked()
                    {
                        self.checked = rows
                            .iter()
                            .filter(|r| !r.ended && group.accounts.iter().any(|a| a.eq_ignore_ascii_case(&r.username)))
                            .map(|r| r.key.clone())
                            .collect();
                        ui.close();
                    }
                }
            })
            .response
            .on_disabled_hover_text("No groups yet: make them in Army > Accounts");
        });
        if widgets::small_button(ui, "None", !self.checked.is_empty()).clicked() {
            self.checked.clear();
        }
        if widgets::small_button(ui, "All", true).clicked() {
            self.checked = rows.iter().filter(|r| !r.ended).map(|r| r.key.clone()).collect();
        }
    }

    fn actions_card(&mut self, ui: &mut Ui, picker: &FilePicker, rows: &[Row]) {
        let targets = self.targets(rows);
        let names: Vec<String> = targets.iter().map(|&i| rows[i].label()).collect();
        let everyone = self.checked.is_empty();
        if self.confirm.is_some_and(|(_, at)| at.elapsed() >= CONFIRM_FOR) {
            self.confirm = None;
        }
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "ACTIONS");
                let who = match (everyone, names.len()) {
                    (_, 0) => "no clients".to_owned(),
                    (true, 1) => "the only client".to_owned(),
                    (true, n) => format!("all {n} clients (check rows to pick)"),
                    (false, _) if names.len() <= 3 => names.join(", "),
                    (false, n) => format!("{n} selected"),
                };
                let color = if everyone && names.len() > 1 { WARNING } else { TEXT };
                ui.label(RichText::new(format!("→ {who}")).small().color(color));
            });
            let any = !targets.is_empty();
            let width = (ui.available_width() - 12.0) / 3.0;
            let size = vec2(width, 28.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, "Start script", Kind::Primary, any, size, "").clicked() {
                    self.send(rows, Command::StartScript(None));
                }
                if self.guarded(
                    ui,
                    "Stop script",
                    Kind::Danger,
                    any,
                    size,
                    everyone && targets.len() > 1,
                ) {
                    self.send(rows, Command::StopScript);
                }
                let load = widgets::button(
                    ui,
                    "Load script…",
                    Kind::Normal,
                    any && !self.pending_script.is_open(),
                    size,
                    "",
                );
                if load.clicked() {
                    self.pending_targets = Some(targets.iter().map(|&i| rows[i].clone()).collect());
                    let start = files::scripts_start_dir(crate::turu::bot().paths.scripts_dir());
                    let dialog = AsyncFileDialog::new()
                        .set_title("Load Script")
                        .add_filter("Scripts", &["ts", "js"])
                        .set_directory(start);
                    self.pending_script.pick_file(picker, ui.ctx(), dialog);
                }
            });
            let width = (ui.available_width() - 18.0) / 4.0;
            let size = vec2(width, 28.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, "Log in", Kind::Normal, any, size, "").clicked() {
                    self.send(rows, Command::Login);
                }
                if self.guarded(ui, "Log out", Kind::Normal, any, size, everyone && targets.len() > 1) {
                    self.send(rows, Command::Logout);
                }
                if widgets::button(ui, "Relog", Kind::Normal, any, size, "").clicked() {
                    self.send(rows, Command::Relogin);
                }
                if widgets::button(ui, "Go to house", Kind::Normal, any, size, "").clicked() {
                    self.send(rows, Command::JumpHouse);
                }
            });
            // VibeSkua's "Start/Stop Auto-Attack for All": no confirmation there, and stopping
            // loses nothing.
            let width = (ui.available_width() - 6.0) / 2.0;
            let size = vec2(width, 28.0);
            ui.horizontal(|ui| {
                let tip = "Each client's Auto Attack with its equipped class (rows running a script are skipped)";
                if widgets::button(ui, "Start Auto-Attack", Kind::Normal, any, size, "")
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.send(rows, Command::StartAutoAttack);
                }
                if widgets::button(ui, "Stop Auto-Attack", Kind::Danger, any, size, "")
                    .on_hover_text("Stops Auto Attack or Auto Hunt; scripts keep running")
                    .clicked()
                {
                    self.send(rows, Command::StopAuto);
                }
            });
            CollapsingHeader::new("Travel")
                .id_salt("turu_army_travel")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        widgets::text_box(ui, &mut self.map, "Map, e.g. battleon-1e9", 170.0);
                        widgets::text_box(ui, &mut self.cell, "Cell[, pad]", 110.0);
                        let map = self.map.trim().to_owned();
                        if widgets::button(
                            ui,
                            "Join",
                            Kind::Normal,
                            any && !map.is_empty(),
                            vec2(0.0, 26.0),
                            "Enter a map",
                        )
                        .clicked()
                        {
                            let (cell, pad) = match self.cell.split_once(',') {
                                Some((cell, pad)) => (cell.trim().to_owned(), pad.trim().to_owned()),
                                None => (self.cell.trim().to_owned(), String::new()),
                            };
                            self.send(rows, Command::JumpMap { map, cell, pad });
                        }
                    });
                    ui.horizontal(|ui| {
                        widgets::text_box(ui, &mut self.player, "Player name", 170.0);
                        let name = self.player.trim().to_owned();
                        if widgets::button(
                            ui,
                            "Go to player",
                            Kind::Normal,
                            any && !name.is_empty(),
                            vec2(0.0, 26.0),
                            "Enter a name",
                        )
                        .clicked()
                        {
                            self.send(rows, Command::JumpPlayer(name));
                        }
                    });
                });
            CollapsingHeader::new("Quest")
                .id_salt("turu_army_quest")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        widgets::text_box(ui, &mut self.quest, "Quest ID", 80.0);
                        widgets::text_box(ui, &mut self.reward, "Reward ID (optional)", 140.0);
                        let id = self.quest.trim().parse::<i64>().ok().filter(|id| *id > 0);
                        if widgets::button(
                            ui,
                            "Accept quest",
                            Kind::Normal,
                            any && id.is_some(),
                            vec2(0.0, 26.0),
                            "Enter a quest ID",
                        )
                        .clicked()
                            && let Some(id) = id
                        {
                            let reward = self.reward.trim().parse().unwrap_or(-1);
                            self.send(rows, Command::AcceptQuest { id, reward });
                        }
                    });
                });
            CollapsingHeader::new("Misc options")
                .id_salt("turu_army_misc")
                .show(ui, |ui| {
                    let mut changed = None;
                    ui.horizontal_wrapped(|ui| {
                        for (i, (label, name)) in MISC_OPTIONS.iter().enumerate() {
                            if ui.checkbox(&mut self.misc[i], *label).changed() {
                                changed = Some((*name, self.misc[i]));
                            }
                        }
                    });
                    if let Some((name, on)) = changed {
                        self.send(rows, Command::SetOptions(json!({ name: on })));
                    }
                });
        });
    }

    /// A button that, when `ask`, needs a second click within a few seconds ("… all? click
    /// again"). True when the action should run.
    fn guarded(
        &mut self,
        ui: &mut Ui,
        label: &'static str,
        kind: Kind,
        enabled: bool,
        size: egui::Vec2,
        ask: bool,
    ) -> bool {
        let armed = self.confirm.is_some_and(|(l, _)| l == label);
        let text = if armed {
            format!("{label} all?")
        } else {
            label.to_owned()
        };
        let tooltip = if ask { "Goes to every client: click twice" } else { "" };
        let kind = if armed { Kind::Danger } else { kind };
        if !widgets::button(ui, &text, kind, enabled, size, tooltip).clicked() {
            return false;
        }
        if !ask || armed {
            self.confirm = None;
            return true;
        }
        self.confirm = Some((label, Instant::now()));
        false
    }

    /// The last outcome, and the earlier ones folded away.
    fn results_card(&self, ui: &mut Ui) {
        let results: Vec<String> = self
            .results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();
        let Some(last) = results.last() else { return };
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(last).monospace().size(11.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::small_button(ui, "Clear", true).clicked() {
                        self.results.lock().unwrap_or_else(|e| e.into_inner()).clear();
                    }
                });
            });
            if results.len() > 1 {
                CollapsingHeader::new(format!("History ({})", results.len()))
                    .id_salt("turu_army_history")
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("turu_army_results")
                            .max_height(140.0)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                for line in &results {
                                    ui.label(RichText::new(line).monospace().size(11.0));
                                }
                            });
                    });
            }
        });
    }
}

/// One row: check box, dot, name, where it is, its script; below, its own buttons.
fn row_ui(ui: &mut Ui, row: &Row, checked: &mut bool) -> Option<RowAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.add_enabled(!row.ended, egui::Checkbox::without_text(checked));
        widgets::dot(ui, row.color, 4.0);
        // Long names and places are cut to the window's width (hover shows them whole).
        let name = row.label();
        let width = (ui.available_width() * 0.45).max(60.0);
        ui.scope(|ui| {
            ui.set_max_width(width);
            ui.add(egui::Label::new(RichText::new(&name).strong().color(TEXT)).truncate())
                .on_hover_text(&name);
        });
        if row.tab.is_some() {
            widgets::pill(ui, |ui| ui.label(RichText::new("tab").size(10.0).color(MUTED)));
        } else if row.headless {
            widgets::pill(ui, |ui| ui.label(RichText::new("no window").size(10.0).color(MUTED)));
        }
        let summary = ui.add(egui::Label::new(RichText::new(&row.summary).small().color(MUTED)).truncate());
        let summary = summary.on_hover_text(&row.summary);
        if let Some(error) = &row.error {
            summary.on_hover_text(error);
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        let mut button = |ui: &mut Ui, label: &str, enabled: bool, tooltip: &str, choice: RowAction| {
            if widgets::small_button(ui, label, enabled)
                .on_hover_text(tooltip)
                .clicked()
            {
                action = Some(choice);
            }
        };
        if row.ended {
            button(ui, "Restart", true, "Launch it again as it was", RowAction::Restart);
            button(ui, "Remove", true, "Take it off the list", RowAction::Close);
            return;
        }
        match &row.script {
            Some((_, true)) => button(ui, "■ Stop", true, "Stop its script", RowAction::Stop),
            Some((name, false)) => button(ui, "▶ Start", row.logged_in, &format!("Start {name}"), RowAction::Start),
            None => button(ui, "▶ Start", false, "No script loaded", RowAction::Start),
        }
        if row.logged_in {
            button(ui, "Log out", true, "Log this account out", RowAction::Logout);
        } else {
            button(ui, "Log in", true, "Log in with the remembered login", RowAction::Login);
        }
        button(ui, "Gear", true, "Its gear: what's worn, equip, bank", RowAction::Gear);
        if let Some(tab) = row.tab
            && widgets::small_button(ui, "Show", tab != crate::turu::active_tab()).clicked()
        {
            crate::turu::request_tab(crate::turu::TabRequest::Select(tab));
        }
        ui.menu_button("⋯", |ui| {
            let mut item = |ui: &mut Ui, label: RichText, choice: RowAction| {
                if ui.button(label).clicked() {
                    action = Some(choice);
                    ui.close();
                }
            };
            item(ui, RichText::new("Relog"), RowAction::Relog);
            if let Some(auto) = &row.auto {
                item(ui, RichText::new(format!("Stop {auto}")), RowAction::StopAuto);
            } else if row.logged_in && !row.script.as_ref().is_some_and(|(_, running)| *running) {
                item(ui, RichText::new("Start Auto-Attack"), RowAction::AutoAttack);
            }
            if row.tab.is_none() {
                item(ui, RichText::new("Logs…"), RowAction::Logs);
                ui.separator();
                item(ui, RichText::new("Restart"), RowAction::Restart);
                item(ui, RichText::new("Close").color(DANGER), RowAction::Close);
            }
        });
        if let Some(auto) = &row.auto {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(auto).size(11.0).color(SUCCESS));
            });
        } else if let Some((name, running)) = &row.script {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let color = if *running { SUCCESS } else { MUTED };
                let state = if *running { "running" } else { "loaded" };
                ui.add(egui::Label::new(RichText::new(name).monospace().size(11.0).color(color)).truncate())
                    .on_hover_text(format!("{name}: {state}"));
            });
        }
    });
    action
}

/// "Alt1: Equip Legion Revenant: done".
fn outcome_line(name: &str, label: &str, result: Result<Value, String>) -> String {
    match result {
        Ok(reply) => match serde_json::from_value::<turu_bot::gear::EquipReport>(reply) {
            Ok(report) if label.starts_with("Equip ") => format!("{name}: {label}: {}", report.summary()),
            _ => format!("{name}: {label}: done"),
        },
        // The client said no (a script runs, not logged in, ...): nothing started there.
        Err(e) if label == Command::StartAutoAttack.label() => format!("{name}: {label}: skipped ({e})"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, ended: bool, script: Option<(&str, bool)>) -> Row {
        Row {
            key: name.to_lowercase(),
            peer: Peer::Client {
                username: name.into(),
                label: name.into(),
            },
            tab: None,
            username: name.into(),
            headless: true,
            color: SUCCESS,
            summary: "Artix · ultradage-123456, r6, Left · 120000/120000 HP".into(),
            error: None,
            script: script.map(|(n, running)| (n.into(), running)),
            auto: None,
            logged_in: !ended,
            ended,
        }
    }

    /// ACTIONS go to the checked rows, or to every row still running when none is checked.
    #[test]
    fn actions_go_to_the_checked_rows_or_everyone() {
        let rows = [
            row("Alt1", false, None),
            row("Alt2", false, None),
            row("Gone", true, None),
        ];
        let mut window = ArmyWindow::default();
        assert_eq!(window.targets(&rows), [0, 1]);
        window.checked.insert("alt2".into());
        assert_eq!(window.targets(&rows), [1]);
        // A checked row that ended isn't sent to.
        window.checked = ["gone".to_owned()].into();
        assert!(window.targets(&rows).is_empty());
    }

    /// Long names, a long place and both button rows keep the window at its width.
    #[test]
    fn rows_fit_the_window() {
        let ctx = egui::Context::default();
        let long = "Legendary Elemental Warrior of the Very Long Name";
        let rows = [
            row(long, false, Some(("VeryLongScriptNameForTheFarm.ts", true))),
            row("Alt2", false, Some(("GoldFarm.ts", false))),
            row("Gone", true, None),
        ];
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 1000.0))),
            ..Default::default()
        };
        let mut widths = Vec::new();
        for _ in 0..8 {
            let mut output = ctx.run_ui(input(), |ui| {
                let ctx = ui.ctx().clone();
                let shown = egui::Window::new("Army Control")
                    .default_size(vec2(560.0, 620.0))
                    .resizable(true)
                    .show(&ctx, |ui| {
                        for row in &rows {
                            let mut checked = false;
                            assert_eq!(row_ui(ui, row, &mut checked), None);
                        }
                    });
                widths.push(shown.expect("the window is open").response.rect.width());
            });
            output.textures_delta.clear();
        }
        let first = widths[0];
        assert!(widths.iter().all(|w| *w <= first + 1.0), "the window grew: {widths:?}");
        assert!(first < 600.0, "the window is wider than its default: {first}");
    }
}
