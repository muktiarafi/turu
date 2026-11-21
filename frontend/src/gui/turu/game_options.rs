//! Options > Game (VibeSkua's 612x720 window): a searchable list of the bot's options, laid out
//! as Application Options is (one column of [`widgets::setting_row`]s in section cards). Changes
//! are written to `bot.options` at once (the engine re-applies them every 250 ms); Save keeps
//! them in `options.json`.

use super::theme::{ACCENT_HOVER, DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
#[cfg(test)]
use crate::models::game_options::UNUSED_GROUP;
use crate::models::game_options::{
    self as model, Entry, FPS_PRESETS, Field, MAX_FPS, PRIORITIES, Section, UNUSED, Warning, defaults, differing,
    entries, matches, options, priority_label, revert_mine,
};
use egui::{ComboBox, Context, Layout, RichText, ScrollArea, Ui, vec2};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use turu_bot::options::BotOptions;

impl Field {
    /// The room the control takes at the row's right (the label and description get the rest).
    fn width(&self) -> f32 {
        match self {
            Field::Bool(_) => 48.0,
            Field::Int(..) => 96.0,
            // The number and its presets' ▾.
            Field::Fps => 96.0 + 6.0 + widgets::target(24.0, 40.0),
            Field::ReloadMap => 80.0,
            Field::Server | Field::Priority => 150.0,
            Field::Text(..) => 200.0,
        }
    }
}

/// A footer action that asks once before it changes many options at once.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirm {
    /// Defaults: puts every option back to its default.
    Defaults,
    /// Revert while a script runs and changed options: it would undo the script's options too.
    Revert,
}

/// A confirming click this soon after the one that asked is ignored: the confirm button takes the
/// asking button's place, so a double-click would otherwise confirm at once.
const CONFIRM_GUARD: Duration = Duration::from_millis(400);

/// How long "Saved to options.json." stays in the status line.
const SAVED_FOR: Duration = Duration::from_secs(4);

pub struct GameOptionsWindow {
    entries: Vec<Entry>,
    search: String,
    filter: Option<Section>,
    /// The Changed chip: only the options that differ from `options.json`.
    changed_only: bool,
    /// `options.json` and when it was read: read again every few seconds, as Application
    /// Options saves two of its options too.
    saved: Option<(BotOptions, Instant)>,
    /// The footer action waiting for its confirming click, and when it asked.
    confirm: Option<(Confirm, Instant)>,
    /// What the last Save did and when, until the options change again ("Saved" fades).
    save_result: Option<(Result<(), String>, Instant)>,
    /// How tall the footer was last frame: the list leaves exactly that much room.
    footer_height: Option<f32>,
    /// The options changed here since the last Save or Revert (by label): what differs from
    /// `options.json` and isn't here was changed by something else, most often a script.
    edited: HashSet<&'static str>,
    /// The switch whose [warning](Entry::warn_off) is up (Private Rooms going off), by label.
    ask_off: Option<&'static str>,
}

impl Default for GameOptionsWindow {
    fn default() -> Self {
        Self {
            entries: entries(),
            search: String::new(),
            filter: None,
            changed_only: false,
            saved: None,
            confirm: None,
            save_result: None,
            footer_height: None,
            edited: HashSet::new(),
            ask_off: None,
        }
    }
}

