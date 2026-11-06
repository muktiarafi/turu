//! Options > HotKeys (VibeSkua `HotKeysView`, 480x500): the actions you gave a key, and the
//! catalog to add more from. The bindings and what each action does are in `hotkeys.rs`.
//!
//! Wide, the two sit side by side: YOUR HOTKEYS and ADD AN ACTION. Narrow, they are two pages,
//! the catalog behind "+ Add an action". A keycap is clicked to set its key: the next press
//! (with Ctrl, Shift or Alt if held) is the key, Escape cancels, Backspace clears it. A key
//! another action holds moves to this one, and the note under the list says from which.
//! Adding an action from the catalog waits for its key at once. Every change is saved.

use super::hotkeys::{self, ACTIONS, Action, Binding, Gesture, Group};
use super::theme::{ACCENT, ACCENT_HOVER, DANGER, EDGE, HOVER, INSET, MUTED, TEXT, TRACK, WARNING};
use super::widgets::{self, Kind};
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Align2, Color32, Context, CornerRadius, CursorIcon, FontId, Galley, Id, Key, Layout, Rect, Response,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Below this width the catalog is a page of its own.
const NARROW: f32 = 600.0;
const GAP: f32 = 8.0;
/// How long a note stays under the list.
const NOTE_FOR: Duration = Duration::from_secs(8);
/// How long "Restore defaults" waits for its second click.
const CONFIRM_FOR: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Info,
    Warning,
    Error,
}

struct Note {
    text: String,
    tone: Tone,
    at: Instant,
    /// A removed binding and where it was, for Undo.
    undo: Option<(usize, Binding)>,
}

#[derive(Default)]
pub struct HotKeysWindow {
    /// The action waiting for its key.
    capturing: Option<&'static str>,
    filter: String,
    search: String,
    group: Option<Group>,
    /// Narrow: the catalog page is shown.
    adding: bool,
    note: Option<Note>,
    confirm_reset: Option<Instant>,
    /// Scroll the list to this action's row once (just added).
    reveal: Option<&'static str>,
    focus_search: bool,
    /// The height under YOUR HOTKEYS' list, measured.
    footer: f32,
}

/// What a row of YOUR HOTKEYS asked for.
enum RowAction {
    Capture,
    Remove,
}

