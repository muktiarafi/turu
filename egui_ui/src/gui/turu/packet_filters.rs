//! The packet tools' "Show n / m" button (VibeSkua `PacketFiltersButton`): its flyout lists the
//! filters as check boxes in three columns, under "SHOW PACKETS" with All, None and Invert.

use super::widgets::{self, Kind};
use egui::{Popup, PopupCloseBehavior, RectAlign, RichText, Ui, vec2};

/// The flyout's width (VibeSkua's 356), less on a narrow screen.
const FLYOUT_WIDTH: f32 = 356.0;

/// The button and its flyout; `checked` holds each of `names`' ticks. Returns whether a tick
/// changed.
pub fn filters_button(ui: &mut Ui, id: &str, names: &[&str], checked: &mut [bool]) -> bool {
    let shown = checked.iter().filter(|&&on| on).count();
    let height = widgets::target(28.0, 40.0);
    let button = widgets::button(
        ui,
        &format!("Show  {shown} / {}  ▾", names.len()),
        Kind::Normal,
        true,
        vec2(140.0, height),
        "",
    )
    .on_hover_text("Which kinds of packets to show");
    let screen = ui.ctx().content_rect().width();
    let width = FLYOUT_WIDTH.min(screen - 32.0).max(200.0);
    let mut changed = false;
    Popup::from_toggle_button_response(&button)
        .id(egui::Id::new(id))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .align(RectAlign::BOTTOM_END)
        .width(width)
        .show(|ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                widgets::section_label(ui, "SHOW PACKETS");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let size = vec2(64.0, widgets::target(22.0, 40.0));
                    // Right to left: Invert, None, All read left to right.
                    if widgets::button(ui, "Invert", Kind::Normal, true, size, "").clicked() {
                        checked.iter_mut().for_each(|on| *on = !*on);
                        changed = true;
                    }
                    if widgets::button(ui, "None", Kind::Normal, true, size, "").clicked() {
                        checked.iter_mut().for_each(|on| *on = false);
                        changed = true;
                    }
                    if widgets::button(ui, "All", Kind::Normal, true, size, "").clicked() {
                        checked.iter_mut().for_each(|on| *on = true);
                        changed = true;
                    }
                });
            });
            ui.separator();
            // Three columns (VibeSkua's UniformGrid), two when the flyout is narrow.
            let columns = if width >= 300.0 { 3 } else { 2 };
            ui.spacing_mut().interact_size.y = widgets::target(22.0, 40.0);
            egui::Grid::new((id, "grid"))
                .num_columns(columns)
                .min_col_width((width - 16.0) / columns as f32)
                .spacing(vec2(4.0, 2.0))
                .show(ui, |ui| {
                    for (i, name) in names.iter().enumerate() {
                        changed |= ui.checkbox(&mut checked[i], RichText::new(*name)).changed();
                        if i % columns == columns - 1 {
                            ui.end_row();
                        }
                    }
                });
        });
    changed
}
