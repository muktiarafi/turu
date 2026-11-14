//! The nav bar: 34 px across the top, or on a touch screen a rail down the left (below). Left: the Turu symbol and VibeSkua's menus. Right: the game's quick
//! actions (Grid, Auto, Jump, Bank), then the script's status with its run controls, the one
//! place a check-in needs.

use super::skills::SkillSets;
use super::theme::{
    ACCENT, ACCENT_HOVER, DANGER, DANGER_HOVER, EDGE, HOVER, MUTED, ON_ACCENT, PANEL, SMALL_RADIUS, SUCCESS, TEXT,
    TRACK, WARNING,
};
use super::widgets::{self, Kind};
use super::{Windows, auto, script_loader};
use crate::map_pads::MapCells;
use egui::containers::menu::{MenuBar, MenuConfig};
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Frame, Id, Layout, Margin, Panel, Popup, PopupCloseBehavior,
    Rect, RectAlign, Response, RichText, Sense, Spinner, Stroke, Style, Ui, pos2, vec2,
};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use turu_bot::api::dispatch;
use turu_bot::map_scripts::MapScript;
use turu_bot::script::{self, ScriptOptionsState, ScriptStatus};

/// The menus, in VibeSkua's order. Bank is an action, not a menu: it sits with the quick
/// actions on the right, and in "Menu" once everything is folded.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Scripts,
    Options,
    Tools,
    Combat,
    Diagnostics,
    Plugins,
    Army,
}

const MENUS: [(Menu, &str); 7] = [
    (Menu::Scripts, "Scripts"),
    (Menu::Options, "Options"),
    (Menu::Tools, "Tools & Helpers"),
    (Menu::Combat, "Combat"),
    (Menu::Diagnostics, "Diagnostics"),
    (Menu::Plugins, "Plugins"),
    (Menu::Army, "Army"),
];

/// The menus this platform has: all but Army where several clients don't work (Android).
fn menus() -> &'static [(Menu, &'static str)] {
    if crate::platform::platform().has_army() {
        &MENUS
    } else {
        &MENUS[..MENUS.len() - 1]
    }
}

/// How the menus fit next to the right side: all of them; Scripts and a "More" menu with the
/// rest; or one "Menu" with everything, Bank included, without the symbol and with a shorter
/// script name (a phone held upright).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fold {
    None,
    More,
    All,
}
/// VibeSkua `MapService.Pads`, in its order.
const PADS: [&str; 8] = ["Spawn", "Center", "Left", "Right", "Up", "Down", "Top", "Bottom"];

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Bank's last press: a spinner in its button while it runs, then for a moment, under it, why
/// nothing opened (`bank.open` itself does nothing logged out or while a map loads).
#[derive(Default)]
struct BankState {
    busy: bool,
    failed: Option<(String, Instant)>,
}

/// How long Bank's failure shows.
const BANK_NOTE_FOR: Duration = Duration::from_secs(4);

/// Fast Map Jump's choices; filled from the game by bot threads.
struct JumpState {
    cells: Vec<String>,
    cell: Option<String>,
    pad: String,
    /// The current map's cells, pads and doors, read from its SWF (`map_pads`): the map file,
    /// and what it says (empty while it's read or when it can't be).
    map: Option<(String, MapCells)>,
    /// The last jump's outcome, shown under the buttons.
    status: Option<String>,
}

pub struct NavBar {
    sets: SkillSets,
    /// The Class items in the inventory, for Auto's class drop-down (VibeSkua `PlayerClasses`).
    owned_classes: Arc<Mutex<Vec<String>>>,
    /// While [`Self::reload_classes`] reads them.
    classes_loading: Arc<AtomicBool>,
    class_dropdown_was_open: bool,
    auto_class: Option<String>,
    auto_mode: Option<String>,
    map_ids: String,
    /// The Auto and Jump popups' open state. Kept here, not in egui's memory: egui keeps one
    /// open popup per window, so opening a drop-down inside them would close them.
    auto_open: bool,
    jump: Arc<Mutex<JumpState>>,
    jump_open: bool,
    cells_dropdown_was_open: bool,
    /// The right side's width on the last frame it was drawn in full (not folded all the way):
    /// the menus fold when they'd run into it.
    right_width: f32,
    /// The right side's width on the last frame, without the script's name: folded all the
    /// way, the name gets what's left, or nothing.
    right_rest: f32,
    bank: Arc<Mutex<BankState>>,
    /// The scripts for the active tab's map, on the rail's script chip.
    map_scripts: MapScriptsCache,
}

/// The scripts made for the active tab's map (`turu_bot::map_scripts`), worked out again when the
/// map or the Search Scripts list changes.
#[derive(Default)]
struct MapScriptsCache {
    map: String,
    generation: u64,
    /// `None` while the scripts folder is read.
    list: Option<Vec<MapScript>>,
}

impl MapScriptsCache {
    /// The scripts for `map`; `None` when there are none (or not yet).
    fn get(&mut self, bot: &turu_bot::Bot, map: &str) -> Option<&[MapScript]> {
        let repo = crate::turu::script_repo();
        let generation = repo.generation();
        if self.map != map || self.generation != generation || self.list.is_none() {
            let snapshot = repo.snapshot();
            self.list = turu_bot::map_scripts::scripts_for(&bot.paths.scripts_dir(), map, &snapshot.entries);
            self.map = map.to_owned();
            self.generation = snapshot.generation;
        }
        self.list.as_deref().filter(|list| !list.is_empty())
    }
}

impl Default for NavBar {
    fn default() -> Self {
        Self {
            sets: SkillSets::default(),
            owned_classes: Arc::default(),
            classes_loading: Arc::default(),
            class_dropdown_was_open: false,
            auto_class: None,
            auto_mode: None,
            map_ids: String::new(),
            auto_open: false,
            jump: Arc::new(Mutex::new(JumpState {
                cells: Vec::new(),
                cell: None,
                pad: String::new(),
                map: None,
                status: None,
            })),
            jump_open: false,
            cells_dropdown_was_open: false,
            right_width: 0.0,
            right_rest: 0.0,
            bank: Arc::default(),
            map_scripts: MapScriptsCache::default(),
        }
    }
}

/// The height of everything in the bar: 26 points, 40 on a touch screen.
fn control_height() -> f32 {
    widgets::target(26.0, 40.0)
}

/// Flat, transparent buttons (the menu bar), all [`control_height`] high so their labels share
/// a baseline with the quick actions'. An open menu is tinted with the accent, as an open
/// Auto or Jump is.
fn bar_style(style: &mut Style) {
    style.spacing.button_padding = vec2(10.0, 0.0);
    style.spacing.interact_size.y = control_height();
    style.spacing.item_spacing = vec2(2.0, 0.0);
    let w = &mut style.visuals.widgets;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = Stroke::NONE;
    w.hovered.weak_bg_fill = HOVER;
    w.hovered.bg_stroke = Stroke::NONE;
    w.active.bg_stroke = Stroke::NONE;
    w.open.weak_bg_fill = OPEN;
    w.open.fg_stroke = Stroke::new(1.0, Color32::WHITE);
}

/// An open menu or popup's button: a dark tint of the accent.
#[cfg(not(feature = "air"))]
const OPEN: Color32 = Color32::from_rgb(52, 48, 92);
#[cfg(feature = "air")]
const OPEN: Color32 = Color32::from_rgb(78, 62, 22);

/// Menu drop-downs: full-width rows, about 44 points high on a touch screen.
fn menu_style(style: &mut Style) {
    style.spacing.button_padding = vec2(10.0, if widgets::touch() { 14.0 } else { 4.0 });
    style.spacing.item_spacing = vec2(0.0, 1.0);
    let w = &mut style.visuals.widgets;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = Stroke::NONE;
    w.hovered.weak_bg_fill = HOVER;
    w.hovered.bg_stroke = Stroke::NONE;
    w.active.bg_stroke = Stroke::NONE;
    w.open.weak_bg_fill = HOVER;
}

/// A menu entry. Closes the menu when clicked.
fn item(ui: &mut Ui, label: &str) -> bool {
    let response = ui.add(Button::new(label).min_size(vec2(190.0, 0.0)));
    let clicked = response.clicked();
    if clicked {
        ui.close();
    }
    clicked
}

impl NavBar {
    /// Bank, as the bar's button does it (the OpenBank hotkey): the bar shows how it went.
    pub(super) fn open_bank(&self) {
        open_bank(&self.bank);
    }

    /// Auto Attack or Auto Hunt with the Auto popup's class and mode, as its buttons; stops it
    /// when one runs (VibeSkua's ToggleAutoAttack/ToggleAutoHunt hotkeys). Like the buttons,
    /// it doesn't start while a script or a Console snippet runs: the log says why.
    pub(super) fn toggle_auto(&self, mode: auto::AutoMode) {
        let bot = crate::turu::bot();
        if auto::running(&bot) {
            auto::stop();
        } else if bot.scripts.status().is_active() {
            bot.log("[HotKeys] Auto doesn't start while a script runs");
        } else if bot.scripts.is_snippet_running() {
            bot.log("[HotKeys] Auto doesn't start while a Console snippet runs");
        } else {
            self.start_auto(mode);
        }
    }

