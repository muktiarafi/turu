//! Diagnostics > Spammer (VibeSkua `PacketSpammerView`): Start/Stop, "Every n ms", Send to
//! client, the QUEUE (Load, Save, Clear; numbered rows, ✕ or Delete removes one) and the
//! composer (Add, Send now). The queue and the loop are the tab's `bot.spammer`.

use super::files::Pending;
use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::gui::FilePicker;
use egui::{Align, Context, DragValue, Key, Label, Layout, RichText, ScrollArea, Ui, vec2};
use rfd::AsyncFileDialog;
use std::time::Duration;

#[derive(Default)]
pub struct SpammerWindow {
    text: String,
    selected: Option<usize>,
    /// Load or Save was clicked; the dialog opens in [`Self::files`].
    want_load: bool,
    want_save: bool,
    load: Pending,
    save: Pending,
    /// The last Load or Save failure.
    note: Option<String>,
}

impl SpammerWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Spammer", [620.0, 460.0], 6)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        if crate::turu::bot().spammer.is_running() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// The Load and Save dialogs (they need the file picker).
    pub fn files(&mut self, picker: &FilePicker, ctx: &Context) {
        let spammer = crate::turu::bot().spammer.clone();
        if std::mem::take(&mut self.want_load) {
            self.load.pick_file(picker, ctx, text_dialog());
        }
        if std::mem::take(&mut self.want_save) {
            self.save
                .save_file(picker, ctx, text_dialog().set_file_name("packets.txt"));
        }
        if let Some(path) = self.load.poll() {
            self.selected = None;
            self.note = spammer
                .load(&path)
                .err()
                .map(|e| format!("Couldn't load {}: {e}", path.display()));
        }
        if let Some(path) = self.save.poll() {
            self.note = spammer
                .save(&path)
                .err()
                .map(|e| format!("Couldn't save {}: {e}", path.display()));
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let spammer = bot.spammer.clone();
        let running = spammer.is_running();
        let progress = spammer.progress().filter(|_| running);

        ui.horizontal(|ui| {
            let (label, kind) = if running {
                ("■  Stop", Kind::Danger)
            } else {
                ("▶  Start", Kind::Primary)
            };
            let can_start = running || !spammer.packets.lock().is_empty();
            if widgets::button(
                ui,
                label,
                kind,
                can_start,
                vec2(140.0, 28.0),
                "Add packets to the queue first",
            )
            .clicked()
            {
                if running {
                    spammer.stop();
                } else {
                    spammer.start(&bot);
                }
            }
            ui.label("Every");
            let mut delay = spammer.delay_ms();
            if ui
                .add(DragValue::new(&mut delay).range(0..=600_000).speed(10).suffix(" ms"))
                .on_hover_text("Delay between packets")
                .changed()
            {
                spammer.set_delay_ms(delay);
            }
            let mut to_client = spammer.to_client();
            if widgets::switch(ui, &mut to_client)
                .on_hover_text("Send the packets to the game client instead of the server")
                .changed()
            {
                spammer.set_to_client(to_client);
            }
            ui.label("Send to client");
            if let Some((_, status)) = &progress {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::pill(ui, |ui| {
                        ui.horizontal(|ui| {
                            widgets::dot(ui, SUCCESS, 3.0);
                            ui.label(RichText::new(status).small().color(SUCCESS));
                        });
                    });
                });
            }
        });

        let count = spammer.packets.lock().len();
        ui.horizontal(|ui| {
            widgets::section_label(ui, "QUEUE");
            widgets::count_badge(ui, count);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let size = vec2(84.0, 24.0);
                if widgets::button(ui, "Clear", Kind::Normal, count > 0, size, "").clicked() {
                    spammer.packets.lock().clear();
                    self.selected = None;
                }
                if widgets::button(ui, "Save", Kind::Normal, count > 0 && !self.save.is_open(), size, "").clicked() {
                    self.want_save = true;
                }
                if widgets::button(ui, "Load", Kind::Normal, !self.load.is_open(), size, "")
                    .on_hover_text("Replace the queue with a saved list")
                    .clicked()
                {
                    self.want_load = true;
                }
            });
        });

        let footer = 44.0 + if self.note.is_some() { 20.0 } else { 0.0 };
        let height = (ui.available_height() - footer).max(80.0);
        let sending = progress.as_ref().map(|(i, _)| *i);
        self.queue(ui, &spammer, height, sending);

        if let Some(note) = &self.note {
            ui.label(RichText::new(note).small().color(WARNING));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let buttons = 84.0 + 112.0 + 2.0 * ui.spacing().item_spacing.x;
            ui.add(
                egui::TextEdit::singleline(&mut self.text)
                    .hint_text("%xt%zm%...  packet to add or send")
                    .font(egui::TextStyle::Monospace)
                    .desired_width((ui.available_width() - buttons).max(120.0))
                    .margin(egui::Margin::symmetric(8, 6))
                    .background_color(super::theme::INSET),
            );
            let filled = !self.text.trim().is_empty();
            if widgets::button(ui, "+  Add", Kind::Normal, filled, vec2(84.0, 28.0), "")
                .on_hover_text("Add to the end of the queue")
                .clicked()
            {
                spammer.add(&self.text);
            }
            if widgets::button(ui, "Send now", Kind::Primary, filled, vec2(112.0, 28.0), "")
                .on_hover_text("Send once now")
                .clicked()
            {
                let packet = self.text.clone();
                crate::turu::spawn_action("Spammer", move |bot| bot.spammer.send(bot, &packet));
            }
        });
    }

    /// The numbered queue; the row being sent is selected while it runs.
    fn queue(&mut self, ui: &mut Ui, spammer: &turu_bot::spammer::Spammer, height: f32, sending: Option<usize>) {
        let packets = spammer.packets.lock().clone();
        if self.selected.is_some_and(|i| i >= packets.len()) {
            self.selected = None;
        }
        let mut remove = None;
        widgets::list_well(ui, height, |ui| {
            if packets.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("The queue is empty: type a packet below and Add it").color(MUTED));
                });
                return;
            }
            ScrollArea::vertical()
                .id_salt("spammer_queue")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, packet) in packets.iter().enumerate() {
                        let selected = sending.map_or(self.selected == Some(i), |s| s == i);
                        let row = widgets::list_row(ui, selected, 26.0, |ui| {
                            ui.add_sized(
                                [28.0, 18.0],
                                Label::new(RichText::new((i + 1).to_string()).monospace().size(11.0).color(MUTED)),
                            );
                            ui.add_space(4.0);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let hovered = ui.rect_contains_pointer(ui.max_rect());
                                if (hovered || selected)
                                    && ui
                                        .add(egui::Button::new(RichText::new("✕").color(DANGER)).frame(false))
                                        .on_hover_text("Remove from the queue")
                                        .clicked()
                                {
                                    remove = Some(i);
                                }
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.add(
                                        Label::new(RichText::new(packet).monospace().size(11.0).color(TEXT)).truncate(),
                                    )
                                    .on_hover_text(packet);
                                });
                            });
                        });
                        if row.clicked() {
                            self.selected = Some(i);
                        }
                    }
                });
        });
        if remove.is_none()
            && let Some(i) = self.selected
            && ui.input(|input| input.key_pressed(Key::Delete))
            && !ui.ctx().egui_wants_keyboard_input()
        {
            remove = Some(i);
        }
        // By the row's index, not its text: the queue can hold the same packet twice.
        if let Some(i) = remove {
            let mut queue = spammer.packets.lock();
            if i < queue.len() {
                queue.remove(i);
            }
            self.selected = None;
        }
    }
}

fn text_dialog() -> AsyncFileDialog {
    AsyncFileDialog::new()
        .add_filter("Text", &["txt"])
        .add_filter("All files", &["*"])
}
