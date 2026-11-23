//! Scripts > Scheduler (VibeSkua `ScriptSchedulerView`): the playlist, run top to bottom. Start /
//! Stop queue with the summary, Load / Save / Add scripts; a table of #, script, status and
//! time, each row's actions (Move up, Move down, Script options, Remove) in its ⋮ menu and on
//! right-click; double-click a name to rename it. The playlist and its runs are the tab's
//! `bot.scheduler`; what it shows and may do is `models::scheduler`'s.

use super::files::Pending;
use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{Align, Color32, Context, Key, Label, Layout, RichText, ScrollArea, Sense, TextEdit, Ui, vec2};

use crate::models::scheduler::{self as model, EntryView, Tint};
use crate::host::FileDialog;
use std::time::Duration;
use turu_bot::script::options::ScriptOptions;

/// The table's columns, as VibeSkua's: #, SCRIPT (the rest), STATUS, TIME, the menu.
const NUMBER: f32 = 40.0;
const STATUS: f32 = 120.0;
const TIME: f32 = 100.0;
const MENU: f32 = 44.0;
const ROW: f32 = 38.0;

#[derive(Default)]
pub struct SchedulerWindow {
    /// The selected entry's id.
    selected: Option<String>,
    /// The entry being renamed: its id, the text, the name before.
    renaming: Option<(String, String, String)>,
    focus_rename: bool,
    want_load: bool,
    want_save: bool,
    load: Pending,
    save: Pending,
    /// Script options was clicked and its options are read: `TuruUi` opens the Script Options
    /// window on them.
    pub edit_options: Option<ScriptOptions>,
    /// "Add scripts": `TuruUi` opens Search Scripts.
    pub open_repo: bool,
}

enum Action {
    Move(String, isize),
    Options(String),
    Remove(String),
}

/// A tint in this palette.
pub fn color(tint: Tint) -> Color32 {
    match tint {
        Tint::Plain => TEXT,
        Tint::Muted => MUTED,
        Tint::Success => SUCCESS,
        Tint::Warning => WARNING,
        Tint::Danger => DANGER,
    }
}

