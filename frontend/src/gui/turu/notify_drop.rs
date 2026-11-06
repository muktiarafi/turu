//! Combat > Runtime > Notifications (VibeSkua `NotifyDropView` / `NotifyDropViewModel`): a list
//! of item names, and a beep (`Platform::beep`, VibeSkua's `ISoundService.Beep`) played a number
//! of times when one of them drops. As VibeSkua, the list isn't saved: it lasts as long as the tab
//! (closing the window keeps it), and each tab has its own.

use super::theme::{DANGER, INSET, MUTED, TEXT};
use super::widgets::{self, Kind, Selection};
use egui::{Align, Context, Key, Label, Layout, RichText, ScrollArea, Ui, vec2};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;
use turu_bot::Bot;
use turu_bot::events::BotEvent;

pub const TITLE: &str = "Notify Drop";

/// Below this width the beep settings and Test take a row each.
const NARROW: f32 = 440.0;
/// From this width the beep settings sit in the WATCHING heading's row.
const WIDE: f32 = 720.0;

/// What the drop handler reads (on the thread that emits the event).
struct Watch {
    items: Mutex<Vec<String>>,
    /// VibeSkua `SoundCount`: beeps per drop.
    count: AtomicU32,
    /// VibeSkua `SoundDelay`: milliseconds between beeps.
    delay: AtomicU32,
}

impl Default for Watch {
    fn default() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            count: AtomicU32::new(5),
            delay: AtomicU32::new(200),
        }
    }
}

