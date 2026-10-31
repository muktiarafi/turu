//! Army Control > ⋯ > Gear (docs/ARMY_EQUIPMENT_PROPOSAL.md, phase 1): one account's worn
//! slots and what it could wear instead, from its gear file (`gear/<username>.json`, which its
//! client keeps current), so it works for a client without a window and shows an offline
//! account's last gear. Equip and Load bank go to the client through its control API.

use super::army::Peer;
use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{Context, Grid, RichText, Ui, vec2};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::army::Command;
use turu_bot::gear::{self, EquipReport, GearFile, Place, Slot};

/// How often the window reads the gear file again.
const RELOAD: Duration = Duration::from_secs(1);

/// What the last action said.
#[derive(Default)]
struct Outcome {
    /// The action running now.
    busy: Option<String>,
    last: Option<(bool, String)>,
}

pub struct GearWindow {
    pub peer: Peer,
    pub open: bool,
    file: Option<GearFile>,
    read_at: Option<Instant>,
    /// The item picked per slot (its ID), when it isn't what's worn.
    picked: HashMap<Slot, i64>,
    /// "Stop the script first": unchecked each time the window opens.
    stop_script: bool,
    outcome: Arc<Mutex<Outcome>>,
}

impl GearWindow {
    pub fn new(peer: Peer) -> Self {
        Self {
            peer,
            open: true,
            file: None,
            read_at: None,
            picked: HashMap::new(),
            stop_script: false,
            outcome: Arc::default(),
        }
    }

    /// Opens it again (the stop box starts unchecked).
    pub fn reopen(&mut self) {
        self.open = true;
        self.stop_script = false;
    }