impl GameOptionsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            self.saved = None;
            self.confirm = None;
            self.save_result = None;
            self.ask_off = None;
            return;
        }
        // VibeSkua's size; on a phone a full-screen sheet (`super::window`), so the footer's
        // buttons stay on screen.
        super::window(ctx, "Game Options", [612.0, 720.0], 1)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        self.warning(ctx);
    }

    /// The question a switch asks before it goes off (Private Rooms: [`model::PUBLIC_ROOMS`]);
    /// the option changes only on its confirm.
    fn warning(&mut self, ctx: &Context) {
        let Some(label) = self.ask_off else { return };
        let Some(entry) = self.entries.iter().find(|e| e.label == label) else {
            self.ask_off = None;
            return;
        };
        let (Some(warning), Field::Bool(access)) = (entry.warn_off, &entry.field) else {
            self.ask_off = None;
            return;
        };
        match warning_modal(ctx, egui::Id::new(("game_options_warn_off", label)), warning) {
            // Escape or a click outside keeps the option as it is.
            Some(Answer::Keep | Answer::Dismiss) => self.ask_off = None,
            Some(Answer::Confirm) => {
                self.ask_off = None;
                *access(&mut crate::turu::bot().options.write()) = false;
                self.edited.insert(label);
            }
            None => {}
        }
    }

    /// Whether the list shows `entry`: the search finds it, and it differs from the saved
    /// options when the Changed chip is on (the section chip is checked apart).
    fn shown(&self, entry: &Entry, needle: &str, differs: &[&str]) -> bool {
        matches(entry, needle) && (!self.changed_only || differs.contains(&entry.label))
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();

        ui.spacing_mut().item_spacing = vec2(6.0, 10.0);
        let mut snapshot = bot.options.read().clone();
        let saved = self.saved_options(&bot);
        let differs = differing(&self.entries, &snapshot, &saved);
        // Changed here, then back (or saved elsewhere): no longer an edit of ours.
        self.edited.retain(|label| differs.contains(label));
        // Nothing differs any more (saved, reverted): the Changed chip has nothing to show.
        if differs.is_empty() {
            self.changed_only = false;
        }

        let needle = self.search.trim().to_lowercase();
        // The chips count what the search (and the Changed chip) find in each section.
        let counts = Section::ALL.map(|section| {
            self.entries
                .iter()
                .filter(|e| e.section == section && self.shown(e, &needle, &differs))
                .count()
        });
        // Fixed above the list at every height, as in the other options windows.
        self.search_bar(ui, counts, differs.len());

        let footer = self.footer_height.unwrap_or(widgets::target(64.0, 76.0)) + ui.spacing().item_spacing.y;
        let list_height = (ui.available_height() - footer).max(80.0);
        ScrollArea::vertical()
            .id_salt("game_options_list")
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let shown: usize = Section::ALL
                    .into_iter()
                    .zip(counts)
                    .filter(|(section, _)| self.filter.is_none_or(|f| f == *section))
                    .map(|(_, count)| count)
                    .sum();
                if shown == 0 {
                    self.empty_state(ui, &differs);
                }
                for section in Section::ALL {
                    if self.filter.is_some_and(|f| f != section) {
                        continue;
                    }
                    let rows: Vec<&Entry> = self
                        .entries
                        .iter()
                        .filter(|e| e.section == section && self.shown(e, &needle, &differs))
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    widgets::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        // The heading only when several sections show (VibeSkua's
                        // `ShowSectionHeaders`); a chip already names the one shown.
                        if self.filter.is_none() {
                            ui.horizontal(|ui| {
                                widgets::section_label(ui, &section.title().to_uppercase());
                                widgets::count_badge(ui, rows.len());
                            });
                            ui.add_space(2.0);
                        }
                        if section == Section::Drops && snapshot.accept_all_drops && snapshot.reject_all_drops {
                            ui.label(
                                RichText::new(
                                    "Accept All Drops and Reject All Drops are both on: Accept All wins, so every \
                                     drop is picked up.",
                                )
                                .small()
                                .color(WARNING),
                            );
                        }
                        // Differs from options.json: changed here, or by something else.
                        let edited = &self.edited;
                        let mark = |entry: &Entry| {
                            differs.contains(&entry.label).then(|| {
                                if edited.contains(entry.label) {
                                    Mark::Edited
                                } else {
                                    Mark::Outside
                                }
                            })
                        };
                        // One column, as Application Options: the controls line up at the right.
                        let mut changed = Vec::new();
                        for (i, run) in rows.chunk_by(|a, b| a.group == b.group).enumerate() {
                            let group = run[0].group;
                            if !group.is_empty() {
                                if i > 0 {
                                    ui.add_space(4.0);
                                }
                                ui.label(RichText::new(group).small().strong().color(MUTED));
                            }
                            for entry in run {
                                if row(ui, entry, mark(entry), &mut snapshot, &bot, &mut self.ask_off) {
                                    changed.push(entry.label);
                                }
                            }
                        }
                        self.edited.extend(changed);
                    });
                }
            });

        let live = bot.options.read().clone();
        if live != snapshot {
            // An option changed (here or by a script): a pending confirm no longer means what it said.
            self.confirm = None;
        }
        let top = ui.cursor().top();
        ui.separator();
        self.footer(ui, &bot, live, saved);
        self.footer_height = Some(ui.min_rect().bottom() - top);
    }

    /// The search box and the chips: the sections, which count what the search finds, and
    /// Changed (`changed` options differ from `options.json`) while anything differs.
    fn search_bar(&mut self, ui: &mut Ui, counts: [usize; 5], changed: usize) {
        let width = ui.available_width();
        widgets::text_box(ui, &mut self.search, "Search game options", width);
        let mut filter = self.filter;
        let mut changed_only = self.changed_only;
        ui.horizontal_wrapped(|ui| {
            let all = counts.iter().sum::<usize>();
            if widgets::chip(ui, &format!("All  {all}"), filter.is_none()) {
                filter = None;
            }
            for (section, count) in Section::ALL.into_iter().zip(counts) {
                let selected = filter == Some(section);
                // A section with nothing to show stays dimmed (unless it's the one shown).
                let label = format!("{}  {count}", section.chip());
                if widgets::chip_enabled(ui, &label, selected, count > 0 || selected) {
                    filter = Some(section);
                }
            }
            if changed > 0 {
                ui.add_space(6.0);
                if widgets::chip(ui, &format!("Changed  {changed}"), changed_only) {
                    changed_only = !changed_only;
                }
            }
        });
        self.filter = filter;
        self.changed_only = changed_only;
    }

    /// What the list shows when nothing matches: says so, and offers a way out.
    fn empty_state(&mut self, ui: &mut Ui, differs: &[&str]) {
        let search = self.search.trim().to_owned();
        let needle = search.to_lowercase();
        let changed = if self.changed_only { "changed " } else { "" };
        let scope = match self.filter {
            Some(section) => format!("{changed}{} options", section.chip()),
            None => format!("{changed}game options"),
        };
        let text = if search.is_empty() {
            format!("No {scope}.")
        } else {
            format!("No {scope} match \"{search}\".")
        };
        // Another section has matches: offer to look in them all.
        let elsewhere = self.filter.is_some() && self.entries.iter().any(|e| self.shown(e, &needle, differs));
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(text).color(TEXT));
            ui.add_space(4.0);
            let size = vec2(0.0, widgets::target(28.0, 40.0));
            let all_sections = if search.is_empty() {
                "Show all sections"
            } else {
                "Search all sections"
            };
            if elsewhere && widgets::button(ui, all_sections, Kind::Normal, true, size, "").clicked() {
                self.filter = None;
            }
            if self.changed_only && widgets::button(ui, "Show all options", Kind::Normal, true, size, "").clicked() {
                self.changed_only = false;
            }
            if !search.is_empty() && widgets::button(ui, "Clear search", Kind::Normal, true, size, "").clicked() {
                self.search.clear();
            }
        });
    }

    /// `options.json` as last read (the defaults when there's none, as the bot starts with).
    fn saved_options(&mut self, bot: &turu_bot::Bot) -> BotOptions {
        let stale = self
            .saved
            .as_ref()
            .is_none_or(|(_, read)| read.elapsed() > Duration::from_secs(2));
        if stale {
            self.saved = Some((bot.paths.load_options().unwrap_or_default(), Instant::now()));
        }
        self.saved.as_ref().map(|(o, _)| o.clone()).unwrap_or_default()
    }

    /// Whether a confirming click on `what` counts: it was asked for, not just now.
    fn confirmed(&self, what: Confirm) -> bool {
        self.confirm
            .is_some_and(|(asked, at)| asked == what && at.elapsed() >= CONFIRM_GUARD)
    }

    /// The status line, then Defaults (asks once), Revert (asks once while a script runs and
    /// changed options) and Save; Revert and Save only when the live options differ from the
    /// saved ones.
    fn footer(&mut self, ui: &mut Ui, bot: &std::sync::Arc<turu_bot::Bot>, live: BotOptions, saved: BotOptions) {
        let unsaved = live != saved;
        if let Some((Ok(()), at)) = &self.save_result {
            if unsaved || at.elapsed() >= SAVED_FOR {
                self.save_result = None;
            } else {
                ui.ctx().request_repaint_after(SAVED_FOR - at.elapsed());
            }
        }
        let reset = defaults(&live);
        let to_reset = differing(&self.entries, &live, &reset).len();
        let running = bot.scripts.is_running();
        let differs = differing(&self.entries, &live, &saved);
        let mine = differs.iter().filter(|l| self.edited.contains(*l)).count();
        let outside = differs.len() - mine;
        // What a pending confirm was about is gone: nothing to reset, the script stopped, or
        // nothing it changed is left.
        match self.confirm {
            Some((Confirm::Defaults, _)) if to_reset == 0 => self.confirm = None,
            Some((Confirm::Revert, _)) if !running || outside == 0 => self.confirm = None,
            _ => {}
        }
        let asking = self.confirm.map(|(what, _)| what);

        let (text, color) = match asking {
            Some(Confirm::Defaults) => (
                format!(
                    "Reset puts {} back to {} default now, also under a running script.",
                    options(to_reset),
                    if to_reset == 1 { "its" } else { "their" }
                ),
                DANGER,
            ),
            Some(Confirm::Revert) => {
                let yours = match mine {
                    0 => String::new(),
                    _ => format!(" Revert mine puts back only the {} you changed.", options(mine)),
                };
                (
                    format!(
                        "A script is running: Revert all also undoes the {} changed outside this window, most \
                         likely by it.{yours}",
                        options(outside)
                    ),
                    DANGER,
                )
            }
            None => match &self.save_result {
                Some((Err(e), _)) => (format!("Couldn't save options.json: {e}"), DANGER),
                _ if unsaved => (self.unsaved_status(&live, &saved, running), WARNING),
                Some((Ok(()), _)) => ("Saved to options.json.".to_owned(), SUCCESS),
                None => ("Changes apply now; Save keeps them for next time.".to_owned(), MUTED),
            },
        };
        ui.add(egui::Label::new(RichText::new(text).small().color(color)).wrap());

        let size = vec2(80.0, widgets::target(28.0, 40.0));
        ui.horizontal(|ui| match asking {
            Some(Confirm::Revert) => {
                if widgets::button(ui, "Revert all", Kind::Danger, true, size, "").clicked()
                    && self.confirmed(Confirm::Revert)
                {
                    *bot.options.write() = saved.clone();
                    self.edited.clear();
                    self.confirm = None;
                    self.save_result = None;
                }
                if mine > 0
                    && widgets::button(ui, "Revert mine", Kind::Normal, true, size, "").clicked()
                    && self.confirmed(Confirm::Revert)
                {
                    let mut options = bot.options.write();
                    *options = revert_mine(&self.entries, &self.edited, &options, &saved);
                    self.edited.clear();
                    self.confirm = None;
                    self.save_result = None;
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                    self.confirm = None;
                }
            }
            Some(Confirm::Defaults) => {
                if widgets::button(
                    ui,
                    &format!("Reset {}", options(to_reset)),
                    Kind::Danger,
                    true,
                    size,
                    "",
                )
                .clicked()
                    && self.confirmed(Confirm::Defaults)
                {
                    // Defaults is an edit made here, of every option it changes.
                    self.edited.extend(differing(&self.entries, &live, &reset));
                    *bot.options.write() = reset.clone();
                    self.confirm = None;
                    self.save_result = None;
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                    self.confirm = None;
                }
                self.save_button(ui, bot, unsaved, size);
            }
            None => {
                if widgets::button(
                    ui,
                    "Defaults",
                    Kind::Normal,
                    to_reset > 0,
                    size,
                    "Every option is at its default",
                )
                .clicked()
                {
                    self.confirm = Some((Confirm::Defaults, Instant::now()));
                }
                if widgets::button(
                    ui,
                    "Revert",
                    Kind::Normal,
                    unsaved,
                    size,
                    "Nothing differs from the saved options",
                )
                .clicked()
                {
                    if running && outside > 0 {
                        // It would undo what the running script set: ask first.
                        self.confirm = Some((Confirm::Revert, Instant::now()));
                    } else {
                        *bot.options.write() = saved.clone();
                        self.edited.clear();
                        self.save_result = None;
                    }
                }
                self.save_button(ui, bot, unsaved, size);
            }
        });
    }

    /// Save, at the footer's right.
    fn save_button(&mut self, ui: &mut Ui, bot: &turu_bot::Bot, unsaved: bool, size: egui::Vec2) {
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::button(ui, "Save", Kind::Primary, unsaved, size, "Nothing to save").clicked() {
                self.confirm = None;
                let result = bot.save_options().map_err(|e| e.to_string());
                if result.is_ok() {
                    self.edited.clear();
                }
                self.save_result = Some((result, Instant::now()));
                self.saved = None;
            }
        });
    }
}