    /// VibeSkua: the picked class and mode only when both are set, else the equipped class in
    /// Base.
    fn start_auto(&self, mode: auto::AutoMode) {
        let map_ids = auto::parse_map_ids(&self.map_ids);
        match (self.auto_class.clone(), self.auto_mode.clone()) {
            (Some(class), Some(class_mode)) => auto::start(mode, Some(class), class_mode, map_ids),
            _ => auto::start(mode, None, "Base".into(), map_ids),
        }
    }

    /// The bar across the top, or on a touch screen the rail down the left
    /// ([`crate::gui::nav_rail`]).
    pub fn show(&mut self, ui: &mut Ui, windows: &mut Windows) {
        if crate::gui::nav_rail() {
            self.show_rail(ui, windows);
            return;
        }
        // The controls are 8 points shorter than the bar: 4 above and below centre them.
        let frame = Frame::new().fill(PANEL).inner_margin(Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 4,
        });
        Panel::top("turu_nav")
            .exact_size(crate::gui::nav_bar_height() as f32)
            .frame(frame)
            .show_separator_line(true)
            .show(ui, |ui| {
                MenuBar::new()
                    .style(bar_style)
                    .config(MenuConfig::new().style(menu_style))
                    .ui(ui, |ui| {
                        let fold = self.fold(ui);
                        if fold != Fold::All {
                            symbol(ui, SYMBOL_SIZE);
                            ui.add_space(SYMBOL_GAP);
                        }
                        let menu = self.menus(ui, windows, fold);
                        let right = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (bank, name) = self.right_side(ui, windows, fold);
                            (ui.min_rect().width(), name, bank)
                        });
                        let (width, name, bank) = right.inner;
                        if fold != Fold::All {
                            self.right_width = width;
                        }
                        self.right_rest = width - name;
                        self.bank_note(ui, bank.as_ref().or(menu.as_ref()), RectAlign::BOTTOM_END);
                    });
            });
    }

    /// The fold that leaves the menus clear of the right side (as wide as on the last frame it
    /// was drawn in full; folded all the way it is narrower, which only leaves more room).
    fn fold(&self, ui: &Ui) -> Fold {
        let padding = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;
        let title = |label: &str| {
            let galley = ui
                .painter()
                .layout_no_wrap(label.to_owned(), FontId::proportional(13.0), TEXT);
            galley.size().x + padding
        };
        let room = ui.available_width() - self.right_width - 12.0;
        let symbol = SYMBOL_SIZE + SYMBOL_GAP + ui.spacing().item_spacing.x;
        let all: f32 = menus().iter().map(|(_, label)| title(label)).sum();
        if symbol + all <= room {
            Fold::None
        } else if symbol + title("Scripts") + title("More") <= room {
            Fold::More
        } else {
            Fold::All
        }
    }

    /// The menus as `fold` puts them; a folded menu is a submenu of "More" or "Menu". Returns
    /// "Menu" when Bank is in it, for Bank's note.
    fn menus(&self, ui: &mut Ui, windows: &mut Windows, fold: Fold) -> Option<Response> {
        match fold {
            Fold::None => {
                for &(menu, label) in menus() {
                    ui.menu_button(label, |ui| menu_contents(ui, windows, menu));
                }
                None
            }
            Fold::More => {
                ui.menu_button("Scripts", |ui| menu_contents(ui, windows, Menu::Scripts));
                ui.menu_button("More", |ui| folded(ui, windows, &menus()[1..], None));
                None
            }
            Fold::All => {
                let menu = ui.menu_button("Menu", |ui| folded(ui, windows, menus(), Some(&self.bank)));
                Some(menu.response)
            }
        }
    }

    /// For a moment after Bank didn't open, why, in a note under its button (or under "Menu"),
    /// or beside it on the rail.
    fn bank_note(&self, ui: &Ui, anchor: Option<&Response>, align: RectAlign) {
        let mut bank = lock(&self.bank);
        let Some((why, at)) = &bank.failed else {
            return;
        };
        let Some(left) = BANK_NOTE_FOR.checked_sub(at.elapsed()) else {
            bank.failed = None;
            return;
        };
        if let Some(anchor) = anchor {
            let text = RichText::new(format!("Bank: {why}")).size(12.0).color(DANGER);
            Popup::from_response(anchor)
                .id(Id::new("turu_bank_note"))
                .open(true)
                .close_behavior(PopupCloseBehavior::IgnoreClicks)
                .align(align)
                .show(|ui| ui.label(text));
        }
        ui.ctx().request_repaint_after(left);
    }

    /// Right to left: the script's status and run controls, a divider, then Bank, Jump, Auto
    /// and Grid. Folded all the way (a phone held upright) Bank moves into "Menu" and the
    /// script's name gets what room is left, up to [`COMPACT_CHIP_NAME_WIDTH`]. Returns Bank's
    /// button, for its note, and how wide the name was drawn.
    fn right_side(&mut self, ui: &mut Ui, windows: &mut Windows, fold: Fold) -> (Option<Response>, f32) {
        let compact = fold == Fold::All;
        let name_width = if compact {
            // What "Menu" leaves, less a gap.
            (ui.available_width() - self.right_rest - 12.0).min(COMPACT_CHIP_NAME_WIDTH)
        } else {
            CHIP_NAME_WIDTH
        };
        let (chip, name) = script_status(ui, name_width);
        if chip.clicked() {
            windows.script_loader = true;
        }

        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(vec2(1.0, 16.0), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, EDGE);
        ui.add_space(6.0);

        let bank = (!compact).then(|| self.bank_toggle(ui));
        self.jump_toggle(ui);
        self.auto_toggle(ui);
        grid_toggle(ui);
        update_notice(ui, windows, compact);
        (bank, name)
    }

    /// Bank: VibeSkua's direct action (no menu), with a spinner while it runs.
    fn bank_toggle(&self, ui: &mut Ui) -> Response {
        let lead = if lock(&self.bank).busy {
            Lead::Spinner
        } else {
            Lead::None
        };
        let response = toggle_button(ui, "Bank", lead, false)
            .on_hover_text(format!("Opens the bank window{}", super::hotkeys::hint("OpenBank")));
        if response.clicked() {
            open_bank(&self.bank);
        }
        response
    }

    fn auto_toggle(&mut self, ui: &mut Ui) {
        let running = auto::running(&crate::turu::bot());
        let lead = if running { Lead::Dot(SUCCESS) } else { Lead::None };
        let response = toggle_button(ui, "Auto", lead, self.auto_open);
        self.auto_popup(ui, &response, RectAlign::BOTTOM_END);
    }

    /// The Auto popup at `button` (Auto on the bar or the rail), which opens and closes it.
    fn auto_popup(&mut self, ui: &Ui, button: &Response, align: RectAlign) {
        if button.clicked() {
            self.auto_open = !self.auto_open;
            if self.auto_open {
                self.sets = SkillSets::load();
            }
        }
        let mut open = self.auto_open;
        let shown = nav_popup(button, &mut open, 280.0, align).show(|ui| self.auto_panel(ui));
        close_on_click_outside(ui.ctx(), &mut open, shown.map(|s| s.response.rect), button.rect);
        self.auto_open = open;
    }

    /// VibeSkua's Auto view: Auto Attack and Auto Hunt, Stop, then the class and mode to fight
    /// with and the maps to hunt in. The two drop-downs line up under the two buttons, and every
    /// control in it is as tall as the buttons (finger-sized on a touch screen).
    fn auto_panel(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let running = auto::running(&bot);
        let script_active = bot.scripts.status().is_active();
        let snippet = bot.scripts.is_snippet_running();
        ui.set_width(AUTO_PANEL_WIDTH);
        ui.spacing_mut().item_spacing = vec2(AUTO_GAP, AUTO_GAP);
        touch_controls(ui);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Combat Automation").strong());
            if running {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::pill(ui, |ui| {
                        ui.label(RichText::new("RUNNING").size(11.0).strong().color(SUCCESS));
                    });
                });
            }
        });
        ui.separator();
        let width = ui.available_width();
        let half = (width - AUTO_GAP) / 2.0;
        let height = widgets::target(30.0, 48.0);
        ui.horizontal(|ui| {
            let tip = if running {
                "Already running"
            } else if snippet {
                "Stop the Console snippet first"
            } else {
                "Stop the script first"
            };
            let can_start = !running && !script_active && !snippet;
            let attack = widgets::button(ui, "Auto Attack", Kind::Primary, can_start, vec2(half, height), tip);
            let hunt = widgets::button(ui, "Auto Hunt", Kind::Normal, can_start, vec2(half, height), tip);
            let mode = if attack.clicked() {
                Some(auto::AutoMode::Attack)
            } else if hunt.clicked() {
                Some(auto::AutoMode::Hunt)
            } else {
                None
            };
            if let Some(mode) = mode {
                self.start_auto(mode);
            }
        });
        if widgets::button(
            ui,
            "Stop Automation",
            Kind::Danger,
            running,
            vec2(width, height),
            "Nothing is running",
        )
        .clicked()
        {
            auto::stop();
        }
        ui.add_space(4.0);
        let mut picked_class = None;
        let mut class_open = false;
        ui.horizontal(|ui| {
            let owned = lock(&self.owned_classes).clone();
            let loading = self.classes_loading.load(Ordering::Relaxed);
            let class = self.auto_class.clone().unwrap_or_else(|| "Select Class...".into());
            // The class list spans the panel: class names are longer than half of it.
            let list = widgets::dropdown(ui, "auto_class", &class, vec2(half, height), width, 420.0, |ui| {
                touch_rows(ui);
                if owned.is_empty() {
                    if loading {
                        ui.horizontal(|ui| {
                            ui.add(Spinner::new().size(12.0));
                            ui.label(RichText::new("Loading classes...").small().color(MUTED));
                        });
                    } else {
                        ui.label(RichText::new("No classes (not logged in?)").small().color(MUTED));
                    }
                }
                for class in &owned {
                    if ui
                        .selectable_label(self.auto_class.as_deref() == Some(class), class)
                        .clicked()
                    {
                        picked_class = Some(class.clone());
                    }
                }
            });
            class_open = list.inner.is_some();
            let modes = self
                .auto_class
                .as_deref()
                .map(|c| self.sets.modes_for(c))
                .unwrap_or_default();
            let mode = self.auto_mode.clone().unwrap_or_else(|| "Select Mode...".into());
            widgets::dropdown(ui, "auto_mode", &mode, vec2(half, height), half, 320.0, |ui| {
                touch_rows(ui);
                if self.auto_class.is_none() {
                    ui.label(RichText::new("Pick a class first").small().color(MUTED));
                } else if modes.is_empty() {
                    ui.label(RichText::new("No saved skills for this class").small().color(MUTED));
                }
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
        // VibeSkua `ReloadClasses` (on opening the class drop-down): re-read the owned classes
        // and the skill sets, and clear the selection.
        if class_open && !self.class_dropdown_was_open {
            self.reload_classes(ui.ctx());
        }
        self.class_dropdown_was_open = class_open;
        // VibeSkua `SelectedClass`: list its modes, pick the first, and equip the class.
        if let Some(class) = picked_class {
            self.auto_mode = self.sets.modes_for(&class).into_iter().next();
            self.auto_class = Some(class.clone());
            crate::turu::spawn_action("Equip class", move |bot| {
                dispatch(bot, "inventory.equipItem", &[json!(class)]).map(|_| ())
            });
        }
        widgets::text_box_sized(
            ui,
            &mut self.map_ids,
            "Manual MapIDs (optional, e.g. 1, 2)",
            vec2(width, height),
        );
    }

    /// Reads the inventory's Class items (sorted, ignoring case) and the skill sets again, and
    /// draws the drop-down again once they are in.
    fn reload_classes(&mut self, ctx: &egui::Context) {
        self.sets = SkillSets::load();
        self.auto_class = None;
        self.auto_mode = None;
        let owned = self.owned_classes.clone();
        let loading = self.classes_loading.clone();
        loading.store(true, Ordering::Relaxed);
        let ctx = ctx.clone();
        crate::turu::spawn_action("Auto classes", move |bot| {
            let read = read_classes(bot, &owned);
            loading.store(false, Ordering::Relaxed);
            ctx.request_repaint();
            read
        });
    }

    fn jump_toggle(&mut self, ui: &mut Ui) {
        let response = toggle_button(ui, "Jump", Lead::None, self.jump_open);
        self.jump_popup(ui, &response, RectAlign::BOTTOM_END);
    }

    /// The Jump popup at `button` (Jump on the bar or the rail), which opens and closes it.
    fn jump_popup(&mut self, ui: &Ui, button: &Response, align: RectAlign) {
        if button.clicked() {
            self.jump_open = !self.jump_open;
            if self.jump_open {
                self.refresh_cells();
            }
        }
        let mut open = self.jump_open;
        let shown = nav_popup(button, &mut open, 260.0, align).show(|ui| self.jump_panel(ui));
        close_on_click_outside(ui.ctx(), &mut open, shown.map(|s| s.response.rect), button.rect);
        self.jump_open = open;
    }

    /// VibeSkua `JumpViewModel`: picking a cell selects a pad ("Enter" with pad "Spawn", else
    /// the chosen pad or "Left"); with Auto-jump on Position Change (Application options) picking
    /// a cell or a pad jumps there at once, else Jump does. Current selects where you are without
    /// jumping; the cell list is refreshed whenever its drop-down opens.
    fn jump_panel(&mut self, ui: &mut Ui) {
        ui.set_width(236.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        touch_controls(ui);
        ui.label(RichText::new("Fast Map Jump").strong());
        ui.separator();
        let half = (ui.available_width() - 6.0) / 2.0;
        let height = widgets::target(28.0, 48.0);
        let mut state = lock(&self.jump);
        let mut picked = None;
        let mut picked_pad = false;
        let mut cells_open = false;
        let auto_jump = crate::turu::bot().settings.read().auto_jump_on_position_change;
        ui.horizontal(|ui| {
            let cells = state.cells.clone();
            let selected = state.cell.clone().unwrap_or_else(|| "Select Cell...".into());
            // The cells are read again when it opens: a `dropdown` grows as they arrive.
            let list = widgets::dropdown(ui, "jump_cell", &selected, vec2(half, height), half, 320.0, |ui| {
                touch_rows(ui);
                if cells.is_empty() {
                    ui.label(RichText::new("No cells (not in a map?)").small().color(MUTED));
                }
                for cell in &cells {
                    if ui.selectable_label(state.cell.as_deref() == Some(cell), cell).clicked() {
                        picked = Some(cell.clone());
                    }
                }
            });
            cells_open = list.inner.is_some();
            let current = state.pad.clone();
            let pads = state
                .cell
                .as_deref()
                .and_then(|cell| state.map()?.pads(cell))
                .map(<[String]>::to_vec)
                .unwrap_or_else(|| PADS.map(str::to_owned).to_vec());
            let selected = if current.is_empty() { "Select Pad..." } else { &current };
            widgets::dropdown(ui, "jump_pad", selected, vec2(half, height), half, 320.0, |ui| {
                touch_rows(ui);
                for pad in pads {
                    if ui.selectable_label(current == pad, &pad).clicked() {
                        if pad != current {
                            picked_pad = true;
                        }
                        state.pad = pad;
                    }
                }
            });
        });
        if let Some(cell) = picked {
            // Turu: the map SWF says which pads the cell has, so the jump lands on one of them
            // (VibeSkua corrects the pad after jumping, in `jumpCorrectRoom`); the jump itself
            // picks the pad the walk from the player's cell arrives on (`jump_to`).
            let auto = state.map().and_then(|map| map.jump_pad(&cell, None, &state.pad));
            if let Some(pad) = auto {
                state.pad = pad;
            } else if cell == "Enter" {
                state.pad = "Spawn".into();
            } else if state.pad.is_empty() {
                state.pad = "Left".into();
            }
            state.cell = Some(cell.clone());
            if auto_jump {
                let pad = state.pad.clone();
                drop(state);
                self.jump_to(cell, pad, true);
                state = lock(&self.jump);
            }
        } else if picked_pad
            && auto_jump
            && let Some(cell) = state.cell.clone()
        {
            let pad = state.pad.clone();
            drop(state);
            self.jump_to(cell, pad, true);
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
            if widgets::button(ui, "Current", Kind::Normal, true, vec2(half, height), "").clicked() {
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
                vec2(half, height),
                "Select a cell and a pad first",
            );
            if jump.clicked()
                && let Some((cell, pad)) = target
            {
                self.jump_to(cell, pad, false);
            }
        });
        if let Some(status) = status {
            ui.label(RichText::new(status).small().color(MUTED));
        }
    }

    /// Reads the current map's cells (`map.cells`) into the drop-down, and, once per map file,
    /// each cell's pads from the map's SWF.
    fn refresh_cells(&self) {
        let state = self.jump.clone();
        crate::turu::spawn_action("Fast Map Jump cells", move |bot| {
            let cells: Vec<String> = serde_json::from_value(dispatch(bot, "map.cells", &[])?).unwrap_or_default();
            let file = turu_bot::map_items::current_file(bot).ok();
            {
                let mut state = lock(&state);
                if state.cells != cells {
                    state.cells = cells;
                }
                let Some(file) = file
                    .clone()
                    .filter(|f| state.map.as_ref().is_none_or(|(known, _)| known != f))
                else {
                    return Ok(());
                };
                state.map = Some((file, MapCells::default()));
            }
            let Some(file) = file else { return Ok(()) };
            let map = turu_bot::map_items::map_swf(bot, &file)
                .and_then(|swf| crate::map_pads::scan(&swf).map_err(turu_bot::BotError::Other))
                .map_err(|e| turu_bot::BotError::Other(format!("cells of {file}: {e}")))?;
            let mut state = lock(&state);
            if let Some((known, slot)) = state.map.as_mut()
                && *known == file
            {
                *slot = map;
            }
            Ok(())
        });
    }

    /// VibeSkua `MapService.Jump`: only when logged in, without auto-correcting the pad. With
    /// `auto` (a cell picked, not a pad), the pad is where the walk from the player's cell
    /// through the map's doors arrives, when the map SWF says.
    fn jump_to(&self, cell: String, pad: String, auto: bool) {
        let state = self.jump.clone();
        lock(&state).status = Some(format!("Jumping to {cell} ({pad})..."));
        crate::turu::spawn_action("Fast Map Jump", move |bot| {
            let status = if dispatch(bot, "player.loggedIn", &[])?.as_bool() != Some(true) {
                "Not logged in".to_owned()
            } else {
                let mut pad = pad;
                if auto {
                    let from = dispatch(bot, "player.cell", &[])?;
                    let mut state = lock(&state);
                    let walked = state.map().and_then(|map| map.jump_pad(&cell, from.as_str(), &pad));
                    if let Some(walked) = walked.filter(|walked| *walked != pad) {
                        state.pad.clone_from(&walked);
                        state.status = Some(format!("Jumping to {cell} ({walked})..."));
                        pad = walked;
                    }
                }
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

/// Bank: VibeSkua's direct action (no menu).
fn open_bank(state: &Arc<Mutex<BankState>>) {
    *lock(state) = BankState {
        busy: true,
        failed: None,
    };
    let state = state.clone();
    crate::turu::spawn_action("Bank", move |bot| {
        use turu_bot::api::player;
        let failed = if !player::logged_in(bot).unwrap_or(false) {
            Some("Not logged in".to_owned())
        } else if !player::world_loaded(bot).unwrap_or(false) {
            Some("The map is still loading".to_owned())
        } else {
            dispatch(bot, "bank.open", &[]).err().map(|e| e.to_string())
        };
        *lock(&state) = BankState {
            busy: false,
            failed: failed.map(|why| (why, Instant::now())),
        };
        Ok(())
    });
}

/// `menus` folded into one drop-down: a submenu each, then Bank as a row when it's given.
fn folded(ui: &mut Ui, windows: &mut Windows, menus: &[(Menu, &str)], bank: Option<&Arc<Mutex<BankState>>>) {
    for &(menu, label) in menus {
        ui.menu_button(label, |ui| menu_contents(ui, windows, menu));
    }
    if let Some(bank) = bank {
        ui.separator();
        if item(ui, "Bank") {
            open_bank(bank);
        }
    }
}

/// A menu's entries.
fn menu_contents(ui: &mut Ui, windows: &mut Windows, menu: Menu) {
    match menu {
        Menu::Scripts => {
            if item(ui, "Script Loader") {
                windows.script_loader = true;
            }
            if item(ui, "Search Scripts") {
                windows.script_repo = true;
            }
            // The Script Loader's reasons.
            let bot = crate::turu::bot();
            let reason = match (bot.scripts.loaded(), bot.scripts.options()) {
                (None, _) => Some("Load a script first"),
                (Some(_), ScriptOptionsState::Loaded(_)) => None,
                (Some(_), ScriptOptionsState::Failed(_)) => Some("The script's options couldn't be read"),
                (Some(_), ScriptOptionsState::NoOptions) => Some("This script declares no options"),
            };
            let mut edit = ui.add_enabled(
                reason.is_none(),
                Button::new("Edit Script Options").min_size(vec2(190.0, 0.0)),
            );
            if let Some(reason) = reason {
                edit = edit.on_disabled_hover_text(reason);
            }
            if edit.clicked() {
                windows.script_options = true;
                ui.close();
            }
            if item(ui, "Scheduler") {
                windows.scheduler = true;
            }
        }
        Menu::Options => {
            if item(ui, "Game") {
                windows.game_options = true;
            }
            if item(ui, "Application") {
                windows.application_options = true;
            }
            if item(ui, "CoreBots") {
                windows.core_bots = true;
            }
            // Not where there's no keyboard to bind (Android).
            if crate::platform::platform().has_hotkeys() && item(ui, "HotKeys") {
                windows.hotkeys = true;
            }
        }
        Menu::Tools => {
            if item(ui, "Loader") {
                windows.loader = true;
            }
            if item(ui, "Grabber") {
                windows.grabber = true;
            }
            if item(ui, "Current Drops") {
                windows.current_drops = true;
            }
            if item(ui, "Junk Items") {
                windows.junk_items = true;
            }
            if item(ui, "Loadouts Manager") {
                windows.loadouts = true;
            }
            if item(ui, "Fast Travel") {
                windows.fast_travel = true;
            }
        }
        Menu::Combat => {
            if item(ui, "Skills") {
                windows.skills = true;
            }
            if item(ui, "Stats") {
                windows.stats = true;
            }
            if item(ui, "Runtime") {
                windows.runtime = true;
            }
        }
        Menu::Diagnostics => {
            if item(ui, "Notification Center") {
                windows.notifications = true;
            }
            if item(ui, "Logs") {
                windows.logs = true;
            }
            if item(ui, "Console") {
                windows.console = true;
            }
            if item(ui, "Spammer") {
                windows.spammer = true;
            }
            if item(ui, "Packet Logger") {
                windows.packet_logger = true;
            }
            if item(ui, "Interceptor") {
                windows.interceptor = true;
            }
        }
        Menu::Plugins => {
            // Turu: VibeSkua's ButlerV5 plugin, built in. Not without several clients (Android).
            if crate::platform::platform().has_army() && item(ui, "Butler") {
                windows.butler = true;
            }
            // Turu: VibeSkua's Cosmetics plugin, built in.
            if item(ui, "Cosmetics") {
                windows.cosmetics = true;
            }
        }
        Menu::Army => {
            if item(ui, "Accounts") {
                windows.accounts = true;
            }
            if item(ui, "Army Control") {
                windows.army = true;
            }
            // VibeSkua Manager's Army Queue tab. Its squads run as processes of their own, which
            // Android can't start (`Army::launch`), so it isn't offered there.
            if crate::platform::platform().launches_processes() && item(ui, "Army Queue") {
                windows.army_queue = true;
            }
            // A client in a new tab, at the login screen (VibeSkua's tab strip "+").
            if item(ui, "New Tab") {
                super::tab_limit::open_tab(Default::default());
            }
        }
    }
}

impl JumpState {
    /// What the current map's SWF says, once read.
    fn map(&self) -> Option<&MapCells> {
        self.map.as_ref().map(|(_, map)| map)
    }
}

/// On a touch screen, the Auto and Jump popups' drop-downs and text boxes are 48 points high,
/// as their buttons are (`widgets::target`).
fn touch_controls(ui: &mut Ui) {
    if widgets::touch() {
        ui.spacing_mut().interact_size.y = 48.0;
    }
}

/// The inventory's Class items, sorted ignoring case, into `owned`.
fn read_classes(bot: &Arc<turu_bot::Bot>, owned: &Mutex<Vec<String>>) -> turu_bot::BotResult<()> {
    let items: Vec<serde_json::Value> =
        serde_json::from_value(dispatch(bot, "inventory.items", &[])?).unwrap_or_default();
    let mut classes: Vec<String> = items
        .iter()
        .filter(|item| {
            item.get("category")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|c| c.eq_ignore_ascii_case("Class"))
        })
        .filter_map(|item| item.get("name").and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .collect();
    classes.sort_by_key(|name| name.to_lowercase());
    *lock(owned) = classes;
    Ok(())
}

/// The Auto popup's content width, and the room between its controls.
const AUTO_PANEL_WIDTH: f32 = 256.0;
const AUTO_GAP: f32 = 6.0;

/// On a touch screen, a drop-down's rows are about 44 points high, as the menus' are.
fn touch_rows(ui: &mut Ui) {
    if widgets::touch() {
        ui.spacing_mut().button_padding.y = 14.0;
    }
}

/// A nav-bar popup at `button` (under it on the bar, beside it on the rail), open while `open`
/// is true. Clicks are handled by [`close_on_click_outside`]: egui's own "close on click
/// outside" also fires when an item of one of its drop-downs is picked (those are separate
/// layers).
fn nav_popup<'a>(button: &Response, open: &'a mut bool, width: f32, align: RectAlign) -> Popup<'a> {
    Popup::from_response(button)
        .open_bool(open)
        .close_behavior(PopupCloseBehavior::IgnoreClicks)
        .align(align)
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

/// A newer Turu (`turu_bot::updates`): a quiet item at the left of the quick actions that opens
/// the update window. Never a modal: a script may be running. Shows how an install goes: a
/// spinner while it downloads, a green dot once it waits for Turu to quit.
fn update_notice(ui: &mut Ui, windows: &mut Windows, compact: bool) {
    let state = turu_bot::updates::state();
    let Some(release) = &state.available else {
        return;
    };
    let lead = update_lead(&state.install);
    let label = if compact {
        "Update".to_owned()
    } else {
        format!("Update {}", release.version)
    };
    let response = toggle_button(ui, &label, lead, windows.client_update).on_hover_text(format!(
        "Update available: {} {} (you have {})",
        turu_bot::updates::app_name(),
        release.version,
        crate::turu::VERSION
    ));
    if response.clicked() {
        windows.client_update = !windows.client_update;
    }
    if matches!(state.install, turu_bot::updates::Install::Busy(_)) {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

/// Grid View (VibeSkua's tabbed host's): every tab at once, with two tabs or more, where the
/// platform offers it (not on a phone).
fn grid_toggle(ui: &mut Ui) {
    if !crate::platform::platform().has_grid_view() || crate::turu::tabs().len() < 2 {
        return;
    }
    let on = crate::grid::on();
    let response = toggle_button(ui, "Grid", Lead::None, on).on_hover_text(if on {
        "Back to one tab at a time"
    } else {
        "Show every tab at once; click a tile to make it the active tab"
    });
    if response.clicked() {
        crate::turu::request_tab(crate::turu::TabRequest::Grid(!on));
    }
}

/// The active tab's script at a glance, at the bar's right end: one capsule with the chip
/// (status dot, name, how long it has run or how its last run ended; opens the Script Loader)
/// and, joined to it, what a check-in reaches for: Start; Pause or Resume and Stop while it
/// runs; Force Stop while it stops. Returns the chip, and how wide its name was drawn.
fn script_status(ui: &mut Ui, name_width: f32) -> (Response, f32) {
    let bot = crate::turu::bot();
    let status = bot.scripts.status();
    let cells = run_cells(&bot, status);
    ui.scope(|ui| {
        // A 1 point gap of the bar's colour parts the capsule's cells.
        ui.spacing_mut().item_spacing.x = 1.0;
        let radius = CornerRadius::same(SMALL_RADIUS);
        for (i, cell) in cells.iter().enumerate().rev() {
            let mut corners = CornerRadius::ZERO;
            if i == cells.len() - 1 {
                corners.ne = radius.ne;
                corners.se = radius.se;
            }
            let size = control_height();
            if run_cell(ui, *cell, vec2(size, size), corners).clicked() {
                cell.run(&bot);
            }
        }
        let corners = if cells.is_empty() {
            radius
        } else {
            CornerRadius {
                nw: radius.nw,
                sw: radius.sw,
                ..CornerRadius::ZERO
            }
        };
        script_chip(ui, &bot, status, name_width, corners)
    })
    .inner
}

/// A run control next to the script chip.
#[derive(Clone, Copy)]
enum RunCell {
    /// `Some(why)` when it can't start now.
    Start(Option<&'static str>),
    Pause,
    Resume,
    Stop,
    /// Offered once the stop has taken [`turu_bot::script::FORCE_STOP_AFTER`].
    ForceStop(bool),
}

/// The Script Loader's run controls for `status`, left to right (none without a script).
fn run_cells(bot: &turu_bot::Bot, status: ScriptStatus) -> Vec<RunCell> {
    match status {
        ScriptStatus::Empty => Vec::new(),
        ScriptStatus::Loaded | ScriptStatus::Error => {
            let why = if auto::running(bot) {
                Some("Stop Auto Attack/Hunt first")
            } else if bot.scripts.is_snippet_running() {
                Some("Stop the Console snippet first")
            } else {
                None
            };
            vec![RunCell::Start(why)]
        }
        ScriptStatus::Compiling => vec![RunCell::Stop],
        ScriptStatus::Running => vec![RunCell::Pause, RunCell::Stop],
        ScriptStatus::Paused => vec![RunCell::Resume, RunCell::Stop],
        ScriptStatus::Stopping => vec![RunCell::ForceStop(bot.scripts.can_force_stop())],
    }
}

impl RunCell {
    fn enabled(self) -> bool {
        match self {
            Self::Start(why) => why.is_none(),
            Self::ForceStop(ready) => ready,
            _ => true,
        }
    }

    fn tip(self) -> String {
        let key = super::hotkeys::hint("ToggleScript");
        match self {
            Self::Start(Some(why)) => why.to_owned(),
            Self::Start(None) => format!("Start the script{key}"),
            Self::Pause => "Pause the script".to_owned(),
            Self::Resume => "Resume the script".to_owned(),
            Self::Stop => format!("Stop the script{key}"),
            Self::ForceStop(true) => {
                format!("Force Stop: interrupts the script's stopping steps{key}")
            }
            Self::ForceStop(false) => script_loader::FORCE_STOP_WAIT.to_owned(),
        }
    }

    /// What the Script Loader's button of the same name does.
    fn run(self, bot: &Arc<turu_bot::Bot>) {
        match self {
            Self::Start(_) => {
                if let Err(e) = script::start(bot) {
                    bot.log(format!("Couldn't start the script: {e}"));
                }
            }
            Self::Pause => script::set_paused(bot, true),
            Self::Resume => script::set_paused(bot, false),
            Self::Stop | Self::ForceStop(_) => script::stop(bot),
        }
    }
}

/// A square run control: Start and Resume in the accent (what you came back for), Pause plain,
/// Stop with a red square that fills red on hover, Force Stop red once it's offered.
fn run_cell(ui: &mut Ui, cell: RunCell, size: egui::Vec2, corners: CornerRadius) -> Response {
    let enabled = cell.enabled();
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let hovered = enabled && response.hovered();
    let (fill, icon) = match cell {
        _ if !enabled => (TRACK, MUTED.gamma_multiply(0.6)),
        RunCell::Start(_) | RunCell::Resume => (if hovered { ACCENT_HOVER } else { ACCENT }, ON_ACCENT),
        RunCell::Pause => (
            if hovered { HOVER } else { TRACK },
            if hovered { Color32::WHITE } else { TEXT },
        ),
        RunCell::Stop => {
            if hovered {
                (DANGER, Color32::WHITE)
            } else {
                (TRACK, DANGER)
            }
        }
        RunCell::ForceStop(_) => (if hovered { DANGER_HOVER } else { DANGER }, Color32::WHITE),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, corners, fill);
    let c = rect.center();
    let s = if widgets::touch() { 1.3 } else { 1.0 };
    match cell {
        RunCell::Start(_) | RunCell::Resume => {
            // Optically centred: a triangle's weight sits left of its box's centre.
            let points = vec![
                pos2(c.x - 3.0 * s, c.y - 4.5 * s),
                pos2(c.x + 4.5 * s, c.y),
                pos2(c.x - 3.0 * s, c.y + 4.5 * s),
            ];
            painter.add(egui::Shape::convex_polygon(points, icon, Stroke::NONE));
        }
        RunCell::Pause => {
            for x in [-3.25, 0.75] {
                let bar = Rect::from_min_size(pos2(c.x + x * s, c.y - 4.5 * s), vec2(2.5 * s, 9.0 * s));
                painter.rect_filled(bar, 0.5, icon);
            }
        }
        RunCell::Stop | RunCell::ForceStop(_) => {
            painter.rect_filled(Rect::from_center_size(c, vec2(8.0 * s, 8.0 * s)), 1.5, icon);
        }
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.on_hover_text(cell.tip())
}

/// The chip: its status dot ("!" while the script waits for you), its name and how long it
/// has run (yellow while paused), or its state when it isn't running (an error red). Wording
/// and clock are the Script Loader's status card's. Without [`MIN_NAME_WIDTH`] of room the name
/// is left out (it stays in the tooltip). Returns the chip and how wide its name was drawn.
fn script_chip(
    ui: &mut Ui,
    bot: &turu_bot::Bot,
    status: ScriptStatus,
    name_width: f32,
    corners: CornerRadius,
) -> (Response, f32) {
    let waits = bot.scripts.waits_for();
    let name = bot
        .scripts
        .loaded()
        .and_then(|path| path.file_stem().map(|n| n.to_string_lossy().into_owned()));
    let (state, state_color) = chip_state(bot, status);
    if status.is_active() {
        // The clock moves while nothing else redraws the bar.
        ui.ctx().request_repaint_after(Duration::from_secs(1));
    }

    let font = FontId::proportional(12.0);
    let name_galley = name.as_ref().filter(|_| name_width >= MIN_NAME_WIDTH).map(|name| {
        let mut job = LayoutJob::simple_singleline(name.clone(), font.clone(), TEXT);
        job.wrap = TextWrapping::truncate_at_width(name_width);
        ui.painter().layout_job(job)
    });
    let state_galley = ui.painter().layout_no_wrap(state.clone(), font.clone(), state_color);
    let sep_galley = ui.painter().layout_no_wrap("·".to_owned(), font, MUTED);
    let name_w = name_galley.as_ref().map_or(0.0, |g| g.size().x);
    let sep_w = if name_galley.is_some() {
        sep_galley.size().x + 10.0
    } else {
        0.0
    };
    let drawn = name_w + sep_w;
    // The clock's room is kept from "59m" on, so the bar doesn't shift each time it ticks.
    let clock = if status.is_active() {
        ui.painter()
            .layout_no_wrap("59m".to_owned(), FontId::proportional(12.0), TEXT)
            .size()
            .x
    } else {
        0.0
    };
    let width = 24.0 + name_w + sep_w + state_galley.size().x.max(clock) + 10.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, control_height()), Sense::click());
    let fill = if response.hovered() { HOVER } else { TRACK };
    let painter = ui.painter();
    painter.rect_filled(rect, corners, fill);
    let dot = pos2(rect.left() + 12.0, rect.center().y);
    if waits.is_some() {
        painter.circle_filled(dot, 6.0, WARNING);
        painter.text(
            dot,
            Align2::CENTER_CENTER,
            "!",
            FontId::proportional(10.0),
            Color32::BLACK,
        );
    } else {
        painter.circle_filled(dot, 3.5, widgets::status_color(status));
    }
    let mut x = rect.left() + 24.0;
    let place = |galley: Arc<egui::Galley>, x: &mut f32, color: Color32| {
        let y = rect.center().y - galley.size().y / 2.0;
        let w = galley.size().x;
        painter.galley(pos2(*x, y), galley, color);
        *x += w;
    };
    if let Some(galley) = name_galley {
        place(galley, &mut x, TEXT);
        x += 5.0;
        place(sep_galley, &mut x, MUTED);
        x += 5.0;
    }
    place(state_galley, &mut x, state_color);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut tip = match script_loader::last_run(bot) {
        Some(run) => format!("{} after {}, at {}", run.how, run.ran, run.at),
        None => script_loader::state_text(status).to_owned(),
    };
    if let Some(name) = &name {
        tip = format!("{name}: {tip}");
    }
    if let Some(what) = waits {
        tip.push_str(&format!(" (waits for you: {what})"));
    }
    (response.on_hover_text(format!("{tip}\nOpen the Script Loader")), drawn)
}

/// The script chip's state and its colour: how long the script has run (yellow while paused),
/// how its last run ended, or the Script Loader's word for it (an error red).
fn chip_state(bot: &turu_bot::Bot, status: ScriptStatus) -> (String, Color32) {
    let elapsed = status
        .is_active()
        .then(|| bot.state.lock().stats.started)
        .flatten()
        .map(|started| started.elapsed());
    let state = match status {
        ScriptStatus::Running => elapsed.map_or_else(|| "Running".to_owned(), script_loader::duration_text),
        // Paused, the clock alone, yellow (as its dot), beside Resume: the chip keeps its width,
        // so pausing doesn't move the buttons left of it under a finger. The tooltip says so.
        ScriptStatus::Paused => elapsed.map_or_else(|| "Paused".to_owned(), script_loader::duration_text),
        ScriptStatus::Error => "Error".to_owned(),
        // After a run: "Finished 6h 12m", "Stopped 3h 40m" (the Script Loader's last run).
        ScriptStatus::Loaded => match script_loader::last_run(bot) {
            Some(run) => format!("{} {}", run.how, run.ran),
            None => script_loader::state_text(status).to_owned(),
        },
        other => script_loader::state_text(other).to_owned(),
    };
    let state_color = match status {
        ScriptStatus::Running => TEXT,
        ScriptStatus::Paused | ScriptStatus::Stopping | ScriptStatus::Compiling => WARNING,
        ScriptStatus::Error => DANGER,
        _ => MUTED,
    };
    (state, state_color)
}

/// The widest the script chip's name gets; longer names end in "…".
const CHIP_NAME_WIDTH: f32 = 150.0;
/// The chip's name with the menus folded all the way.
const COMPACT_CHIP_NAME_WIDTH: f32 = 72.0;
/// Less room than this, and the chip leaves the name out.
const MIN_NAME_WIDTH: f32 = 28.0;

/// The Turu symbol (`docs/logo/turu-symbol-small.svg`, the cut for small sizes): a tile in the
/// accent (violet; gold in Turu Mas) with two closed eyes (`ON_ACCENT`), drawn rather than
/// loaded so it stays sharp at any scale.
const SYMBOL_SIZE: f32 = 18.0;
/// Between the symbol and the first menu.
const SYMBOL_GAP: f32 = 6.0;

fn symbol(ui: &mut Ui, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let painter = ui.painter();
    // The SVG's 256 grid: the tile spans 16..240, its corners round by about a quarter.
    let unit = size / 224.0;
    painter.rect_filled(rect, size * 5.0 / 18.0, ACCENT);
    // Each eye is a crescent (the bottom half of a ring) centred at x 80 and 176, y 112; the
    // ring runs from radius 18 to 46, so it is drawn as a stroke at radius 32, width 28.
    for cx in [80.0, 176.0] {
        let center = pos2(rect.left() + (cx - 16.0) * unit, rect.top() + (112.0 - 16.0) * unit);
        let radius = 32.0 * unit;
        let width = 26.0 * unit;
        let points: Vec<_> = (0..=16)
            .map(|i| {
                let a = std::f32::consts::PI * i as f32 / 16.0;
                pos2(center.x + radius * a.cos(), center.y + radius * a.sin())
            })
            .collect();
        // Round ends, as the SVG's 14-radius caps.
        painter.circle_filled(points[0], width / 2.0, ON_ACCENT);
        painter.circle_filled(points[16], width / 2.0, ON_ACCENT);
        painter.add(egui::Shape::line(points, Stroke::new(width, ON_ACCENT)));
    }
    response
}

/// What leads a quick action's label.
#[derive(Clone, Copy)]
enum Lead {
    None,
    /// A status dot (Auto running).
    Dot(Color32),
    /// Its action is running (Bank).
    Spinner,
}

/// A flat quick-action button ("Grid", "Auto", "Jump", "Bank"), tinted while its popup is
/// `open` as an open menu is.
fn toggle_button(ui: &mut Ui, label: &str, lead: Lead, open: bool) -> Response {
    let font = FontId::proportional(13.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, TEXT);
    let led = !matches!(lead, Lead::None);
    let width = galley.size().x + 20.0 + if led { 13.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, control_height()), Sense::click());
    let radius = CornerRadius::same(SMALL_RADIUS);
    if open {
        ui.painter().rect_filled(rect, radius, OPEN);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, radius, HOVER);
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut x = rect.left() + 10.0;
    match lead {
        Lead::None => {}
        Lead::Dot(color) => {
            ui.painter().circle_filled(pos2(x + 3.5, rect.center().y), 3.5, color);
        }
        Lead::Spinner => {
            let spin = Rect::from_center_size(pos2(x + 4.5, rect.center().y), vec2(10.0, 10.0));
            Spinner::new().size(10.0).color(TEXT).paint_at(ui, spin);
        }
    }
    if led {
        x += 13.0;
    }
    let color = if open || response.hovered() {
        Color32::WHITE
    } else {
        TEXT
    };
    ui.painter()
        .galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
    response
}

// The rail: the nav bar on a touch screen (`crate::gui::nav_rail`), down the left side so the
// game keeps the screen's whole height. Top: the symbol, Menu (every menu), Auto, Jump, Bank and
// Grid as icon-over-label items. Bottom: the script's status and its run controls, stacked into
// one column. Popups and menus open beside it, over the game.

/// The rail's content width: the rail less 4 points each side.
const RAIL_INNER: f32 = 56.0;
/// A rail item: its indicator (an icon in a pill), its label and room below it.
const RAIL_ITEM_HEIGHT: f32 = 52.0;
/// An item's indicator pill, Material's 56 x 32 narrowed to the rail.
const RAIL_PILL: egui::Vec2 = egui::Vec2::new(48.0, 30.0);
/// The script chip on the rail: its status dot over its state.
const RAIL_CHIP_HEIGHT: f32 = 48.0;
/// A run control under the chip, as tall as the bar's on a touch screen.
const RAIL_CELL_HEIGHT: f32 = 44.0;
/// Above the first item and below the last run control.
const RAIL_PAD: f32 = 8.0;
/// The symbol's row at the top.
const RAIL_SYMBOL_ROW: f32 = 36.0;
/// The symbol on the rail.
const RAIL_SYMBOL_SIZE: f32 = 22.0;
/// The least room between the items and the status.
const RAIL_GAP: f32 = 12.0;

/// What has a place of its own on the rail. A short screen folds the least needed first: the
/// symbol, then Update and Bank, which move into Menu as on the folded bar.
#[derive(Clone, Copy)]
struct RailFit {
    symbol: bool,
    update: bool,
    bank: bool,
}

/// The most run controls the chip has under it (Pause or Resume, and Stop).
const MOST_RUN_CELLS: f32 = 2.0;

/// What fits in `height` points with an update `update` and Grid `grid`. It always counts the
/// most run controls, so starting or stopping the script never moves the items under a finger.
fn rail_fit(height: f32, update: bool, grid: bool) -> RailFit {
    let status = RAIL_CHIP_HEIGHT + MOST_RUN_CELLS * (RAIL_CELL_HEIGHT + 1.0);
    let items = 3.0 + if grid { 1.0 } else { 0.0 };
    let mut room = height - status - RAIL_GAP - items * RAIL_ITEM_HEIGHT;
    let mut take = |want: bool, needs: f32| {
        let fits = want && room >= needs;
        if fits {
            room -= needs;
        }
        fits
    };
    // Most needed first: what's left over goes to the next.
    let bank = take(true, RAIL_ITEM_HEIGHT);
    let update = take(update, RAIL_ITEM_HEIGHT);
    let symbol = take(true, RAIL_SYMBOL_ROW);
    RailFit { symbol, update, bank }
}

/// Hides or shows the rail and keeps the choice in `settings.json` (`side_bar_hidden`), so it
/// holds when Turu opens again.
fn set_rail_hidden(hidden: bool) {
    crate::gui::set_rail_hidden(hidden);
    let bot = crate::turu::bot();
    if bot.settings.read().side_bar_hidden != hidden {
        bot.settings.write().side_bar_hidden = hidden;
        bot.save_settings();
    }
}

/// While the rail is hidden: a small tab on the screen's left edge, halfway down, that brings it
/// back. It floats over the game, which has the whole width then.
fn rail_handle(ui: &mut Ui, inset: f32) {
    let screen = ui.ctx().content_rect();
    let size = vec2(24.0 + inset, 64.0);
    let pos = pos2(screen.left(), screen.center().y - size.y / 2.0);
    egui::Area::new(egui::Id::new("turu_rail_handle"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            let fill = if response.hovered() || response.is_pointer_button_down_on() {
                ACCENT
            } else {
                PANEL.gamma_multiply(0.85)
            };
            let corners = CornerRadius {
                ne: 10,
                se: 10,
                ..CornerRadius::ZERO
            };
            ui.painter().rect_filled(rect, corners, fill);
            ui.painter().text(
                pos2(rect.left() + inset + 12.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                "›",
                egui::FontId::proportional(22.0),
                TEXT,
            );
            if response.on_hover_text("Show the side bar").clicked() {
                set_rail_hidden(false);
            }
        });
}

/// The icons a rail item draws, in one 1.6-point stroke.
#[derive(Clone, Copy)]
enum RailIcon {
    Menu,
    /// A crosshair: combat automation.
    Auto,
    /// A map pin: to a cell of this map.
    Jump,
    /// A chest.
    Bank,
    Grid,
    /// An arrow into a tray.
    Update,
}

impl NavBar {
    fn show_rail(&mut self, ui: &mut Ui, windows: &mut Windows) {
        let inset = crate::gui::rail_inset();
        // The first time: hidden as it was when Turu last closed.
        static RESTORED: std::sync::Once = std::sync::Once::new();
        RESTORED.call_once(|| crate::gui::set_rail_hidden(crate::turu::bot().settings.read().side_bar_hidden));
        if crate::gui::rail_hidden() {
            rail_handle(ui, inset);
            return;
        }
        Panel::left("turu_rail")
            .exact_size(crate::gui::left_width() as f32)
            .resizable(false)
            .frame(Frame::new().fill(PANEL))
            .show_separator_line(true)
            .show(ui, |ui| {
                // Clear of a cutout on this side, 4 points in from each edge.
                let full = ui.max_rect();
                let inner = Rect::from_min_max(
                    pos2(full.left() + inset + 4.0, full.top() + RAIL_PAD),
                    pos2(full.right() - 4.0, full.bottom() - RAIL_PAD),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(Align::Center)),
                    |ui| self.rail(ui, windows),
                );
            });
    }

    fn rail(&mut self, ui: &mut Ui, windows: &mut Windows) {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let bot = crate::turu::bot();
        let status = bot.scripts.status();
        let cells = run_cells(&bot, status);
        let update = turu_bot::updates::state();
        // As the bar's button: phones have no Grid View (`Platform::has_grid_view`).
        let grid = crate::platform::platform().has_grid_view() && crate::turu::tabs().len() > 1;
        let fit = rail_fit(ui.available_height(), update.available.is_some(), grid);

        if fit.symbol {
            ui.add_space((RAIL_SYMBOL_ROW - RAIL_SYMBOL_SIZE) / 2.0 - 2.0);
            // A tap on the logo hides the rail (the game gets the whole width).
            if symbol(ui, RAIL_SYMBOL_SIZE)
                .on_hover_text("Hide the side bar")
                .clicked()
            {
                set_rail_hidden(true);
            }
            ui.add_space((RAIL_SYMBOL_ROW - RAIL_SYMBOL_SIZE) / 2.0 + 2.0);
        }

        // Menu: every menu, then what didn't get a place of its own. Its badge says an update
        // waits in it.
        let menu_lead = match (&update.available, fit.update) {
            (Some(_), false) => update_lead(&update.install),
            _ => Lead::None,
        };
        let menu = rail_item(ui, RailIcon::Menu, "Menu", menu_lead, |response| {
            Popup::is_id_open(&response.ctx, Popup::default_response_id(response))
        });
        let folded_bank = (!fit.bank).then_some(&self.bank);
        let config = MenuConfig::new().style(menu_style);
        Popup::menu(&menu)
            .align(RectAlign::RIGHT_START)
            .style(menu_style)
            .info(egui::UiStackInfo::new(egui::UiKind::Menu).with_tag_value(MenuConfig::MENU_CONFIG_TAG, config))
            .show(|ui| {
                // Taller than a phone held sideways: it scrolls.
                let room = ui.ctx().content_rect().height() - 24.0;
                egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                    if let (Some(release), false) = (&update.available, fit.update)
                        && item(ui, &format!("Update to {}", release.version))
                    {
                        windows.client_update = true;
                    }
                    folded(ui, windows, menus(), folded_bank);
                    ui.separator();
                    if item(ui, "Hide Side Bar") {
                        set_rail_hidden(true);
                    }
                });
            });

        let running = auto::running(&bot);
        let lead = if running { Lead::Dot(SUCCESS) } else { Lead::None };
        let auto_open = self.auto_open;
        let auto = rail_item(ui, RailIcon::Auto, "Auto", lead, |_| auto_open);
        self.auto_popup(ui, &auto, RectAlign::RIGHT_START);

        let jump_open = self.jump_open;
        let jump = rail_item(ui, RailIcon::Jump, "Jump", Lead::None, |_| jump_open);
        self.jump_popup(ui, &jump, RectAlign::RIGHT_START);

        let bank = fit.bank.then(|| {
            let lead = if lock(&self.bank).busy {
                Lead::Spinner
            } else {
                Lead::None
            };
            let response = rail_item(ui, RailIcon::Bank, "Bank", lead, |_| false);
            if response.clicked() {
                open_bank(&self.bank);
            }
            response
        });
        self.bank_note(ui, Some(bank.as_ref().unwrap_or(&menu)), RectAlign::RIGHT_START);

        if grid {
            let on = crate::grid::on();
            if rail_item(ui, RailIcon::Grid, "Grid", Lead::None, |_| on).clicked() {
                crate::turu::request_tab(crate::turu::TabRequest::Grid(!on));
            }
        }

        if let (Some(_), true) = (&update.available, fit.update) {
            let shown = windows.client_update;
            if rail_item(ui, RailIcon::Update, "Update", update_lead(&update.install), |_| shown).clicked() {
                windows.client_update = !windows.client_update;
            }
        }
        if matches!(update.install, turu_bot::updates::Install::Busy(_)) {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        // The status at the bottom, drawn upwards: the run controls, then the chip.
        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            let radius = SMALL_RADIUS;
            for (i, cell) in cells.iter().enumerate().rev() {
                let mut corners = CornerRadius::ZERO;
                if i == cells.len() - 1 {
                    corners.sw = radius;
                    corners.se = radius;
                }
                if run_cell(ui, *cell, vec2(RAIL_INNER, RAIL_CELL_HEIGHT), corners).clicked() {
                    cell.run(&bot);
                }
            }
            let mut corners = CornerRadius {
                nw: radius,
                ne: radius,
                ..CornerRadius::ZERO
            };
            if cells.is_empty() {
                corners = CornerRadius::same(radius);
            }
            let chip = rail_chip(ui, &bot, status, corners);
            // The scripts made for this map: a badge with how many while none runs.
            let map = bot.state.lock().map_name.clone().filter(|map| !map.is_empty());
            let here = map
                .as_deref()
                .and_then(|map| self.map_scripts.get(&bot, map).map(|list| (map, list)));
            if let Some((_, list)) = here
                && !status.is_active()
            {
                map_badge(ui, chip.rect, list.len());
            }
            // With scripts for this map, favourite or recent ones, the chip lists them to start
            // one with a tap (and the Script Loader); without, it opens the Script Loader.
            let quick = turu_bot::quick_scripts::get(&bot.paths.quick_scripts_file());
            if quick.is_empty() && here.is_none() {
                if chip.clicked() {
                    windows.script_loader = true;
                }
            } else {
                let config = MenuConfig::new().style(menu_style);
                Popup::menu(&chip)
                    .align(RectAlign::RIGHT_END)
                    .style(menu_style)
                    .info(
                        egui::UiStackInfo::new(egui::UiKind::Menu).with_tag_value(MenuConfig::MENU_CONFIG_TAG, config),
                    )
                    .show(|ui| {
                        // Taller than a phone held sideways: it scrolls.
                        let room = ui.ctx().content_rect().height() - 24.0;
                        egui::ScrollArea::vertical()
                            .id_salt("turu_quick_scripts")
                            .max_height(room)
                            .show(ui, |ui| quick_scripts_menu(ui, &bot, here, &quick, windows));
                    });
            }
        });
    }
}

/// How many scripts are made for the map, in an accent bubble on the chip's top right corner.
fn map_badge(ui: &Ui, chip: Rect, count: usize) {
    let text = if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    };
    let galley = ui.painter().layout_no_wrap(text, FontId::proportional(10.0), ON_ACCENT);
    let size = vec2((galley.size().x + 8.0).max(16.0), 16.0);
    let rect = Rect::from_center_size(pos2(chip.right() - size.x / 2.0 - 2.0, chip.top() + 10.0), size);
    ui.painter().rect_filled(rect, size.y / 2.0, ACCENT);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, ON_ACCENT);
}

/// The rail chip's list: the Script Loader, the scripts made for the map the tab is on
/// (`turu_bot::map_scripts`), then the favourite and the recently started scripts
/// (`turu_bot::quick_scripts`); a tap loads one and starts it (a map's script not downloaded yet
/// is downloaded first). Scripts no longer on disk are left out.
fn quick_scripts_menu(
    ui: &mut Ui,
    bot: &Arc<turu_bot::Bot>,
    here: Option<(&str, &[MapScript])>,
    quick: &turu_bot::quick_scripts::QuickScripts,
    windows: &mut Windows,
) {
    if item(ui, "Script Loader") {
        windows.script_loader = true;
    }
    // Script Loader's rules for Start.
    let why = if bot.scripts.status().is_active() {
        Some("Stop the script first")
    } else if auto::running(bot) || bot.scripts.is_snippet_running() {
        Some("Stop Auto or the Console snippet first")
    } else {
        None
    };
    if let Some((map, list)) = here {
        ui.separator();
        ui.label(
            RichText::new(format!("On {map} · {}", list.len()))
                .size(11.0)
                .color(MUTED),
        );
        egui::ScrollArea::vertical()
            .id_salt("turu_map_scripts")
            .max_height(5.5 * (control_height() + 4.0))
            .show(ui, |ui| {
                for script in list {
                    let (text, tip) = if script.downloaded {
                        (script.name.clone(), script.path.clone())
                    } else {
                        (
                            format!("{} ↓", script.name),
                            format!("{} (downloaded when started)", script.path),
                        )
                    };
                    let button = Button::new(text).min_size(vec2(220.0, control_height()));
                    if ui.add_enabled(why.is_none(), button).on_hover_text(tip).clicked() {
                        ui.close();
                        if script.downloaded {
                            start_quick_script(bot, &bot.paths.scripts_dir().join(&script.path));
                        } else {
                            download_and_start(bot, script.path.clone());
                        }
                    }
                }
            });
    }
    let lists = [
        ("Favourites", quick.favorites.iter().collect::<Vec<_>>()),
        ("Recent", quick.recent_only().collect()),
    ];
    for (title, list) in lists {
        let list: Vec<_> = list.into_iter().filter(|path| path.is_file()).collect();
        if list.is_empty() {
            continue;
        }
        ui.separator();
        ui.label(RichText::new(title).size(11.0).color(MUTED));
        for path in list {
            let name = path
                .file_stem()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let button = Button::new(name).min_size(vec2(190.0, control_height()));
            let response = ui
                .add_enabled(why.is_none(), button)
                .on_hover_text(path.display().to_string());
            if response.clicked() {
                ui.close();
                start_quick_script(bot, path);
            }
        }
    }
    if let Some(why) = why {
        ui.label(RichText::new(why).size(11.0).color(MUTED));
    }
}

/// Downloads `path` (relative to the scripts folder) from the scripts repository, then loads it and
/// starts it, off the UI thread.
fn download_and_start(bot: &Arc<turu_bot::Bot>, path: String) {
    bot.log(format!("Downloading {path}"));
    let bot = bot.clone();
    let spawned = std::thread::Builder::new()
        .name("Turu Map Script".into())
        .spawn(move || match crate::turu::script_repo().ensure(&path) {
            Ok(local) => start_quick_script(&bot, &local),
            Err(e) => bot.log(format!("Couldn't download {path}: {e}")),
        });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start the download: {e}");
    }
}

/// Loads `path` and starts it, as Load Script then Start Script would.
fn start_quick_script(bot: &Arc<turu_bot::Bot>, path: &std::path::Path) {
    bot.scripts.load(path);
    bot.log(format!("Loaded script {}", path.display()));
    if let Err(e) = script::start(bot) {
        bot.log(format!("Couldn't start the script: {e}"));
    }
}

/// What leads Update: how its install goes.
fn update_lead(install: &turu_bot::updates::Install) -> Lead {
    match install {
        turu_bot::updates::Install::Busy(_) => Lead::Spinner,
        turu_bot::updates::Install::OnQuit(_) => Lead::Dot(SUCCESS),
        turu_bot::updates::Install::Failed(_) => Lead::Dot(DANGER),
        turu_bot::updates::Install::Idle => Lead::Dot(ACCENT),
    }
}

/// A rail item: `icon` in a pill over `label`, the pill tinted while what it opens is `open`
/// (asked once the item has its response) and while a finger is on it. A [`Lead::Dot`] is a
/// badge on the icon; a [`Lead::Spinner`] takes the icon's place.
fn rail_item(ui: &mut Ui, icon: RailIcon, label: &str, lead: Lead, open: impl FnOnce(&Response) -> bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(RAIL_INNER, RAIL_ITEM_HEIGHT), Sense::click());
    let open = open(&response);
    let pressed = response.is_pointer_button_down_on() || response.hovered();
    let painter = ui.painter();
    let pill = Rect::from_center_size(pos2(rect.center().x, rect.top() + 2.0 + RAIL_PILL.y / 2.0), RAIL_PILL);
    if open {
        painter.rect_filled(pill, RAIL_PILL.y / 2.0, OPEN);
    } else if pressed {
        painter.rect_filled(pill, RAIL_PILL.y / 2.0, HOVER);
    }
    let ink = if open { Color32::WHITE } else { TEXT };
    match lead {
        Lead::Spinner => {
            let spin = Rect::from_center_size(pill.center(), vec2(16.0, 16.0));
            Spinner::new().size(16.0).color(ink).paint_at(ui, spin);
        }
        Lead::None | Lead::Dot(_) => rail_icon(painter, icon, pill.center(), ink),
    }
    if let Lead::Dot(color) = lead {
        let at = pill.center() + vec2(10.0, -8.0);
        painter.circle_filled(at, 5.5, if open { OPEN } else { PANEL });
        painter.circle_filled(at, 4.0, color);
    }
    painter.text(
        pos2(rect.center().x, pill.bottom() + 3.0),
        Align2::CENTER_TOP,
        label,
        FontId::proportional(11.0),
        ink,
    );
    response
}