    pub fn show(&mut self, ctx: &Context) {
        if !self.open {
            return;
        }
        if self.read_at.is_none_or(|t| t.elapsed() >= RELOAD) {
            self.read_at = Some(Instant::now());
            let username = self.peer.username();
            self.file = (!username.is_empty())
                .then(|| gear::read(&crate::turu::bot().paths.gear_dir(), &username))
                .flatten();
        }
        let title = format!("Gear: {}", self.peer.label());
        let mut open = self.open;
        super::window(ctx, &title, [460.0, 440.0], 6)
            .open(&mut open)
            .show(ctx, |ui| self.ui(ui));
        self.open = open;
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let online = self.peer.online();
        let script = self.peer.running_script();
        let busy = self.outcome.lock().unwrap_or_else(|e| e.into_inner()).busy.clone();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (color, state) = if online {
                    (SUCCESS, "online")
                } else {
                    (MUTED, "offline")
                };
                widgets::dot(ui, color, 4.0);
                ui.label(RichText::new(self.peer.label()).strong().color(TEXT));
                ui.label(RichText::new(state).small().color(MUTED));
                if let Some(name) = &script {
                    ui.label(RichText::new(format!("Script running: {name}")).small().color(WARNING));
                }
            });
            ui.horizontal(|ui| {
                let bank = match self.file.as_ref().map(|f| f.bank_as_of) {
                    None => "No gear file yet: it's written once the account logs in".to_owned(),
                    Some(None) => "Bank not read yet".to_owned(),
                    Some(Some(at)) => format!("Bank as of {}", ago(at)),
                };
                ui.label(RichText::new(bank).small().color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can = online && busy.is_none() && (script.is_none() || self.stop_script);
                    if widgets::small_button(ui, "Load bank", can)
                        .on_hover_text("Open and load the bank, and add it to the gear file")
                        .clicked()
                    {
                        self.run("Load bank", Command::LoadBank { stop: self.stop_script });
                    }
                    if widgets::small_button(ui, "Refresh", online && busy.is_none())
                        .on_hover_text("Read the gear from the game now")
                        .clicked()
                    {
                        self.run("Refresh", Command::RefreshGear);
                    }
                });
            });
            if script.is_some() {
                ui.horizontal(|ui| {
                    widgets::switch(ui, &mut self.stop_script);
                    ui.label("Stop the script first").on_hover_text(
                        "Gear isn't changed under a running script. This stops it, and it stays stopped.",
                    );
                });
            }
        });
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "SLOTS");
            let Some(file) = self.file.clone() else {
                ui.label(RichText::new("Nothing to show yet.").color(MUTED));
                return;
            };
            let can_equip = online && busy.is_none() && (script.is_none() || self.stop_script);
            let mut equip = None;
            Grid::new("turu_gear_slots")
                .num_columns(3)
                .spacing(vec2(8.0, 6.0))
                .show(ui, |ui| {
                    for slot in Slot::ALL {
                        ui.label(RichText::new(slot.label()).color(MUTED));
                        let worn = file.worn(slot);
                        let choices = file.for_slot(slot);
                        let picked = self.picked.get(&slot).copied();
                        let shown = picked.and_then(|id| choices.iter().find(|i| i.id == id)).map_or_else(
                            || worn.map_or_else(|| "(none)".to_owned(), |i| i.name.clone()),
                            |i| item_label(i),
                        );
                        egui::ComboBox::from_id_salt(("turu_gear_slot", slot))
                            .selected_text(shown)
                            .width(240.0)
                            .show_ui(ui, |ui| {
                                for item in &choices {
                                    let selected = picked.map_or(item.equipped, |id| id == item.id);
                                    if ui.selectable_label(selected, item_label(item)).clicked() {
                                        if item.equipped {
                                            self.picked.remove(&slot);
                                        } else {
                                            self.picked.insert(slot, item.id);
                                        }
                                    }
                                }
                                if choices.is_empty() {
                                    ui.label(RichText::new("Nothing for this slot").color(MUTED));
                                }
                            });
                        let target = picked.filter(|id| worn.is_none_or(|w| w.id != *id));
                        let tooltip = if script.is_some() && !self.stop_script {
                            "A script is running: check \"Stop the script first\""
                        } else if !online {
                            "The account isn't online"
                        } else {
                            "Pick another item first"
                        };
                        if widgets::button(
                            ui,
                            "Equip",
                            Kind::Primary,
                            can_equip && target.is_some(),
                            vec2(0.0, 22.0),
                            tooltip,
                        )
                        .clicked()
                            && let Some(id) = target
                        {
                            equip = Some((slot, id));
                        }
                        ui.end_row();
                    }
                });
            if let Some((slot, id)) = equip {
                self.picked.remove(&slot);
                let name = file
                    .items
                    .iter()
                    .find(|i| i.id == id)
                    .map_or_else(|| id.to_string(), |i| i.name.clone());
                self.run(
                    &format!("Equip {name}"),
                    Command::Equip {
                        item: id.to_string(),
                        stop: self.stop_script,
                    },
                );
            }
        });
        let outcome = self.outcome.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(action) = &outcome.busy {
            ui.label(RichText::new(format!("{action}…")).small().color(MUTED));
        } else if let Some((ok, text)) = &outcome.last {
            ui.label(RichText::new(text).small().color(if *ok { TEXT } else { DANGER }));
        }
    }

    /// Sends the command off the UI thread, then reads the gear again.
    fn run(&self, action: &str, command: Command) {
        let (peer, outcome, action) = (self.peer.clone(), self.outcome.clone(), action.to_owned());
        outcome.lock().unwrap_or_else(|e| e.into_inner()).busy = Some(action.clone());
        let spawned = std::thread::Builder::new()
            .name("Turu Gear Action".into())
            .spawn(move || {
                let result = peer.send(&command).map(|reply| match command {
                    Command::Equip { .. } => serde_json::from_value::<EquipReport>(reply)
                        .map_or_else(|_| "done".to_owned(), |report| report.summary()),
                    Command::LoadBank { .. } => match reply["stoppedScript"].as_str() {
                        Some(name) => format!("stopped {name}; bank loaded"),
                        None => "bank loaded".to_owned(),
                    },
                    _ => "gear read".to_owned(),
                });
                if !matches!(command, Command::RefreshGear) {
                    let _ = peer.send(&Command::RefreshGear);
                }
                let mut outcome = outcome.lock().unwrap_or_else(|e| e.into_inner());
                outcome.busy = None;
                outcome.last = Some(match result {
                    Ok(text) => (true, format!("{action}: {text}")),
                    Err(e) => (false, format!("{action}: {e}")),
                });
            });
        if let Err(e) = spawned {
            let mut outcome = self.outcome.lock().unwrap_or_else(|e| e.into_inner());
            outcome.busy = None;
            outcome.last = Some((false, e.to_string()));
        }
    }
}

/// "Necrotic Sword (bank)", "Legion Revenant (worn)".
fn item_label(item: &gear::GearItem) -> String {
    if item.equipped {
        format!("{} (worn)", item.name)
    } else if item.place == Place::Bank {
        format!("{} (bank)", item.name)
    } else {
        item.name.clone()
    }
}

/// "just now", "12 min ago", "3 h ago", "2 days ago".
fn ago(at_ms: u64) -> String {
    let minutes = gear::now_ms().saturating_sub(at_ms) / 60_000;
    match minutes {
        0 => "just now".into(),
        1..60 => format!("{minutes} min ago"),
        60..2880 => format!("{} h ago", minutes / 60),
        _ => format!("{} days ago", minutes / 1440),
    }
}
