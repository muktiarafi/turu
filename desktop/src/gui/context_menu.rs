use crate::custom_event::RuffleEvent;
use crate::mugetsu::{Graphics, MenuSettings, Setting};
use egui::{
    Align, Align2, Area, Button, Checkbox, Color32, CornerRadius, FontId, Frame, Id, Key,
    KeyboardShortcut, Layout, Margin, Modifiers, Order, Pos2, Rect, Response, Sense, Shadow,
    Stroke, Style, Ui, Widget, pos2, vec2,
};
use ruffle_core::{ContextMenuItem, PlayerEvent};
use unic_langid::LanguageIdentifier;
use winit::event_loop::EventLoopProxy;

use super::text;

pub struct ContextMenu {
    items: Vec<ContextMenuItem>,
    /// AQW build: Mugetsu's settings, below the movie's items (AQW has none of its own).
    settings: Option<MenuSettings>,
    position: Option<Pos2>,
    close_event: PlayerEvent,
}

impl ContextMenu {
    pub fn new(
        items: Vec<ContextMenuItem>,
        settings: Option<MenuSettings>,
        close_event: PlayerEvent,
    ) -> Self {
        Self {
            items,
            settings,
            position: None,
            close_event,
        }
    }

    pub fn close_event(self) -> PlayerEvent {
        self.close_event
    }

    pub fn show(
        &mut self,
        locale: &LanguageIdentifier,
        egui_ctx: &egui::Context,
        event_loop: &EventLoopProxy<RuffleEvent>,
        fullscreen: bool,
    ) -> bool {
        if let Some(settings) = self.settings {
            return self.show_mugetsu(egui_ctx, event_loop, settings, fullscreen);
        }
        let mut item_clicked = false;
        self.position = self.position.or(egui_ctx.pointer_latest_pos());

        let area = Area::new(Id::new("context_menu"))
            .order(Order::Foreground)
            .fixed_pos(self.position.unwrap_or_default())
            .constrain_to(egui_ctx.content_rect())
            .interactable(true)
            .show(egui_ctx, |ui| {
                set_menu_style(ui.style_mut());
                Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_max_width(150.0);
                    ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                        for (i, item) in self.items.iter().enumerate() {
                            if i != 0 && item.separator_before {
                                ui.separator();
                            }
                            let clicked = if item.checked {
                                Checkbox::new(&mut true, &item.caption).ui(ui).clicked()
                            } else {
                                let button = Button::new(&item.caption)
                                    .wrap_mode(egui::TextWrapMode::Extend);

                                ui.add_enabled(item.enabled, button).clicked()
                            };
                            if clicked {
                                let _ =
                                    event_loop.send_event(RuffleEvent::ContextMenuItemClicked(i));
                                item_clicked = true;
                            }
                        }

                        if fullscreen {
                            ui.separator();
                            if Button::new(text(locale, "context-menu-exit-fullscreen"))
                                .shortcut_text(ui.ctx().format_shortcut(&KeyboardShortcut::new(
                                    Modifiers::NONE,
                                    Key::Escape,
                                )))
                                .wrap_mode(egui::TextWrapMode::Extend)
                                .ui(ui)
                                .clicked()
                            {
                                let _ = event_loop.send_event(RuffleEvent::ExitFullScreen);
                                item_clicked = true;
                            }
                        }
                    })
                })
            });

        let should_close = item_clicked
            || area.response.clicked_elsewhere()
            || egui_ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));

        !should_close
    }

    /// AQW build: Mugetsu's settings panel (the movie's own items, if any, on top).
    fn show_mugetsu(
        &mut self,
        egui_ctx: &egui::Context,
        event_loop: &EventLoopProxy<RuffleEvent>,
        settings: MenuSettings,
        fullscreen: bool,
    ) -> bool {
        let mut item_clicked = false;
        self.position = self.position.or(egui_ctx.pointer_latest_pos());
        // Sized with the window: as designed at about 1000 points tall (a maximized window on
        // a 1080p screen), smaller in a smaller window (down to 60%), so it never covers half of
        // a small game window. Everything is laid out at that size (text rendered sharp at it).
        let z = (egui_ctx.content_rect().height() / 1000.0).clamp(0.6, 1.0);
        let send = |event| {
            let _ = event_loop.send_event(event);
        };

        let area = Area::new(Id::new("context_menu"))
            .order(Order::Foreground)
            .fixed_pos(self.position.unwrap_or_default())
            .constrain_to(egui_ctx.content_rect())
            .interactable(true)
            .show(egui_ctx, |ui| {
                panel_style(ui.style_mut());
                Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, EDGE))
                    .corner_radius(CornerRadius::same((12.0 * z) as u8))
                    .inner_margin(Margin::same((10.0 * z) as i8))
                    .shadow(Shadow {
                        offset: [0, (6.0 * z) as i8],
                        blur: (24.0 * z) as u8,
                        spread: 0,
                        color: Color32::from_black_alpha(140),
                    })
                    .show(ui, |ui| {
                        ui.set_width(WIDTH * z);
                        ui.spacing_mut().item_spacing = vec2(0.0, 2.0 * z);

                        // Header.
                        let (rect, _) =
                            ui.allocate_exact_size(vec2(WIDTH * z, 24.0 * z), Sense::hover());
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
                            format!("v{}", crate::mugetsu::VERSION),
                            FontId::proportional(12.0 * z),
                            MUTED,
                        );
                        ui.add_space(4.0 * z);

                        // The movie's own items (AQW has none).
                        if !self.items.is_empty() {
                            for (i, item) in self.items.iter().enumerate() {
                                if !item.enabled {
                                    continue;
                                }
                                if action_row(ui, z, &item.caption, item.checked.then_some("✓"))
                                    .clicked()
                                {
                                    send(RuffleEvent::ContextMenuItemClicked(i));
                                    item_clicked = true;
                                }
                            }
                        }

                        section(ui, z, "GRAPHICS");
                        row_title(ui, z, "Edge smoothing", "Flash-style is lighter on laptops");
                        if let Some(choice) = segmented(
                            ui,
                            z,
                            "edges",
                            &["Flash-style", "MSAA"],
                            if settings.flash_edges { 0 } else { 1 },
                        ) {
                            send(RuffleEvent::MugetsuSetting(Setting::FlashEdges(choice == 0)));
                            item_clicked = true;
                        }
                        ui.add_space(6.0 * z);
                        if toggle_row(
                            ui,
                            z,
                            "Frame interpolation",
                            "Smoother motion, best on strong PCs",
                            settings.interpolation,
                        ) {
                            send(RuffleEvent::MugetsuSetting(Setting::Interpolation(
                                !settings.interpolation,
                            )));
                            item_clicked = true;
                        }
                        if settings.graphics_choice {
                            ui.add_space(4.0 * z);
                            row_title(ui, z, "Graphics API", "Changing it restarts Mugetsu");
                            let (options, values) = graphics_options();
                            let current = values
                                .iter()
                                .position(|&g| g == settings.graphics)
                                .unwrap_or(0);
                            if let Some(choice) = segmented(ui, z, "graphics", options, current) {
                                send(RuffleEvent::MugetsuSetting(Setting::Graphics(values[choice])));
                                item_clicked = true;
                            }
                        }

                        section(ui, z, "DISPLAY");
                        if toggle_row(ui, z, "Show FPS", "A small counter in the corner", settings.show_fps)
                        {
                            send(RuffleEvent::MugetsuSetting(Setting::ShowFps(!settings.show_fps)));
                            item_clicked = true;
                        }
                        if toggle_row(ui, z, "Full screen", "Esc leaves full screen", fullscreen) {
                            send(if fullscreen {
                                RuffleEvent::ExitFullScreen
                            } else {
                                RuffleEvent::EnterFullScreen
                            });
                            item_clicked = true;
                        }

                        section(ui, z, "HELP");
                        if action_row(ui, z, "Open log folder", Some("↗")).clicked() {
                            send(RuffleEvent::MugetsuSetting(Setting::OpenLogFolder));
                            item_clicked = true;
                        }
                    })
            });

        let should_close = item_clicked
            || area.response.clicked_elsewhere()
            || egui_ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));

        !should_close
    }
}