impl GameOptionsWindow {
    /// The status line while the live options differ from `options.json` (`model::unsaved_status`).
    fn unsaved_status(&self, live: &BotOptions, saved: &BotOptions, running: bool) -> String {
        model::unsaved_status(&self.entries, &self.edited, live, saved, running)
    }
}

/// Why a row differs from `options.json`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// Changed in this window.
    Edited,
    /// Changed by something else, most often a script.
    Outside,
}

/// How a [`warning_modal`] was answered.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Answer {
    /// Its danger button ("Use public rooms").
    Confirm,
    /// Its other button ("Keep private").
    Keep,
    /// Escape or a click outside: no answer.
    Dismiss,
}

/// `warning` as a modal: its title, its text, then its danger button and its other button. Game
/// Options and CoreBots Options ask about public rooms with it ([`model::PUBLIC_ROOMS`]), so the
/// question looks and reads the same in both, as Android's `PublicRoomsDialog` does.
pub(super) fn warning_modal(ctx: &Context, id: egui::Id, warning: &Warning) -> Option<Answer> {
    let mut answer = None;
    // Each button as wide as its words (`min_size`), finger-tall on a touch screen.
    let size = vec2(96.0, widgets::target(28.0, 44.0));
    let response = egui::Modal::new(id).show(ctx, |ui| {
        ui.set_max_width(380.0);
        ui.heading(warning.title);
        ui.label(warning.text);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, warning.confirm, Kind::Danger, true, size, "").clicked() {
                answer = Some(Answer::Confirm);
            }
            if widgets::button(ui, warning.cancel, Kind::Normal, true, size, "").clicked() {
                answer = Some(Answer::Keep);
            }
        });
    });
    if response.should_close() && answer.is_none() {
        answer = Some(Answer::Dismiss);
    }
    answer
}

