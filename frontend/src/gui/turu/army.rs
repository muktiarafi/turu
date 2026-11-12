//! Army > Army Control (VibeSkua's tabbed host "Army Control" menu): one list of this window's
//! tabs and the clients Accounts launched, each with buttons of its own (script, log in/out,
//! Gear, and a ⋯ menu), and ACTIONS that go to the checked rows, or to every row when none is
//! checked, through their control API.

use super::army_travel::{self, Travel};
use super::client_logs::ClientLogs;
use super::files::{self, Pending};
use super::gear::GearWindow;
use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::platform::FileDialog;
use crate::turu::TabId;
use egui::{Align, Checkbox, CollapsingHeader, Context, Label, Layout, RichText, ScrollArea, Ui, vec2};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::accounts::Accounts;
use turu_bot::army::{Client, Command};
use turu_bot::monitor::Snapshot;
use turu_bot::monitor::snapshot::BotState;

/// How many command outcomes the window keeps.
const RESULTS: usize = 40;
/// Narrower than this (a phone, a small window), a row keeps Start/Stop and ⋯ only.
const NARROW: f32 = 480.0;
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

    /// Its game comes from the monitor's hub, which the Army poll feeds. The `n`th client is
    /// "Client n" while its Streamer Mode is on.
    fn of_client(n: usize, client: &Arc<Client>) -> Self {
        let state = client.state();
        let snapshot = crate::turu::monitor_hub().army_snapshot(&client.username);
        let streamer = snapshot.as_ref().is_some_and(|s| s.streamer);
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
                label: if streamer {
                    format!("Client {n}")
                } else {
                    client.label.clone()
                },
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
    /// Map Travel, the player picker and Change Server (VibeSkua's travel menu items).
    travel: Travel,
    quest: String,
    reward: String,
    misc: [bool; MISC_OPTIONS.len()],
    pending_script: Pending,
    /// The rows the script being picked goes to.
    pending_targets: Option<Vec<Row>>,
    /// An action on every row waiting for its second click, and since when.
    confirm: Option<(&'static str, Instant)>,
    results: Arc<Mutex<VecDeque<String>>>,
    /// Search… asked for a script: the answer goes to Search Scripts' pick mode
    /// ([`Self::take_script_search`]).
    script_search: Option<crate::platform::Answer>,
}

impl Default for ArmyWindow {
    fn default() -> Self {
        Self {
            checked: HashSet::new(),
            groups: Vec::new(),
            groups_read: None,
            gear: Vec::new(),
            logs: Vec::new(),
            travel: Travel::default(),
            quest: String::new(),
            reward: String::new(),
            misc: [false; MISC_OPTIONS.len()],
            pending_script: Pending::default(),
            pending_targets: None,
            confirm: None,
            results: Arc::default(),
            script_search: None,
        }
    }
}

impl ArmyWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
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
            window.show(ctx);
        }
        // The clients' visited maps are recorded while Turu runs, as VibeSkua's.
        if crate::platform::platform().has_army() {
            let rows = Self::rows();
            self.travel.poll(&travel_peers(&rows));
            let targets = self.targets(&rows).len();
            let current = self.current_key(&rows);
            self.travel.show(ctx, current.as_deref(), targets);
            for command in std::mem::take(&mut self.travel.commands) {
                self.send(&rows, command);
            }
        }
        if !*open {
            return;
        }
        self.window(ctx, open, &Self::rows());
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// The window itself, listing `rows`.
    fn window(&mut self, ctx: &Context, open: &mut bool, rows: &[Row]) {
        let mut window = super::window(ctx, "Army Control", [560.0, 620.0], 5).open(open);
        // On a phone a floating window would cover the game and still be cramped: a sheet.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 600.0 || area.height() < 620.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui, rows));
    }

    /// Search…'s request: Search Scripts should open in its pick mode with this answer.
    pub fn take_script_search(&mut self) -> Option<crate::platform::Answer> {
        self.script_search.take()
    }

    /// The client "Use Current Client Location" reads (VibeSkua: the selected tab): the only
    /// checked row, else the active tab.
    fn current_key(&self, rows: &[Row]) -> Option<String> {
        if self.checked.len() == 1 {
            return self.checked.iter().next().cloned();
        }
        let active = crate::turu::active_tab();
        rows.iter().find(|r| r.tab == Some(active)).map(|r| r.key.clone())
    }

    /// Every tab, then every launched client.
    fn rows() -> Vec<Row> {
        let tabs = crate::turu::tabs().into_iter().map(|(tab, bot)| Row::of_tab(tab, &bot));
        let clients = crate::turu::army().clients();
        tabs.chain(clients.iter().enumerate().map(|(i, c)| Row::of_client(i + 1, c)))
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
            }
        }
        crate::turu::army().broadcast(command, local, Some(&only), move |name, result| {
            let name = shown.get(name).map_or(name, String::as_str);
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

    fn ui(&mut self, ui: &mut Ui, rows: &[Row]) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        self.checked
            .retain(|key| rows.iter().any(|r| &r.key == key && !r.ended));
        self.clients_card(ui, rows);
        self.actions_card(ui, rows);
        self.results_card(ui);
    }

    fn clients_card(&mut self, ui: &mut Ui, rows: &[Row]) {
        let army = crate::turu::army();
        let queued = army.queued();
        let narrow = ui.available_width() < NARROW;
        let mut actions = Vec::new();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            self.clients_header(ui, rows, queued.len());
            let row_height = match (narrow, widgets::touch()) {
                (_, true) => 60.0,
                (_, false) => 44.0,
            };
            ScrollArea::vertical()
                .id_salt("turu_army_clients")
                .max_height(widgets::target(300.0, 360.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for row in rows {
                        let mut checked = self.checked.contains(&row.key);
                        let picked = std::cell::Cell::new(None);
                        let response = widgets::list_row(ui, checked, row_height, |ui| {
                            picked.set(row_ui(ui, row, &mut checked, narrow));
                        });
                        // The whole row checks it, not only its box: an easy target for a finger.
                        if response.clicked() && !row.ended {
                            checked = !checked;
                        }
                        if let Some(action) = picked.get() {
                            actions.push((row, action));
                        }
                        if checked && !row.ended {
                            self.checked.insert(row.key.clone());
                        } else {
                            self.checked.remove(&row.key);
                        }
                    }
                    for username in &queued {
                        widgets::list_row(ui, false, row_height, |ui| {
                            ui.add_space(widgets::target(18.0, 30.0) + 6.0);
                            ui.spinner();
                            ui.label(RichText::new(format!("{username}: waiting to start")).color(MUTED));
                        });
                    }
                    if rows.iter().all(|r| r.tab.is_some()) && queued.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Start more accounts from Army > Accounts.")
                                .size(12.0)
                                .color(MUTED),
                        );
                    }
                });
        });
        for (row, action) in actions {
            self.act(row, action);
        }
    }

    /// Above the rows: a box that checks or clears them all, how many are checked, Groups ⏷
    /// (an Army > Accounts group's rows) and ⋯ (Clear ended, Close all launched clients).
    fn clients_header(&mut self, ui: &mut Ui, rows: &[Row], queued: usize) {
        let live: Vec<&Row> = rows.iter().filter(|r| !r.ended).collect();
        let checked = live.iter().filter(|r| self.checked.contains(&r.key)).count();
        ui.horizontal(|ui| {
            // The box lines up with the rows' boxes (`widgets::list_row`'s inset).
            ui.add_space(8.0);
            let mut all = !live.is_empty() && checked == live.len();
            let some = checked > 0 && checked < live.len();
            let check = ui
                .add_enabled(!live.is_empty(), Checkbox::without_text(&mut all).indeterminate(some))
                .on_hover_text("Checks or clears every client (All / None)");
            if check.changed() {
                if all && !some {
                    self.checked = live.iter().map(|r| r.key.clone()).collect();
                } else {
                    self.checked.clear();
                }
            }
            let mut count = match (checked, live.len()) {
                (0, 1) => "1 client".to_owned(),
                (0, n) => format!("{n} clients"),
                (c, n) => format!("{c} of {n} selected"),
            };
            if queued > 0 {
                count.push_str(&format!(" · {queued} waiting"));
            }
            let color = if checked > 0 { TEXT } else { MUTED };
            ui.add(Label::new(RichText::new(count).size(12.0).color(color)).truncate());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let army = crate::turu::army();
                let launched = rows.iter().any(|r| r.tab.is_none());
                ui.add_enabled_ui(launched, |ui| {
                    ui.menu_button("⋯", |ui| {
                        touch_rows(ui);
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
                    })
                    .response
                    .on_hover_text("Clear ended, close all launched clients");
                });
                self.groups_menu(ui, rows);
            });
        });
    }

    /// Groups ⏷: checks an Army > Accounts group's rows, and only those.
    fn groups_menu(&mut self, ui: &mut Ui, rows: &[Row]) {
        if self.groups_read.is_none_or(|t| t.elapsed() >= Duration::from_secs(3)) {
            self.groups_read = Some(Instant::now());
            self.groups = Accounts::load(&crate::turu::bot().paths.accounts_file()).groups;
        }
        let groups = self.groups.clone();
        ui.add_enabled_ui(!groups.is_empty(), |ui| {
            ui.menu_button("Groups ⏷", |ui| {
                touch_rows(ui);
                for group in &groups {
                    if ui
                        .button(format!("{}  ({})", group.name, group.accounts.len()))
                        .on_hover_text("Checks this group's clients, and only those")
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
    }

    fn actions_card(&mut self, ui: &mut Ui, rows: &[Row]) {
        let targets = self.targets(rows);
        let names: Vec<String> = targets.iter().map(|&i| rows[i].label()).collect();
        let everyone = self.checked.is_empty();
        if self.confirm.is_some_and(|(_, at)| at.elapsed() >= CONFIRM_FOR) {
            self.confirm = None;
        }
        let any = !targets.is_empty();
        let ask = everyone && targets.len() > 1;
        let height = widgets::target(28.0, 44.0);
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
                ui.add(Label::new(RichText::new(format!("→ {who}")).size(12.0).color(color)).truncate())
                    .on_hover_text(names.join(", "));
            });

            group_label(ui, "SCRIPT");
            let mut grid = Grid::new(ui, 4, 116.0, height);
            ui.horizontal_wrapped(|ui| {
                let size = grid.next(ui);
                if widgets::button(ui, "Start script", Kind::Primary, any, size, "").clicked() {
                    self.send(rows, Command::StartScript(None));
                }
                let size = grid.next(ui);
                if self.guarded(ui, "Stop script", Kind::Normal, any, size, ask) {
                    self.send(rows, Command::StopScript);
                }
                let waiting = self.pending_script.is_open();
                let size = grid.next(ui);
                let search = widgets::button(ui, "Search…", Kind::Normal, any && !waiting, size, "")
                    .on_hover_text("Pick a script in Search Scripts and load it in each client");
                if search.clicked() {
                    self.pending_targets = Some(targets.iter().map(|&i| rows[i].clone()).collect());
                    let mut asked = None;
                    self.pending_script.open(ui.ctx(), |answer| {
                        asked = Some(answer);
                        true
                    });
                    self.script_search = asked;
                }
                if crate::platform::platform().has_file_dialogs() {
                    let size = grid.next(ui);
                    let load = widgets::button(ui, "Load file…", Kind::Normal, any && !waiting, size, "")
                        .on_hover_text("Pick a script file and load it in each client");
                    if load.clicked() {
                        self.pending_targets = Some(targets.iter().map(|&i| rows[i].clone()).collect());
                        let start = files::scripts_start_dir(crate::turu::bot().paths.scripts_dir());
                        let dialog = FileDialog::new()
                            .set_title("Load Script")
                            .add_filter("Scripts", &["ts", "js"])
                            .set_directory(start);
                        self.pending_script.pick_file(ui.ctx(), dialog);
                    }
                } else {
                    // Without file dialogs (Android): also the script this tab has loaded.
                    let loaded = crate::turu::bot().scripts.loaded();
                    let size = grid.next(ui);
                    let load = widgets::button(
                        ui,
                        "This tab's script",
                        Kind::Normal,
                        any && loaded.is_some(),
                        size,
                        "Load one in the Script Loader (Search Scripts) first",
                    )
                    .on_hover_text("Loads the script this tab's Script Loader has loaded");
                    if load.clicked()
                        && let Some(path) = loaded
                    {
                        self.send(rows, Command::LoadScript(path.display().to_string()));
                    }
                }
            });

            group_label(ui, "ACCOUNT");
            let mut grid = Grid::new(ui, 4, 100.0, height);
            ui.horizontal_wrapped(|ui| {
                let size = grid.next(ui);
                if widgets::button(ui, "Log in", Kind::Normal, any, size, "").clicked() {
                    self.send(rows, Command::Login);
                }
                let size = grid.next(ui);
                if self.guarded(ui, "Log out", Kind::Normal, any, size, ask) {
                    self.send(rows, Command::Logout);
                }
                let size = grid.next(ui);
                if widgets::button(ui, "Relog", Kind::Normal, any, size, "").clicked() {
                    self.send(rows, Command::Relogin);
                }
                let size = grid.next(ui);
                if widgets::button(ui, "Go to house", Kind::Normal, any, size, "").clicked() {
                    self.send(rows, Command::JumpHouse);
                }
            });

            // VibeSkua's "Start/Stop Auto-Attack for All": no confirmation there, and stopping
            // loses nothing.
            group_label(ui, "AUTO-ATTACK");
            let mut grid = Grid::new(ui, 2, 140.0, height);
            ui.horizontal_wrapped(|ui| {
                let tip = "Each client's Auto Attack with its equipped class (rows running a script are skipped)";
                let size = grid.next(ui);
                if widgets::button(ui, "Start Auto-Attack", Kind::Normal, any, size, "")
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.send(rows, Command::StartAutoAttack);
                }
                let size = grid.next(ui);
                if widgets::button(ui, "Stop Auto-Attack", Kind::Normal, any, size, "")
                    .on_hover_text("Stops Auto Attack or Auto Hunt; scripts keep running")
                    .clicked()
                {
                    self.send(rows, Command::StopAuto);
                }
            });

            ui.add_space(2.0);
            CollapsingHeader::new("Travel")
                .id_salt("turu_army_travel")
                .default_open(true)
                .show(ui, |ui| {
                    let mut grid = Grid::new(ui, 3, 130.0, widgets::target(26.0, 44.0));
                    ui.horizontal_wrapped(|ui| {
                        let size = grid.next(ui);
                        if widgets::button(ui, "Map Travel…", Kind::Normal, true, size, "")
                            .on_hover_text("Recent destinations and saved map presets")
                            .clicked()
                        {
                            self.travel.open = true;
                        }
                        let size = grid.next(ui);
                        if widgets::button(ui, "Jump to Player…", Kind::Normal, any, size, "")
                            .on_hover_text("Pick a player from the open clients, or type a name")
                            .clicked()
                        {
                            self.travel.open_player_picker(&travel_peers(rows));
                        }
                        let size = grid.next(ui);
                        if widgets::button(ui, "Change Server…", Kind::Normal, any, size, "")
                            .on_hover_text("Relog the clients on another server")
                            .clicked()
                        {
                            self.travel.open_server_picker();
                        }
                    });
                });
            CollapsingHeader::new("Quest")
                .id_salt("turu_army_quest")
                .show(ui, |ui| {
                    let gap = ui.spacing().item_spacing.x;
                    let room = ui.available_width();
                    // Two fields and the button on a line, or the fields over a full-width button.
                    let inline = room >= 380.0;
                    let button = if inline { 120.0 } else { room };
                    let fields = if inline { room - button - gap } else { room };
                    let id = self.quest.trim().parse::<i64>().ok().filter(|id| *id > 0);
                    ui.horizontal_wrapped(|ui| {
                        // Rounded down, so the two never wrap on a fraction of a point.
                        let quest = ((fields - gap) * 0.4).floor();
                        widgets::text_box(ui, &mut self.quest, "Quest ID", quest);
                        let reward = (fields - gap - quest - 1.0).floor();
                        widgets::text_box(ui, &mut self.reward, "Reward ID (optional)", reward);
                        if !inline {
                            ui.end_row();
                        }
                        let accept = widgets::button(
                            ui,
                            "Accept quest",
                            Kind::Normal,
                            any && id.is_some(),
                            vec2(button, widgets::target(26.0, 44.0)),
                            "Enter a quest ID",
                        );
                        if accept.clicked()
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
                    // Two columns of switches, or one when narrow.
                    let columns = if ui.available_width() >= 360.0 { 2 } else { 1 };
                    let width = (ui.available_width() - 12.0 * (columns - 1) as f32) / columns as f32;
                    let mut changed = None;
                    for pair in MISC_OPTIONS.chunks(columns) {
                        ui.horizontal(|ui| {
                            for (label, name) in pair {
                                let i = MISC_OPTIONS.iter().position(|(_, n)| n == name).unwrap_or(0);
                                ui.allocate_ui_with_layout(
                                    vec2(width, widgets::target(22.0, 40.0)),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.set_min_width(width);
                                        if widgets::switch(ui, &mut self.misc[i]).changed() {
                                            changed = Some((*name, self.misc[i]));
                                        }
                                        ui.label(*label);
                                    },
                                );
                            }
                        });
                    }
                    if let Some((name, on)) = changed {
                        self.send(rows, Command::SetOptions(json!({ name: on })));
                    }
                });
        });
    }

    /// A button that, when `ask`, needs a second click within a few seconds ("… all?", in red).
    /// True when the action should run.
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

    /// The last outcome (cut to the width, whole on hover), and the earlier ones folded away.
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
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::small_button(ui, "Clear", true).clicked() {
                        self.results.lock().unwrap_or_else(|e| e.into_inner()).clear();
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add(Label::new(RichText::new(last).monospace().size(11.0)).truncate())
                            .on_hover_text(last);
                    });
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
                                    ui.add(Label::new(RichText::new(line).monospace().size(11.0)).wrap());
                                }
                            });
                    });
            }
        });
    }
}

