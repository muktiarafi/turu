//! Army > Army Queue (VibeSkua Manager's "Army Queue" tab, `ArmyQueueUserControl`): squads of
//! accounts that run one after another, each with a default script and per-account Script
//! overrides, saved as you edit and as named presets. The run itself is
//! `turu_bot::army::queue`'s; this is its editor and status.

use super::files::Pending;
use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::platform::FileDialog;
use egui::{Align, ComboBox, Context, Layout, Modal, RichText, ScrollArea, Ui, vec2};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use turu_bot::accounts::{Accounts, Group};
use turu_bot::army::queue::{
    self, ArmyLauncher, MemberStatus, Outcome, Presets, QueueConfig, QueueRunner, QueueStatus, StepStatus,
};
use turu_bot::script::ScriptOptionsState;
use turu_bot::script::options::ScriptOptions;

/// Where a picked script goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// A squad's default script.
    Step(usize),
    /// A squad's account's own script.
    Member(usize, usize),
}

/// "+ Pick Accounts": every account, the squad's checked.
struct Picker {
    search: String,
    /// Username, label, checked.
    accounts: Vec<(String, String, bool)>,
}

pub struct ArmyQueueWindow {
    config: QueueConfig,
    /// What `ArmyQueueActive.json` holds (the config is saved when it differs).
    saved: Option<QueueConfig>,
    selected: usize,
    runner: Arc<QueueRunner>,
    /// The queue as it started (the run's statuses follow its squads).
    running: Option<QueueConfig>,
    presets: Presets,
    preset: String,
    show_help: Option<bool>,
    picker: Option<Picker>,
    groups: Vec<Group>,
    groups_read: Option<std::time::Instant>,
    pending_script: Pending,
    pending_target: Option<Target>,
    /// "Browse"/"Script" opened Search Scripts: the script it loads goes to the target (the
    /// path loaded before, to see the change).
    repo_target: Option<(Target, Option<PathBuf>)>,
    /// `TuruUi` opens Search Scripts.
    pub open_repo: bool,
    reading: Arc<Mutex<Option<ScriptOptionsState>>>,
    /// Options was clicked and its options are read: `TuruUi` opens the Script Options window.
    pub edit_options: Option<ScriptOptions>,
    notice: Option<(String, String)>,
    confirm_stop: bool,
}

