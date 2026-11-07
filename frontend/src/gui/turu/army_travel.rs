//! Army Control's travel (VibeSkua's tabbed host "Map Travel...", "Jump All to Player..." and
//! "Change Server for All..."): the Map Travel window with the maps the clients visited and the
//! saved map presets, the preset editor, the "Jump to Map / Cell" box, the player picker and the
//! server picker. What they jump to goes to Army Control's targets (`ArmyWindow::send`).
//!
//! Where each client is gets read every second while Turu runs, as VibeSkua's clients report it,
//! and a client's new map or room goes to the top of the history (`turu_bot::army::jump`).

use super::theme::{DANGER, MUTED};
use super::widgets::{self, Kind};
use egui::{Align, ComboBox, Context, Layout, Modal, RichText, Ui, vec2};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::army::Command;
use turu_bot::army::jump::{self, Destination, JumpSettings, Location, Visits};

/// How often the clients' locations are read (VibeSkua's one-second timer).
const POLL_EVERY: Duration = Duration::from_secs(1);
/// A location older than this isn't current (VibeSkua: 5 s).
const FRESH_FOR: Duration = Duration::from_secs(5);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// A row of Army Control, as travel needs it.
pub struct Peer {
    /// Army Control's row key ("tab:..." or the username, lowercase).
    pub key: String,
    /// The tab's bot, to read its location; `None` for a launched client.
    pub bot: Option<Arc<Bot>>,
    /// A launched client's login (its location comes from the monitor hub).
    pub username: String,
    /// The logged-in player's name, for the player picker.
    pub player: Option<String>,
}

#[derive(Default)]
struct Shared {
    settings: JumpSettings,
    visits: Visits,
    /// Each client's last location and when it was read.
    latest: HashMap<String, (Location, Instant)>,
}

/// The preset editor (VibeSkua `OpenJumpMapPresetEditor`).
struct Editor {
    /// The preset being edited, `None` for "Create Map Preset".
    editing: Option<usize>,
    preset: Destination,
    error: Option<String>,
}

/// The server picker: the list loads on another thread.
struct ServerPicker {
    list: Arc<Mutex<Option<Result<Vec<String>, String>>>>,
    picked: String,
}

pub struct Travel {
    shared: Arc<Mutex<Shared>>,
    loaded: bool,
    polling: Arc<AtomicBool>,
    polled: Option<Instant>,
    file: Option<std::path::PathBuf>,
    /// The Map Travel window is open.
    pub open: bool,
    history_pick: usize,
    preset_pick: usize,
    editor: Option<Editor>,
    manual: Option<String>,
    /// The player picker's box (`Some` while it shows).
    player: Option<(String, Vec<String>)>,
    server: Option<ServerPicker>,
    /// A message box: title and text.
    notice: Option<(String, String)>,
    /// Commands for Army Control to send to its targets.
    pub commands: Vec<Command>,
}

impl Default for Travel {
    fn default() -> Self {
        Self {
            shared: Arc::default(),
            loaded: false,
            polling: Arc::default(),
            polled: None,
            file: None,
            open: false,
            history_pick: 0,
            preset_pick: 0,
            editor: None,
            manual: None,
            player: None,
            server: None,
            notice: None,
            commands: Vec::new(),
        }
    }
}

impl Travel {
    fn file(&mut self) -> std::path::PathBuf {
        self.file
            .get_or_insert_with(|| crate::turu::bot().paths.army_jump_file())
            .clone()
    }

    fn ensure_loaded(&mut self) {
        if !self.loaded {
            self.loaded = true;
            let file = self.file();
            lock(&self.shared).settings = JumpSettings::load(&file);
        }
    }

    fn save(&mut self) {
        let file = self.file();
        if let Err(e) = lock(&self.shared).settings.save(&file) {
            crate::turu::bot()
                .logs
                .debug
                .push(format!("Couldn't save Army Control's jump settings: {e}"));
        }
    }

