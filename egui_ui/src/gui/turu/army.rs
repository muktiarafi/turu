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
use crate::host::FileDialog;
use crate::models::army::{self as model, MISC_OPTIONS, Row, RowAction as ModelAction};
use crate::models::scheduler::Tint;
use egui::{Align, Checkbox, CollapsingHeader, Context, Label, Layout, RichText, ScrollArea, Ui, vec2};
use serde_json::json;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use turu_bot::army::Command;

pub use crate::models::army::Peer;

/// Narrower than this (a phone, a small window), a row keeps Start/Stop and ⋯ only.
const NARROW: f32 = 480.0;
/// How long a "click again to confirm" stays armed.
const CONFIRM_FOR: Duration = Duration::from_secs(4);

/// What a row's buttons asked for: the model's commands, or a window of this one's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Model(ModelAction),
    Gear,
    Logs,
}

/// A row's state mark in egui's palette.
fn tint_color(tint: Tint) -> egui::Color32 {
    match tint {
        Tint::Plain => TEXT,
        Tint::Muted => MUTED,
        Tint::Success => SUCCESS,
        Tint::Warning => WARNING,
        Tint::Danger => DANGER,
    }
}

pub struct ArmyWindow {
    /// The checked rows' keys; none checked means every row.
    checked: HashSet<String>,
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
    /// Search… asked for a script: the answer goes to Search Scripts' pick mode
    /// ([`Self::take_script_search`]).
    script_search: Option<crate::host::Answer>,
}

