//! Diagnostics > Logger: outgoing packets (`%xt%zm%...%`), newest at the bottom, capped at
//! 1000 lines. The engine logs them only while logging is on.

use super::files::SaveText;
use super::theme::MUTED;
use super::widgets::{self, Kind, LogView};
use crate::gui::FilePicker;
use egui::{Context, RichText, Ui, vec2};
use std::sync::atomic::Ordering;

#[derive(Default)]
pub struct PacketLogger {
    search: String,
    view: LogView,
    /// What `view` was built from: (buffer version, search).
    built_from: Option<(u64, String)>,
    save: SaveText,
}

impl PacketLogger {
    pub fn logging(&self) -> bool {
        crate::turu::bot().logs.packet_logging.load(Ordering::Relaxed)
    }

    fn set_logging(&mut self, on: bool) {
        crate::turu::bot().logs.packet_logging.store(on, Ordering::Relaxed);
    }

    pub fn show(&mut self, ctx: &Context, picker: &FilePicker, open: &mut bool) {
        self.save.poll();
        if !*open {
            return;
        }
        super::window(ctx, "Logger", [560.0, 380.0], 5)
            .open(open)
            .show(ctx, |ui| self.ui(ui, picker));
        if self.logging() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut Ui, picker: &FilePicker) {
        let buffer = &crate::turu::bot().logs.packets;
        let key = (buffer.version(), self.search.clone());
        if self.built_from.as_ref() != Some(&key) {
            let needle = self.search.to_lowercase();
            let lines = buffer.snapshot();
            let rows: Vec<String> = lines
                .iter()
                .map(|line| line.formatted())
                .filter(|line| needle.is_empty() || line.to_lowercase().contains(&needle))
                .collect();
            self.view.set_rows(rows, lines.len());
            self.built_from = Some(key);
        }

        ui.horizontal(|ui| {
            let logging = self.logging();
            let (label, kind) = if logging {
                ("Stop Logging", Kind::Danger)
            } else {
                ("Start Logging", Kind::Primary)
            };
            if widgets::button(ui, label, kind, true, vec2(110.0, 26.0), "").clicked() {
                self.set_logging(!logging);
            }
            ui.label(
                RichText::new(if logging { "Logging" } else { "Stopped" })
                    .small()
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                super::logs::log_buttons(ui, picker, buffer, &mut self.save, "packets.txt");
            });
        });
        ui.horizontal(|ui| {
            widgets::text_box(ui, &mut self.search, "Search packets", ui.available_width() - 90.0);
            widgets::count_badge(ui, self.view.count());
        });
        let height = ui.available_height();
        self.view.show(ui, "packets_view", height);
    }
}
