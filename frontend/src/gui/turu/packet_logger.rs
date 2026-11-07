//! Diagnostics > Logger: outgoing packets (`%xt%zm%...%`), newest at the bottom, capped at
//! 1000 lines. The engine logs them only while logging is on, and only those the "Show n / m"
//! filters keep.

use super::files::SaveText;
use super::theme::MUTED;
use super::widgets::{self, Kind, LogView};
use egui::{Context, RichText, Ui, vec2};
use std::sync::atomic::Ordering;
use turu_bot::packet_filters::LOGGER_FILTERS;

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

    fn set_logging(&self, on: bool) {
        crate::turu::bot().logs.packet_logging.store(on, Ordering::Relaxed);
    }

    /// VibeSkua's "Show n / m" filters: a packet an unticked filter takes isn't logged from then
    /// on (`turu_bot::packet_filters::logger_keeps`); what's logged stays.
    fn filters_button(&self, ui: &mut egui::Ui) {
        let logs = &crate::turu::bot().logs;
        let hidden = logs.packet_hidden.load(Ordering::Relaxed);
        let names = LOGGER_FILTERS;
        let mut checked: Vec<bool> = (0..names.len()).map(|i| hidden & (1 << i) == 0).collect();
        if super::packet_filters::filters_button(ui, "packet_logger_filters", &names, &mut checked) {
            let hidden = checked
                .iter()
                .enumerate()
                .filter(|(_, on)| !**on)
                .fold(0u32, |mask, (i, _)| mask | 1 << i);
            logs.packet_hidden.store(hidden, Ordering::Relaxed);
        }
    }

    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        self.save.poll();
        if !*open {
            return;
        }
        super::window(ctx, "Logger", [560.0, 380.0], 5)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        if self.logging() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
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
                super::logs::log_buttons(ui, buffer, &mut self.save, "packets.txt");
            });
        });
        ui.horizontal(|ui| {
            let filters = 140.0 + ui.spacing().item_spacing.x;
            widgets::text_box(
                ui,
                &mut self.search,
                "Search packets",
                (ui.available_width() - 90.0 - filters).max(80.0),
            );
            self.filters_button(ui);
            widgets::pill(ui, |ui| {
                ui.label(
                    RichText::new(self.view.count().to_string())
                        .size(widgets::log_count_size())
                        .color(MUTED),
                );
            });
        });
        let height = ui.available_height();
        self.view.show(ui, "packets_view", height);
    }
}
