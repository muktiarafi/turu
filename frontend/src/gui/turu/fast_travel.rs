//! Tools & Helpers > Fast Travel (VibeSkua `FastTravelView` / `FastTravelViewModel`): saved
//! destinations (name, map, cell, pad) that a tap travels to, with VibeSkua's private room
//! switch, search, and add form. Laid out for any window size instead of VibeSkua's fixed three
//! columns: the tiles take one to three columns, each has a ⋮ menu (Edit, Remove) rather than
//! buttons shown on hover, editing reuses the add form rather than a dialog, and every change is
//! saved (`FastTravels.json`), so there's no Save button. The travel runs on a bot thread
//! (`turu_bot::api::map::fast_travel`) and says how it went.

use super::theme::{ACCENT, CARD, DANGER, EDGE, HOVER, MUTED, SMALL_RADIUS, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Button, Color32, ComboBox, Context, CornerRadius, FontId, Key, Layout, Modal, Rect, Response, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use turu_bot::api::{map, player};
use turu_bot::settings::FastTravel;

/// VibeSkua `MapService.Pads`.
const PADS: [&str; 8] = ["Spawn", "Center", "Left", "Right", "Up", "Down", "Top", "Bottom"];
/// VibeSkua `MapService.PrivateRoomNumber`'s default.
const DEFAULT_ROOM: i64 = 111111;
/// A tile is never narrower than this; the columns (at most three) follow from it.
const MIN_TILE_WIDTH: f32 = 190.0;
const MAX_COLUMNS: usize = 3;
const GAP: f32 = 6.0;
/// Below this width the form puts each field on a row of its own (in pairs).
const NARROW: f32 = 440.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Muted,
    Success,
    Warning,
    Danger,
}

impl Tone {
    fn color(self) -> Color32 {
        match self {
            Tone::Muted => MUTED,
            Tone::Success => SUCCESS,
            Tone::Warning => WARNING,
            Tone::Danger => DANGER,
        }
    }
}

/// What the bot threads hand back.
#[derive(Default)]
struct Shared {
    /// The destination being travelled to.
    traveling: Option<FastTravel>,
    /// How the last travel (or Get current) went.
    status: Option<(String, Tone)>,
    /// Get current's map, cell and pad, for the form to take.
    current: Option<(String, String, String)>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// What a tile's menu asked for.
enum Action {
    Edit(usize),
    Remove(usize),
}

pub struct FastTravelWindow {
    list: Vec<FastTravel>,
    search: String,
    private_room: bool,
    room: i64,
    /// The add form (or, with `editing`, the destination being edited).
    form: FastTravel,
    /// The index the form edits; `None`: it adds.
    editing: Option<usize>,
    confirm_clear: bool,
    was_open: bool,
    /// The form card's height last frame, so the tiles take the rest of the window.
    form_height: Option<f32>,
    shared: Arc<Mutex<Shared>>,
}

impl Default for FastTravelWindow {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            search: String::new(),
            private_room: true,
            room: DEFAULT_ROOM,
            form: empty_form(),
            editing: None,
            confirm_clear: false,
            was_open: false,
            form_height: None,
            shared: Arc::default(),
        }
    }
}

/// VibeSkua's new destination: no name, `battleon`, `Enter`, `Spawn`.
fn empty_form() -> FastTravel {
    FastTravel::new("", "battleon", "Enter", "Spawn")
}

