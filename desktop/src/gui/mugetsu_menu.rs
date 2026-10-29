//! AQW build (Mugetsu): the right-click menu's settings as a "Mugetsu" menu in Ruffle's menu
//! bar (shown with --gui). Changes go through the same events as the right-click menu, so
//! they're applied the same way.

use crate::custom_event::RuffleEvent;
use crate::gui::context_menu::*;
use crate::mugetsu::{MenuSettings, Setting, VERSION};
use egui::*;
use winit::event_loop::EventLoopProxy;

pub fn contents(ui: &mut Ui, event_loop: &EventLoopProxy<RuffleEvent>) {
    let send = |setting| {
        let _ = event_loop.send_event(RuffleEvent::MugetsuSetting(setting));
    };
    let settings = MenuSettings::now();
    panel_style(ui.style_mut());
    let z = 1.0;
    ui.set_width(WIDTH * z);
    ui.spacing_mut().item_spacing = vec2(0.0, 2.0 * z);

    // Header.
    let (rect, _) = ui.allocate_exact_size(vec2(WIDTH * z, 24.0 * z), Sense::hover());
    ui.painter().text(
        pos2(rect.left() + 4.0 * z, rect.center().y),
        Align2::LEFT_CENTER,
        "Mugetsu",
        FontId::proportional(16.0 * z),
        TEXT,
    );
    ui.painter().text(
        pos2(rect.right() - 4.0 * z, rect.center().y),
        Align2::RIGHT_CENTER,
        format!("v{VERSION}"),
        FontId::proportional(12.0 * z),
        MUTED,
    );
    ui.add_space(4.0 * z);

    section(ui, z, "GRAPHICS");
    row_title(ui, z, "Edge smoothing", "Flash-style is lighter on laptops");
    if let Some(choice) = segmented(
        ui,
        z,
        "edges_gui",
        &["Flash-style", "MSAA"],
        if settings.flash_edges { 0 } else { 1 },
    ) {
        send(Setting::FlashEdges(choice == 0));
    }
    ui.add_space(6.0 * z);
    if toggle_row(
        ui,
        z,
        "Frame interpolation",
        "Smoother motion, best on strong PCs",
        settings.interpolation,
    ) {
        send(Setting::Interpolation(!settings.interpolation));
    }
    if settings.graphics_choice {
        ui.add_space(4.0 * z);
        row_title(ui, z, "Graphics API", "Changing it restarts Mugetsu");
        let (options, values) = graphics_options();
        let current = values.iter().position(|&g| g == settings.graphics).unwrap_or(0);
        if let Some(choice) = segmented(ui, z, "graphics_gui", options, current) {
            send(Setting::Graphics(values[choice]));
        }
    }

    section(ui, z, "DISPLAY");
    if toggle_row(ui, z, "Show FPS", "A small counter in the corner", settings.show_fps) {
        send(Setting::ShowFps(!settings.show_fps));
    }

    section(ui, z, "HELP");
    if action_row(ui, z, "Open log folder", Some("↗")).clicked() {
        send(Setting::OpenLogFolder);
        ui.close();
    }
}

pub fn menu_button(ui: &mut Ui, event_loop: &EventLoopProxy<RuffleEvent>) {
    ui.menu_button("Mugetsu", |ui| contents(ui, event_loop));
}
