//! Army > Army Control (VibeSkua's tabbed host "Army Control" menu): the clients Accounts
//! launched, and commands sent to all of them (and to this client) through their control API.

use super::files::{self, Pending};
use super::theme::{DANGER, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use crate::gui::FilePicker;
use egui::{Align, Context, Layout, RichText, ScrollArea, Ui, vec2};
use rfd::AsyncFileDialog;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
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

pub struct ArmyWindow {
    /// Commands also go to this window's tabs.
    include_tabs: bool,
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
        if !*open {
            return;
        }
        super::window(ctx, "Army Control", [560.0, 600.0], 5)
            .open(open)
            .show(ctx, |ui| self.ui(ui, picker));
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// Sends the command to every client (and this one, when included); outcomes are listed.
    fn send(&self, command: Command) {
        let label = command.label();
        let results = self.results.clone();
        let local = if self.include_tabs {
            crate::turu::tabs()
                .into_iter()
                .map(|(_, bot)| (format!("{} (tab)", crate::turu::tab_label(&bot)), bot))
                .collect()
        } else {
            Vec::new()
        };
        crate::turu::army().broadcast(command, local, move |name, result| {
            let line = match result {
                Ok(_) => format!("{name}: {label}: done"),
                Err(e) => format!("{name}: {label}: {e}"),
            };
            let mut results = results.lock().unwrap_or_else(|e| e.into_inner());
            if results.len() == RESULTS {
                results.pop_front();
            }
            results.push_back(line);
        });
    }

    fn ui(&mut self, ui: &mut Ui, picker: &FilePicker) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        self.clients_card(ui);
        self.commands_card(ui, picker);
        self.results_card(ui);
    }

    fn clients_card(&self, ui: &mut Ui) {
        let army = crate::turu::army();
        let clients = army.clients();
        let queued = army.queued();
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
                    for (tab, bot) in crate::turu::tabs() {
                        tab_row(ui, tab, &bot);
                    }
                    if clients.is_empty() && queued.is_empty() {
                        ui.label(RichText::new("No other clients: start accounts from Army > Accounts.").color(MUTED));
                    }
                    for client in &clients {
                        client_row(ui, client);
                    }
                    for username in &queued {
                        ui.label(RichText::new(format!("{username}: waiting to start")).color(MUTED));
                    }
                });
        });
    }

    fn commands_card(&mut self, ui: &mut Ui, picker: &FilePicker) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "ARMY CONTROL");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label("Include this window's tabs");
                    widgets::switch(ui, &mut self.include_tabs);
                });
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

/// One of this window's tabs: a dot while its script runs, its name, its script, and Show.
/// (Only what the bot keeps: asking its game would mean a Flash call from the UI thread.)
fn tab_row(ui: &mut Ui, tab: crate::turu::TabId, bot: &turu_bot::Bot) {
    let status = bot.scripts.status();
    let active = tab == crate::turu::active_tab();
    ui.horizontal(|ui| {
        widgets::dot(ui, if status.is_active() { SUCCESS } else { MUTED }, 4.0);
        ui.label(RichText::new(crate::turu::tab_label(bot)).strong().color(TEXT));
        widgets::pill(ui, |ui| ui.label(RichText::new("tab").size(10.0).color(MUTED)));
        let map = bot.state.lock().last_map.clone();
        if let Some(map) = map {
            ui.label(RichText::new(map).small().color(MUTED));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
}

/// One client: a dot, its name, where it is, its script, and Close. Its game comes from the
/// monitor's hub, which the Army poll feeds.
fn client_row(ui: &mut Ui, client: &Arc<Client>) {
    let state = client.state();
    let snapshot = crate::turu::monitor_hub().army_snapshot(&client.username);
    let (color, summary) = match (&state.exited, state.answered, &snapshot) {
        (Some(exit), _, _) => (DANGER, format!("ended ({exit})")),
        (None, true, Some(snapshot)) => (SUCCESS, summarize(snapshot)),
        (None, _, _) => (MUTED, "starting…".to_owned()),
    };
    ui.horizontal(|ui| {
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