impl FastTravelWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if *open && !self.was_open {
            self.load();
        }
        self.was_open = *open;
        if !*open {
            return;
        }
        let mut window = super::window(ctx, "Fast Travel", [600.0, 470.0], 4).open(open);
        // On a phone a floating window leaves the form under the keyboard: it fills the screen.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 620.0 || area.height() < 520.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui));
        self.clear_dialog(ctx);
        if lock(&self.shared).traveling.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// The saved list, read again each time the window opens (another tab may have changed it).
    /// A file that can't be read is kept beside it as `.bad`, and the defaults take its place.
    fn load(&mut self) {
        let path = crate::turu::bot().paths.fast_travels_file();
        self.list = match FastTravel::load(&path) {
            Ok(list) => list,
            Err(e) => {
                let bad = path.with_extension("json.bad");
                let _ = std::fs::rename(&path, &bad);
                self.set_status(
                    format!("Couldn't read {} ({e}): kept as {}", path.display(), bad.display()),
                    Tone::Danger,
                );
                FastTravel::defaults()
            }
        };
        if self.editing.is_some_and(|i| i >= self.list.len()) {
            self.stop_editing();
        }
    }

    /// VibeSkua `SaveFastTravels`, after every change.
    fn save(&self) {
        let path = crate::turu::bot().paths.fast_travels_file();
        if let Err(e) = FastTravel::save(&path, &self.list) {
            self.set_status(format!("Couldn't save {}: {e}", path.display()), Tone::Danger);
        }
    }

    fn set_status(&self, text: String, tone: Tone) {
        lock(&self.shared).status = Some((text, tone));
    }

    fn stop_editing(&mut self) {
        self.editing = None;
        self.form = empty_form();
    }

    fn ui(&mut self, ui: &mut Ui) {
        if let Some((map, cell, pad)) = lock(&self.shared).current.take() {
            self.form.map = map;
            self.form.cell = cell;
            self.form.pad = pad;
        }
        let (traveling, status) = {
            let s = lock(&self.shared);
            (s.traveling.clone(), s.status.clone())
        };
        ui.spacing_mut().item_spacing = vec2(GAP, 8.0);
        let control = widgets::target(28.0, 40.0);
        let narrow = ui.available_width() < NARROW;

        // Search, and VibeSkua's private room (its own row when narrow).
        if narrow {
            widgets::text_box(ui, &mut self.search, "Search name, map or cell", ui.available_width());
            ui.horizontal(|ui| self.private_room_controls(ui, control));
        } else {
            ui.horizontal(|ui| {
                let width = ui.available_width() - self.private_room_width(ui, control) - GAP;
                widgets::text_box(ui, &mut self.search, "Search name, map or cell", width);
                self.private_room_controls(ui, control);
            });
        }

        // Heading: the count of what the search shows, how the last travel went, and Clear.
        let shown: Vec<usize> = (0..self.list.len())
            .filter(|&i| self.list[i].matches(&self.search))
            .collect();
        ui.horizontal(|ui| {
            widgets::section_label(ui, "DESTINATIONS");
            widgets::count_badge(ui, shown.len());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let clear = widgets::button(
                    ui,
                    "Clear",
                    Kind::Normal,
                    !self.list.is_empty(),
                    vec2(64.0, widgets::target(22.0, 40.0)),
                    "The list is empty",
                )
                .on_hover_text("Remove every destination");
                if clear.clicked() {
                    self.confirm_clear = true;
                }
                if let Some((text, tone)) = &status {
                    status_line(ui, text, tone.color());
                }
            });
        });

        // The tiles, then the form under them.
        let form_height = self.form_height.unwrap_or_else(|| form_height(control, narrow));
        let tile_height = widgets::target(50.0, 60.0);
        let list_height = (ui.available_height() - form_height - ui.spacing().item_spacing.y).max(tile_height + 10.0);
        let mut action = None;
        widgets::list_well(ui, list_height, |ui| {
            action = self.tiles(ui, &shown, traveling.as_ref(), tile_height);
        });
        match action {
            Some(Action::Edit(i)) => {
                self.form = self.list[i].clone();
                self.editing = Some(i);
            }
            Some(Action::Remove(i)) => {
                self.list.remove(i);
                match self.editing {
                    Some(e) if e == i => self.stop_editing(),
                    Some(e) if e > i => self.editing = Some(e - 1),
                    _ => {}
                }
                self.save();
            }
            None => {}
        }
        let top = ui.cursor().top();
        self.form(ui, control, narrow);
        let height = ui.min_rect().bottom() - top;
        if self.form_height.is_none_or(|h| (h - height).abs() > 0.5) {
            self.form_height = Some(height);
            ui.ctx().request_repaint();
        }
    }

    fn private_room_width(&self, ui: &Ui, control: f32) -> f32 {
        let label = ui
            .painter()
            .layout_no_wrap("Private room".to_owned(), FontId::proportional(13.0), TEXT)
            .size()
            .x;
        let switch = if widgets::touch() { 48.0 } else { 34.0 };
        switch + label + room_width(control) + GAP * 2.0
    }

    /// VibeSkua `UsePrivateRoom` and `PrivateRoomNumber`: travels join `map-<room>`.
    fn private_room_controls(&mut self, ui: &mut Ui, control: f32) {
        widgets::switch(ui, &mut self.private_room).on_hover_text("Join private rooms with this number");
        let label = ui.add(egui::Label::new("Private room").sense(Sense::click()).selectable(false));
        if label.clicked() {
            self.private_room = !self.private_room;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            widgets::number(
                ui,
                "turu_fast_travel_room",
                &mut self.room,
                1..=999_999_999,
                1.0,
                "",
                vec2(room_width(control), control),
            )
            .on_hover_text("Room number");
        });
    }

    /// The destinations as tiles, in as many columns (up to three) as fit; returns what a tile's
    /// menu asked for.
    fn tiles(&self, ui: &mut Ui, shown: &[usize], traveling: Option<&FastTravel>, height: f32) -> Option<Action> {
        if shown.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                let text = if self.list.is_empty() {
                    "No destinations yet. Fill in the form below (Get current takes where you are), then Add."
                        .to_owned()
                } else {
                    format!("No destinations match \u{201C}{}\u{201D}.", self.search.trim())
                };
                ui.add(egui::Label::new(RichText::new(text).color(MUTED)).wrap());
            });
            return None;
        }
        let mut action = None;
        let width = ui.available_width();
        let columns = (((width + GAP) / (MIN_TILE_WIDTH + GAP)).floor() as usize).clamp(1, MAX_COLUMNS);
        let rows = shown.len().div_ceil(columns);
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        ScrollArea::vertical()
            .id_salt("turu_fast_travel_tiles")
            .auto_shrink([false, false])
            .show_rows(ui, height, rows, |ui, range| {
                // The scroll bar takes its room only once the list scrolls.
                let tile_width = ((ui.available_width() - GAP * (columns - 1) as f32) / columns as f32).floor();
                for row in range {
                    ui.horizontal(|ui| {
                        for &index in shown.iter().skip(row * columns).take(columns) {
                            let dest = &self.list[index];
                            let busy = traveling == Some(dest);
                            let edited = self.editing == Some(index);
                            if let Some(asked) = self.tile(ui, index, dest, busy, edited, vec2(tile_width, height)) {
                                action = Some(asked);
                            }
                        }
                    });
                }
            });
        action
    }

    /// One destination: a click (or Enter while focused) travels there; ⋮ edits or removes it.
    fn tile(
        &self,
        ui: &mut Ui,
        index: usize,
        dest: &FastTravel,
        busy: bool,
        edited: bool,
        size: Vec2,
    ) -> Option<Action> {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let hovered = ui.rect_contains_pointer(rect);
        let painter = ui.painter();
        painter.rect_filled(
            rect,
            CornerRadius::same(SMALL_RADIUS),
            if hovered { HOVER } else { CARD },
        );
        let stroke = if busy || edited || response.has_focus() {
            Stroke::new(if response.has_focus() { 2.0 } else { 1.0 }, ACCENT)
        } else {
            Stroke::new(1.0, EDGE)
        };
        painter.rect_stroke(rect, CornerRadius::same(SMALL_RADIUS), stroke, StrokeKind::Inside);

        let icon = pos2(rect.left() + 18.0, rect.center().y);
        // Painted, not added: a widget here would move the row's cursor back into this tile and
        // shift the tiles after it.
        if busy {
            travel_spinner(ui, icon, ACCENT);
        } else {
            travel_icon(ui.painter(), icon, ACCENT);
        }

        let menu_width = widgets::target(28.0, 44.0);
        let text_left = rect.left() + 34.0;
        let text_width = (rect.right() - menu_width - 6.0 - text_left).max(10.0);
        let line = |text: &str, font: FontId, color: Color32| {
            let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
            job.wrap = TextWrapping::truncate_at_width(text_width);
            ui.painter().layout_job(job)
        };
        let name = line(&dest.name, FontId::proportional(13.0), TEXT);
        let place = format!("{} \u{B7} {} \u{B7} {}", dest.map, dest.cell, dest.pad);
        let detail = line(&place, FontId::monospace(11.0), MUTED);
        let top = rect.center().y - (name.size().y + 2.0 + detail.size().y) / 2.0;
        let detail_top = top + name.size().y + 2.0;
        ui.painter().galley(pos2(text_left, top), name, TEXT);
        ui.painter().galley(pos2(text_left, detail_top), detail, MUTED);

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.on_hover_text(format!("Travel to {place}")).clicked() && !busy {
            self.travel(dest.clone());
        }

        // The menu, drawn last so it gets the click over the tile.
        let menu_rect = Rect::from_min_max(
            pos2(rect.right() - menu_width - 4.0, rect.top() + 4.0),
            pos2(rect.right() - 4.0, rect.bottom() - 4.0),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(menu_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let mut action = None;
        let button = Button::new(RichText::new("⋮").size(16.0).color(MUTED))
            .fill(Color32::TRANSPARENT)
            .min_size(menu_rect.size());
        egui::containers::menu::MenuButton::from_button(button).ui(&mut child, |ui| {
            ui.set_min_width(150.0);
            touch_rows(ui);
            if ui.button("Edit").clicked() {
                action = Some(Action::Edit(index));
                ui.close();
            }
            if ui.button(RichText::new("Remove").color(DANGER)).clicked() {
                action = Some(Action::Remove(index));
                ui.close();
            }
        });
        action
    }

    /// VibeSkua `FastTravelEditorViewModel`: the add form, which also edits a destination.
    fn form(&mut self, ui: &mut Ui, control: f32, narrow: bool) {
        widgets::card(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            ui.spacing_mut().interact_size.y = control;
            let width = ui.available_width();
            ui.horizontal(|ui| {
                ui.set_min_height(18.0);
                match self.editing.and_then(|i| self.list.get(i)) {
                    Some(dest) => {
                        let label = format!("EDITING {}", dest.name.to_uppercase());
                        ui.add(
                            egui::Label::new(
                                RichText::new(label)
                                    .size(widgets::small_label_size())
                                    .strong()
                                    .color(ACCENT),
                            )
                            .truncate(),
                        );
                    }
                    None => widgets::section_label(ui, "ADD A DESTINATION"),
                }
            });

            let mut submit = false;
            let half = (width - GAP) / 2.0;
            if narrow {
                submit |= field(ui, &mut self.form.name, "Name, e.g. Ultra Alteon", width, control);
                submit |= field(ui, &mut self.form.map, "Map", width, control);
            } else {
                ui.horizontal(|ui| {
                    submit |= field(ui, &mut self.form.name, "Name, e.g. Ultra Alteon", half * 1.15, control);
                    submit |= field(ui, &mut self.form.map, "Map", half * 0.85, control);
                });
            }
            if narrow {
                ui.horizontal(|ui| submit |= self.cell_and_pad(ui, half, control));
                ui.horizontal(|ui| submit |= self.buttons(ui, half, control));
            } else {
                ui.horizontal(|ui| {
                    let pair = (width - GAP * 3.0) / 4.0;
                    submit |= self.cell_and_pad(ui, pair, control);
                    submit |= self.buttons(ui, pair, control);
                });
            }
            if submit {
                self.submit();
            }
        });
    }

    /// The Cell box and the Pad drop-down, `width` each; true when Enter was pressed in Cell.
    fn cell_and_pad(&mut self, ui: &mut Ui, width: f32, control: f32) -> bool {
        let submit = field(ui, &mut self.form.cell, "Cell", width, control);
        let pad = self.form.pad.clone();
        ComboBox::from_id_salt("turu_fast_travel_pad")
            .selected_text(if pad.trim().is_empty() { "Pad" } else { &pad })
            .width(width)
            .show_ui(ui, |ui| {
                touch_rows(ui);
                for p in PADS {
                    if ui.selectable_label(pad == p, p).clicked() {
                        self.form.pad = p.to_owned();
                    }
                }
            });
        submit
    }

    /// Get current, then Add (or, editing, Cancel and Save), `width` each; true for Add/Save.
    fn buttons(&mut self, ui: &mut Ui, width: f32, control: f32) -> bool {
        let size = vec2(width, control);
        if self.editing.is_some() {
            if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                self.stop_editing();
            }
        } else if widgets::button(ui, "Get current", Kind::Normal, true, size, "")
            .on_hover_text("Fill in the map, cell and pad you are in")
            .clicked()
        {
            self.get_current();
        }
        let label = if self.editing.is_some() { "Save" } else { "Add" };
        let valid = self.form.is_valid();
        widgets::button(
            ui,
            label,
            Kind::Primary,
            valid,
            size,
            "Fill in the name, map, cell and pad",
        )
        .clicked()
    }

    /// VibeSkua `AddFastTravel`, or the edit dialog's OK: the form's destination is saved.
    fn submit(&mut self) {
        if !self.form.is_valid() {
            return;
        }
        let dest = self.form.trimmed();
        match self.editing.filter(|&i| i < self.list.len()) {
            Some(i) => self.list[i] = dest,
            None => self.list.push(dest),
        }
        self.save();
        self.stop_editing();
    }

    /// VibeSkua `GetCurrent`: the map, cell and pad you are in, into the form.
    fn get_current(&self) {
        let shared = self.shared.clone();
        crate::turu::spawn_action("Fast Travel current", move |bot| {
            let here = if player::logged_in(bot)? && player::world_loaded(bot)? {
                Some((map::name(bot)?, player::cell(bot)?, player::pad(bot)?))
            } else {
                None
            };
            let mut s = lock(&shared);
            match here {
                Some(here) if !here.0.is_empty() => s.current = Some(here),
                _ => s.status = Some(("Not in a map: log in first".to_owned(), Tone::Danger)),
            }
            Ok(())
        });
    }

    /// VibeSkua `MapService.Travel`, on a bot thread, saying how it went.
    fn travel(&self, dest: FastTravel) {
        let room = self.private_room.then_some(self.room);
        {
            let mut s = lock(&self.shared);
            if s.traveling.is_some() {
                return;
            }
            s.traveling = Some(dest.clone());
            s.status = Some((format!("Traveling to {}...", dest.name), Tone::Muted));
        }
        let shared = self.shared.clone();
        crate::turu::spawn_action("Fast Travel", move |bot| {
            let outcome = travel_outcome(bot, &dest, room);
            let mut s = lock(&shared);
            s.traveling = None;
            s.status = Some(match outcome {
                Ok(outcome) => outcome,
                Err(e) => (format!("Travel failed: {e}"), Tone::Danger),
            });
            Ok(())
        });
    }

    /// VibeSkua `ClearFastTravels`' question.
    fn clear_dialog(&mut self, ctx: &Context) {
        if !self.confirm_clear {
            return;
        }
        let mut answer = None;
        let response = Modal::new(egui::Id::new("turu_fast_travel_clear")).show(ctx, |ui| {
            ui.set_min_width(260.0);
            ui.set_max_width(380.0);
            ui.heading("Clear Fast Travels");
            ui.label("This will clear all fast travels from the list. Continue?");
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let size = vec2(72.0, widgets::target(28.0, 48.0));
                if widgets::button(ui, "Yes", Kind::Danger, true, size, "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "No", Kind::Normal, true, size, "").clicked() {
                    answer = Some(false);
                }
            });
        });
        if answer.is_none() && !response.should_close() {
            return;
        }
        self.confirm_clear = false;
        if answer == Some(true) {
            self.list.clear();
            self.stop_editing();
            self.save();
        }
    }
}

