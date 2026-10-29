//! VibeSkua's 34 px top nav bar: menus on the left, Auto / Jump / "+" on the right. Entries
//! for windows Turu doesn't have yet are shown greyed with the tooltip "Coming soon" so the
//! layout matches VibeSkua.

use super::skills::SkillSets;
use super::theme::{ACCENT, EDGE, HOVER, MUTED, PANEL, SUCCESS, TEXT};
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
/// VibeSkua `MapService.Pads`, in its order.
const PADS: [&str; 8] = ["Spawn", "Center", "Left", "Right", "Up", "Down", "Top", "Bottom"];

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Fast Map Jump's choices; filled from the game by bot threads.
struct JumpState {
    cells: Vec<String>,
    cell: Option<String>,
    pad: String,
    /// The last jump's outcome, shown under the buttons.
    status: Option<String>,
}

pub struct NavBar {
    sets: SkillSets,
    auto_class: Option<String>,
    auto_mode: Option<String>,
    map_ids: String,
    /// The Auto and Jump popups' open state. Kept here, not in egui's memory: egui keeps one
    /// open popup per window, so opening a drop-down inside them would close them.
    auto_open: bool,
    jump: Arc<Mutex<JumpState>>,
    jump_open: bool,
    cells_dropdown_was_open: bool,
}

