//! Diagnostics > Interceptor (VibeSkua `PacketInterceptorView`): the server box with refresh,
//! Connect/Disconnect, Log packets and the Connected chip; search and the "Show n / m" category
//! filters; the captured packets (OUT, IN or BLOCK, multi-select, Ctrl/Cmd+C copies, scrolling
//! sideways for long ones); the count, Copy, Export and Clear.
//!
//! The capture is the tab's `bot.capture`, fed by the app's socket wrapper (`app/src/capture.rs`):
//! Connect starts it at once, without the relog VibeSkua's proxy needs. Picking a server makes
//! Connect also relog there, as VibeSkua's does.

use super::files::SaveText;
use super::theme::{ACCENT, DANGER, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use egui::{Align, Context, CornerRadius, Key, Label, Layout, Modifiers, RichText, ScrollArea, Ui, vec2};
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;
use turu_bot::capture::{self, InterceptedPacket};
use turu_bot::packet_filters;

/// The server box's first entry: connect on the server you're on.
const CURRENT: &str = "Current server";

#[derive(Default)]
pub struct InterceptorWindow {
    /// The server Connect relogs to; `None` stays on the current one.
    server: Option<String>,
    search: String,
    /// Each category filter's tick (`packet_filters::interceptor_filters`); all ticked at first.
    filters: Vec<bool>,
    /// Each logged packet's commands, by number, parsed once (VibeSkua's lazy `Commands`).
    commands: HashMap<u64, Vec<String>>,
    /// Shown packets (number, packet), built from the capture's version, the search and the
    /// filters.
    rows: Vec<(u64, InterceptedPacket)>,
    /// The longest shown packet, in characters: how far the list scrolls sideways.
    widest: usize,
    built_from: Option<(u64, String, Vec<bool>)>,
    /// Selected packets, by number.
    selected: BTreeSet<u64>,
    anchor: Option<u64>,
    /// Export was clicked; the dialog opens in [`Self::files`].
    export: Option<String>,
    save: SaveText,
}

impl InterceptorWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Interceptor", [760.0, 540.0], 7)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        if crate::turu::bot().capture.is_running() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// The Export dialog (not drawn in the window).
    pub fn files(&mut self, ctx: &Context) {
        self.save.poll();
        if let Some(text) = self.export.take() {
            self.save.ask(ctx, "packets.txt", text);
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let capture = bot.capture.clone();
        let running = capture.is_running();

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("interceptor_server")
                .selected_text(self.server.as_deref().unwrap_or(CURRENT))
                .width(200.0)
                .show_ui(ui, |ui| {
                    turu_bot::api::servers::refresh_in_background(&bot);
                    if ui.selectable_label(self.server.is_none(), CURRENT).clicked() {
                        self.server = None;
                    }
                    for name in widgets::server_names(&bot) {
                        if ui
                            .selectable_label(self.server.as_ref() == Some(&name), &name)
                            .clicked()
                        {
                            self.server = Some(name);
                        }
                    }
                });
            let refreshing = bot.servers.is_refreshing();
            if widgets::button(ui, "⟳", Kind::Normal, !refreshing, vec2(32.0, 28.0), "")
                .on_hover_text("Refresh server list")
                .clicked()
            {
                crate::turu::spawn_action("Servers", |bot| {
                    turu_bot::api::servers::get_servers(bot, true);
                    Ok(())
                });
            }
            let (label, kind) = if running {
                ("Disconnect", Kind::Danger)
            } else {
                ("Connect", Kind::Primary)
            };
            let tooltip = if running {
                "Stop capturing: packets pass untouched (the game stays connected)"
            } else if self.server.is_some() {
                "Capture the game's packets, and relog on the chosen server"
            } else {
                "Capture the game's packets from now on"
            };
            if widgets::button(ui, label, kind, true, vec2(140.0, 28.0), "")
                .on_hover_text(tooltip)
                .clicked()
            {
                if running {
                    capture.stop();
                } else {
                    capture.start();
                    if let Some(server) = self.server.clone() {
                        crate::turu::spawn_action("Interceptor", move |bot| {
                            let ctx = turu_bot::api::servers::Ctx::new(bot);
                            turu_bot::api::servers::relogin_named(&ctx, &server).map(|_| ())
                        });
                    }
                }
            }
            let mut logging = capture.is_logging();
            if widgets::switch(ui, &mut logging)
                .on_hover_text("Add intercepted packets to the list")
                .changed()
            {
                capture.set_logging(logging);
            }
            ui.label("Log packets");
            if running {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::pill(ui, |ui| {
                        ui.horizontal(|ui| {
                            widgets::dot(ui, SUCCESS, 3.0);
                            ui.label(RichText::new("Connected").small().color(SUCCESS));
                        });
                    });
                });
            }
        });

        let names = packet_filters::interceptor_filters();
        if self.filters.len() != names.len() {
            self.filters = vec![true; names.len()];
        }
        ui.horizontal(|ui| {
            let width = ui.available_width() - 140.0 - ui.spacing().item_spacing.x;
            widgets::text_box(ui, &mut self.search, "Search packets", width.max(80.0));
            super::packet_filters::filters_button(ui, "interceptor_filters", &names, &mut self.filters);
        });
        self.rebuild(&capture);

        let height = (ui.available_height() - 40.0).max(80.0);
        self.list(ui, height);

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let shown = self.rows.len();
            let picked = self.rows.iter().filter(|(n, _)| self.selected.contains(n)).count();
            let mut text = if shown == 1 {
                "1 packet".to_owned()
            } else {
                format!("{shown} packets")
            };
            if picked > 0 {
                text.push_str(&format!(" · {picked} selected"));
            }
            ui.label(RichText::new(text).size(widgets::log_count_size()).color(MUTED));
            if picked > 0
                && ui
                    .add(egui::Button::new(RichText::new("✕").small().color(MUTED)).frame(false))
                    .on_hover_text("Unselect all")
                    .clicked()
            {
                self.selected.clear();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let size = vec2(84.0, 24.0);
                if widgets::button(ui, "Clear", Kind::Normal, true, size, "").clicked() {
                    capture.clear();
                    self.selected.clear();
                }
                if widgets::button(ui, "Export", Kind::Normal, true, size, "")
                    .on_hover_text("Save every captured packet to a text file, each marked OUT, IN or BLOCK")
                    .clicked()
                {
                    self.export = Some(capture::export_text(&capture.packets()));
                }
                if widgets::button(ui, "Copy", Kind::Normal, shown > 0, size, "")
                    .on_hover_text("Copy the selected packets, or all of them")
                    .clicked()
                {
                    self.copy(ui.ctx());
                }
            });
        });
    }

    /// The shown packets: those the search finds (VibeSkua: a search of spaces finds all) in a
    /// ticked category (VibeSkua `MatchesPacket`).
    fn rebuild(&mut self, capture: &turu_bot::capture::CaptureProxy) {
        let key = (capture.version(), self.search.clone(), self.filters.clone());
        if self.built_from.as_ref() == Some(&key) {
            return;
        }
        let needle = (!self.search.trim().is_empty()).then(|| self.search.to_lowercase());
        let (first, packets) = capture.snapshot();
        self.commands.retain(|n, _| *n >= first);
        let mut rows = Vec::new();
        for (n, packet) in (first..).zip(packets) {
            if needle
                .as_ref()
                .is_some_and(|needle| !packet.packet.to_lowercase().contains(needle))
            {
                continue;
            }
            let commands = self
                .commands
                .entry(n)
                .or_insert_with(|| packet_filters::commands(&packet.packet));
            if packet_filters::interceptor_shows(commands, &self.filters) {
                rows.push((n, packet));
            }
        }
        self.rows = rows;
        self.widest = self
            .rows
            .iter()
            .map(|(_, p)| p.packet.chars().count())
            .max()
            .unwrap_or(0);
        self.selected.retain(|n| *n >= first);
        self.built_from = Some(key);
    }

    /// The selected packets, or every shown one when none is selected.
    fn copy(&self, ctx: &Context) {
        let picked: Vec<&str> = self
            .rows
            .iter()
            .filter(|(n, _)| self.selected.contains(n))
            .map(|(_, p)| p.packet.as_str())
            .collect();
        let text = if picked.is_empty() {
            self.rows
                .iter()
                .map(|(_, p)| p.packet.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            picked.join("\n")
        };
        if !text.is_empty() {
            ctx.copy_text(text);
        }
    }

    fn list(&mut self, ui: &mut Ui, height: f32) {
        let mut clicked = None;
        widgets::list_well(ui, height, |ui| {
            if self.rows.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("No packets yet: Connect to capture the game's packets").color(MUTED));
                });
                return;
            }
            let size = widgets::log_text_size(11.0);
            let row_height = if widgets::grandpa_mode() { 28.0 } else { 24.0 };
            // Long packets aren't cut: the list scrolls sideways (VibeSkua's horizontal scroll bar).
            let char_width = ui
                .ctx()
                .fonts_mut(|f| f.glyph_width(&egui::FontId::monospace(size), 'M'));
            let content_width = 52.0 + 6.0 + 24.0 + self.widest as f32 * char_width;
            ScrollArea::both()
                .id_salt("interceptor_packets")
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show_rows(ui, row_height, self.rows.len(), |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let row_width = content_width.max(ui.available_width());
                    for (n, packet) in &self.rows[range] {
                        let selected = self.selected.contains(n);
                        let row = ui
                            .allocate_ui(vec2(row_width, row_height), |ui| {
                                ui.set_width(row_width);
                                widgets::list_row(ui, selected, row_height, |ui| {
                                    direction_chip(ui, packet);
                                    ui.add_space(6.0);
                                    let mut text = RichText::new(&packet.packet).monospace().size(size);
                                    text = if packet.is_blocked() {
                                        text.color(MUTED).strikethrough()
                                    } else {
                                        text.color(TEXT)
                                    };
                                    ui.add(Label::new(text).extend()).on_hover_text(&packet.packet);
                                })
                            })
                            .inner;
                        if row.clicked() {
                            clicked = Some(*n);
                        }
                    }
                });
        });
        if let Some(n) = clicked {
            let modifiers = ui.input(|i| i.modifiers);
            self.click(n, modifiers);
        }
        if !self.selected.is_empty()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::C))
        {
            self.copy(ui.ctx());
        }
    }

    /// VibeSkua's multiple selection: a click toggles a packet, Shift+click adds the shown
    /// packets from the last one clicked.
    fn click(&mut self, n: u64, modifiers: Modifiers) {
        if modifiers.shift
            && let Some(anchor) = self.anchor
        {
            let (from, to) = (anchor.min(n), anchor.max(n));
            let range: Vec<u64> = self
                .rows
                .iter()
                .map(|(m, _)| *m)
                .filter(|m| (from..=to).contains(m))
                .collect();
            self.selected.extend(range);
            return;
        }
        if !self.selected.remove(&n) {
            self.selected.insert(n);
        }
        self.anchor = Some(n);
    }
}

/// The OUT / IN / BLOCK tag before each packet.
fn direction_chip(ui: &mut Ui, packet: &InterceptedPacket) {
    let (fill, color) = match packet.outbound {
        Some(true) => (ACCENT.gamma_multiply(0.25), ACCENT),
        Some(false) => (SUCCESS.gamma_multiply(0.2), SUCCESS),
        None => (DANGER.gamma_multiply(0.2), DANGER),
    };
    let (rect, _) = ui.allocate_exact_size(vec2(52.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        packet.direction(),
        egui::FontId::proportional(10.0),
        color,
    );
}