/// Travels to `dest` and says where that left you.
fn travel_outcome(bot: &turu_bot::Bot, dest: &FastTravel, room: Option<i64>) -> turu_bot::BotResult<(String, Tone)> {
    if !player::logged_in(bot)? {
        return Ok(("Not logged in".to_owned(), Tone::Danger));
    }
    if !player::alive(bot)? {
        return Ok(("You're dead: travel once you respawn".to_owned(), Tone::Warning));
    }
    map::fast_travel(bot, &dest.map, &dest.cell, &dest.pad, room)?;
    let (now, cell) = (map::name(bot)?, player::cell(bot)?);
    Ok(
        if now.eq_ignore_ascii_case(&dest.map) && cell.eq_ignore_ascii_case(&dest.cell) {
            (format!("At {}", dest.name), Tone::Success)
        } else {
            (
                format!("Didn't reach {} \u{B7} {}: in {now} \u{B7} {cell}", dest.map, dest.cell),
                Tone::Warning,
            )
        },
    )
}

/// The room number box's width.
fn room_width(control: f32) -> f32 {
    if control > 30.0 { 104.0 } else { 88.0 }
}

/// The form card's height before it was first drawn.
fn form_height(control: f32, narrow: bool) -> f32 {
    let rows = if narrow { 4.0 } else { 2.0 };
    // The card's margins and edge, its heading, the rows and the gaps between them.
    22.0 + 18.0 + rows * control + rows * GAP
}

