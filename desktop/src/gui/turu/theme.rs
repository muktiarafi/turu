//! Turu's egui look: Mugetsu's dark palette (the right-click panel in `context_menu.rs`) with
//! VibeSkua's status colours.

use crate::gui::context_menu::{ACCENT as MUGETSU_ACCENT, EDGE as MUGETSU_EDGE, MUTED as MUGETSU_MUTED};
use crate::gui::context_menu::{ROW_HOVER as MUGETSU_HOVER, TEXT as MUGETSU_TEXT, TRACK as MUGETSU_TRACK};
use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle, vec2};

/// Window and bar background (Mugetsu's panel colour, opaque).
pub const PANEL: Color32 = Color32::from_rgb(17, 19, 26);
/// Cards and text boxes sit on a slightly lighter surface.
pub const CARD: Color32 = Color32::from_rgb(24, 27, 37);
/// Text box / list background, darker than the panel.
pub const INSET: Color32 = Color32::from_rgb(12, 13, 19);
pub const EDGE: Color32 = MUGETSU_EDGE;
pub const HOVER: Color32 = MUGETSU_HOVER;
pub const TRACK: Color32 = MUGETSU_TRACK;
pub const ACCENT: Color32 = MUGETSU_ACCENT;
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(144, 129, 244);
pub const ACCENT_PRESSED: Color32 = Color32::from_rgb(105, 90, 214);
pub const TEXT: Color32 = MUGETSU_TEXT;
pub const MUTED: Color32 = MUGETSU_MUTED;
/// VibeSkua's status colours (`Colors.axaml`).
pub const SUCCESS: Color32 = Color32::from_rgb(0x34, 0xD3, 0x99);
pub const WARNING: Color32 = Color32::from_rgb(0xFB, 0xBF, 0x24);
pub const DANGER: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);
pub const DANGER_HOVER: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);
pub const DANGER_PRESSED: Color32 = Color32::from_rgb(0xB9, 0x1C, 0x1C);

/// Mugetsu's corner radius.
pub const RADIUS: u8 = 12;
/// Radius of buttons, text boxes and rows.
pub const SMALL_RADIUS: u8 = 8;

/// The egui style of Turu's bar and windows. Built on `Style::default()`, dark.
pub fn style() -> Style {
    let mut style = Style::default();

    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::new(13.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::new(16.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace));

    let spacing = &mut style.spacing;
    spacing.item_spacing = vec2(8.0, 6.0);
    spacing.button_padding = vec2(10.0, 5.0);
    spacing.interact_size = vec2(32.0, 24.0);
    spacing.menu_margin = Margin::same(6);
    spacing.window_margin = Margin::same(12);
    spacing.combo_width = 120.0;
    spacing.scroll.bar_width = 6.0;
    spacing.scroll.floating = false;

    let visuals = &mut style.visuals;
    *visuals = egui::Visuals::dark();
    visuals.override_text_color = None;
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.window_stroke = Stroke::new(1.0, EDGE);
    visuals.window_corner_radius = CornerRadius::same(RADIUS);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.extreme_bg_color = INSET;
    visuals.faint_bg_color = CARD;
    visuals.code_bg_color = INSET;
    visuals.hyperlink_color = ACCENT;
    visuals.window_shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(140),
    };
    visuals.popup_shadow = visuals.window_shadow;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.text_cursor.stroke = Stroke::new(2.0, TEXT);
    visuals.striped = false;

    let radius = CornerRadius::same(SMALL_RADIUS);
    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, EDGE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = TRACK;
    w.inactive.weak_bg_fill = TRACK;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = HOVER;
    w.hovered.weak_bg_fill = HOVER;
    w.hovered.bg_stroke = Stroke::new(1.0, EDGE);
    w.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = ACCENT;
    w.active.weak_bg_fill = ACCENT_PRESSED;
    w.active.bg_stroke = Stroke::NONE;
    w.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open.bg_fill = HOVER;
    w.open.weak_bg_fill = HOVER;
    w.open.bg_stroke = Stroke::NONE;
    w.open.fg_stroke = Stroke::new(1.0, TEXT);
    w.open.corner_radius = radius;

    style.interaction.selectable_labels = true;
    style
}

/// Installs Turu's style for both of egui's themes (the system theme picks between them, and
/// Turu is dark either way).
pub fn install(ctx: &egui::Context) {
    let style = style();
    ctx.all_styles_mut(|s| *s = style.clone());
}
