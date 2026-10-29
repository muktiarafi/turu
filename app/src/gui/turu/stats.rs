//! Combat > Stats: the script clock and the counters.

use super::theme::{CARD, EDGE, MUTED, TEXT};
use super::widgets::{self, Kind};
use egui::{Context, CornerRadius, RichText, Stroke, Ui, vec2};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct StatsWindow {
    /// The clock keeps its last value after the script ends.
    last_elapsed: Duration,
}

impl StatsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Stats", [360.0, 250.0], 3)
            .open(open)
            .resizable(false)
            .show(ctx, |ui| self.ui(ui));
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let active = bot.scripts.status().is_active();
        let stats = bot.state.lock().stats.clone();
        if active && let Some(started) = stats.started {
            self.last_elapsed = started.elapsed();
        }
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    widgets::section_label(ui, "SCRIPT TIME");
                    let secs = self.last_elapsed.as_secs();
                    let clock = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
                    ui.label(RichText::new(clock).monospace().size(22.0).strong().color(TEXT));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, "Reset", Kind::Normal, true, vec2(70.0, 26.0), "").clicked() {
                        let mut state = bot.state.lock();
                        state.stats = Default::default();
                        state.stats.started = active.then(Instant::now);
                        self.last_elapsed = Duration::ZERO;
                    }
                });
            });
        });
        ui.add_space(2.0);
        let counters = [
            ("KILLS", stats.kills),
            ("DEATHS", stats.deaths),
            ("PICKUPS", stats.drops),
            ("ACCEPTED", stats.quests_accepted),
            ("COMPLETED", stats.quests_completed),
            ("RE-LOGINS", stats.relogins),
        ];
        let width = (ui.available_width() - 2.0 * ui.spacing().item_spacing.x) / 3.0;
        for row in counters.chunks(3) {
            ui.horizontal(|ui| {
                for (label, value) in row {
                    egui::Frame::new()
                        .fill(CARD)
                        .stroke(Stroke::new(1.0, EDGE))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(8))
                        .show(ui, |ui| {
                            ui.set_width(width - 18.0);
                            widgets::section_label(ui, label);
                            ui.label(RichText::new(value.to_string()).size(18.0).strong().color(TEXT));
                        });
                }
            });
        }
        ui.label(
            RichText::new("Counters follow the running script and Auto Attack.")
                .small()
                .color(MUTED),
        );
    }
}