impl HotKeysWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !crate::platform::platform().has_hotkeys() {
            *open = false;
        }
        if !*open {
            self.stop_capture();
            return;
        }
        let mut still_open = true;
        super::window(ctx, "HotKeys", [640.0, 540.0], 3)
            .open(&mut still_open)
            .show(ctx, |ui| self.ui(ui));
        if !still_open {
            *open = false;
            self.stop_capture();
        }
        if self.capturing.is_some() || self.note.is_some() || self.confirm_reset.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn stop_capture(&mut self) {
        self.capturing = None;
    }

    fn start_capture(&mut self, action: &'static str) {
        self.capturing = Some(action);
    }

    fn ui(&mut self, ui: &mut Ui) {
        let mut bindings = hotkeys::bindings();
        if let Some(action) = self.capturing {
            hotkeys::mark_capturing(ui.ctx());
            self.capture(ui.ctx(), &mut bindings, action);
        }
        if self.note.as_ref().is_some_and(|n| n.at.elapsed() >= NOTE_FOR) {
            self.note = None;
        }
        if self.confirm_reset.is_some_and(|at| at.elapsed() >= CONFIRM_FOR) {
            self.confirm_reset = None;
        }
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        let height = ui.available_height();
        if ui.available_width() < NARROW {
            if self.adding {
                self.catalog_pane(ui, &mut bindings, height, true);
            } else {
                self.assigned_pane(ui, &mut bindings, height, true);
            }
            return;
        }
        ui.horizontal_top(|ui| {
            let left = ((ui.available_width() - GAP) * 0.58).floor();
            ui.allocate_ui_with_layout(vec2(left, height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(left);
                self.assigned_pane(ui, &mut bindings, height, false);
            });
            ui.allocate_ui_with_layout(vec2(ui.available_width(), height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(ui.available_width());
                self.catalog_pane(ui, &mut bindings, height, false);
            });
        });
    }

    /// Takes the first key press for the action waiting for its key.
    fn capture(&mut self, ctx: &Context, bindings: &mut [Binding], action: &'static str) {
        let Some(index) = bindings.iter().position(|b| b.action == action) else {
            self.stop_capture();
            return;
        };
        let press = ctx.input_mut(|input| {
            let at = input
                .events
                .iter()
                .position(|e| matches!(e, egui::Event::Key { pressed: true, key, .. } if !is_modifier(*key)))?;
            match input.events.remove(at) {
                egui::Event::Key { key, modifiers, .. } => Some((key, modifiers)),
                _ => None,
            }
        });
        let Some((key, modifiers)) = press else { return };
        self.stop_capture();
        let title = bindings[index].action().title;
        if key == Key::Escape {
            return;
        }
        if modifiers.is_none() && matches!(key, Key::Backspace | Key::Delete) {
            if bindings[index].gesture.take().is_some() {
                self.save(bindings, Some(format!("{title} has no key now")), Tone::Info);
            }
            return;
        }
        let Some(gesture) = Gesture::of_press(key, modifiers) else {
            self.set_note(format!("{} can't be a hotkey", key.name()), Tone::Warning);
            return;
        };
        let mut moved = None;
        for (i, other) in bindings.iter_mut().enumerate() {
            if i != index && other.gesture == Some(gesture) {
                other.gesture = None;
                moved = Some(other.action().title);
            }
        }
        bindings[index].gesture = Some(gesture);
        let label = gesture.label();
        match moved {
            Some(from) => self.save(
                bindings,
                Some(format!("{label} moved from {from} to {title}")),
                Tone::Warning,
            ),
            None => self.save(bindings, None, Tone::Info),
        }
    }

    /// Writes the bindings; `note` (or the error) shows under the list.
    fn save(&mut self, bindings: &[Binding], note: Option<String>, tone: Tone) {
        match hotkeys::set_bindings(bindings.to_vec()) {
            Ok(()) => {
                if let Some(text) = note {
                    self.set_note(text, tone);
                }
            }
            Err(e) => self.set_note(format!("Couldn't save the hotkeys: {e}"), Tone::Error),
        }
    }

    fn set_note(&mut self, text: String, tone: Tone) {
        self.note = Some(Note {
            text,
            tone,
            at: Instant::now(),
            undo: None,
        });
    }

    /// YOUR HOTKEYS: the bound actions, a filter, the note and Restore defaults.
    fn assigned_pane(&mut self, ui: &mut Ui, bindings: &mut Vec<Binding>, height: f32, narrow: bool) {
        let top = ui.min_rect().bottom();
        let control = widgets::target(26.0, 40.0);
        ui.horizontal(|ui| {
            ui.set_min_height(control);
            widgets::section_label(ui, "YOUR HOTKEYS");
            widgets::count_badge(ui, bindings.len());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let width = (ui.available_width() - GAP).clamp(90.0, 170.0);
                let filter = widgets::text_box(ui, &mut self.filter, "Filter", width);
                if filter.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.filter.clear();
                }
            });
        });

        let filter = self.filter.trim().to_lowercase();
        let shown: Vec<usize> = (0..bindings.len())
            .filter(|&i| filter.is_empty() || binding_matches(&bindings[i], &filter))
            .collect();

        // The footer (note, hint, Restore defaults, and on a narrow window the button to the
        // catalog) as tall as it was last frame: the hint may wrap.
        let used = ui.min_rect().bottom() - top;
        let row = widgets::target(46.0, 56.0);
        let footer = if self.footer > 0.0 { self.footer } else { 100.0 };
        let well = (height - used - footer - GAP).max(row * 2.0 + 10.0);
        let mut action = None;
        let reveal = self.reveal.take();
        widgets::list_well(ui, well, |ui| {
            if bindings.is_empty() {
                empty_text(ui, "No hotkeys yet. Add an action, then press the key you want for it.");
                return;
            }
            if shown.is_empty() {
                empty_text(
                    ui,
                    &format!("No hotkey matches \u{201C}{}\u{201D}.", self.filter.trim()),
                );
                return;
            }
            ScrollArea::vertical()
                .id_salt("turu_hotkeys_assigned")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for &i in &shown {
                        let binding = &bindings[i];
                        let capturing = self.capturing == Some(binding.action);
                        let (response, asked) = assigned_row(ui, binding, capturing, row);
                        if reveal == Some(binding.action) {
                            response.scroll_to_me(Some(Align::Center));
                        }
                        if let Some(asked) = asked {
                            action = Some((i, asked));
                        }
                    }
                });
        });
        match action {
            Some((i, RowAction::Capture)) => {
                if self.capturing == Some(bindings[i].action) {
                    self.stop_capture();
                } else {
                    self.start_capture(bindings[i].action);
                }
            }
            Some((i, RowAction::Remove)) => {
                if self.capturing == Some(bindings[i].action) {
                    self.stop_capture();
                }
                let removed = bindings.remove(i);
                let title = removed.action().title;
                let text = match removed.gesture {
                    Some(g) => format!("Removed {title} ({})", g.label()),
                    None => format!("Removed {title}"),
                };
                self.save(bindings, Some(text), Tone::Info);
                if let Some(note) = &mut self.note {
                    note.undo = Some((i, removed));
                }
            }
            None => {}
        }

        let footer_top = ui.min_rect().bottom();
        self.note_line(ui, bindings);
        self.hint_line(ui, bindings);
        ui.horizontal(|ui| {
            let confirming = self.confirm_reset.is_some();
            let label = if confirming {
                "Click again to restore"
            } else {
                "Restore defaults"
            };
            let kind = if confirming { Kind::Danger } else { Kind::Outline };
            let reset = widgets::button(ui, label, kind, true, vec2(0.0, control), "")
                .on_hover_text("Go back to VibeSkua's seven default hotkeys (F2 to F6, F9, F10)");
            if reset.clicked() {
                if confirming {
                    self.confirm_reset = None;
                    self.stop_capture();
                    *bindings = hotkeys::defaults();
                    self.save(bindings, Some("Restored the default hotkeys".into()), Tone::Info);
                } else {
                    self.confirm_reset = Some(Instant::now());
                }
            }
        });
        if narrow {
            let add = widgets::button(
                ui,
                "+  Add an action",
                Kind::Normal,
                true,
                vec2(ui.available_width(), control),
                "",
            );
            if add.clicked() {
                self.adding = true;
                self.focus_search = true;
                self.stop_capture();
            }
        }
        let footer = ui.min_rect().bottom() - footer_top;
        if (footer - self.footer).abs() > 0.5 {
            self.footer = footer;
            ui.ctx().request_repaint();
        }
    }

    /// The note under the list (one line, whole on hover), with Undo after a removal.
    fn note_line(&mut self, ui: &mut Ui, bindings: &mut Vec<Binding>) {
        if self.note.is_none() {
            return;
        }
        let mut undo = false;
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 22.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_height(22.0);
                let Some(note) = &self.note else { return };
                let color = match note.tone {
                    Tone::Info => TEXT,
                    Tone::Warning => WARNING,
                    Tone::Error => DANGER,
                };
                widgets::dot(ui, color, 3.0);
                let undo_width = if note.undo.is_some() { 48.0 } else { 0.0 };
                let width = ui.available_width() - undo_width;
                let font = FontId::proportional(widgets::small_label_size() + 1.0);
                ui.add(single_line(&note.text, font, color, width, true))
                    .on_hover_text(&note.text);
                if note.undo.is_some() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        undo = widgets::small_button(ui, "Undo", true).clicked();
                    });
                }
            },
        );
        if undo && let Some((i, binding)) = self.note.take().and_then(|n| n.undo) {
            let title = binding.action().title;
            // A key the binding had may have gone to another action since.
            let mut binding = binding;
            if bindings
                .iter()
                .any(|b| b.gesture.is_some() && b.gesture == binding.gesture)
            {
                binding.gesture = None;
            }
            bindings.insert(i.min(bindings.len()), binding);
            self.reveal = Some(bindings[i.min(bindings.len() - 1)].action);
            self.save(bindings, Some(format!("{title} is back")), Tone::Info);
        }
    }

    /// What to do now: while waiting for a key, how to give it; else how to change one. A key
    /// that also types says so, as it fires while you type in the game's chat.
    fn hint_line(&self, ui: &mut Ui, bindings: &[Binding]) {
        let size = widgets::small_label_size();
        let text = match self.capturing.and_then(hotkeys::find) {
            Some(action) => RichText::new(format!(
                "Press the key for {}, with Ctrl, Shift or Alt if you like. Esc cancels, Backspace clears.",
                action.title
            ))
            .color(ACCENT_HOVER),
            None if bindings.iter().any(|b| b.gesture.is_some_and(|g| g.types())) => {
                RichText::new("Keys marked \u{2022} also type: they fire in the game's chat too. Add Ctrl or Alt.")
                    .color(WARNING)
            }
            None => RichText::new("Click a key to change it.").color(MUTED),
        };
        ui.add(egui::Label::new(text.size(size)).wrap());
    }

    /// ADD AN ACTION: the actions without a binding, searchable and by group.
    fn catalog_pane(&mut self, ui: &mut Ui, bindings: &mut Vec<Binding>, height: f32, narrow: bool) {
        let top = ui.min_rect().bottom();
        let control = widgets::target(26.0, 40.0);
        ui.horizontal(|ui| {
            ui.set_min_height(control);
            if narrow {
                let back = ui
                    .add(egui::Button::new(RichText::new("\u{2190}").size(15.0)).min_size(Vec2::splat(control)))
                    .on_hover_text("Back to your hotkeys (Esc)");
                let escape = self.search.is_empty() && ui.input(|i| i.key_pressed(Key::Escape));
                if back.clicked() || escape {
                    self.adding = false;
                }
            }
            widgets::section_label(ui, "ADD AN ACTION");
        });
        let search = widgets::text_box(ui, &mut self.search, "Search actions", ui.available_width());
        if std::mem::take(&mut self.focus_search) {
            search.request_focus();
        }
        if search.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.search.clear();
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            if widgets::chip(ui, "All", self.group.is_none()) {
                self.group = None;
            }
            for group in Group::ALL {
                if widgets::chip(ui, group.short(), self.group == Some(group)) {
                    self.group = Some(group);
                }
            }
        });

        let terms: Vec<String> = self.search.split_whitespace().map(str::to_lowercase).collect();
        let available: Vec<&'static Action> = ACTIONS
            .iter()
            .filter(|a| !bindings.iter().any(|b| b.action == a.id))
            .filter(|a| self.group.is_none_or(|g| a.group == g))
            .filter(|a| terms.iter().all(|t| action_matches(a, t)))
            .collect();

        let used = ui.min_rect().bottom() - top;
        let row = widgets::target(44.0, 56.0);
        let well = (height - used - GAP).max(row * 2.0 + 10.0);
        let mut added = None;
        widgets::list_well(ui, well, |ui| {
            if available.is_empty() {
                let text = if !terms.is_empty() {
                    format!("No action matches \u{201C}{}\u{201D}.", self.search.trim())
                } else {
                    "Every action here has a hotkey.".to_owned()
                };
                empty_text(ui, &text);
                return;
            }
            ScrollArea::vertical()
                .id_salt("turu_hotkeys_catalog")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let headed = self.group.is_none() && terms.is_empty();
                    let mut last = None;
                    for action in &available {
                        if headed && last != Some(action.group) {
                            if last.is_some() {
                                ui.add_space(6.0);
                            }
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                widgets::section_label(ui, &action.group.label().to_uppercase());
                            });
                            last = Some(action.group);
                        }
                        if catalog_row(ui, action, row).clicked() {
                            added = Some(*action);
                        }
                    }
                });
        });
        if let Some(action) = added {
            bindings.push(Binding {
                action: action.id,
                gesture: None,
            });
            self.save(bindings, None, Tone::Info);
            self.reveal = Some(action.id);
            self.adding = false;
            self.start_capture(action.id);
        }
    }
}