impl Default for ArmyWindow {
    fn default() -> Self {
        Self {
            checked: HashSet::new(),
            gear: Vec::new(),
            logs: Vec::new(),
            travel: Travel::default(),
            quest: String::new(),
            reward: String::new(),
            misc: [false; MISC_OPTIONS.len()],
            pending_script: Pending::default(),
            pending_targets: None,
            confirm: None,
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
        if crate::host::host().has_army() {
            // The shell's other tabs are polled while Army Control is drawn: and while Map Travel
            // or its pickers are open, which read where they are.
            if self.travel.is_open() {
                crate::turu::army_control_drawn();
            }
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
    pub fn take_script_search(&mut self) -> Option<crate::host::Answer> {
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

    fn rows() -> Vec<Row> {
        model::rows()
    }

    /// The rows ACTIONS go to: the checked ones, else all that run.
    fn targets(&self, rows: &[Row]) -> Vec<usize> {
        model::targets(rows, &self.checked)
    }

    fn send_to(&self, rows: &[Row], command: Command) {
        model::send_to(rows, command);
    }

    /// Sends the command to the rows ACTIONS go to.
    fn send(&self, rows: &[Row], command: Command) {
        model::send(rows, &self.checked, command);
    }

    /// A row's own button.
    fn act(&mut self, row: &Row, action: RowAction) {
        let peer = row.peer.clone();
        match action {
            RowAction::Gear => match self.gear.iter_mut().find(|w| w.peer.same(&peer)) {
                Some(window) => window.reopen(),
                None => self.gear.push(GearWindow::new(peer)),
            },
            RowAction::Logs => {
                if let Some(window) = self.logs.iter_mut().find(|w| w.peer.same(&peer)) {
                    window.open = true;
                } else {
                    self.logs.push(ClientLogs::new(peer));
                }
            }
            RowAction::Model(action) => {
                if action == ModelAction::Close {
                    self.checked.remove(&row.key);
                }
                model::act_on(row, action);
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, rows: &[Row]) {
        crate::turu::army_control_drawn();
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
                    if rows.iter().all(Row::is_tab) && queued.is_empty() {
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
                let launched = rows.iter().any(|r| !r.is_tab());
                ui.add_enabled_ui(launched, |ui| {
                    ui.menu_button("⋯", |ui| {
                        touch_rows(ui);
                        if ui.button("Clear ended").clicked() {
                            model::clear_ended();
                            ui.close();
                        }
                        if ui
                            .button(RichText::new("Close all launched clients").color(DANGER))
                            .clicked()
                        {
                            model::close_all();
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
        let groups = model::groups();
        ui.add_enabled_ui(!groups.is_empty(), |ui| {
            ui.menu_button("Groups ⏷", |ui| {
                touch_rows(ui);
                for group in &groups {
                    if ui
                        .button(format!("{}  ({})", group.name, group.accounts.len()))
                        .on_hover_text("Checks this group's clients, and only those")
                        .clicked()
                    {
                        self.checked = model::group_keys(rows, group).into_iter().collect();
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
        let ask = model::asks_twice(&self.checked, targets.len());
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
                if crate::host::host().has_file_dialogs() {
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
        let results = model::results();
        let Some(last) = results.last() else { return };
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::small_button(ui, "Clear", true).clicked() {
                        model::clear_results();
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
                    // A shell tab that's down: the shell restarts it (or its strip does).
                    if !row.is_tab() {
                        item(ui, RichText::new("Remove"), true, RowAction::Model(ModelAction::Close));
                    }
                    return;
                }
                if let Peer::Shell { id, .. } = &row.peer
                    && ui.button("Show its tab").clicked()
                {
                    crate::turu::shell_tab_action(crate::turu::ShellTabAction::Select(*id));
                    ui.close();
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
                        item(
                            ui,
                            RichText::new("Log out"),
                            true,
                            RowAction::Model(ModelAction::Logout),
                        );
                    } else {
                        item(ui, RichText::new("Log in"), true, RowAction::Model(ModelAction::Login));
                    }
                    item(ui, RichText::new("Gear…"), true, RowAction::Gear);
                }
                item(ui, RichText::new("Relog"), true, RowAction::Model(ModelAction::Relog));
                if let Some(auto) = &row.auto {
                    item(
                        ui,
                        RichText::new(format!("Stop {auto}")),
                        true,
                        RowAction::Model(ModelAction::StopAuto),
                    );
                } else {
                    let can = row.auto_attack().enabled;
                    item(
                        ui,
                        RichText::new("Start Auto-Attack"),
                        can,
                        RowAction::Model(ModelAction::AutoAttack),
                    );
                }
                if row.tab.is_none() {
                    item(ui, RichText::new("Logs…"), true, RowAction::Logs);
                }
                if !row.is_tab() {
                    ui.separator();
                    item(
                        ui,
                        RichText::new("Restart"),
                        true,
                        RowAction::Model(ModelAction::Restart),
                    );
                    item(
                        ui,
                        RichText::new("Close").color(DANGER),
                        true,
                        RowAction::Model(ModelAction::Close),
                    );
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
                RowAction::Model(ModelAction::Restart),
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
                    button(
                        ui,
                        "Log out",
                        68.0,
                        true,
                        "Log this account out",
                        RowAction::Model(ModelAction::Logout),
                    );
                } else {
                    button(
                        ui,
                        "Log in",
                        68.0,
                        true,
                        "Log in with the remembered login",
                        RowAction::Model(ModelAction::Login),
                    );
                }
            }
            match &row.script {
                Some((_, true)) => button(
                    ui,
                    "■ Stop",
                    68.0,
                    true,
                    "Stop its script",
                    RowAction::Model(ModelAction::Stop),
                ),
                Some((name, false)) => {
                    let tip = if row.logged_in {
                        format!("Start {name}")
                    } else {
                        "Log in first".to_owned()
                    };
                    button(
                        ui,
                        "▶ Start",
                        68.0,
                        row.logged_in,
                        &tip,
                        RowAction::Model(ModelAction::Start),
                    );
                }
                None => button(
                    ui,
                    "▶ Start",
                    68.0,
                    false,
                    "No script loaded",
                    RowAction::Model(ModelAction::Start),
                ),
            }
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            // Long names and places stop at the buttons (hover shows them whole).
            ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
            ui.vertical(|ui| {
                ui.spacing_mut().interact_size.y = 16.0;
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    widgets::dot(ui, tint_color(row.tint), 4.0);
                    let name = row.label();
                    ui.add(Label::new(RichText::new(&name).strong().color(TEXT)).truncate())
                        .on_hover_text(&name);
                    if let Some(kind) = row.kind() {
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
    crate::models::army_travel::peers(rows)
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