impl Default for NavBar {
    fn default() -> Self {
        Self {
            sets: SkillSets::default(),
            auto_class: None,
            auto_mode: None,
            map_ids: String::new(),
            auto_open: false,
            jump: Arc::new(Mutex::new(JumpState {
                cells: Vec::new(),
                cell: None,
                pad: String::new(),
                status: None,
            })),
            jump_open: false,
            cells_dropdown_was_open: false,
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
        let response = toggle_button(ui, "Auto", running.then_some(SUCCESS), self.auto_open);
        if response.clicked() {
            self.auto_open = !self.auto_open;
            if self.auto_open {
                self.sets = SkillSets::load();
            }
        }
        let mut open = self.auto_open;
        let shown = nav_popup(&response, &mut open, 280.0).show(|ui| self.auto_panel(ui));
        close_on_click_outside(ui.ctx(), &mut open, shown.map(|s| s.response.rect), response.rect);
        self.auto_open = open;
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
        let response = toggle_button(ui, "Jump", None, self.jump_open);
        if response.clicked() {
            self.jump_open = !self.jump_open;
            if self.jump_open {
                self.refresh_cells();
            }
        }
        let mut open = self.jump_open;
        let shown = nav_popup(&response, &mut open, 260.0).show(|ui| self.jump_panel(ui));
        close_on_click_outside(ui.ctx(), &mut open, shown.map(|s| s.response.rect), response.rect);
        self.jump_open = open;
    }

    /// VibeSkua `JumpViewModel`: picking a cell jumps there at once ("Enter" with pad
    /// "Spawn", else the chosen pad or "Left"); Current selects where you are without
    /// jumping; the cell list is refreshed whenever its drop-down opens.
    fn jump_panel(&mut self, ui: &mut Ui) {
        ui.set_width(236.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        ui.label(RichText::new("Fast Map Jump").strong());
        ui.separator();
        let half = (ui.available_width() - 6.0) / 2.0;
        let mut state = lock(&self.jump);
        let mut picked = None;
        let mut cells_open = false;
        ui.horizontal(|ui| {
            let cells = state.cells.clone();
            let combo = ComboBox::from_id_salt("jump_cell")
                .selected_text(state.cell.as_deref().unwrap_or("Select Cell..."))
                .width(half - 18.0)
                .height(320.0)
                .show_ui(ui, |ui| {
                    if cells.is_empty() {
                        ui.label(RichText::new("No cells (not in a map?)").small().color(MUTED));
                    }
                    for cell in &cells {
                        if ui.selectable_label(state.cell.as_deref() == Some(cell), cell).clicked() {
                            picked = Some(cell.clone());
                        }
                    }
                });
            cells_open = combo.inner.is_some();
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
        if let Some(cell) = picked {
            if cell == "Enter" {
                state.pad = "Spawn".into();
            } else if state.pad.is_empty() {
                state.pad = "Left".into();
            }
            state.cell = Some(cell.clone());
            let pad = state.pad.clone();
            drop(state);
            self.jump_to(cell, pad);
            state = lock(&self.jump);
        }
        let target = state
            .cell
            .clone()
            .filter(|_| !state.pad.is_empty())
            .map(|cell| (cell, state.pad.clone()));
        let status = state.status.clone();
        drop(state);
        if cells_open && !self.cells_dropdown_was_open {
            self.refresh_cells();
        }
        self.cells_dropdown_was_open = cells_open;

        ui.horizontal(|ui| {
            if widgets::button(ui, "Current", Kind::Normal, true, vec2(half, 28.0), "").clicked() {
                self.refresh_cells();
                let state = self.jump.clone();
                crate::turu::spawn_action("Fast Map Jump current", move |bot| {
                    let playing = dispatch(bot, "player.playing", &[])?.as_bool() == Some(true);
                    let (cell, pad) = if playing {
                        (dispatch(bot, "player.cell", &[])?, dispatch(bot, "player.pad", &[])?)
                    } else {
                        (json!("Enter"), json!("Spawn"))
                    };
                    let mut state = lock(&state);
                    state.cell = cell.as_str().filter(|c| !c.is_empty()).map(str::to_owned);
                    state.pad = pad.as_str().unwrap_or_default().to_owned();
                    Ok(())
                });
            }
            let jump = widgets::button(
                ui,
                "Jump",
                Kind::Primary,
                target.is_some(),
                vec2(half, 28.0),
                "Select a cell and a pad first",
            );
            if jump.clicked()
                && let Some((cell, pad)) = target
            {
                self.jump_to(cell, pad);
            }
        });
        if let Some(status) = status {
            ui.label(RichText::new(status).small().color(MUTED));
        }
    }

    /// Reads the current map's cells (`map.cells`) into the drop-down.
    fn refresh_cells(&self) {
        let state = self.jump.clone();
        crate::turu::spawn_action("Fast Map Jump cells", move |bot| {
            let cells: Vec<String> =
                serde_json::from_value(dispatch(bot, "map.cells", &[])?).unwrap_or_default();
            let mut state = lock(&state);
            if state.cells != cells {
                state.cells = cells;
            }
            Ok(())
        });
    }

    /// VibeSkua `MapService.Jump`: only when logged in, without auto-correcting the pad.
    fn jump_to(&self, cell: String, pad: String) {
        let state = self.jump.clone();
        lock(&state).status = Some(format!("Jumping to {cell} ({pad})..."));
        crate::turu::spawn_action("Fast Map Jump", move |bot| {
            let status = if dispatch(bot, "player.loggedIn", &[])?.as_bool() != Some(true) {
                "Not logged in".to_owned()
            } else {
                match dispatch(bot, "map.jump", &[json!(cell), json!(pad), json!(false)]) {
                    Ok(_) => format!("Jumped to {cell} ({pad})"),
                    Err(e) => format!("Jump failed: {e}"),
                }
            };
            lock(&state).status = Some(status);
            Ok(())
        });
    }
}

/// A nav-bar popup under `button`, open while `open` is true. Clicks are handled by
/// [`close_on_click_outside`]: egui's own "close on click outside" also fires when an item of
/// one of its drop-downs is picked (those are separate layers).
fn nav_popup<'a>(button: &Response, open: &'a mut bool, width: f32) -> Popup<'a> {
    Popup::from_response(button)
        .open_bool(open)
        .close_behavior(PopupCloseBehavior::IgnoreClicks)
        .align(RectAlign::BOTTOM_END)
        .width(width)
}

/// Closes a nav-bar popup when a click lands outside it and its toggle button, unless it lands
/// on another pop-up layer, such as one of its own drop-downs.
fn close_on_click_outside(ctx: &egui::Context, open: &mut bool, popup: Option<egui::Rect>, button: egui::Rect) {
    let Some(popup) = popup else { return };
    let Some(pos) = ctx.input(|i| i.pointer.any_click().then(|| i.pointer.interact_pos()).flatten()) else {
        return;
    };
    if popup.contains(pos) || button.contains(pos) {
        return;
    }
    let on_popup = ctx
        .layer_id_at(pos)
        .is_some_and(|layer| matches!(layer.order, egui::Order::Foreground | egui::Order::Tooltip));
    if !on_popup {
        *open = false;
    }
}

/// A flat toggle button for a popup ("Auto", "Jump") with an optional status dot, highlighted
/// while its popup is `open`.
fn toggle_button(ui: &mut Ui, label: &str, dot: Option<Color32>, open: bool) -> Response {
    let font = FontId::proportional(13.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, TEXT);
    let width = galley.size().x + 22.0 + if dot.is_some() { 12.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
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
    response
}