fn is_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::AltLeft
            | Key::AltRight
            | Key::SuperLeft
            | Key::SuperRight
    )
}

/// The filter: the title, description or key contains `filter` (lowercase).
fn binding_matches(binding: &Binding, filter: &str) -> bool {
    let action = binding.action();
    action_matches(action, filter)
        || binding
            .gesture
            .is_some_and(|g| g.label().to_lowercase().contains(filter))
}

/// The search: the title, description or VibeSkua name contains `term` (lowercase).
fn action_matches(action: &Action, term: &str) -> bool {
    [action.title, action.description, action.id]
        .iter()
        .any(|text| text.to_lowercase().contains(term))
}

/// A row of YOUR HOTKEYS: the title and description, the keycap and remove.
fn assigned_row(ui: &mut Ui, binding: &Binding, capturing: bool, height: f32) -> (Response, Option<RowAction>) {
    let action = binding.action();
    let id = Id::new(("turu_hotkey_row", action.id));
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let hovered = ui.rect_contains_pointer(rect);
    let fill = if capturing {
        TRACK
    } else if hovered {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), fill);

    let mut asked = None;
    let remove_size = widgets::target(24.0, 40.0);
    let remove_rect = Rect::from_center_size(
        pos2(rect.right() - 4.0 - remove_size / 2.0, rect.center().y),
        Vec2::splat(remove_size),
    );
    let remove = ui.interact(remove_rect, id.with("remove"), Sense::click());
    if remove.hovered() {
        ui.painter()
            .rect_filled(remove_rect, CornerRadius::same(6), DANGER.gamma_multiply(0.18));
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let cross = if remove.hovered() {
        DANGER
    } else if hovered || widgets::touch() {
        MUTED
    } else {
        MUTED.gamma_multiply(0.5)
    };
    ui.painter().text(
        remove_rect.center(),
        Align2::CENTER_CENTER,
        "\u{2715}",
        FontId::proportional(12.0),
        cross,
    );
    if remove.on_hover_text("Remove this hotkey").clicked() {
        asked = Some(RowAction::Remove);
    }

    let (cap_rect, cap) = keycap(
        ui,
        id,
        binding.gesture,
        capturing,
        remove_rect.left() - 6.0,
        rect.center().y,
    );
    let tip = match (capturing, binding.gesture) {
        (true, _) => "Press a key (Esc cancels, Backspace clears)",
        (false, Some(_)) => "Click to set a new key",
        (false, None) => "Click, then press the key for this action",
    };
    if cap.on_hover_text(tip).clicked() {
        asked = Some(RowAction::Capture);
    }

    let typing = binding.gesture.is_some_and(|g| g.types());
    let text_left = rect.left() + 10.0;
    let mut text_right = cap_rect.left() - 10.0;
    if typing {
        // The mark the hint line names: this key types too.
        let mark = pos2(cap_rect.left() - 9.0, rect.center().y);
        ui.painter().circle_filled(mark, 3.0, WARNING);
        text_right -= 8.0;
    }
    let width = (text_right - text_left).max(20.0);
    let title = truncated(ui, action.title, FontId::proportional(13.0), TEXT, width);
    let description = truncated(
        ui,
        action.description,
        FontId::proportional(widgets::small_label_size()),
        MUTED,
        width,
    );
    let title_height = title.size().y;
    let top = rect.center().y - (title_height + 2.0 + description.size().y) / 2.0;
    let text_rect = Rect::from_min_max(pos2(text_left, rect.top()), pos2(text_left + width, rect.bottom()));
    ui.painter().galley(pos2(text_left, top), title, TEXT);
    ui.painter()
        .galley(pos2(text_left, top + title_height + 2.0), description, MUTED);
    let text = ui.interact(text_rect, id.with("text"), Sense::hover());
    text.on_hover_text(if typing {
        format!(
            "{}\n\nThis key also types: it fires while you type in the game's chat. Add Ctrl or Alt to avoid that.",
            action.description
        )
    } else {
        action.description.to_owned()
    });
    (response, asked)
}

