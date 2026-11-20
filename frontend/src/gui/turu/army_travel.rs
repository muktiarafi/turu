//! Army Control's travel (VibeSkua's tabbed host "Map Travel...", "Jump All to Player..." and
//! "Change Server for All..."): the Map Travel window, with Go to (a map, room, cell and pad typed
//! in; Turu's, in place of VibeSkua's "Jump to Map / Cell" box), the maps the clients visited and
//! the saved map presets; the preset editor, the player picker and the server picker. What they
//! jump to goes to Army Control's targets (`ArmyWindow::send`).
//!
//! The settings, where each client is and the server list are the model's (`models::army_travel`).

use super::theme::{DANGER, MUTED, WARNING};
use super::widgets::{self, Kind};
use crate::models::army_travel::{PADS, Servers, TRAVEL, Travel as Model};
use egui::{Align, ComboBox, Context, Layout, Modal, RichText, Ui, vec2};
use std::time::Duration;
use turu_bot::army::Command;
use turu_bot::army::jump::{self, Destination, Location};

pub use crate::models::army_travel::Peer;

/// The room between controls on a row.
const GAP: f32 = 6.0;
/// The width of a Jump button (and Go to's Room box).
const JUMP_WIDTH: f32 = 84.0;
/// Narrower than this, Go to's Jump gets a row of its own and the presets' header buttons shrink.
const NARROW: f32 = 320.0;

/// The preset editor (VibeSkua `OpenJumpMapPresetEditor`).
struct Editor {
    /// The preset being edited, `None` for "Create Map Preset".
    editing: Option<usize>,
    preset: Destination,
    error: Option<String>,
}

/// The windows and pickers; the settings, the clients' locations and the server list are the
/// model's (`models::army_travel`, shared with a native page).
#[derive(Default)]
pub struct Travel {
    /// The model, when not the process's own (tests).
    model: Option<&'static Model>,
    loaded: bool,
    /// The Map Travel window is open.
    pub open: bool,
    history_pick: usize,
    preset_pick: usize,
    editor: Option<Editor>,
    /// Go to's fields (`name` unused); they start with the last jump.
    go_to: Destination,
    /// The player picker's box (`Some` while it shows).
    player: Option<(String, Vec<String>)>,
    /// The server picker's choice (`Some` while it shows).
    server: Option<String>,
    /// A message box: title and text.
    notice: Option<(String, String)>,
    /// Commands for Army Control to send to its targets.
    pub commands: Vec<Command>,
}

