//! The message boxes scripts show (`bot.showMessageBox`, VibeSkua `ShowMessageBox`): a modal
//! with the script's caption, message and buttons. The script waits until one is clicked;
//! closing the box (Escape, a click outside) answers "closed".

use super::widgets::{self, Kind};
use egui::{Align, Context, Layout, Modal, ScrollArea, vec2};

/// Shows the message box the script waits on, if any, and passes on the answer.
pub fn show(ctx: &Context) {
    let scripts = &crate::turu::bot().scripts;
    let Some(message_box) = scripts.message_box() else {
        return;
    };
    let mut clicked = None;
    let response = Modal::new(egui::Id::new(("turu_message_box", message_box.id))).show(ctx, |ui| {
        ui.set_min_width(300.0);
        ui.set_max_width(420.0);
        ui.heading(&message_box.caption);
        ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            ui.label(&message_box.message);
        });
        ui.add_space(4.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // VibeSkua lists them left to right; the last one ("Yes", "OK") is the main one.
            let last = message_box.buttons.len() - 1;
            for (i, label) in message_box.buttons.iter().enumerate().rev() {
                let kind = if i == last { Kind::Primary } else { Kind::Normal };
                if widgets::button(ui, label, kind, true, vec2(72.0, 28.0), "").clicked() {
                    clicked = Some(i);
                }
            }
        });
    });
    if clicked.is_some() || response.should_close() {
        scripts.answer_message_box(message_box.id, clicked);
    }
}