/// A one-line form field `width` wide and `height` high; true when Enter was pressed in it.
fn field(ui: &mut Ui, text: &mut String, hint: &str, width: f32, height: f32) -> bool {
    let mut response: Response = ui.add_sized(
        vec2(width, height),
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .vertical_align(Align::Center)
            .margin(egui::Margin::symmetric(8, 4))
            .background_color(super::theme::INSET),
    );
    super::native_field::follow(ui, &mut response, text, hint, super::native_field::Kind::Line);
    response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
}

/// How the last travel went, cut to the room left of Clear (whole on hover).
fn status_line(ui: &mut Ui, text: &str, color: Color32) {
    let width = (ui.available_width() - 8.0).max(0.0);
    let mut job = LayoutJob::simple_singleline(
        text.to_owned(),
        FontId::proportional(widgets::small_label_size()),
        color,
    );
    job.wrap = TextWrapping::truncate_at_width(width);
    let galley = ui.painter().layout_job(job);
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
    response.on_hover_text(text);
}

/// A navigation arrow (VibeSkua's `NavigationVariantOutline`), outlined, 14 points across.
fn travel_icon(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let points = [vec2(6.0, -6.0), vec2(-6.0, -1.0), vec2(-0.5, 0.5), vec2(1.0, 6.0)]
        .map(|p| center + p)
        .to_vec();
    painter.add(egui::epaint::PathShape::closed_line(points, Stroke::new(1.5, color)));
}