impl Travel {
    fn model(&self) -> &'static Model {
        self.model.unwrap_or(&TRAVEL)
    }

    fn ensure_loaded(&mut self) {
        if !self.loaded {
            self.loaded = true;
            self.go_to = self.model().settings().last_jump();
        }
    }

    /// Reads where every client is, once a second, on another thread; new visits go to the
    /// history.
    pub fn poll(&mut self, peers: &[Peer]) {
        self.ensure_loaded();
        self.model().poll(peers);
    }

    /// The current location of the client `key`, while fresh.
    fn location(&self, key: Option<&str>) -> Option<Location> {
        self.model().location(key?)
    }

    /// Map Travel or one of its pickers is open: the clients' locations are wanted.
    pub fn is_open(&self) -> bool {
        self.open || self.editor.is_some() || self.player.is_some() || self.server.is_some()
    }

    /// VibeSkua `JumpArmyToDestination`.
    fn jump(&mut self, destination: &Destination) {
        let command = self.model().jump(destination);
        self.commands.push(command);
    }

    /// "Jump to Player...": the picker, or why it can't open.
    pub fn open_player_picker(&mut self, peers: &[Peer]) {
        self.ensure_loaded();
        let players = Model::players(peers);
        if players.is_empty() {
            self.notice = Some((
                "Jump All to Player".into(),
                "No logged-in player names were found in the open client tabs. Log in to at least one tab and try \
                 again."
                    .into(),
            ));
            return;
        }
        let initial = self.model().initial_player(&players).unwrap_or_default();
        self.player = Some((initial, players));
    }

    /// "Change Server...": loads the online servers, then asks which.
    pub fn open_server_picker(&mut self) {
        self.model().load_servers();
        self.server = Some(String::new());
    }

    /// The windows and dialogs that are open. `current` is the client "Use Current Client
    /// Location" reads (VibeSkua: the selected tab); `targets` how many clients a jump reaches.
    pub fn show(&mut self, ctx: &Context, current: Option<&str>, targets: usize) {
        if self.open {
            self.ensure_loaded();
            let mut open = true;
            super::window(ctx, "Map Travel", [520.0, 380.0], 6)
                .open(&mut open)
                .show(ctx, |ui| self.map_travel(ui, current, targets));
            self.open &= open;
        }
        self.editor_dialog(ctx, current);
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

    /// VibeSkua `MenuItem_MapTravel_Click`: Go to, then the recent destinations and the saved
    /// presets, each a drop-down with its Jump beside it. `targets` is how many clients a jump
    /// reaches; none, and the Jumps are off.
    fn map_travel(&mut self, ui: &mut Ui, current: Option<&str>, targets: usize) {
        let settings = self.model().settings();
        let (history, presets) = (settings.history, settings.presets);
        let control = widgets::target(28.0, 40.0);
        ui.spacing_mut().item_spacing = vec2(GAP, 8.0);
        ui.spacing_mut().interact_size.y = control;
        let narrow = ui.available_width() < NARROW;
        let reach = targets > 0;
        let off = "No clients to send: check a row in Army Control, or start one";

        self.go_to_ui(ui, current, targets, control, narrow);
        ui.separator();

        ui.horizontal(|ui| {
            widgets::section_label(ui, "RECENT DESTINATIONS");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let clear = header_button(ui, "Clear", Kind::Normal, !history.is_empty(), "Nothing to clear")
                    .on_hover_text("Forget the visited maps");
                if clear.clicked() {
                    self.model().change(|s| s.clear_history());
                }
            });
        });
        self.history_pick = self.history_pick.min(history.len().saturating_sub(1));
        let jump = picker_row(
            ui,
            "turu_army_history",
            control,
            reach && !history.is_empty(),
            off,
            |ui| {
                let selected = history
                    .get(self.history_pick)
                    .map_or_else(|| "No visited maps yet".to_owned(), Destination::display_name);
                picker_combo(ui, "turu_army_history", selected, |ui| {
                    for (i, d) in history.iter().enumerate() {
                        let label = format!("{}  ·  {}", d.display_name(), d.name);
                        ui.selectable_value(&mut self.history_pick, i, label);
                    }
                })
                .on_hover_text("Maps the clients travelled to, the newest first");
            },
        );
        if jump {
            self.jump(&history[self.history_pick]);
        }
        ui.separator();

        let any = !presets.is_empty();
        ui.horizontal(|ui| {
            widgets::section_label(ui, "SAVED PRESETS");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if header_button(ui, "Delete", Kind::Danger, any, "No presets yet").clicked() {
                    let pick = self.preset_pick;
                    self.model().change(|s| s.delete_preset(pick));
                }
                if header_button(ui, "Edit", Kind::Normal, any, "No presets yet").clicked() {
                    self.open_editor(Some(self.preset_pick), current);
                }
                let new = header_button(ui, if narrow { "New" } else { "New…" }, Kind::Normal, true, "")
                    .on_hover_text("Save a preset: Go to's destination when one is typed, else where the client is");
                if new.clicked() {
                    self.open_editor(None, current);
                }
            });
        });
        self.preset_pick = self.preset_pick.min(presets.len().saturating_sub(1));
        let jump = picker_row(ui, "turu_army_presets", control, reach && any, off, |ui| {
            let selected = presets
                .get(self.preset_pick)
                .map_or_else(|| "No presets yet".to_owned(), Destination::preset_label);
            picker_combo(ui, "turu_army_presets", selected, |ui| {
                for (i, d) in presets.iter().enumerate() {
                    ui.selectable_value(&mut self.preset_pick, i, d.preset_label());
                }
            });
        });
        if jump && let Some(preset) = presets.get(self.preset_pick) {
            self.jump(preset);
        }
    }

    /// Go to: a map, its room (optional: any room), then the cell and pad, which start on Enter
    /// and Spawn. Enter in a box jumps; `map,room,cell,pad` pasted into Map fills them all.
    fn go_to_ui(&mut self, ui: &mut Ui, current: Option<&str>, targets: usize, control: f32, narrow: bool) {
        let here = self.location(current);
        ui.horizontal(|ui| {
            widgets::section_label(ui, "GO TO");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let use_current = header_button(
                    ui,
                    "Use current",
                    Kind::Normal,
                    here.is_some(),
                    "The selected client hasn't said where it is yet",
                )
                .on_hover_text("Fill in where the selected client is (the only checked row, else the active tab)");
                if use_current.clicked()
                    && let Some(here) = &here
                {
                    self.go_to = here.to_destination();
                }
            });
        });
        let width = ui.available_width();
        let mut enter = false;
        ui.horizontal(|ui| {
            let map = go_to_box(ui, &mut self.go_to.map, "Map", width - JUMP_WIDTH - GAP, control);
            if map.changed()
                && self.go_to.map.contains(',')
                && let Some(pasted) = jump::parse_manual(&self.go_to.map)
            {
                self.go_to = pasted;
            }
            enter |= submitted(ui, &map);
            let room = go_to_box(ui, &mut self.go_to.room, "Room", JUMP_WIDTH, control)
                .on_hover_text("The room number; empty for any room");
            if room.changed() {
                self.go_to.room.retain(|c| c.is_ascii_digit());
            }
            enter |= submitted(ui, &room);
        });
        let valid = !self.go_to.map.trim().is_empty();
        let mut jump = false;
        ui.horizontal(|ui| {
            let fields = if narrow { width } else { width - JUMP_WIDTH - GAP };
            let half = (fields - GAP) / 2.0;
            let cell = go_to_box(ui, &mut self.go_to.cell, "Cell (Enter)", half, control);
            enter |= submitted(ui, &cell);
            let pad = self.go_to.pad.trim().to_owned();
            ComboBox::from_id_salt("turu_army_go_to_pad")
                .selected_text(if pad.is_empty() { "Spawn" } else { pad.as_str() })
                .width(half)
                .show_ui(ui, |ui| {
                    ui.spacing_mut().interact_size.y = widgets::target(20.0, 40.0);
                    for p in PADS {
                        if ui.selectable_label(pad.eq_ignore_ascii_case(p), p).clicked() {
                            self.go_to.pad = p.to_owned();
                        }
                    }
                })
                .response
                .on_hover_text("The pad to stand on in that cell");
            if !narrow {
                jump = jump_button(ui, valid && targets > 0, control, JUMP_WIDTH);
            }
        });
        if narrow {
            jump = jump_button(ui, valid && targets > 0, control, width);
        }
        // What a jump does, in words: where, and to how many clients.
        let (text, color) = if targets == 0 {
            ("No clients to send.".to_owned(), WARNING)
        } else if valid {
            let clients = if targets == 1 {
                "1 client".to_owned()
            } else {
                format!("{targets} clients")
            };
            let d = &self.go_to;
            let place = format!("{} - {}, {}", d.map_to_join(), d.cell_or_default(), d.pad_or_default());
            (format!("Sends {clients} to {place}"), MUTED)
        } else {
            (
                "Type a map to go to; the room, cell and pad are optional.".to_owned(),
                MUTED,
            )
        };
        ui.add(egui::Label::new(RichText::new(text).size(widgets::small_label_size()).color(color)).truncate());
        if (jump || enter) && valid && targets > 0 {
            let destination = self.go_to.clone();
            self.jump(&destination);
        }
    }

    fn open_editor(&mut self, editing: Option<usize>, current: Option<&str>) {
        let preset = match editing {
            Some(i) => self.model().settings().presets.get(i).cloned(),
            // New presets start with Go to's destination when one is typed, else the selected
            // client's current location when there is one.
            None if !self.go_to.map.trim().is_empty() => Some(Destination {
                name: String::new(),
                ..self.go_to.clone()
            }),
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
        let model = self.model();
        let Some(editor) = &mut self.editor else {
            return;
        };
        let current_location = current.and_then(|k| model.location(k));
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
                let saved = model.change(|s| s.save_preset(editing, preset));
                match saved {
                    Ok(index) => {
                        self.preset_pick = index;
                        self.editor = None;
                    }
                    Err(e) => editor.error = Some(e),
                }
            }
            Some(false) => self.editor = None,
            None if response.should_close() => self.editor = None,
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
                let command = self.model().jump_player(&name);
                self.commands.push(command);
            }
            Some(false) => self.player = None,
            None if response.should_close() => self.player = None,
            None => {}
        }
    }

    /// VibeSkua `MenuItem_ChangeServerAll_Click`.
    fn server_dialog(&mut self, ctx: &Context, targets: usize) {
        let model = self.model();
        let Some(picked) = &mut self.server else {
            return;
        };
        let names = match model.servers() {
            Servers::None | Servers::Loading => {
                ctx.request_repaint_after(Duration::from_millis(200));
                None
            }
            Servers::Failed(e) => {
                self.server = None;
                model.forget_servers();
                self.notice = Some(("Change Server for All".into(), e));
                return;
            }
            Servers::Ready(names) => Some(names),
        };
        if let Some(names) = &names
            && picked.is_empty()
        {
            *picked = names
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
                        .selected_text(picked.as_str())
                        .show_ui(ui, |ui| {
                            ui.spacing_mut().interact_size.y = widgets::target(20.0, 40.0);
                            for name in names {
                                ui.selectable_value(picked, name.clone(), name);
                            }
                        });
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ready = names.is_some() && !picked.is_empty();
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
                let server = picked.trim().to_owned();
                self.server = None;
                model.forget_servers();
                if targets == 0 {
                    self.notice = Some((
                        "Change Server for All".into(),
                        "No active client windows were available to receive the server change.".into(),
                    ));
                } else {
                    self.commands.push(Command::ChangeServer(server));
                }
            }
            Some(false) => {
                self.server = None;
                model.forget_servers();
            }
            None if response.should_close() => {
                self.server = None;
                model.forget_servers();
            }
            _ => {}
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

/// A small button in a section's heading row: finger-sized on a touch screen.
fn header_button(ui: &mut Ui, label: &str, kind: Kind, enabled: bool, disabled: &str) -> egui::Response {
    widgets::button(
        ui,
        label,
        kind,
        enabled,
        vec2(0.0, widgets::target(22.0, 40.0)),
        disabled,
    )
}

/// A drop-down filling the row but for its Jump on the right; true when Jump was clicked.
fn picker_row(ui: &mut Ui, id: &str, control: f32, enabled: bool, disabled: &str, combo: impl FnOnce(&mut Ui)) -> bool {
    let width = ui.available_width();
    ui.push_id(id, |ui| {
        ui.horizontal(|ui| {
            ui.allocate_ui(vec2(width - JUMP_WIDTH - GAP, control), combo);
            widgets::button(ui, "Jump", Kind::Primary, enabled, vec2(JUMP_WIDTH, control), disabled).clicked()
        })
        .inner
    })
    .inner
}

/// A drop-down as wide as the room it's given, its text cut off with an ellipsis.
fn picker_combo(ui: &mut Ui, id: &str, selected: String, add: impl FnOnce(&mut Ui)) -> egui::Response {
    ComboBox::from_id_salt(id)
        .width(ui.available_width())
        .truncate()
        .selected_text(selected)
        .show_ui(ui, |ui| {
            ui.spacing_mut().interact_size.y = widgets::target(20.0, 40.0);
            add(ui);
        })
        .response
}

/// One of Go to's boxes.
fn go_to_box(ui: &mut Ui, text: &mut String, hint: &str, width: f32, control: f32) -> egui::Response {
    widgets::text_box_sized(ui, text, hint, vec2(width.max(48.0), control))
}

/// Enter was pressed in this box.
fn submitted(ui: &Ui, response: &egui::Response) -> bool {
    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

/// Go to's Jump, `width` wide.
fn jump_button(ui: &mut Ui, enabled: bool, control: f32, width: f32) -> bool {
    widgets::button(
        ui,
        "Jump",
        Kind::Primary,
        enabled,
        vec2(width, control),
        "Type a map, and have a client to send",
    )
    .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::turu::tests::{Show, last_wide, run};

    /// Map Travel, typed into and with a preset and a visit, stays on screen from a desktop down
    /// to a phone's width, and nothing in it is cut off at the right.
    #[test]
    fn map_travel_fits_any_width() {
        let dir = std::env::temp_dir().join(format!("turu-map-travel-{}", std::process::id()));
        let model: &'static Model = Box::leak(Box::new(Model::new(dir.join("army-control-jump.json"))));
        let mut travel = Travel {
            model: Some(model),
            loaded: true,
            open: true,
            go_to: Destination {
                map: "tercessuinotlim".into(),
                room: "123456789".into(),
                cell: "Boss".into(),
                pad: "Left".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        {
            let long = Destination {
                name: "A preset with a long name to cut off".into(),
                map: "ultradage".into(),
                room: "9999".into(),
                cell: "Boss".into(),
                pad: "Left".into(),
            };
            model.change(|s| {
                s.presets = vec![long.clone()];
                s.history = vec![long];
            });
        }
        let mut show: Show = Box::new(move |ctx, _| {
            travel.show(ctx, None, 3);
        });
        for screen in [
            vec2(1400.0, 1000.0),
            vec2(900.0, 600.0),
            vec2(640.0, 420.0),
            vec2(360.0, 740.0),
            vec2(300.0, 600.0),
        ] {
            let (rect, area, overflow) = run(screen, "Map Travel", &mut show);
            assert!(
                area.expand(1.0).contains_rect(rect),
                "{screen:?}: {rect:?} outside {area:?}"
            );
            assert!(last_wide() <= 0.5, "{screen:?}: {} wider than the window", last_wide());
            if screen.y >= 1000.0 {
                assert!(overflow <= 0.5, "scrolls at its default size by {overflow}");
            }
        }
    }
}