/// One option's row ([`widgets::setting_row`]): its label, `mark` and description, with its
/// control at the right (under them on a phone, when they'd be too narrow beside it). An option
/// that [requires](Entry::requires) another is indented, and dimmed while that one is off.
/// Returns whether the control changed the option.
fn row(
    ui: &mut Ui,
    entry: &Entry,
    mark: Option<Mark>,
    snapshot: &mut BotOptions,
    bot: &std::sync::Arc<turu_bot::Bot>,
    ask_off: &mut Option<&'static str>,
) -> bool {
    let mut changed = false;
    // Some(whether to dim) for an option that requires another.
    let dim = entry.requires.map(|requires| !requires(snapshot));
    let mut draw = |ui: &mut Ui| {
        widgets::setting_row(
            ui,
            |ui| title(ui, entry, mark),
            entry.description,
            entry.field.width(),
            |ui| changed = control(ui, entry, snapshot, bot, ask_off),
        );
    };
    match dim {
        Some(dim) => {
            ui.indent(entry.label, |ui| {
                if dim {
                    ui.multiply_opacity(0.55);
                }
                draw(ui);
            });
        }
        None => draw(ui),
    }
    changed
}

/// An option's label; `mark`, a tag after it saying why it differs from options.json ("Edited"
/// here, or "Changed outside" by something else).
fn title(ui: &mut Ui, entry: &Entry, mark: Option<Mark>) {
    ui.label(RichText::new(entry.label).color(TEXT));
    let Some(mark) = mark else { return };
    let (text, color, why) = match mark {
        Mark::Edited => (
            "Edited",
            ACCENT_HOVER,
            "You changed this here: it applies now, Save keeps it, Revert puts it back.",
        ),
        Mark::Outside => (
            "Changed outside",
            WARNING,
            "Changed outside this window, most likely by a script: it differs from options.json.",
        ),
    };
    // Outlined on the card, not filled: on the grey pill the accent text would be under 4:1.
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.6)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(widgets::small_label_size()).color(color))
        })
        .inner
        .on_hover_text(why);
}