/// A key drawn as a keycap, its right edge at `right`: the key in monospace, "Set key" when
/// there's none, "Press a key…" with a breathing accent while it waits for one.
fn keycap(ui: &Ui, id: Id, gesture: Option<Gesture>, capturing: bool, right: f32, center_y: f32) -> (Rect, Response) {
    let mono = FontId::monospace(12.0);
    let plain = FontId::proportional(12.0);
    let (text, font, color) = match (capturing, gesture) {
        (true, _) => ("Press a key\u{2026}".to_owned(), plain, ACCENT_HOVER),
        (false, Some(g)) => (g.label(), mono, TEXT),
        (false, None) => ("Set key".to_owned(), plain, MUTED),
    };
    let galley = ui.painter().layout_no_wrap(text, font, color);
    let height = widgets::target(26.0, 40.0);
    let width = (galley.size().x + 20.0).max(64.0);
    let rect = Rect::from_min_size(pos2(right - width, center_y - height / 2.0), vec2(width, height));
    let response = ui.interact(rect, id.with("key"), Sense::click());
    let painter = ui.painter();
    let radius = CornerRadius::same(6);
    if capturing {
        // A slow breath on the accent, so the waiting keycap reads as live.
        let t = ui.input(|i| i.time) as f32;
        let breath = 0.16 + 0.08 * (t * 3.0).sin();
        painter.rect_filled(rect, radius, ACCENT.gamma_multiply(breath));
        painter.rect_stroke(rect, radius, Stroke::new(1.5, ACCENT), StrokeKind::Inside);
    } else if gesture.is_some() {
        // A keycap: the face over a darker lip at the bottom.
        painter.rect_filled(rect, radius, EDGE);
        let face = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 2.0));
        let fill = if response.hovered() { HOVER } else { TRACK };
        painter.rect_filled(face, radius, fill);
        if response.hovered() {
            painter.rect_stroke(rect, radius, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
        }
    } else {
        let fill = if response.hovered() { HOVER } else { INSET };
        painter.rect_filled(rect, radius, fill);
        let edge = if response.hovered() { ACCENT } else { EDGE };
        painter.rect_stroke(rect, radius, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
    let lift = if gesture.is_some() && !capturing { -1.0 } else { 0.0 };
    painter.galley(rect.center() - galley.size() / 2.0 + vec2(0.0, lift), galley, color);
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    (rect, response)
}

/// A row of ADD AN ACTION: the title and description, and + on the right. Click to add.
fn catalog_row(ui: &mut Ui, action: &Action, height: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter().rect_filled(rect, CornerRadius::same(6), HOVER);
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let plus = pos2(rect.right() - 16.0, rect.center().y);
    ui.painter().text(
        plus,
        Align2::CENTER_CENTER,
        "+",
        FontId::proportional(18.0),
        if hovered { ACCENT_HOVER } else { MUTED },
    );
    let left = rect.left() + 10.0;
    let width = (rect.right() - 34.0 - left).max(20.0);
    let title = truncated(ui, action.title, FontId::proportional(13.0), TEXT, width);
    let description = truncated(
        ui,
        action.description,
        FontId::proportional(widgets::small_label_size()),
        MUTED,
        width,
    );
    let block = title.size().y + 2.0 + description.size().y;
    let top = rect.center().y - block / 2.0;
    let title_height = title.size().y;
    ui.painter().galley(pos2(left, top), title, TEXT);
    ui.painter()
        .galley(pos2(left, top + title_height + 2.0), description, MUTED);
    response.on_hover_text(format!(
        "{}\n\nClick to add it, then press its key.",
        action.description
    ))
}

/// One line of `text`, cut to `width` with an ellipsis.
fn truncated(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(width.max(10.0));
    ui.painter().layout_job(job)
}

/// One line cut to `width` with an ellipsis; `hover`: it can show a tooltip.
fn single_line(text: &str, font: FontId, color: Color32, width: f32, hover: bool) -> egui::Label {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(width.max(10.0));
    let label = egui::Label::new(job).selectable(false);
    if hover { label.sense(Sense::hover()) } else { label }
}

/// Centered muted text in a list.
fn empty_text(ui: &mut Ui, text: &str) {
    ui.add_space(14.0);
    ui.vertical_centered(|ui| {
        ui.add(egui::Label::new(RichText::new(text).color(MUTED)).wrap());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_title_description_and_name() {
        let lag = hotkeys::find("ToggleLagKiller").unwrap();
        assert!(action_matches(lag, "lag"));
        assert!(action_matches(lag, "cpu"));
        assert!(action_matches(lag, "togglelag"));
        assert!(!action_matches(lag, "bank"));
        let binding = Binding {
            action: lag.id,
            gesture: Gesture::parse("Ctrl+F6"),
        };
        assert!(binding_matches(&binding, "f6"));
    }
}