/// A small heading over a group of ACTIONS' buttons.
fn group_label(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(text).size(10.5).color(MUTED));
}

/// Buttons in even columns, as many to a line as fit at their narrowest (at most `count`),
/// sharing the line; drawn in `horizontal_wrapped`, a new line every `columns` buttons
/// (Turu's buttons draw in a scope of their own, which doesn't wrap by itself).
struct Grid {
    columns: usize,
    size: egui::Vec2,
    placed: usize,
}

impl Grid {
    fn new(ui: &Ui, count: usize, min: f32, height: f32) -> Self {
        let gap = ui.spacing().item_spacing.x;
        let room = ui.available_width();
        let fit = (((room + gap) / (min + gap)).floor() as usize).clamp(1, count);
        // As few columns as keep the same number of lines: 2 + 2, not 3 + 1.
        let columns = count.div_ceil(count.div_ceil(fit));
        // Rounded down, so a line never overflows on a fraction of a point.
        let width = ((room - gap * (columns - 1) as f32) / columns as f32).floor();
        Self {
            columns,
            size: vec2(width, height),
            placed: 0,
        }
    }

    /// The next button's size, starting a line when this one is full.
    fn next(&mut self, ui: &mut Ui) -> egui::Vec2 {
        if self.placed > 0 && self.placed.is_multiple_of(self.columns) {
            ui.end_row();
        }
        self.placed += 1;
        self.size
    }
}