// --- Mugetsu's panel -------------------------------------------------------------------------

pub(crate) const WIDTH: f32 = 268.0;
pub(crate) const PANEL: Color32 = Color32::from_rgba_premultiplied(17, 19, 26, 242);
pub(crate) const EDGE: Color32 = Color32::from_rgba_premultiplied(58, 62, 78, 255);
pub(crate) const ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(40, 44, 58, 255);
pub(crate) const TRACK: Color32 = Color32::from_rgba_premultiplied(48, 52, 66, 255);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(124, 108, 240);
pub(crate) const TEXT: Color32 = Color32::from_rgb(232, 234, 242);
pub(crate) const MUTED: Color32 = Color32::from_rgb(140, 146, 166);

/// The graphics API choices on this system, and the values they stand for.
pub(crate) fn graphics_options() -> (&'static [&'static str], &'static [Option<Graphics>]) {
    #[cfg(target_os = "linux")]
    return (
        &["Auto", "Vulkan", "OpenGL"],
        &[None, Some(Graphics::Vulkan), Some(Graphics::Gl)],
    );
    #[cfg(target_os = "macos")]
    return (&["Auto", "Metal"], &[None, Some(Graphics::Metal)]);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return (
        &["Auto", "Vulkan", "DirectX 12"],
        &[None, Some(Graphics::Vulkan), Some(Graphics::Dx12)],
    );
}

pub(crate) fn panel_style(style: &mut Style) {
    style.visuals.override_text_color = Some(TEXT);
    style.spacing.button_padding = vec2(0.0, 0.0);
    style.interaction.selectable_labels = false;
}