impl Default for ArmyQueueWindow {
    fn default() -> Self {
        Self {
            config: QueueConfig::default(),
            saved: None,
            selected: 0,
            runner: QueueRunner::new(),
            running: None,
            presets: Presets::default(),
            preset: String::new(),
            show_help: None,
            picker: None,
            groups: Vec::new(),
            groups_read: None,
            pending_script: Pending::default(),
            pending_target: None,
            repo_target: None,
            open_repo: false,
            reading: Arc::default(),
            edit_options: None,
            notice: None,
            confirm_stop: false,
        }
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

impl ArmyQueueWindow {
    /// Loads the active queue and the presets once (VibeSkua's constructor).
    fn ensure_loaded(&mut self) {
        if self.saved.is_some() {
            return;
        }
        let paths = &crate::turu::bot().paths;
        self.config = QueueConfig::load_active(&paths.army_queue_active_file());
        self.saved = Some(self.config.clone());
        self.presets = Presets::load(&paths.army_queues_file());
        self.preset = self.presets.names().into_iter().next().unwrap_or_default();
        self.selected = 0;
    }

    /// VibeSkua `AutoSave`: the queue as edited, not while it runs.
    fn auto_save(&mut self) {
        if self.runner.is_running() || self.saved.as_ref() == Some(&self.config) {
            return;
        }
        let file = crate::turu::bot().paths.army_queue_active_file();
        if let Err(e) = self.config.save(&file) {
            crate::turu::bot()
                .logs
                .debug
                .push(format!("Error auto-saving Army Queue: {e}"));
        }
        self.saved = Some(self.config.clone());
    }

    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        self.take_scripts();
        if let Some(outcome) = self.runner.take_outcome() {
            self.running = None;
            if outcome == Outcome::Finished {
                self.notice = Some((
                    "Army Queue Finished".into(),
                    "All scheduled Army Queue squads have completed their runs!".into(),
                ));
            }
        }
        if self.runner.is_running() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        if *open {
            self.ensure_loaded();
            super::window(ctx, "Army Queue", [600.0, 640.0], 6)
                .open(open)
                .show(ctx, |ui| self.ui(ui));
            self.auto_save();
        }
        self.picker_dialog(ctx);
        self.stop_dialog(ctx);
        if let Some((title, text)) = &self.notice {
            let mut close = false;
            let response = Modal::new(egui::Id::new("turu_army_queue_notice")).show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.heading(title);
                ui.label(text);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = widgets::button(ui, "OK", Kind::Primary, true, size(72.0), "").clicked();
                });
            });
            if close || response.should_close() {
                self.notice = None;
            }
        }
    }

    /// A script picked from a file or loaded from Search Scripts, and options read.
    fn take_scripts(&mut self) {
        if let Some(path) = self.pending_script.poll()
            && let Some(target) = self.pending_target.take()
        {
            self.set_script(target, Some(path.display().to_string()));
        }
        if let Some((target, before)) = &self.repo_target {
            let loaded = crate::turu::bot().scripts.loaded();
            if loaded.is_some() && loaded != *before {
                let target = *target;
                self.repo_target = None;
                self.set_script(target, loaded.map(|p| p.display().to_string()));
            }
        }
        let read = self.reading.lock().ok().and_then(|mut r| r.take());
        match read {
            Some(ScriptOptionsState::Loaded(options)) => self.edit_options = Some(options),
            Some(ScriptOptionsState::NoOptions) => {
                self.notice = Some((
                    "No Options".into(),
                    "The loaded script has no options to configure.".into(),
                ))
            }
            Some(ScriptOptionsState::Failed(e)) => {
                self.notice = Some((
                    "Script Error".into(),
                    format!("Script cannot be configured as it has compilation errors:\n{e}"),
                ))
            }
            None => {}
        }
    }

    fn set_script(&mut self, target: Target, path: Option<String>) {
        match target {
            Target::Step(i) => {
                if let Some(step) = self.config.steps.get_mut(i) {
                    step.step_script_path = path;
                }
            }
            Target::Member(i, j) => {
                if let Some(account) = self.config.steps.get_mut(i).and_then(|s| s.accounts.get_mut(j)) {
                    account.script_path = path;
                }
            }
        }
    }

    fn browse_file(&mut self, ctx: &Context, target: Target) {
        self.pending_target = Some(target);
        let start = super::files::scripts_start_dir(crate::turu::bot().paths.scripts_dir());
        let dialog = FileDialog::new()
            .set_title("Select Script")
            .add_filter("Scripts", &["ts", "js"])
            .set_directory(start);
        self.pending_script.pick_file(ctx, dialog);
    }

    fn browse_repo(&mut self, target: Target) {
        self.repo_target = Some((target, crate::turu::bot().scripts.loaded()));
        self.open_repo = true;
    }

    /// VibeSkua `EditStepScriptConfig` / `EditMemberScriptConfig`: the script's options in the
    /// squad's (or account's) own storage.
    fn configure(&mut self, target: Target) {
        let Some(step) = self.config.steps.get(match target {
            Target::Step(i) | Target::Member(i, _) => i,
        }) else {
            return;
        };
        let (script, storage) = match target {
            Target::Step(_) => match step.step_script_path.as_deref().filter(|p| !p.trim().is_empty()) {
                Some(path) => (path.to_owned(), queue::step_storage(&step.name, path)),
                None => {
                    self.notice = Some((
                        "No Script Selected".into(),
                        "Please select a script for this squad first.".into(),
                    ));
                    return;
                }
            },
            Target::Member(_, j) => {
                let Some(account) = step.accounts.get(j).filter(|a| a.has_script()) else {
                    self.notice = Some((
                        "No Script Selected".into(),
                        "Please assign a script to this account first.".into(),
                    ));
                    return;
                };
                let path = account.script_path.clone().unwrap_or_default();
                let storage = queue::member_storage(&step.name, &account.username, &path);
                (path, storage)
            }
        };
        let path = PathBuf::from(&script);
        if !path.is_file() {
            self.notice = Some(("Script Not Found".into(), format!("Script file not found:\n{script}")));
            return;
        }
        let slot = self.reading.clone();
        let bot = crate::turu::bot();
        let _ = std::thread::Builder::new()
            .name("Turu Script Options".into())
            .spawn(move || {
                let state = bot.scripts.read_options(&path, Some(&storage));
                if let Ok(mut slot) = slot.lock() {
                    *slot = Some(state);
                }
            });
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let status = self.runner.status();
        self.header(ui, &status);
        if self
            .show_help
            .unwrap_or_else(|| !crate::turu::bot().settings.read().army_queue_help_dismissed)
        {
            self.help(ui);
        }
        if status.running {
            status_card(ui, &status);
        }
        let running = status.running;
        // The statuses follow the queue as it started: no editing while it runs.
        ui.add_enabled_ui(!running, |ui| {
            self.squads(ui, running);
            self.selected = self.selected.min(self.config.steps.len().saturating_sub(1));
            self.squad(ui, &status);
            self.presets_bar(ui);
        });
    }

    fn header(&mut self, ui: &mut Ui, status: &QueueStatus) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Army Queue").size(18.0).strong().color(TEXT));
            if widgets::small_button(ui, "How to Use", true)
                .on_hover_text("Toggle Quick Guide")
                .clicked()
            {
                let shown = self
                    .show_help
                    .unwrap_or_else(|| !crate::turu::bot().settings.read().army_queue_help_dismissed);
                self.show_help = Some(!shown);
            }
            if !status.running {
                if widgets::button(ui, "▶ Start Queue", Kind::Primary, true, size(120.0), "").clicked() {
                    self.start();
                }
                return;
            }
            if status.can_pause()
                && widgets::button(ui, "Pause Queue", Kind::Normal, true, size(110.0), "")
                    .on_hover_text("Pause after current squad finishes - will NOT start next squad")
                    .clicked()
            {
                self.runner.pause();
            }
            if status.can_resume()
                && widgets::button(ui, "Resume Queue", Kind::Primary, true, size(110.0), "")
                    .on_hover_text("Resume queue and proceed to next squad")
                    .clicked()
            {
                self.runner.resume();
            }
            if widgets::button(ui, "■ Stop Queue", Kind::Danger, true, size(110.0), "")
                .on_hover_text("Stop queue completely")
                .clicked()
            {
                self.confirm_stop = true;
            }
        });
    }

    /// VibeSkua `StartQueue`.
    fn start(&mut self) {
        let launcher = Arc::new(ArmyLauncher {
            army: crate::turu::army().clone(),
            accounts_file: crate::turu::bot().paths.accounts_file(),
        });
        match self.runner.start(self.config.clone(), launcher) {
            Ok(()) => self.running = Some(self.config.clone()),
            Err(e) => self.notice = Some(("Empty Queue".into(), e)),
        }
    }

    fn help(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Quick Guide: How Army Queue Works").strong().size(12.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::small_button(ui, "✕", true).clicked() {
                        self.dismiss_help();
                    }
                });
            });
            for line in [
                "1. Create Squads: Keep Squad 1 or click '+ Squad' to add more squads (runs one after another).",
                "2. Pick Players: Click '+ Pick Accounts' to check off accounts directly from your list.",
                "3. Assign Scripts: Set a squad script, or choose an account's own Script to override it (e.g. LoO, \
                 AP, SC, DPS).",
                "4. Hit 'Start Queue': Squad 1 runs -> auto-exits on completion -> Squad 2 starts automatically!",
            ] {
                ui.add(egui::Label::new(RichText::new(line).size(11.0)).wrap());
            }
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Tip: You can reuse the same account in multiple squads (e.g., your main buffer as carry in \
                         Squad 1 and Squad 2).",
                    )
                    .size(10.0)
                    .italics()
                    .color(MUTED),
                )
                .wrap(),
            );
            if widgets::small_button(ui, "Got it, hide guide", true).clicked() {
                self.dismiss_help();
            }
        });
    }

    /// VibeSkua `DismissHelpGuide`: hidden for good.
    fn dismiss_help(&mut self) {
        self.show_help = Some(false);
        let bot = crate::turu::bot();
        bot.settings.write().army_queue_help_dismissed = true;
        bot.save_settings();
    }

    /// The squad tabs (drag one onto another to reorder), "+" and "Import Group".
    fn squads(&mut self, ui: &mut Ui, running: bool) {
        let mut select = None;
        let mut close = None;
        let mut moved = None;
        ui.horizontal_wrapped(|ui| {
            for (i, step) in self.config.steps.iter().enumerate() {
                let selected = i == self.selected;
                let mut tab = |ui: &mut Ui| {
                    widgets::pill(ui, |ui| {
                        ui.horizontal(|ui| {
                            let name = RichText::new(&step.name).strong().size(12.0);
                            let name = if selected {
                                name.color(SUCCESS)
                            } else {
                                name.color(TEXT)
                            };
                            if ui.add(egui::Button::new(name).frame(false)).clicked() {
                                select = Some(i);
                            }
                            ui.label(RichText::new(step.accounts.len().to_string()).size(10.0).color(MUTED));
                            if ui
                                .add(egui::Button::new(RichText::new("✕").size(10.0).color(MUTED)).frame(false))
                                .on_hover_text("Close Squad")
                                .clicked()
                            {
                                close = Some(i);
                            }
                        });
                    });
                };
                if running {
                    tab(ui);
                    continue;
                }
                let response = ui
                    .dnd_drag_source(egui::Id::new(("turu_army_queue_squad", i)), i, tab)
                    .response
                    .on_hover_text("Drag onto another squad to move it there");
                if let Some(from) = response.dnd_release_payload::<usize>() {
                    moved = Some((*from, i));
                }
            }
            if widgets::small_button(ui, "+", true)
                .on_hover_text("Add Squad")
                .clicked()
            {
                self.selected = self.config.add_step();
            }
            let groups_read = self.groups.clone();
            ui.menu_button("Import Group", |ui| {
                if groups_read.is_empty() {
                    ui.label(RichText::new("No groups yet: make them in Army > Accounts").color(MUTED));
                }
                for group in &groups_read {
                    if ui
                        .button(format!("{} ({})", group.name, group.accounts.len()))
                        .clicked()
                    {
                        self.import_group(group);
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("Import an existing Manager Group as a Squad");
        });
        // Army > Accounts' groups, read again every few seconds.
        if self.groups_read.is_none_or(|t| t.elapsed() >= Duration::from_secs(3)) {
            self.groups_read = Some(std::time::Instant::now());
            self.groups = Accounts::load(&crate::turu::bot().paths.accounts_file()).groups;
        }
        if let Some(i) = select {
            self.selected = i;
        }
        if let Some(i) = close {
            self.selected = self.config.remove_step(i, self.selected);
        }
        if let Some((from, to)) = moved {
            self.selected = self.config.move_step(from, to);
        }
    }

    /// VibeSkua `ImportGroupAsStep`: a squad named after the group, with its accounts.
    fn import_group(&mut self, group: &Group) {
        let accounts = Accounts::load(&crate::turu::bot().paths.accounts_file());
        let mut step = queue::QueueStep::new(&group.name);
        for username in &group.accounts {
            let label = accounts.find(username).map_or(username.as_str(), |a| a.label());
            step.accounts.push(queue::QueueAccount::new(username, label));
        }
        self.config.steps.push(step);
        self.selected = self.config.steps.len() - 1;
    }

    /// The selected squad: its name, default script and accounts.
    fn squad(&mut self, ui: &mut Ui, status: &QueueStatus) {
        let i = self.selected;
        if i >= self.config.steps.len() {
            return;
        }
        let ctx = ui.ctx().clone();
        let mut action: Option<(Target, &'static str)> = None;
        let mut pick = false;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            let step = &mut self.config.steps[i];
            ui.horizontal_wrapped(|ui| {
                widgets::text_box(ui, &mut step.name, "Squad Name", 150.0);
                if let Some(state) = status.steps.get(i).filter(|_| status.running) {
                    let color = match state {
                        StepStatus::Running => SUCCESS,
                        StepStatus::Completed => MUTED,
                        StepStatus::Pending => WARNING,
                    };
                    ui.label(RichText::new(state.label()).size(11.0).color(color));
                }
            });
            ui.horizontal_wrapped(|ui| {
                match step.step_script_path.as_deref().filter(|p| !p.trim().is_empty()) {
                    Some(path) => {
                        ui.label(RichText::new(file_name(path)).monospace().color(TEXT))
                            .on_hover_text(path);
                    }
                    None => {
                        ui.label(
                            RichText::new("Default Script (Optional - applies to all squad members)")
                                .size(11.0)
                                .color(MUTED),
                        );
                    }
                }
                if widgets::small_button(ui, "Browse", true)
                    .on_hover_text("Browse and download scripts from repository")
                    .clicked()
                {
                    action = Some((Target::Step(i), "repo"));
                }
                if widgets::small_button(ui, "File…", !self.pending_script.is_open())
                    .on_hover_text("Select script file from folder")
                    .clicked()
                {
                    action = Some((Target::Step(i), "file"));
                }
                if step.has_script() {
                    if widgets::small_button(ui, "Options", true)
                        .on_hover_text("Configure options for this squad's script")
                        .clicked()
                    {
                        action = Some((Target::Step(i), "options"));
                    }
                    if widgets::small_button(ui, "✕", true)
                        .on_hover_text("Clear default script")
                        .clicked()
                    {
                        action = Some((Target::Step(i), "clear"));
                    }
                }
            });
            if widgets::button(ui, "+ Pick Accounts", Kind::Primary, true, size(130.0), "")
                .on_hover_text("Pick individual accounts to add to this squad")
                .clicked()
            {
                pick = true;
            }
            if step.accounts.is_empty() {
                ui.add(
                    egui::Label::new(
                        RichText::new("No players in this squad yet. Click '+ Pick Accounts' above to pick players.")
                            .color(MUTED),
                    )
                    .wrap(),
                );
                return;
            }
            let mut remove = None;
            for (j, account) in step.accounts.iter().enumerate() {
                ui.separator();
                ui.horizontal(|ui| {
                    let name = account.label().to_owned();
                    ui.scope(|ui| {
                        ui.set_max_width((ui.available_width() * 0.45).max(80.0));
                        ui.add(egui::Label::new(RichText::new(&name).strong().color(TEXT)).truncate())
                            .on_hover_text(&account.username);
                    });
                    let script = account.script_label();
                    let color = if account.has_script() { TEXT } else { MUTED };
                    ui.add(egui::Label::new(RichText::new(&script).monospace().size(11.0).color(color)).truncate())
                        .on_hover_text(account.script_path.as_deref().unwrap_or("Uses the squad's script"));
                    let state = status.member(i, j);
                    if status.running || state != MemberStatus::Idle {
                        let color = match state {
                            MemberStatus::Running => SUCCESS,
                            MemberStatus::AccountMissing | MemberStatus::LaunchFailed => DANGER,
                            _ => MUTED,
                        };
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(state.label()).size(10.0).strong().color(color));
                        });
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if widgets::small_button(ui, "Script", true)
                        .on_hover_text("Choose a script for this account (overrides the squad default)")
                        .clicked()
                    {
                        action = Some((Target::Member(i, j), "repo"));
                    }
                    if widgets::small_button(ui, "File…", !self.pending_script.is_open())
                        .on_hover_text("Select a script for this account from disk")
                        .clicked()
                    {
                        action = Some((Target::Member(i, j), "file"));
                    }
                    if account.has_script() {
                        if widgets::small_button(ui, "Options", true)
                            .on_hover_text("Configure options for this account's script")
                            .clicked()
                        {
                            action = Some((Target::Member(i, j), "options"));
                        }
                        if widgets::small_button(ui, "✕", true)
                            .on_hover_text("Revert to squad default script")
                            .clicked()
                        {
                            action = Some((Target::Member(i, j), "clear"));
                        }
                    }
                    if widgets::small_button(ui, "Remove", true)
                        .on_hover_text("Remove account from squad")
                        .clicked()
                    {
                        remove = Some(j);
                    }
                });
            }
            if let Some(j) = remove {
                step.accounts.remove(j);
            }
        });
        if pick {
            self.open_picker();
        }
        match action {
            Some((target, "repo")) => self.browse_repo(target),
            Some((target, "file")) => self.browse_file(&ctx, target),
            Some((target, "options")) => self.configure(target),
            Some((target, "clear")) => self.set_script(target, None),
            _ => {}
        }
    }

    /// "Preset:", Load, the name and "Save Preset".
    fn presets_bar(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Preset:").strong().size(11.0));
                let names = self.presets.names();
                ComboBox::from_id_salt("turu_army_queue_preset")
                    .width(160.0)
                    .truncate()
                    .selected_text(self.preset.as_str())
                    .show_ui(ui, |ui| {
                        ui.spacing_mut().interact_size.y = widgets::target(20.0, 40.0);
                        for name in names {
                            ui.selectable_value(&mut self.preset, name.clone(), name);
                        }
                    });
                if widgets::small_button(ui, "Load", !self.preset.is_empty()).clicked() {
                    self.load_preset();
                }
                ui.add_space(8.0);
                widgets::text_box(ui, &mut self.config.name, "Preset Name", 140.0);
                if widgets::small_button(ui, "Save Preset", true).clicked() {
                    self.save_preset();
                }
            });
        });
    }

    /// VibeSkua `LoadSelectedPreset`.
    fn load_preset(&mut self) {
        match self.presets.load_preset(&self.preset) {
            Some(config) => {
                self.config = config;
                self.selected = 0;
            }
            None => {
                self.notice = Some(("Load Error".into(), format!("Preset '{}' not found.", self.preset)));
            }
        }
    }

    /// VibeSkua `SavePreset`.
    fn save_preset(&mut self) {
        let file = crate::turu::bot().paths.army_queues_file();
        let mut presets = Presets::load(&file);
        let saved = presets.put(&self.config).and_then(|name| {
            presets
                .save(&file)
                .map(|()| name)
                .map_err(|e| format!("Failed to save preset: {e}"))
        });
        match saved {
            Ok(name) => {
                self.config.name = name.clone();
                self.presets = presets;
                self.notice = Some(("Preset Saved".into(), format!("Preset '{name}' saved successfully!")));
                self.preset = name;
            }
            Err(e) => self.notice = Some(("Save Preset".into(), e)),
        }
    }

    /// VibeSkua `OpenAccountPicker`.
    fn open_picker(&mut self) {
        let Some(step) = self.config.steps.get(self.selected) else {
            self.notice = Some(("Army Queue".into(), "Please create or select a squad first.".into()));
            return;
        };
        let accounts = Accounts::load(&crate::turu::bot().paths.accounts_file());
        self.picker = Some(Picker {
            search: String::new(),
            accounts: accounts
                .accounts
                .iter()
                .map(|a| {
                    let checked = step
                        .accounts
                        .iter()
                        .any(|m| m.username.eq_ignore_ascii_case(&a.username));
                    (a.username.clone(), a.label().to_owned(), checked)
                })
                .collect(),
        });
    }

    /// "Pick Accounts for Squad".
    fn picker_dialog(&mut self, ctx: &Context) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        let mut answer = None;
        let response = Modal::new(egui::Id::new("turu_army_queue_picker")).show(ctx, |ui| {
            let width = 380.0_f32.min(ctx.content_rect().width() - 48.0).max(200.0);
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Pick Accounts for Squad").size(14.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::small_button(ui, "✕", true).clicked() {
                        answer = Some(false);
                    }
                });
            });
            let needle = picker.search.trim().to_lowercase();
            let shows = |(username, label, _): &(String, String, bool)| {
                needle.is_empty() || username.to_lowercase().contains(&needle) || label.to_lowercase().contains(&needle)
            };
            ui.horizontal(|ui| {
                widgets::text_box(ui, &mut picker.search, "Search accounts...", (width - 90.0).max(80.0));
                if widgets::small_button(ui, "All", true).clicked() {
                    for account in picker.accounts.iter_mut().filter(|a| shows(a)) {
                        account.2 = true;
                    }
                }
                if widgets::small_button(ui, "None", true).clicked() {
                    for account in &mut picker.accounts {
                        account.2 = false;
                    }
                }
            });
            let height = (ctx.content_rect().height() * 0.5).clamp(120.0, 360.0);
            ScrollArea::vertical()
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if picker.accounts.is_empty() {
                        ui.label(RichText::new("No accounts yet: add them in Army > Accounts.").color(MUTED));
                    }
                    for account in picker.accounts.iter_mut().filter(|a| shows(a)) {
                        ui.horizontal(|ui| {
                            ui.set_min_height(widgets::target(22.0, 40.0));
                            ui.checkbox(&mut account.2, RichText::new(&account.1).strong());
                        });
                    }
                });
            ui.columns(2, |columns| {
                let width = columns[0].available_width();
                if widgets::button(&mut columns[0], "Done", Kind::Primary, true, vec2(width, 30.0), "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(&mut columns[1], "Cancel", Kind::Normal, true, vec2(width, 30.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => {
                let picked: Vec<(String, String)> = picker
                    .accounts
                    .iter()
                    .filter(|a| a.2)
                    .map(|(u, l, _)| (u.clone(), l.clone()))
                    .collect();
                self.config.set_members(self.selected, &picked);
                self.picker = None;
                self.auto_save();
            }
            Some(false) => self.picker = None,
            None if response.should_close() => self.picker = None,
            None => {}
        }
    }

    /// VibeSkua `StopQueue`'s question.
    fn stop_dialog(&mut self, ctx: &Context) {
        if !self.confirm_stop {
            return;
        }
        let mut answer = None;
        let response = Modal::new(egui::Id::new("turu_army_queue_stop")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading("Stop Queue");
            ui.label("Are you sure you want to stop the running Army Queue?");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Yes", Kind::Danger, true, size(72.0), "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "No", Kind::Normal, true, size(72.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        if answer == Some(true) {
            self.runner.stop();
        }
        if answer.is_some() || response.should_close() {
            self.confirm_stop = false;
        }
    }
}

/// The running queue's status and progress.
fn status_card(ui: &mut Ui, status: &QueueStatus) {
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{:.0}%", status.progress))
                        .size(11.0)
                        .color(MUTED),
                );
                ui.add(egui::Label::new(RichText::new(&status.text).size(11.0).strong()).truncate())
                    .on_hover_text(&status.text);
            });
        });
        ui.add(egui::ProgressBar::new((status.progress / 100.0) as f32).desired_height(6.0));
    });
}

fn size(width: f32) -> egui::Vec2 {
    vec2(width, widgets::target(28.0, 40.0))
}