    /// Reads where every client is, once a second, on another thread; new visits go to the
    /// history.
    pub fn poll(&mut self, peers: &[Peer]) {
        self.ensure_loaded();
        if self.polled.is_some_and(|t| t.elapsed() < POLL_EVERY) || self.polling.swap(true, Ordering::AcqRel) {
            return;
        }
        self.polled = Some(Instant::now());
        let hub = crate::turu::monitor_hub();
        let now = turu_bot::gear::now_ms();
        let mut tabs = Vec::new();
        let mut found = Vec::new();
        for peer in peers {
            match &peer.bot {
                Some(bot) => tabs.push((peer.key.clone(), bot.clone())),
                None => {
                    let location = hub
                        .army_snapshot(&peer.username)
                        .filter(|s| now.saturating_sub(s.sampled_at) < FRESH_FOR.as_millis() as u64)
                        .and_then(|s| Location::from_full_name(&s.map, &s.cell, &s.pad, &peer.username));
                    if let Some(location) = location {
                        found.push((peer.key.clone(), location));
                    }
                }
            }
        }
        let keys: Vec<String> = peers.iter().map(|p| p.key.clone()).collect();
        let (shared, polling, file) = (self.shared.clone(), self.polling.clone(), self.file());
        let spawned = std::thread::Builder::new()
            .name("Turu Army Travel".into())
            .spawn(move || {
                // Flash calls: off the UI thread.
                for (key, bot) in tabs {
                    if let Some(location) = Location::read(&bot) {
                        found.push((key, location));
                    }
                }
                let mut shared = lock(&shared);
                let Shared {
                    settings,
                    visits,
                    latest,
                } = &mut *shared;
                visits.retain(|k| keys.iter().any(|key| key == k));
                latest.retain(|k, _| keys.iter().any(|key| key == k));
                let mut changed = false;
                for (key, location) in found {
                    changed |= visits.record(settings, &key, &location);
                    latest.insert(key, (location, Instant::now()));
                }
                if changed && let Err(e) = settings.save(&file) {
                    tracing::warn!("Couldn't save Army Control's jump settings: {e}");
                }
                drop(shared);
                polling.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.polling.store(false, Ordering::Release);
        }
    }

    /// The current location of the client `key`, while fresh.
    fn location(&self, key: Option<&str>) -> Option<Location> {
        let shared = lock(&self.shared);
        let (location, at) = shared.latest.get(key?)?;
        (at.elapsed() < FRESH_FOR && !location.map.is_empty()).then(|| location.clone())
    }

    /// VibeSkua `JumpArmyToDestination`.
    fn jump(&mut self, destination: &Destination) {
        lock(&self.shared).settings.remember_jump(destination);
        self.save();
        self.commands.push(destination.command());
    }

    /// "Jump to Player...": the picker, or why it can't open.
    pub fn open_player_picker(&mut self, peers: &[Peer]) {
        self.ensure_loaded();
        let mut players: Vec<String> = peers.iter().filter_map(|p| p.player.clone()).collect();
        players.sort_by_key(|p| p.to_lowercase());
        players.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        if players.is_empty() {
            self.notice = Some((
                "Jump All to Player".into(),
                "No logged-in player names were found in the open client tabs. Log in to at least one tab and try \
                 again."
                    .into(),
            ));
            return;
        }
        let last = lock(&self.shared).settings.player.clone();
        let initial = players
            .iter()
            .find(|p| p.eq_ignore_ascii_case(&last))
            .unwrap_or(&players[0])
            .clone();
        self.player = Some((initial, players));
    }

    /// "Change Server...": loads the online servers, then asks which.
    pub fn open_server_picker(&mut self) {
        let list = Arc::new(Mutex::new(None));
        let slot = list.clone();
        let bot = crate::turu::bot();
        let _ = std::thread::Builder::new().name("Turu Servers".into()).spawn(move || {
            let servers = turu_bot::api::servers::get_servers(&bot, true);
            let result = if servers.is_empty() {
                Err("Could not load the current server list. Check your connection and try again.".to_owned())
            } else {
                let mut names: Vec<String> = servers
                    .into_iter()
                    .filter(|s| s.online && !s.name.trim().is_empty() && !s.ip.trim().is_empty())
                    .map(|s| s.name.trim().to_owned())
                    .collect();
                names.sort_by_key(|n| n.to_lowercase());
                names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
                if names.is_empty() {
                    Err("The server API did not return any online servers.".to_owned())
                } else {
                    Ok(names)
                }
            };
            *lock(&slot) = Some(result);
        });
        self.server = Some(ServerPicker {
            list,
            picked: String::new(),
        });
    }

    /// The windows and dialogs that are open. `current` is the client "Use Current Client
    /// Location" reads (VibeSkua: the selected tab); `targets` how many clients a jump reaches.
    pub fn show(&mut self, ctx: &Context, current: Option<&str>, targets: usize) {
        if self.open {
            self.ensure_loaded();
            let mut open = true;
            super::window(ctx, "Map Travel", [520.0, 360.0], 6)
                .open(&mut open)
                .show(ctx, |ui| self.map_travel(ui, current));
            self.open &= open;
        }
        self.editor_dialog(ctx, current);
        self.manual_dialog(ctx);
        self.player_dialog(ctx);
        self.server_dialog(ctx, targets);
        if let Some((title, text)) = &self.notice {
            let mut close = false;
            let response = Modal::new(egui::Id::new("turu_army_travel_notice")).show(ctx, |ui| {
                dialog_width(ui, 360.0);
                ui.heading(title);
                ui.label(text);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = widgets::button(ui, "OK", Kind::Primary, true, button_size(72.0), "").clicked();
                });
            });
            if close || response.should_close() {
                self.notice = None;
            }
        }
    }

    /// VibeSkua `MenuItem_MapTravel_Click`.
    fn map_travel(&mut self, ui: &mut Ui, current: Option<&str>) {
        ui.label("Choose a recent destination or a saved preset to jump all clients.");
        ui.add_space(4.0);
        let (history, presets) = {
            let shared = lock(&self.shared);
            (shared.settings.history.clone(), shared.settings.presets.clone())
        };
        let width = ui.available_width();
        widgets::section_label(ui, "RECENT DESTINATIONS");
        self.history_pick = self.history_pick.min(history.len().saturating_sub(1));
        let selected = history
            .get(self.history_pick)
            .map_or_else(|| "No visited maps yet".to_owned(), Destination::display_name);
        ComboBox::from_id_salt("turu_army_history")
            .width(width)
            .truncate()
            .selected_text(selected)
            .show_ui(ui, |ui| {
                ui.spacing_mut().interact_size.y = widgets::target(20.0, 40.0);
                for (i, d) in history.iter().enumerate() {
                    let label = format!("{} ({})", d.display_name(), d.name);
                    ui.selectable_value(&mut self.history_pick, i, label);
                }
            })
            .response
            .on_hover_text("Maps the clients travelled to, the newest first");
        let any = !history.is_empty();
        ui.horizontal_wrapped(|ui| {
            if widgets::button(
                ui,
                "Select / Jump",
                Kind::Primary,
                any,
                button_size(110.0),
                "No visited maps yet",
            )
            .clicked()
            {
                self.jump(&history[self.history_pick]);
            }
            if widgets::button(ui, "Clear History", Kind::Normal, any, button_size(100.0), "").clicked() {
                lock(&self.shared).settings.clear_history();
                self.save();
            }
        });
        ui.separator();
        widgets::section_label(ui, "SAVED PRESETS");
        self.preset_pick = self.preset_pick.min(presets.len().saturating_sub(1));
        let selected = presets
            .get(self.preset_pick)
            .map_or_else(|| "No presets yet".to_owned(), Destination::preset_label);
        ComboBox::from_id_salt("turu_army_presets")
            .width(width)
            .truncate()
            .selected_text(selected)
            .show_ui(ui, |ui| {
                ui.spacing_mut().interact_size.y = widgets::target(20.0, 40.0);
                for (i, d) in presets.iter().enumerate() {
                    ui.selectable_value(&mut self.preset_pick, i, d.preset_label());
                }
            });
        let any = !presets.is_empty();
        ui.horizontal_wrapped(|ui| {
            if widgets::button(
                ui,
                "Select / Jump",
                Kind::Primary,
                any,
                button_size(110.0),
                "No presets yet",
            )
            .clicked()
            {
                self.jump(&presets[self.preset_pick]);
            }
            if widgets::button(ui, "Create", Kind::Normal, true, button_size(80.0), "").clicked() {
                self.open_editor(None, current);
            }
            if widgets::button(ui, "Edit", Kind::Normal, any, button_size(70.0), "").clicked() {
                self.open_editor(Some(self.preset_pick), current);
            }
            if widgets::button(ui, "Delete", Kind::Danger, any, button_size(80.0), "").clicked() {
                lock(&self.shared).settings.delete_preset(self.preset_pick);
                self.save();
            }
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if widgets::button(ui, "Jump to Map / Cell...", Kind::Normal, true, button_size(140.0), "").clicked() {
                self.manual = Some(lock(&self.shared).settings.manual_text());
            }
            if widgets::button(ui, "Close", Kind::Normal, true, button_size(84.0), "").clicked() {
                self.open = false;
            }
        });
    }

    fn open_editor(&mut self, editing: Option<usize>, current: Option<&str>) {
        let preset = match editing {
            Some(i) => lock(&self.shared).settings.presets.get(i).cloned(),
            // New presets start with the selected client's current location when there is one.
            None => Some(self.location(current).map(|l| l.to_destination()).unwrap_or_default()),
        };
        if let Some(preset) = preset {
            self.editor = Some(Editor {
                editing,
                preset,
                error: None,
            });
        }
    }

    fn editor_dialog(&mut self, ctx: &Context, current: Option<&str>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let current_location = {
            let shared = lock(&self.shared);
            current
                .and_then(|k| shared.latest.get(k))
                .filter(|(l, at)| at.elapsed() < FRESH_FOR && !l.map.is_empty())
                .map(|(l, _)| l.clone())
        };
        let title = if editor.editing.is_none() {
            "Create Map Preset"
        } else {
            "Edit Map Preset"
        };
        let mut answer = None;
        let response = Modal::new(egui::Id::new("turu_army_preset_editor")).show(ctx, |ui| {
            let width = dialog_width(ui, 420.0);
            ui.heading(title);
            // A phone's screen is shorter than five touch-sized rows: the fields scroll, the title
            // and the buttons stay on screen.
            scroll_body(ui, 150.0, |ui| {
                let p = &mut editor.preset;
                for (label, value, hint) in [
                    ("Preset name", &mut p.name, "Preset name"),
                    ("Map", &mut p.map, "Map name"),
                    ("Room number (optional)", &mut p.room, "Room number (optional)"),
                    ("Cell", &mut p.cell, "Cell"),
                    ("Pad", &mut p.pad, "Pad"),
                ] {
                    ui.label(label);
                    widgets::text_box(ui, value, hint, width);
                }
                if widgets::button(
                    ui,
                    "Use Current Client Location",
                    Kind::Normal,
                    true,
                    button_size(0.0),
                    "",
                )
                .clicked()
                {
                    match &current_location {
                        Some(location) => {
                            let name = std::mem::take(&mut p.name);
                            *p = location.to_destination();
                            if !name.trim().is_empty() {
                                p.name = name;
                            }
                            editor.error = None;
                        }
                        None => {
                            editor.error = Some(
                                "The selected client has not reported a current location yet. Log in and wait \
                                 a moment, then try again."
                                    .into(),
                            )
                        }
                    }
                }
                if let Some(error) = &editor.error {
                    ui.label(RichText::new(error).color(DANGER));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Save Preset", Kind::Primary, true, button_size(100.0), "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, button_size(84.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => {
                let (editing, preset) = (editor.editing, editor.preset.clone());
                let saved = lock(&self.shared).settings.save_preset(editing, preset);
                match saved {
                    Ok(index) => {
                        self.preset_pick = index;
                        self.editor = None;
                        self.save();
                    }
                    Err(e) => editor.error = Some(e),
                }
            }
            Some(false) => self.editor = None,
            None if response.should_close() => self.editor = None,
            None => {}
        }
    }

    /// VibeSkua `MenuItem_JumpMap_Click`: "Jump Army to Map / Cell".
    fn manual_dialog(&mut self, ctx: &Context) {
        let Some(text) = &mut self.manual else {
            return;
        };
        let mut answer = None;
        let response = Modal::new(egui::Id::new("turu_army_manual_jump")).show(ctx, |ui| {
            let width = dialog_width(ui, 380.0);
            ui.heading("Jump Army to Map / Cell");
            ui.label("Enter destination as Map,Room,Cell (room is optional):");
            let edit = widgets::text_box(ui, text, "e.g., yulgar,12345,Enter", width);
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "OK", Kind::Primary, true, button_size(72.0), "").clicked() || enter {
                    answer = Some(true);
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, button_size(72.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => {
                let destination = jump::parse_manual(text);
                self.manual = None;
                if let Some(destination) = destination {
                    self.jump(&destination);
                }
            }
            Some(false) => self.manual = None,
            None if response.should_close() => self.manual = None,
            None => {}
        }
    }

    /// VibeSkua `MenuItem_JumpPlayer_Click`: the tabs' players, or any name typed in.
    fn player_dialog(&mut self, ctx: &Context) {
        let Some((text, players)) = &mut self.player else {
            return;
        };
        let mut answer = None;
        let response = Modal::new(egui::Id::new("turu_army_jump_player")).show(ctx, |ui| {
            let width = dialog_width(ui, 340.0);
            ui.heading("Jump Army to Player");
            ui.label("Select a player from the open client tabs:");
            ui.horizontal(|ui| {
                widgets::text_box(ui, text, "Player name", (width - 40.0).max(80.0));
                ui.menu_button("▾", |ui| {
                    ui.spacing_mut().interact_size.y = widgets::target(20.0, 40.0);
                    for player in players.iter() {
                        if ui.selectable_label(text.eq_ignore_ascii_case(player), player).clicked() {
                            *text = player.clone();
                            ui.close();
                        }
                    }
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let name = !text.trim().is_empty();
                if widgets::button(ui, "Jump", Kind::Primary, name, button_size(84.0), "Enter a name").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, button_size(84.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => {
                let name = text.trim().to_owned();
                self.player = None;
                lock(&self.shared).settings.player = name.clone();
                self.save();
                self.commands.push(Command::JumpPlayer(name));
            }
            Some(false) => self.player = None,
            None if response.should_close() => self.player = None,
            None => {}
        }
    }

    /// VibeSkua `MenuItem_ChangeServerAll_Click`.
    fn server_dialog(&mut self, ctx: &Context, targets: usize) {
        let Some(picker) = &mut self.server else {
            return;
        };
        let list = lock(&picker.list).clone();
        let names = match list {
            None => {
                ctx.request_repaint_after(Duration::from_millis(200));
                None
            }
            Some(Err(e)) => {
                self.server = None;
                self.notice = Some(("Change Server for All".into(), e));
                return;
            }
            Some(Ok(names)) => Some(names),
        };
        if let Some(names) = &names
            && picker.picked.is_empty()
        {
            picker.picked = names
                .iter()
                .find(|n| n.eq_ignore_ascii_case("Artix"))
                .unwrap_or(&names[0])
                .clone();
        }
        let mut answer = None;
        let response = Modal::new(egui::Id::new("turu_army_change_server")).show(ctx, |ui| {
            let width = dialog_width(ui, 340.0);
            ui.heading("Change Server for All");
            match &names {
                None => {
                    ui.label(RichText::new("Loading servers…").color(MUTED));
                }
                Some(names) => {
                    ui.label("Select the server for all open clients:");
                    ComboBox::from_id_salt("turu_army_server_pick")
                        .width(width)
                        .selected_text(picker.picked.as_str())
                        .show_ui(ui, |ui| {
                            ui.spacing_mut().interact_size.y = widgets::target(20.0, 40.0);
                            for name in names {
                                ui.selectable_value(&mut picker.picked, name.clone(), name);
                            }
                        });
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ready = names.is_some() && !picker.picked.is_empty();
                if widgets::button(ui, "Change Server", Kind::Primary, ready, button_size(110.0), "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, button_size(84.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => {
                let server = picker.picked.trim().to_owned();
                self.server = None;
                if targets == 0 {
                    self.notice = Some((
                        "Change Server for All".into(),
                        "No active client windows were available to receive the server change.".into(),
                    ));
                } else {
                    self.commands.push(Command::ChangeServer(server));
                }
            }
            Some(false) => self.server = None,
            None if response.should_close() => self.server = None,
            None => {}
        }
    }
}

/// A dialog as wide as `width`, or the screen less a margin on a phone. Returns the width.
fn dialog_width(ui: &mut Ui, width: f32) -> f32 {
    let width = width.min(ui.ctx().content_rect().width() - 48.0).max(200.0);
    ui.set_width(width);
    width
}

/// A dialog's middle, scrolling when the screen is too short for it; `reserve` is the height its
/// title and buttons need.
fn scroll_body(ui: &mut Ui, reserve: f32, add: impl FnOnce(&mut Ui)) {
    let max = (ui.ctx().content_rect().height() - reserve).max(120.0);
    egui::ScrollArea::vertical()
        .max_height(max)
        .auto_shrink([false, true])
        .show(ui, add);
}

fn button_size(width: f32) -> egui::Vec2 {
    vec2(width, widgets::target(28.0, 40.0))
}