/// Draws `icon` centred on `c`, about 18 points across.
fn rail_icon(painter: &egui::Painter, icon: RailIcon, c: egui::Pos2, ink: Color32) {
    let stroke = Stroke::new(1.6, ink);
    let line = |a: egui::Vec2, b: egui::Vec2| painter.line_segment([c + a, c + b], stroke);
    match icon {
        RailIcon::Menu => {
            for y in [-5.0, 0.0, 5.0] {
                line(vec2(-8.0, y), vec2(8.0, y));
            }
        }
        RailIcon::Auto => {
            painter.circle_stroke(c, 6.0, stroke);
            line(vec2(0.0, -9.0), vec2(0.0, -3.5));
            line(vec2(0.0, 3.5), vec2(0.0, 9.0));
            line(vec2(-9.0, 0.0), vec2(-3.5, 0.0));
            line(vec2(3.5, 0.0), vec2(9.0, 0.0));
            painter.circle_filled(c, 1.3, ink);
        }
        RailIcon::Jump => {
            // A teardrop: the head's arc over the top, then down its two tangents to the point.
            let head = c + vec2(0.0, -2.5);
            let r = 5.5;
            let point = c + vec2(0.0, 8.5);
            let beta = (r / (point.y - head.y)).acos();
            let start = std::f32::consts::FRAC_PI_2 + beta;
            let sweep = std::f32::consts::TAU - 2.0 * beta;
            let mut points: Vec<_> = (0..=20)
                .map(|i| {
                    let a = start + sweep * i as f32 / 20.0;
                    head + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            points.push(point);
            painter.add(egui::Shape::closed_line(points, stroke));
            painter.circle_filled(head, 1.8, ink);
        }
        RailIcon::Bank => {
            let chest = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(17.0, 13.0));
            painter.rect_stroke(chest, 2.5, stroke, egui::StrokeKind::Middle);
            let lid = chest.top() + 4.5;
            painter.line_segment([pos2(chest.left(), lid), pos2(chest.right(), lid)], stroke);
            painter.rect_filled(Rect::from_center_size(pos2(c.x, lid + 1.0), vec2(3.5, 5.0)), 1.0, ink);
        }
        RailIcon::Grid => {
            for (x, y) in [(-4.0, -4.0), (4.0, -4.0), (-4.0, 4.0), (4.0, 4.0)] {
                let tile = Rect::from_center_size(c + vec2(x, y), vec2(6.0, 6.0));
                painter.rect_stroke(tile, 1.5, stroke, egui::StrokeKind::Middle);
            }
        }
        RailIcon::Update => {
            line(vec2(0.0, -8.0), vec2(0.0, 2.0));
            line(vec2(-4.0, -2.0), vec2(0.0, 2.0));
            line(vec2(4.0, -2.0), vec2(0.0, 2.0));
            painter.add(egui::Shape::line(
                vec![
                    c + vec2(-7.5, 3.0),
                    c + vec2(-7.5, 7.5),
                    c + vec2(7.5, 7.5),
                    c + vec2(7.5, 3.0),
                ],
                stroke,
            ));
        }
    }
}

/// The script chip on the rail: the status dot ("!" while the script waits for you) over a
/// short state: the clock while it runs (yellow while paused), how its last run ended, or the
/// Script Loader's word for it. The name and the rest are in the Script Loader it opens.
fn rail_chip(ui: &mut Ui, bot: &turu_bot::Bot, status: ScriptStatus, corners: CornerRadius) -> Response {
    let (text, color) = chip_state(bot, status);
    if status.is_active() {
        ui.ctx().request_repaint_after(Duration::from_secs(1));
    }
    let text = match status {
        ScriptStatus::Loaded => script_loader::last_run(bot).map_or(text, |run| run.how.to_owned()),
        ScriptStatus::Empty => "No script".to_owned(),
        _ => text,
    };
    let (rect, response) = ui.allocate_exact_size(vec2(RAIL_INNER, RAIL_CHIP_HEIGHT), Sense::click());
    let pressed = response.is_pointer_button_down_on() || response.hovered();
    let painter = ui.painter();
    painter.rect_filled(rect, corners, if pressed { HOVER } else { TRACK });
    let dot = pos2(rect.center().x, rect.top() + 16.0);
    if bot.scripts.waits_for().is_some() {
        painter.circle_filled(dot, 6.0, WARNING);
        painter.text(
            dot,
            Align2::CENTER_CENTER,
            "!",
            FontId::proportional(10.0),
            Color32::BLACK,
        );
    } else {
        painter.circle_filled(dot, 4.0, widgets::status_color(status));
    }
    let mut job = LayoutJob::simple_singleline(text, FontId::proportional(11.0), color);
    job.wrap = TextWrapping::truncate_at_width(RAIL_INNER - 6.0);
    let galley = painter.layout_job(job);
    let at = pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 28.0);
    painter.galley(at, galley, color);
    response
}