/// The travel icon's stand-in while travelling: a turning arc in the icon's place, size and stroke.
fn travel_spinner(ui: &Ui, center: egui::Pos2, color: Color32) {
    ui.ctx().request_repaint();
    let start = (ui.input(|i| i.time) * std::f64::consts::TAU) as f32;
    let points = (0..=24)
        .map(|i| {
            let angle = start + i as f32 / 24.0 * 1.5 * std::f32::consts::PI;
            center + 6.0 * vec2(angle.cos(), angle.sin())
        })
        .collect();
    ui.painter()
        .add(egui::epaint::PathShape::line(points, Stroke::new(1.5, color)));
}

/// On a touch screen, a menu's or drop-down's rows are about 44 points high.
fn touch_rows(ui: &mut Ui) {
    if widgets::touch() {
        ui.spacing_mut().button_padding.y = 14.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tiles take one column on a phone and up to three on a wide window.
    #[test]
    fn columns_follow_the_width() {
        let columns = |width: f32| (((width + GAP) / (MIN_TILE_WIDTH + GAP)).floor() as usize).clamp(1, MAX_COLUMNS);
        assert_eq!(columns(340.0), 1);
        assert_eq!(columns(520.0), 2);
        assert_eq!(columns(900.0), 3);
        assert_eq!(columns(100.0), 1);
    }
}