impl Watch {
    fn lock(&self) -> MutexGuard<'_, Vec<String>> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// VibeSkua `PlayBeepsAsync`, on a thread of its own: `count` beeps, `delay` ms apart.
    fn play(&self) {
        let count = self.count.load(Ordering::Relaxed);
        let delay = Duration::from_millis(self.delay.load(Ordering::Relaxed).into());
        let spawned = std::thread::Builder::new()
            .name("Turu Notify Drop".into())
            .spawn(move || {
                for i in 0..count {
                    crate::platform::platform().beep();
                    if i + 1 < count {
                        std::thread::sleep(delay);
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::warn!("Notify Drop beep failed: {e}");
        }
    }
}

#[derive(Default)]
pub struct NotifyDropWindow {
    input: String,
    watch: Arc<Watch>,
    selection: Selection,
    /// The bot whose drops are watched, and the handler's ID: subscribed while the list has
    /// items (VibeSkua registers on the first Add and unregisters when Remove empties it).
    subscription: Option<(Weak<Bot>, u64)>,
}

impl Drop for NotifyDropWindow {
    fn drop(&mut self) {
        self.unsubscribe();
    }
}

impl NotifyDropWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        // VibeSkua's 534x337; taller on a touch screen, whose controls and rows are finger-sized.
        let size = if widgets::touch() {
            [560.0, 480.0]
        } else {
            [534.0, 337.0]
        };
        let mut window = super::window(ctx, TITLE, size, 5).open(open);
        // On a phone a floating window leaves the box under the keyboard: it fills the screen.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 560.0 || area.height() < 400.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut Ui) {
        let control = widgets::target(28.0, 40.0);
        let spacing = ui.spacing().item_spacing;
        let width = ui.available_width();

        row(ui, control, |ui| {
            let box_width = (ui.available_width() - 84.0 - spacing.x).max(120.0);
            let response = ui
                .add_sized(
                    [box_width, control],
                    egui::TextEdit::singleline(&mut self.input)
                        .hint_text("Item name (separate several with |)")
                        .vertical_align(Align::Center)
                        .margin(egui::Margin::symmetric(8, 5))
                        .background_color(INSET),
                )
                .on_hover_text("Use the exact in-game item names");
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let add = widgets::button(ui, "+  Add", Kind::Primary, true, vec2(84.0, control), "").clicked();
            if add || enter {
                self.add();
            }
        });

        let items = self.watch.lock().clone();
        let clear_size = vec2(84.0, widgets::target(24.0, 40.0));
        // Wide (a phone on its side, a big window): the beep settings join the heading, so the
        // list keeps its rows on a short screen.
        let wide = width >= WIDE;
        row(ui, control, |ui| {
            // A row of their own, as high as they are: the badge would stretch to the row's height.
            ui.horizontal(|ui| {
                widgets::section_label(ui, "WATCHING");
                widgets::count_badge(ui, items.len());
            });
            if wide {
                if widgets::button(ui, "Clear all", Kind::Outline, !items.is_empty(), clear_size, "").clicked() {
                    self.clear();
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.test_button(ui, 96.0, control);
                    ui.add_space(4.0);
                    self.beep_settings(ui, control, true);
                });
            } else {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, "Clear all", Kind::Outline, !items.is_empty(), clear_size, "").clicked() {
                        self.clear();
                    }
                });
            }
        });

        let narrow = width < NARROW;
        let footer = if wide {
            0.0
        } else if narrow {
            2.0 * control + 2.0 * spacing.y
        } else {
            control + spacing.y
        };
        let height = (ui.available_height() - footer).max(3.0 * widgets::target(26.0, 44.0));
        self.list(ui, &items, height);

        if narrow {
            // A phone held upright: the settings, then Test across the window.
            row(ui, control, |ui| self.beep_settings(ui, control, false));
            ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                self.test_button(ui, ui.available_width(), control);
            });
        } else if !wide {
            row(ui, control, |ui| {
                self.beep_settings(ui, control, false);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.test_button(ui, 96.0, control)
                });
            });
        }
    }

    /// The watched items: a click picks rows (several, as VibeSkua's `SelectionMode="Multiple"`);
    /// the row's ✕, Delete or the right-click menu removes them.
    fn list(&mut self, ui: &mut Ui, items: &[String], height: f32) {
        if self.selection.picked().iter().any(|&i| i >= items.len()) {
            self.selection.clear();
        }
        let touch = widgets::touch();
        let row_height = widgets::target(26.0, 44.0);
        let mut remove: Option<Vec<usize>> = None;
        let mut clear = false;
        widgets::list_well(ui, height, |ui| {
            if items.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() / 2.0 - 24.0).max(4.0));
                    ui.label(RichText::new("Nothing watched yet").color(TEXT));
                    ui.label(RichText::new("Turu beeps when a listed item drops.").color(MUTED));
                    ui.label(RichText::new("Add exact item names above.").color(MUTED));
                });
                return;
            }
            let shown: Vec<usize> = (0..items.len()).collect();
            ScrollArea::vertical()
                .id_salt("notify_drop_list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, item) in items.iter().enumerate() {
                        let selected = self.selection.contains(i);
                        let row = widgets::list_row(ui, selected, row_height, |ui| {
                            widgets::dot(ui, MUTED, 3.0);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let hovered = ui.rect_contains_pointer(ui.max_rect());
                                if (hovered || selected || touch)
                                    && ui
                                        .add(egui::Button::new(RichText::new("✕").color(DANGER)).frame(false))
                                        .on_hover_text("Stop watching this item")
                                        .clicked()
                                {
                                    remove = Some(vec![i]);
                                }
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.add(Label::new(RichText::new(item).color(TEXT)).truncate())
                                        .on_hover_text(item);
                                });
                            });
                        });
                        if row.clicked() {
                            let shift = ui.input(|i| i.modifiers.shift);
                            self.selection.click(i, &shown, true, shift);
                        }
                        row.context_menu(|ui| {
                            if ui.button("Remove Selected").clicked() {
                                remove = Some(self.selection.picked());
                                ui.close();
                            }
                            if ui.button("Clear All").clicked() {
                                clear = true;
                                ui.close();
                            }
                        });
                    }
                });
        });
        if remove.is_none()
            && !self.selection.is_empty()
            && ui.input(|i| i.key_pressed(Key::Delete))
            && !ui.ctx().egui_wants_keyboard_input()
        {
            remove = Some(self.selection.picked());
        }
        if clear {
            self.clear();
        } else if let Some(rows) = remove {
            self.remove(&rows);
        }
    }

    /// "Beep [n] times every [ms]" (VibeSkua's `NumericUpDown`s: 1 to 25, 10 to 2000 ms), in a
    /// row the caller lays out; `reversed` for a right-to-left row.
    fn beep_settings(&self, ui: &mut Ui, control: f32, reversed: bool) {
        let watch = &self.watch;
        let order = if reversed { [3, 2, 1, 0] } else { [0, 1, 2, 3] };
        for part in order {
            match part {
                0 => {
                    ui.label(RichText::new("Beep").color(MUTED));
                }
                1 => {
                    let mut count = watch.count.load(Ordering::Relaxed);
                    let size = vec2(56.0, control);
                    let response = widgets::number(ui, "notify_drop_count", &mut count, 1..=25, 0.1, "", size)
                        .on_hover_text("How many beeps per drop");
                    if response.changed() {
                        watch.count.store(count, Ordering::Relaxed);
                    }
                }
                2 => {
                    ui.label(RichText::new("times every").color(MUTED));
                }
                _ => {
                    let mut delay = watch.delay.load(Ordering::Relaxed);
                    let size = vec2(84.0, control);
                    let response = widgets::number(ui, "notify_drop_delay", &mut delay, 10..=2000, 10.0, " ms", size)
                        .on_hover_text("Milliseconds between beeps");
                    if response.changed() {
                        watch.delay.store(delay, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    fn test_button(&self, ui: &mut Ui, width: f32, control: f32) {
        if widgets::button(ui, "Test beep", Kind::Normal, true, vec2(width, control), "")
            .on_hover_text("Play the alert")
            .clicked()
        {
            self.watch.play();
        }
    }

    /// VibeSkua `AddDrop`: `|`-separated, trimmed, names not in the list yet.
    fn add(&mut self) {
        let added = {
            let mut items = self.watch.lock();
            let before = items.len();
            for name in split(&self.input) {
                if !items.iter().any(|i| i == name) {
                    items.push(name.to_owned());
                }
            }
            items.len() > before
        };
        if added {
            self.subscribe();
        }
        self.input.clear();
    }

    /// VibeSkua `RemoveDrops`: unwatches when the list is empty after it.
    fn remove(&mut self, rows: &[usize]) {
        let empty = {
            let mut items = self.watch.lock();
            let mut i = 0;
            items.retain(|_| {
                i += 1;
                !rows.contains(&(i - 1))
            });
            items.is_empty()
        };
        self.selection.clear();
        if empty {
            self.unsubscribe();
        }
    }

    /// VibeSkua `RemoveAllDropsCommand` (the list's `Clear`).
    fn clear(&mut self) {
        self.watch.lock().clear();
        self.selection.clear();
        self.unsubscribe();
    }

    /// Watches this tab's bot: a listed item that drops (VibeSkua's `ItemDroppedMessage`, sent for
    /// the drop and again when it reaches the inventory) plays the beeps. The handler runs where
    /// the event is emitted (also inside `Bot::on_flash_call`): it only starts a thread.
    fn subscribe(&mut self) {
        if self.subscription.is_some() {
            return;
        }
        let bot = crate::turu::bot();
        let watch = Arc::downgrade(&self.watch);
        let id = bot.events.subscribe(move |event| {
            let (BotEvent::ItemDropped { item } | BotEvent::ItemAdded { item, .. }) = event else {
                return;
            };
            let Some(watch) = watch.upgrade() else { return };
            let listed = watch.lock().contains(&item.name);
            if listed {
                watch.play();
            }
        });
        self.subscription = Some((Arc::downgrade(&bot), id));
    }

    fn unsubscribe(&mut self) {
        if let Some((bot, id)) = self.subscription.take()
            && let Some(bot) = bot.upgrade()
        {
            bot.events.unsubscribe(id);
        }
    }
}

/// A row `height` tall across `ui`, its contents centred on it.
fn row<R>(ui: &mut Ui, height: f32, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(vec2(width, height), Layout::left_to_right(Align::Center), add_contents)
        .inner
}

/// The names in `input`: split on `|`, trimmed, empty ones dropped.
fn split(input: &str) -> impl Iterator<Item = &str> {
    input.split('|').map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_splits_on_bars_and_skips_duplicates() {
        let mut window = NotifyDropWindow::default();
        window.input = " Void Aura | | Void Aura|Gem of Mastery ".into();
        window.add();
        assert_eq!(*window.watch.lock(), ["Void Aura", "Gem of Mastery"]);
        assert!(window.input.is_empty());
        assert!(window.subscription.is_some());
        window.input = "Gem of Mastery|Dragon Runestone".into();
        window.add();
        assert_eq!(
            *window.watch.lock(),
            ["Void Aura", "Gem of Mastery", "Dragon Runestone"]
        );
        window.remove(&[0, 2]);
        assert_eq!(*window.watch.lock(), ["Gem of Mastery"]);
        assert!(window.subscription.is_some());
        window.remove(&[0]);
        assert!(window.subscription.is_none());
    }
}
