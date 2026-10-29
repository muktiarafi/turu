//! VibeSkua's 34 px top nav bar: menus on the left, Auto / Jump / "+" on the right. Entries
//! for windows Turu doesn't have yet are shown greyed with the tooltip "Coming soon" so the
//! layout matches VibeSkua.

use super::skills::SkillSets;
use super::theme::{ACCENT, EDGE, HOVER, PANEL, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use super::{Windows, auto};
use egui::containers::menu::{MenuBar, MenuConfig};
use egui::{
    Align, Align2, Button, Color32, ComboBox, CornerRadius, FontId, Frame, Layout, Margin, Panel, Popup,
    PopupCloseBehavior, RectAlign, Response, RichText, Sense, Style, Ui, vec2,
};
use serde_json::json;
use std::sync::{Arc, Mutex, MutexGuard};
use turu_bot::api::dispatch;

/// Height of the bar; the same as `gui::MENU_HEIGHT`.
const HEIGHT: f32 = 34.0;
const PADS: [&str; 7] = ["Spawn", "Left", "Right", "Top", "Bottom", "Up", "Down"];

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Fast Map Jump's choices; filled from the game by bot threads.
struct JumpState {
    cells: Vec<String>,
    cell: Option<String>,
    pad: String,
}

pub struct NavBar {
    sets: SkillSets,
    auto_class: Option<String>,
    auto_mode: Option<String>,
    map_ids: String,
    auto_was_open: bool,
    jump: Arc<Mutex<JumpState>>,
    jump_was_open: bool,
}

impl Default for NavBar {
    fn default() -> Self {
        Self {
            sets: SkillSets::default(),
            auto_class: None,
            auto_mode: None,
            map_ids: String::new(),
            auto_was_open: false,
            jump: Arc::new(Mutex::new(JumpState {
                cells: Vec::new(),
                cell: None,
                pad: "Left".into(),
            })),
            jump_was_open: false,
        }
    }
}

/// Flat, transparent buttons (the menu bar).
fn bar_style(style: &mut Style) {
    style.spacing.button_padding = vec2(10.0, 4.0);
    style.spacing.item_spacing = vec2(2.0, 0.0);
    let w = &mut style.visuals.widgets;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = egui::Stroke::NONE;
    w.hovered.weak_bg_fill = HOVER;
    w.hovered.bg_stroke = egui::Stroke::NONE;
    w.active.bg_stroke = egui::Stroke::NONE;
    w.open.weak_bg_fill = HOVER;
}

/// Menu drop-downs: full-width rows.
fn menu_style(style: &mut Style) {
    style.spacing.button_padding = vec2(10.0, 4.0);
    style.spacing.item_spacing = vec2(0.0, 1.0);
    let w = &mut style.visuals.widgets;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = egui::Stroke::NONE;
    w.hovered.weak_bg_fill = HOVER;
    w.hovered.bg_stroke = egui::Stroke::NONE;
    w.active.bg_stroke = egui::Stroke::NONE;
    w.open.weak_bg_fill = HOVER;
}

/// A menu entry; greyed with "Coming soon" when disabled. Closes the menu when clicked.
fn item(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let response = ui.add_enabled(enabled, Button::new(label).min_size(vec2(190.0, 0.0)));
    let response = if enabled {
        response
    } else {
        response.on_disabled_hover_text("Coming soon")
    };
    let clicked = response.clicked();
    if clicked {
        ui.close();
    }
    clicked
}

impl NavBar {
    pub fn show(&mut self, ui: &mut Ui, windows: &mut Windows) {
        let frame = Frame::new().fill(PANEL).inner_margin(Margin {
            left: 6,
            right: 6,
            top: 3,
            bottom: 3,
        });
        Panel::top("turu_nav")
            .exact_size(HEIGHT)
            .frame(frame)
            .show_separator_line(true)
            .show(ui, |ui| {
                MenuBar::new()
                    .style(bar_style)
                    .config(MenuConfig::new().style(menu_style))
                    .ui(ui, |ui| {
                        self.menus(ui, windows);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| self.right_side(ui, windows));
                    });
            });
    }

    fn menus(&self, ui: &mut Ui, windows: &mut Windows) {
        ui.menu_button("Scripts", |ui| {
            if item(ui, "Script Loader", true) {
                windows.script_loader = true;
            }
            item(ui, "Scheduler", false);
        });
        ui.menu_button("Options", |ui| {
            if item(ui, "Game", true) {
                windows.game_options = true;
            }
            for name in ["Application", "CoreBots", "Application Themes", "HotKeys"] {
                item(ui, name, false);
            }
        });
        ui.menu_button("Tools & Helpers", |ui| {
            for name in ["Loadouts Manager", "Fast Travel", "Loader", "Grabber"] {
                item(ui, name, false);
            }
            if item(ui, "Current Drops", true) {
                windows.current_drops = true;
            }
            item(ui, "Junk Items", false);
        });
        ui.menu_button("Combat", |ui| {
            if item(ui, "Skills", true) {
                windows.skills = true;
            }
            if item(ui, "Stats", true) {
                windows.stats = true;
            }
            item(ui, "Runtime", false);
        });
        if ui.add(Button::new("Bank")).clicked() {
            crate::turu::spawn_action("Bank", |bot| dispatch(bot, "bank.open", &[]).map(|_| ()));
        }
        ui.menu_button("Diagnostics", |ui| {
            if item(ui, "Logs", true) {
                windows.logs = true;
            }
            item(ui, "Console", false);
            item(ui, "Spammer", false);
            if item(ui, "Logger", true) {
                windows.packet_logger = true;
            }
            item(ui, "Interceptor", false);
        });
        ui.menu_button("Plugins", |ui| {
            item(ui, "View Plugins", false);
        });
    }

    /// Right to left: "+", a divider, Jump, Auto.
    fn right_side(&mut self, ui: &mut Ui, windows: &mut Windows) {
        let (rect, plus) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
        if plus.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(8), HOVER);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "+",
            FontId::proportional(18.0),
            TEXT,
        );
        if plus.on_hover_text("Show All Bot Windows & Tools").clicked() {
            windows.script_loader = true;
        }

        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(vec2(1.0, 18.0), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, EDGE);
        ui.add_space(4.0);

        self.jump_toggle(ui);
        self.auto_toggle(ui);
    }

    fn auto_toggle(&mut self, ui: &mut Ui) {
        let running = auto::running();
        let (response, open) = toggle_button(ui, "Auto", running.then_some(SUCCESS));
        if open && !self.auto_was_open {
            self.sets = SkillSets::load();
        }
        self.auto_was_open = open;
        Popup::from_toggle_button_response(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .align(RectAlign::BOTTOM_END)
            .width(280.0)
            .show(|ui| self.auto_panel(ui));
    }

    fn auto_panel(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let running = auto::running();
        let script_active = bot.scripts.status().is_active();
        ui.set_width(256.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Combat Automation").strong());
            if running {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::pill(ui, |ui| {
                        ui.label(RichText::new("RUNNING").size(9.0).strong().color(SUCCESS));
                    });
                });
            }
        });
        let half = (ui.available_width() - 6.0) / 2.0;
        ui.horizontal(|ui| {
            let tip = if running {
                "Already running"
            } else {
                "Stop the script first"
            };
            let attack = widgets::button(
                ui,
                "Auto Attack",
                Kind::Primary,
                !running && !script_active,
                vec2(half, 30.0),
                tip,
            );
            if attack.clicked() {
                let mode = self.auto_mode.clone().unwrap_or_else(|| "Base".into());
                auto::start(self.auto_class.clone(), mode);
            }
            widgets::button(ui, "Auto Hunt", Kind::Normal, false, vec2(half, 30.0), "Coming soon");
        });
        if widgets::button(
            ui,
            "Stop Automation",
            Kind::Danger,
            running,
            vec2(ui.available_width(), 30.0),
            "Nothing is running",
        )
        .clicked()
        {
            auto::stop();
        }
        ui.horizontal(|ui| {
            ComboBox::from_id_salt("auto_class")
                .selected_text(self.auto_class.as_deref().unwrap_or("Select Class..."))
                .width(half - 18.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.auto_class.is_none(), "Equipped class")
                        .clicked()
                    {
                        self.auto_class = None;
                        self.auto_mode = None;
                    }
                    for class in self.sets.classes() {
                        if ui
                            .selectable_label(self.auto_class.as_deref() == Some(class), class)
                            .clicked()
                        {
                            self.auto_class = Some(class.clone());
                            self.auto_mode = None;
                        }
                    }
                });
            let modes = self
                .auto_class
                .as_deref()
                .map(|c| self.sets.modes(c))
                .unwrap_or_default();
            ComboBox::from_id_salt("auto_mode")
                .selected_text(self.auto_mode.as_deref().unwrap_or("Select Mode..."))
                .width(half - 18.0)
                .show_ui(ui, |ui| {
                    for mode in modes {
                        if ui
                            .selectable_label(self.auto_mode.as_deref() == Some(&mode), &mode)
                            .clicked()
                        {
                            self.auto_mode = Some(mode);
                        }
                    }
                });
        });
        ui.add_enabled_ui(false, |ui| {
            widgets::text_box(
                ui,
                &mut self.map_ids,
                "Manual MapIDs (optional, e.g. 1, 2)",
                ui.available_width(),
            );
        })
        .response
        .on_disabled_hover_text("Coming soon (Auto Hunt)");
    }

    fn jump_toggle(&mut self, ui: &mut Ui) {
        let (response, open) = toggle_button(ui, "Jump", None);
        if open && !self.jump_was_open {
            let state = self.jump.clone();
            crate::turu::spawn_action("Fast Map Jump cells", move |bot| {
                let cells: Vec<String> = serde_json::from_value(dispatch(bot, "map.cells", &[])?).unwrap_or_default();
                lock(&state).cells = cells;
                Ok(())
            });
        }
        self.jump_was_open = open;
        Popup::from_toggle_button_response(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .align(RectAlign::BOTTOM_END)
            .width(260.0)
            .show(|ui| self.jump_panel(ui));
    }

    fn jump_panel(&self, ui: &mut Ui) {
        ui.set_width(236.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        ui.label(RichText::new("Fast Map Jump").strong());
        let half = (ui.available_width() - 6.0) / 2.0;
        let mut state = lock(&self.jump);
        ui.horizontal(|ui| {
            let cells = state.cells.clone();
            ComboBox::from_id_salt("jump_cell")
                .selected_text(state.cell.as_deref().unwrap_or("Select Cell..."))
                .width(half - 18.0)
                .show_ui(ui, |ui| {
                    for cell in &cells {
                        if ui.selectable_label(state.cell.as_deref() == Some(cell), cell).clicked() {
                            state.cell = Some(cell.clone());
                        }
                    }
                });
            let current = state.pad.clone();
            ComboBox::from_id_salt("jump_pad")
                .selected_text(if current.is_empty() { "Select Pad..." } else { &current })
                .width(half - 18.0)
                .show_ui(ui, |ui| {
                    for pad in PADS {
                        if ui.selectable_label(current == pad, pad).clicked() {
                            state.pad = pad.to_owned();
                        }
                    }
                });
        });
        let target = state.cell.clone().map(|cell| (cell, state.pad.clone()));
        drop(state);
        ui.horizontal(|ui| {
            if widgets::button(ui, "Current", Kind::Normal, true, vec2(half, 28.0), "").clicked() {
                let state = self.jump.clone();
                crate::turu::spawn_action("Fast Map Jump current", move |bot| {
                    let cell = dispatch(bot, "player.cell", &[])?;
                    let pad = dispatch(bot, "player.pad", &[])?;
                    let mut state = lock(&state);
                    state.cell = cell.as_str().map(str::to_owned);
                    if let Some(pad) = pad.as_str() {
                        state.pad = pad.to_owned();
                    }
                    Ok(())
                });
            }
            let jump = widgets::button(
                ui,
                "Jump",
                Kind::Primary,
                target.is_some(),
                vec2(half, 28.0),
                "Select a cell first",
            );
            if jump.clicked()
                && let Some((cell, pad)) = target
            {
                crate::turu::spawn_action("Fast Map Jump", move |bot| {
                    dispatch(bot, "map.jump", &[json!(cell), json!(pad)]).map(|_| ())
                });
            }
        });
    }
}

/// A flat toggle button for a popup ("Auto", "Jump") with an optional status dot. Returns the
/// response and whether its popup is open.
fn toggle_button(ui: &mut Ui, label: &str, dot: Option<Color32>) -> (Response, bool) {
    let font = FontId::proportional(13.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, TEXT);
    let width = galley.size().x + 22.0 + if dot.is_some() { 12.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
    let open = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response));
    if open {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), ACCENT.gamma_multiply(0.35));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), HOVER);
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut x = rect.left() + 11.0;
    if let Some(color) = dot {
        ui.painter()
            .circle_filled(egui::pos2(x + 3.0, rect.center().y), 3.0, color);
        x += 12.0;
    }
    let color = if open || response.hovered() {
        Color32::WHITE
    } else {
        TEXT
    };
    ui.painter()
        .galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
    (response, open)
}