/// On a touch screen, a menu's rows are about 44 points high.
fn touch_rows(ui: &mut Ui) {
    if widgets::touch() {
        ui.spacing_mut().button_padding.y = 14.0;
        ui.spacing_mut().interact_size.y = 40.0;
    }
}

/// One row (inside `widgets::list_row`): check box; the name, its kind and state mark; under
/// it what it runs and where it is; on the right its own buttons. Narrow, Start/Stop and ⋯
/// only (Log in/out and Gear move into ⋯).
fn row_ui(ui: &mut Ui, row: &Row, checked: &mut bool, narrow: bool) -> Option<RowAction> {
    // Both the buttons and the menu set it.
    let action = std::cell::Cell::new(None);
    let touch = widgets::touch();
    if touch {
        let spacing = ui.spacing_mut();
        spacing.icon_width = 24.0;
        spacing.icon_width_inner = 14.0;
    }
    ui.add_enabled(!row.ended, Checkbox::without_text(checked));
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let size = |width: f32| vec2(widgets::target(width, width + 8.0), widgets::target(24.0, 40.0));
        let button = |ui: &mut Ui, label: &str, width: f32, enabled: bool, tip: &str, choice: RowAction| {
            if widgets::button(ui, label, Kind::Normal, enabled, size(width), "")
                .on_hover_text(tip)
                .clicked()
            {
                action.set(Some(choice));
            }
        };
        let menu = egui::Button::new("⋯").min_size(size(30.0));
        egui::containers::menu::MenuButton::from_button(menu)
            .ui(ui, |ui| {
                touch_rows(ui);
                ui.set_min_width(180.0);
                let item = |ui: &mut Ui, label: RichText, enabled: bool, choice: RowAction| {
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        action.set(Some(choice));
                        ui.close();
                    }
                };
                if row.ended {
                    item(ui, RichText::new("Remove"), true, RowAction::Close);
                    return;
                }
                if let Some(tab) = row.tab {
                    let show = ui
                        .add_enabled(tab != crate::turu::active_tab(), egui::Button::new("Show its tab"))
                        .on_disabled_hover_text("It's the tab shown");
                    if show.clicked() {
                        crate::turu::request_tab(crate::turu::TabRequest::Select(tab));
                        ui.close();
                    }
                }
                if narrow {
                    if row.logged_in {
                        item(ui, RichText::new("Log out"), true, RowAction::Logout);
                    } else {
                        item(ui, RichText::new("Log in"), true, RowAction::Login);
                    }
                    item(ui, RichText::new("Gear…"), true, RowAction::Gear);
                }
                item(ui, RichText::new("Relog"), true, RowAction::Relog);
                if let Some(auto) = &row.auto {
                    item(ui, RichText::new(format!("Stop {auto}")), true, RowAction::StopAuto);
                } else {
                    let can = row.logged_in && !row.script.as_ref().is_some_and(|(_, running)| *running);
                    item(ui, RichText::new("Start Auto-Attack"), can, RowAction::AutoAttack);
                }
                if row.tab.is_none() {
                    item(ui, RichText::new("Logs…"), true, RowAction::Logs);
                    ui.separator();
                    item(ui, RichText::new("Restart"), true, RowAction::Restart);
                    item(ui, RichText::new("Close").color(DANGER), true, RowAction::Close);
                }
            })
            .0
            .on_hover_text("More: relog, Auto-Attack, logs, restart, close");
        if row.ended {
            button(
                ui,
                "Restart",
                72.0,
                true,
                "Launch it again as it was",
                RowAction::Restart,
            );
        } else {
            if !narrow {
                button(
                    ui,
                    "Gear",
                    52.0,
                    true,
                    "Its gear: what's worn, equip, bank",
                    RowAction::Gear,
                );
                if row.logged_in {
                    button(ui, "Log out", 68.0, true, "Log this account out", RowAction::Logout);
                } else {
                    button(
                        ui,
                        "Log in",
                        68.0,
                        true,
                        "Log in with the remembered login",
                        RowAction::Login,
                    );
                }
            }
            match &row.script {
                Some((_, true)) => button(ui, "■ Stop", 68.0, true, "Stop its script", RowAction::Stop),
                Some((name, false)) => {
                    let tip = if row.logged_in {
                        format!("Start {name}")
                    } else {
                        "Log in first".to_owned()
                    };
                    button(ui, "▶ Start", 68.0, row.logged_in, &tip, RowAction::Start);
                }
                None => button(ui, "▶ Start", 68.0, false, "No script loaded", RowAction::Start),
            }
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            // Long names and places stop at the buttons (hover shows them whole).
            ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
            ui.vertical(|ui| {
                ui.spacing_mut().interact_size.y = 16.0;
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    widgets::dot(ui, row.color, 4.0);
                    let name = row.label();
                    ui.add(Label::new(RichText::new(&name).strong().color(TEXT)).truncate())
                        .on_hover_text(&name);
                    let kind = if row.tab.is_some() {
                        Some("tab")
                    } else if row.headless {
                        Some("no window")
                    } else {
                        None
                    };
                    if let Some(kind) = kind {
                        ui.label(RichText::new(kind).size(10.5).color(MUTED));
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    // What it runs comes first: it's what a narrow row keeps.
                    if let Some(auto) = &row.auto {
                        ui.label(RichText::new(auto).size(11.0).color(SUCCESS));
                        ui.label(RichText::new("·").size(11.0).color(MUTED));
                    } else if let Some((name, running)) = &row.script {
                        let color = if *running { SUCCESS } else { MUTED };
                        let state = if *running { "running" } else { "loaded" };
                        ui.label(RichText::new(name).monospace().size(11.0).color(color))
                            .on_hover_text(format!("{name}: {state}"));
                        ui.label(RichText::new("·").size(11.0).color(MUTED));
                    }
                    let color = if row.error.is_some() { DANGER } else { MUTED };
                    let summary = ui.add(Label::new(RichText::new(&row.summary).size(11.0).color(color)).truncate());
                    let summary = summary.on_hover_text(&row.summary);
                    if let Some(error) = &row.error {
                        summary.on_hover_text(error);
                    }
                });
            });
        });
    });
    action.get()
}

/// The rows as Map Travel needs them: tabs read their own location, clients come from the hub.
fn travel_peers(rows: &[Row]) -> Vec<army_travel::Peer> {
    rows.iter()
        .filter(|r| !r.ended)
        .map(|r| {
            let (bot, player) = match &r.peer {
                Peer::Tab(bot) => {
                    let name = bot.state.lock().username.clone().filter(|n| !n.trim().is_empty());
                    (Some(bot.clone()), name.or_else(|| Some(r.username.clone())))
                }
                Peer::Client { .. } => (None, Some(r.username.clone())),
            };
            army_travel::Peer {
                key: r.key.clone(),
                bot,
                username: r.username.clone(),
                player: player.filter(|p| r.logged_in && !p.trim().is_empty()),
            }
        })
        .collect()
}

/// "Alt1: Equip Legion Revenant: done".
fn outcome_line(name: &str, label: &str, result: Result<Value, String>) -> String {
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
                            assert_eq!(row_ui(ui, row, &mut checked, false), None);
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