/// `entry`'s control, greyed for an [`UNUSED`] option; returns whether it changed the option.
/// A switch with a [warning](Entry::warn_off) that the user turns off stays on and sets `ask_off`.
fn control(
    ui: &mut Ui,
    entry: &Entry,
    snapshot: &mut BotOptions,
    bot: &std::sync::Arc<turu_bot::Bot>,
    ask_off: &mut Option<&'static str>,
) -> bool {
    if UNUSED.contains(&entry.label) {
        ui.add_enabled_ui(false, |ui| edit(ui, entry.label, &entry.field, snapshot, bot, None))
            .response
            .on_disabled_hover_text("Coming soon: Turu doesn't use this yet");
        return false;
    }
    let warn = entry.warn_off.is_some().then_some(&mut *ask_off);
    edit(
        ui,
        entry.label,
        &entry.field,
        snapshot,
        bot,
        warn.map(|a| (entry.label, a)),
    )
}

/// One option's control. Writes to the live options as soon as it changes, and returns whether
/// it did.
fn edit(
    ui: &mut Ui,
    label: &str,
    field: &Field,
    snapshot: &mut BotOptions,
    bot: &std::sync::Arc<turu_bot::Bot>,
    ask_off: Option<(&'static str, &mut Option<&'static str>)>,
) -> bool {
    let mut changed = false;
    // Combo boxes are as tall as `interact_size`: finger-sized on a touch screen.
    ui.spacing_mut().interact_size.y = widgets::target(24.0, 40.0);
    match field {
        Field::Bool(access) => {
            let mut value = *access(snapshot);
            if widgets::switch(ui, &mut value).changed() {
                match ask_off {
                    // Turu: asks before Private Rooms goes off; `GameOptionsWindow::warning` turns it off.
                    Some((label, ask)) if !value => *ask = Some(label),
                    _ => {
                        *access(&mut bot.options.write()) = value;
                        changed = true;
                    }
                }
            }
        }
        Field::Int(access, min, suffix) => {
            let mut value = *access(snapshot);
            let speed = (value.abs() as f64 / 200.0).max(1.0);
            let suffix = suffix.map(|s| format!(" {s}")).unwrap_or_default();
            let size = vec2(96.0, widgets::target(24.0, 40.0));
            if widgets::number(ui, label, &mut value, *min..=i32::MAX, speed, &suffix, size).changed() {
                *access(&mut bot.options.write()) = value;
                changed = true;
            }
        }
        Field::Fps => {
            let mut value = snapshot.set_fps;
            let format = widgets::NumberFormat {
                suffix: " FPS",
                zero: Some("Off"),
                ..Default::default()
            };
            let size = vec2(96.0, widgets::target(24.0, 40.0));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if widgets::number(ui, label, &mut value, 0..=MAX_FPS, 1.0, format, size).changed() {
                    bot.options.write().set_fps = value;
                    changed = true;
                }
                let side = widgets::target(24.0, 40.0);
                ui.scope(|ui| {
                    ui.spacing_mut().interact_size = vec2(side, side);
                    ui.menu_button("▾", |ui| {
                        for fps in FPS_PRESETS {
                            let text = if fps == 0 {
                                "Off".to_owned()
                            } else {
                                format!("{fps} FPS")
                            };
                            let button = egui::Button::selectable(snapshot.set_fps == fps, text)
                                .min_size(vec2(120.0, widgets::target(24.0, 40.0)));
                            if ui.add(button).clicked() {
                                bot.options.write().set_fps = fps;
                                changed = true;
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text("The usual frame rates");
                });
            });
        }
        Field::Text(access, hint) => {
            let mut value = access(snapshot).clone();
            if widgets::text_box(ui, &mut value, hint, 200.0).changed() {
                *access(&mut bot.options.write()) = value;
                changed = true;
            }
        }
        Field::Server => {
            if let Some(name) = widgets::server_combo(ui, "relogin_server", bot, &snapshot.relogin_server) {
                bot.options.write().relogin_server = name;
                changed = true;
            }
        }
        Field::Priority => {
            let current = snapshot.hunt_priority;
            ComboBox::from_id_salt("hunt_priority")
                .selected_text(priority_label(current))
                .width(150.0)
                .show_ui(ui, |ui| {
                    ui.spacing_mut().interact_size.y = widgets::target(24.0, 40.0);
                    for choice in PRIORITIES {
                        if ui.selectable_label(current == choice, priority_label(choice)).clicked() {
                            bot.options.write().hunt_priority = choice;
                            changed = true;
                        }
                    }
                });
        }
        Field::ReloadMap => {
            let in_game = bot.state.lock().in_game;
            let size = vec2(80.0, widgets::target(24.0, 40.0));
            if widgets::button(ui, "Reload", Kind::Normal, in_game, size, "Log in first").clicked() {
                model::reload_map(bot);
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(needle: &str) -> Vec<&'static str> {
        entries()
            .into_iter()
            .filter(|e| matches(e, needle))
            .map(|e| e.label)
            .collect()
    }

    /// A section title's word finds its section ("network": Relogin), but "&" or a word inside
    /// a title finds only what says it, not every option.
    #[test]
    fn search_matches_title_words_not_fragments() {
        let all = entries().len();
        assert_eq!(found("").len(), all);
        assert!(found("&").is_empty());
        assert!(found("and").len() < all);
        let relogin = entries().iter().filter(|e| e.section == Section::Relogin).count();
        assert!(found("network").len() >= relogin);
        assert!(found("combat").contains(&"Magnetise"));
        assert!(found("headless").contains(&"Headless Mode"));
    }

    /// The options that differ from the saved ones, in the window's order; Reload Map never.
    #[test]
    fn differing_lists_changed_options() {
        let entries = entries();
        let saved = BotOptions::default();
        let mut live = saved.clone();
        assert!(differing(&entries, &live, &saved).is_empty());
        live.hide_players = true;
        live.hunt_delay = 500;
        live.relogin_server = "Artix".into();
        live.relogin_try_delay = 5000; // Application Options' option: not in this window.
        assert_eq!(
            differing(&entries, &live, &saved),
            ["Hide Players", "Hunt Delay", "Relogin Server"]
        );
    }

    /// The status says which changes were made here and which elsewhere (a script).
    #[test]
    fn unsaved_status_tells_whose_changes() {
        let mut window = GameOptionsWindow::default();
        let saved = BotOptions::default();
        let mut live = saved.clone();
        live.hide_players = true;
        window.edited.insert("Hide Players");
        assert_eq!(
            window.unsaved_status(&live, &saved, false),
            "You changed 1 option: it applies now; Save keeps it for next time."
        );
        live.aggro_monsters = true;
        live.skip_cutscenes = true;
        live.accept_all_drops = true;
        assert_eq!(
            window.unsaved_status(&live, &saved, false),
            "Aggro Monsters, Accept All Drops and 1 more were changed outside this window, most likely by a \
             script. You changed 1 option. Save keeps them too; Revert puts the saved options back."
        );
        assert!(
            window
                .unsaved_status(&live, &saved, true)
                .ends_with("Revert asks first while the script runs.")
        );
    }

    /// Revert mine puts back only the options changed here; a script's stay.
    #[test]
    fn revert_mine_keeps_outside_changes() {
        let entries = entries();
        let saved = BotOptions::default();
        let mut live = saved.clone();
        live.hide_players = true; // ours
        live.custom_name = "Me".into(); // ours
        live.aggro_monsters = true; // the script's
        let edited = HashSet::from(["Hide Players", "Custom Name"]);
        let reverted = revert_mine(&entries, &edited, &live, &saved);
        assert_eq!(differing(&entries, &reverted, &saved), ["Aggro Monsters"]);
    }

    /// Unused options come last in their section, under their own heading; Private Number
    /// follows Private Rooms, which it requires.
    #[test]
    fn only_private_rooms_asks_before_going_off() {
        let asking: Vec<&str> = entries()
            .iter()
            .filter(|e| e.warn_off.is_some())
            .map(|e| e.label)
            .collect();
        assert_eq!(asking, ["Private Rooms"]);
        let entries = entries();
        let entry = entries.iter().find(|e| e.label == "Private Rooms").unwrap();
        assert!(matches!(entry.field, Field::Bool(_)));
        assert!(entry.warn_off.unwrap().text.contains("report"));
    }

    #[test]
    fn entries_order() {
        let list = entries();
        for section in Section::ALL {
            let rows: Vec<&Entry> = list.iter().filter(|e| e.section == section).collect();
            let first_unused = rows
                .iter()
                .position(|e| UNUSED.contains(&e.label))
                .unwrap_or(rows.len());
            assert!(rows[first_unused..].iter().all(|e| e.group == UNUSED_GROUP));
            assert!(rows[..first_unused].iter().all(|e| e.group != UNUSED_GROUP));
        }
        // Sections stay together, in the chips' order.
        let order: Vec<usize> = list.iter().map(|e| e.section as usize).collect();
        assert!(order.is_sorted());
        let at = |label| list.iter().position(|e| e.label == label).unwrap();
        assert_eq!(at("Private Number"), at("Private Rooms") + 1);
        assert!(list[at("Private Number")].requires.is_some());
        assert_eq!(list[at("Hunt Priority")].section as usize, Section::Combat as usize);
    }

    /// Every option the window greys out is one it has (a renamed label would leave it live).
    #[test]
    fn unused_options_exist() {
        let labels: Vec<&str> = entries().iter().map(|e| e.label).collect();
        for label in UNUSED {
            assert!(labels.contains(&label), "{label} isn't an option");
        }
    }
}