/// A small gray heading above a group, with a line after it.
pub(crate) fn section(ui: &mut Ui, z: f32, title: &str) {
    ui.add_space(8.0 * z);
    let (rect, _) = ui.allocate_exact_size(vec2(WIDTH * z, 16.0 * z), Sense::hover());
    let text = ui.painter().text(
        pos2(rect.left() + 6.0 * z, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0 * z),
        MUTED,
    );
    ui.painter().hline(
        (text.right() + 6.0 * z)..=(rect.right() - 6.0 * z),
        rect.center().y,
        Stroke::new(1.0, TRACK),
    );
    ui.add_space(2.0 * z);
}

/// A setting's name and hint (for a control below them).
pub(crate) fn row_title(ui: &mut Ui, z: f32, title: &str, hint: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(WIDTH * z, 36.0 * z), Sense::hover());
    paint_title(ui, z, rect, title, hint);
}

fn paint_title(ui: &Ui, z: f32, rect: Rect, title: &str, hint: &str) {
    let painter = ui.painter();
    painter.text(
        pos2(rect.left() + 8.0 * z, rect.top() + 11.0 * z),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(14.5 * z),
        TEXT,
    );
    painter.text(
        pos2(rect.left() + 8.0 * z, rect.top() + 27.0 * z),
        Align2::LEFT_CENTER,
        hint,
        FontId::proportional(11.5 * z),
        MUTED,
    );
}

/// A row with a title, a hint and an on/off switch; returns whether it was clicked.
pub(crate) fn toggle_row(ui: &mut Ui, z: f32, title: &str, hint: &str, on: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(WIDTH * z, 40.0 * z), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same((8.0 * z) as u8), ROW_HOVER);
    }
    paint_title(ui, z, rect.shrink2(vec2(0.0, 2.0 * z)), title, hint);
    // The switch, sliding (animated) between off and on.
    let how_on = ui.ctx().animate_bool_responsive(response.id, on);
    let switch = Rect::from_center_size(
        pos2(rect.right() - 26.0 * z, rect.center().y),
        vec2(34.0 * z, 19.0 * z),
    );
    let radius = switch.height() / 2.0;
    let track = lerp_color(TRACK, ACCENT, how_on);
    ui.painter().rect_filled(switch, CornerRadius::same(radius as u8), track);
    let knob_x = egui::lerp((switch.left() + radius)..=(switch.right() - radius), how_on);
    ui.painter()
        .circle_filled(pos2(knob_x, switch.center().y), radius - 3.0 * z, Color32::WHITE);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// Pill-shaped choices side by side; returns the one clicked, if it isn't `selected`.
pub(crate) fn segmented(ui: &mut Ui, z: f32, id: &str, options: &[&str], selected: usize) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(vec2(WIDTH * z, 30.0 * z), Sense::hover());
    let rect = rect.shrink2(vec2(6.0 * z, 2.0 * z));
    ui.painter().rect_filled(rect, CornerRadius::same((13.0 * z) as u8), TRACK);
    let width = rect.width() / options.len() as f32;
    let mut chosen = None;
    for (i, option) in options.iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(rect.left() + width * i as f32, rect.top()),
            vec2(width, rect.height()),
        );
        let response = ui.interact(cell, Id::new((id, i)), Sense::click());
        let active = ui.ctx().animate_bool_responsive(response.id, i == selected);
        let pill = cell.shrink(2.0 * z);
        let pill_radius = CornerRadius::same((11.0 * z) as u8);
        if active > 0.0 {
            ui.painter().rect_filled(pill, pill_radius, ACCENT.gamma_multiply(active));
        } else if response.hovered() {
            ui.painter().rect_filled(pill, pill_radius, ROW_HOVER);
        }
        ui.painter().text(
            cell.center(),
            Align2::CENTER_CENTER,
            *option,
            FontId::proportional(13.0 * z),
            if i == selected { Color32::WHITE } else { MUTED },
        );
        if response.hovered() && i != selected {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() && i != selected {
            chosen = Some(i);
        }
    }
    chosen
}

/// A clickable row with a title (and something on the right, like an arrow).
pub(crate) fn action_row(ui: &mut Ui, z: f32, title: &str, right: Option<&str>) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(WIDTH * z, 30.0 * z), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same((8.0 * z) as u8), ROW_HOVER);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        pos2(rect.left() + 8.0 * z, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(14.5 * z),
        TEXT,
    );
    if let Some(right) = right {
        ui.painter().text(
            pos2(rect.right() - 10.0 * z, rect.center().y),
            Align2::RIGHT_CENTER,
            right,
            FontId::proportional(13.0 * z),
            MUTED,
        );
    }
    response
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

// Shamelessly stolen from egui menu::set_menu_style, a private internal function
fn set_menu_style(style: &mut Style) {
    style.spacing.button_padding = vec2(2.0, 0.0);
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
}
