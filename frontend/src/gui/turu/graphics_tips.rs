//! Mugetsu 2.0.0's one-time tips for built-in graphics (`crate::graphics_tips`), in a tab: a box
//! over the game saying to set AQW's Quality to Low, or offering Performance mode with a button
//! that turns it on (Options > Application has it either way).

use super::widgets::{self, Kind};
use crate::graphics_tips::{self, Tip};
use egui::{Align, Context, Layout, Modal, vec2};

/// Shows the oldest tip waiting, if any, and passes on the answer.
pub fn show(ctx: &Context) {
    if crate::models::prompts::native_shown() {
        return;
    }
    let Some(tip) = graphics_tips::pending() else {
        return;
    };
    let (title, body, hint) = crate::models::prompts::tip_texts(&tip);
    let mut answered = false;
    let mut turn_on = false;
    let response = Modal::new(egui::Id::new("turu_graphics_tip")).show(ctx, |ui| {
        ui.set_min_width(300.0);
        ui.set_max_width(420.0);
        ui.heading(title);
        ui.label(body);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(hint).small().weak());
        ui.add_space(4.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| match &tip {
            Tip::QualityLow { .. } => {
                answered |= widgets::button(ui, "OK", Kind::Primary, true, vec2(72.0, 28.0), "").clicked();
            }
            Tip::PerformanceMode { .. } => {
                if widgets::button(ui, "Turn on", Kind::Primary, true, vec2(72.0, 28.0), "").clicked() {
                    answered = true;
                    turn_on = true;
                }
                answered |= widgets::button(ui, "Not now", Kind::Normal, true, vec2(72.0, 28.0), "").clicked();
            }
        });
    });
    if answered || response.should_close() {
        crate::models::prompts::answer_tip(&tip, turn_on);
    }
}