impl SchedulerWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        self.poll_options();
        if !*open {
            return;
        }
        super::window(ctx, "Scheduler", [640.0, 460.0], 7)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        if crate::turu::bot().scheduler.is_running() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// The Load and Save dialogs (not drawn in the window).
    pub fn files(&mut self, ctx: &Context) {
        let bot = crate::turu::bot();
        let folder = bot.paths.playlists_dir();
        let _ = std::fs::create_dir_all(&folder);
        if std::mem::take(&mut self.want_load) {
            self.load.pick_file(ctx, json_dialog().set_directory(&folder));
        }
        if std::mem::take(&mut self.want_save) {
            self.save
                .save_file(ctx, json_dialog().set_directory(&folder).set_file_name("playlist.json"));
        }
        if let Some(path) = self.load.poll() {
            self.selected = None;
            model::load_file(&bot, &path);
        }
        if let Some(path) = self.save.poll() {
            model::save_file(&bot, &path);
        }
    }

    /// Script options' read (from this window or a shell's page): the Script Options window opens.
    fn poll_options(&mut self) {
        if let Some(options) = model::take_read(&crate::turu::bot()) {
            self.edit_options = Some(options);
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let view = model::view(&bot);
        let running = view.running;
        let entries = &view.entries;

        ui.horizontal(|ui| {
            let (label, kind) = if running {
                ("■  Stop queue", Kind::Danger)
            } else {
                ("▶  Start queue", Kind::Primary)
            };
            if widgets::button(
                ui,
                label,
                kind,
                view.toggle.enabled,
                vec2(140.0, 28.0),
                &view.toggle.reason,
            )
            .clicked()
            {
                model::toggle(&bot);
            }
            ui.add(Label::new(RichText::new(&view.summary).small().color(MUTED)).truncate());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "+  Add scripts", Kind::Normal, true, vec2(140.0, 28.0), "")
                    .on_hover_text("Queue scripts from Search Scripts")
                    .clicked()
                {
                    self.open_repo = true;
                }
                let can_save = view.save.enabled && !self.save.is_open();
                if widgets::button(ui, "Save", Kind::Normal, can_save, vec2(84.0, 28.0), &view.save.reason)
                    .on_hover_text("Save this playlist")
                    .clicked()
                {
                    self.want_save = true;
                }
                let can_load = view.load.enabled && !self.load.is_open();
                if widgets::button(ui, "Load", Kind::Normal, can_load, vec2(84.0, 28.0), &view.load.reason)
                    .on_hover_text("Load a saved playlist")
                    .clicked()
                {
                    self.want_load = true;
                }
            });
        });
        if let Some(note) = &view.note {
            ui.label(RichText::new(&note.text).small().color(color(note.tint)));
        }

        let footer = 26.0;
        let height = (ui.available_height() - footer).max(120.0);
        let mut action = None;
        widgets::list_well(ui, height, |ui| {
            header(ui);
            if entries.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("The playlist is empty. Add scripts to queue them.").color(MUTED));
                });
                return;
            }
            ScrollArea::vertical()
                .id_salt("scheduler_queue")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, entry) in entries.iter().enumerate() {
                        if let Some(a) = self.row(ui, i, entry, entries.len()) {
                            action = Some(a);
                        }
                    }
                });
        });
        ui.label(RichText::new("Double-click a name to rename it").small().color(MUTED));

        match action {
            Some(Action::Move(id, by)) => {
                self.selected = Some(id.clone());
                model::move_by(&bot, &id, by);
            }
            Some(Action::Remove(id)) => model::remove(&bot, &id),
            Some(Action::Options(id)) => {
                model::options(&bot, &id);
                // The read ends on another thread: look again soon.
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
            None => {}
        }
    }

    fn row(&mut self, ui: &mut Ui, index: usize, entry: &EntryView, count: usize) -> Option<Action> {
        let selected = self.selected.as_deref() == Some(entry.id.as_str()) || entry.current;
        let mut action = None;
        let row = widgets::list_row(ui, selected, ROW, |ui| {
            ui.add_sized(
                [NUMBER - 8.0, 18.0],
                Label::new(RichText::new((index + 1).to_string()).size(11.0).color(MUTED)),
            );
            let name_width = (ui.available_width() - STATUS - TIME - MENU).max(60.0);
            ui.allocate_ui_with_layout(vec2(name_width, ROW), Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(12.0);
                self.name(ui, entry);
            });
            ui.allocate_ui_with_layout(vec2(STATUS, ROW), Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(12.0);
                status_chip(ui, entry);
            });
            ui.allocate_ui_with_layout(vec2(TIME, ROW), Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                let color = if entry.tint == Tint::Muted { MUTED } else { TEXT };
                ui.label(RichText::new(&entry.time).monospace().size(11.0).color(color));
            });
            ui.allocate_ui_with_layout(
                vec2(MENU, ROW),
                Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    ui.menu_button(RichText::new("⋮").color(MUTED), |ui| {
                        action = menu(ui, index, entry, count);
                    })
                    .response
                    .on_hover_text("Move, options, remove");
                },
            );
        });
        row.context_menu(|ui| {
            if let Some(a) = menu(ui, index, entry, count) {
                action = Some(a);
            }
        });
        if row.clicked() {
            self.selected = Some(entry.id.clone());
        }
        action
    }

    /// The name, or its text box while renaming (Enter or leaving the box ends it, Escape undoes it).
    fn name(&mut self, ui: &mut Ui, entry: &EntryView) {
        if let Some((id, text, before)) = &mut self.renaming
            && *id == entry.id
        {
            let mut response = ui.add(TextEdit::singleline(&mut *text).desired_width(ui.available_width() - 12.0));
            super::native_field::follow(ui, &mut response, text, "", super::native_field::Kind::Line);
            if std::mem::take(&mut self.focus_rename) {
                response.request_focus();
            }
            let escape = ui.input(|i| i.key_pressed(Key::Escape));
            if escape {
                *text = before.clone();
            }
            if escape || response.lost_focus() || ui.input(|i| i.key_pressed(Key::Enter)) {
                model::rename(&crate::turu::bot(), id, text);
                self.renaming = None;
            }
            return;
        }
        let color = if entry.tint == Tint::Muted { MUTED } else { TEXT };
        let name = ui
            .add(
                Label::new(RichText::new(&entry.name).strong().color(color))
                    .truncate()
                    .sense(Sense::click()),
            )
            .on_hover_text(&entry.path);
        if name.double_clicked() {
            self.renaming = Some((entry.id.clone(), entry.name.clone(), entry.name.clone()));
            self.focus_rename = true;
        } else if name.clicked() {
            self.selected = Some(entry.id.clone());
        }
    }
}

/// The table's header, on the columns of the rows.
fn header(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.set_height(26.0);
        ui.add_space(8.0);
        ui.add_sized([NUMBER - 8.0, 18.0], Label::new(column("#")));
        let name_width = (ui.available_width() - STATUS - TIME - MENU - 8.0).max(60.0);
        ui.allocate_ui_with_layout(vec2(name_width, 18.0), Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(12.0);
            ui.label(column("SCRIPT"));
        });
        ui.allocate_ui_with_layout(vec2(STATUS, 18.0), Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(12.0);
            ui.label(column("STATUS"));
        });
        ui.allocate_ui_with_layout(vec2(TIME, 18.0), Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);
            ui.label(column("TIME"));
        });
    });
    ui.separator();
}

fn column(text: &str) -> RichText {
    RichText::new(text).size(10.0).strong().color(MUTED)
}

/// Running green, stopped amber, problems red, the rest plain.
fn status_chip(ui: &mut Ui, entry: &EntryView) {
    widgets::pill(ui, |ui| {
        ui.label(RichText::new(entry.status).small().color(color(entry.tint)))
    });
}

/// A row's menu: Move up, Move down, Script options, Remove.
fn menu(ui: &mut Ui, index: usize, entry: &EntryView, count: usize) -> Option<Action> {
    let mut action = None;
    if ui.add_enabled(index > 0, egui::Button::new("↑  Move up")).clicked() {
        action = Some(Action::Move(entry.id.clone(), -1));
    }
    if ui
        .add_enabled(index + 1 < count, egui::Button::new("↓  Move down"))
        .clicked()
    {
        action = Some(Action::Move(entry.id.clone(), 1));
    }
    if ui.button("⚙  Script options").clicked() {
        action = Some(Action::Options(entry.id.clone()));
    }
    ui.separator();
    if ui.button(RichText::new("✕  Remove").color(DANGER)).clicked() {
        action = Some(Action::Remove(entry.id.clone()));
    }
    if action.is_some() {
        ui.close();
    }
    action
}

fn json_dialog() -> FileDialog {
    FileDialog::new()
        .add_filter("JSON Files", &["json"])
        .add_filter("All files", &["*"])
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Show, run};
    use super::*;
    use egui::vec2;

    /// A full playlist with long names stays on screen and needs no scrolling at its size.
    #[test]
    fn playlist_rows_fit_the_window() {
        let dir = std::env::temp_dir().join(format!("turu-scheduler-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bot = crate::turu::bot();
        for name in [
            "Farm.ts",
            "AVeryLongScriptNameThatKeepsGoingAndGoingAndGoing.ts",
            "Farm.ts",
        ] {
            let path = dir.join(name);
            std::fs::write(&path, "").unwrap();
            bot.scheduler.add(&path).unwrap();
        }
        let mut window = SchedulerWindow::default();
        let mut show: Show = Box::new(move |ctx, open| window.show(ctx, open));
        for screen in [vec2(1400.0, 1000.0), vec2(640.0, 420.0)] {
            let (rect, area, overflow) = run(screen, "Scheduler", &mut show);
            assert!(
                area.expand(1.0).contains_rect(rect),
                "{screen:?}: {rect:?} outside {area:?}"
            );
            if screen.y >= 1000.0 {
                assert!(overflow <= 0.5, "scrolls at its default size: {overflow}");
                assert!(rect.width() <= 641.0, "rows widened the window: {rect:?}");
            }
        }
        assert_eq!(bot.scheduler.entries()[2].name, "Farm (2).ts");
    }
}
